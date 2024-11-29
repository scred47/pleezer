//! Events emitted during Deezer Connect playback and remote control.
//!
//! This module defines the events that can be triggered during playback
//! and remote control operations. These events can be used to:
//! * Monitor playback state changes
//! * Track remote control connections
//! * React to track changes
//!
//! # Example
//!
//! ```rust
//! use pleezer::events::Event;
//!
//! fn handle_event(event: Event) {
//!     match event {
//!         Event::Play => println!("Playback started"),
//!         Event::TrackChanged => println!("New track playing"),
//!         Event::Connected => println!("Remote control connected"),
//!         // ... handle other events ...
//!     }
//! }
//! ```

/// Events that can be emitted by the Deezer Connect player or remote.
///
/// These events represent significant state changes in playback
/// or remote control status.
///
/// # Events
///
/// Events fall into two categories:
///
/// Playback Events:
/// * [`Play`](Self::Play) - Playback starts
/// * [`Pause`](Self::Pause) - Playback pauses
/// * [`TrackChanged`](Self::TrackChanged) - Current track changes
///
/// Connection Events:
/// * [`Connected`](Self::Connected) - Remote connects
/// * [`Disconnected`](Self::Disconnected) - Remote disconnects
///
/// # Example
///
/// ```rust
/// use pleezer::events::Event;
///
/// // Events can be copied and compared
/// let event = Event::Play;
/// assert_eq!(event, Event::Play);
/// assert_ne!(event, Event::Pause);
///
/// // Events can be used in match expressions
/// let message = match event {
///     Event::Play => "Started playing",
///     Event::Pause => "Paused playback",
///     _ => "Other event",
/// };
/// ```
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Event {
    /// Playback has started.
    ///
    /// Emitted when a track begins playing, either from a paused
    /// state or when starting a new track.
    Play,

    /// Playback has paused.
    ///
    /// Emitted when playback is suspended but can be resumed
    /// from the current position.
    Pause,

    /// Current track has changed.
    ///
    /// Emitted when switching to a different track, whether through
    /// manual selection, automatic progression, or remote control.
    TrackChanged,

    /// Remote control has connected.
    ///
    /// Emitted when a Deezer client establishes a remote control
    /// connection to this player.
    Connected,

    /// Remote control has disconnected.
    ///
    /// Emitted when a connected Deezer client ends its remote
    /// control session with this player.
    Disconnected,
}
