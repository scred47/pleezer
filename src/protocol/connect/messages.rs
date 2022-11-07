use std::{
    fmt::{self, Write},
    str::FromStr,
};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{DeserializeFromStr, SerializeDisplay};

use super::{Channel, Contents};

/// A list of messages on a [Deezer Connect][Connect] websocket.
///
/// The aim of this implementation is to provide an ergonomic and strongly
/// typed abstraction for those messages that are used to furnish remote
/// control capabilities. For example, [Connect] has messages for UX tracking,
/// but `Message` has no such variant.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[derive(Clone, Debug, PartialEq)]
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
    Subscribe { channel: Channel },

    /// An unsubscription from a [`Channel`](struct.Channel.html).
    Unsubscribe { channel: Channel },
}

impl Serialize for Message {
    /// Convert this `Message` into a [`WireMessage`], then serialize it into
    /// [JSON].
    ///
    /// [JSON]: https://www.json.org/
    /// [`WireMessage`]: enum.WireMessage.html
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let json_message =
            WireMessage::try_from(self.clone()).map_err(|e| serde::ser::Error::custom(e))?;
        let json =
            serde_json::to_string(&json_message).map_err(|e| serde::ser::Error::custom(e))?;
        serializer.collect_str(&json)
    }
}

impl<'de> Deserialize<'de> for Message {
    /// Deserialize [JSON] into a [`WireMessage`], then convert it into a
    /// `Message`.
    ///
    /// [JSON]: https://www.json.org/
    /// [`WireMessage`]: enum.WireMessage.html
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let json_message = WireMessage::deserialize(deserializer)?;
        Self::try_from(json_message).map_err(|e| serde::de::Error::custom(e))
    }
}

/// A list of messages on a [Deezer Connect][Connect] websocket in their [JSON]
/// wire formats.
///
/// The [`Message`] enum provides an ergonomic abstraction over this wire
/// format that should be used instead.
///
/// The aim of this implementation is to provide an ergonomic and strongly
/// typed abstraction for those messages that are used to furnish remote
/// control capabilities. For example, [Connect] has messages for UX tracking,
/// but `WireMessage` has no such variant.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
/// [JSON]: https://www.json.org/
/// [`Message`]: struct.Message.html
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum WireMessage {
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

/// A list of message stanzas on a [Deezer Connect][Connect] websocket.
///
/// The [`Message`] enum provides an ergonomic abstraction over these stanzas
/// that should be used instead.
///
/// The aim of this implementation is to provide an ergonomic and strongly
/// typed abstraction for those messages that are used to furnish remote
/// control capabilities. For example, [Connect] has a stanza for UX tracking,
/// but `Stanza` has no such variant.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079

#[derive(Copy, Clone, Debug, Hash, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
enum Stanza {
    /// A stanza marking that the message elements that follow were received
    /// over a [`Channel`](struct.Channel.html).
    #[serde(rename = "msg")]
    Receive,

    /// A stanza marking that the message elements that follow are to be sent
    /// into a [`Channel`](struct.Channel.html).
    #[serde(rename = "send")]
    Send,

    /// A stanza marking that the message elements that follow are to subscribe
    /// to a [`Channel`](struct.Channel.html).
    #[serde(rename = "sub")]
    Subscribe,

    /// A stanza marking that the message elements that follow are to
    /// unsubscribe from a [`Channel`](struct.Channel.html).
    #[serde(rename = "unsub")]
    Unsubscribe,
}

impl fmt::Display for Stanza {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl TryFrom<Message> for WireMessage {
    type Error = super::Error;

    /// Performs the conversion from [`Message`] into `WireMessage`.
    ///
    /// [`Message`]: struct.Message.html
    fn try_from(message: Message) -> Result<Self, Self::Error> {
        let wire_message = match message {
            Message::Receive { channel, contents } => {
                let contents_event = contents.event;
                let channel_event = channel.event;
                if contents_event != channel_event {
                    return Err(Self::Error::Malformed(format!(
                        "channel event {channel_event} should match contents event {contents_event}",
                    )));
                }

                Self::WithContents(Stanza::Receive, channel, contents.to_owned())
            }
            Message::Send { channel, contents } => {
                let contents_event = contents.event;
                let channel_event = channel.event;
                if contents_event != channel_event {
                    return Err(Self::Error::Malformed(format!(
                        "channel event {channel_event} should match contents event {contents_event}",
                    )));
                }

                Self::WithContents(Stanza::Send, channel, contents.to_owned())
            }
            Message::Subscribe { channel } => Self::Subscription(Stanza::Subscribe, channel),
            Message::Unsubscribe { channel } => Self::Subscription(Stanza::Unsubscribe, channel),
        };

        Ok(wire_message)
    }
}

impl TryFrom<WireMessage> for Message {
    type Error = super::Error;

    /// Performs the conversion from [`WireMessage`] into `Message`.
    ///
    /// [`WireMessage`]: struct.WireMessage.html
    fn try_from(wire_message: WireMessage) -> Result<Self, Self::Error> {
        let message = match wire_message {
            WireMessage::WithContents(stanza, channel, contents) => {
                let contents_event = contents.event;
                let channel_event = channel.event;
                if contents_event != channel_event {
                    return Err(Self::Error::Malformed(format!(
                        "channel event {channel_event} should match contents event {contents_event}",
                    )));
                }

                match stanza {
                    Stanza::Send => Self::Send { channel, contents },
                    Stanza::Receive => Self::Receive { channel, contents },
                    _ => {
                        return Err(Self::Error::Unsupported(format!(
                            "stanza {stanza} should match for message with contents"
                        )));
                    }
                }
            }
            WireMessage::Subscription(stanza, channel) => match stanza {
                Stanza::Subscribe => Self::Subscribe { channel },
                Stanza::Unsubscribe => Self::Unsubscribe { channel },
                _ => {
                    return Err(Self::Error::Unsupported(format!(
                        "stanza {stanza} should match for subscription message"
                    )));
                }
            },
        };

        Ok(message)
    }
}
