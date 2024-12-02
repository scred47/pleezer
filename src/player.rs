//! Audio playback and track management.
//!
//! This module handles:
//! * Audio device configuration
//! * Track playback and decryption
//! * Queue management
//! * Volume normalization
//! * Event notifications
//!
//! # Device Management
//!
//! The audio device is handled in three phases:
//! 1. Selection during construction (`new()`)
//! 2. Opening on demand (`start()`)
//! 3. Closing when done (`stop()`)
//!
//! This design prevents ALSA from acquiring the device until it's actually needed.
//!
//! //! # Audio Pipeline
//!
//! The playback pipeline consists of:
//! 1. Track download and decryption
//! 2. Audio format decoding (MP3/FLAC)
//! 3. Volume normalization (optional)
//! 4. Logarithmic volume control
//! 5. Audio device output
//!
//! # Features
//!
//! * Track preloading for gapless playback
//! * Volume normalization with limiter
//! * Flexible audio device selection
//! * Multiple audio host support
//!
//! # Example
//!
//! ```rust
//! use pleezer::player::Player;
//!
//! // Create player with default audio device
//! let mut player = Player::new(&config, "").await?;
//!
//! // Configure playback
//! player.set_normalization(true);
//! player.set_volume(volume);
//!
//! // Open the audio device
//! player.start()?;
//!
//! // Add tracks and start playback
//! player.set_queue(tracks);
//! player.play()?;
//!
//! // When done, close the audio device
//! player.stop();
//! ```

use std::{collections::HashSet, sync::Arc, time::Duration};

use cpal::traits::{DeviceTrait, HostTrait};
use md5::{Digest, Md5};
use rodio::Source;
use url::Url;

use crate::{
    config::Config,
    decrypt::{Decrypt, Key},
    error::{Error, ErrorKind, Result},
    events::Event,
    http,
    protocol::{
        connect::{
            contents::{AudioQuality, RepeatMode},
            Percentage,
        },
        gateway::{self, MediaUrl},
    },
    track::{Track, TrackId},
};

/// Audio sample type used by the decoder.
///
/// This is the native format that rodio's decoder produces,
/// used for internal audio processing.
type SampleFormat = <rodio::decoder::Decoder<std::fs::File> as Iterator>::Item;

/// Audio playback manager.
///
/// Handles:
/// * Audio device management
/// * Track downloading and decoding
/// * Queue management
/// * Playback control
/// * Volume normalization
///
/// Audio device lifecycle:
/// * Device is selected during construction
/// * Device is opened with `start()`
/// * Device is closed with `stop()`
/// * Device state affects method behavior:
///   - Most playback operations require an open device
///   - Configuration can be changed when device is closed
pub struct Player {
    /// Preferred audio quality setting.
    ///
    /// Actual quality may be lower if track isn't available
    /// in the preferred quality.
    audio_quality: AudioQuality,

    /// License token for media access.
    ///
    /// Required for downloading encrypted tracks.
    license_token: String,

    /// Key for track decryption.
    ///
    /// Used with Blowfish CBC encryption.
    bf_secret: Key,

    /// Ordered list of tracks for playback.
    queue: Vec<Track>,

    /// Set of track IDs to skip during playback.
    ///
    /// Tracks are added here when they fail to load
    /// or become unavailable.
    skip_tracks: HashSet<TrackId>,

    /// Current position in the queue.
    ///
    /// May exceed queue length to prepare for
    /// future queue updates.
    position: usize,

    /// Position to seek to after track loads.
    ///
    /// Used when seek is requested before track
    /// is fully loaded.
    deferred_seek: Option<Duration>,

    /// HTTP client for downloading tracks.
    ///
    /// Uses cookie-less client as tracks don't
    /// require authentication.
    client: http::Client,

    /// Current repeat mode setting.
    ///
    /// Controls behavior at queue boundaries.
    repeat_mode: RepeatMode,

    /// Whether shuffle mode is enabled.
    ///
    /// Note: Not yet implemented.
    shuffle: bool,

    /// Whether volume normalization is enabled.
    normalization: bool,

    /// Target gain for volume normalization in dB.
    ///
    /// Used to calculate normalization ratios.
    gain_target_db: i8,

    /// Raw volume setting as a percentage (0.0 to 1.0).
    ///
    /// This stores the user-set volume before logarithmic scaling is applied.
    /// The actual output volume uses logarithmic scaling for better perceived control.
    volume: Percentage,

    /// Channel for sending playback events.
    ///
    /// Events include:
    /// * Play/Pause
    /// * Track changes
    /// * Connection status
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<Event>>,

    /// Selected audio output device.
    ///
    /// Device is chosen during construction but not opened until `start()`.
    device: rodio::Device,

    /// Audio output configuration.
    ///
    /// Contains sample rate, format, and buffer size settings
    /// selected during construction.
    device_config: rodio::SupportedStreamConfig,

    /// Audio output sink.
    ///
    /// Handles final audio output and volume control.
    /// Only available when device is open (between `start()` and `stop()`).
    sink: Option<rodio::Sink>,

    /// Audio output stream handle.
    ///
    /// Must be kept alive to maintain playback.
    /// Only available when device is open (between `start()` and `stop()`).
    stream: Option<rodio::OutputStream>,

    /// Queue of audio sources.
    ///
    /// Contains decoded and processed audio data ready for playback.
    /// Only available when device is open (between `start()` and `stop()`).
    sources: Option<Arc<rodio::queue::SourcesQueueInput<SampleFormat>>>,

    /// When current track started playing.
    ///
    /// Used to calculate playback progress.
    playing_since: Duration,

    /// Completion signal for current track.
    ///
    /// Receiver is notified when track finishes.
    current_rx: Option<std::sync::mpsc::Receiver<()>>,

    /// Completion signal for preloaded track.
    ///
    /// Receiver is notified when preloaded track
    /// would finish. Used for gapless playback.
    preload_rx: Option<std::sync::mpsc::Receiver<()>>,

    /// Base URL for media content.
    ///
    /// Used to construct track download URLs.
    media_url: Url,
}

impl Player {
    /// Logarithmic volume scale factor for a dynamic range of 60 dB.
    ///
    /// Equal to 10^(60/20) = 1000.0
    const LOG_VOLUME_SCALE_FACTOR: f32 = 1000.0;

    /// Logarithmic volume growth rate for a dynamic range of 60 dB.
    ///
    /// Equal to ln(1000) ≈ 6.907755279
    const LOG_VOLUME_GROWTH_RATE: f32 = 6.907_755_4;

    /// Creates a new player instance.
    ///
    /// # Arguments
    ///
    /// * `config` - Player configuration including normalization settings
    /// * `device` - Audio device specification string:
    ///   ```text
    ///   [<host>][|<device>][|<sample rate>][|<sample format>]
    ///   ```
    ///   All parts are optional. Use empty string for system default.
    ///
    /// Note: This only stores the device specification without opening it,
    /// preventing ALSA from acquiring the device until `start()` is called.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Audio device specification is invalid
    /// * Device is not available
    /// * HTTP client creation fails
    /// * Decryption key is invalid
    pub async fn new(config: &Config, device: &str) -> Result<Self> {
        let (device, device_config) = Self::get_device(device)?;
        let client = http::Client::without_cookies(config)?;

        let bf_secret = if let Some(secret) = config.bf_secret {
            secret
        } else {
            debug!("no bf_secret specified, fetching one from the web player");
            Config::try_key(&client).await?
        };

        if format!("{:x}", Md5::digest(*bf_secret)) != Config::BF_SECRET_MD5 {
            return Err(Error::permission_denied("the bf_secret is not valid"));
        }

        #[expect(clippy::cast_possible_truncation)]
        let gain_target_db = gateway::user_data::Gain::default().target as i8;

        Ok(Self {
            queue: Vec::new(),
            skip_tracks: HashSet::new(),
            position: 0,
            audio_quality: AudioQuality::default(),
            client,
            license_token: String::new(),
            media_url: MediaUrl::default().into(),
            bf_secret,
            repeat_mode: RepeatMode::default(),
            shuffle: false,
            normalization: config.normalization,
            gain_target_db,
            volume: Percentage::from_ratio_f32(1.0),
            event_tx: None,
            playing_since: Duration::ZERO,
            deferred_seek: None,
            current_rx: None,
            preload_rx: None,
            device,
            device_config,
            sink: None,
            stream: None,
            sources: None,
        })
    }

    /// Selects and configures an audio output device.
    ///
    /// # Arguments
    ///
    /// * `device` - Device specification string in format:
    ///   ```text
    ///   [<host>][|<device>][|<sample rate>][|<sample format>]
    ///   ```
    ///
    /// # Returns
    ///
    /// Returns the selected device and its configuration.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Host is not found
    /// * Device is not found
    /// * Sample rate is invalid
    /// * Sample format is not supported
    /// * Device cannot be acquired (e.g., in use by another application)
    fn get_device(device: &str) -> Result<(rodio::Device, rodio::SupportedStreamConfig)> {
        // The device string has the following format:
        // "[<host>][|<device>][|<sample rate>][|<sample format>]" (case-insensitive)
        // From left to right, the fields are optional, but each field
        // depends on the preceding fields being specified.
        let mut components = device.split('|');

        // The host is the first field.
        let host = match components.next() {
            Some("") | None => cpal::default_host(),
            Some(name) => {
                let host_ids = cpal::available_hosts();
                host_ids
                    .into_iter()
                    .find_map(|host_id| {
                        let host = cpal::host_from_id(host_id).ok()?;
                        if host.id().name().eq_ignore_ascii_case(name) {
                            Some(host)
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| Error::not_found(format!("audio host {name} not found")))?
            }
        };

        // The device is the second field.
        let device = match components.next() {
            Some("") | None => host.default_output_device().ok_or_else(|| {
                Error::not_found(format!(
                    "default audio output device not found on {}",
                    host.id().name()
                ))
            })?,
            Some(name) => {
                let mut devices = host.output_devices()?;
                devices
                    .find(|device| device.name().is_ok_and(|n| n.eq_ignore_ascii_case(name)))
                    .ok_or_else(|| {
                        Error::not_found(format!(
                            "audio output device {name} not found on {}",
                            host.id().name()
                        ))
                    })?
            }
        };

        let config = match components.next() {
            Some("") | None => device.default_output_config().map_err(|e| {
                Error::unavailable(format!("default output configuration unavailable: {e}"))
            })?,
            Some(rate) => {
                let rate = rate
                    .parse()
                    .map_err(|_| Error::invalid_argument(format!("invalid sample rate {rate}")))?;
                let rate = cpal::SampleRate(rate);

                let format = match components.next() {
                    Some("") | None => None,
                    other => other,
                };

                device
                    .supported_output_configs()?
                    .find_map(|config| {
                        if format.is_none_or(|format| {
                            config
                                .sample_format()
                                .to_string()
                                .eq_ignore_ascii_case(format)
                        }) {
                            config.try_with_sample_rate(rate)
                        } else {
                            None
                        }
                    })
                    .ok_or_else(|| {
                        Error::unavailable(format!(
                            "audio output device {} does not support sample rate {} with {} sample format",
                            device.name().as_deref().unwrap_or("UNKNOWN"),
                            rate.0,
                            format.unwrap_or("default")
                        ))
                    })?
            }
        };

        info!(
            "audio output device: {} on {}",
            device.name().as_deref().unwrap_or("UNKNOWN"),
            host.id().name()
        );

        #[expect(clippy::cast_precision_loss)]
        let sample_rate = config.sample_rate().0 as f32 / 1000.0;
        info!(
            "audio output configuration: {sample_rate:.1} kHz in {}",
            config.sample_format()
        );
        trace!("audio buffer size: {:#?}", config.buffer_size());

        Ok((device, config))
    }

    /// Opens the audio output device for playback.
    ///
    /// Must be called before playback operations like `play()` or `set_progress()`.
    /// The device remains open until `stop()` is called or the player is dropped.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Audio device cannot be opened
    /// * Output stream creation fails
    /// * Sink creation fails
    pub fn start(&mut self) -> Result<()> {
        debug!("opening output device");
        let (stream, handle) =
            rodio::OutputStream::try_from_device_config(&self.device, self.device_config.clone())?;
        let sink = rodio::Sink::try_new(&handle)?;

        // Set the volume to the last known value.
        sink.set_volume(self.volume.as_ratio_f32());

        // The output source will output silence when the queue is empty.
        // That will cause the sink to report as "playing", so we need to pause it.
        let (sources, output) = rodio::queue::queue(true);
        sink.append(output);
        sink.pause();

        self.sink = Some(sink);
        self.sources = Some(sources);
        self.stream = Some(stream);

        Ok(())
    }

    /// Closes the audio output device and stops playback.
    ///
    /// Releases audio device resources and clears any queued audio.
    /// The player can be restarted with `start()`.
    ///
    /// Note: This method is automatically called when the player is dropped,
    /// ensuring proper cleanup of audio device resources.
    pub fn stop(&mut self) {
        // Don't care if the sink is already dropped: we're already "stopped".
        if let Ok(sink) = self.sink_mut() {
            debug!("closing output device");
            sink.stop();
        }

        self.sources = None;
        self.stream = None;
        self.sink = None;
    }

    /// The list of supported sample rates.
    ///
    /// This list is used to filter out unreasonable sample rates.
    /// Common sample rates in Hz:
    /// * 44100 - CD audio, most streaming services
    /// * 48000 - Professional digital audio, DVDs, most DAWs
    /// * 88200/96000 - High resolution audio
    /// * 176400/192000 - Studio quality
    /// * 352800/384000 - Ultra high definition audio
    const SAMPLE_RATES: [u32; 8] = [
        44_100, 48_000, 88_200, 96_000, 176_400, 192_000, 352_800, 384_000,
    ];

    /// Lists available audio output devices.
    ///
    /// Returns a sorted list of device specifications in the format:
    /// ```text
    /// <host>|<device>|<sample rate>|<sample format>
    /// ```
    ///
    /// Only includes devices supporting common sample rates:
    /// * 44.1/48 kHz (standard)
    /// * 88.2/96 kHz (high resolution)
    /// * 176.4/192 kHz (studio)
    /// * 352.8/384 kHz (ultra HD)
    ///
    /// Default device is marked with "(default)" suffix.
    #[must_use]
    pub fn enumerate_devices() -> Vec<String> {
        let hosts = cpal::available_hosts();

        // Create a set to store the unique device names.
        // On Alsa hosts, the same device may otherwise be enumerated multiple times.
        let mut result = HashSet::new();

        // Get the default host, device and config.
        let default_host = cpal::default_host();
        let default_device = default_host.default_output_device();
        let default_config = default_device
            .as_ref()
            .and_then(|device| device.default_output_config().ok());

        // Enumerate all available hosts, devices and configs.
        for host in hosts
            .into_iter()
            .filter_map(|id| cpal::host_from_id(id).ok())
        {
            if let Ok(devices) = host.output_devices() {
                for device in devices {
                    if let Ok(configs) = device.supported_output_configs() {
                        for config in configs {
                            if let Ok(device_name) = device.name() {
                                for sample_rate in &Self::SAMPLE_RATES {
                                    if let Some(config) =
                                        config.try_with_sample_rate(cpal::SampleRate(*sample_rate))
                                    {
                                        let mut line = format!(
                                            "{}|{}|{}|{}",
                                            host.id().name(),
                                            device_name,
                                            config.sample_rate().0,
                                            config.sample_format(),
                                        );

                                        // Check if this is the default host, device
                                        // and config.
                                        if default_host.id() == host.id()
                                            && default_device.as_ref().is_some_and(
                                                |default_device| {
                                                    default_device.name().is_ok_and(
                                                        |default_name| default_name == device_name,
                                                    )
                                                },
                                            )
                                            && default_config.as_ref().is_some_and(
                                                |default_config| *default_config == config,
                                            )
                                        {
                                            line.push_str(" (default)");
                                        }

                                        result.insert(line);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let mut result: Vec<_> = result.into_iter().collect();
        result.sort();
        result
    }

    /// Advances to the next track in the queue.
    ///
    /// Handles:
    /// * Repeat mode logic
    /// * Position updates
    /// * Event notifications
    ///
    /// Behavior depends on repeat mode:
    /// * `None`: Stops at end of queue
    /// * `One`: Stays on current track
    /// * `All`: Loops back to start of queue
    fn go_next(&mut self) {
        let old_position = self.position;
        let repeat_mode = self.repeat_mode();
        if repeat_mode != RepeatMode::One {
            let next = self.position.saturating_add(1);
            if next < self.queue.len() {
                // Move to the next track.
                self.position = next;
            } else {
                // Reached the end of the queue: rewind to the beginning.
                if repeat_mode != RepeatMode::All {
                    // Don't care if the sink is already dropped: we're already "paused".
                    let _ = self.pause();
                };
                self.position = 0;
            }
        }

        if self.position() != old_position {
            self.notify(Event::TrackChanged);
        }

        // Even if we were already playing, we need to report another playback stream.
        if self.is_playing() {
            self.notify(Event::Play);
        }
    }

    /// The audio gain control (AGC) attack time.
    /// This value is equal to what Spotify uses.
    const AGC_ATTACK_TIME: Duration = Duration::from_millis(5);

    /// The audio gain control (AGC) release time.
    /// This value is equal to what Spotify uses.
    const AGC_RELEASE_TIME: Duration = Duration::from_millis(100);

    /// Loads and prepares a track for playback.
    ///
    /// Downloads, decrypts, and configures audio processing for a track:
    /// 1. Downloads encrypted content
    /// 2. Sets up decryption
    /// 3. Configures audio decoder
    /// 4. Applies volume normalization if enabled
    ///
    /// # Arguments
    ///
    /// * `position` - Queue position of track to load
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Audio device is not open (no sources available)
    /// * Track download fails
    /// * Decryption fails
    /// * Audio decoding fails
    // TODO : consider controlflow
    async fn load_track(
        &mut self,
        position: usize,
    ) -> Result<Option<std::sync::mpsc::Receiver<()>>> {
        let track = self
            .queue
            .get_mut(position)
            .ok_or_else(|| Error::not_found(format!("track at position {position} not found")))?;

        let sources = self
            .sources
            .as_mut()
            .ok_or(Error::unavailable("audio sources not available"))?;

        if track.handle().is_none() {
            let download = tokio::time::timeout(Duration::from_secs(3), async {
                // Start downloading the track.
                let medium = track
                    .get_medium(
                        &self.client,
                        &self.media_url,
                        self.audio_quality,
                        self.license_token.clone(),
                    )
                    .await?;

                // Return `None` on success to indicate that the track is not yet appended
                // to the sink.
                track.start_download(&self.client, &medium).await
            })
            .await??;

            // Append the track to the sink.
            let decryptor = Decrypt::new(track, download, &self.bf_secret)?;
            let mut decoder = match track.quality() {
                AudioQuality::Lossless => rodio::Decoder::new_flac(decryptor),
                _ => rodio::Decoder::new_mp3(decryptor),
            }?;

            if let Some(progress) = self.deferred_seek.take() {
                // Set the track position only if `progress` is beyond the track start. We start
                // at the beginning anyway, and this prevents decoder errors.
                if !progress.is_zero() {
                    if let Err(e) = decoder.try_seek(progress) {
                        error!("failed to seek to deferred position: {}", e);
                    }
                }
            }

            let mut difference = 0.0;
            let mut ratio = 1.0;
            if self.normalization {
                match track.gain() {
                    Some(gain) => {
                        difference = f32::from(self.gain_target_db) - gain;

                        // Keep -1 dBTP of headroom on tracks with lossy decoding to avoid
                        // clipping due to inter-sample peaks.
                        if difference > 0.0 && !track.is_lossless() {
                            difference -= 1.0;
                        }

                        ratio = f32::powf(10.0, difference / 20.0);
                    }
                    None => {
                        warn!("track {track} has no gain information, skipping normalization");
                    }
                }
            }

            let rx = if ratio < 1.0 {
                debug!(
                    "attenuating track {track} by {difference:.1} dB ({})",
                    Percentage::from_ratio_f32(ratio)
                );
                let attenuated = decoder.amplify(ratio);
                sources.append_with_signal(attenuated)
            } else if ratio > 1.0 {
                debug!(
                    "amplifying track {track} by {difference:.1} dB ({}) (with limiter)",
                    Percentage::from_ratio_f32(ratio)
                );
                let amplified = decoder.automatic_gain_control(
                    ratio,
                    Self::AGC_ATTACK_TIME.as_secs_f32(),
                    Self::AGC_RELEASE_TIME.as_secs_f32(),
                    difference,
                );
                sources.append_with_signal(amplified)
            } else {
                sources.append_with_signal(decoder)
            };

            return Ok(Some(rx));
        }

        Ok(None)
    }

    /// Returns the current playback position from the sink.
    ///
    /// Returns `Duration::ZERO` if audio device is not open.
    #[must_use]
    fn get_pos(&self) -> Duration {
        // If the sink is not available, we're not playing anything, so the position is 0.
        self.sink
            .as_ref()
            .map_or(Duration::ZERO, rodio::Sink::get_pos)
    }

    /// Main playback loop.
    ///
    /// Continuously:
    /// * Monitors current track completion
    /// * Manages track preloading
    /// * Handles playback transitions
    /// * Processes track unavailability
    ///
    /// Audio playback requires calling `start()` to open the audio device,
    /// but track loading and queue management will work without it.
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Track loading fails critically
    /// * Audio system fails
    pub async fn run(&mut self) -> Result<()> {
        loop {
            match self.current_rx.as_mut() {
                Some(current_rx) => {
                    // Check if the current track has finished playing.
                    if current_rx.try_recv().is_ok() {
                        // Save the point in time when the track finished playing.
                        self.playing_since = self.get_pos();

                        // Move the preloaded track, if any, to the current track.
                        self.current_rx = self.preload_rx.take();
                        self.go_next();
                    }

                    // Preload the next track if all of the following conditions are met:
                    // - the repeat mode is not "Repeat One"
                    // - the current track is done downloading
                    if self.preload_rx.is_none()
                        && self.repeat_mode() != RepeatMode::One
                        && self.track().is_some_and(Track::is_complete)
                    {
                        let next_position = self.position.saturating_add(1);
                        if let Some(next_track) = self.queue.get(next_position) {
                            let next_track_id = next_track.id();
                            if !self.skip_tracks.contains(&next_track_id) {
                                match self.load_track(next_position).await {
                                    Ok(rx) => {
                                        self.preload_rx = rx;
                                    }
                                    Err(e) => {
                                        error!("failed to preload next track: {e}");
                                        self.mark_unavailable(next_track_id);
                                    }
                                }
                            }
                        }
                    }
                }

                None => {
                    if let Some(track) = self.track() {
                        let track_id = track.id();
                        if self.skip_tracks.contains(&track_id) {
                            self.go_next();
                        } else {
                            match self.load_track(self.position).await {
                                Ok(rx) => {
                                    if let Some(rx) = rx {
                                        self.current_rx = Some(rx);
                                        self.notify(Event::TrackChanged);
                                        if self.is_playing() {
                                            self.notify(Event::Play);
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("failed to load track: {e}");
                                    self.mark_unavailable(track_id);
                                    self.go_next();
                                }
                            }
                        }
                    }
                }
            }

            // Yield to the runtime to allow other tasks to run.
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// Marks a track as unavailable for playback.
    ///
    /// Tracks marked unavailable will be skipped during playback.
    /// Logs a warning the first time a track is marked unavailable.
    fn mark_unavailable(&mut self, track_id: TrackId) {
        if self.skip_tracks.insert(track_id) {
            warn!("marking track {track_id} as unavailable");
        }
    }

    /// Sends a playback event notification.
    ///
    /// Events are sent through the registered channel if available.
    /// Failures are logged but do not interrupt playback.
    fn notify(&self, event: Event) {
        if let Some(event_tx) = &self.event_tx {
            if let Err(e) = event_tx.send(event) {
                error!("failed to send event: {e}");
            }
        }
    }

    /// Registers an event notification channel.
    ///
    /// Events sent include:
    /// * Play/Pause state changes
    /// * Track changes
    /// * Connection status
    pub fn register(&mut self, event_tx: tokio::sync::mpsc::UnboundedSender<Event>) {
        self.event_tx = Some(event_tx);
    }

    /// Returns a mutable reference to the sink if available.
    ///
    /// # Errors
    /// Returns error if audio device is not open.
    fn sink_mut(&mut self) -> Result<&mut rodio::Sink> {
        self.sink
            .as_mut()
            .ok_or(Error::unavailable("audio sink not available"))
    }

    /// Starts or resumes playback.
    ///
    /// Emits a Play event if playback actually starts.
    /// Does nothing if already playing.
    ///
    /// # Errors
    ///
    /// Returns error if audio device is not open.
    pub fn play(&mut self) -> Result<()> {
        if !self.is_playing() {
            debug!("starting playback");
            self.sink_mut()?.play();

            // Playback reporting happens every time a track starts playing or is unpaused.
            self.notify(Event::Play);
        }

        Ok(())
    }

    /// Pauses playback.
    ///
    /// Emits a Pause event if playback was actually playing.
    /// Does nothing if already paused.
    ///
    /// # Errors
    ///
    /// Returns error if audio device is not open.
    pub fn pause(&mut self) -> Result<()> {
        if self.is_playing() {
            debug!("pausing playback");
            // Don't care if the sink is already dropped: we're already "paused".
            let _ = self.sink_mut().map(|sink| sink.pause());
            self.notify(Event::Pause);
        }
        Ok(())
    }

    /// Returns whether playback is active.
    ///
    /// # Returns
    ///
    /// `true` if both:
    /// * A track is loaded (`current_rx` is Some)
    /// * Audio device is open and sink is not paused
    ///
    /// Note: Will return `false` if audio device is not open,
    /// even if a track is loaded and ready to play.
    #[must_use]
    pub fn is_playing(&self) -> bool {
        self.current_rx.is_some() && self.sink.as_ref().is_some_and(|sink| !sink.is_paused())
    }

    /// Sets the playback state.
    ///
    /// Convenience method that:
    /// * Calls `play()` if `should_play` is true
    /// * Calls `pause()` if `should_play` is false
    ///
    /// # Arguments
    ///
    /// * `should_play` - Desired playback state
    ///
    /// # Errors
    ///
    /// Returns error if audio device is not open.
    pub fn set_playing(&mut self, should_play: bool) -> Result<()> {
        if should_play {
            self.play()
        } else {
            self.pause()
        }
    }

    /// Returns the currently playing track, if any.
    #[must_use]
    pub fn track(&self) -> Option<&Track> {
        self.queue.get(self.position)
    }

    /// Replaces the entire playback queue.
    ///
    /// * Clears current queue and playback state
    /// * Resets position to start
    /// * Clears skip track list
    pub fn set_queue(&mut self, tracks: Vec<Track>) {
        self.clear();
        self.position = 0;
        self.queue = tracks;
        self.skip_tracks.clear();
        self.skip_tracks.shrink_to_fit();
    }

    /// Adds tracks to the end of the queue.
    ///
    /// Preserves current playback position and state.
    pub fn extend_queue(&mut self, tracks: Vec<Track>) {
        self.queue.extend(tracks);
    }

    /// Sets the current playback position in the queue.
    ///
    /// Position can exceed queue length to prepare for
    /// future queue updates.
    ///
    /// Note: Setting to current position is ignored to
    /// prevent interrupting seeks.
    pub fn set_position(&mut self, position: usize) {
        // If the position is already set, do nothing. Deezer also sends the same position when
        // seeking, in which case we should not clear the current track.
        if self.position == position {
            return;
        }

        info!("setting playlist position to {position}");

        // Clear the sink, which will drop any handles to the current and next tracks.
        self.clear();
        self.position = position;
    }

    /// Clears the playback state.
    ///
    /// * Creates new empty source queue (if sink is active)
    /// * Resets track downloads
    /// * Resets internal playback state (position, receivers)
    ///
    /// When sink is active:
    /// * Creates new empty source queue
    /// * Maintains playback capability
    pub fn clear(&mut self) {
        if let Ok(sink) = self.sink_mut() {
            // Don't just clear the sink, because that makes Rodio stop playback. The following code
            // works around that by creating a new, empty queue of sources and skipping to it.
            let (sources, output) = rodio::queue::queue(true);
            sink.append(output);
            sink.skip_one();
            self.sources = Some(sources);
        }

        // Resetting the sink drops any downloads of the current and next tracks.
        // We need to reset the download state of those tracks.
        if let Some(current) = self.queue.get_mut(self.position) {
            current.reset_download();
        }
        if let Some(next) = self.queue.get_mut(self.position.saturating_add(1)) {
            next.reset_download();
        }

        self.playing_since = Duration::ZERO;
        self.current_rx = None;
        self.preload_rx = None;
    }

    /// Returns whether shuffle mode is enabled.
    #[must_use]
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    /// Sets shuffle mode for playback.
    ///
    /// Note: Shuffle functionality is not yet implemented.
    /// Setting to true will log a warning.
    pub fn set_shuffle(&mut self, shuffle: bool) {
        info!("setting shuffle to {shuffle}");
        self.shuffle = shuffle;

        // TODO: implement shuffle
        if shuffle {
            warn!("shuffle is not yet implemented");
        }
    }

    /// Returns the current repeat mode.
    #[must_use]
    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    /// Sets the repeat mode for playback.
    ///
    /// When setting to `RepeatMode::One`:
    /// * Clears preloaded track
    /// * Disables track preloading
    pub fn set_repeat_mode(&mut self, repeat_mode: RepeatMode) {
        info!("setting repeat mode to {repeat_mode}");
        self.repeat_mode = repeat_mode;

        if repeat_mode == RepeatMode::One {
            // This only clears the preloaded track.
            self.sources.as_mut().map(|sources| sources.clear());
            self.preload_rx = None;
        }
    }

    /// Returns the last volume setting as a percentage.
    ///
    /// Returns the raw volume value that was set, before logarithmic scaling is applied.
    /// The actual audio output uses logarithmic scaling to match human perception.
    ///
    /// # Returns
    ///
    /// * The last volume set via `set_volume()`
    /// * 1.0 (100%) if volume was never set
    ///
    /// Note: This returns the stored volume setting even if the audio device is closed.
    #[must_use]
    pub fn volume(&self) -> Percentage {
        self.volume
    }
    /// Sets playback volume with logarithmic scaling.
    ///
    /// The volume control uses a logarithmic scale that matches human perception:
    /// * Logarithmic scaling across a 60 dB dynamic range
    /// * Linear fade to zero for very low volumes (< 10%)
    /// * Smooth transitions across the entire range
    ///
    /// No effect if new volume equals current volume.
    ///
    /// # Arguments
    ///
    /// * `volume` - Target volume percentage (0.0 to 1.0)
    ///
    /// # Errors
    ///
    /// Returns error if audio device is not open.
    pub fn set_volume(&mut self, volume: Percentage) -> Result<()> {
        if volume == self.volume() {
            return Ok(());
        }

        info!("setting volume to {volume}");
        self.volume = volume;

        let volume = volume.as_ratio_f32().clamp(0.0, 1.0);
        let mut amplitude = volume;

        // Apply logarithmic volume scaling with a smooth transition to zero.
        // Source: https://www.dr-lex.be/info-stuff/volumecontrols.html
        if amplitude > 0.0 && amplitude < 1.0 {
            amplitude =
                f32::exp(Self::LOG_VOLUME_GROWTH_RATE * volume) / Self::LOG_VOLUME_SCALE_FACTOR;
            if volume < 0.1 {
                amplitude *= volume * 10.0;
            }
            debug!(
                "volume scaled logarithmically: {}",
                Percentage::from_ratio_f32(amplitude)
            );
        }

        self.sink_mut().map(|sink| sink.set_volume(amplitude))
    }

    /// Returns current playback progress.
    ///
    /// Returns None if no track is playing.
    /// Progress is calculated as:
    /// * Current sink position (or zero if device not open)
    /// * Minus track start time
    /// * Divided by track duration
    #[must_use]
    pub fn progress(&self) -> Option<Percentage> {
        // The progress is the difference between the current position of the sink, which is the
        // total duration played, and the time the current track started playing.
        let progress = self.get_pos().saturating_sub(self.playing_since);

        self.track().map(|track| {
            let ratio = progress.div_duration_f32(track.duration());
            Percentage::from_ratio_f32(ratio)
        })
    }

    /// Sets playback position within current track.
    ///
    /// # Behavior
    ///
    /// * If progress < 1.0: Seeks within track
    /// * If progress >= 1.0: Skips to next track
    /// * If track not loaded: Defers seek
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * No track is playing
    /// * Seek operation fails
    ///
    /// # Errors
    ///
    /// * No track is playing
    /// * Audio device is not open
    /// * Seek operation fails
    pub fn set_progress(&mut self, progress: Percentage) -> Result<()> {
        if let Some(track) = self.track() {
            info!("setting track progress to {progress}");
            let progress = progress.as_ratio_f32();
            if progress < 1.0 {
                let progress = track.duration().mul_f32(progress);
                match self
                    .sink_mut()
                    .and_then(|sink| sink.try_seek(progress).map_err(Into::into))
                {
                    Ok(()) => {
                        // Reset the playing time to zero, as the sink will now reset it also.
                        self.playing_since = Duration::ZERO;
                        self.deferred_seek = None;
                    }
                    Err(e) => {
                        if matches!(e.kind, ErrorKind::Unavailable | ErrorKind::Unimplemented) {
                            // If the current track is not buffered yet, we can't seek.
                            // In that case, we defer the seek until the track is buffered.
                            self.deferred_seek = Some(progress);
                        } else {
                            // If the seek failed for any other reason, we return an error.
                            return Err(e);
                        }
                    }
                }
            } else {
                // Setting the progress to 1.0 is equivalent to skipping to the next track.
                // This prevents `UnexpectedEof` when seeking to the end of the track.
                self.clear();
                self.go_next();
            }
        }

        Ok(())
    }

    /// Returns current position in the queue.
    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }

    /// Sets the license token for media access.
    pub fn set_license_token(&mut self, license_token: impl Into<String>) {
        self.license_token = license_token.into();
    }

    /// Enables or disables volume normalization.
    pub fn set_normalization(&mut self, normalization: bool) {
        self.normalization = normalization;
    }

    /// Sets target gain for volume normalization.
    ///
    /// Logs info message if normalization is enabled.
    ///
    /// # Arguments
    ///
    /// * `gain_target_db` - Target gain in decibels
    pub fn set_gain_target_db(&mut self, gain_target_db: i8) {
        if self.normalization {
            info!("normalizing volume to {gain_target_db} dB");
        }
        self.gain_target_db = gain_target_db;
    }

    /// Sets preferred audio quality for playback.
    ///
    /// Note: Actual quality may be lower if track is not
    /// available in requested quality.
    pub fn set_audio_quality(&mut self, quality: AudioQuality) {
        self.audio_quality = quality;
    }

    /// Returns whether volume normalization is enabled.
    #[must_use]
    pub fn normalization(&self) -> bool {
        self.normalization
    }

    /// Returns current license token.
    #[must_use]
    pub fn license_token(&self) -> &str {
        &self.license_token
    }

    /// Returns current preferred audio quality setting.
    #[must_use]
    pub fn audio_quality(&self) -> AudioQuality {
        self.audio_quality
    }

    /// Returns current normalization target gain.
    #[must_use]
    pub fn gain_target_db(&self) -> i8 {
        self.gain_target_db
    }

    /// Sets the media content URL.
    pub fn set_media_url(&mut self, url: Url) {
        self.media_url = url;
    }

    /// Returns whether the audio device is open.
    ///
    /// True if `start()` has been called and the device was successfully opened.
    /// False if device has not been opened or has been closed with `stop()`.
    ///
    /// # Example
    /// ```
    /// let mut player = Player::new(&config, "").await?;
    /// assert!(!player.is_started());
    ///
    /// player.start()?;
    /// assert!(player.is_started());
    ///
    /// player.stop();
    /// assert!(!player.is_started());
    /// ```
    #[must_use]
    pub fn is_started(&self) -> bool {
        self.sink.is_some()
    }
}

impl Drop for Player {
    /// Ensures the audio device is properly closed when the player is dropped.
    fn drop(&mut self) {
        self.stop();
    }
}
