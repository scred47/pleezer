//! ARL deserialization for Deezer gateway responses.
//!
//! Provides a type for deserializing Authentication Reference Links (ARLs)
//! from the Deezer gateway API responses, with built-in redaction for security.
//!
//! # Security
//!
//! ARLs in responses are automatically redacted in debug output to prevent
//! accidental credential exposure.
//!
//! # Example
//!
//! ```rust
//! use deezer::gateway::{Arl, Response};
//!
//! // Parse gateway response
//! let response: Response<Arl> = serde_json::from_str(json)?;
//!
//! // Token is redacted in debug output
//! println!("{:?}", response);  // Prints: Response { results: Arl("REDACTED") }
//! ```

use serde::Deserialize;
use veil::Redact;

use super::Method;

impl Method for Arl {
    /// Gateway method name for retrieving an Authentication Reference Link.
    ///
    /// This endpoint returns a new or refreshed ARL token for authentication.
    /// The method name follows Deezer's dot-notation format:
    /// - `user`: The API domain
    /// - `getArl`: The specific operation
    ///
    /// # API Response
    ///
    /// Returns a response containing the ARL token:
    /// ```json
    /// {
    ///     "error": {},
    ///     "results": {
    ///         "arl": "abcdef123456..."  // Actual token is much longer
    ///     }
    /// }
    /// ```
    ///
    /// # Security Note
    ///
    /// Access to this endpoint should be restricted as it provides
    /// authentication credentials.
    const METHOD: &'static str = "user.getArl";
}

/// Authentication Reference Link for Deezer services.
///
/// This type wraps an ARL token string, providing:
/// * Secure handling (redaction, constant-time comparison)
/// * Serialization support
/// * Type safety
///
/// # Security Notes
///
/// ARLs should be treated as sensitive credentials:
/// * Store securely
/// * Never log or display
/// * Protect from unauthorized access
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Redact, Hash)]
#[redact(all)]
pub struct Arl(pub String);
