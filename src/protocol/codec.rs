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

use serde_with::SerializeDisplay;
use std::{fmt, str::FromStr};

use crate::error::Error;

/// Supported audio codecs for live streams.
/// Note: Deezer does not use FLAC for live streams.
#[derive(Copy, Clone, Default, Eq, PartialEq, SerializeDisplay, Debug, Hash)]
pub enum Codec {
    /// Advanced Audio Coding
    AAC,
    /// Free Lossless Audio Codec
    FLAC,
    /// MPEG Layer-3
    #[default]
    MP3,
}

/// Formats codec type for display.
///
/// Used for serialization and logging. Shows codec name in lowercase:
/// * "aac"
/// * "flac"
/// * "mp3"
impl fmt::Display for Codec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Codec::AAC => write!(f, "aac"),
            Codec::FLAC => write!(f, "flac"),
            Codec::MP3 => write!(f, "mp3"),
        }
    }
}

impl FromStr for Codec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "aac" => Ok(Codec::AAC),
            "flac" => Ok(Codec::FLAC),
            "mp3" => Ok(Codec::MP3),
            _ => Err(Error::invalid_argument(format!("{s} is not a valid codec"))),
        }
    }
}
