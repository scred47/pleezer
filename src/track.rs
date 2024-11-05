use std::{
    fmt, fs,
    num::NonZeroU64,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime},
};

use stream_download::{
    self, http::HttpStream, source::SourceStream, storage::temp::TempStorageProvider,
    StreamDownload, StreamPhase, StreamState,
};
use tempfile::{NamedTempFile, TempPath};
use time::OffsetDateTime;

use crate::{
    error::{Error, Result},
    http,
    protocol::{
        connect::AudioQuality,
        gateway,
        media::{self, Cipher, CipherFormat, Format, Medium},
    },
};

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum DownloadState {
    #[default]
    Pending,
    Starting,
    Downloading,
    Complete,
}

#[derive(Debug)]
pub struct Track {
    // TODO : replace NonZeroU64 with TrackId everywhere
    id: NonZeroU64,
    track_token: String,
    title: String,
    artist: String,
    gain: f32,
    expiry: SystemTime,
    quality: AudioQuality,
    duration: Duration,
    state: Arc<Mutex<DownloadState>>,
    buffered: Arc<Mutex<Duration>>,
    file: Option<NamedTempFile>,
    data: Option<StreamDownload<TempStorageProvider>>,
    file_size: Option<u64>,
    cipher: Cipher,
}

impl Track {
    /// Amount of seconds to audio to buffer before the track can be read from.
    const PREFETCH_LENGTH: Duration = Duration::from_secs(3);

    /// The default amount of bytes to prefetch before the track can be read
    /// from. This is used when the track does not provide a `Content-Length`
    /// header, and is equal to what the official Deezer client uses.
    const PREFETCH_DEFAULT: usize = 60 * 1024;

    #[must_use]
    pub fn id(&self) -> NonZeroU64 {
        self.id
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    #[must_use]
    pub fn gain(&self) -> f32 {
        self.gain
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn artist(&self) -> &str {
        &self.artist
    }

    #[must_use]
    pub fn expiry(&self) -> SystemTime {
        self.expiry
    }

    /// The duration of the track that has been buffered.
    #[must_use]
    pub fn buffered(&self) -> Duration {
        // Return the buffered duration, or when the lock is poisoned because
        // the download task panicked, return the last value before the panic.
        // Practically, this should mean that this track will never be fully
        // buffered.
        *self.buffered.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use]
    pub fn quality(&self) -> AudioQuality {
        self.quality
    }

    #[must_use]
    pub fn cipher(&self) -> Cipher {
        self.cipher
    }

    #[must_use]
    pub fn is_encrypted(&self) -> bool {
        self.cipher != Cipher::NONE
    }

    /// Returns the current download state of the track.
    ///
    /// # Panics
    ///
    /// Panics if the lock is poisoned.
    #[must_use]
    pub fn state(&self) -> DownloadState {
        *self.state.lock().unwrap()
    }

    #[must_use]
    pub fn is_pending(&self) -> bool {
        self.state() == DownloadState::Pending
    }

    #[must_use]
    pub fn is_starting(&self) -> bool {
        self.state() == DownloadState::Starting
    }

    #[must_use]
    pub fn is_downloading(&self) -> bool {
        self.state() == DownloadState::Downloading
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.state() == DownloadState::Complete
    }

    const BF_CBC_STRIPE_MP3_64: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::MP3_64,
    };

    const BF_CBC_STRIPE_MP3_128: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::MP3_128,
    };

    const BF_CBC_STRIPE_MP3_320: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::MP3_320,
    };

    const BF_CBC_STRIPE_MP3_MISC: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::MP3_MISC,
    };

    const BF_CBC_STRIPE_FLAC: CipherFormat = CipherFormat {
        cipher: Cipher::BF_CBC_STRIPE,
        format: Format::FLAC,
    };

    const CIPHER_FORMATS_MP3_64: [CipherFormat; 2] =
        [Self::BF_CBC_STRIPE_MP3_64, Self::BF_CBC_STRIPE_MP3_MISC];

    const CIPHER_FORMATS_MP3_128: [CipherFormat; 3] = [
        Self::BF_CBC_STRIPE_MP3_128,
        Self::BF_CBC_STRIPE_MP3_64,
        Self::BF_CBC_STRIPE_MP3_MISC,
    ];

    const CIPHER_FORMATS_MP3_320: [CipherFormat; 4] = [
        Self::BF_CBC_STRIPE_MP3_320,
        Self::BF_CBC_STRIPE_MP3_128,
        Self::BF_CBC_STRIPE_MP3_64,
        Self::BF_CBC_STRIPE_MP3_MISC,
    ];

    const CIPHER_FORMATS_FLAC: [CipherFormat; 5] = [
        Self::BF_CBC_STRIPE_FLAC,
        Self::BF_CBC_STRIPE_MP3_320,
        Self::BF_CBC_STRIPE_MP3_128,
        Self::BF_CBC_STRIPE_MP3_64,
        Self::BF_CBC_STRIPE_MP3_MISC,
    ];

    const MEDIA_GET_URL: &'static str = "https://media.deezer.com/v1/get_url";

    /// Get a HTTP media source for the track.
    ///
    /// The `license_token` is a token that is required to access this track
    /// with the requested quality.
    ///
    /// # Errors
    ///
    /// Returns an error if the requested audio quality is unknown, or if the
    /// media source could not be retrieved.
    pub async fn get_medium(
        &self,
        client: &http::Client,
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

        let get_url = Self::MEDIA_GET_URL.parse::<reqwest::Url>()?;
        let response = client.unlimited.post(get_url).json(&request).send().await?;
        let result = response.json::<media::Response>().await?;

        // The official client also seems to always use the first media object.
        let result = result
            .data
            .first()
            .and_then(|data| data.media.first())
            .cloned()
            .ok_or_else(|| Error::not_found(format!("no media found for track {self}")))?;

        trace!("{} (redacted): {{ ... }}", Self::MEDIA_GET_URL);

        let available_quality = AudioQuality::from(result.format);

        if quality != available_quality {
            warn!(
                "requested track {self} in {} audio quality, but got {}",
                quality, available_quality
            );
        }

        Ok(result)
    }

    /// Start downloading the track with the given `client` and from the given
    /// `medium`. The download will be started in the background.
    ///
    /// # Errors
    ///
    /// Returns an error if the no sources are found for the track, if the URL
    /// has no host name, if the track is not available for download, or if the
    /// download link expired.
    ///
    /// # Panics
    ///
    /// Panics if a lock is poisoned, which would be from the main thread
    /// panicking.
    pub async fn start_download(&mut self, client: &http::Client, medium: Medium) -> Result<()> {
        // Don't hold the lock because some await points are coming up.
        // Instead, set the state to `Starting` to prevent multiple downloads
        // from starting at the same time before the state is set to `Downloading`.
        *self.state.lock().unwrap() = DownloadState::Starting;

        // Deezer usually returns multiple sources for a track. The official
        // client seems to always use the first one. We start with the first
        // and continue with the next one if the first one fails to start.
        let mut stream = None;
        let now = SystemTime::now();

        #[expect(clippy::iter_next_slice)]
        while let Some(source) = medium.sources.iter().next() {
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
                    stream = Some(http_stream);
                    break;
                }
                Err(err) => {
                    warn!("failed to start download of track {self} from {host_str}: {err}",);
                    continue;
                }
            };
        }

        let stream = stream.ok_or_else(|| {
            Error::unavailable(format!("no valid sources found for track {self}"))
        })?;

        // Set actual audio quality and cipher type.
        self.quality = medium.format.into();
        self.cipher = medium.cipher.typ;

        // Calculate the prefetch size based on the audio quality. This assumes
        // that the track is encoded with a constant bitrate, which is not
        // necessarily true. However, it is a good approximation.
        let mut prefetch_size = None;
        if let Some(file_size) = stream.content_length() {
            debug!("downloading {file_size} bytes for track {self}");
            self.file_size = Some(file_size);

            if !self.duration.is_zero() {
                let size = Self::PREFETCH_LENGTH.as_secs()
                    * file_size.saturating_div(self.duration.as_secs());
                trace!("prefetch size for track {self}: {size} bytes");
                prefetch_size = Some(size);
            }
        } else {
            debug!("downloading track {self} with unknown file size");
        };
        let prefetch_size = prefetch_size.unwrap_or(Self::PREFETCH_DEFAULT as u64);

        // A progress callback that logs the download progress.
        let track_str = self.to_string();
        let duration = self.duration;
        let buffered = Arc::clone(&self.buffered);
        let track_state = Arc::clone(&self.state);
        let callback = move |stream: &HttpStream<_>,
                             stream_state: StreamState,
                             _: &tokio_util::sync::CancellationToken| {
            match stream_state.phase {
                StreamPhase::Complete => {
                    debug!("download of track {track_str} completed");

                    // Prevent rounding errors and set the buffered duration
                    // equal to the total duration. It's OK to unwrap here: if
                    // the mutex is poisoned, then the main thread panicked and
                    // we should propagate the error.
                    *buffered.lock().unwrap() = duration;
                    *track_state.lock().unwrap() = DownloadState::Complete;
                }
                _ => {
                    if let Some(file_size) = stream.content_length() {
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
                }
            }
        };

        // Create a temporary file to store the downloaded data and reopen it
        // to pass it to the download object.
        let temp_file = NamedTempFile::new()?;
        let file = temp_file.reopen()?;
        let path = temp_file.path().to_owned();

        // Start the download and store the download object. The `await` here
        // will *not* block until the download is complete, but only until the
        // download is started. The download will continue in the background.
        let download = StreamDownload::from_stream(
            stream,
            TempStorageProvider::with_tempfile_builder(move || {
                Ok(NamedTempFile::from_parts(
                    // temp_file.reopen()?,
                    file.try_clone()?,
                    TempPath::from_path(&path),
                ))
            }),
            stream_download::Settings::default()
                .on_progress(callback)
                .prefetch_bytes(prefetch_size),
        )
        .await?;

        // Store the temporary file. This is necessary to keep the file open
        // (the file is deleted when the last handle is closed) and to be able
        // to reopen it for reading later.
        self.file = Some(temp_file);

        *self.state.lock().unwrap() = DownloadState::Downloading;
        self.data = Some(download);

        Ok(())
    }

    /// Returns the file size of the track, if known after the download has
    /// started.
    #[must_use]
    pub fn file_size(&self) -> Option<u64> {
        self.file_size
    }

    /// Reopen an independent file handle to the downloaded track.
    ///
    /// # Errors
    ///
    /// Returns an error if the download has not been started yet.
    pub fn try_file(&self) -> Result<fs::File> {
        self.file
            .as_ref()
            .ok_or_else(|| Error::unavailable("track {self} download not started"))
            .and_then(|file| file.reopen().map_err(Into::into))
    }
}

impl From<gateway::ListData> for Track {
    fn from(item: gateway::ListData) -> Self {
        Self {
            id: item.track_id,
            track_token: item.track_token,
            title: item.title,
            artist: item.artist,
            duration: item.duration,
            gain: item.gain,
            expiry: item.expiry,
            quality: AudioQuality::Standard,
            buffered: Arc::new(Mutex::new(Duration::ZERO)),
            state: Arc::new(Mutex::new(DownloadState::Pending)),
            file: None,
            data: None,
            file_size: None,
            cipher: Cipher::BF_CBC_STRIPE,
        }
    }
}

impl fmt::Display for Track {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}: \"{} - {}\"", self.id, self.artist, self.title)
    }
}
