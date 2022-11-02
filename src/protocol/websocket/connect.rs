use std::{
    collections::HashMap,
    fmt::{self, Write},
    num::{self, NonZeroU64},
    str::FromStr,
};

use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use serde_with::{json::JsonString, serde_as, TryFromInto};
use thiserror::Error;

use super::*;

/// A Deezer Connect websocket message.
///
/// Messages have a certain type and belong to a certain application.
/// Applications can be thought of as scoped message streams to subscribe to.
#[derive(Clone, PartialEq, Deserialize, Serialize, Debug)]
pub struct Message(String, String, Option<MessageContents>);

#[derive(Error, Debug)]
pub enum MessageError {
    #[error("formatting error")]
    FormatError(#[from] fmt::Error),
    #[error("invalid data: {0}")]
    InvalidData(String),
    #[error("error parsing json: {0}")]
    JsonError(#[from] serde_json::Error),
    #[error("error parsing integer: {0}")]
    ParseIntError(#[from] num::ParseIntError),
}

pub type MessageResult<T> = Result<T, MessageError>;

impl Message {
    /// Returns a new Deezer Connect websocket message
    ///
    /// # Parameters
    ///
    /// - `typ`: an enum variant that represents the message type
    /// - `from`: an enum variant that represents the sender
    /// - `dest`: an enum variant that represents the receiver
    /// - `app`: an enum variant that represents the message stream
    /// - `contents`: an optional struct that holds the message contents
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `typ` is `MessageType::Log` with no `contents`
    /// - `typ` is `MessageType::Sub` or `MessageType::Unsub` with some `contents`
    ///
    /// # Examples
    pub fn new(
        typ: MessageType,
        from: User,
        dest: User,
        app: App,
        contents: Option<MessageContents>,
    ) -> MessageResult<Self> {
        // `Log` messages are formatted differently: they consist of a tuple of
        // only the message type and contents as a JSON string.
        if typ == MessageType::Log {
            let contents = contents
                .ok_or_else(|| MessageError::InvalidData("log without contents".to_string()))?;
            let json = serde_json::to_string(&contents)?;
            return Ok(Self(typ.to_string(), json, None));
        }

        if matches!(typ, MessageType::Sub | MessageType::Unsub) && contents.is_some() {
            return Err(MessageError::InvalidData(format!(
                "sub or unsub with contents: {contents:#?}"
            )));
        }

        let namespace = format!("{from}_{dest}_{app}");
        Ok(Self(typ.to_string(), namespace, contents))
    }

    pub fn typ(&self) -> MessageResult<MessageType> {
        self.0.parse::<MessageType>()
    }

    pub fn from(&self) -> MessageResult<User> {
        let id = self
            .1
            .split('_')
            .nth(1)
            .ok_or_else(|| MessageError::InvalidData("sender not found".to_string()))?;
        id.parse::<User>().map_err(Into::into)
    }

    pub fn dest(&self) -> MessageResult<User> {
        let id = self
            .1
            .split('_')
            .nth(2)
            .ok_or_else(|| MessageError::InvalidData("receiver not found".to_string()))?;
        id.parse::<User>().map_err(Into::into)
    }

    pub fn app(&self) -> MessageResult<App> {
        let parts: Vec<&str> = self.1.split('_').collect();
        if parts.len() < 3 {
            return Err(MessageError::InvalidData("app not found".to_string()));
        }

        let app = parts[2..].join("_");
        app.parse::<App>().map_err(Into::into)
    }

    pub fn contents(&self) -> &Option<MessageContents> {
        &self.2
    }
}

/// Scoped message streams that can be subscribed to on the Deezer Connect
/// websocket.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum App {
    /// Playback control and status information.
    RemoteCommand,

    /// Discovery and connection offers of Deezer Connect devices.
    RemoteDiscover,

    /// Playback queue publications from the controlling device.
    RemoteQueue,

    /// Echoes all messages for a user ID. That includes echoes of messages
    /// that the subscriber itself sent for that user ID.
    UserFeed(User),
}

/// Wire value for `App::RemoteCommand`.
pub const APP_REMOTE_COMMAND: &str = "REMOTECOMMAND";
/// Wire value for `App::RemoteDiscover`.
pub const APP_REMOTE_DISCOVER: &str = "REMOTEDISCOVER";
/// Wire value for `App::RemoteQueue`.
pub const APP_REMOTE_QUEUE: &str = "REMOTEQUEUE";
/// Wire value for `App::UserFeed`.
pub const APP_USER_FEED: &str = "USERFEED";

impl fmt::Display for App {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RemoteCommand => write!(f, "{APP_REMOTE_COMMAND}"),
            Self::RemoteDiscover => write!(f, "{APP_REMOTE_DISCOVER}"),
            Self::RemoteQueue => write!(f, "{APP_REMOTE_QUEUE}"),
            Self::UserFeed(id) => write!(f, "{APP_USER_FEED}_{id}"),
        }
    }
}

impl FromStr for App {
    type Err = MessageError;

    /// Parses a string `s` to return a variant of `App`.
    ///
    /// The parameter `s` is converted into uppercase.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!("REMOTEDISCOVER".parse(), Ok(App::RemoteDiscover));
    /// assert_eq!("USERFEED_4787654542".parse(), Ok(App::UserFeed(4787654542)));
    /// assert_eq!("foo".parse(), Ok(App::Unknown("FOO")));
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (app, id) = s
            .split_once('_')
            .map_or((s, None), |split| (split.0, Some(split.1)));

        // `unwrap` success guaranteed above.
        let variant = match app.to_uppercase().as_ref() {
            APP_REMOTE_COMMAND => Self::RemoteCommand,
            APP_REMOTE_DISCOVER => Self::RemoteDiscover,
            APP_REMOTE_QUEUE => Self::RemoteQueue,
            APP_USER_FEED => {
                if let Some(id) = id {
                    let id = id.parse::<User>()?;
                    Self::UserFeed(id)
                } else {
                    return Err(MessageError::InvalidData(
                        "no user id for user feed".to_string(),
                    ));
                }
            }
            _ => return Err(Self::Err::InvalidData(format!("unknown app: {s}"))),
        };

        Ok(variant)
    }
}

/// Message types for the Deezer Connect websocket.
#[derive(Clone, Eq, PartialEq, Debug)]
pub enum MessageType {
    /// UX tracking metrics.
    Log,

    /// A message received from the server.
    Msg,

    /// A message to send from the client.
    Send,

    /// Subscription to an application.
    Sub,

    /// Unsubscription from an application.
    Unsub,
}

/// Wire value for `MessageType::Log`.
pub const TYPE_LOG: &str = "sendlog";
/// Wire value for `MessageType::Msg`.
pub const TYPE_MSG: &str = "msg";
/// Wire value for `MessageType::Send`.
pub const TYPE_SEND: &str = "send";
/// Wire value for `MessageType::Sub`.
pub const TYPE_SUB: &str = "sub";
/// Wire value for `MessageType::Unsub`.
pub const TYPE_UNSUB: &str = "unsub";

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Log => write!(f, "{TYPE_LOG}"),
            Self::Msg => write!(f, "{TYPE_MSG}"),
            Self::Send => write!(f, "{TYPE_SEND}"),
            Self::Sub => write!(f, "{TYPE_SUB}"),
            Self::Unsub => write!(f, "{TYPE_UNSUB}"),
        }
    }
}

impl FromStr for MessageType {
    type Err = MessageError;

    /// Parses a string `s` to return a variant of `MessageType`.
    ///
    /// The parameter `s` is converted into lowercase.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!("msg".parse(), Ok(MessageType::Msg));
    /// assert_eq!("FOO".parse(), Ok(MessageType::Unknown("foo")));
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let variant = match s.to_lowercase().as_ref() {
            TYPE_LOG => Self::Log,
            TYPE_MSG => Self::Msg,
            TYPE_SEND => Self::Send,
            TYPE_SUB => Self::Sub,
            TYPE_UNSUB => Self::Unsub,
            _ => return Err(Self::Err::InvalidData(format!("unknown message type: {s}"))),
        };

        Ok(variant)
    }
}

/// Message sender or receiver identification.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum User {
    /// A Deezer user ID.
    Id(NonZeroU64),

    /// An unspecified Deezer receiver or sender.
    ///
    /// Used as receiver with `App:UserFeed` this means: messages from anyone.
    Unspecified,
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => write!(f, "{id}"),
            Self::Unspecified => write!(f, "-1"),
        }
    }
}

impl FromStr for User {
    type Err = num::ParseIntError;

    /// Parses a string `s` to return a variant of `User`.
    ///
    /// # Parameters
    ///
    /// - `s`: a string slice that must hold an integer representation
    ///
    /// # Returns
    ///
    /// Integer values greater than zero are returned as `User::Id`. A value of
    /// "-1" is returned as `User::Unspecified`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `s` does not represent an integer value
    /// - `s` represents a zero value
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!("1234567890".parse(), Ok(User(1234567890)));
    /// assert_eq!("-1".parse(), Ok(User::Unspecified));
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-1" {
            return Ok(Self::Unspecified);
        }

        let id = s.parse::<NonZeroU64>()?;
        Ok(Self::Id(id))
    }
}
