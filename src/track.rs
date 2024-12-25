//! Track management and playback preparation.
//!
//! This module handles Deezer track operations including:
//! * Track metadata management
//! * Media source retrieval
//! * Download management
//! * Format handling
//! * Encryption detection
//!
//! # Track Lifecycle
//!
//! 1. Creation
//!    * From gateway API response
//!    * Contains metadata and tokens
//!
//! 2. Media Source Resolution
//!    * Retrieves download URLs
//!    * Negotiates quality/format
//!    * Validates availability
//!
//! 3. Download Management
//!    * Background downloading
//!    * Progress tracking
//!    * Buffer management
//!
//! # Quality Fallback
//!
//! When requested quality isn't available, the system attempts fallback in order:
//! * FLAC → MP3 320 → MP3 128 → MP3 64
//! * MP3 320 → MP3 128 → MP3 64
//! * MP3 128 → MP3 64
//!
//! # Integration
//!
//! Works with:
//! * [`player`](crate::player) - For playback management
//! * [`gateway`](crate::gateway) - For track metadata
//! * [`decrypt`](crate::decrypt) - For encrypted content
//!
//! # Example
//!
//! ```rust
//! use pleezer::track::Track;
//!
//! // Create track from gateway data
//! let mut track = Track::from(track_data);
//!
//! // Get media source
//! let medium = track.get_medium(&client, &media_url, quality, license_token).await?;
//!
//! // Start download
//! track.start_download(&client, &medium).await?;
//!
//! // Monitor progress
//! println!("Downloaded: {:?} of {:?}", track.buffered(), track.duration());
//! ```

use std::{
    fmt,
    num::NonZeroI64,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime},
};

use stream_download::{
    self, http::HttpStream, source::SourceStream, storage::temp::TempStorageProvider,
    StreamDownload, StreamHandle, StreamPhase, StreamState,
};
use time::OffsetDateTime;
use url::Url;

use crate::{
    error::{Error, Result},
    http,
    protocol::{
        self,
        connect::AudioQuality,
        gateway,
        media::{self, Cipher, CipherFormat, Data, Format, Medium},
    },
    util::ToF32,
};

/// A unique identifier for a track.
///
/// * Positive IDs: Regular Deezer tracks
/// * Negative IDs: User-uploaded tracks
#[expect(clippy::module_name_repetitions)]
pub type TrackId = NonZeroI64;

/// Represents a Deezer track with metadata and download state.
///
/// Combines track metadata (title, artist, etc) with download management
/// functionality including quality settings, buffering state, and
/// encryption information.
///
/// # Example
///
/// ```rust
/// use pleezer::track::Track;
///
/// let track = Track::from(track_data);
/// println!("Track: {} by {}", track.title(), track.artist());
/// println!("Duration: {:?}", track.duration());
/// ```
#[derive(Debug)]
pub struct Track {
    /// Unique identifier for the track.
    /// Negative values indicate user-uploaded content.
    id: TrackId,

    /// Authentication token specific to this track.
    /// Required for media access requests.
    track_token: String,

    /// Title of the track.
    title: String,

    /// Main artist name.
    artist: String,

    /// Title of the album containing this track.
    album_title: String,

    /// Identifier for the album's cover artwork.
    /// Used to construct cover image URLs.
    album_cover: String,

    /// Replay gain value in decibels.
    /// Used for volume normalization if available.
    gain: Option<f32>,

    /// When this track's access token expires.
    /// After this time, new tokens must be requested.
    expiry: SystemTime,

    /// Current audio quality setting.
    /// May be lower than requested if higher quality unavailable.
    quality: AudioQuality,

    /// Total duration of the track.
    duration: Duration,

    /// Amount of audio data downloaded and available for playback.
    /// Protected by mutex for concurrent access from download task.
    buffered: Arc<Mutex<Duration>>,

    /// Total size of the audio file in bytes.
    /// Available only after download begins.
    file_size: Option<u64>,

    /// Encryption cipher used for this track.
    /// `Cipher::NONE` represents unencrypted content.
    cipher: Cipher,

    /// Handle to active download if any.
    /// None if download hasn't started or was reset.
    handle: Option<StreamHandle>,
}

impl Track {
    /// Amount of audio to buffer before playback can start.
    ///
    /// This helps prevent playback interruptions by ensuring
    /// enough audio data is available.
    const PREFETCH_LENGTH: Duration = Duration::from_secs(3);

    /// Default prefetch size in bytes when Content-Length is unknown.
    ///
    /// Used when server doesn't provide file size. Value matches
    /// official Deezer client behavior.
    const PREFETCH_DEFAULT: usize = 60 * 1024;

    /// Returns the track's unique identifier.
    #[must_use]
    pub fn id(&self) -> TrackId {
        self.id
    }

    /// Returns the track duration.
    ///
    /// The duration represents the total playback time of the track.
    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// Returns the track's replay gain value if available.
    ///
    /// Replay gain is used for volume normalization:
    /// * Positive values indicate track is quieter than reference
    /// * Negative values indicate track is louder than reference
    /// * None indicates no gain information available
    #[must_use]
    pub fn gain(&self) -> Option<f32> {
        self.gain
    }

    /// Returns the track title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the track artist name.
    #[must_use]
    pub fn artist(&self) -> &str {
        &self.artist
    }

    /// Returns the album title for this track.
    #[must_use]
    pub fn album_title(&self) -> &str {
        &self.album_title
    }

    /// The ID of the album cover image.
    ///
    /// This ID can be used to construct a URL for retrieving the album cover image.
    /// Album covers are always square and available in various resolutions up to 1920x1920.
    ///
    /// # URL Format
    /// ```text
    /// https://e-cdns-images.dzcdn.net/images/cover/{album_cover}/{resolution}x{resolution}.{format}
    /// ```
    /// where:
    /// - `{album_cover}` is the ID returned by this method
    /// - `{resolution}` is the desired resolution in pixels (e.g., 500)
    /// - `{format}` is either `jpg` or `png`
    ///
    /// # Recommended Usage
    /// - Default resolution: 500x500
    /// - Default format: `jpg` (smaller file size)
    /// - Alternative: `png` (higher quality but larger file size)
    ///
    /// # Example
    /// ```text
    /// https://e-cdns-images.dzcdn.net/images/cover/f286f9e7dc818e181c37b944e2461101/500x500.jpg
    /// ```
    #[must_use]
    pub fn album_cover(&self) -> &str {
        &self.album_cover
    }

    /// Returns the track's expiration time.
    ///
    /// After this time, the track becomes unavailable for download
    /// and may need token refresh.
    #[must_use]
    pub fn expiry(&self) -> SystemTime {
        self.expiry
    }

    /// Returns the duration of audio data currently buffered.
    ///
    /// This represents how much of the track has been downloaded and
    /// is available for playback.
    ///
    /// # Panics
    ///
    /// Returns last known value if lock is poisoned due to download task panic.
    #[must_use]
    pub fn buffered(&self) -> Duration {
        // Return the buffered duration, or when the lock is poisoned because
        // the download task panicked, return the last value before the panic.
        // Practically, this should mean that this track will never be fully
        // buffered.
        *self.buffered.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Returns the track's audio quality.
    #[must_use]
    pub fn quality(&self) -> AudioQuality {
        self.quality
    }

    /// Returns the encryption cipher used for this track.
    #[must_use]
    pub fn cipher(&self) -> Cipher {
        self.cipher
    }

    /// Returns whether the track is encrypted.
    ///
    /// True if the track uses any cipher other than NONE.
    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.cipher != Cipher::NONE
    }

    /// Returns whether this track uses lossless audio encoding.
    ///
    /// True only for FLAC encoded tracks.
    #[must_use]
    pub fn is_lossless(&self) -> bool {
        self.quality == AudioQuality::Lossless
    }

    /// Cipher format for 64kbps MP3 files using Blowfish CBC stripe encryption.
    const BF_CBC_STRIPE_MP3_64: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::MP3_64,
    };

    /// Cipher format for 128kbps MP3 files using Blowfish CBC stripe encryption.
    const BF_CBC_STRIPE_MP3_128: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::MP3_128,
    };

    /// Cipher format for 320kbps MP3 files using Blowfish CBC stripe encryption.
    const BF_CBC_STRIPE_MP3_320: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::MP3_320,
    };

    /// Cipher format for MP3 files with unknown bitrate using Blowfish CBC stripe encryption.
    const BF_CBC_STRIPE_MP3_MISC: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::MP3_MISC,
    };

    /// Cipher format for FLAC files using Blowfish CBC stripe encryption.
    const BF_CBC_STRIPE_FLAC: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::FLAC,
    };

    /// Available cipher formats for basic quality.
    const CIPHER_FORMATS_MP3_64: [CipherFormat; 2] =
        [Self::BF_CBC_STRIPE_MP3_64, Self::BF_CBC_STRIPE_MP3_MISC];

    /// Available cipher formats for standard quality.
    const CIPHER_FORMATS_MP3_128: [CipherFormat; 3] = [
        Self::BF_CBC_STRIPE_MP3_128,
        Self::BF_CBC_STRIPE_MP3_64,
        Self::BF_CBC_STRIPE_MP3_MISC,
    ];

    /// Available cipher formats for high quality.
    const CIPHER_FORMATS_MP3_320: [CipherFormat; 4] = [
        Self::BF_CBC_STRIPE_MP3_320,
        Self::BF_CBC_STRIPE_MP3_128,
        Self::BF_CBC_STRIPE_MP3_64,
        Self::BF_CBC_STRIPE_MP3_MISC,
    ];

    /// Available cipher formats for lossless quality.
    const CIPHER_FORMATS_FLAC: [CipherFormat; 5] = [
        Self::BF_CBC_STRIPE_FLAC,
        Self::BF_CBC_STRIPE_MP3_320,
        Self::BF_CBC_STRIPE_MP3_128,
        Self::BF_CBC_STRIPE_MP3_64,
        Self::BF_CBC_STRIPE_MP3_MISC,
    ];

    /// API endpoint for retrieving media sources.
    const MEDIA_ENDPOINT: &'static str = "v1/get_url";

    /// Retrieves a media source for the track.
    ///
    /// Attempts to get download URLs for the requested quality level,
    /// falling back to lower qualities if necessary.
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client for API requests
    /// * `media_url` - Base URL for media content
    /// * `quality` - Preferred audio quality
    /// * `license_token` - Token authorizing media access
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Track has expired
    /// * Quality level is unknown
    /// * Media source unavailable
    /// * Network request fails
    ///
    /// # Quality Fallback
    ///
    /// If requested quality unavailable, attempts lower qualities in order:
    /// * FLAC → MP3 320 → MP3 128 → MP3 64
    /// * MP3 320 → MP3 128 → MP3 64
    /// * MP3 128 → MP3 64
    pub async fn get_medium(
        &self,
        client: &http::Client,
        media_url: &Url,
        quality: AudioQuality,
        license_token: impl Into<String>,
    ) -> Result<Medium> {
        if self.expiry <= SystemTime::now() {
            return Err(Error::unavailable(format!(
                "track {self} no longer available since {}",
                OffsetDateTime::from(self.expiry)
            )));
        }

        let cipher_formats = match quality {
            AudioQuality::Basic => Self::CIPHER_FORMATS_MP3_64.to_vec(),
            AudioQuality::Standard => Self::CIPHER_FORMATS_MP3_128.to_vec(),
            AudioQuality::High => Self::CIPHER_FORMATS_MP3_320.to_vec(),
            AudioQuality::Lossless => Self::CIPHER_FORMATS_FLAC.to_vec(),
            AudioQuality::Unknown => {
                return Err(Error::unknown("unknown audio quality for track {self}"));
            }
        };

        let request = media::Request {
            license_token: license_token.into(),
            track_tokens: vec![self.track_token.clone()],
            media: vec![media::Media {
                typ: media::Type::FULL,
                cipher_formats,
            }],
        };

        // Do not use `client.unlimited` but instead apply rate limiting.
        // This is to prevent hammering the Deezer API in case of deserialize errors.
        let get_url = media_url.join(Self::MEDIA_ENDPOINT)?;
        let body = serde_json::to_string(&request)?;
        let request = client.post(get_url, body);

        let response = client.execute(request).await?;
        let body = response.text().await?;
        let result: media::Response = protocol::json(&body, Self::MEDIA_ENDPOINT)?;

        // Deezer only sends a single media object.
        let result = match result.data.first() {
            Some(data) => match data {
                Data::Media { media } => media.first().cloned().ok_or(Error::not_found(
                    format!("empty media data for track {self}"),
                ))?,
                Data::Errors { errors } => {
                    return Err(Error::unavailable(errors.first().map_or_else(
                        || format!("unknown error getting media for track {self}"),
                        ToString::to_string,
                    )));
                }
            },
            None => return Err(Error::not_found(format!("no media data for track {self}"))),
        };

        let available_quality = AudioQuality::from(result.format);

        // User-uploaded tracks are not reported with any quality. We could estimate the quality
        // based on the bitrate, but the official client does not do this either.
        if !self.is_user_uploaded() && quality != available_quality {
            warn!(
                "requested track {self} in {}, but got {}",
                quality, available_quality
            );
        }

        Ok(result)
    }

    /// Returns whether this is a user-uploaded track.
    ///
    /// User-uploaded tracks are identified by negative IDs and may
    /// have different availability and quality characteristics.
    #[must_use]
    pub fn is_user_uploaded(&self) -> bool {
        self.id.is_negative()
    }

    /// Opens a stream for downloading the track content.
    ///
    /// Attempts to open the first available source URL, falling back
    /// to alternatives if needed.
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client for making requests
    /// * `medium` - Media source information
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * No valid sources available
    /// * Track expired or not yet available
    /// * Network error occurs
    async fn open_stream(
        &self,
        client: &http::Client,
        medium: &Medium,
    ) -> Result<HttpStream<reqwest::Client>> {
        let mut result = Err(Error::unavailable(format!(
            "no valid sources found for track {self}"
        )));

        let now = SystemTime::now();

        // Deezer usually returns multiple sources for a track. The official
        // client seems to always use the first one. We start with the first
        // and continue with the next one if the first one fails to start.
        for source in &medium.sources {
            // URLs can theoretically be non-HTTP, and we only support HTTP(S) URLs.
            let Some(host_str) = source.url.host_str() else {
                warn!("skipping source with invalid host for track {self}");
                continue;
            };

            // Check if the track is in a timeframe where it can be downloaded.
            // If not, it can be that the download link expired and needs to be
            // refreshed, that the track is not available yet, or that the track is
            // no longer available.
            if medium.not_before > now {
                warn!(
                    "track {self} is not available for download until {} from {host_str}",
                    OffsetDateTime::from(medium.not_before)
                );
                continue;
            }
            if medium.expiry <= now {
                warn!(
                    "track {self} is no longer available for download since {} from {host_str}",
                    OffsetDateTime::from(medium.expiry)
                );
                continue;
            }

            // Perform the request and stream the response.
            match HttpStream::new(client.unlimited.clone(), source.url.clone()).await {
                Ok(http_stream) => {
                    debug!("starting download of track {self} from {host_str}");
                    result = Ok(http_stream);
                    break;
                }
                Err(err) => {
                    warn!("failed to start download of track {self} from {host_str}: {err}",);
                    continue;
                }
            };
        }

        result
    }

    /// Starts downloading the track.
    ///
    /// Initiates a background download task that:
    /// * Streams content from source
    /// * Tracks download progress
    /// * Updates buffer state
    /// * Enables playback before completion
    ///
    /// # Arguments
    ///
    /// * `client` - HTTP client for download
    /// * `medium` - Media source information
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * No valid source found
    /// * Track unavailable
    /// * Network error occurs
    /// * Download cannot start
    ///
    /// # Progress Tracking
    ///
    /// Download progress is tracked via:
    /// * `buffered()` - Amount downloaded
    /// * `is_complete()` - Download status
    /// * `file_size()` - Total size if known
    ///
    /// # Panics
    ///
    /// * When the buffered duration mutex is poisoned in the progress callback
    /// * When duration calculation overflows during progress calculation
    pub async fn start_download(
        &mut self,
        client: &http::Client,
        medium: &Medium,
    ) -> Result<StreamDownload<TempStorageProvider>> {
        let stream = self.open_stream(client, medium).await?;

        // Set actual audio quality and cipher type.
        self.quality = medium.format.into();
        self.cipher = medium.cipher.typ;

        // Calculate the prefetch size based on the audio quality. This assumes
        // that the track is encoded with a constant bitrate, which is not
        // necessarily true. However, it is a good approximation.
        let mut prefetch_size = None;
        if let Some(file_size) = stream.content_length() {
            info!("downloading {file_size} bytes for track {self}");
            self.file_size = Some(file_size);

            if !self.duration.is_zero() {
                let size = Self::PREFETCH_LENGTH.as_secs()
                    * file_size.saturating_div(self.duration.as_secs());
                trace!("prefetch size for track {self}: {size} bytes");
                prefetch_size = Some(size);
            }
        } else {
            info!("downloading track {self} with unknown file size");
        };
        let prefetch_size = prefetch_size.unwrap_or(Self::PREFETCH_DEFAULT as u64);

        // A progress callback that logs the download progress.
        let track_str = self.to_string();
        let duration = self.duration;
        let buffered = Arc::clone(&self.buffered);
        let callback = move |stream: &HttpStream<_>,
                             stream_state: StreamState,
                             _: &tokio_util::sync::CancellationToken| {
            if stream_state.phase == StreamPhase::Complete {
                info!("completed download of track {track_str}");

                // Prevent rounding errors and set the buffered duration
                // equal to the total duration. It's OK to unwrap here: if
                // the mutex is poisoned, then the main thread panicked and
                // we should propagate the error.
                *buffered.lock().unwrap() = duration;
            } else if let Some(file_size) = stream.content_length() {
                if file_size > 0 {
                    // `f64` not for precision, but to be able to fit
                    // as big as possible file sizes.
                    // TODO : use `Percentage` type
                    #[expect(clippy::cast_precision_loss)]
                    let progress = stream_state.current_position as f64 / file_size as f64;

                    // OK to unwrap: see rationale above.
                    *buffered.lock().unwrap() = duration.mul_f64(progress);
                }
            }
        };

        // Start the download. The `await` here will *not* block until the download is complete,
        // but only until the download is started. The download will continue in the background.
        let download = StreamDownload::from_stream(
            stream,
            TempStorageProvider::default(),
            stream_download::Settings::default()
                .on_progress(callback)
                .prefetch_bytes(prefetch_size)
                .cancel_on_drop(true),
        )
        .await?;

        self.handle = Some(download.handle());
        Ok(download)
    }

    /// Returns a handle to the track's download if active.
    ///
    /// Returns None if download hasn't started.
    #[must_use]
    pub fn handle(&self) -> Option<StreamHandle> {
        self.handle.clone()
    }

    /// Returns whether the track download is complete.
    ///
    /// A track is complete when the buffered duration equals
    /// the total track duration.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.buffered().as_secs() == self.duration.as_secs()
    }

    /// Resets the track's download state.
    ///
    /// Clears:
    /// * Download handle
    /// * File size information
    /// * Buffer progress
    ///
    /// Useful when needing to restart an interrupted download.
    ///
    /// # Panics
    ///
    /// Panics if the buffered lock is poisoned.
    pub fn reset_download(&mut self) {
        self.handle = None;
        self.file_size = None;
        *self.buffered.lock().unwrap() = Duration::ZERO;
    }

    /// Returns the total file size if known.
    ///
    /// Size becomes available after download starts and server
    /// provides Content-Length.
    #[must_use]
    pub fn file_size(&self) -> Option<u64> {
        self.file_size
    }
}

/// Creates a Track from gateway list data.
///
/// Initializes track with:
/// * Basic metadata (ID, title, artist, etc)
/// * Default quality (Standard)
/// * Default cipher (`BF_CBC_STRIPE`)
/// * Empty download state
impl From<gateway::ListData> for Track {
    fn from(item: gateway::ListData) -> Self {
        Self {
            id: item.track_id,
            track_token: item.track_token,
            title: item.title.to_string(),
            artist: item.artist.to_string(),
            album_title: item.album_title.to_string(),
            album_cover: item.album_cover,
            duration: item.duration,
            gain: item.gain.map(ToF32::to_f32_lossy),
            expiry: item.expiry,
            quality: AudioQuality::Standard,
            buffered: Arc::new(Mutex::new(Duration::ZERO)),
            file_size: None,
            cipher: Cipher::BF_CBC_STRIPE,
            handle: None,
        }
    }
}

/// Formats track for display, showing ID, artist and title.
///
/// Format: "{id}: "{artist} - {title}""
///
/// # Example
///
/// ```text
/// 12345: "Artist Name - Track Title"
/// ```
impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: \"{} - {}\"", self.id, self.artist, self.title)
    }
}
