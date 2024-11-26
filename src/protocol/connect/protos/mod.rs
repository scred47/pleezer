//! Protocol Buffer definitions for the [Deezer Connect][Connect] protocol.
//!
//! This module contains the auto-generated Rust code from Protocol Buffer definitions
//! that describe the message formats used in Deezer Connect communication.
//!
//! # Structure
//!
//! The Protocol Buffer definitions are compiled into Rust code at build time
//! and included in this module. The generated code provides:
//! * Message type definitions
//! * Serialization/deserialization implementations
//! * Helper methods for working with the protocol messages
//!
//! # Generation
//!
//! The Rust code is generated during the build process using:
//! * The `protobuf-codegen` Protocol Buffer compiler
//! * Source `.proto` files in the project's `protos` directory
//! * Build configuration in `build.rs`
//!
//! # Usage
//!
//! The generated types can be used directly from this module:
//! ```rust,no_run
//! use connect::protos::SomeGeneratedMessage;
//!
//! let msg = SomeGeneratedMessage::new();
//! ```
//!
//! [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079

// Allow pedantic lints in generated code
#![allow(clippy::pedantic)]

// Include the generated Rust code from Protocol Buffers
include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
