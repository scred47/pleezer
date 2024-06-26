use std::{num::NonZeroU64, time::Duration};

use thiserror::Error;

use crate::protocol::connect::{
    contents::{self, AudioQuality, RepeatMode},
    queue, Element, Percentage,
};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
#[error("{0}")]
pub struct Error(String);

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct Track {
    // TODO : improve visibility
    pub element: Element,
    pub quality: AudioQuality,
    pub duration: Duration,
    pub buffered: Duration,
    pub progress: Percentage,
}

impl Track {
    #[must_use]
    pub fn progress(&self) -> Percentage {
        // // TODO: replace with `Duration::div_duration_f64` once stabilized
        // let position = self.position.as_secs_f64();
        // if position > 0.0 {
        //     let duration = self.duration.as_secs_f64();
        //     Percentage::from_ratio(duration / position)
        // } else {
        //     Percentage::from_ratio(0.0)
        // }
        self.progress
    }

    #[must_use]
    pub fn id(&self) -> NonZeroU64 {
        self.element.track_id
    }
}

#[derive(Clone, Debug, Default)]
pub struct Player {
    track: Option<Track>,
    queue: Option<queue::List>,
    playing: bool,
    repeat_mode: RepeatMode,
    shuffle: bool,

    // TODO : replace with Rodio volume
    volume: Percentage,
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

    pub fn playing(&self) -> bool {
        self.playing
    }

    pub fn set_playing(&mut self, should_play: bool) {
        if self.playing {
            if !should_play {
                self.stop();
            }
        } else {
            if should_play {
                self.play();
            }
        }
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

    pub fn skip_to(&self, _position: usize) -> Option<Track> {
        todo!()
    }

    pub fn set_element(&mut self, element: contents::Element) {
        debug!("setting track to {}", element);

        self.track = Some(Track {
            element,
            // TODO : get actual user audio quality
            quality: AudioQuality::Lossless,
            duration: Duration::from_secs(100),
            buffered: Duration::from_secs(100),
            progress: Percentage::from_ratio(0.0),
        });
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
        self.volume
    }

    pub fn set_volume(&mut self, volume: Percentage) {
        debug!("setting volume to {volume}");
        self.volume = volume;
    }

    #[must_use]
    pub fn progress(&self) -> Option<Percentage> {
        self.track.as_ref().map(|track| track.progress())
    }

    pub fn set_progress(&mut self, progress: Percentage) -> Result<()> {
        if !(0.0..=1.0).contains(&progress.as_ratio()) {
            return Err(Error(format!("progress cannot be set to {progress}")));
        }

        if let Some(ref mut track) = &mut self.track {
            debug!("setting track progress to {progress}");
            // OK to multiply unchecked, because `progress` is clamped above.
            //track.position = track.duration.mul_f64(position);
            track.progress = progress;
            Ok(())
        } else {
            Err(Error(
                "position cannot be set without an active track".to_string(),
            ))
        }
    }

    pub fn load_track(&mut self, _track: Track) -> Result<()> {
        // retrieve metadata from web url, not download (yet) ?
        Ok(())
    }
}
