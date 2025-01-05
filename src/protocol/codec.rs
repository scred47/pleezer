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
use std::{fmt, str::FromStr, time::Duration};

use crate::{error::Error, util::ToF32};

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

impl Codec {
    /// AAC frames are fixed at 1024 samples.
    const AAC_SAMPLES_PER_FRAME: usize = 1_024;

    /// FLAC frames are variable, but may not exceed 4,608 samples up to 48 kHz.
    const FLAC_MAX_SAMPLES_PER_FRAME: usize = 4_608;

    /// FLAC frames are variable, but may not exceed 16,384 samples above 48 kHz.
    const FLAC_MAX_SAMPLES_PER_FRAME_HI_RES: usize = 16_384;

    /// MP3 frames are fixed at 1,152 samples.
    const MP3_SAMPLES_PER_FRAME: usize = 1_152;

    /// Returns the maximum duration of a frame for the codec at the given sample rate.
    ///
    /// Frame sizes at 44.1 kHz:
    /// * AAC: 1024 samples ≈ 23.220ms (fixed)
    /// * MP3: 1152 samples ≈ 26.122ms (fixed)
    /// * FLAC: Up to 4608 samples ≈ 104.490ms (variable)
    ///
    /// For FLAC at higher sample rates (>48 kHz), allows up to 16384 samples per frame.
    ///
    /// Note: While FLAC frames can be variable length, we return the maximum possible
    /// frame duration to ensure seeks land before frame boundaries.
    #[must_use]
    pub fn frame_duration(&self, sample_rate: usize) -> Duration {
        let samples = match self {
            Codec::AAC => Self::AAC_SAMPLES_PER_FRAME,
            Codec::FLAC => {
                if sample_rate > 48_000 {
                    Self::FLAC_MAX_SAMPLES_PER_FRAME_HI_RES
                } else {
                    Self::FLAC_MAX_SAMPLES_PER_FRAME
                }
            }
            Codec::MP3 => Self::MP3_SAMPLES_PER_FRAME,
        }
        .to_f32_lossy();

        let span = (samples / sample_rate.to_f32_lossy()).clamp(0.0, Duration::MAX.as_secs_f32());
        if span.is_nan() {
            Duration::default()
        } else {
            Duration::from_secs_f32(span)
        }
    }
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
