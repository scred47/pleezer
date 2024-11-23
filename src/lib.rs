//! TODO: add top-level docs
#![deny(clippy::all)]
#![warn(clippy::pedantic)]

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

#[allow(clippy::enum_glob_use)]
pub mod error;
