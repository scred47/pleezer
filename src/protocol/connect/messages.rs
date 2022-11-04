use std::{
    fmt::{self, Write},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

use super::{Channel, Contents};

/// A list of [Deezer Connect][Connect] websocket messages.
///
/// This implementation does not aim to cover all messages on the [`Connect`]
/// websocket. Rather, the focus is to provide an ergonomic and strongly typed
/// abstraction only for those messages that are used to provide remote control
/// capabilities. For example, [`Connect`] has messages for UX tracking, but
/// `Message` has no such variant.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Message {
    /// A message with [`Contents`] to send into a [`Channel`].
    ///
    /// [`Channel`]: struct.Channel.html
    /// [`Contents`]: struct.Contents.html
    Send {
        channel: Channel,
        contents: Contents,
    },
    
    /// A message with [`Contents`] received over a [`Channel`].
    ///
    /// [`Channel`]: struct.Channel.html
    /// [`Contents`]: struct.Contents.html
    Receive {
        channel: Channel,
        contents: Contents,
    },
    
    /// A subscription to a [`Channel`](struct.Channel.html).
    Subscribe {
        channel: Channel,
    },
    
    /// An unsubscription from a [`Channel`](struct.Channel.html).
    Unsubscribe {
        channel: Channel,
    },
}

impl Serialize for Message {
    /// Convert this `Message` into a [`JsonMessage`], then serialize it into
    /// [`JSON`].
    ///
    /// [`JsonMessage`]: enum.JsonMessage.html
    /// [JSON]: https://www.json.org/
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let json_message = JsonMessage::from(self);
        let json = serde_json::to_string(&message).map_err(|e| serde::ser::Error::custom(e))?;
        serializer.collect_str(&json)
    }
}

impl<'de> Deserialize<'de> for Message {
    /// Deserialize [`JSON`] into a [`JsonMessage`], then convert it into a
    /// `Message`.
    ///
    /// [`JsonMessage`]: enum.JsonMessage.html
    /// [JSON]: https://www.json.org/
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let json_message = JsonMessage::deserialize(deserializer)?;
        Self::try_from(json_message).map_err(|e| serde::de::Error::custom(e))
    }
}

/// A list of [Deezer Connect][Connect] websocket messages for serialization
/// and deserialization into [JSON] as wire format.
///
/// The [`Message`] enum provides an ergonomic abstraction over this wire
/// format that should be used instead.
///
/// This implementation does not aim to cover all messages on the [`Connect`]
/// websocket. Rather, the focus is to provide only those wire formats that are
/// required by the [`Message`] variants. For example, [`Connect`] has a format
/// for UX tracking, but `JsonMessage` has no such variant.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
/// [JSON]: https://www.json.org/
/// [`Message`]: struct.Message.html
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(untagged)]
enum JsonMessage {
    /// A sequence to send or receive message [`Contents`] over a [`Channel`].
    /// On the wire this is a three-element [JSON] array composed of two
    /// strings followed by a map.
    ///
    /// [`Channel`]: struct.Channel.html
    /// [`Contents`]: struct.Contents.html
    /// [JSON]: https://www.json.org/
    WithContents(Stanza, Channel, Contents),
    
    /// A sequence to subscribe to or unsubscribe from a [`Channel`]. On the
    /// wire this is a two-element [JSON] array composed of two strings.
    ///
    /// [`Channel`]: struct.Channel.html
    /// [`Contents`]: struct.Contents.html
    /// [JSON]: https://www.json.org/
    Subscription(Stanza, Channel),
}

/// A list of [Deezer Connect][Connect] websocket message stanzas on the wire.
///
/// The [`Message`] enum provides an ergonomic abstraction over these stanzas
/// that should be used instead.
///
/// This implementation does not aim to cover all message stanzas on the
/// [`Connect`] websocket. Rather, the focus is to provide only those stanzas
/// that are required by the [`Message`] variants. For example, [`Connect`] has
/// a stanza for UX tracking, but `Stanza` has no such variant.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[derive(Copy, Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Stanza {
    /// A stanza marking that the message elements that follow were received
    /// over a [`Channel`](struct.Channel.html).
    Receive,

    /// A stanza marking that the message elements that follow are to be sent
    /// into a [`Channel`](struct.Channel.html).
    Send,

    /// A stanza marking that the message elements that follow are to subscribe
    /// to a [`Channel`](struct.Channel.html).
    Subscribe,

    /// A stanza marking that the message elements that follow are to
    /// unsubscribe from a [`Channel`](struct.Channel.html).
    Unsubscribe,
}

impl Stanza {
    /// Wire value for [`Stanza::Receive`](#variant.Receive).
    const STR_RECEIVE: &str = "msg";
    
    /// Wire value for [`Stanza::Send`](#variant.Send).
    const STR_SEND: &str = "send";
    
    /// Wire value for [`Stanza::Subscribe`](#variant.Subscribe).
    const STR_SUBSCRIBE: &str = "sub";
    
    /// Wire value for [`Stanza::Unsubscribe`](#variant.Unsubscribe).
    const STR_UNSUBSCRIBE: &str = "unsub";
}

impl From<Message> for JsonMessage {
    /// Converts to a `JsonMessage` from a [`Message`](struct.Message.html).
    fn from(message: Message) -> Self {
        match message {
            Message::Receive { channel, contents } => Self::WithContents(
                Stanza::Receive,
                channel,
                contents.to_owned(),
            ),
            Message::Send { channel, contents } => Self::WithContents(
                Stanza::Send,
                channel,
                contents.to_owned(),
            ),
            Message::Subscribe { channel } => {
                Self::Subscription(Stanza::Subscribe, channel,
            }
            Message::Unsubscribe { channel } => {
                Self::Subscription(Stanza::Unsubscribe, channel,
            }
        }
    }
}

impl TryFrom<JsonMessage> for Message {
    type Error = super::Error;
    
    /// Performs the conversion from [`JsonMessage`] into `Message`.
    ///
    /// [`JsonMessage`]: struct.JsonMessage.html
    fn try_from(message: JsonMessage) -> Result<Self, Self::Error> {
        let variant = match message {
            JsonMessage::WithContents(stanza, channel, contents) => {
                match stanza {
                    Stanza::Send => Self::Send { channel, contents },
                    Stanza::Receive => Self::Receive { channel, contents },
                    _ => {
                        return Err(Self::Error::Unsupported(format!(
                            "unexpected stanza for format with contents: `{stanza}`"
                        )));
                    }
                }
            }
            JsonMessage::Subscription(stanza, channel) => {
                match stanza {
                    Stanza::Subscribe => Self::Subscribe { channel },
                    Stanza::Unsubscribe => Self::Unsubscribe { channel },
                    _ => {
                        return Err(Self::Error::Unsupported(format!(
                            "unexpected stanza for subscription format: `{stanza}`"
                        )));
                    }
                }
            }
        }
        
        Ok(variant)
    }
}

impl fmt::Display for Stanza {
    /// Formats a `Stanza` as a string for use on a [Deezer Connect][Connect]
    /// websocket.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Receive => write!(f, "{}", Self::STR_RECEIVE),
            Self::Send => write!(f, "{}", Self::STR_SEND),
            Self::Subscribe => write!(f, "{}", Self::STR_SUBSCRIBE),
            Self::Unsubscribe => write!(f, "{}", Self::STR_UNSUBSCRIBE),
        }
    }
}

impl FromStr for Stanza {
    type Err = super::Error;

    /// Parses a string `s` on a [Deezer Connect][Connect] websocket to return
    /// a variant of `Stanza`.
    ///
    /// The string `s` is parsed as lowercase.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let variant = match s.to_lowercase().as_ref() {
            Self::STR_RECEIVE => Self::Receive,
            Self::STR_SEND => Self::Send,
            Self::STR_SUBSCRIBE => Self::Subscribe,
            Self::STR_UNSUBSCRIBE => Self::Unsubscribe,
            _ => return Err(Self::Err::Unsupported(format!("stanza `{s}`"))),
        };

        Ok(variant)
    }
}