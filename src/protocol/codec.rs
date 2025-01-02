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

/// Converts a string to a [`Codec`].
///
/// # Supported formats
/// - AAC: "aac", "m4a", "m4b"
/// - FLAC: "flac"
/// - MP3: "mp3"
///
/// # Errors
/// Returns [`Error::invalid_argument`] if the string doesn't match any supported codec format.
///
/// # Examples
/// ```
/// use std::str::FromStr;
/// use pleezer::protocol::Codec;
///
/// assert_eq!(Codec::from_str("aac").unwrap(), Codec::AAC);
/// assert_eq!(Codec::from_str("m4a").unwrap(), Codec::AAC);
/// assert_eq!(Codec::from_str("m4b").unwrap(), Codec::AAC);
/// assert_eq!(Codec::from_str("flac").unwrap(), Codec::FLAC);
/// assert_eq!(Codec::from_str("mp3").unwrap(), Codec::MP3);
///
/// assert!(Codec::from_str("wav").is_err());
/// ```
impl FromStr for Codec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "aac" | "m4a" | "m4b" => Ok(Codec::AAC),
            "flac" => Ok(Codec::FLAC),
            "mp3" => Ok(Codec::MP3),
            _ => Err(Error::invalid_argument(format!("{s} is not a valid codec"))),
        }
    }
}
