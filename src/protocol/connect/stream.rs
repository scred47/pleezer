use std::{
    fmt::{self, Write},
    num::NonZeroU64,
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DeserializeFromStr, DisplayFromStr, SerializeDisplay};
use uuid::Uuid;

use super::channel::UserId;

/// The contents of a [`Message`] on a [`Stream`] [`Channel`] on a
/// [Deezer Connect][Connect] websocket.
///
/// [`Channel`]: ../channel/struct.Channel.html
/// [`Message`]: ../messages/enum.Message.html
/// [`Stream`]: ../channel/enum.Channel.html#variant.Stream
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct StreamContents {
    /// The [Deezer Connect][Connect] websocket [`Message`] [`StreamAction`]
    /// that these `StreamContents` are for.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`StreamAction`]: enum.StreamAction.html
    /// [`Message`]: ../messages/enum.Message.html
    #[serde(rename = "ACTION")]
    pub action: StreamAction,

    /// The [Deezer Connect][Connect] websocket [`Message`] [`StreamEvent`]
    /// that these `StreamContents` are for.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`StreamAction`]: enum.StreamEvent.html
    /// [`Message`]: ../messages/enum.Message.html
    #[serde(rename = "APP")]
    pub event: StreamEvent,

    /// The value of these [Deezer Connect][Connect] websocket [`Message`]
    /// `StreamContents`.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Message`]: ../messages/enum.Message.html
    #[serde(rename = "VALUE")]
    pub value: StreamValue,
}

#[serde_as]
#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct StreamValue {
    #[serde(rename = "USER_ID")]
    #[serde_as(as = "DisplayFromStr")]
    user: UserId,

    #[serde(rename = "UNIQID")]
    uuid: Uuid,

    #[serde(rename = "SNG_ID")]
    track: NonZeroU64,
}

#[derive(Copy, Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, Hash)]
pub enum StreamAction {
    Play,
}

#[derive(Copy, Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, Hash)]
pub enum StreamEvent {
    Limitation,
}

impl StreamAction {
    const PLAY: &'static str = "PLAY";
}

impl StreamEvent {
    const LIMITATION: &'static str = "LIMITATION";
}

impl fmt::Display for StreamContents {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} {}: {} {}",
            self.event, self.value.uuid, self.action, self.value.track,
        )
    }
}

impl fmt::Display for StreamAction {
    /// Formats an `StreamAction` as a wire string for use on a
    /// [Deezer Connect][Connect] websocket.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Play => write!(f, "{}", Self::PLAY),
        }
    }
}

impl FromStr for StreamAction {
    type Err = super::Error;

    /// Parses a wire string `s` on a [Deezer Connect][Connect] websocket to
    /// return a variant of `StreamAction`.
    ///
    /// The string `s` is parsed as uppercase.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let variant = match s {
            Self::PLAY => Self::Play,
            _ => return Err(Self::Err::Unsupported(format!("stream action `{s}`"))),
        };

        Ok(variant)
    }
}

impl fmt::Display for StreamEvent {
    /// Formats an `StreamEvent` as a wire string for use on a
    /// [Deezer Connect][Connect] websocket.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Limitation => write!(f, "{}", Self::LIMITATION),
        }
    }
}

impl FromStr for StreamEvent {
    type Err = super::Error;

    /// Parses a wire string `s` on a [Deezer Connect][Connect] websocket to
    /// return a variant of `StreamEvent`.
    ///
    /// The string `s` is parsed as uppercase.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let variant = match s {
            Self::LIMITATION => Self::Limitation,
            _ => return Err(Self::Err::Unsupported(format!("stream action `{s}`"))),
        };

        Ok(variant)
    }
}
