//! Authentication token management for Deezer users.
//!
//! This module provides functionality for managing user authentication tokens used in
//! Deezer Connect sessions. It handles token lifecycle management including storage,
//! expiration tracking, and validity checks.
//!
//! # Token Architecture
//!
//! The authentication system uses time-limited tokens that:
//! * Identify a specific user
//! * Grant access to Deezer services
//! * Expire after a set duration
//! * Must be refreshed periodically
//!
//! # Integration
//!
//! This module works closely with:
//! * [`gateway`](crate::gateway) - Handles token refresh and session management
//! * [`player`](crate::player) - Uses tokens for media access
//! * [`http`](crate::http) - Includes tokens in API requests
//!
//! # Token Lifecycle
//!
//! 1. Token Creation
//!    * Generated during login via [`gateway::Gateway`]
//!    * Contains user ID and expiration time
//!
//! 2. Token Usage
//!    * Used to authenticate API requests
//!    * Checked for expiration before use
//!    * Time-to-live monitored
//!
//! 3. Token Expiration
//!    * Tokens become invalid at expiration time
//!    * Must be refreshed via gateway
//!    * Expired tokens trigger re-authentication
//!
//! # Example
//!
//! ```rust
//! use pleezer::tokens::UserToken;
//! use std::time::{SystemTime, Duration};
//!
//! let token = UserToken {
//!     user_id: 123456789,
//!     token: "secret_token".to_string(),
//!     expires_at: SystemTime::now() + Duration::from_secs(3600),
//! };
//!
//! if !token.is_expired() {
//!     println!("Token valid for: {:?}", token.time_to_live());
//! }
//! ```

use std::{
    fmt,
    time::{Duration, SystemTime},
};

use crate::protocol::connect::UserId;

/// User authentication token for Deezer Connect sessions.
///
/// Contains the necessary information to authenticate a user and track token validity:
/// * User ID to identify the Deezer account
/// * Authentication token string
/// * Expiration timestamp
///
/// # Token Validity
///
/// Tokens have a limited lifetime and must be refreshed before expiration. Use
/// [`time_to_live()`](Self::time_to_live) to check remaining validity time and
/// [`is_expired()`](Self::is_expired) to check if immediate refresh is needed.
///
/// # Example
///
/// ```rust
/// use pleezer::tokens::UserToken;
/// use std::time::{SystemTime, Duration};
///
/// let token = UserToken {
///     user_id: 123456789,
///     token: "secret_token".to_string(),
///     expires_at: SystemTime::now() + Duration::from_secs(3600),
/// };
///
/// // Check if token needs refresh
/// if token.is_expired() {
///     println!("Token expired, needs refresh");
/// } else {
///     println!("Token valid for {:?}", token.time_to_live());
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserToken {
    /// Deezer user ID associated with this token.
    pub user_id: UserId,

    /// Authentication token string.
    pub token: String,

    /// Timestamp when this token expires.
    pub expires_at: SystemTime,
}

impl UserToken {
    /// Returns the remaining validity time of the token.
    ///
    /// Calculates how long until the token expires by comparing the expiration
    /// timestamp with the current system time.
    ///
    /// # Returns
    ///
    /// * If the token is still valid: Duration until expiration
    /// * If the token is expired: Zero duration
    ///
    /// # Example
    ///
    /// ```rust
    /// use pleezer::tokens::UserToken;
    /// use std::time::{SystemTime, Duration};
    ///
    /// let token = UserToken {
    ///     user_id: 123456789,
    ///     token: "secret_token".to_string(),
    ///     expires_at: SystemTime::now() + Duration::from_secs(3600),
    /// };
    ///
    /// println!("Token valid for: {:?}", token.time_to_live());
    /// ```
    #[must_use]
    pub fn time_to_live(&self) -> Duration {
        self.expires_at
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO)
    }

    /// Checks if the token has expired.
    ///
    /// A token is considered expired if the current system time is equal to
    /// or later than the expiration timestamp.
    ///
    /// # Returns
    ///
    /// * `true` if the token has expired
    /// * `false` if the token is still valid
    ///
    /// # Example
    ///
    /// ```rust
    /// use pleezer::tokens::UserToken;
    /// use std::time::{SystemTime, Duration};
    ///
    /// let token = UserToken {
    ///     user_id: 123456789,
    ///     token: "secret_token".to_string(),
    ///     expires_at: SystemTime::now() + Duration::from_secs(3600),
    /// };
    ///
    /// if token.is_expired() {
    ///     println!("Token needs refresh");
    /// }
    /// ```
    #[must_use]
    pub fn is_expired(&self) -> bool {
        SystemTime::now() >= self.expires_at
    }
}

impl fmt::Display for UserToken {
    /// Formats the token as a string, returning just the token value.
    ///
    /// This implementation is used when the token needs to be included in
    /// request headers or other contexts where only the token string is needed.
    ///
    /// # Example
    ///
    /// ```rust
    /// use pleezer::tokens::UserToken;
    /// use std::time::{SystemTime, Duration};
    ///
    /// let token = UserToken {
    ///     user_id: 123456789,
    ///     token: "secret_token".to_string(),
    ///     expires_at: SystemTime::now() + Duration::from_secs(3600),
    /// };
    ///
    /// assert_eq!(token.to_string(), "secret_token");
    /// ```
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.token)
    }
}
