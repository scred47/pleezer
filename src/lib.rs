//! TODO: add docs
#![deny(clippy::all)]
#![warn(clippy::pedantic)]

#[macro_use]
extern crate log;

pub mod arl;
pub mod audio;
pub mod config;
pub mod connect;
pub mod gateway;
pub mod session;
pub mod token;
pub mod util;
