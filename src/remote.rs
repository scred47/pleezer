use std::{collections::HashSet, ops::ControlFlow, pin::Pin, time::Duration};

use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use log::Level;
use lru_time_cache::LruCache;
use semver;
use tokio_tungstenite::{
    tungstenite::{
        client::ClientRequestBuilder, protocol::frame::Frame, Message as WebsocketMessage,
    },
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

use crate::{
    config::{Config, Credentials},
    error::{Error, ErrorKind, Result},
    gateway::Gateway,
    player::Player,
    protocol::connect::{
        queue, stream, Body, Channel, Contents, DeviceId, Event, Headers, Message, Percentage,
        QueueItem, RepeatMode, Status, UserId,
    },
    tokens::UserToken,
    track::Track,
};

pub struct Client {
    device_id: DeviceId,
    device_name: String,

    credentials: Credentials,
    gateway: Gateway,
    // TODO : merge with gateway
    user_token: Option<UserToken>,

    scheme: String,
    version: String,
    websocket_tx:
        Option<SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, WebsocketMessage>>,

    subscriptions: HashSet<Event>,

    connection_state: ConnectionState,
    watchdog_rx: Pin<Box<tokio::time::Sleep>>,
    watchdog_tx: Pin<Box<tokio::time::Sleep>>,

    discovery_state: DiscoveryState,
    connection_offers: LruCache<String, DeviceId>,
    interruptions: bool,

    player: Player,
    reporting_timer: Pin<Box<tokio::time::Sleep>>,

    queue: Option<queue::List>,
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
enum DiscoveryState {
    Available,
    Connecting {
        controller: DeviceId,
        ready_message_id: String,
    },
    Taken,
}

#[derive(Clone, Debug, PartialEq)]
enum ConnectionState {
    Disconnected,
    Connected { controller: DeviceId },
}

#[must_use]
fn from_now(seconds: Duration) -> Option<tokio::time::Instant> {
    tokio::time::Instant::now().checked_add(seconds)
}

impl Client {
    const TOKEN_EXPIRATION_THRESHOLD: Duration = Duration::from_secs(60);
    const REPORTING_INTERVAL: Duration = Duration::from_secs(3);
    const WATCHDOG_RX_TIMEOUT: Duration = Duration::from_secs(10);
    const WATCHDOG_TX_TIMEOUT: Duration = Duration::from_secs(5);

    const MESSAGE_SIZE_MAX: usize = 8192;

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - the `app_version` in `config` is not in [`SemVer`] format
    ///
    /// [SemVer]: https://semver.org/
    pub fn new(config: &Config, player: Player, secure: bool) -> Result<Self> {
        // Construct version in the form of `Mmmppp` where:
        // - `M` is the major version
        // - `mm` is the minor version
        // - `ppp` is the patch version
        let semver = semver::Version::parse(&config.app_version)?;
        let major = semver.major;
        let minor = semver.minor;
        let patch = semver.patch;

        // Trim leading zeroes.
        let version = if major > 0 {
            format!("{major}{minor:0>2}{patch:0>3}")
        } else if minor > 0 {
            format!("{minor}{patch:0>3}")
        } else {
            format!("{patch}")
        };
        debug!("remote version: {version}");

        let scheme = if secure { "wss" } else { "ws" };
        debug!("remote scheme: {scheme}");

        // Controllers send discovery requests every two seconds.
        let time_to_live = Duration::from_secs(5);
        let connection_offers = LruCache::with_expiry_duration(time_to_live);

        // Timers are set in the message handlers. They should be moved into
        // a state variant once `select!` supports `if let` statements:
        // https://github.com/tokio-rs/tokio/issues/4173
        let reporting_timer = tokio::time::sleep(Duration::ZERO);
        let watchdog_rx = tokio::time::sleep(Duration::ZERO);
        let watchdog_tx = tokio::time::sleep(Duration::ZERO);

        Ok(Self {
            device_id: config.device_id.into(),
            device_name: config.device_name.clone(),

            credentials: config.credentials.clone(),
            gateway: Gateway::new(config)?,
            user_token: None,

            scheme: scheme.to_owned(),
            version,
            websocket_tx: None,

            subscriptions: HashSet::new(),

            connection_state: ConnectionState::Disconnected,
            watchdog_rx: Box::pin(watchdog_rx),
            watchdog_tx: Box::pin(watchdog_tx),

            player,
            reporting_timer: Box::pin(reporting_timer),

            discovery_state: DiscoveryState::Available,
            connection_offers,
            interruptions: config.interruptions,

            queue: None,
        })
    }

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - the websocket could not be connected to
    /// - sending or receiving messages failed
    pub async fn start(&mut self) -> Result<()> {
        if let Credentials::Login { email, password } = &self.credentials {
            let arl = self.gateway.login(email, password).await?;

            let len = arl.len();
            let min = usize::min(len, 8);
            trace!("redacted arl {}... with {} characters", &arl[0..min], len);
        }

        // Loop until a user token is supplied that expires after the
        // threshold. If rate limiting is necessary, then that should be done
        // by the token token_provider.
        let (user_token, time_to_live) = loop {
            let token = self.gateway.user_token().await?;

            let time_to_live = token
                .time_to_live()
                .checked_sub(Self::TOKEN_EXPIRATION_THRESHOLD);

            if let Some(duration) = time_to_live {
                debug!("user id: {}", token.user_id);
                info!(
                    "user casting quality: {}",
                    self.gateway.audio_quality().unwrap_or_default(),
                );

                // This takes a few milliseconds and would normally
                // truncate (round down). Return `ceil` is more human
                // readable.
                debug!(
                    "user data time to live: {:.0}s",
                    duration.as_secs_f32().ceil(),
                );

                break (token, duration);
            }

            // Flush user tokens that expire within the threshold.
            self.gateway.flush_user_token();
        };

        // Set timer for user token expiration. Wake a short while before
        // actual expiration. This prevents API request errors when the
        // expiration is checked with only a few seconds on the clock.
        let expiry = tokio::time::sleep(time_to_live);
        tokio::pin!(expiry);

        let uri = format!(
            "{}://live.deezer.com/ws/{}?version={}",
            self.scheme, user_token, self.version
        )
        .parse::<http::Uri>()?;
        let mut request = ClientRequestBuilder::new(uri);

        // Decorate the websocket request with the same cookies as the gateway.
        if let Some(cookies) = self.gateway.cookies() {
            if let Ok(cookie_str) = cookies.to_str() {
                request = request.with_header("Cookie", cookie_str);
            } else {
                warn!("unable to set cookie header on websocket");
            }
        }

        let (ws_stream, _) = tokio_tungstenite::connect_async(request).await?;
        let (websocket_tx, mut websocket_rx) = ws_stream.split();
        self.websocket_tx = Some(websocket_tx);

        self.user_token = Some(user_token);

        self.subscribe(Event::Stream).await?;
        self.subscribe(Event::RemoteDiscover).await?;

        info!("ready for discovery");

        let loop_result = loop {
            tokio::select! {
                biased;

                () = &mut self.watchdog_tx, if self.is_connected() => {
                    if let Err(e) = self.send_ping().await {
                        error!("error sending ping: {e}");
                    }
                }

                () = &mut self.watchdog_rx, if self.is_connected() => {
                    error!("controller is not responding");
                    let _drop = self.disconnect().await;
                }

                () = &mut expiry => {
                    break Err(Error::deadline_exceeded("user token expired"));
                }

                () = &mut self.reporting_timer, if self.is_connected() && self.player.playing() => {
                    if let Err(e) = self.report_playback_progress().await {
                        error!("error reporting playback progress: {e}");
                    }
                }

                Some(message) = websocket_rx.next() => {
                    match message {
                        Ok(message) => {
                            // Do not parse exceedingly large messages to
                            // prevent out of memory conditions.
                            let message_size = message.len();
                            if message_size > Self::MESSAGE_SIZE_MAX {
                                error!("ignoring oversized message with {message_size} bytes");
                                continue;
                            }

                            match self.handle_message(&message).await {
                                ControlFlow::Continue(()) => continue,

                                ControlFlow::Break(e) => {
                                    if e.kind == ErrorKind::DeadlineExceeded {
                                        info!("stopping client: {}", e.to_string());
                                        self.gateway.flush_user_token();
                                        break Ok(());
                                    }

                                    break Err(Error::internal(format!("error handling message: {e}")));                                }
                            }
                        }
                        Err(e) => error!("error receiving message: {e}"),
                    }
                }
            }
        };

        self.stop().await;

        loop_result
    }

    fn reset_watchdog_rx(&mut self) {
        if let Some(deadline) = from_now(Self::WATCHDOG_RX_TIMEOUT) {
            self.watchdog_rx.as_mut().reset(deadline);
        }
    }

    fn reset_watchdog_tx(&mut self) {
        if let Some(deadline) = from_now(Self::WATCHDOG_TX_TIMEOUT) {
            self.watchdog_tx.as_mut().reset(deadline);
        }
    }

    fn reset_reporting_timer(&mut self) {
        if let Some(deadline) = from_now(Self::REPORTING_INTERVAL) {
            self.reporting_timer.as_mut().reset(deadline);
        }
    }

    pub async fn stop(&mut self) {
        let _drop = self.disconnect().await;

        // Cancel any remaining subscriptions not handled by `disconnect`.
        let subscriptions = self.subscriptions.clone();
        for event in subscriptions {
            if self.unsubscribe(event).await.is_ok() {
                self.subscriptions.remove(&event);
            }
        }
    }

    fn message(&self, destination: DeviceId, channel: Channel, body: Body) -> Message {
        let contents = Contents {
            event: channel.event,
            headers: Headers {
                from: self.device_id.clone(),
                destination: Some(destination),
            },
            body,
        };

        Message::Send { channel, contents }
    }

    fn command(&self, destination: DeviceId, body: Body) -> Message {
        let remote_command = self.channel(Event::RemoteCommand);
        self.message(destination, remote_command, body)
    }

    fn discover(&self, destination: DeviceId, body: Body) -> Message {
        let remote_discover = self.channel(Event::RemoteDiscover);
        self.message(destination, remote_discover, body)
    }

    fn report_stream(&self, track: &Track) -> Message {
        let contents = stream::Contents {
            action: stream::Action::Play,
            event: stream::Event::Limitation,
            value: stream::Value {
                user: self.user_id(),
                // TODO: keep uuid when song remains the same
                // (e.g. is paused/played)
                uuid: Uuid::new_v4(),
                track_id: track.id(),
            },
        };

        Message::StreamSend {
            channel: self.channel(Event::Stream),
            contents,
        }
    }

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(controller) = self.controller() {
            let close = Body::Close {
                message_id: Uuid::new_v4().into(),
            };

            let command = self.command(controller.clone(), close);
            self.send_message(command).await?;

            self.reset_states();
            return Ok(());
        }

        Err(Error::failed_precondition(
            "disconnect should have an active connection".to_string(),
        ))
    }

    async fn handle_discovery_request(&mut self, from: DeviceId) -> Result<()> {
        // Controllers keep sending discovery requests about every two seconds
        // until it accepts some offer. `connection_offers` implements a LRU
        // cache to evict stale offers.
        let message_id = Uuid::new_v4().to_string();
        self.connection_offers
            .insert(message_id.clone(), from.clone());

        let offer = Body::ConnectionOffer {
            message_id,
            from: self.device_id.clone(),
            device_name: self.device_name.clone(),
        };

        let discover = self.discover(from, offer);
        self.send_message(discover).await
    }

    async fn handle_connect(&mut self, from: DeviceId, offer_id: &str) -> Result<()> {
        let controller = self.connection_offers.remove(offer_id).ok_or_else(|| {
            Error::failed_precondition(format!("connection offer {offer_id} should be active"))
        })?;

        if controller != from {
            return Err(Error::failed_precondition(format!(
                "connection offer for {controller} should be for {from}"
            )));
        }

        if self.discovery_state == DiscoveryState::Taken {
            debug!("not allowing interruptions from {from}");

            // This is a known and valid condition. Return `Ok` so the
            // control flow may continue.
            return Ok(());
        }

        // Subscribe to both channels. If one fails, try to roll back.
        self.subscribe(Event::RemoteQueue).await?;
        if let Err(e) = self.subscribe(Event::RemoteCommand).await {
            let _drop = self.unsubscribe(Event::RemoteQueue).await;
            return Err(e);
        }

        let message_id = Uuid::new_v4().to_string();
        let ready = Body::Ready {
            message_id: message_id.clone(),
        };

        let command = self.command(from.clone(), ready);
        self.send_message(command).await?;

        self.discovery_state = DiscoveryState::Connecting {
            controller: from,
            ready_message_id: message_id,
        };

        Ok(())
    }

    #[must_use]
    fn is_connected(&self) -> bool {
        if let ConnectionState::Connected { .. } = &self.connection_state {
            return true;
        }

        false
    }

    fn controller(&self) -> Option<DeviceId> {
        if let ConnectionState::Connected { controller, .. } = &self.connection_state {
            return Some(controller.clone());
        }

        if let DiscoveryState::Connecting { controller, .. } = &self.discovery_state {
            return Some(controller.clone());
        }

        None
    }

    async fn handle_status(
        &mut self,
        from: DeviceId,
        command_id: &str,
        status: Status,
    ) -> Result<()> {
        if status != Status::OK {
            return Err(Error::failed_precondition(format!(
                "controller failed to process {command_id}"
            )));
        }

        if let DiscoveryState::Connecting {
            controller,
            ready_message_id,
        } = self.discovery_state.clone()
        {
            if from == controller && command_id == ready_message_id {
                if let ConnectionState::Connected { controller, .. } = &self.connection_state {
                    // Evict the active connection.
                    let close = Body::Close {
                        message_id: Uuid::new_v4().into(),
                    };

                    let command = self.command(controller.clone(), close);
                    self.send_message(command).await?;
                }

                if self.interruptions {
                    self.discovery_state = DiscoveryState::Available;
                } else {
                    self.discovery_state = DiscoveryState::Taken;
                }

                self.connection_state = ConnectionState::Connected { controller: from };

                info!("connected to {controller}");
                return Ok(());
            }

            return Err(Error::failed_precondition(
                "should match controller and ready message".to_string(),
            ));
        }

        // Ignore other status messages.
        Ok(())
    }

    async fn handle_close(&mut self) -> Result<()> {
        if self.controller().is_some() {
            self.unsubscribe(Event::RemoteQueue).await?;
            self.unsubscribe(Event::RemoteCommand).await?;

            self.reset_states();
            return Ok(());
        }

        Err(Error::failed_precondition(
            "close should have an active connection".to_string(),
        ))
    }

    fn reset_states(&mut self) {
        if let Some(controller) = self.controller() {
            info!("disconnected from {controller}");
        }

        self.connection_state = ConnectionState::Disconnected;
        self.discovery_state = DiscoveryState::Available;
    }

    async fn handle_publish_queue(&mut self, list: queue::List) -> Result<()> {
        if self.controller().is_some() {
            let queue = self.gateway.list_to_queue(list.clone()).await.unwrap();
            trace!("{queue:#?}");

            self.queue = Some(list);
            self.player.set_queue(queue);

            return Ok(());
        }

        Err(Error::failed_precondition(
            "queue publication should have an active connection".to_string(),
        ))
    }

    async fn send_ping(&mut self) -> Result<()> {
        if let Some(controller) = self.controller() {
            let ping = Body::Ping {
                message_id: Uuid::new_v4().into(),
            };

            let command = self.command(controller.clone(), ping);
            return self.send_message(command).await;
        }

        Err(Error::failed_precondition(
            "ping should have an active connection".to_string(),
        ))
    }

    async fn handle_refresh_queue(&mut self) -> Result<()> {
        if let Some(controller) = self.controller() {
            if let Some(ref queue) = self.queue {
                let contents = Body::PublishQueue {
                    message_id: Uuid::new_v4().into(),
                    queue: queue.clone(),
                };

                let channel = self.channel(Event::RemoteQueue);
                let publish_queue = self.message(controller.clone(), channel, contents);
                self.send_message(publish_queue).await
            } else {
                Err(Error::failed_precondition(
                    "queue refresh should have a published queue".to_string(),
                ))
            }
        } else {
            Err(Error::failed_precondition(
                "queue refresh should have an active connection".to_string(),
            ))
        }
    }

    async fn send_acknowledgement(&mut self, acknowledgement_id: &str) -> Result<()> {
        if let Some(controller) = self.controller() {
            let acknowledgement = Body::Acknowledgement {
                message_id: Uuid::new_v4().into(),
                acknowledgement_id: acknowledgement_id.to_string(),
            };

            let command = self.command(controller, acknowledgement);
            return self.send_message(command).await;
        }

        Err(Error::failed_precondition(
            "acknowledgement should have an active connection".to_string(),
        ))
    }

    #[expect(clippy::too_many_arguments)]
    async fn handle_skip(
        &mut self,
        message_id: &str,
        queue_id: &str,
        item: Option<QueueItem>,
        progress: Option<Percentage>,
        should_play: Option<bool>,
        set_shuffle: Option<bool>,
        set_repeat_mode: Option<RepeatMode>,
        set_volume: Option<Percentage>,
    ) -> Result<()> {
        // Check for controller, not if we are connected: the first `Skip`
        // message is received during the handshake, before the connection is
        // ready.
        if self.controller().is_some() {
            self.send_acknowledgement(message_id).await?;

            self.set_player_state(
                queue_id,
                item,
                progress,
                should_play,
                set_shuffle,
                set_repeat_mode,
                set_volume,
            )
            .await?;

            // Status response to the first skip - received during the
            // handshake - is "1" (Error).
            let status = if self.is_connected() {
                Status::OK
            } else {
                Status::Error
            };

            self.send_status(message_id, status).await?;

            Ok(())
        } else {
            Err(Error::failed_precondition(
                "skip should have an active connection".to_string(),
            ))
        }
    }

    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `progress` could not be set
    /// - playback progress could not be reported
    #[expect(clippy::too_many_arguments)]
    pub async fn set_player_state(
        &mut self,
        queue_id: &str,
        item: Option<QueueItem>,
        progress: Option<Percentage>,
        should_play: Option<bool>,
        set_shuffle: Option<bool>,
        set_repeat_mode: Option<RepeatMode>,
        set_volume: Option<Percentage>,
    ) -> Result<()> {
        // Set the element (track) before setting progress & playback.
        if let Some(item) = item {
            if item.queue_id == queue_id {
                if let Some(ref local) = self.queue {
                    if local.id != queue_id {
                        return Err(Error::failed_precondition(format!(
                            "remote queue {queue_id} does not match local queue {}",
                            local.id
                        )));
                    }
                } else {
                    // Weird but non-fatal - just play a single track then
                    warn!("setting track without a local queue");
                }
                self.player.set_item(item);
            } else {
                return Err(Error::failed_precondition(format!(
                    "queue {queue_id} does not match queue item {item}"
                )));
            }
        }

        if let Some(progress) = progress {
            self.player.set_progress(progress)?;
        }

        if let Some(shuffle) = set_shuffle {
            self.player.set_shuffle(shuffle);
        }

        if let Some(repeat_mode) = set_repeat_mode {
            self.player.set_repeat_mode(repeat_mode);
        }

        if let Some(volume) = set_volume {
            self.player.set_volume(volume);
        }

        if let Some(should_play) = should_play {
            self.player.set_playing(should_play);

            if let Some(track) = self.player.track() {
                // TODO : send message when actually streaming
                let streaming = self.report_stream(track);
                if let Err(e) = self.send_message(streaming).await {
                    // Non-fatal: print the error, but continue processing.
                    error!("unable to notify streaming: {e}");
                }
            } else {
                return Err(Error::failed_precondition(
                    "start playing should have an active track".to_string(),
                ));
            }
        }

        self.report_playback_progress().await
    }

    async fn send_status(&mut self, command_id: &str, status: Status) -> Result<()> {
        if let Some(controller) = self.controller() {
            let status = Body::Status {
                message_id: Uuid::new_v4().into(),
                command_id: command_id.to_string(),
                status,
            };

            let command = self.command(controller.clone(), status);
            return self.send_message(command).await;
        }

        Err(Error::failed_precondition(
            "status should have an active connection".to_string(),
        ))
    }

    async fn report_playback_progress(&mut self) -> Result<()> {
        // Reset the timer regardless of success or failure, to prevent getting
        // stuck in a reporting state.
        self.reset_reporting_timer();

        if let Some(controller) = self.controller() {
            if let Some(track) = &self.player.track() {
                let progress = Body::PlaybackProgress {
                    message_id: Uuid::new_v4().into(),
                    track: track.item().clone(),
                    // TODO: use actual track quality
                    quality: self.gateway.audio_quality().unwrap_or_default(),
                    duration: track.duration(),
                    buffered: track.buffered(),
                    progress: self.player.progress().unwrap_or_default(),
                    volume: self.player.volume(),
                    is_playing: self.player.playing(),
                    is_shuffle: self.player.shuffle(),
                    repeat_mode: self.player.repeat_mode(),
                };

                let command = self.command(controller.clone(), progress);
                self.send_message(command).await?;

                Ok(())
            } else {
                Err(Error::failed_precondition(
                    "playback progress should have active track".to_string(),
                ))
            }
        } else {
            Err(Error::failed_precondition(
                "playback progress should have an active connection".to_string(),
            ))
        }
    }

    async fn handle_message(&mut self, message: &WebsocketMessage) -> ControlFlow<Error, ()> {
        match message {
            WebsocketMessage::Text(message) => {
                match serde_json::from_str::<Message>(message) {
                    Ok(message) => {
                        match message.clone() {
                            Message::Receive { contents, .. } => {
                                let from = contents.headers.from;

                                // Ignore echoes of own messages.
                                if from == self.device_id {
                                    return ControlFlow::Continue(());
                                }

                                // Ignore messages directed at others.
                                if let Some(destination) = contents.headers.destination {
                                    if destination != self.device_id {
                                        return ControlFlow::Continue(());
                                    }
                                }

                                if let Some(controller) = self.controller() {
                                    if controller == from {
                                        self.reset_watchdog_rx();
                                    }
                                }

                                if log_enabled!(Level::Trace) {
                                    trace!("{message:#?}");
                                } else {
                                    debug!("{message}");
                                }

                                if let Err(e) = self.dispatch(from, contents.body).await {
                                    error!("error handling message: {e}");
                                }
                            }

                            // Ignore streaming information from others.
                            Message::StreamReceive { .. } => return ControlFlow::Continue(()),

                            _ => {
                                trace!("ignoring unexpected message: {message:#?}");
                            }
                        }
                    }

                    Err(e) => {
                        error!("error parsing message: {e}");
                        debug!("{message:#?}");
                    }
                }
            }

            // Deezer Connect sends pings as text message payloads, but so far
            // not as websocket frames. Aim for RFC 6455 compliance anyway.
            WebsocketMessage::Ping(payload) => {
                debug!("ping -> pong");
                let pong = Frame::pong(payload.clone());
                if let Err(e) = self.send_frame(WebsocketMessage::Frame(pong)).await {
                    error!("{e}");
                }
            }

            WebsocketMessage::Close(payload) => {
                return ControlFlow::Break(Error::aborted(format!(
                    "connection closed by server: {payload:?}"
                )))
            }

            _ => {
                trace!("ignoring unimplemented frame: {message:#?}");
            }
        }

        ControlFlow::Continue(())
    }

    async fn dispatch(&mut self, from: DeviceId, body: Body) -> Result<()> {
        match body {
            // TODO - Think about maintaining a queue of message IDs to be
            // acknowledged, evictingt them one by one.
            Body::Acknowledgement { .. } => Ok(()),

            Body::Close { .. } => self.handle_close().await,

            Body::Connect { from, offer_id, .. } => self.handle_connect(from, &offer_id).await,

            Body::DiscoveryRequest { from, .. } => self.handle_discovery_request(from).await,

            // Pings don't use dedicated WebSocket frames, but are sent as
            // normal data. An acknowledgement serves as pong.
            Body::Ping { message_id } => self.send_acknowledgement(&message_id).await,

            Body::PublishQueue { queue, .. } => self.handle_publish_queue(queue).await,

            Body::RefreshQueue { .. } => self.handle_refresh_queue().await,

            Body::Skip {
                message_id,
                queue_id,
                track,
                progress,
                should_play,
                set_shuffle,
                set_repeat_mode,
                set_volume,
            } => {
                self.handle_skip(
                    &message_id,
                    &queue_id,
                    track,
                    progress,
                    should_play,
                    set_shuffle,
                    set_repeat_mode,
                    set_volume,
                )
                .await
            }

            Body::Status {
                command_id, status, ..
            } => self.handle_status(from, &command_id, status).await,

            Body::Stop { .. } => {
                self.player.stop();
                Ok(())
            }

            Body::ConnectionOffer { .. } | Body::PlaybackProgress { .. } | Body::Ready { .. } => {
                trace!("ignoring message intended for a controller");
                Ok(())
            }
        }
    }

    async fn send_frame(&mut self, frame: WebsocketMessage) -> Result<()> {
        match &mut self.websocket_tx {
            Some(tx) => tx.send(frame).await.map_err(Into::into),
            None => Err(Error::unavailable(
                "websocket stream unavailable".to_string(),
            )),
        }
    }

    async fn send_message(&mut self, message: Message) -> Result<()> {
        // Reset the timer regardless of success or failure, to prevent getting
        // stuck in a reporting state.
        self.reset_watchdog_tx();

        if log_enabled!(Level::Trace) {
            trace!("{message:#?}");
        } else {
            debug!("{message}");
        }

        let json = serde_json::to_string(&message)?;
        let frame = WebsocketMessage::Text(json);
        self.send_frame(frame).await
    }

    async fn subscribe(&mut self, event: Event) -> Result<()> {
        if !self.subscriptions.contains(&event) {
            let channel = self.channel(event);

            let subscribe = Message::Subscribe { channel };
            self.send_message(subscribe).await?;

            self.subscriptions.insert(event);
        }

        Ok(())
    }

    async fn unsubscribe(&mut self, event: Event) -> Result<()> {
        if self.subscriptions.contains(&event) {
            let channel = self.channel(event);

            let unsubscribe = Message::Unsubscribe { channel };
            self.send_message(unsubscribe).await?;

            self.subscriptions.remove(&event);
        }

        Ok(())
    }

    #[must_use]
    fn user_id(&self) -> UserId {
        self.user_token
            .as_ref()
            .map_or(UserId::Unspecified, |token| UserId::Id(token.user_id))
    }

    #[must_use]
    fn channel(&self, event: Event) -> Channel {
        let user_id = self.user_id();
        let from = if let Event::UserFeed(_) = event {
            UserId::Unspecified
        } else {
            user_id
        };

        Channel {
            from,
            to: user_id,
            event,
        }
    }
}
