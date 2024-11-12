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
pub mod remote;
pub mod tokens;
pub mod track;

#[allow(clippy::enum_glob_use)]
pub mod error;

use rand::{rngs::SmallRng, SeedableRng};
use std::cell::RefCell;

thread_local! {
    /// A thread-local random number generator that is insecure but fast.
    pub(crate) static SMALL_RNG: RefCell<SmallRng> = RefCell::new(SmallRng::from_entropy());
}

/// Access a pre-initialized random number generator that is insecure but fast.
pub fn with_small_rng<F, R>(f: F) -> R
where
    F: FnOnce(&mut SmallRng) -> R,
{
    SMALL_RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        f(&mut rng)
    })
}
