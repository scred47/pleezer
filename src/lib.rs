//! TODO: add top-level docs
#![deny(clippy::all)]
#![warn(clippy::pedantic)]
// TODO : add documentation
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]

#[macro_use]
extern crate log;

pub mod arl;
pub mod config;
pub mod gateway;
pub mod http;
pub mod player;
pub mod protocol;
pub mod remote;
pub mod tokens;
