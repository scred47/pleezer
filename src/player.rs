use crate::{
    error::{Error, Result},
    protocol::{
        connect::{
            contents::{self, RepeatMode},
            Percentage,
        },
        gateway::Queue,
    },
    track::Track,
};

#[derive(Clone, Debug, Default)]
pub struct Player {
    track: Option<Track>,
    queue: Option<Queue>,
    playing: bool,
    repeat_mode: RepeatMode,
    shuffle: bool,
}

impl Player {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
        }
    }

    #[must_use]
    pub fn queue(&self) -> Option<&Queue> {
        self.queue.as_ref()
    }

    pub fn set_queue(&mut self, queue: Queue) {
        self.queue = Some(queue);
    }

    #[must_use]
    pub fn track(&self) -> Option<&Track> {
        self.track.as_ref()
    }

    #[expect(clippy::must_use_candidate)]
    pub fn skip_to(&self, _position: usize) -> Option<&Track> {
        todo!()
    }

    pub fn set_item(&mut self, item: contents::QueueItem) {
        debug!("setting track to {}", item);
        self.track = Some(item.into());
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
    pub fn progress(&self) -> Option<Percentage> {
        // TODO: get TrackPosition from Rodio
        Some(Percentage::default())
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

        if self.track.is_some() {
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

    /// # Errors
    ///
    /// TODO
    pub fn load_track(&mut self, _track: &Track) -> Result<()> {
        // retrieve metadata from web url, not download (yet) ?
        Ok(())
    }
}
