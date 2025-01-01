//! Live radio stream handling for Deezer's gateway API.
//!
//! Provides livestream-specific wrappers and types for:
//! * Stream URLs in multiple formats (AAC/MP3)
//! * Multiple bitrate options
//! * Station metadata
//! * Availability status
//!
//! Livestreams have unique characteristics:
//! * No track duration/progress
//! * Multiple parallel streams
//! * Codec selection
//! * Always external URLs
//!
//! # Wire Format
//!
//! Response format:
//! ```json
//! {
//!     "EPISODE_ID": "123456",
//!     "AVAILABLE": true,
//!     "DURATION": "1800",
//!     "EPISODE_TITLE": "Episode Title",
//!     "SHOW_NAME": "Podcast Name",
//!     "SHOW_ART_MD5": "cover_id",
//!     "TRACK_TOKEN": "secret_token",
//!     "TRACK_TOKEN_EXPIRE": "1234567890",
//!     "EPISODE_DIRECT_STREAM_URL": "https://..."
//! }
//! ```

use std::ops::Deref;

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use super::{ListData, Method};
use crate::{protocol::Codec, track::TrackId};

/// Gateway method name for retrieving radio streams.
///
/// Returns stream information including:
/// * Station metadata
/// * Multiple quality streams
/// * Codec options
/// * Availability status
impl Method for LivestreamData {
    const METHOD: &'static str = "livestream.getData";
}

/// Wrapper for livestream data.
///
/// Contains the same track information as [`ListData`] but specifically
/// for podcast episodes. The wrapper allows specialized handling while
/// reusing the underlying data structure.
#[derive(Clone, PartialEq, Deserialize, Debug)]
#[serde(transparent)]
#[expect(clippy::module_name_repetitions)]
pub struct LivestreamData(pub ListData);

/// Provides access to the underlying livestream data.
///
/// Allows transparent access to the livestream fields while maintaining
/// type safety for livestream-specific operations.
impl Deref for LivestreamData {
    type Target = ListData;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Request parameters for track list data.
///
/// Used to request information for multiple tracks in a single query.
/// Supports different content types through enum variants.
///
/// # Example
///
/// ```rust
/// use deezer::gateway::{Request, TrackId};
///
/// let request = Request::Songs {
///     song_ids: vec![123456.into(), 789012.into()],
/// };
/// ```
#[serde_as]
#[derive(Clone, Eq, PartialEq, Serialize, Debug, Hash)]
pub struct Request {
    /// Live stream ID to fetch information for.
    pub livestream_id: TrackId,

    /// List of audio codecs supported by the client
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub supported_codecs: Vec<Codec>,
}
