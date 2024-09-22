use crate::track::Track;

#[derive(Clone, Debug, PartialEq, PartialOrd)]
pub enum Event {
    TrackChanged(Track),
    // TODO - proposals:
    // QueueChanged(Queue),
    // PlayingChanged(bool),
    // ShuffleChanged(bool),
    // RepeatModeChanged(RepeatMode),
    // VolumeChanged(Percentage),
    // ProgressChanged(Percentage),
}
