use std::{
    fmt,
    io::{Read, Seek, SeekFrom, Write},
    num::NonZeroU64,
    sync::{Arc, Mutex, PoisonError},
    time::{Duration, SystemTime},
};

use futures_util::StreamExt;
use tempfile::tempfile;
use tokio::sync::oneshot;

use crate::{
    error::{Error, Result},
    http,
    protocol::connect::AudioQuality,
    protocol::media::{self, Cipher, CipherFormat, Format},
};

pub struct Track {
    // TODO : replace NonZeroU64 with TrackId everywhere
    track_id: NonZeroU64,
    track_token: String,
    quality: AudioQuality,
    duration: Duration,
    buffered: Duration, //Arc<Mutex<Duration>>,
    data: Option<std::fs::File>,
    cipher: Cipher,
}

impl Track {
    /// Creates a new `Track` with the given `track_id`, `duration`, and
    /// `track_token`.
    ///
    /// # Errors
    ///
    /// Returns an error if creating the temporary file to store the track data
    /// fails.
    pub fn new(
        track_id: NonZeroU64,
        duration: Duration,
        track_token: impl Into<String>,
    ) -> Result<Self> {
        Ok(Self {
            track_id,
            track_token: track_token.into(),
            quality: AudioQuality::default(),
            duration,
            buffered: Duration::ZERO, //Arc::new(Mutex::new(Duration::ZERO)),
            data: None,
            cipher: Cipher::default(),
        })
    }

    #[must_use]
    pub fn id(&self) -> NonZeroU64 {
        self.track_id
    }

    #[must_use]
    pub fn duration(&self) -> Duration {
        self.duration
    }

    /// The duration of the track that has been buffered.
    #[must_use]
    pub fn buffered(&self) -> Duration {
        if self.data.is_none() {
            return Duration::ZERO;
        }

        // Return the buffered duration, or when the lock is poisoned because
        // the download task panicked, return the last value before the panic.
        // Practically, this should mean that this track will never be fully
        // buffered.
        self.buffered //*self.buffered.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.data.is_some() && self.buffered() >= self.duration
    }

    #[must_use]
    pub fn quality(&self) -> AudioQuality {
        self.quality
    }

    #[must_use]
    pub fn cipher(&self) -> Cipher {
        self.cipher
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
        client: http::Client,
        quality: AudioQuality,
        license_token: impl Into<String>,
    ) -> Result<media::Medium> {
        let cipher_formats = match quality {
            AudioQuality::Basic => Self::CIPHER_FORMATS_MP3_64.to_vec(),
            AudioQuality::Standard => Self::CIPHER_FORMATS_MP3_128.to_vec(),
            AudioQuality::High => Self::CIPHER_FORMATS_MP3_320.to_vec(),
            AudioQuality::Lossless => Self::CIPHER_FORMATS_FLAC.to_vec(),
            AudioQuality::Unknown => {
                return Err(Error::unknown("unknown audio quality"));
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
        result
            .media
            .first()
            .cloned()
            .ok_or(Error::not_found(format!(
                "no media found for track {}",
                self.track_id
            )))
    }

    /// Downloads the track.
    ///
    /// This method will download the track from the given HTTP media source.
    /// The download will be aborted if the given `abort_rx` channel receives a
    /// `true` value. This is useful for cancelling the download when the track
    /// is no longer needed.
    ///
    /// # Errors
    ///
    /// This method may return an error if the track is not available for
    /// download or an I/O error occurs. Aborting the download will *not* result
    /// in an error.
    ///
    /// # Panics
    ///
    /// This method will panic if the mutex guarding the buffered data is
    /// poisoned, i.e. another thread panicked while holding the lock.
    pub async fn download(
        &mut self,
        client: http::Client,
        medium: media::Medium,
        mut abort_rx: oneshot::Receiver<()>,
    ) -> Result<()> {
        let now = SystemTime::now();
        if medium.not_before > now {
            return Err(Error::unavailable(format!(
                "track {} is not available for download until {:?}",
                self.track_id, medium.not_before
            )));
        }
        if medium.expiry <= now {
            return Err(Error::unavailable(format!(
                "track {} is no longer available for download since {:?}",
                self.track_id, medium.expiry
            )));
        }

        let source = medium.sources.first().ok_or(Error::unavailable(format!(
            "no sources found for track {}",
            self.track_id
        )))?;

        let host_str = source
            .url
            .host_str()
            .ok_or(Error::invalid_argument("url has no host name"))?;
        debug!(
            "starting download of track {} from {host_str}",
            self.track_id
        );

        // Create a new temporary file to store the track data.
        let mut tempfile = tempfile()?;

        // Set actual audio quality and cipher type.
        self.quality = medium.format.into();
        self.cipher = medium.cipher_type.typ;

        // Perform the request and stream the response into the data buffer.
        let response = client.unlimited.get(source.url.as_ref()).send().await?;

        // If the content length is not provided, default to 0. This should
        // never happen, but when it does, the download will just continue
        // until the stream is exhausted without calculating the buffered
        // progress in the meantime.
        let file_size = response.content_length().unwrap_or(0);
        debug!("downloading {file_size} bytes for track {}", self.track_id);

        let mut stream = response.bytes_stream();
        let mut bytes_downloaded = 0;

        loop {
            tokio::select! {
                biased;

                _ = &mut abort_rx => {
                    debug!("aborting download of track {}", self.track_id);
                    break Ok(());
                },

                chunk = stream.next() => {
                    match chunk
                    {
                        None => {
                            debug!("download of track {} complete", self.track_id);

                            // Prevent rounding errors and set the buffered
                            // duration to the exact duration of the track.
                            // OK to unwrap: if the lock is poisoned, then
                            // propagating the error is the correct behavior.
                            self.buffered = self.duration;//*self.buffered.lock().unwrap() = self.duration;

                            tempfile.seek(SeekFrom::Start(0))?;
                            self.data = Some(tempfile);

                            break Ok(());
                        }

                        Some(Err(e)) => break Err(Error::from(e)),

                        Some(Ok(chunk)) => {
                            tempfile.write_all(&chunk)?;
                            bytes_downloaded += chunk.len() as u64;

                            if file_size > 0 {
                                // `file_size` not fitting into `f64` would be
                                // so rare that it's not worth handling.
                                #[expect(clippy::cast_precision_loss)]
                                let length = (bytes_downloaded as f64) / (file_size as f64);

                                self.buffered = self.duration.mul_f64(length).clamp(Duration::ZERO, self.duration);
                            }
                        },
                    }
                },
            }
        }
    }
}

impl Read for Track {
    /// Read from the data once fully buffered, otherwise return an error.
    ///
    /// # Errors
    ///
    /// If the track is not fully buffered, this method will return an error
    /// with the kind `WouldBlock`.
    /// If the track is not downloaded, this method will return an error with
    /// the kind `NotFound`.
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.is_complete() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "track not fully buffered",
            ));
        }

        self.data
            .as_mut()
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "track not downloaded",
            ))
            .and_then(|data| data.read(buf))
    }
}

impl Seek for Track {
    /// Seek in the data once fully buffered, otherwise return an error.
    ///
    /// # Errors
    ///
    /// If the track is not fully buffered, this method will return an error
    /// with the kind `WouldBlock`.
    /// If the track is not downloaded, this method will return an error with
    /// the kind `NotFound`.
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        if !self.is_complete() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::WouldBlock,
                "track not fully buffered",
            ));
        }

        self.data
            .as_mut()
            .ok_or(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "track not downloaded",
            ))
            .and_then(|data| data.seek(pos))
    }
}

impl fmt::Display for Track {
    // TODO : pretty print with artist and title if available
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.track_id)
    }
}
