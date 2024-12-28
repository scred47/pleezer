//! Core types and functionality for the Deezer Connect protocol.
//!
//! This module provides a Rust implementation of Deezer's Connect protocol,
//! which enables remote control and synchronization between Deezer-enabled devices.
//!
//! # Protocol Structure
//!
//! The protocol is built around several key concepts:
//!
//! * **Channels** ([`channel`]): Define message routing and types
//!   - User-to-user communication paths
//!   - Message type identification and routing
//!   - Subscription management and lifecycle
//!
//! * **Messages** ([`messages`], [`contents`]): Handle various operations
//!   - Playback control and status updates
//!   - Device discovery and connection management
//!   - Queue management and synchronization
//!   - Stream activity reporting
//!
//! * **Streaming** ([`stream`]): Report playback activity
//!   - Track playback monitoring and progress
//!   - User activity and state tracking
//!   - Quality and performance metrics
//!
//! * **Queue Management** ([`protos`]): Handle playback queues
//!   - Queue content updates and sync
//!   - Protocol buffer serialization/deserialization
//!   - State management
//!
//! # Example
//!
//! ```rust
//! use deezer::{Channel, Contents, DeviceId, Headers, Ident, Message};
//!
//! // Create a playback control message
//! let msg = Message::Send {
//!     channel: Channel::new(Ident::RemoteCommand),
//!     contents: Contents {
//!         ident: Ident::RemoteCommand,
//!         headers: Headers {
//!             from: DeviceId::default(),
//!             destination: None,
//!         },
//!         body: /* message-specific content */,
//!     },
//! };
//! ```
//!
//! # Architecture
//!
//! The implementation uses a layered approach:
//! * High-level message types ([`Message`], [`Contents`]) for application use
//! * Channel-based routing and subscriptions ([`Channel`], [`Ident`])
//! * Wire format serialization for protocol compatibility
//! * Protocol buffer handling for complex data structures

pub mod channel;
pub mod contents;
pub mod messages;
pub mod protos;
pub mod stream;

pub use channel::{Channel, Ident, UserId};
pub use contents::{
    AudioQuality, Body, Contents, DeviceId, DeviceType, Headers, Percentage, QueueItem, RepeatMode,
    Status,
};
pub use messages::Message;
pub use protos::queue;
