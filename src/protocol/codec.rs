//! Audio format support for Deezer content.
//!
//! This module handles both audio containers and codecs:
//!
//! Containers:
//! * ADTS - Audio Data Transport Stream (AAC)
//! * FLAC - Free Lossless Audio Codec (native container)
//! * MP3 - MPEG Layer-3 (native container)
//! * MP4 - MPEG-4 Part 14 (AAC, MP3 or even FLAC)
//! * WAV - Waveform Audio File Format (PCM)
//!
//! Codecs:
//! * AAC - Advanced Audio Coding (in ADTS or MP4)
//! * FLAC - Free Lossless Audio Codec
//! * MP3 - MPEG Layer-3
//! * PCM - Pulse Code Modulation (in WAV)
//!
//! Content type mapping:
//! * Songs: MP3 or FLAC (native containers)
//! * Episodes: MP3, MP4 (AAC), or WAV
//! * Livestreams: ADTS (AAC) or MP3

use serde_with::SerializeDisplay;
use std::{fmt, str::FromStr, time::Duration};

use crate::{error::Error, util::ToF32};

/// Supported audio formats.
#[derive(Copy, Clone, Default, Eq, PartialEq, SerializeDisplay, Debug, Hash)]
pub enum Codec {
    /// Audio Data Transport Stream container
    ///
    /// A container format specifically for AAC audio streams.
    /// Used for live streams and some podcasts.
    ADTS,

    /// FLAC native container
    ///
    /// Both a codec and container format for lossless compression.
    /// High-Fidelity format for Deezer's streaming catalogue.
    FLAC,

    /// MP3 native container
    ///
    /// Both a codec and container format.
    /// Primary format for Deezer's streaming catalogue.
    #[default]
    MP3,

    /// MPEG-4 Part 14 container
    ///
    /// A container format that typically holds AAC audio but can also contain MP3
    /// or even FLAC streams. Used for podcasts and some live streams.
    MP4,

    /// WAV container
    ///
    /// Container format for uncompressed PCM audio.
    /// Used for some podcast content.
    WAV,
}

impl Codec {
    /// AAC frames are fixed at 1024 samples.
    /// Used in both ADTS and MP4 containers.
    const AAC_SAMPLES_PER_FRAME: usize = 1_024;

    /// FLAC frames are variable, but may not exceed 4,608 samples up to 48 kHz.
    /// FLAC codec and container are unified.
    const FLAC_MAX_SAMPLES_PER_FRAME: usize = 4_608;

    /// FLAC frames are variable, but may not exceed 16,384 samples above 48 kHz.
    /// Higher limit for high-resolution audio.
    const FLAC_MAX_SAMPLES_PER_FRAME_HI_RES: usize = 16_384;

    /// MP3 frames are fixed at 1,152 samples.
    /// MP3 codec and container are unified.
    const MP3_SAMPLES_PER_FRAME: usize = 1_152;

    /// WAV frames contain uncompressed PCM data, one sample per channel.
    const WAV_SAMPLES_PER_FRAME: usize = 1;

    /// Returns the maximum duration of a frame for the format's codec at the given sample rate.
    ///
    /// Frame sizes are fixed for most codecs:
    /// * AAC (in ADTS/MP4): 1024 samples
    /// * MP3: 1152 samples
    /// * PCM (in WAV): 1 sample per channel
    ///
    /// FLAC uses variable-length frames with maximum sizes:
    /// * Up to 48 kHz: 4608 samples
    /// * Above 48 kHz: 16384 samples
    ///
    /// For example, at 44.1 kHz:
    /// * AAC: ≈ 23.220ms
    /// * MP3: ≈ 26.122ms
    /// * FLAC: up to ≈ 104.490ms
    /// * PCM: ≈ 0.023ms per channel
    ///
    /// Notes:
    /// - For MP4 containers, we assume AAC codec
    /// - For FLAC, we return maximum possible frame duration for safe seeking
    /// - For WAV, assumes stereo PCM data
    #[must_use]
    pub fn frame_duration(&self, sample_rate: u32, channels: u16) -> Duration {
        let samples = match self {
            Codec::ADTS | Codec::MP4 => Self::AAC_SAMPLES_PER_FRAME,
            Codec::FLAC => {
                if sample_rate > 48_000 {
                    Self::FLAC_MAX_SAMPLES_PER_FRAME_HI_RES
                } else {
                    Self::FLAC_MAX_SAMPLES_PER_FRAME
                }
            }
            Codec::MP3 => Self::MP3_SAMPLES_PER_FRAME,
            Codec::WAV => Self::WAV_SAMPLES_PER_FRAME * channels as usize,
        }
        .to_f32_lossy();

        let span = (samples / sample_rate.to_f32_lossy()).clamp(0.0, Duration::MAX.as_secs_f32());
        if span.is_nan() {
            Duration::default()
        } else {
            Duration::from_secs_f32(span)
        }
    }

    /// Audio Data Transport Stream container
    ///
    /// A container format specifically for AAC audio streams.
    /// Used for live streams and some podcasts.
    ///
    /// Typical characteristics:
    /// - Bitrate: 64-256 kbps (CBR/VBR)
    /// - Sample format: 16-bit
    /// - Sample rate: 44.1 kHz
    #[must_use]
    #[inline]
    pub fn extension(&self) -> &'static str {
        match self {
            Codec::ADTS => "aac",
            Codec::FLAC => "flac",
            Codec::MP3 => "mp3",
            Codec::MP4 => "m4a",
            Codec::WAV => "wav",
        }
    }

    /// Returns the MIME type for this format.
    ///
    /// # Examples
    /// ```rust
    /// use pleezer::protocol::Format;
    ///
    /// assert_eq!(Format::ADTS.mime_type(), "audio/aac");
    /// assert_eq!(Format::MP4.mime_type(), "audio/mp4");
    /// ```
    #[must_use]
    #[inline]
    pub fn mime_type(&self) -> &'static str {
        match self {
            Codec::ADTS => "audio/aac",
            Codec::FLAC => "audio/flac",
            Codec::MP3 => "audio/mpeg",
            Codec::MP4 => "audio/mp4",
            Codec::WAV => "audio/wav",
        }
    }
}

/// Formats the audio format for display.
///
/// Shows the primary codec name in lowercase, regardless of container:
/// * ADTS/MP4 -> "aac"
/// * FLAC -> "flac"
/// * MP3 -> "mp3"
/// * WAV -> "wav"
///
/// # Examples
/// ```rust
/// use pleezer::protocol::Format;
///
/// assert_eq!(Format::ADTS.to_string(), "aac");
/// assert_eq!(Format::MP4.to_string(), "aac");  // MP4 assumed to contain AAC
/// ```
impl fmt::Display for Codec {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Codec::ADTS | Codec::MP4 => write!(f, "aac"),
            Codec::FLAC => write!(f, "flac"),
            Codec::MP3 => write!(f, "mp3"),
            Codec::WAV => write!(f, "wav"),
        }
    }
}

/// Parses a string into an audio format.
///
/// Recognizes both container and common file extensions:
///
/// # Container formats
/// - ADTS: "adts", "aac"
/// - FLAC: "flac"
/// - MP3: "mp3"
/// - MP4: "mp4", "m4a", "m4b"
/// - WAV: "wav"
///
/// Note that some strings map to container formats that typically
/// hold specific codecs (e.g., "aac" maps to ADTS container).
///
/// # Examples
/// ```rust
/// use std::str::FromStr;
/// use pleezer::protocol::Format;
///
/// // Container format parsing
/// assert_eq!(Format::from_str("adts")?, Format::ADTS);
/// assert_eq!(Format::from_str("mp4")?, Format::MP4);
///
/// // Common extension parsing
/// assert_eq!(Format::from_str("m4a")?, Format::MP4);
/// assert_eq!(Format::from_str("aac")?, Format::ADTS);
/// ```
///
/// # Errors
/// Returns [`Error::invalid_argument`] if the string doesn't match any supported format.
impl FromStr for Codec {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "aac" | "adts" => Ok(Codec::ADTS),
            "flac" => Ok(Codec::FLAC),
            "mp3" => Ok(Codec::MP3),
            "m4a" | "m4b" | "mp4" => Ok(Codec::MP4),
            "wav" => Ok(Codec::WAV),
            _ => Err(Error::invalid_argument(format!(
                "unable to parse codec from {s}",
            ))),
        }
    }
}
