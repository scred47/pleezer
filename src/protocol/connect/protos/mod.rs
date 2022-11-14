// Allow pedantic lints out of our control.
#![allow(clippy::pedantic)]

// Import the generated Rust protobufs.
include!(concat!(env!("OUT_DIR"), "/protos/mod.rs"));
