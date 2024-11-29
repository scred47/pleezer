//! Protocol types and structures for Deezer services.
//!
//! This module contains the core data types used across different Deezer
//! protocols and APIs:
//!
//! * [`connect`] - Deezer Connect protocol for remote playback control
//! * [`gateway`] - Gateway API for user data and authentication
//! * [`media`] - Media streaming and track access

pub mod connect;
pub mod gateway;
pub mod media;
