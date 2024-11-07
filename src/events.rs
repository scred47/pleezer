use std::num::NonZeroU64;

#[derive(Clone, Debug)]
pub enum Event {
    Play(NonZeroU64),
    // TODO - proposals:
    // QueueChanged(Queue),
    // PlayingChanged(bool),
    // ShuffleChanged(bool),
    // RepeatModeChanged(RepeatMode),
    // VolumeChanged(Percentage),
    // ProgressChanged(Percentage),
}
