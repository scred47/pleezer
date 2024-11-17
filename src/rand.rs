use rand::{rngs::SmallRng, SeedableRng};
use std::cell::RefCell;

// TODO : see if we can make this more like rand's `thread_rng()`.

thread_local! {
    /// A thread-local random number generator that is insecure but fast.
    pub(crate) static SMALL_RNG: RefCell<SmallRng> = RefCell::new(SmallRng::from_entropy());
}

/// Access a pre-initialized random number generator that is insecure but fast.
pub fn with_rng<F, R>(f: F) -> R
where
    F: FnOnce(&mut SmallRng) -> R,
{
    SMALL_RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        f(&mut rng)
    })
}
