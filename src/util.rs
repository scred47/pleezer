use std::time;

/// Get the current system time in epoch format.
///
/// # Returns
///
/// Current system time in seconds from epoch.
///
/// # Panics
///
/// Panics if the system time is before epoch.
///
/// # Examples
///
/// ```rust
/// assert!(unix_timestamp() - std::time::UNIX_EPOCH > 0);
/// ```
pub fn now_from_epoch() -> u64 {
    time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .expect("system time is before epoch")
        .as_secs()
}
