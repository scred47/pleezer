use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait};

use crate::{
    config::Config,
    decrypt::Decrypt,
    error::{Error, Result},
    events::Event,
    http,
    protocol::connect::{
        contents::{AudioQuality, RepeatMode},
        Percentage,
    },
    track::Track,
};

pub struct Player {
    /// The *preferred* audio quality. The actual quality may be lower if the
    /// track is not available in the preferred quality.
    pub audio_quality: AudioQuality,

    /// The license token to use for downloading tracks.
    pub license_token: String,

    /// The list of tracks to play, a.k.a. the playlist.
    tracks: Vec<Track>,

    /// The current position in the playlist.
    position: Option<usize>,

    /// The track that is currently playing. When this is different from the
    /// track at `position`, the player is transitioning to another track.
    track_in_sink: Option<usize>,

    /// The HTTP client to use for downloading tracks.
    client: http::Client,

    /// The repeat mode.
    repeat_mode: RepeatMode,

    /// Whether the playlist should be shuffled.
    shuffle: bool,

    /// The channel to send playback events to.
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<Event>>,

    /// The audio output sink.
    sink: rodio::Sink,

    /// The audio output stream. Although not used directly, this field is
    /// necessary to retain to keep the sink alive.
    _stream: rodio::OutputStream,
}

impl Player {
    /// Creates a new `Player` with the given `Config`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if no HTTP client can be built from the `Config`.
    pub fn new(config: &Config, device: &str) -> Result<Self> {
        let (sink, stream) = Self::open_sink(device)?;

        Ok(Self {
            tracks: Vec::new(),
            position: None,
            track_in_sink: None,
            audio_quality: AudioQuality::default(),
            client: http::Client::without_cookies(config)?,
            license_token: String::new(),
            repeat_mode: RepeatMode::default(),
            shuffle: false,
            event_tx: None,
            _stream: stream,
            sink,
        })
    }

    fn open_sink(device: &str) -> Result<(rodio::Sink, rodio::OutputStream)> {
        let (stream, handle) = {
            // The device string has the following format:
            // [<host>][:<device>][:<sample rate>:<sample format>]
            // From left to right, the fields are optional, but each field
            // depends on the preceding fields being specified.
            let mut components = device.split(':');

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

            let (stream, handle) = match components.next() {
                Some("") | None => rodio::OutputStream::try_from_device(&device)?,
                Some(rate) => {
                    let rate = rate.parse().map_err(|_| {
                        Error::invalid_argument(format!("invalid sample rate {rate}"))
                    })?;
                    let rate = cpal::SampleRate(rate);

                    let format = match components.next() {
                        Some("") | None => None,
                        other => other,
                    };

                    let config = device
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
                            })?;

                    rodio::OutputStream::try_from_device_config(&device, config)?
                }
            };

            info!(
                "audio output device: {} on {}",
                device.name().as_deref().unwrap_or("UNKNOWN"),
                host.id().name()
            );

            (stream, handle)
        };

        let sink = rodio::Sink::try_new(&handle)?;

        Ok((sink, stream))
    }

    #[must_use]
    pub fn enumerate_devices() -> Vec<String> {
        let hosts = cpal::available_hosts();
        let mut result = Vec::new();

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
                                let max_sample_rate = config.with_max_sample_rate();
                                let mut line = format!(
                                    "{}:{}:{}:{}",
                                    host.id().name(),
                                    device_name,
                                    max_sample_rate.sample_rate().0,
                                    max_sample_rate.sample_format(),
                                );

                                // Check if this is the default host, device
                                // and config.
                                if default_host.id() == host.id()
                                    && default_device.as_ref().is_some_and(|default_device| {
                                        default_device
                                            .name()
                                            .is_ok_and(|default_name| default_name == device_name)
                                    })
                                    && default_config.as_ref().is_some_and(|default_config| {
                                        *default_config == max_sample_rate
                                    })
                                {
                                    line.push_str(" (default)");
                                }

                                result.push(line);
                            }
                        }
                    }
                }
            }
        }

        result
    }

    fn skip_one(&mut self) {
        // TODO : wrap if repeat, or stop
        self.position = self.position.map(|position| position + 1);
    }

    /// Run the player.
    ///
    /// This function will monitor the position in the playlist and start downloading
    /// the track if it is pending. It will then play the track and skip to the next
    /// track when the current track is finished.
    ///
    /// # Errors
    ///
    /// This function may return an error if the player fails to start downloading
    /// the track, or if the player fails to play the track.
    pub async fn run(&mut self) -> Result<()> {
        loop {
            // TODO : change into track id
            if self.position != self.track_in_sink {
                // TODO if self.position.and_then()...
                if let Some(position) = self.position {
                    if let Some(target_track) = self.tracks.get_mut(position) {
                        if target_track.is_complete() {
                            let decryptor = Decrypt::new(&target_track, b"0123456789123456")?;
                            let decoder = rodio::Decoder::new(decryptor)?;
                            self.sink.append(decoder);
                        }

                        // Start downloading the track if it is pending.
                        if target_track.is_pending() {
                            match target_track
                                .get_medium(
                                    &self.client,
                                    self.audio_quality,
                                    self.license_token.clone(),
                                )
                                .await
                            {
                                Ok(medium) => {
                                    // TODO : if Ok, add to the sink
                                    if let Err(e) =
                                        target_track.start_download(&self.client, medium).await
                                    {
                                        error!(
                                            "skipping track {target_track}, failed to start download: {e}",
                                        );
                                        //self.skip_one();
                                    }
                                }
                                Err(err) => {
                                    error!(
                                        "skipping track {target_track}, failed to get medium: {err}",
                                    );
                                    self.skip_one();
                                }
                            }
                        }
                    }
                } else {
                    // Clear the sink if the queue has become empty.
                    self.sink.clear();
                    self.track_in_sink = self.position;
                }
            }

            // Yield to the runtime to allow other tasks to run.
            tokio::task::yield_now().await;
        }
    }

    pub fn register(&mut self, event_tx: tokio::sync::mpsc::UnboundedSender<Event>) {
        self.event_tx = Some(event_tx);
    }

    pub fn play(&mut self) {
        debug!("starting playback");
        self.sink.play();
    }

    pub fn stop(&mut self) {
        debug!("stopping playback");
        self.sink.pause();
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        !self.sink.is_paused()
    }

    pub fn set_playing(&mut self, should_play: bool) {
        if self.is_playing() {
            if !should_play {
                self.stop();
            }
        } else if should_play {
            self.play();

            if let Some(track) = self.track() {
                // TODO - notify when moving to next track
                if let Some(event_tx) = &self.event_tx {
                    if let Err(e) = event_tx.send(Event::TrackChanged(track.id())) {
                        error!("failed to send track changed event: {e}");
                    }
                }
            }
        }
    }

    #[must_use]
    pub fn track(&self) -> Option<&Track> {
        self.tracks.get(self.position?)
    }

    pub fn set_tracks(&mut self, tracks: Vec<Track>) {
        self.position = None;
        self.tracks = tracks;
    }

    /// Sets the playlist position.
    ///
    /// # Errors
    ///
    /// Returns an error if the position is out of range.
    pub fn set_position(&mut self, position: usize) -> Result<()> {
        let len = self.tracks.len();
        if position >= len {
            return Err(Error::out_of_range(format!(
                "invalid position {position} for queue with {len} items",
            )));
        }

        debug!("setting playlist position to {position}");
        self.position = Some(position);

        Ok(())
    }

    #[must_use]
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        debug!("setting shuffle to {shuffle}");
        self.shuffle = shuffle;
    }

    #[must_use]
    pub fn repeat_mode(&self) -> RepeatMode {
        self.repeat_mode
    }

    pub fn set_repeat_mode(&mut self, repeat_mode: RepeatMode) {
        debug!("setting repeat mode to {repeat_mode}");
        self.repeat_mode = repeat_mode;
    }

    #[must_use]
    pub fn volume(&self) -> Percentage {
        let ratio = self.sink.volume();
        Percentage::from_ratio_f32(ratio)
    }

    pub fn set_volume(&mut self, volume: Percentage) {
        debug!("setting volume to {volume}");
        let ratio = volume.as_ratio_f32();
        self.sink.set_volume(ratio);
    }

    #[must_use]
    pub fn progress(&self) -> Option<Percentage> {
        let progress = self.sink.get_pos();
        self.track().map(|track| {
            let ratio = track.duration().div_duration_f32(progress);
            Percentage::from_ratio_f32(ratio)
        })
    }

    /// # Errors
    ///
    /// Will return `Err` if:
    /// - there is no active track
    pub fn set_progress(&mut self, progress: Percentage) -> Result<()> {
        if !(0.0..=1.0).contains(&progress.as_ratio_f32()) {
            return Err(Error::invalid_argument(format!(
                "progress cannot be set to {progress}"
            )));
        }

        if self.track().is_some() {
            debug!("setting track progress to {progress}");
            // OK to multiply unchecked, because `progress` is clamped above.
            let progress = self.track().map_or(Duration::ZERO, |track| {
                track.duration().mul_f32(progress.as_ratio_f32())
            });
            self.sink.try_seek(progress).map_err(Into::into)
        } else {
            Err(Error::failed_precondition(
                "position cannot be set without an active track".to_string(),
            ))
        }
    }

    #[must_use]
    pub fn position(&self) -> Option<usize> {
        self.position
    }
}
