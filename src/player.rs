use std::time::{Duration, Instant};

use thiserror::Error;
use uuid::Uuid;

use crate::protocol::connect::{queue, ListItem, Percentage, Quality, Repeat};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
#[error("{0}")]
pub struct Error(String);

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub struct State {
    pub track: ListItem,
    pub quality: Quality,
    pub duration: Duration,
    pub buffered: Duration,
    pub progress: Percentage,
    pub volume: Percentage,
    pub is_playing: bool,
    pub is_shuffle: bool,
    pub repeat_mode: Repeat,

    instant: Instant,
}

pub trait Connect {
    fn queue(&self) -> Option<queue::List>;
    fn set_queue(&mut self, queue: queue::List);

    fn state(&self) -> Option<State>;
    fn set_state(
        &mut self,
        queue_id: Uuid,
        track: Option<ListItem>,
        progress: Option<Percentage>,
        should_play: Option<bool>,
        set_shuffle: Option<bool>,
        set_repeat: Option<Repeat>,
        set_volume: Option<Percentage>,
    ) -> Result<()>;

    fn stop(&mut self);
}

#[derive(Debug)]
pub struct Player {
    queue: Option<queue::List>,
    state: Option<State>,
}

impl Player {
    pub fn new() -> Self {
        Self {
            queue: None,
            state: None,
        }
    }

    fn progress(&self) -> Option<Percentage> {
        self.state.as_ref().map(|state| {
            let elapsed = state.instant.elapsed().as_secs_f64();
            let progress = Percentage::from_ratio(elapsed / state.duration.as_secs_f64());
            trace!("{progress}");
            progress
        })
    }

    fn volume(&self) -> Percentage {
        self.state
            .as_ref()
            .map_or(Percentage::from_ratio(0.5), |state| state.volume)
    }

    fn quality(&self) -> Option<Quality> {
        Some(Quality::Lossless)
    }

    fn duration(&self) -> Option<Duration> {
        Some(Duration::from_secs(348))
    }

    fn buffered(&self) -> Option<Duration> {
        Some(Duration::from_secs(348))
    }
}

impl Connect for Player {
    fn stop(&mut self) {
        // is_playing = false
    }

    fn queue(&self) -> Option<queue::List> {
        self.queue.clone()
    }

    fn set_queue(&mut self, queue: queue::List) {
        let mut queue = queue;
        queue.id = Uuid::new_v4().to_string();
        queue.timestamp = 0;

        for context in &mut queue.contexts {
            if let Some(container) = context.container.as_mut() {
                container.typ =
                    ::protobuf::EnumOrUnknown::new(queue::ContainerType::CONTAINER_TYPE_DEFAULT);
            }
        }

        self.queue = Some(queue);
        self.state = None;
    }

    fn state(&self) -> Option<State> {
        let mut state = self.state.clone();
        if let Some(state) = &mut state {
            state.is_playing = true;
            state.progress = self.progress().unwrap();
        }
        state
    }

    fn set_state(
        &mut self,
        _queue_id: Uuid,
        track: Option<ListItem>,
        progress: Option<Percentage>,
        should_play: Option<bool>,
        set_shuffle: Option<bool>,
        set_repeat_mode: Option<Repeat>,
        set_volume: Option<Percentage>,
    ) -> Result<()> {
        let previous_state = self.state.clone();

        // TODO: check whether queue matches

        self.state = Some(State {
            track: track
                .or(previous_state.as_ref().map(|state| state.track))
                .ok_or_else(|| Error("should have initial track".to_string()))?,
            quality: self
                .quality()
                .ok_or_else(|| Error("should have some quality".to_string()))?,
            duration: self
                .duration()
                .ok_or_else(|| Error("should have some duration".to_string()))?,
            buffered: self
                .buffered()
                .ok_or_else(|| Error("should have some buffered".to_string()))?,
            progress: progress
                .or(self.progress())
                .unwrap_or_default(),
            is_playing: should_play
                .or(previous_state.as_ref().map(|state| state.is_playing))
                .unwrap_or_default(),
            is_shuffle: set_shuffle
                .or(previous_state.as_ref().map(|state| state.is_shuffle))
                .unwrap_or_default(),
            repeat_mode: set_repeat_mode
                .or(previous_state.as_ref().map(|state| state.repeat_mode))
                .unwrap_or_default(),
            volume: set_volume.unwrap_or_else(|| self.volume()),
            instant: previous_state
                .as_ref()
                .map_or(Instant::now(), |state| state.instant),
        });

        Ok(())
    }
}
