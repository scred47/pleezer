//! Fast, non-cryptographic random number generation.
//!
//! This module provides thread-local access to a fast but non-secure RNG.
//! Used for operations where cryptographic security is not required:
//! * Jitter for retry delays
//! * Client ID generation
//! * Request ID generation
//!
//! # Example
//!
//! ```rust
//! use pleezer::rand::with_rng;
//!
//! // Generate random delay between 5 and 6 seconds
//! let ms = with_rng(|rng| rng.gen_range(5_000..6_000));
//! ```
//!
//! # Security Note
//!
//! The RNG used here (`SmallRng`) is NOT cryptographically secure.
//! Do not use for security-sensitive purposes like token generation.

use rand::{rngs::SmallRng, SeedableRng};
use std::cell::RefCell;

// TODO : see if we can make this more like rand's `thread_rng()`.

thread_local! {
    /// Thread-local fast RNG instance.
    ///
    /// Uses `SmallRng` for speed over security:
    /// * Non-cryptographic algorithm
    /// * Optimized for performance
    /// * Seeded from system entropy once
    pub(crate) static SMALL_RNG: RefCell<SmallRng> = RefCell::new(SmallRng::from_entropy());
}

/// Access the thread-local RNG with a closure.
///
/// Provides mutable access to the pre-initialized RNG instance.
/// The RNG is fast but NOT cryptographically secure.
///
/// # Arguments
///
/// * `f` - Closure that receives mutable RNG reference
///
/// # Examples
///
/// ```rust
/// use rand::Rng;
///
/// // Generate random number
/// let n = with_rng(|rng| rng.gen_range(0..100));
///
/// // Multiple operations
/// let values = with_rng(|rng| {
///     let x = rng.gen::<f32>();
///     let y = rng.gen::<f32>();
///     (x, y)
/// });
/// ```
pub fn with_rng<F, R>(f: F) -> R
where
    F: FnOnce(&mut SmallRng) -> R,
{
    SMALL_RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        f(&mut rng)
    })
}
