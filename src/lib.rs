//! TODO: add top-level docs
#![deny(clippy::all)]
#![warn(clippy::pedantic)]

#[macro_use]
extern crate log;

pub mod arl;
pub mod config;
pub mod error;
pub mod gateway;
pub mod http;
pub mod player;
pub mod protocol;
pub mod remote;
pub mod tokens;
pub mod track;
