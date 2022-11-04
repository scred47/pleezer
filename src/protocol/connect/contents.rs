use std::{
    collections::HashMap,
    fmt::{self, Write},
    io::Read,
};

use flate2::{Compression, read::{DeflateDecoder, DeflateEncoder}};
use serde::{de::Error, de::IntoDeserializer, Deserialize, Deserializer, Serialize, Serializer};
use serde_with::{json::JsonString, serde_as};

use super::channel::Event;

// Import the generated Rust protobufs.
include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));

/// The `Contents` of a [`Message`] on a [Deezer Connect][Connect] websocket.
///
/// [`Message`]: ../messages/enum.Message.html
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Contents {
    /// The [Deezer Connect][Connect] websocket [`Message`] [`Event`] that
    /// these `Contents` are for.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Event`]: ../channel/enum.Event.html
    /// [`Message`]: ../messages/enum.Message.html
    #[serde(rename = "APP")]
    pub event: Event,

    // The [Deezer Connect][Connect] websocket [`Message`] [`Headers`] that are
    // attached to these `Contents`.
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
    /// [JsonString]: https://docs.rs/serde_with/latest/serde_with/json/struct.JsonString.html
    /// [`Message`]: ../messages/enum.Message.html
    /// [`String`]: https://doc.rust-lang.org/std/string/
    #[serde_as(as = "JsonString")]
    pub body: Body,
}

/// The `Headers` attached to some [`Message`] [`Contents`] on a
/// [Deezer Connect][Connect] websocket.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
/// [`Contents`]: struct.Contents.html
/// [`Message`]: ../messages/enum.Message.html
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Headers {
    /// The source of some [`Message`] [`Contents`].
    ///
    /// [`Contents`]: struct.Contents.html
    /// [`Message`]: ../messages/enum.Message.html
    pub from: String,

    /// The optional destination for some [`Message`] [`Contents`].
    ///
    /// [`Contents`]: struct.Contents.html
    /// [`Message`]: ../messages/enum.Message.html
    pub destination: Option<String>,
}

/// The [`Body`] of some [Deezer Connect][Connect] websocket [`Message`]
/// [`Contents`].
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
/// [`Contents`]: struct.Contents.html
/// [`Message`]: ../messages/enum.Message.html
#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "camelCase")]
pub struct Body {
    /// The [`Uuid`] of some [Deezer Connect][Connect] websocket [`Message`]
    /// that has this `Body`.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Message`]: ../messages/enum.Message.html
    /// [`Uuid`]: https://docs.rs/uuid/latest/uuid/
    pub message_id: String,

    /// The [`MessageType`] that tags the `payload` of some
    /// [Deezer Connect][Connect] websocket [`Message`] that has this `Body`.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Message`]: ../messages/enum.Message.html
    /// [`MessageType`]: enum.MessageType.html
    pub message_type: String,

    /// The protocol version of some [Deezer Connect][Connect] websocket
    /// [`Message`] that has this `Body`.
    ///
    /// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
    /// [`Message`]: ../messages/enum.Message.html
    pub protocol_version: String,

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
    #[serde_as(as = "TryFromInto<Vec<u8>>")]
    pub payload: Payload,

    /// Unknown field that seems always empty.
    ///
    /// This implementation is provided for sake of completeness and may change
    /// in the future.
    pub clock: HashMap<String, Value>,
}

#[derive(Clone, Deserialize, PartialEq, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum RemotePayload {
    // TODO:
    // Ready - empty map payload?
    // Ping - empty map payload?
    // Close - empty map payload?
    // Stop - empty map payload .. internal function? disconnect, cache.clear, queueneedsrefresh false, closed true.
    // RefreshQueue - empty string payload?
    // PublishQueue == PlaybackQueue?
    // Connect / Disconnect - internal functions?
    // PlaybackStatus = Status?
    PlaybackProgress {
        queue_id: String,
        element_id: String,
        progress: f64,
        buffered: i64,
        duration: i64,
        quality: i64,
        volume: f64,
        is_playing: bool,
        is_shuffle: bool,
        repeat_mode: i64,
    },
    Ack {
        acknowledgement_id: String,
    },
    PlaybackStatus {
        command_id: String,
        status: i64,
    },
    WithParams {
        from: String,
        params: RemoteParams,
    },
    Skip {
        queue_id: String,
        element_id: String,
        progress: f64,
        should_play: bool,
        set_shuffle: bool,
        set_repeat_mode: i64,
        set_volume: f64,
    },
    // This protobuf is deserialized manually with `TryFromInto`.
    #[serde(skip)]
    PlaybackQueue(queue::List),
}

#[derive(Clone, Deserialize, Eq, PartialEq, Serialize, Debug)]
#[serde(rename_all = "camelCase")]
#[serde(untagged)]
pub enum RemoteParams {
    ConnectionOffer {
        device_name: String,
        device_type: String,
        supported_control_versions: Vec<String>,
    },
    Connect {
        offer_id: String,
    },
    DiscoveryRequest {
        discovery_session: String,
    },
}

impl From<Payload> for Vec<u8> {
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
    /// [`Vec`] https://doc.rust-lang.org/std/vec/struct.Vec.html
    fn from(v: Payload) -> Self {
        let mut buffer: Vec<u8> = vec![];
        let result = match v {
            Payload::PlaybackQueue(list) => {
                let protobuf = list.write_to_bytes();
                let mut deflater = DeflateEncoder::new(&protobuf[..], Compression::fast());
                deflater.read_to_end(&mut buffer)
            },
            _ => v.to_writer(buffer),
        };
        
        if let Err(e) = result {
            // Do not panic in `From`. Worst case: serialization failed and
            // an empty buffer will be Base64 encoded. The error message may be
            // forwarded transparently as both `flate2` and `serde_json`
            // provide good messages by themselves.
            error!("{e}"); 
        }
        
        base64::encode(buffer)
    }
}

impl TryFrom<Vec<u8>> for Payload {
    type Error = super::Error;
    fn try_from(v: String) -> Result<Self, Self::Error> {
        let decoded = base64::decode(&v)?;

        match std::str::from_utf8(decoded.clone()) {
            Ok(s) => {
                // Most payloads are strings that contain JSON.
                serde_json::from_str::<Self>(s)?
            }
            Err(_) => {
                // Some payloads are deflated protobufs.
                let mut inflater = DeflateDecoder::new(&decoded[..]);
                let mut buffer: Vec<u8> = vec![];
                inflater
                    .read_to_end(&mut buffer)?;

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

                Err(Self::Error::Unsupported("protobuf should match some variant".to_string())
            }
        }
    }
}
