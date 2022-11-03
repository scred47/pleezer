use std::{
    collections::HashMap,
    fmt::{self, Write},
    num::{self, NonZeroU64},
    str::FromStr,
};

use serde::{de::Error, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use serde_with::{json::JsonString, serde_as, DeserializeFromStr, SerializeDisplay};
use thiserror::Error;

use super::*;

/// A Deezer Connect websocket message.
#[derive(Clone, PartialEq, Deserialize, Serialize, Debug)]
#[serde(untagged)]
pub enum Message {
    Contents(Stanza, Channel, MessageContents),
    Log(Stanza, HashMap<String, Value>),
    Subscription(Stanza, Channel),
}

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
    /// - `event`: an enum variant that represents the event
    /// - `contents`: an optional struct that holds the message contents
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `typ` is `Stanza::Log` with no `contents`
    /// - `typ` is `Stanza::Sub` or `Stanza::Unsub` with some `contents`
    ///
    /// # Examples

    fn new_subscription(typ: Stanza, from: User, to: User, event: Event) -> Self {
        debug_assert!(matches!(typ, Stanza::Subscribe | Stanza::Unsubscribe));

        Self::Subscription(typ, Channel { from, to, event })
    }

    pub fn new_subscribe(from: User, to: User, event: Event) -> Self {
        Self::new_subscription(Stanza::Subscribe, from, to, event)
    }

    pub fn new_unsubscribe(from: User, to: User, event: Event) -> Self {
        Self::new_subscription(Stanza::Unsubscribe, from, to, event)
    }

    pub fn new_log(metrics: HashMap<String, Value>) -> Self {
        Self::Log(Stanza::Log, metrics)
    }

    fn new_contents(
        typ: Stanza,
        from: User,
        to: User,
        event: Event,
        contents: MessageContents,
    ) -> Self {
        debug_assert!(matches!(typ, Stanza::Send | Stanza::Receive));
        Self::Contents(typ, Channel { from, to, event }, contents)
    }

    pub fn new_send(
        typ: Stanza,
        from: User,
        to: User,
        event: Event,
        contents: MessageContents,
    ) -> Self {
        Self::new_contents(Stanza::Send, from, to, event, contents)
    }

    pub fn new_receive(
        typ: Stanza,
        from: User,
        to: User,
        event: Event,
        contents: MessageContents,
    ) -> Self {
        Self::new_contents(Stanza::Receive, from, to, event, contents)
    }

    pub fn typ(&self) -> &Stanza {
        match self {
            Self::Contents(typ, _, _) => typ,
            Self::Subscription(typ, _) => typ,
            Self::Log(typ, _) => typ,
        }
    }

    pub fn from(&self) -> MessageResult<User> {
        let channel = self.channel()?;
        Ok(channel.from)
    }

    pub fn to(&self) -> MessageResult<User> {
        let channel = self.channel()?;
        Ok(channel.to)
    }

    pub fn event(&self) -> MessageResult<Event> {
        let channel = self.channel()?;
        Ok(channel.event)
    }

    fn channel(&self) -> MessageResult<&Channel> {
        let channel = match self {
            Self::Contents(_, channel, _) => channel,
            Self::Subscription(_, channel) => channel,
            Self::Log(_, _) => {
                return Err(MessageError::InvalidData("log has no event".to_string()))
            }
        };

        Ok(channel)
    }

    pub fn contents(&self) -> MessageResult<&MessageContents> {
        if let Self::Contents(_, _, contents) = &self {
            return Ok(contents);
        }

        Err(MessageError::InvalidData(
            "variant has no contents".to_string(),
        ))
    }
}

/// Scoped message streams that can be subscribed to on the Deezer Connect
/// websocket.
#[derive(Copy, Clone, Eq, SerializeDisplay, DeserializeFromStr, PartialEq, Debug)]
pub enum Event {
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

impl Event {
    /// Wire value for `Event::RemoteCommand`.
    const REMOTE_COMMAND: &str = "REMOTECOMMAND";
    /// Wire value for `Event::RemoteDiscover`.
    const REMOTE_DISCOVER: &str = "REMOTEDISCOVER";
    /// Wire value for `Event::RemoteQueue`.
    const REMOTE_QUEUE: &str = "REMOTEQUEUE";
    /// Wire value for `Event::UserFeed`.
    const USER_FEED: &str = "USERFEED";
}

impl fmt::Display for Event {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RemoteCommand => write!(f, "{}", Self::REMOTE_COMMAND),
            Self::RemoteDiscover => write!(f, "{}", Self::REMOTE_DISCOVER),
            Self::RemoteQueue => write!(f, "{}", Self::REMOTE_QUEUE),
            Self::UserFeed(id) => write!(f, "{}{}{}", Self::USER_FEED, Channel::SEPARATOR, id),
        }
    }
}

impl FromStr for Event {
    type Err = MessageError;

    /// Parses a string `s` to return a variant of `Event`.
    ///
    /// The parameter `s` is converted into uppercase.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!("REMOTEDISCOVER".parse(), Ok(Event::RemoteDiscover));
    /// assert_eq!("USERFEED_4787654542".parse(), Ok(Event::UserFeed(4787654542)));
    /// assert_eq!("foo".parse(), Ok(Event::Unknown("FOO")));
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (event, id) = s
            .split_once('_')
            .map_or((s, None), |split| (split.0, Some(split.1)));

        // `unwrap` success guaranteed above.
        let variant = match event.to_uppercase().as_ref() {
            Self::REMOTE_COMMAND => Self::RemoteCommand,
            Self::REMOTE_DISCOVER => Self::RemoteDiscover,
            Self::REMOTE_QUEUE => Self::RemoteQueue,
            Self::USER_FEED => {
                if let Some(id) = id {
                    let id = id.parse::<User>()?;
                    Self::UserFeed(id)
                } else {
                    return Err(MessageError::InvalidData(
                        "no user id for user feed".to_string(),
                    ));
                }
            }
            _ => return Err(Self::Err::InvalidData(format!("unknown event: {s}"))),
        };

        Ok(variant)
    }
}

/// Message types for the Deezer Connect websocket.
#[derive(Clone, Eq, Serialize, Deserialize, PartialEq, Debug)]
pub enum Stanza {
    /// UX tracking metrics.
    #[serde(rename = "sendlog")]
    Log,

    /// A message received from the server.
    #[serde(rename = "msg")]
    Receive,

    /// A message to send from the client.
    Send,

    /// Subscription to an event.
    #[serde(rename = "sub")]
    Subscribe,

    /// Unsubscription from an event.
    #[serde(rename = "unsub")]
    Unsubscribe,
}

impl Stanza {
    /// Wire value for `Stanza::Log`.
    const LOG: &str = "sendlog";
    /// Wire value for `Stanza::Receive`.
    const RECEIVE: &str = "msg";
    /// Wire value for `Stanza::Send`.
    const SEND: &str = "send";
    /// Wire value for `Stanza::Subscribe`.
    const SUBSCRIBE: &str = "sub";
    /// Wire value for `Stanza::Unsubsribe`.
    const UNSUBSCRIBE: &str = "unsub";
}

impl fmt::Display for Stanza {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Log => write!(f, "{}", Self::LOG),
            Self::Receive => write!(f, "{}", Self::RECEIVE),
            Self::Send => write!(f, "{}", Self::SEND),
            Self::Subscribe => write!(f, "{}", Self::SUBSCRIBE),
            Self::Unsubscribe => write!(f, "{}", Self::UNSUBSCRIBE),
        }
    }
}

impl FromStr for Stanza {
    type Err = MessageError;

    /// Parses a string `s` to return a variant of `Stanza`.
    ///
    /// The parameter `s` is converted into lowercase.
    ///
    /// # Examples
    ///
    /// ```
    /// assert_eq!("msg".parse(), Ok(Stanza::Msg));
    /// assert_eq!("FOO".parse(), Ok(Stanza::Unknown("foo")));
    /// ```
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let variant = match s.to_lowercase().as_ref() {
            Self::LOG => Self::Log,
            Self::RECEIVE => Self::Receive,
            Self::SEND => Self::Send,
            Self::SUBSCRIBE => Self::Subscribe,
            Self::UNSUBSCRIBE => Self::Unsubscribe,
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
    /// Used as receiver with `Event:UserFeed` this means: messages from anyone.
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

#[derive(Clone, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, Debug)]
pub struct Channel {
    pub from: User,
    pub to: User,
    pub event: Event,
}

impl Channel {
    pub const SEPARATOR: char = '_';
}

impl fmt::Display for Channel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}{}{}",
            self.from,
            Self::SEPARATOR,
            self.to,
            Self::SEPARATOR,
            self.event
        )
    }
}

impl FromStr for Channel {
    type Err = MessageError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(Self::SEPARATOR).into_iter();

        let from = parts.next().ok_or(MessageError::InvalidData(
            "from not found in channel".to_string(),
        ))?;
        let from = from.parse::<User>()?;

        let to = parts.next().ok_or(MessageError::InvalidData(
            "to not found in channel".to_string(),
        ))?;
        let to = to.parse::<User>()?;

        let event = parts.next().ok_or(MessageError::InvalidData(
            "event not found in channel".to_string(),
        ))?;
        let mut event = event.to_string();
        if let Some(id) = parts.next() {
            write!(event, "{}{}", Self::SEPARATOR, id)?;
        }
        let event = event.parse::<Event>()?;

        while let Some(unknown) = parts.next() {
            trace!("unknown part in channel: {unknown}");
        }

        Ok(Self { from, to, event })
    }
}
