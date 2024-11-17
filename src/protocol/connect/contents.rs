use std::{
    collections::{HashMap, HashSet},
    fmt::{self, Write},
    io::Read,
    str::FromStr,
    time::Duration,
};

use base64::prelude::*;
use flate2::{
    read::{DeflateDecoder, DeflateEncoder},
    Compression,
};
use protobuf::Message;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{
    json::JsonString, serde_as, DeserializeFromStr, DisplayFromStr, DurationSeconds,
    NoneAsEmptyString, SerializeDisplay,
};
use uuid::Uuid;

use super::channel::Ident;
use super::protos::queue;
use crate::{error::Error, track::TrackId};

// Most IDs are UUIDs, but case sensitive, while Deezer Connect uses
// uppercase on iOS and lowercase on Android. Therefore, many IDs are typed
// as `String` (and borrowed as &`str`) instead of a true `Uuid`.

/// The `Contents` of a [`Message`] on a [Deezer Connect][Connect] websocket.
///
/// [`Message`]: ../messages/enum.Message.html
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Contents {
    /// The [Deezer Connect][Connect] websocket [`Message`] [`Event`] that
    /// these `Contents` are for.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Event`]: ../channel/enum.Event.html
    /// [`Message`]: ../messages/enum.Message.html
    #[serde(rename = "APP")]
    pub ident: Ident,

    /// The [Deezer Connect][Connect] websocket [`Message`] [`Headers`] that
    /// are attached to these `Contents`.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Header`]: struct.Header.html
    /// [`Message`]: ../messages/enum.Message.html
    pub headers: Headers,

    /// The [`Body`] of these [Deezer Connect][Connect] websocket [`Message`]
    /// `Contents`.
    ///
    /// The wire format of this field is peculiar, in that it is [JSON]
    /// embedded in a [`String`]. The `Serialize` and `Deserialize`
    /// [implementations][JsonString] of `Contents` handle this transparently.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Body`]: struct.Body.html
    /// [JSON]: https://www.json.org/
    /// [JsonString]: https://docs.rs/serde_with/latest/serde_with/json/struct.JsonString.html
    /// [`Message`]: ../messages/enum.Message.html
    /// [`String`]: https://doc.rust-lang.org/std/string/
    #[serde_as(as = "JsonString")]
    pub body: Body,
}

impl fmt::Display for Contents {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // FIXME: padding is not respected.
        write!(f, "{:<16}", self.body.message_type())
    }
}

/// The `Headers` attached to some [`Message`] [`Contents`] on a
/// [Deezer Connect][Connect] websocket.
///
/// [Deezer Connect][Connect] devices are identified by some [UUID]
/// presentation, sometimes formatted according to RFC4122 with hyphens,
/// sometimes without hyphens and prepended by some character. One hypothesis
/// is that these identify some form of Android AAIDs, Apple IDFAs, and others.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
/// [`Contents`]: struct.Contents.html
/// [`Message`]: ../messages/enum.Message.html
/// [UUID]: http://tools.ietf.org/html/rfc4122
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Headers {
    /// The source of some [`Message`] [`Contents`].
    ///
    /// [`Contents`]: struct.Contents.html
    /// [`Message`]: ../messages/enum.Message.html
    pub from: DeviceId,

    /// The optional destination for some [`Message`] [`Contents`].
    ///
    /// [`Contents`]: struct.Contents.html
    /// [`Message`]: ../messages/enum.Message.html
    pub destination: Option<DeviceId>,
}

impl fmt::Display for Headers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "from {}", self.from)?;

        if let Some(destination) = &self.destination {
            write!(f, " to {destination}")?;
        }

        Ok(())
    }
}

#[derive(
    Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum DeviceId {
    Uuid(Uuid),
    Other(String),
}

impl Default for DeviceId {
    fn default() -> Self {
        Self::Uuid(Uuid::new_v4())
    }
}

impl From<Uuid> for DeviceId {
    fn from(uuid: Uuid) -> Self {
        Self::Uuid(uuid)
    }
}

impl FromStr for DeviceId {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let device = match Uuid::try_parse(s) {
            Ok(uuid) => Self::from(uuid),
            Err(_) => Self::Other(s.to_owned()),
        };

        Ok(device)
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Uuid(uuid) => write!(f, "{uuid}"),
            Self::Other(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Acknowledgement {
        message_id: String,
        acknowledgement_id: String,
    },

    Close {
        message_id: String,
    },

    Connect {
        message_id: String,
        from: DeviceId,
        offer_id: String,
    },

    ConnectionOffer {
        message_id: String,
        from: DeviceId,
        device_name: String,
    },

    DiscoveryRequest {
        message_id: String,
        from: DeviceId,
        discovery_session: String,
    },

    PlaybackProgress {
        message_id: String,
        track: QueueItem,
        quality: AudioQuality,
        duration: Duration,
        buffered: Duration,
        progress: Option<Percentage>,
        volume: Percentage,
        is_playing: bool,
        is_shuffle: bool,
        repeat_mode: RepeatMode,
    },

    PublishQueue {
        message_id: String,
        queue: queue::List,
    },

    Ping {
        message_id: String,
    },

    Ready {
        message_id: String,
    },

    RefreshQueue {
        message_id: String,
    },

    Skip {
        message_id: String,
        queue_id: Option<String>,
        track: Option<QueueItem>,
        progress: Option<Percentage>,
        should_play: Option<bool>,
        set_repeat_mode: Option<RepeatMode>,
        set_shuffle: Option<bool>,
        set_volume: Option<Percentage>,
    },

    Status {
        message_id: String,
        command_id: String,
        status: Status,
    },

    Stop {
        message_id: String,
    },
}

impl Body {
    #[must_use]
    pub fn message_type(&self) -> MessageType {
        match self {
            Self::Acknowledgement { .. } => MessageType::Acknowledgement,
            Self::Close { .. } => MessageType::Close,
            Self::Connect { .. } => MessageType::Connect,
            Self::ConnectionOffer { .. } => MessageType::ConnectionOffer,
            Self::DiscoveryRequest { .. } => MessageType::DiscoveryRequest,
            Self::PlaybackProgress { .. } => MessageType::PlaybackProgress,
            Self::PublishQueue { .. } => MessageType::PublishQueue,
            Self::Ping { .. } => MessageType::Ping,
            Self::Ready { .. } => MessageType::Ready,
            Self::RefreshQueue { .. } => MessageType::RefreshQueue,
            Self::Skip { .. } => MessageType::Skip,
            Self::Status { .. } => MessageType::Status,
            Self::Stop { .. } => MessageType::Stop,
        }
    }

    #[must_use]
    pub fn message_id(&self) -> &str {
        match self {
            Self::Acknowledgement { message_id, .. }
            | Self::Close { message_id, .. }
            | Self::Connect { message_id, .. }
            | Self::ConnectionOffer { message_id, .. }
            | Self::DiscoveryRequest { message_id, .. }
            | Self::PlaybackProgress { message_id, .. }
            | Self::PublishQueue { message_id, .. }
            | Self::Ping { message_id, .. }
            | Self::Ready { message_id, .. }
            | Self::RefreshQueue { message_id, .. }
            | Self::Skip { message_id, .. }
            | Self::Status { message_id, .. }
            | Self::Stop { message_id, .. } => message_id,
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Hash,
    Serialize_repr,
    Deserialize_repr,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
)]
#[repr(u64)]
pub enum Status {
    OK = 0,

    // Assume failure unless explicitly specified otherwise. This is what
    // Deezer Connect does itself.
    #[default]
    Error = 1,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Status::OK => write!(f, "Ok"),
            Status::Error => write!(f, "Err"),
        }
    }
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Hash,
    Serialize_repr,
    Deserialize_repr,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
)]
// `i64` because this is serialized into and deserialized from JSON.
#[repr(i64)]
pub enum RepeatMode {
    #[default]
    None = 0,
    All = 1,
    One = 2,
    Unrecognized = -1,
}

impl fmt::Display for RepeatMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RepeatMode::None => write!(f, "None"),
            RepeatMode::All => write!(f, "All"),
            RepeatMode::One => write!(f, "One"),
            RepeatMode::Unrecognized => Err(fmt::Error),
        }
    }
}

/// Audio quality levels as per Deezer on desktop.
///
/// Note that the remote device has no control over the audio quality of the
/// player.
#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Hash,
    Serialize_repr,
    Deserialize_repr,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
)]
// `i64` because this is serialized into and deserialized from JSON.
#[repr(i64)]
pub enum AudioQuality {
    /// 64 kbps MP3
    Basic = 0,

    /// 128 kbps MP3 (default)
    #[default]
    Standard = 1,

    /// 320 kbps MP3 (requires Premium subscription)
    High = 2,

    #[expect(clippy::doc_markdown)]
    /// 1411 kbps FLAC (requires HiFi subscription)
    Lossless = 3,

    /// Unknown bitrate and/or format
    Unknown = -1,
}

impl fmt::Display for AudioQuality {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioQuality::Basic => write!(f, "Basic"),
            AudioQuality::Standard => write!(f, "Standard"),
            AudioQuality::High => write!(f, "High Quality"),
            AudioQuality::Lossless => write!(f, "High Fidelity"),
            AudioQuality::Unknown => write!(f, "Unknown"),
        }
    }
}

impl FromStr for AudioQuality {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let variant = match s {
            "low" => AudioQuality::Basic,
            "standard" => AudioQuality::Standard,
            "high" => AudioQuality::High,
            "lossless" => AudioQuality::Lossless,
            _ => AudioQuality::Unknown,
        };

        Ok(variant)
    }
}

#[derive(Copy, Clone, Debug, Default, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Percentage(f64);

impl Percentage {
    #[must_use]
    pub fn from_ratio_f32(ratio: f32) -> Self {
        Self(ratio.into())
    }

    #[must_use]
    pub fn from_ratio_f64(ratio: f64) -> Self {
        Self(ratio)
    }

    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub fn as_ratio_f32(&self) -> f32 {
        self.0 as f32
    }

    #[must_use]
    pub fn as_ratio_f64(&self) -> f64 {
        self.0
    }

    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub fn as_percent_f32(&self) -> f32 {
        self.0 as f32 * 100.0
    }

    #[must_use]
    pub fn as_percent_f64(&self) -> f64 {
        self.0 * 100.0
    }
}

impl fmt::Display for Percentage {
    /// Formats an `Percentage` for display with a `%` sign.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.as_percent_f32())
    }
}

#[derive(
    Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialOrd, Ord, PartialEq, Eq, Hash,
)]
pub struct QueueItem {
    pub queue_id: String,
    pub track_id: TrackId,
    // `usize` because this will index into an array. Also from the protobuf it
    // is known that this really an `u32`.
    pub position: usize,
}

impl QueueItem {
    const SEPARATOR: char = '-';
}

impl fmt::Display for QueueItem {
    /// Formats an `Event` as a wire string for use on a
    /// [Deezer Connect][Connect] websocket.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}{}{}{}",
            self.queue_id,
            Self::SEPARATOR,
            self.track_id,
            Self::SEPARATOR,
            self.position
        )
    }
}

impl FromStr for QueueItem {
    type Err = Error;

    /// Parses a wire string `s` on a [Deezer Connect][Connect] websocket to
    /// return an track on a queue.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let mut parts = s.split(Self::SEPARATOR);

        let mut queue_id = String::new();
        for i in 0..5 {
            match parts.next() {
                Some(part) => {
                    write!(queue_id, "{part}")?;
                    if i < 4 {
                        write!(queue_id, "{}", Self::SEPARATOR)?;
                    }
                }
                None => {
                    return Err(Self::Err::invalid_argument(format!(
                        "list element string slice should hold five `queue_id` parts, found {i}"
                    )))
                }
            }
        }

        if let Err(e) = Uuid::try_parse(&queue_id) {
            return Err(Self::Err::invalid_argument(format!("queue id: {e}")));
        }

        let track_id = parts.next().ok_or_else(|| {
            Self::Err::invalid_argument(
                "list element string slice should hold `track_id` part".to_string(),
            )
        })?;

        // User-uploaded track IDs are negative. If the track ID is empty, then
        // see if the next part is a user-uploaded track ID.
        let track_id = if track_id.is_empty() {
            if let Some(user_uploaded_id) = parts.next() {
                let negative_track_id = format!("-{user_uploaded_id}");
                negative_track_id.parse::<TrackId>()?
            } else {
                return Err(Self::Err::invalid_argument(
                    "user-uploaded track id should not be empty".to_string(),
                ));
            }
        } else {
            track_id.parse::<TrackId>()?
        };

        let position = parts.next().ok_or_else(|| {
            Self::Err::invalid_argument(
                "list element string slice should hold `position` part".to_string(),
            )
        })?;
        let position = position.parse::<usize>()?;

        Ok(Self {
            queue_id,
            track_id,
            position,
        })
    }
}

// For syntactic sugar this could be changed into `serde_with::SerializeAs` but
// this now follows the same idiom as serializing a `Message`.
impl Serialize for Body {
    /// Convert this `Body` into a [`WireBody`], then serialize it into [JSON].
    ///
    /// [JSON]: https://www.json.org/
    /// [`WireMessage`]: enum.WireMessage.html
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        let wire_body = WireBody::from(self.clone());
        wire_body.serialize(serializer)
    }
}

// For syntactic sugar this could be changed into `serde_with::DeserializeAs` but
// this now follows the same idiom as deserializing a `Message`.
impl<'de> Deserialize<'de> for Body {
    /// Deserialize [JSON] into a [`WireBody`], then convert it into a `Body`.
    ///
    /// [JSON]: https://www.json.org/
    /// [`WireMessage`]: enum.WireMessage.html
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let wire_body = WireBody::deserialize(deserializer)?;
        Self::try_from(wire_body).map_err(serde::de::Error::custom)
    }
}

/// The [`WireBody`] of some [Deezer Connect][Connect] websocket [`Message`]
/// [`Contents`].
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
/// [`Contents`]: struct.Contents.html
/// [`Message`]: ../messages/enum.Message.html
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct WireBody {
    /// The [`Uuid`] of some [Deezer Connect][Connect] websocket [`Message`]
    /// that has this `Body`.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Message`]: ../messages/enum.Message.html
    /// [`Uuid`]: https://docs.rs/uuid/latest/uuid/
    message_id: String,

    /// The [`MessageType`] that tags the `payload` of some
    /// [Deezer Connect][Connect] websocket [`Message`] that has this `Body`.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Message`]: ../messages/enum.Message.html
    /// [`MessageType`]: enum.MessageType.html
    message_type: MessageType,

    /// The protocol version of some [Deezer Connect][Connect] websocket
    /// [`Message`] that has this `Body`.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Message`]: ../messages/enum.Message.html
    protocol_version: String,

    /// The [`Payload`] of some [Deezer Connect][Connect] websocket [`Message`]
    /// that has this `Body`.
    ///
    /// The wire format of this field is peculiar, in that it is encoded as
    /// [Base64]. Then, depending on the tagged `MessageType`, it may either
    /// contain [JSON] or a [protocol buffer][Protobuf] that is compressed
    /// with [DEFLATE]. The [`Serialize`] and [`Deserialize`] of `Body`
    /// handle this transparently.
    ///
    /// [Base64]: https://datatracker.ietf.org/doc/html/rfc3548
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [DEFLATE]: https://datatracker.ietf.org/doc/html/rfc1951
    /// [`Deserialize`]: #impl-TryFromVec%3Cu8%3E-for-Payload
    /// [JSON]: https://www.json.org/
    /// [`Message`]: ../messages/enum.Message.html
    /// [Protobuf]: https://developers.google.com/protocol-buffers
    /// [`Serialize`]: #impl-From%3CPayload%3E-for-Vec%3Cu8%3E
    #[serde_as(as = "DisplayFromStr")]
    payload: Payload,

    /// Unknown field that seems always empty.
    ///
    /// This implementation is provided for sake of completeness and may change
    /// in the future.
    clock: HashMap<String, serde_json::Value>,
}

#[derive(Copy, Clone, Debug, Hash, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub enum MessageType {
    #[serde(rename = "ack")]
    Acknowledgement,
    Close,
    Connect,
    ConnectionOffer,
    DiscoveryRequest,
    PlaybackProgress,
    PublishQueue,
    Ping,
    Ready,
    RefreshQueue,
    Skip,
    Status,
    Stop,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
// `serde_with::serde_as` seems to ignore the `rename_all` pragma together with
// `untagged`, so `rename_all` is repeated for every variant.
#[serde(untagged)]
pub enum Payload {
    #[serde(rename_all = "camelCase")]
    PlaybackProgress {
        queue_id: String,
        element_id: QueueItem,
        #[serde_as(as = "DurationSeconds<u64>")]
        duration: Duration,
        #[serde_as(as = "DurationSeconds<u64>")]
        buffered: Duration,
        progress: Option<Percentage>,
        volume: Percentage,
        quality: AudioQuality,
        is_playing: bool,
        is_shuffle: bool,
        repeat_mode: RepeatMode,
    },

    #[serde(rename_all = "camelCase")]
    Acknowledgement {
        acknowledgement_id: String,
    },

    #[serde(rename_all = "camelCase")]
    Status {
        command_id: String,
        status: Status,
    },

    WithParams {
        from: DeviceId,
        params: Params,
    },

    #[serde(rename_all = "camelCase")]
    Skip {
        queue_id: Option<String>,
        element_id: Option<QueueItem>,
        progress: Option<Percentage>,
        should_play: Option<bool>,
        set_repeat_mode: Option<RepeatMode>,
        set_shuffle: Option<bool>,
        set_volume: Option<Percentage>,
    },

    String(#[serde_as(as = "NoneAsEmptyString")] Option<String>),

    // This protobuf is deserialized manually with `FromStr`.
    #[serde(skip)]
    PublishQueue(queue::List),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Params {
    ConnectionOffer {
        device_name: String,
        device_type: String,
        supported_control_versions: HashSet<String>,
    },

    Connect {
        offer_id: String,
    },

    DiscoveryRequest {
        discovery_session: String,
    },
}

impl fmt::Display for Payload {
    // TODO: UPDATE DOCS
    /// Converts to a [`Vec`]<[`u8`]> from a [`Payload`] of some
    /// [Deezer Connect][Connect] websocket [`Message`] [`Body`].
    ///
    /// [`Payload`]s may be sent over the wire as either [Base64] encoded
    /// [JSON], or [Base64] encoded [protocol buffers][Protobuf] that are
    /// compressed with [DEFLATE]. [Connect] devices may receive
    /// [protocol buffers][Protobuf] but do not seem to send them. However,
    /// because [`From`] must not fail, an implementation for
    /// [protocol buffers][Protobuf] is provided but untested.
    ///
    /// [Base64]: https://datatracker.ietf.org/doc/html/rfc3548
    /// [`Body`]: struct.Body.html
    /// [DEFLATE]: https://datatracker.ietf.org/doc/html/rfc1951
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`From`]: https://doc.rust-lang.org/std/convert/trait.From.html
    /// [JSON]: https://www.json.org/
    /// [`Message`]: ../messages/enum.Message.html
    /// [`Payload`]: enum.Payload.html
    /// [Protobuf]: https://developers.google.com/protocol-buffers
    /// [`u8`]: https://doc.rust-lang.org/std/primitive.u8.html
    /// [`Vec`]: https://doc.rust-lang.org/std/vec/struct.Vec.html
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut buffer: Vec<u8> = vec![];

        if let Payload::PublishQueue(queue) = self {
            trace!("YEAH YEAH YEAH!");
            match queue.write_to_bytes() {
                Ok(protobuf) => {
                    let mut deflater = DeflateEncoder::new(&protobuf[..], Compression::fast());
                    if let Err(e) = deflater.read_to_end(&mut buffer) {
                        error!("{e}");
                        return Err(fmt::Error);
                    }
                }
                Err(e) => {
                    error!("{e}");
                    return Err(fmt::Error);
                }
            }
        } else {
            // Do not Base64 encode empty strings.
            if let Payload::String(s) = self {
                if s.as_ref().map_or(true, String::is_empty) {
                    return Ok(());
                }
            }

            if let Err(e) = serde_json::to_writer(&mut buffer, self) {
                error!("{e}");
                return Err(fmt::Error);
            }
        }

        write!(f, "{}", BASE64_STANDARD.encode(buffer))
    }
}

impl FromStr for Payload {
    type Err = Error;

    /// TODO : first decode base64 in fromstr, then deserialize json with traits
    fn from_str(encoded: &str) -> std::result::Result<Self, Self::Err> {
        let decoded = BASE64_STANDARD.decode(encoded)?;

        if let Ok(s) = std::str::from_utf8(&decoded) {
            // 1. `serde_with::NoneAsEmptyString` does not apply to `FromStr`.
            // 2. Deezer on Android can send empty maps
            if s.is_empty() || s == "{}" {
                return Ok(Self::String(None));
            }

            // Most payloads are strings that contain JSON.
            serde_json::from_str::<Self>(s).map_err(Into::into)
        } else {
            // Some payloads are deflated protobufs.
            let mut inflater = DeflateDecoder::new(&decoded[..]);
            let mut buffer: Vec<u8> = vec![];
            inflater.read_to_end(&mut buffer)?;

            if let Ok(queue) = queue::List::parse_from_bytes(&buffer) {
                // All fields are optional in proto3, so successful parsing
                // does not mean that it parsed the right message.
                if !queue.id.is_empty() {
                    // TODO : why did I comment this out?
                    //     if list.shuffled {
                    //         warn!("encountered shuffled playback queue; please report this to the developers");
                    //         trace!("{list:#?}");
                    //     }
                    //
                    //     let number_of_tracks = list.tracks.len();
                    //     let tracks = if list.tracks_order.len() == number_of_tracks {
                    //         let mut ordered_tracks = Vec::with_capacity(number_of_tracks);
                    //         for index in list.tracks_order {
                    //             ordered_tracks.push(list.tracks[index as usize].clone().id);
                    //         }
                    //         ordered_tracks
                    //     } else {
                    //         list.tracks.into_iter().map(|track| track.id).collect()
                    //     };
                    //
                    //     let tracks = tracks.into_iter().filter_map(|track| track.parse::<u64>().ok()).collect();
                    return Ok(Self::PublishQueue(queue));
                }
            }

            Err(Self::Err::unimplemented(
                "protobuf should match some variant".to_string(),
            ))
        }
    }
}

impl WireBody {
    pub(crate) const COMMAND_VERSION: &'static str = "com.deezer.remote.command.proto1";
    pub(crate) const DISCOVERY_VERSION: &'static str = "com.deezer.remote.discovery.proto1";
    pub(crate) const QUEUE_VERSION: &'static str = "com.deezer.remote.queue.proto1";
    pub(crate) const SUPPORTED_CONTROL_VERSIONS: [&'static str; 1] = ["1.0.0-beta2"];

    #[must_use]
    pub(crate) fn supports_control_versions(control_versions: &HashSet<String>) -> bool {
        for version in control_versions {
            if Self::SUPPORTED_CONTROL_VERSIONS.contains(&version.as_str()) {
                return true;
            }
        }

        false
    }

    #[must_use]
    pub(crate) fn supported_protocol_version(&self) -> bool {
        matches!(
            self.protocol_version.as_ref(),
            WireBody::COMMAND_VERSION | WireBody::DISCOVERY_VERSION | WireBody::QUEUE_VERSION
        )
    }
}

impl From<Body> for WireBody {
    /// Converts to a `WireBody` from a [`Body`](struct.Body.html).
    #[expect(clippy::too_many_lines)]
    fn from(body: Body) -> Self {
        let clock: HashMap<String, serde_json::Value> = HashMap::new();

        match body {
            Body::Acknowledgement {
                message_id,
                acknowledgement_id,
            } => WireBody {
                message_id,
                message_type: MessageType::Acknowledgement,
                protocol_version: Self::COMMAND_VERSION.to_string(),
                payload: Payload::Acknowledgement { acknowledgement_id },
                clock,
            },

            Body::Close { message_id } => WireBody {
                message_id,
                message_type: MessageType::Close,
                protocol_version: Self::COMMAND_VERSION.to_string(),
                payload: Payload::String(None),
                clock,
            },

            Body::Connect {
                message_id,
                from,
                offer_id,
            } => WireBody {
                message_id,
                message_type: MessageType::Connect,
                protocol_version: Self::DISCOVERY_VERSION.to_string(),
                payload: Payload::WithParams {
                    from,
                    params: Params::Connect { offer_id },
                },
                clock,
            },

            Body::ConnectionOffer {
                message_id,
                from,
                device_name,
            } => WireBody {
                message_id,
                message_type: MessageType::ConnectionOffer,
                protocol_version: Self::DISCOVERY_VERSION.to_string(),
                payload: Payload::WithParams {
                    from,
                    params: Params::ConnectionOffer {
                        device_name,
                        device_type: "web".to_string(),
                        supported_control_versions: Self::SUPPORTED_CONTROL_VERSIONS
                            .into_iter()
                            .map(ToString::to_string)
                            .collect(),
                    },
                },
                clock,
            },

            Body::DiscoveryRequest {
                message_id,
                from,
                discovery_session,
            } => WireBody {
                message_id,
                message_type: MessageType::DiscoveryRequest,
                protocol_version: Self::DISCOVERY_VERSION.to_string(),
                payload: Payload::WithParams {
                    from,
                    params: Params::DiscoveryRequest { discovery_session },
                },
                clock,
            },

            Body::Ping { message_id } => WireBody {
                message_id,
                message_type: MessageType::Ping,
                protocol_version: Self::COMMAND_VERSION.to_string(),
                payload: Payload::String(None),
                clock,
            },

            Body::PlaybackProgress {
                message_id,
                track,
                duration,
                buffered,
                progress,
                volume,
                quality,
                is_playing,
                is_shuffle,
                repeat_mode,
            } => WireBody {
                message_id,
                message_type: MessageType::PlaybackProgress,
                protocol_version: Self::COMMAND_VERSION.to_string(),
                payload: Payload::PlaybackProgress {
                    queue_id: track.queue_id.clone(),
                    element_id: track,
                    duration,
                    buffered,
                    progress,
                    volume,
                    quality,
                    is_playing,
                    is_shuffle,
                    repeat_mode,
                },
                clock,
            },

            Body::PublishQueue { message_id, queue } => WireBody {
                message_id,
                message_type: MessageType::PublishQueue,
                protocol_version: Self::QUEUE_VERSION.to_string(),
                payload: Payload::PublishQueue(queue),
                clock,
            },

            Body::Ready { message_id } => WireBody {
                message_id,
                message_type: MessageType::Ready,
                protocol_version: Self::COMMAND_VERSION.to_string(),
                payload: Payload::String(None),
                clock,
            },

            Body::RefreshQueue { message_id } => WireBody {
                message_id,
                message_type: MessageType::RefreshQueue,
                protocol_version: Self::QUEUE_VERSION.to_string(),
                payload: Payload::String(None),
                clock,
            },

            Body::Skip {
                message_id,
                queue_id,
                track,
                progress,
                should_play,
                set_shuffle,
                set_repeat_mode,
                set_volume,
            } => WireBody {
                message_id,
                message_type: MessageType::Skip,
                protocol_version: Self::COMMAND_VERSION.to_string(),
                payload: Payload::Skip {
                    queue_id,
                    element_id: track,
                    progress,
                    should_play,
                    set_shuffle,
                    set_repeat_mode,
                    set_volume,
                },
                clock,
            },

            Body::Status {
                message_id,
                command_id,
                status,
            } => WireBody {
                message_id,
                message_type: MessageType::Status,
                protocol_version: Self::COMMAND_VERSION.to_string(),
                payload: Payload::Status { command_id, status },
                clock,
            },

            Body::Stop { message_id } => WireBody {
                message_id,
                message_type: MessageType::Stop,
                protocol_version: Self::COMMAND_VERSION.to_string(),
                payload: Payload::String(None),
                clock,
            },
        }
    }
}

impl TryFrom<WireBody> for Body {
    type Error = Error;

    /// Performs the conversion from [`WireBody`] into `Body`.
    ///
    /// [`WireMessage`]: struct.WireMessage.html
    #[expect(clippy::too_many_lines)]
    fn try_from(wire_body: WireBody) -> std::result::Result<Self, Self::Error> {
        if !wire_body.supported_protocol_version() {
            warn!("protocol version {} is unknown", wire_body.protocol_version);
        }

        let message_id = wire_body.message_id;
        let message_type = wire_body.message_type;

        let body = match message_type {
            MessageType::Acknowledgement => {
                if let Payload::Acknowledgement { acknowledgement_id } = wire_body.payload {
                    Self::Acknowledgement {
                        message_id,
                        acknowledgement_id,
                    }
                } else {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::failed_precondition(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            }

            MessageType::Close => Body::Close { message_id },

            MessageType::Connect => {
                if let Payload::WithParams { from, params } = wire_body.payload {
                    if let Params::Connect { offer_id } = params {
                        Self::Connect {
                            message_id,
                            from,
                            offer_id,
                        }
                    } else {
                        trace!("{params:#?}");
                        return Err(Self::Error::failed_precondition(format!(
                            "params should match message type {message_type}"
                        )));
                    }
                } else {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::failed_precondition(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            }

            MessageType::ConnectionOffer => {
                if let Payload::WithParams { from, params } = wire_body.payload {
                    if let Params::ConnectionOffer {
                        device_name,
                        supported_control_versions,
                        ..
                    } = params
                    {
                        if !WireBody::supports_control_versions(&supported_control_versions) {
                            warn!(
                                "control versions {:?} are unknown",
                                supported_control_versions
                            );
                        }

                        Self::ConnectionOffer {
                            message_id,
                            from,
                            device_name,
                        }
                    } else {
                        trace!("{params:#?}");
                        return Err(Self::Error::failed_precondition(format!(
                            "params should match message type {message_type}"
                        )));
                    }
                } else {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::failed_precondition(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            }

            MessageType::DiscoveryRequest => {
                if let Payload::WithParams { from, params } = wire_body.payload {
                    if let Params::DiscoveryRequest { discovery_session } = params {
                        Self::DiscoveryRequest {
                            message_id,
                            from,
                            discovery_session,
                        }
                    } else {
                        trace!("{params:#?}");
                        return Err(Self::Error::failed_precondition(format!(
                            "params should match message type {message_type}"
                        )));
                    }
                } else {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::failed_precondition(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            }

            MessageType::Ping => Self::Ping { message_id },

            MessageType::PlaybackProgress => {
                if let Payload::PlaybackProgress {
                    element_id,
                    buffered,
                    duration,
                    progress,
                    volume,
                    quality,
                    is_playing,
                    is_shuffle,
                    repeat_mode,
                    ..
                } = wire_body.payload
                {
                    Self::PlaybackProgress {
                        message_id,
                        track: element_id,
                        buffered,
                        duration,
                        progress,
                        volume,
                        quality,
                        is_playing,
                        is_shuffle,
                        repeat_mode,
                    }
                } else {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::failed_precondition(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            }

            MessageType::PublishQueue => {
                if let Payload::PublishQueue(queue) = wire_body.payload {
                    Self::PublishQueue { message_id, queue }
                } else {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::failed_precondition(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            }

            MessageType::Ready => Self::Ready { message_id },

            MessageType::RefreshQueue => Self::RefreshQueue { message_id },

            MessageType::Skip => {
                if let Payload::Skip {
                    queue_id,
                    element_id,
                    progress,
                    should_play,
                    set_shuffle,
                    set_repeat_mode,
                    set_volume,
                    ..
                } = wire_body.payload
                {
                    Self::Skip {
                        message_id,
                        queue_id,
                        track: element_id,
                        progress,
                        should_play,
                        set_shuffle,
                        set_repeat_mode,
                        set_volume,
                    }
                } else {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::failed_precondition(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            }

            MessageType::Status => {
                if let Payload::Status { command_id, status } = wire_body.payload {
                    Self::Status {
                        message_id,
                        command_id,
                        status,
                    }
                } else {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::failed_precondition(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            }

            MessageType::Stop => Self::Stop { message_id },
        };

        Ok(body)
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
