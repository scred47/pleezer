use clap::ValueEnum;
use serde_repr::{Deserialize_repr, Serialize_repr};

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
    ValueEnum,
)]
// `u64` because this is serialized into and deserialized from JSON.
#[repr(u64)]
pub enum Quality {
    /// 64 kbps MP3
    Basic = 0,

    /// 128 kbps MP3 (default)
    #[default]
    Standard = 1,

    /// 320 kbps MP3 (requires Premium subscription)
    High = 2,

    /// 1411 kbps FLAC (requires HiFi subscription)
    Lossless = 3,
    // `Unknown` may be received over the wire but has undefined behavior.
    // There is no point in parsing or offering it.
    // Unknown = -1,
}
