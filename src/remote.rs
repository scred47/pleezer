use std::{num::NonZeroU64, ops::ControlFlow, time::Duration};

use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use semver;
use thiserror::Error;
use tokio_tungstenite::{
    tungstenite::{protocol::frame::Frame, Message as WebsocketMessage},
    MaybeTlsStream, WebSocketStream,
};

use crate::{
    config::{self, Config},
    protocol, token,
};

pub type ClientResult<T> = Result<T, ClientError>;

// TODO: implement Debug manually
pub struct Client {
    user_token: Option<token::UserToken>,
    provider: Box<dyn token::UserTokenProvider>,

    scheme: String,
    version: String,
    ws_tx:
        Option<SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, WebsocketMessage>>,
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("error parsing app version: {0}")]
    SemverError(#[from] semver::Error),
    #[error("invalid connection: {0}")]
    ConnectionError(String),
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("user token error: {0}")]
    UserTokenError(#[from] token::UserTokenError),
    #[error("websocket error: {0}")]
    WebSocketError(#[from] tokio_tungstenite::tungstenite::Error),
}

impl Client {
    pub fn new<P>(config: &Config, provider: P, secure: bool) -> ClientResult<Self>
    where
        P: token::UserTokenProvider + 'static,
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

        Ok(Self {
            provider: Box::new(provider),
            user_token: None,
            scheme: scheme.to_owned(),
            version,
            ws_tx: None,
        })
    }

    pub async fn start(&mut self) -> ClientResult<()> {
        let user_token = self.provider.user_token().await?;

        // Token must be a base62 encoded string of 64 characters.
        let token = user_token.as_str();
        let count = token.chars().count();
        if count != 64 || token.contains(|chr| chr < '0' || chr > 'z') {
            return Err(token::UserTokenError::Invalid(format!(
                "user token invalid ({count} characters)"
            ))
            .into());
        }

        // Set timer for user token expiration. Wake a short while before
        // actual expiration. This prevents API request errors when the
        // expiration is checked with only a few seconds on the clock.
        let expiry = tokio::time::Instant::from_std(user_token.expires_at());
        const EXPIRATION_THRESHOLD: Duration = Duration::from_secs(60);
        let expiry = tokio::time::sleep_until(expiry.checked_sub(EXPIRATION_THRESHOLD).ok_or(
            token::UserTokenError::Invalid("expiration out of bounds".to_string()),
        )?);
        tokio::pin!(expiry);

        let user_id = user_token.user_id();
        self.user_token = Some(user_token.to_owned());

        let url = format!(
            "{}://live.deezer.com/ws/{}?version={}",
            self.scheme, token, self.version
        );
        let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
        let (ws_tx, mut ws_rx) = ws_stream.split();
        self.ws_tx = Some(ws_tx);

        self.subscribe(user_id, user_id, "REMOTEDISCOVER").await?;
        info!("ready for discovery");

        let bonus = r#"["msg","4787654542_4787654542_REMOTEDISCOVER",{"APP":"REMOTEDISCOVER","body":"{\"messageId\":\"56E10C0A-ABF2-4D1C-BF12-80858AFB1AE7\",\"protocolVersion\":\"com.deezer.remote.discovery.proto1\",\"payload\":\"eyJmcm9tIjoieTRhOTcxZWNmMTFjOTE2ZjY2MWUxMzI4ZDM1YWY2NWQ3IiwicGFyYW1zIjp7ImRpc2NvdmVyeV9zZXNzaW9uIjoiRUI5MDRBM0UtNTc3RS00QTI1LTlGNEItOTA5RkY2RUMwMDVDIn19\",\"messageType\":\"discoveryRequest\",\"clock\":{}}","headers":{"from":"y4a971ecf11c916f661e1328d35af65d7"}}]"#;
 // let bonus = r#"["msg","4787654542_4787654542_REMOTEDISCOVER",{"APP":"REMOTEDISCOVER","body":"{\"messageId\":\"b7a54826-7688-4f6f-811c-a7214ddaef65\",\"messageType\":\"connectionOffer\",\"protocolVersion\":\"com.deezer.remote.discovery.proto1\",\"clock\":{},\"payload\":\"eyJmcm9tIjoiMzVhOWY4MTItYTMzMC00ZmY5LTkzZGItNDc1NDNmZGYzZGZiIiwicGFyYW1zIjp7ImRldmljZV9uYW1lIjoiUm9kZXJpY2tzLWlNYWMtMy5sb2NhbCIsImRldmljZV90eXBlIjoid2ViIiwic3VwcG9ydGVkX2NvbnRyb2xfdmVyc2lvbnMiOlsiMS4wLjAtYmV0YTIiXX19\"}","headers":{"destination":"y4a971ecf11c916f661e1328d35af65d7","from":"35a9f812-a330-4ff9-93db-47543fdf3dfb"}}]"#;
        let f = serde_json::from_str::<protocol::connect::Message>(&bonus);
        trace!("{f:#?}");

        loop {
            tokio::select! {
                () = &mut expiry => {
                    // Flush the user token so that it is refreshed in case
                    // this remote client is restarted by the caller.
                    self.provider.flush_user_token();
                    return Err(ClientError::ConnectionError(format!("user token expired")));
                }
                Some(message) = ws_rx.next() => {
                    match message {
                        Ok(message) => {
                            // Do not parse exceedingly large messages to
                            // prevent out of memory conditions.
                            let message_size = message.len();
                            if message_size > 8192 {
                                error!("ignoring oversized message with {message_size} bytes");
                            }

                            if let ControlFlow::Break(e) = self.handle_message(&message).await {
                                return Err(ClientError::ConnectionError(format!("error handling message: {e}")));
                            }
                        }
                        Err(e) => error!("error receiving message: {e}"),
                    }
                }
            }
        }
    }

    async fn handle_message(&mut self, message: &WebsocketMessage) -> ControlFlow<ClientError, ()> {
        let result = match message {
            WebsocketMessage::Text(message) => {
                match serde_json::from_str::<protocol::connect::Message>(message) {
                    Ok(message) => {
                        // if let Some(encoded) = &message.contents().body.payload {
                        //     match base64::decode(encoded) {
                        //         Ok(decoded) => {
                        //             let payload = serde_json::from_slice::<RemotePayload>(&decoded);
                        //             trace!("payload: {payload:?}");
                        //         },
                        //         Err(e) => trace!("error decoding base64: {e}"),
                        //     }
                        // }
                    }
                    Err(e) => {
                        trace!("{message:#?}");
                        error!("error parsing message: {e}");
                    }
                }

                Ok(())
            }
            // Deezer Connect sends pings as text message payloads, but
            // seemingly not as websocket frames. Aim for RFC compliance
            // anyway.
            WebsocketMessage::Ping(payload) => {
                trace!("ping -> pong");
                let pong = Frame::pong(payload.clone());
                self.send_message(WebsocketMessage::Frame(pong)).await
            }
            WebsocketMessage::Close(payload) => Err(ClientError::ConnectionError(format!(
                "connection closed by server: {payload:?}"
            ))),
            _ => {
                trace!("message type unimplemented");
                Ok(())
            }
        };

        if let Err(e) = result {
            ControlFlow::Break(e)
        } else {
            ControlFlow::Continue(())
        }
    }

    async fn send_message(&mut self, message: WebsocketMessage) -> ClientResult<()> {
        trace!("sending message: {message:?}");
        match &mut self.ws_tx {
            Some(tx) => tx.send(message).await.map_err(Into::into),
            None => Err(ClientError::ConnectionError(format!(
                "websocket stream unavailable"
            ))),
        }
    }

    async fn send_text(&mut self, command: &str, payload: &str) -> ClientResult<()> {
        let text = format!("[\"{command}\",\"{payload}\"]");
        self.send_message(WebsocketMessage::Text(text)).await
    }

    async fn subscribe(
        &mut self,
        from: NonZeroU64,
        to: NonZeroU64,
        subscription: &str,
    ) -> ClientResult<()> {
        let payload = format!("{from}_{to}_{subscription}");
        self.send_text("sub", &payload).await
    }
}
