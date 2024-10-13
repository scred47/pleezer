use std::{
    collections::{HashMap, VecDeque},
    num::NonZeroU64,
};

use tokio::sync::oneshot;

use crate::{
    config::Config,
    error::{Error, Result},
    events::Event,
    http,
    protocol::{
        connect::{
            contents::{self, AudioQuality, RepeatMode},
            Percentage,
        },
        gateway,
    },
    track::Track,
};

///
struct QueueItem {
    track: Track,
    download: Option<Task>,
}

struct Task {
    handle: tokio::task::JoinHandle<()>,
    abort_tx: oneshot::Sender<()>,
}

pub struct Player {
    /// The *preferred* audio quality. The actual quality may be lower if the
    /// track is not available in the preferred quality.
    pub audio_quality: AudioQuality,

    /// The license token to use for downloading tracks.
    pub license_token: String,

    /// The queue of tracks to play, a.k.a. the playlist.
    queue: Vec<QueueItem>,

    /// The current position in the queue.
    position: Option<usize>,

    /// The HTTP client to use for downloading tracks.
    client: http::Client,

    /// Whether the player is currently playing.
    playing: bool,

    /// The repeat mode.
    repeat_mode: RepeatMode,

    /// Whether the queue should be shuffled.
    shuffle: bool,

    /// The channel to send playback events to.
    event_tx: Option<tokio::sync::mpsc::UnboundedSender<Event>>,
}

struct Download {
    track: Track,
    task: Option<Task>,
}

impl Player {
    /// Creates a new `Player` with the given `Config`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if no HTTP client can be built from the `Config`.
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            queue: Vec::new(),
            position: None,
            audio_quality: AudioQuality::default(),
            client: http::Client::without_cookies(config)?,
            license_token: String::new(),
            playing: false,
            repeat_mode: RepeatMode::default(),
            shuffle: false,
            event_tx: None,
        })
    }

    pub async fn run(&mut self) -> Result<()> {
        loop {
            if self.queue.is_empty() {
                // TODO : prune downloads, drain playback buffer
            } else {
                // let track = self.queue().get(self.position).ok_or_else(|| {
                //     Error::out_of_range(format!(
                //         "invalid position {} for queue with {} items",
                //         self.position,
                //         self.queue.len()
                //     ))
                // })?;
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
        self.playing = true;
    }

    pub fn stop(&mut self) {
        debug!("stopping playback");
        self.playing = false;
    }

    #[must_use]
    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn set_playing(&mut self, should_play: bool) {
        if self.playing {
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
        let position = self.position?;
        self.queue.get(position).map(|item| &item.track)
    }

    // #[must_use]
    // pub fn queue(&self) -> &gateway::Queue {
    //     self.queue.as_ref()
    // }

    pub async fn set_queue(&mut self, queue: gateway::Queue) {
        self.queue = queue;

        // TODO : retain downloads that are also in the new queue
        self.abort_downloads();
    }

    /// Aborts all downloads in the queue.
    fn abort_downloads(&mut self) {
        for item in self.queue.iter_mut() {
            if let Some(download) = item.download.take() {
                let _ = download.abort_tx.send(());
            }
        }
    }

    pub fn set_item(&mut self, item: &contents::QueueItem) -> Result<()> {
        if let Some(local) = self.queue.get(item.position) {
            let track_id = local.track.id();
            if track_id != item.track_id {
                return Err(Error::invalid_argument(format!(
                    "track ID mismatch: expected {track_id}, got {} on position {}",
                    item.track_id, item.position
                )));
            }
        }

        debug!("setting track to {}", item);
        self.position = Some(item.position);

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
        // TODO: get volume from Rodio
        Percentage::default()
    }

    pub fn set_volume(&mut self, volume: Percentage) {
        debug!("setting volume to {volume}");
        // TODO: set volume in Rodio
    }

    #[must_use]
    pub fn progress(&self) -> Percentage {
        // TODO: get TrackPosition from Rodio
        Percentage::default()
    }

    /// # Errors
    ///
    /// Will return `Err` if:
    /// - there is no active track
    pub fn set_progress(&mut self, progress: Percentage) -> Result<()> {
        if !(0.0..=1.0).contains(&progress.as_ratio()) {
            return Err(Error::invalid_argument(format!(
                "progress cannot be set to {progress}"
            )));
        }

        if self.track().is_some() {
            debug!("setting track progress to {progress}");
            // TODO
            // OK to multiply unchecked, because `progress` is clamped above.
            //track.position = track.duration.mul_f64(position);
            Ok(())
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

impl Drop for Player {
    fn drop(&mut self) {
        self.abort_downloads();
    }
}
