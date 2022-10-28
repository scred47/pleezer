use std::{ops::ControlFlow, time::SystemTime};

use futures_util::{stream::SplitSink, SinkExt, StreamExt};
use thiserror::Error;
use tokio_tungstenite::{
    tungstenite::{protocol::frame::Frame, Message},
    MaybeTlsStream, WebSocketStream,
};

use crate::{
    config::{self, Config},
    protocol::websocket::*,
    token,
};

pub type ClientResult<T> = Result<T, ClientError>;

// TODO: implement Debug manually
pub struct Client {
    user_token: Option<token::UserToken>,
    provider: Box<dyn token::UserTokenProvider>,

    scheme: String,
    version: String,
    ws_tx: Option<SplitSink<WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>, Message>>,
}

#[derive(Error, Debug)]
pub enum ClientError {
    #[error("invalid configuration: {0}")]
    ConfigError(#[from] config::ConfigError),
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
        let semver = config.semver()?;
        let major = semver[0];
        let minor = semver[1];
        let patch = semver[2];

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
        let user_id = user_token.user_id();
        let token = user_token.as_str();
        self.user_token = Some(user_token.to_owned());

        // Token must be a base62 encoded string of 64 characters.
        let count = token.chars().count();
        if count != 64 || token.contains(|chr| chr < '0' || chr > 'z') {
            return Err(token::UserTokenError::Invalid(format!(
                "user token invalid ({count} characters)"
            ))
            .into());
        }

        // Set timer for user token expiration.
        let time_to_live = user_token
            .expires_at()
            .duration_since(SystemTime::now())
            .map_err(|e| ClientError::InvalidData(format!("system time error: {e}")))?;
        let expiry = tokio::time::sleep(time_to_live);
        tokio::pin!(expiry);

        let url = format!(
            "{}://live.deezer.com/ws/{}?version={}",
            self.scheme, token, self.version
        );
        let (ws_stream, _) = tokio_tungstenite::connect_async(url).await?;
        let (ws_tx, mut ws_rx) = ws_stream.split();
        self.ws_tx = Some(ws_tx);

        self.subscribe(user_id, user_id, "REMOTEDISCOVER").await?;
        info!("ready for discovery");

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
                            trace!("message received: {message}");
                            if let ControlFlow::Break(e) = self.handle_message(message).await {
                                return Err(ClientError::ConnectionError(format!("error handling message: {e}")));
                            }
                        }
                        Err(e) => error!("error receiving message: {e}"),
                    }
                }
            }
        }
    }

    async fn handle_message(&mut self, message: Message) -> ControlFlow<ClientError, ()> {
        let result = match message {
            Message::Text(message) => {
                match serde_json::from_str::<RemoteMessage>(&message) {
                    Ok(message) => {
                        trace!("message: {message:?}");
                        if let Ok(body) = message.contents().body() {
                            trace!("payload: {:?}", body.payload());
                        }
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
                    Err(e) => trace!("error parsing message: {e}"),
                }

                Ok(())
            }
            Message::Ping(payload) => {
                trace!("ping -> pong");
                let pong = Frame::pong(payload.clone());
                self.send_message(Message::Frame(pong)).await
            }
            Message::Close(payload) => Err(ClientError::ConnectionError(format!(
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

    async fn send_message(&mut self, message: Message) -> ClientResult<()> {
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
        self.send_message(Message::Text(text)).await
    }

    async fn subscribe(&mut self, from: u64, to: u64, subscription: &str) -> ClientResult<()> {
        let payload = format!("{from}_{to}_{subscription}");
        self.send_text("sub", &payload).await
    }
}
