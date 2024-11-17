use crate::track::TrackId;

#[derive(Clone, Debug)]
pub enum Event {
    Play(TrackId),
    // TODO - proposals:
    // QueueChanged(Queue),
    // PlayingChanged(bool),
    // ShuffleChanged(bool),
    // RepeatModeChanged(RepeatMode),
    // VolumeChanged(Percentage),
    // ProgressChanged(Percentage),
}
