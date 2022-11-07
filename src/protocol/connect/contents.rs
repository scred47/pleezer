use std::{
    collections::HashMap,
    fmt::{self, Write},
    io::Read,
    num::NonZeroU64,
    str::FromStr,
};

use flate2::{
    read::{DeflateDecoder, DeflateEncoder},
    Compression,
};
use protobuf::{EnumOrUnknown, Message};
use serde::{de::Error, de::IntoDeserializer, Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{
    json::JsonString, serde_as, DeserializeFromStr, NoneAsEmptyString, SerializeDisplay,
    TryFromInto,
};
use uuid::Uuid;

use super::channel::Event;
use crate::audio::Quality;

// Import the generated Rust protobufs.
include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

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
    pub event: Event,

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
    pub from: Device,

    /// The optional destination for some [`Message`] [`Contents`].
    ///
    /// [`Contents`]: struct.Contents.html
    /// [`Message`]: ../messages/enum.Message.html
    pub destination: Option<Device>,
}

#[derive(
    Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum Device {
    Uuid(Uuid),
    Other(String),
}

impl From<Uuid> for Device {
    fn from(uuid: Uuid) -> Self {
        Self::Uuid(uuid)
    }
}

impl FromStr for Device {
    type Err = super::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let device = match Uuid::try_parse(&s) {
            Ok(uuid) => Self::from(uuid),
            Err(_) => Self::Other(s.to_owned()),
        };

        Ok(device)
    }
}

impl fmt::Display for Device {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Device::Uuid(uuid) => write!(f, "{uuid}"),
            Device::Other(s) => write!(f, "{s}"),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Body {
    Acknowledgement {
        message_id: Uuid,
        acknowledgement_id: Uuid,
    },
    Close {
        message_id: Uuid,
    },
    Connect {
        message_id: Uuid,
        from: Device,
        offer_id: Uuid,
    },
    ConnectionOffer {
        message_id: Uuid,
        from: Device,
        device_name: String,
    },
    DiscoveryRequest {
        message_id: Uuid,
        from: Device,
        discovery_session: Uuid,
    },
    PlaybackProgress {
        message_id: Uuid,
        queue_id: Uuid,
        track: Element,
        progress: Ratio,
        buffered: f64,
        duration: f64,
        quality: Quality,
        volume: Ratio,
        is_playing: bool,
        is_shuffle: bool,
        repeat_mode: Repeat,
    },
    PlaybackQueue {
        message_id: Uuid,
        list: queue::List,
    },
    Ping {
        message_id: Uuid,
    },
    Ready {
        message_id: Uuid,
    },
    Skip {
        message_id: Uuid,
        queue_id: Uuid,
        track: Element,
        progress: Ratio,
        should_play: bool,
        set_repeat_mode: Repeat,
        set_shuffle: Option<bool>,
        set_volume: Option<Ratio>,
    },
    Status {
        message_id: Uuid,
        command_id: Uuid,
        status: Status,
    },
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize_repr,
    Deserialize_repr,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    Hash,
)]
#[repr(u64)]
pub enum Status {
    OK = 0,

    // Assume failure unless explicitly specified otherwise. This is what
    // Deezer Connect does itself.
    #[default]
    Error = 1,
}

#[derive(
    Copy,
    Clone,
    Debug,
    Default,
    Serialize_repr,
    Deserialize_repr,
    PartialOrd,
    Ord,
    PartialEq,
    Eq,
    Hash,
)]
#[repr(i64)]
pub enum Repeat {
    #[default]
    RepeatNone = 0,
    RepeatAll = 1,
    RepeatOne = 2,
    Unrecognized = -1,
}

#[derive(Copy, Clone, Debug, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Ratio(f64);

impl Ratio {
    pub fn new(ratio: f64) -> Result<Self, super::Error> {
        match Self::is_valid(ratio) {
            true => Ok(Self(ratio)),
            false => Err(super::Error::InvalidInput(
                "ratio should be within bounds".to_string(),
            )),
        }
    }

    pub fn is_valid(ratio: f64) -> bool {
        ratio >= 0.0 && ratio <= 1.0
    }
}

impl From<Ratio> for f64 {
    fn from(ratio: Ratio) -> Self {
        ratio.0
    }
}

impl TryFrom<f64> for Ratio {
    type Error = super::Error;

    fn try_from(ratio: f64) -> Result<Self, Self::Error> {
        Self::new(ratio)
    }
}

#[derive(
    Copy, Clone, Debug, SerializeDisplay, DeserializeFromStr, PartialOrd, Ord, PartialEq, Eq, Hash,
)]
pub struct Element {
    pub queue_id: Uuid,
    pub track_id: NonZeroU64,
    // `u32` instead of `u64`: known from the protobuf.
    pub position: u32,
}

impl fmt::Display for Element {
    /// Formats an `Event` as a wire string for use on a
    /// [Deezer Connect][Connect] websocket.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        todo!()
    }
}

impl FromStr for Element {
    type Err = super::Error;

    /// Parses a wire string `s` on a [Deezer Connect][Connect] websocket to
    /// return an `Element` on a queue.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('-').into_iter();

        let mut queue_id = String::new();
        for i in 0..5 {
            match parts.next() {
                Some(part) => write!(queue_id, "{part}")?,
                None => {
                    return Err(Self::Err::Malformed(format!(
                        "element string slice should hold five `queue_id` parts, found {i}"
                    )))
                }
            }
        }
        let queue_id = Uuid::try_parse(&queue_id)
            .map_err(|e| Self::Err::Malformed(format!("queue id: {e}")))?;

        let track_id = parts.next().ok_or(Self::Err::Malformed(
            "element string slice should hold `track_id` part".to_string(),
        ))?;
        let track_id = track_id
            .parse::<NonZeroU64>()
            .map_err(|e| Self::Err::Malformed(format!("track id: {e}")))?;

        let position = parts.next().ok_or(Self::Err::Malformed(
            "element string slice should hold `position` part".to_string(),
        ))?;
        let position = position
            .parse::<u32>()
            .map_err(|e| Self::Err::Malformed(format!("position: {e}")))?;

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
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let json_message = WireBody::from(self.clone());
        let json =
            serde_json::to_string(&json_message).map_err(|e| serde::ser::Error::custom(e))?;
        serializer.collect_str(&json)
    }
}

// For syntactic sugar this could be changed into `serde_with::DeserializeAs` but
// this now follows the same idiom as deserializing a `Message`.
impl<'de> Deserialize<'de> for Body {
    /// Deserialize [JSON] into a [`WireBody`], then convert it into a `Body`.
    ///
    /// [JSON]: https://www.json.org/
    /// [`WireMessage`]: enum.WireMessage.html
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let json_message = WireBody::deserialize(deserializer)?;
        Self::try_from(json_message).map_err(|e| serde::de::Error::custom(e))
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
    message_id: Uuid,

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
    /// [TryFromInto]: https://docs.rs/serde_with/latest/serde_with/struct.TryFromInto.html
    ///
    // `TryFromInto<Vec<u8>>` fails to deserialize and may not be faster, as it
    // still requires cloning for ownership.
    #[serde_as(as = "TryFromInto<String>")]
    payload: Payload,

    /// Unknown field that seems always empty.
    ///
    /// This implementation is provided for sake of completeness and may change
    /// in the future.
    clock: HashMap<String, Value>,
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
    PlaybackQueue,
    Ping,
    Ready,
    Skip,
    Status,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
// `serde_with::serde_as` seems to ignore the `rename_all` pragma together with
// `untagged`, so `rename_all` is repeated for every variant.
#[serde(untagged)]
pub enum Payload {
    #[serde(rename_all = "camelCase")]
    PlaybackProgress {
        queue_id: Uuid,
        element_id: Element,
        progress: Ratio,
        buffered: f64,
        duration: f64,
        quality: Quality,
        volume: Ratio,
        is_playing: bool,
        is_shuffle: bool,
        repeat_mode: Repeat,
    },
    #[serde(rename_all = "camelCase")]
    Acknowledgement {
        acknowledgement_id: Uuid,
    },
    #[serde(rename_all = "camelCase")]
    Status {
        command_id: Uuid,
        status: Status,
    },
    WithParams {
        from: Device,
        params: Params,
    },
    #[serde(rename_all = "camelCase")]
    Skip {
        queue_id: Uuid,
        element_id: Element,
        progress: Ratio,
        should_play: bool,
        set_repeat_mode: Repeat,
        set_shuffle: Option<bool>,
        set_volume: Option<Ratio>,
    },

    Str(#[serde_as(as = "NoneAsEmptyString")] Option<String>),

    // This protobuf is deserialized manually with `TryFromInto`.
    #[serde(skip)]
    PlaybackQueue(queue::List),
}

#[derive(Clone, Debug, Hash, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", untagged)]
pub enum Params {
    ConnectionOffer {
        device_name: String,
        device_type: String,
        supported_control_versions: Vec<String>,
    },
    Connect {
        offer_id: Uuid,
    },
    DiscoveryRequest {
        discovery_session: Uuid,
    },
}

impl From<Payload> for String {
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
    fn from(payload: Payload) -> Self {
        let mut buffer: Vec<u8> = vec![];

        match &payload {
            Payload::PlaybackQueue(list) => {
                // Do not panic in `From`. Worst case: serialization failed and
                // an empty or partial buffer will be Base64 encoded. The error
                // message may be forwarded transparently as both `flate2` and
                // `serde_json` provide clear enough messages by themselves.
                match list.write_to_bytes() {
                    Ok(protobuf) => {
                        let mut deflater = DeflateEncoder::new(&protobuf[..], Compression::fast());
                        if let Err(e) = deflater.read_to_end(&mut buffer) {
                            error!("{e}");
                        }
                    }
                    Err(e) => error!("{e}"),
                }
            }
            _ => {
                if let Err(e) = serde_json::to_writer(&mut buffer, &payload) {
                    error!("{e}");
                }
            }
        }

        base64::encode(buffer)
    }
}

impl TryFrom<String> for Payload {
    type Error = super::Error;

    fn try_from(encoded: String) -> Result<Self, Self::Error> {
        let decoded = base64::decode(encoded)?;

        match std::str::from_utf8(&decoded) {
            Ok(s) => {
                // Most payloads are strings that contain JSON.
                serde_json::from_str::<Self>(s).map_err(Into::into)
            }
            Err(_) => {
                // Some payloads are deflated protobufs.
                let mut inflater = DeflateDecoder::new(&decoded[..]);
                let mut buffer: Vec<u8> = vec![];
                inflater.read_to_end(&mut buffer)?;

                if let Ok(list) = queue::List::parse_from_bytes(&buffer) {
                    // All fields are optional in proto3, so successful parsing
                    // does not mean that it parsed the right message.
                    if !list.id.is_empty() {
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
                        return Ok(Self::PlaybackQueue(list));
                    }
                }

                Err(Self::Error::Unsupported(
                    "protobuf should match some variant".to_string(),
                ))
            }
        }
    }
}

impl From<Body> for WireBody {
    /// Converts to a `WireBody` from a [`Body`](struct.Body.html).
    fn from(body: Body) -> Self {
        const COMMAND_VERSION: &'static str = "com.deezer.remote.command.proto1";
        const DISCOVERY_VERSION: &'static str = "com.deezer.remote.command.proto1";
        const QUEUE_VERSION: &'static str = "com.deezer.remote.queue.proto1";

        let clock: HashMap<String, Value> = HashMap::new();

        match body {
            Body::Acknowledgement {
                message_id,
                acknowledgement_id,
            } => WireBody {
                message_id,
                message_type: MessageType::Acknowledgement,
                protocol_version: COMMAND_VERSION.to_string(),
                payload: Payload::Acknowledgement { acknowledgement_id },
                clock,
            },
            Body::Close { message_id } => WireBody {
                message_id,
                message_type: MessageType::Close,
                protocol_version: COMMAND_VERSION.to_string(),
                payload: Payload::Str(None),
                clock,
            },
            Body::Connect {
                message_id,
                from,
                offer_id,
            } => WireBody {
                message_id,
                message_type: MessageType::Connect,
                protocol_version: DISCOVERY_VERSION.to_string(),
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
                protocol_version: DISCOVERY_VERSION.to_string(),
                payload: Payload::WithParams {
                    from,
                    params: Params::ConnectionOffer {
                        device_name,
                        device_type: "web".to_string(),
                        supported_control_versions: vec!["1.0.0-beta2".to_string()],
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
                protocol_version: DISCOVERY_VERSION.to_string(),
                payload: Payload::WithParams {
                    from,
                    params: Params::DiscoveryRequest { discovery_session },
                },
                clock,
            },
            Body::Ping { message_id } => WireBody {
                message_id,
                message_type: MessageType::Ping,
                protocol_version: COMMAND_VERSION.to_string(),
                payload: Payload::Str(None),
                clock,
            },
            Body::PlaybackProgress {
                message_id,
                queue_id,
                track,
                progress,
                buffered,
                duration,
                quality,
                volume,
                is_playing,
                is_shuffle,
                repeat_mode,
            } => WireBody {
                message_id,
                message_type: MessageType::PlaybackProgress,
                protocol_version: COMMAND_VERSION.to_string(),
                payload: Payload::PlaybackProgress {
                    queue_id,
                    element_id: track,
                    progress,
                    buffered,
                    duration,
                    quality,
                    volume,
                    is_playing,
                    is_shuffle,
                    repeat_mode,
                },
                clock,
            },
            Body::PlaybackQueue { message_id, list } => WireBody {
                message_id,
                message_type: MessageType::PlaybackQueue,
                protocol_version: QUEUE_VERSION.to_string(),
                payload: Payload::PlaybackQueue(list),
                clock,
            },
            Body::Ready { message_id } => WireBody {
                message_id,
                message_type: MessageType::Ready,
                protocol_version: COMMAND_VERSION.to_string(),
                payload: Payload::Str(None),
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
                protocol_version: COMMAND_VERSION.to_string(),
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
                protocol_version: COMMAND_VERSION.to_string(),
                payload: Payload::Status { command_id, status },
                clock,
            },
        }
    }
}

impl TryFrom<WireBody> for Body {
    type Error = super::Error;

    /// Performs the conversion from [`WireBody`] into `Body`.
    ///
    /// [`WireMessage`]: struct.WireMessage.html
    fn try_from(wire_body: WireBody) -> Result<Self, Self::Error> {
        let message_id = wire_body.message_id;
        let message_type = wire_body.message_type;

        let body = match message_type {
            MessageType::Acknowledgement => match wire_body.payload {
                Payload::Acknowledgement { acknowledgement_id } => Body::Acknowledgement {
                    message_id,
                    acknowledgement_id,
                },
                _ => {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::Malformed(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            },
            MessageType::Close => Body::Close { message_id },
            MessageType::Connect => match wire_body.payload {
                Payload::WithParams { from, params } => match params {
                    Params::Connect { offer_id } => Body::Connect {
                        message_id,
                        from,
                        offer_id,
                    },
                    _ => {
                        trace!("{params:#?}");
                        return Err(Self::Error::Malformed(format!(
                            "params should match message type {message_type}"
                        )));
                    }
                },
                _ => {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::Malformed(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            },
            MessageType::ConnectionOffer => match wire_body.payload {
                Payload::WithParams { from, params } => match params {
                    Params::ConnectionOffer { device_name, .. } => Body::ConnectionOffer {
                        message_id,
                        from,
                        device_name,
                    },
                    _ => {
                        trace!("{params:#?}");
                        return Err(Self::Error::Malformed(format!(
                            "params should match message type {message_type}"
                        )));
                    }
                },
                _ => {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::Malformed(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            },
            MessageType::DiscoveryRequest => match wire_body.payload {
                Payload::WithParams { from, params } => match params {
                    Params::DiscoveryRequest { discovery_session } => Body::DiscoveryRequest {
                        message_id,
                        from,
                        discovery_session,
                    },
                    _ => {
                        trace!("{params:#?}");
                        return Err(Self::Error::Malformed(format!(
                            "params should match message type {message_type}"
                        )));
                    }
                },
                _ => {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::Malformed(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            },
            MessageType::Ping => Body::Ping { message_id },
            MessageType::PlaybackProgress => match wire_body.payload {
                Payload::PlaybackProgress {
                    queue_id,
                    element_id,
                    progress,
                    buffered,
                    duration,
                    quality,
                    volume,
                    is_playing,
                    is_shuffle,
                    repeat_mode,
                } => Body::PlaybackProgress {
                    message_id,
                    queue_id,
                    track: element_id,
                    progress,
                    buffered,
                    duration,
                    quality,
                    volume,
                    is_playing,
                    is_shuffle,
                    repeat_mode,
                },
                _ => {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::Malformed(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            },
            MessageType::PlaybackQueue => match wire_body.payload {
                Payload::PlaybackQueue(list) => Body::PlaybackQueue { message_id, list },
                _ => {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::Malformed(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            },
            MessageType::Ready => Body::Ready { message_id },
            MessageType::Skip => match wire_body.payload {
                Payload::Skip {
                    queue_id,
                    element_id,
                    progress,
                    should_play,
                    set_shuffle,
                    set_repeat_mode,
                    set_volume,
                } => Body::Skip {
                    message_id,
                    queue_id,
                    track: element_id,
                    progress,
                    should_play,
                    set_shuffle,
                    set_repeat_mode,
                    set_volume,
                },
                _ => {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::Malformed(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            },
            MessageType::Status => match wire_body.payload {
                Payload::Status { command_id, status } => Body::Status {
                    message_id,
                    command_id,
                    status,
                },
                _ => {
                    trace!("{:#?}", wire_body.payload);
                    return Err(Self::Error::Malformed(format!(
                        "payload should match message type {message_type}"
                    )));
                }
            },
        };

        Ok(body)
    }
}

impl fmt::Display for MessageType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
