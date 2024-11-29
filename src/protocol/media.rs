//! Media streaming types and formats for Deezer.
//!
//! This module handles media access requests and responses, including:
//! * Track streaming URLs
//! * Audio formats and quality levels
//! * Content encryption
//! * Access tokens and expiry
//!
//! # Wire Format
//!
//! Request:
//! ```json
//! {
//!     "license_token": "secret",
//!     "media": [{
//!         "type": "FULL",
//!         "formats": [{
//!             "cipher": "BF_CBC_STRIPE",
//!             "format": "MP3_320"
//!         }]
//!     }],
//!     "track_tokens": ["token1", "token2"]
//! }
//! ```
//!
//! Response:
//! ```json
//! {
//!     "data": [{
//!         "media": [{
//!             "media_type": "FULL",
//!             "cipher": {"type": "BF_CBC_STRIPE"},
//!             "format": "MP3_320",
//!             "sources": [{
//!                 "url": "https://...",
//!                 "provider": "cdn"
//!             }],
//!             "nbf": 1234567890,
//!             "exp": 1234599999
//!         }]
//!     }]
//! }
//! ```

use std::{fmt, time::SystemTime};

use serde::{Deserialize, Serialize};
use serde_with::{formats::Flexible, serde_as, TimestampSeconds};
use url::Url;
use veil::Redact;

use super::connect::AudioQuality;

/// Media access request.
///
/// Used to request streaming URLs for tracks with specific
/// format and encryption requirements.
#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug, Hash)]
pub struct Request {
    /// License authentication token
    pub license_token: String,
    /// Requested media formats
    pub media: Vec<Media>,
    /// Track-specific access tokens
    pub track_tokens: Vec<String>,
}

/// Media format request.
///
/// Specifies the type of media (full/preview) and desired
/// format/encryption combinations.
#[serde_as]
#[derive(Clone, Default, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug, Hash)]
pub struct Media {
    /// Full track or preview clip
    #[serde(default)]
    #[serde(rename = "type")]
    pub typ: Type,

    /// Requested format and encryption combinations
    #[serde(rename = "formats")]
    pub cipher_formats: Vec<CipherFormat>,
}

/// Media content type.
///
/// Determines whether to return the full track or just
/// a preview clip.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash,
)]
pub enum Type {
    /// Full-length track
    #[default]
    FULL,
    /// Preview clip (typically 30 seconds)
    PREVIEW,
}

impl fmt::Display for Type {
    /// Formats the media type for display.
    ///
    /// Shows either "FULL" or "PREVIEW" matching the protocol's
    /// string representation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::protocol::media::Type;
    ///
    /// assert_eq!(Type::FULL.to_string(), "FULL");
    /// assert_eq!(Type::PREVIEW.to_string(), "PREVIEW");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Format and encryption combination.
///
/// Specifies both the audio format (quality level) and
/// encryption method for the content.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash,
)]
pub struct CipherFormat {
    /// Encryption method
    pub cipher: Cipher,
    /// Audio format
    pub format: Format,
}

/// Content encryption method.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash,
)]
#[expect(non_camel_case_types)]
pub enum Cipher {
    /// Blowfish CBC with striping
    #[default]
    BF_CBC_STRIPE,
    /// No encryption
    NONE,
}

impl fmt::Display for Cipher {
    /// Formats the cipher type for display.
    ///
    /// Shows either "`BF_CBC_STRIPE`" or "NONE" matching the protocol's
    /// string representation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::protocol::media::Cipher;
    ///
    /// assert_eq!(Cipher::BF_CBC_STRIPE.to_string(), "BF_CBC_STRIPE");
    /// assert_eq!(Cipher::NONE.to_string(), "NONE");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Audio format and quality level.
///
/// Represents different audio formats and their quality levels,
/// mapped to specific numeric IDs in the protocol.
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash,
)]
#[expect(non_camel_case_types)]
#[repr(i64)]
pub enum Format {
    /// External source (-1)
    EXTERNAL = -1,
    /// FLAC lossless (9)
    FLAC = 9,
    /// 64 kbps MP3 (10)
    MP3_64 = 10,
    /// 128 kbps MP3 (1, default)
    #[default]
    MP3_128 = 1,
    /// 320 kbps MP3 (3)
    MP3_320 = 3,
    /// Other or unknown MP3 bitrate (0)
    MP3_MISC = 0,
}

impl fmt::Display for Format {
    /// Formats the audio format for display.
    ///
    /// Shows the format name (e.g., "`MP3_320`", "FLAC") matching
    /// the protocol's string representation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::protocol::media::Format;
    ///
    /// assert_eq!(Format::MP3_320.to_string(), "MP3_320");
    /// assert_eq!(Format::FLAC.to_string(), "FLAC");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl From<Format> for AudioQuality {
    /// Converts a media format to its corresponding audio quality level.
    ///
    /// Maps format types to quality levels:
    /// * `MP3_64` -> Basic (64 kbps)
    /// * `MP3_128` -> Standard (128 kbps)
    /// * `MP3_320` -> High (320 kbps)
    /// * FLAC -> Lossless
    /// * Others -> Unknown
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::protocol::{media::Format, connect::AudioQuality};
    ///
    /// assert_eq!(AudioQuality::from(Format::MP3_320), AudioQuality::High);
    /// assert_eq!(AudioQuality::from(Format::FLAC), AudioQuality::Lossless);
    /// assert_eq!(AudioQuality::from(Format::EXTERNAL), AudioQuality::Unknown);
    /// ```
    fn from(format: Format) -> Self {
        match format {
            Format::MP3_64 => AudioQuality::Basic,
            Format::MP3_128 => AudioQuality::Standard,
            Format::MP3_320 => AudioQuality::High,
            Format::FLAC => AudioQuality::Lossless,
            _ => AudioQuality::Unknown,
        }
    }
}

/// Media access response.
///
/// Contains either media URLs or error information.
#[derive(Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash)]
pub struct Response {
    pub data: Vec<Data>,
}

/// Response data variant.
///
/// Can contain either media information or error details.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash)]
#[serde(untagged)]
pub enum Data {
    Media { media: Vec<Medium> },
    Errors { errors: Vec<Error> },
}

/// Media access error.
///
/// Represents an error response from the media server, containing
/// both an error code and descriptive message.
///
/// # Wire Format
///
/// ```json
/// {
///     "errors": [{
///         "code": 404,
///         "message": "Track not found"
///     }]
/// }
/// ```
///
/// # Examples
///
/// ```rust
/// use deezer::protocol::media::Error;
///
/// let error = Error {
///     code: 404,
///     message: "Track not found".to_string(),
/// };
///
/// // Displays as: "Track not found (404)"
/// println!("{}", error);
/// ```
#[derive(Clone, Eq, Default, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash)]
pub struct Error {
    /// Numeric error code
    code: i64,
    /// Human-readable error description
    message: String,
}

impl fmt::Display for Error {
    /// Formats an error for display.
    ///
    /// Shows both the error message and code in the format:
    /// `"{message} ({code})"`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::protocol::media::Error;
    ///
    /// let error = Error {
    ///     code: 404,
    ///     message: "Not found".to_string(),
    /// };
    /// assert_eq!(error.to_string(), "Not found (404)");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}

/// Media access details.
///
/// Contains all information needed to access a media file,
/// including URLs, format, and validity period.
#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash)]
pub struct Medium {
    /// Full track or preview
    #[serde(default)]
    pub media_type: Type,

    /// Encryption method
    #[serde(default)]
    pub cipher: CipherType,

    /// Audio format
    #[serde(default)]
    pub format: Format,

    /// Available download sources
    pub sources: Vec<Source>,

    /// Start of validity period
    #[serde(rename = "nbf")]
    #[serde_as(as = "TimestampSeconds<i64, Flexible>")]
    pub not_before: SystemTime,

    /// End of validity period
    #[serde(rename = "exp")]
    #[serde_as(as = "TimestampSeconds<i64, Flexible>")]
    pub expiry: SystemTime,
}

/// Encryption method wrapper for media content.
///
/// Used in responses to specify how the media content is encrypted.
/// The wrapper structure matches the protocol's JSON format.
///
/// # Wire Format
///
/// ```json
/// {
///     "type": "BF_CBC_STRIPE"  // or "NONE"
/// }
/// ```
///
/// # Examples
///
/// ```rust
/// use deezer::protocol::media::{Cipher, CipherType};
///
/// let cipher = CipherType {
///     typ: Cipher::BF_CBC_STRIPE,
/// };
///
/// // Default is BF_CBC_STRIPE
/// assert_eq!(CipherType::default().typ, Cipher::BF_CBC_STRIPE);
/// ```
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash,
)]
pub struct CipherType {
    /// The encryption method to use
    #[serde(rename = "type")]
    pub typ: Cipher,
}

/// Media source information.
///
/// Contains the URL and provider for downloading media content.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Redact, Hash)]
pub struct Source {
    /// Download URL (redacted in debug output)
    #[redact]
    pub url: Url,

    /// Content provider name (e.g., "cdn")
    #[serde(default)]
    pub provider: String,
}
