//! Track list data retrieval from Deezer's gateway API.
//!
//! This module handles fetching detailed track information, including:
//! * Track metadata (title, artist, album)
//! * Playback information (duration, gain)
//! * Authentication (track tokens)
//! * Media assets (album covers)
//!
//! # Wire Format
//!
//! Response format:
//! ```json
//! {
//!     "SNG_ID": "123456",
//!     "ART_NAME": "Artist Name",
//!     "ALB_TITLE": "Album Title",
//!     "ALB_PICTURE": "album_cover_id",
//!     "DURATION": "180",
//!     "SNG_TITLE": "Track Title",
//!     "GAIN": "-1.3",
//!     "TRACK_TOKEN": "secret_token",
//!     "TRACK_TOKEN_EXPIRE": "1234567890"
//! }
//! ```
//!
//! Request format:
//! ```json
//! {
//!     "sng_ids": ["123456", "789012"]
//! }
//! ```

use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use serde_with::{
    formats::Flexible, serde_as, DisplayFromStr, DurationSeconds, PickFirst, TimestampSeconds,
};
use veil::Redact;

use crate::track::TrackId;

use super::{Method, StringOrUnknown};

/// Gateway method name for retrieving track information.
///
/// This endpoint returns detailed track data including:
/// * Metadata (titles, artists, albums)
/// * Playback information (duration, gain)
/// * Authentication tokens
/// * Media asset identifiers
impl Method for ListData {
    const METHOD: &'static str = "song.getListData";
}

/// Collection of track list data responses.
pub type Queue = Vec<ListData>;

/// Detailed track information from Deezer's gateway.
///
/// Contains all the metadata and authentication information needed
/// to play a track, including titles, tokens, and media assets.
///
/// # Fields
///
/// * `track_id` - Unique track identifier
/// * `artist` - Artist name (defaults to "UNKNOWN")
/// * `album_title` - Album name (defaults to "UNKNOWN")
/// * `album_cover` - Album artwork identifier
/// * `duration` - Track length
/// * `title` - Track name (defaults to "UNKNOWN")
/// * `gain` - Volume normalization value
/// * `track_token` - Authentication token for playback
/// * `expiry` - Token expiration timestamp
///
/// # Example
///
/// ```rust
/// use deezer::gateway::{ListData, Response};
///
/// let response: Response<ListData> = /* gateway response */;
/// if let Some(track) = response.first() {
///     println!("{} by {}", track.title, track.artist);
///     println!("Token expires: {:?}", track.expiry);
/// }
/// ```
#[serde_as]
#[derive(Clone, PartialEq, PartialOrd, Deserialize, Redact)]
#[serde(rename_all = "UPPERCASE")]
pub struct ListData {
    /// Unique track identifier.
    ///
    /// This ID is consistent across all Deezer services and can be:
    /// * Positive - Regular Deezer tracks
    /// * Negative - User-uploaded tracks
    #[serde(rename = "SNG_ID")]
    #[serde_as(as = "PickFirst<(_, serde_with::DisplayFromStr)>")]
    pub track_id: TrackId,

    /// Artist name.
    ///
    /// Defaults to "UNKNOWN" if not provided or invalid.
    /// For tracks with multiple artists, this contains only the main artist.
    #[serde(default)]
    #[serde(rename = "ART_NAME")]
    pub artist: StringOrUnknown,

    /// Album title.
    ///
    /// Defaults to "UNKNOWN" if not provided or invalid.
    /// For singles or EPs, this might be the same as the track title.
    #[serde(default)]
    #[serde(rename = "ALB_TITLE")]
    pub album_title: StringOrUnknown,

    /// Album cover identifier.
    ///
    /// When available, this ID can be used to construct image URLs:
    /// ```text
    /// https://e-cdns-images.dzcdn.net/images/cover/{album_cover}/{resolution}x{resolution}.{format}
    /// ```
    /// Where:
    /// * `resolution` is the desired size in pixels (up to 1920)
    /// * `format` is either:
    ///   - `jpg` for smaller file size
    ///   - `png` for higher quality
    ///
    /// Deezer's default format is 500x500.jpg
    ///
    /// Defaults to an empty string when no cover is available.
    #[serde(default)]
    #[serde(rename = "ALB_PICTURE")]
    pub album_cover: String,

    /// Track duration.
    ///
    /// The actual playback length of the track, parsed from seconds.
    /// Used for progress calculation and UI display.
    #[serde_as(as = "DurationSeconds<String, Flexible>")]
    pub duration: Duration,

    /// Track title.
    ///
    /// Defaults to "UNKNOWN" if not provided or invalid.
    /// This is the main display title of the track.
    #[serde(default)]
    #[serde(rename = "SNG_TITLE")]
    pub title: StringOrUnknown,

    /// Track's average loudness in decibels (dB).
    ///
    /// Used to calculate volume normalization. May be absent if
    /// loudness data isn't available.
    ///
    /// Negative values indicate quieter tracks (typical range: -20 to 0 dB).
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub gain: Option<f64>,

    /// Authentication token for track playback.
    ///
    /// This token is required to access the track's media content and:
    /// * Is unique per track
    /// * Has a limited validity period
    /// * Should be kept secure
    #[redact]
    pub track_token: String,

    /// Token expiration timestamp.
    ///
    /// The time at which the `track_token` becomes invalid.
    /// New tokens should be requested after expiration.
    #[serde(rename = "TRACK_TOKEN_EXPIRE")]
    #[serde_as(as = "TimestampSeconds<i64, Flexible>")]
    pub expiry: SystemTime,
}

/// Request parameters for track list data.
///
/// Used to request information for multiple tracks in a single query.
///
/// # Example
///
/// ```rust
/// use deezer::gateway::{Request, TrackId};
///
/// let request = Request {
///     track_ids: vec![123456.into(), 789012.into()],
/// };
/// ```
#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug, Hash)]
pub struct Request {
    /// List of track IDs to fetch information for.
    ///
    /// Each ID must be:
    /// * Non-zero
    /// * Either positive (Deezer tracks) or negative (user uploads)
    /// * Valid within Deezer's catalog
    #[serde(rename = "sng_ids")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub track_ids: Vec<TrackId>,
}
