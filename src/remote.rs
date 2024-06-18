use std::{collections::HashSet, ops::ControlFlow, pin::Pin, time::Duration};

use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use lru_time_cache::LruCache;
use semver;
use thiserror::Error;
use tokio_tungstenite::{
    tungstenite::{protocol::frame::Frame, Message as WebsocketMessage},
    MaybeTlsStream, WebSocketStream,
};
use uuid::Uuid;

use crate::{
    config::Config,
    player,
    protocol::connect::{
        queue, Body, Channel, Contents, DeviceId, Element, Event, Headers, Message, Percentage,
        RepeatMode, Status, UserId,
    },
    tokens::{UserToken, UserTokenError, UserTokenProvider},
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("connection error: {0}")]
    Connection(String),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Protocol(String),

    #[error("error parsing app version: {0}")]
    Semver(#[from] semver::Error),

    #[error("user token error: {0}")]
    UserToken(#[from] UserTokenError),

    #[error("websocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),
}

// TODO: implement Debug manually to not print the user_token
pub struct Client {
    device_id: DeviceId,
    device_name: String,

    user_token: Option<UserToken>,
    token_provider: Box<dyn UserTokenProvider>,

    scheme: String,
    version: String,
    websocket_tx:
        Option<SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, WebsocketMessage>>,

    subscriptions: HashSet<Event>,

    connection_state: ConnectionState,
    watchdog_rx: Pin<Box<tokio::time::Sleep>>,
    watchdog_tx: Pin<Box<tokio::time::Sleep>>,

    discovery_state: DiscoveryState,
    connection_offers: LruCache<Uuid, DeviceId>,
    interruptions: bool,

    player: player::Player,
    reporting_timer: Pin<Box<tokio::time::Sleep>>,
}

#[derive(Clone, Debug, PartialOrd, Ord, PartialEq, Eq)]
enum DiscoveryState {
    Available,
    Connecting {
        controller: DeviceId,
        ready_message_id: Uuid,
        skip_message_id: Option<Uuid>,
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
    const REPORTING_INTERVAL: Duration = Duration::from_secs(2);
    const WATCHDOG_RX_TIMEOUT: Duration = Duration::from_secs(10);
    const WATCHDOG_TX_TIMEOUT: Duration = Duration::from_secs(5);

    /// todo
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - the `app_version` in `config` is not in [`SemVer`] format
    ///
    /// [SemVer]: https://semver.org/
    pub fn new<P>(
        config: &Config,
        token_provider: P,
        player: player::Player,
        secure: bool,
    ) -> Result<Self>
    where
        P: UserTokenProvider + 'static,
    {
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

            token_provider: Box::new(token_provider),
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
        })
    }

    /// todo
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - the websocket could not be connected to
    /// - sending or receiving messages failed
    pub async fn start(&mut self) -> Result<()> {
        // Loop until an user token is supplied that expires after the
        // threshold. If rate limiting is necessary, then that should be done
        // by the token token_provider.
        let (user_token, time_to_live) = loop {
            let token = self.token_provider.user_token().await?;

            let time_to_live = token
                .time_to_live()
                .checked_sub(Self::TOKEN_EXPIRATION_THRESHOLD);
            if let Some(duration) = time_to_live {
                break (token, duration);
            }

            // Flush user tokens that expire within the threshold.
            self.token_provider.flush_user_token();
        };

        // Set timer for user token expiration. Wake a short while before
        // actual expiration. This prevents API request errors when the
        // expiration is checked with only a few seconds on the clock.
        let expiry = tokio::time::sleep(time_to_live);
        tokio::pin!(expiry);

        let url = format!(
            "{}://live.deezer.com/ws/{}?version={}",
            self.scheme, user_token, self.version
        );

        self.user_token = Some(user_token);

        let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
        let (websocket_tx, mut websocket_rx) = ws_stream.split();
        self.websocket_tx = Some(websocket_tx);

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
                    break Err(UserTokenError::Refresh.into());
                }

                () = &mut self.reporting_timer, if self.is_connected() => {
                    if let Err(e) = self.report_playback_progress().await {
                        error!("{e}");
                    }
                }

                Some(message) = websocket_rx.next() => {
                    match message {
                        Ok(message) => {
                            // Do not parse exceedingly large messages to
                            // prevent out of memory conditions.
                            let message_size = message.len();
                            if message_size > 8192 {
                                error!("ignoring oversized message with {message_size} bytes");
                            }

                            match self.handle_message(&message).await {
                                ControlFlow::Continue(_) => continue,

                                ControlFlow::Break(Error::UserToken(UserTokenError::Refresh)) => {
                                    info!("stopping client: {}", UserTokenError::Refresh);
                                    self.token_provider.flush_user_token();
                                    break Ok(());
                                }

                                ControlFlow::Break(e) => break Err(Error::Protocol(format!("error handling message: {e}"))),
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

    async fn disconnect(&mut self) -> Result<()> {
        if let Some(controller) = self.controller() {
            let close = Body::Close {
                message_id: Uuid::new_v4(),
            };

            let command = self.command(controller.clone(), close);
            self.send_message(command).await?;

            self.reset_states();
            return Ok(());
        }

        Err(Error::Protocol(
            "disconnect should have an active connection".to_string(),
        ))
    }

    async fn handle_discovery_request(&mut self, from: DeviceId) -> Result<()> {
        // Controllers keep sending discovery requests about every two seconds
        // until it accepts some offer. `connection_offers` implements a LRU
        // cache to evict stale offers.
        let message_id = Uuid::new_v4();
        self.connection_offers.insert(message_id, from.clone());

        let offer = Body::ConnectionOffer {
            message_id,
            from: self.device_id.clone(),
            device_name: self.device_name.clone(),
        };

        let discover = self.discover(from, offer);
        self.send_message(discover).await
    }

    async fn handle_connect(&mut self, from: DeviceId, offer_id: Uuid) -> Result<()> {
        let controller = self.connection_offers.remove(&offer_id).ok_or_else(|| {
            Error::Protocol(format!("connection offer {offer_id} should be active"))
        })?;

        if controller != from {
            return Err(Error::Protocol(format!(
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

        let message_id = Uuid::new_v4();
        let ready = Body::Ready { message_id };

        let command = self.command(from.clone(), ready);
        self.send_message(command).await?;

        self.discovery_state = DiscoveryState::Connecting {
            controller: from,
            ready_message_id: message_id,
            skip_message_id: None,
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

        return None;
    }

    async fn handle_status(
        &mut self,
        from: DeviceId,
        command_id: Uuid,
        status: Status,
    ) -> Result<()> {
        if status != Status::OK {
            return Err(Error::Protocol(format!(
                "controller failed to process {command_id}"
            )));
        }

        if let DiscoveryState::Connecting {
            controller,
            ready_message_id,
            skip_message_id,
        } = self.discovery_state.clone()
        {
            if from == controller && command_id == ready_message_id {
                if skip_message_id.is_some() {
                    if let ConnectionState::Connected { controller, .. } = &self.connection_state {
                        // Evict the active connection.
                        let close = Body::Close {
                            message_id: Uuid::new_v4(),
                        };

                        let command = self.command(controller.clone(), close);
                        self.send_message(command).await?;
                    }

                    self.report_playback_progress().await?;

                    if self.interruptions {
                        self.discovery_state = DiscoveryState::Available;
                    } else {
                        self.discovery_state = DiscoveryState::Taken;
                    }

                    self.connection_state = ConnectionState::Connected { controller: from };

                    info!("connected to {controller}");
                    return Ok(());
                } else {
                    return Err(Error::Protocol(
                        "should have received skip before initial status".to_string(),
                    ));
                }
            } else {
                return Err(Error::Protocol(
                    "should match controller and ready message".to_string(),
                ));
            }
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

        Err(Error::Protocol(
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

    async fn handle_publish_queue(&mut self, queue: queue::List) -> Result<()> {
        if self.controller().is_some() {
            self.player.set_queue(queue.clone());

            return Ok(());
        }

        Err(Error::Protocol(
            "queue publication should have an active connection".to_string(),
        ))
    }

    async fn send_ping(&mut self) -> Result<()> {
        if let Some(controller) = self.controller() {
            let ping = Body::Ping {
                message_id: Uuid::new_v4(),
            };

            let command = self.command(controller.clone(), ping);
            return self.send_message(command).await;
        }

        Err(Error::Protocol(
            "ping should have an active connection".to_string(),
        ))
    }

    async fn handle_refresh_queue(&mut self) -> Result<()> {
        if let Some(controller) = self.controller() {
            if let Some(queue) = self.player.queue() {
                let queue = Body::PublishQueue {
                    message_id: Uuid::new_v4(),
                    queue,
                };

                let remote_queue = self.channel(Event::RemoteQueue);
                let queue = self.message(controller.clone(), remote_queue, queue);
                self.send_message(queue).await?;

                return self.report_playback_progress().await;
            } else {
                return Err(Error::Protocol(
                    "queue refresh should have a published queue".to_string(),
                ));
            }
        }

        Err(Error::Protocol(
            "queue refresh should have an active connection".to_string(),
        ))
    }

    async fn send_acknowledgement(&mut self, acknowledgement_id: Uuid) -> Result<()> {
        if let Some(controller) = self.controller() {
            trace!("acking {acknowledgement_id}");

            let acknowledgement = Body::Acknowledgement {
                message_id: Uuid::new_v4(),
                acknowledgement_id,
            };

            let command = self.command(controller, acknowledgement);
            return self.send_message(command).await;
        }

        Err(Error::Protocol(
            "acknowledgement should have an active connection".to_string(),
        ))
    }

    async fn handle_skip(
        &mut self,
        message_id: Uuid,
        queue_id: Uuid,
        element: Option<Element>,
        progress: Option<Percentage>,
        should_play: Option<bool>,
        set_shuffle: Option<bool>,
        set_repeat_mode: Option<RepeatMode>,
        set_volume: Option<Percentage>,
    ) -> Result<()> {
        if self.controller().is_some() {
            self.send_acknowledgement(message_id).await?;

            let status = match self.player.set_state(
                queue_id,
                element,
                progress,
                should_play,
                set_shuffle,
                set_repeat_mode,
                set_volume,
            ) {
                Ok(_) => Status::OK,
                Err(e) => {
                    error!("{e}");
                    Status::Error
                }
            };

            if let ConnectionState::Connected { .. } = &self.connection_state {
                return self.send_status(message_id, status).await;
            }

            if let DiscoveryState::Connecting {
                controller,
                ready_message_id,
                ..
            } = self.discovery_state.clone()
            {
                self.discovery_state = DiscoveryState::Connecting {
                    controller,
                    ready_message_id,
                    skip_message_id: Some(message_id),
                };
            }

            return Ok(());
        }

        Err(Error::Protocol(
            "skip should have an active connection".to_string(),
        ))
    }

    async fn send_status(&mut self, command_id: Uuid, status: Status) -> Result<()> {
        if let Some(controller) = self.controller() {
            trace!("reporting status for {command_id}");

            let status = Body::Status {
                message_id: Uuid::new_v4(),
                command_id,
                status,
            };

            let command = self.command(controller.clone(), status);
            return self.send_message(command).await;
        }

        Err(Error::Protocol(
            "status should have an active connection".to_string(),
        ))
    }

    async fn report_playback_progress(&mut self) -> Result<()> {
        // Reset the timer regardless of success or failure, to prevent getting
        // stuck in a reporting state.
        self.reset_reporting_timer();

        if let Some(controller) = self.controller() {
            if let Some(track) = &self.player.track {
                let progress = Body::PlaybackProgress {
                    message_id: Uuid::new_v4(),
                    track: track.element,
                    quality: track.quality,
                    duration: track.duration,
                    buffered: track.buffered,
                    progress: track.progress(),
                    volume: self.player.volume(),
                    is_playing: self.player.playing,
                    is_shuffle: self.player.shuffle,
                    repeat_mode: self.player.repeat_mode,
                };

                let command = self.command(controller.clone(), progress);
                self.send_message(command).await?;

                return Ok(());
            } else {
                return Err(Error::Protocol(
                    "playback progress should have active track".to_string(),
                ));
            }
        }

        Err(Error::Protocol(
            "playback progress should have an active connection".to_string(),
        ))
    }

    async fn handle_acknowledgement(&mut self, acknowledgement_id: Uuid) -> Result<()> {
        trace!("controller acknowledged {acknowledgement_id}");
        Ok(())
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

                                debug!("{message}");

                                if let Some(controller) = self.controller() {
                                    if controller == from {
                                        self.reset_watchdog_rx();
                                    }
                                }

                                let result = match contents.body {
                                    Body::Acknowledgement {
                                        acknowledgement_id, ..
                                    } => self.handle_acknowledgement(acknowledgement_id).await,

                                    Body::Close { .. } => self.handle_close().await,

                                    Body::Connect { from, offer_id, .. } => {
                                        self.handle_connect(from, offer_id).await
                                    }

                                    Body::DiscoveryRequest { from, .. } => {
                                        self.handle_discovery_request(from).await
                                    }

                                    Body::Ping { .. } => Ok(()),

                                    Body::PublishQueue { queue, .. } => {
                                        self.handle_publish_queue(queue).await
                                    }

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
                                            message_id,
                                            queue_id,
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
                                    } => self.handle_status(from, command_id, status).await,

                                    Body::Stop { .. } => {
                                        self.player.stop();
                                        Ok(())
                                    }

                                    Body::ConnectionOffer { .. }
                                    | Body::PlaybackProgress { .. }
                                    | Body::Ready { .. } => {
                                        trace!("ignoring message intended for a controller");
                                        Ok(())
                                    }
                                };

                                if let Err(e) = result {
                                    error!("error handling message: {e}");
                                }
                            }

                            _ => {
                                trace!("ignoring unexpected message: {message}");
                            }
                        }
                    }

                    Err(e) => {
                        trace!("{message:#?}");
                        error!("error parsing message: {e}");
                    }
                }
            }

            // Deezer Connect sends pings as text message payloads, but so far
            // not as websocket frames. Aim for RFC compliance anyway.
            WebsocketMessage::Ping(payload) => {
                debug!("ping -> pong");
                let pong = Frame::pong(payload.clone());
                if let Err(e) = self.send_frame(WebsocketMessage::Frame(pong)).await {
                    error!("{e}");
                }
            }

            WebsocketMessage::Close(payload) => {
                return ControlFlow::Break(Error::Connection(format!(
                    "connection closed by server: {payload:?}"
                )))
            }

            _ => {
                trace!("ignoring unimplemented frame: {message:#?}");
            }
        }

        ControlFlow::Continue(())
    }

    async fn send_frame(&mut self, frame: WebsocketMessage) -> Result<()> {
        match &mut self.websocket_tx {
            Some(tx) => tx.send(frame).await.map_err(Into::into),
            None => Err(Error::Connection(
                "websocket stream unavailable".to_string(),
            )),
        }
    }

    async fn send_message(&mut self, message: Message) -> Result<()> {
        debug!("{message}");

        // Reset the timer regardless of success or failure, to prevent getting
        // stuck in a reporting state.
        self.reset_watchdog_tx();

        let json = serde_json::to_string(&message)?;
        trace!("{json:#?}");
        let frame = WebsocketMessage::Text(json);
        self.send_frame(frame).await
    }

    async fn subscribe(&mut self, event: Event) -> Result<()> {
        if self.subscriptions.get(&event).is_none() {
            let channel = self.channel(event);

            let subscribe = Message::Subscribe { channel };
            self.send_message(subscribe).await?;

            self.subscriptions.insert(event);
        }

        Ok(())
    }

    async fn unsubscribe(&mut self, event: Event) -> Result<()> {
        if self.subscriptions.get(&event).is_some() {
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
