//! Headless streaming player for the Deezer Connect protocol.
//!
//! **pleezer** is a library and application that implements the Deezer Connect protocol,
//! enabling remote-controlled audio playback of Deezer content. It provides:
//!
//! # Core Features
//!
//! * **Remote Control**: Acts as a receiver for Deezer Connect, allowing control from
//!   official Deezer apps
//! * **Audio Playback**: High-quality audio streaming with gapless playback support
//! * **Format Support**: Handles MP3 and FLAC formats based on subscription level
//! * **Volume Normalization**: Optional audio leveling with configurable target gain
//!
//! # Architecture
//!
//! The library is organized into several key modules:
//!
//! * **Connection Management**
//!   - [`remote`]: Implements Deezer Connect protocol
//!   - [`gateway`]: Handles API authentication and requests
//!   - [`http`]: Manages HTTP connections and cookies
//!
//! * **Audio Processing**
//!   - [`player`]: Controls audio playback and queues
//!   - [`track`]: Manages track metadata and downloads
//!   - [`decrypt`]: Handles encrypted content
//!
//! * **Authentication**
//!   - [`arl`]: ARL token management
//!   - [`tokens`]: Session token handling
//!
//! * **Configuration**
//!   - [`config`]: Application settings
//!   - [`proxy`]: Network proxy support
//!
//! * **Protocol**
//!   - [`protocol`]: Deezer Connect message types
//!   - [`events`]: Event system for state changes
//!
//! * **Utilities**
//!   - [`util`]: General helper functions
//!   - [`rand`]: Random number generation
//!   - [`error`]: Error types and handling
//!
//! # Example
//!
//! ```rust,no_run
//! use pleezer::{config::Config, player::Player, remote::Client};
//!
//! async fn example() -> pleezer::error::Result<()> {
//!     // Create player with configuration
//!     let config = Config::new()?;
//!     let player = Player::new(&config, "").await?;
//!
//!     // Create and start client
//!     let mut client = Client::new(&config, player)?;
//!     client.start().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Protocol Documentation
//!
//! For details on the Deezer Connect protocol implementation, see the
//! [`protocol`] and [`remote`] modules.
//!
//! # Error Handling
//!
//! Errors are handled through the types in the [`error`] module, with
//! most functions returning [`Result`](error::Result).
//!
//! # Concurrency
//!
//! The library uses async/await for concurrency and is designed to work with
//! the Tokio async runtime. Most operations are asynchronous and can run
//! concurrently.

#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![doc(test(attr(ignore)))]

#[macro_use]
extern crate log;

pub mod arl;
pub mod config;
pub mod decrypt;
pub mod events;
pub mod gateway;
pub mod http;
pub mod player;
pub mod protocol;
pub mod proxy;
pub mod rand;
pub mod remote;
pub mod tokens;
pub mod track;
pub mod util;

#[expect(clippy::enum_glob_use)]
pub mod error;
