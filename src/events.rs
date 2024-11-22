/// Events that can be emitted by the Deezer Connect player or remote.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// Event emitted when the player has started playing a track.
    Play,

    /// Event emitted when the player has paused a track.
    Pause,

    /// Event emitted when the player has changed the track.
    TrackChanged,

    /// Event emitted when a remote control has connected.
    Connected,

    /// Event emitted when a remote control has disconnected.
    Disconnected,
}
