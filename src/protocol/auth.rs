//! OAuth authentication response types.
//!
//! This module contains types for handling OAuth authentication responses
//! from Deezer's login endpoints. These responses include:
//! * Access tokens for API access
//! * Token expiration information (currently unused by Deezer)
//!
//! # Example Response
//!
//! ```json
//! {
//!     "access_token": "secret_token",
//!     "expire": 0,
//!     "expires": 0
//! }
//! ```
//!
//! # Note
//!
//! While the response includes expiration fields, Deezer currently returns 0 for both
//! `expire` and `expires`. Token expiration is handled through other mechanisms.
//! These fields are preserved for protocol compatibility.

use std::time::{Duration, SystemTime};

use serde::Deserialize;
use serde_with::{formats::Flexible, serde_as, DurationSeconds, TimestampSeconds};
use veil::Redact;

/// User authentication data from OAuth login.
///
/// Contains the access token and timing information needed
/// to maintain an authenticated session.
#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Redact, Hash)]
pub struct User {
    /// OAuth access token for API authentication
    #[redact]
    pub access_token: String,

    /// How long the token remains valid
    ///
    /// Note: Currently always returns 0 in Deezer responses
    #[serde_as(as = "Option<DurationSeconds<u64, Flexible>>")]
    pub expire: Option<Duration>,

    /// When the token will expire
    ///
    /// Note: Currently always returns 0 in Deezer responses
    #[serde_as(as = "Option<TimestampSeconds<i64, Flexible>>")]
    pub expires: Option<SystemTime>,
}
