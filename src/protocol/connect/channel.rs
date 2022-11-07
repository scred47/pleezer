use std::{
    fmt::{self, Write},
    num::{self, NonZeroU64},
    str::FromStr,
};

use serde::{Deserialize, Serialize};
use serde_with::{DeserializeFromStr, SerializeDisplay};

/// A `Channel` on a [Deezer Connect][Connect] websocket.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
/// [`Message`]: ../messages/enum.Message.html
#[derive(
    Copy, Clone, Debug, Hash, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord,
)]
pub struct Channel {
    /// The sending [Deezer] [`User`].
    ///
    /// [Deezer]: https://www.deezer.com/
    /// [`User`]: enum.User.html
    pub from: User,

    /// The receiving [Deezer] [`User`].
    ///
    /// [Deezer]: https://www.deezer.com/
    /// [`User`]: enum.User.html
    pub to: User,

    /// The [Deezer Connect][Connect] [`Event`] variant.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [Deezer]: https://www.deezer.com/
    /// [`Event`]: enum.Event.html
    pub event: Event,
}

/// A list of user representations on a [Deezer Connect][Connect] websocket.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[derive(Copy, Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum User {
    /// A [Deezer] user ID.
    ///
    /// [Deezer]: https://www.deezer.com/
    Id(NonZeroU64),

    /// An unspecified [Deezer] receiver or sender.
    ///
    /// Used as `from` in [`Event:UserFeed`][UserFeed] this means: messages
    /// from anyone.
    ///
    /// [Deezer]: https://www.deezer.com/
    /// [UserFeed]: enum.Event.html#variant.UserFeed
    Unspecified,
}

/// A list of [Deezer Connect][Connect] websocket message events.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[derive(
    Copy, Clone, Debug, Hash, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord,
)]
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

impl Channel {
    /// [Deezer Connect][Connect] websocket wire character that separates the
    /// `Channel` parts.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    pub(crate) const SEPARATOR: char = '_';
}

impl Event {
    /// Wire value for [`Event::RemoteCommand`](#variant.RemoteCommand).
    const REMOTE_COMMAND: &str = "REMOTECOMMAND";

    /// Wire value for [`Event::RemoteDiscover`](#variant.RemoteDiscover).
    const REMOTE_DISCOVER: &str = "REMOTEDISCOVER";

    /// Wire value for [`Event::RemoteQueue`](#variant.RemoteQueue).
    const REMOTE_QUEUE: &str = "REMOTEQUEUE";

    /// Wire value for [`Event::UserFeed`](#variant.UserFeed).
    const USER_FEED: &str = "USERFEED";
}

impl fmt::Display for Channel {
    /// Formats a `Channel` as a wire string for use on a
    /// [Deezer Connect][Connect] websocket.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
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
    type Err = super::Error;

    /// Parses a [Deezer Connect][Connect] websocket wire string `s` to return
    /// a `Channel`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `s` does not contain a known channel representation
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(Self::SEPARATOR).into_iter();

        let from = parts.next().ok_or(Self::Err::Malformed(
            "channel string slice should hold `from` part".to_string(),
        ))?;
        let from = from.parse::<User>()?;

        let to = parts.next().ok_or(Self::Err::Malformed(
            "channel string slice should hold `to` part".to_string(),
        ))?;
        let to = to.parse::<User>()?;

        let event = parts.next().ok_or(Self::Err::Malformed(
            "channel string slice should hold `event` part".to_string(),
        ))?;
        let mut event = event.to_string();
        if let Some(id) = parts.next() {
            write!(event, "{}{}", Self::SEPARATOR, id)?;
        }
        let event = event.parse::<Event>()?;

        if let Some(unknown) = parts.next() {
            return Err(Self::Err::Unsupported(format!(
                "channel string slice holds unknown trailing parts: `{s}`"
            )));
        }

        Ok(Self { from, to, event })
    }
}

impl fmt::Display for User {
    /// Formats a `User` as a wire string for use on a
    /// [Deezer Connect][Connect] websocket.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Id(id) => write!(f, "{id}"),
            Self::Unspecified => write!(f, "-1"),
        }
    }
}

impl FromStr for User {
    type Err = super::Error;

    /// Parses a [Deezer Connect][Connect] websocket wire string `s` to return
    /// a variant of `User`.
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
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "-1" {
            return Ok(Self::Unspecified);
        }

        let id = s
            .parse::<NonZeroU64>()
            .map_err(|e| Self::Err::Malformed(format!("user id: {e}")))?;
        Ok(Self::Id(id))
    }
}

impl From<NonZeroU64> for User {
    /// Converts to a `User` from a [`NonZeroU64`](https://doc.rust-lang.org/std/num/struct.NonZeroU64.html).
    fn from(id: NonZeroU64) -> Self {
        Self::Id(id)
    }
}

impl fmt::Display for Event {
    /// Formats an `Event` as a wire string for use on a
    /// [Deezer Connect][Connect] websocket.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
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
    type Err = super::Error;

    /// Parses a wire string `s` on a [Deezer Connect][Connect] websocket to
    /// return a variant of `Event`.
    ///
    /// The string `s` is parsed as uppercase.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (event, id) = s
            .split_once('_')
            .map_or((s, None), |split| (split.0, Some(split.1)));

        let event = event.to_uppercase();
        let variant = match event.as_ref() {
            Self::REMOTE_COMMAND => Self::RemoteCommand,
            Self::REMOTE_DISCOVER => Self::RemoteDiscover,
            Self::REMOTE_QUEUE => Self::RemoteQueue,
            Self::USER_FEED => {
                if let Some(id) = id {
                    let id = id.parse::<User>()?;
                    Self::UserFeed(id)
                } else {
                    return Err(Self::Err::Malformed(format!(
                        "event `{event}` should have user id suffix"
                    )));
                }
            }
            _ => return Err(Self::Err::Unsupported(format!("event `{s}`"))),
        };

        Ok(variant)
    }
}
