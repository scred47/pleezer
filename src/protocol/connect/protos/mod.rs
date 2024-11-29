//! Protocol Buffer definitions for queue management in Deezer Connect.
//!
//! This module contains auto-generated Rust code from Protocol Buffer definitions,
//! primarily handling queue data structures in the Deezer Connect protocol.
//!
//! # Queue Data Format
//!
//! Queue messages use Protocol Buffers for efficient serialization and
//! versioning. The data is:
//! 1. Serialized to Protocol Buffer format
//! 2. DEFLATE compressed
//! 3. Base64 encoded for transmission
//!
//! Example wire format:
//! ```json
//! {
//!     "messageId": "msg123",
//!     "messageType": "publishQueue",
//!     "protocolVersion": "com.deezer.remote.queue.proto1",
//!     "payload": "base64-encoded-deflated-protobuf"
//! }
//! ```
//!
//! # Generated Types
//!
//! Key message types include:
//! * `queue::List` - Complete queue contents
//! * `queue::Track` - Individual track information
//! * `queue::Order` - Track ordering/shuffle state
//!
//! # Usage Example
//!
//! ```rust,no_run
//! use connect::protos::queue;
//!
//! // Create a queue publication message
//! let queue = queue::List {
//!     id: "queue123".to_string(),
//!     tracks: vec![/* track data */],
//!     tracks_order: vec![/* track positions */],
//!     // ... other fields ...
//! };
//!
//! // Serialize for transmission
//! let bytes = queue.write_to_bytes()?;
//! ```
//!
//! # Code Generation
//!
//! The Rust code is generated during build using:
//! * `protobuf-codegen` compiler
//! * `.proto` source files in `protos/`
//! * Build configuration in `build.rs`
//!
//! Note: The generated code allows pedantic lints to avoid
//! warnings from the auto-generated implementations.

// Allow pedantic lints in generated code
#![allow(clippy::pedantic)]

// Include the generated Rust code from Protocol Buffers
include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
