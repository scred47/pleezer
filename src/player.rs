use std::{sync::Arc, time::Duration};

use cpal::traits::{DeviceTrait, HostTrait};

use crate::{
    config::Config,
    decrypt::{Decrypt, Key},
    error::{Error, Result},
    events::Event,
    http,
    protocol::connect::{
        contents::{AudioQuality, RepeatMode},
        Percentage,
    },
    track::{State, Track},
};

/// The sample format used by the player, as determined by the decoder.
type SampleFormat = <rodio::decoder::Decoder<std::fs::File> as Iterator>::Item;

pub struct Player {
    /// The *preferred* audio quality. The actual quality may be lower if the
    /// track is not available in the preferred quality.
    pub audio_quality: AudioQuality,

    /// The license token to use for downloading tracks.
    pub license_token: String,

    /// The decryption key to use for decrypting tracks.
    pub bf_secret: Key,

    /// The track queue, a.k.a. the playlist.
    queue: Vec<Track>,

    /// The current position in the queue.
    position: usize,

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

    /// The source queue with the audio data.
    sources: Arc<rodio::queue::SourcesQueueInput<SampleFormat>>,

    /// The point in time of the sink when the current track started playing.
    playing_since: Duration,

    /// The signal to receive when the current track has finished playing.
    current_rx: Option<std::sync::mpsc::Receiver<()>>,

    /// The signal to receive when the preloaded track will finish playing.
    preload_rx: Option<std::sync::mpsc::Receiver<()>>,

    /// The audio output stream. Although not used directly, this field must be retained to keep
    /// the sink alive.
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
        let (sources, output) = rodio::queue::queue(true);
        sink.append(output);

        Ok(Self {
            queue: Vec::new(),
            position: 0,
            audio_quality: AudioQuality::default(),
            client: http::Client::without_cookies(config)?,
            license_token: String::new(),
            bf_secret: config.bf_secret,
            repeat_mode: RepeatMode::default(),
            shuffle: false,
            event_tx: None,
            playing_since: Duration::ZERO,
            current_rx: None,
            preload_rx: None,
            _stream: stream,
            sink,
            sources,
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

    fn go_next(&mut self) {
        let repeat_mode = self.repeat_mode();
        if repeat_mode != RepeatMode::One {
            let next = self.position.saturating_add(1);
            if next < self.queue.len() {
                // Move to the next track.
                self.position = next;
                self.notify_play();
            } else {
                // Reached the end of the queue: rewind to the beginning.
                if repeat_mode != RepeatMode::All {
                    self.pause();
                };
                self.position = 0;
            }
        }
    }

    async fn load_track(
        &mut self,
        position: usize,
    ) -> Result<Option<std::sync::mpsc::Receiver<()>>> {
        if let Some(track) = self.queue.get_mut(position) {
            match track.state() {
                State::Pending => {
                    // Start downloading the track.
                    let medium = track
                        .get_medium(&self.client, self.audio_quality, self.license_token.clone())
                        .await?;
                    track
                        .start_download(&self.client, &medium)
                        .await
                        .map(|()| None)
                }
                State::Buffered | State::Complete => {
                    // Append the track to the sink.
                    // TODO : don't bail out on error
                    let decryptor = Decrypt::new(track, &self.bf_secret)?;
                    let decoder = match track.quality() {
                        AudioQuality::Lossless => rodio::Decoder::new_flac(decryptor),
                        _ => rodio::Decoder::new_mp3(decryptor),
                    }?;

                    let rx = self.sources.append_with_signal(decoder);
                    Ok(Some(rx))
                }
                State::Starting => {
                    // Wait for the track to buffer.
                    Ok(None)
                }
            }
        } else {
            Err(Error::out_of_range(format!(
                "queue has no track at position {position}"
            )))
        }
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
            match self.current_rx.as_mut() {
                Some(current_rx) => {
                    // Check if the current track has finished playing.
                    if current_rx.try_recv().is_ok() {
                        // Save the point in time when the track finished playing.
                        self.playing_since = self.playing_since.saturating_add(self.sink.get_pos());

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
                        if self.queue.len() > next_position {
                            match self.load_track(next_position).await {
                                Ok(rx) => {
                                    self.preload_rx = rx;
                                }
                                Err(e) => {
                                    error!("failed to preload track: {e}");
                                }
                            }
                        }
                    }
                }

                None => {
                    if self.track().is_some() {
                        match self.load_track(self.position).await {
                            Ok(rx) => {
                                if let Some(rx) = rx {
                                    self.current_rx = Some(rx);
                                    self.notify_play();
                                }
                            }
                            Err(e) => {
                                error!("failed to load track: {e}");
                                self.go_next();
                            }
                        };
                    }
                }
            }

            // Yield to the runtime to allow other tasks to run.
            tokio::task::yield_now().await;
        }
    }

    fn notify_play(&self) {
        if self.is_playing() {
            if let Some(track) = self.track() {
                if let Some(event_tx) = &self.event_tx {
                    if let Err(e) = event_tx.send(Event::Play(track.id())) {
                        error!("failed to send track changed event: {e}");
                    }
                }
            }
        }
    }

    pub fn register(&mut self, event_tx: tokio::sync::mpsc::UnboundedSender<Event>) {
        self.event_tx = Some(event_tx);
    }

    pub fn play(&mut self) {
        debug!("starting playback");
        self.sink.play();

        // Playback reporting happens every time a track starts playing or is unpaused.
        self.notify_play();
    }

    pub fn pause(&mut self) {
        debug!("pausing playback");
        self.sink.pause();
    }

    #[must_use]
    pub fn is_playing(&self) -> bool {
        !self.sink.is_paused() && !self.sink.empty()
    }

    pub fn set_playing(&mut self, should_play: bool) {
        if self.is_playing() {
            if !should_play {
                self.pause();
            }
        } else if should_play {
            self.play();
        }
    }

    #[must_use]
    pub fn track(&self) -> Option<&Track> {
        self.queue.get(self.position)
    }

    pub fn set_queue(&mut self, tracks: Vec<Track>) {
        self.clear();
        self.position = 0;
        self.queue = tracks;
    }

    /// Sets the playlist position.
    ///
    /// # Errors
    ///
    /// Returns an error if the position is out of range.
    pub fn set_position(&mut self, position: usize) -> Result<()> {
        if self.position == position {
            return Ok(());
        }

        let len = self.queue.len();
        if position >= len {
            return Err(Error::out_of_range(format!(
                "invalid position {position} for queue with {len} items",
            )));
        }

        debug!("setting playlist position to {position}");

        // While skipping to another track, cancel the download of the current track if it is
        // still pending. Also cancel the download of the next track, unless it is the track that
        // we are skipping to.
        if let Some(track) = self.queue.get_mut(self.position) {
            track.cancel_download();
        }
        let next_position = self.position.saturating_add(1);
        if position != next_position {
            if let Some(next_track) = self.queue.get_mut(next_position) {
                next_track.cancel_download();
            }
        }

        self.clear();
        self.position = position;

        Ok(())
    }

    pub fn clear(&mut self) {
        // Don't just clear the sink, because that would stop the playback. The following code
        // works around that by creating a new, empty queue of sources and skipping to it.
        let (sources, output) = rodio::queue::queue(true);
        self.sink.append(output);
        self.sink.skip_one();
        self.sources = sources;

        self.playing_since = Duration::ZERO;
        self.current_rx = None;
        self.preload_rx = None;
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

        if repeat_mode == RepeatMode::One {
            // This only clears the preloaded track.
            self.sources.clear();
            self.preload_rx = None;
        }
    }

    #[must_use]
    pub fn volume(&self) -> Percentage {
        let ratio = self.sink.volume();
        Percentage::from_ratio_f32(ratio)
    }

    pub fn set_volume(&mut self, volume: Percentage) {
        if volume == self.volume() {
            return;
        }

        debug!("setting volume to {volume}");
        let ratio = volume.as_ratio_f32();
        self.sink.set_volume(ratio);
    }

    #[must_use]
    pub fn progress(&self) -> Option<Percentage> {
        // The progress is the difference between the current position of the sink, which is the
        // total duration played, and the time the current track started playing.
        let progress = self.sink.get_pos().saturating_sub(self.playing_since);

        self.track().map(|track| {
            let ratio = progress.div_duration_f32(track.duration());
            Percentage::from_ratio_f32(ratio)
        })
    }

    /// # Errors
    ///
    /// Will return `Err` if:
    /// - there is no active track
    pub fn set_progress(&mut self, progress: Percentage) -> Result<()> {
        if let Some(track) = self.track() {
            debug!("setting track progress to {progress}");
            let progress = progress.as_ratio_f32();

            // The proper way of checking for floating point equality.
            if (progress - 1.0).abs() <= f32::EPSILON {
                // Setting the progress to 1.0 is equivalent to skipping to the next track.
                // This prevents `UnexpectedEof` when seeking to the end of the track.
                self.clear();
                self.go_next();
            } else {
                let progress = track.duration().mul_f32(progress);
                self.sink.try_seek(progress)?;

                // Allow the sink to catch up with the new position, so the next call to `get_pos`
                // will return the correct value. 5 ms is what Rodio uses as periodic access.
                std::thread::sleep(Duration::from_millis(5));

                // Reset the playing time to zero, as the sink will now reset it also.
                self.playing_since = Duration::ZERO;
            }
        }

        Ok(())
    }

    #[must_use]
    pub fn position(&self) -> usize {
        self.position
    }
}
