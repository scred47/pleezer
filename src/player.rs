use std::time::Duration;

use thiserror::Error;
use uuid::Uuid;

use crate::protocol::connect::{
    contents::{self, AudioQuality, RepeatMode},
    queue, Element, Percentage,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
#[error("{0}")]
pub struct Error(String);

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Track {
    pub element: Element,
    pub quality: AudioQuality,
    pub duration: Duration,
    pub buffered: Duration,
    pub position: Duration,
}

impl Track {
    pub fn progress(&self) -> Percentage {
        // TODO: replace with `Duration::div_duration_f64` once stabilized
        let position = self.position.as_secs_f64();
        if position > 0.0 {
            let duration = self.duration.as_secs_f64();
            Percentage::from_ratio(duration / position)
        } else {
            Percentage::from_ratio(0.0)
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Player {
    pub track: Option<Track>,
    pub queue: Option<queue::List>,
    pub playing: bool,
    pub repeat_mode: RepeatMode,
    pub shuffle: bool,

    // TODO : replace with Rodio volume
    volume: Percentage,
}

impl Player {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn play(&mut self) -> Result<()> {
        self.playing = true;
        Ok(())
    }

    pub fn stop(&mut self) -> Result<()> {
        self.playing = false;
        Ok(())
    }

    #[must_use]
    pub fn queue(&self) -> Option<queue::List> {
        self.queue.clone()
    }

    pub fn set_queue(&mut self, queue: queue::List) {
        self.queue = Some(queue);
    }

    #[must_use]
    pub fn track(&self) -> Option<Track> {
        self.track.clone()
    }

    pub fn skip_to(&self, position: usize) -> Option<Track> {
        todo!()
    }

    pub fn set_track(&mut self, track: Track) -> Result<()> {
        todo!()
    }

    pub fn set_shuffle(&mut self, shuffle: bool) {
        self.shuffle = shuffle;
    }

    #[must_use]
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }

    #[must_use]
    pub fn volume(&self) -> Percentage {
        self.volume
    }

    pub fn set_volume(&mut self, volume: Percentage) {
        self.volume = volume;
    }

    pub fn set_position(&mut self, progress: Percentage) -> Result<()> {
        let progress = progress.as_ratio();
        if progress < 0.0 || progress > 1.0 {
            return Err(Error(format!("position cannot be set to {progress}")));
        }

        if let Some(mut track) = self.track {
            if let Some(position) = track.duration.checked_mul(progress as u32) {
                track.position = position;
                Ok(())
            } else {
                Err(Error(format!(
                    "failed setting track with duration {:?} to position {progress}",
                    track.duration
                )))
            }
        } else {
            Err(Error(
                "position cannot be set without an active track".to_string(),
            ))
        }
    }

    pub fn load_track(&mut self, track: Track) -> Result<()> {
        // retrieve metadata from web url, not download (yet) ?
        Ok(())
    }

    pub fn set_state(
        &mut self,
        _queue_id: Uuid,
        element: Option<contents::Element>,
        progress: Option<Percentage>,
        should_play: Option<bool>,
        set_shuffle: Option<bool>,
        set_repeat_mode: Option<RepeatMode>,
        set_volume: Option<Percentage>,
    ) -> Result<()> {
        // TODO: check whether queue matches

        if let Some(element) = element {
            // TODO : move to load_track() or something
            debug!("setting track to {}", element);
            self.track = Some(Track {
                element,
                // TODO : get actual user audio quality
                quality: AudioQuality::Lossless,
                duration: Duration::from_secs(100),
                buffered: Duration::from_secs(100),
                position: Duration::from_secs(0),
            });
        }

        if let Some(progress) = progress {
            if let Some(mut track) = self.track {
                // TODO : make it seek()
                debug!("setting track position to {progress}");
                self.set_position(progress);
            } else {
                error!("cannot set track position without a track");
            }
        }

        if let Some(should_play) = should_play {
            if should_play {
                self.play();
            } else {
                self.stop();
            }
        }

        if let Some(shuffle) = set_shuffle {
            debug!("setting shuffle to {shuffle}");
            self.shuffle = shuffle;
        }

        if let Some(repeat_mode) = set_repeat_mode {
            debug!("setting repeat mode to {repeat_mode}");
            self.repeat_mode = repeat_mode;
        }

        if let Some(volume) = set_volume {
            debug!("setting volume to {volume}");
            self.volume = volume;
        }

        Ok(())
    }
}
