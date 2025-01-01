//! Audio codec support for Deezer content.
//!
//! Defines supported audio formats:
//! * AAC - Advanced Audio Coding (streams)
//! * FLAC - Free Lossless Audio Codec (downloads)
//! * MP3 - MPEG Layer-3 (both)
//!
//! Different content types support different codecs:
//! * Songs - MP3 and FLAC
//! * Episodes - MP3 only
//! * Livestreams - AAC and MP3

use serde::Serialize;
use std::fmt;

/// Supported audio codecs for live streams.
/// Note: Deezer does not use FLAC for live streams.
#[derive(Clone, Eq, PartialEq, Serialize, Debug, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Codec {
    /// Advanced Audio Coding
    AAC,
    /// Free Lossless Audio Codec
    FLAC,
    /// MPEG Layer-3
    MP3,
}

/// Formats codec type for display.
///
/// Used for serialization and logging. Shows codec name in uppercase:
/// * "AAC"
/// * "FLAC"
/// * "MP3"
impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}
