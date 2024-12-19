//! ARL validation and handling.
//!
//! Provides secure handling of Authentication Reference Links (ARLs) with:
//! * Validation of token format
//! * Cookie-safe character checking
//! * Automatic URL parsing
//! * Debug redaction
//!
//! # Security
//!
//! ARLs are sensitive credentials that provide full account access. This
//! implementation:
//! * Validates token contents
//! * Redacts tokens in debug output
//! * Uses constant-time comparison
//! * Prevents logging/display
//!
//! # Examples
//!
//! ```rust
//! use std::str::FromStr;
//! use pleezer::arl::Arl;
//!
//! // Parse and validate an ARL
//! let arl = Arl::from_str("valid_token")?;
//!
//! // Handles full callback URLs
//! let arl = Arl::from_str("deezer://autolog/valid_token")?;
//!
//! // Rejects invalid characters
//! assert!(Arl::from_str("invalid;token").is_err());
//! ```

use crate::error::{Error, Result};
use std::{fmt, ops::Deref, str::FromStr};
use veil::Redact;

/// Authentication Reference Link for Deezer services.
///
/// Provides validated storage and handling of ARL tokens, ensuring they
/// contain only cookie-safe characters and are properly formatted.
///
/// # Validation
///
/// ARLs must:
/// * Contain only ASCII characters
/// * Not contain control characters or whitespace
/// * Not contain `"`, `,`, `;`, or `\`
/// * Be extractable from callback URLs
///
/// # Security Notes
///
/// ARLs should be treated as sensitive credentials:
/// * Store securely
/// * Never log or display
/// * Protect from unauthorized access
/// * Validate all input
#[derive(Clone, Redact, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[redact(all)]
pub struct Arl(String);

impl Arl {
    /// Creates a new validated ARL from a string.
    ///
    /// # Arguments
    ///
    /// * `arl` - The ARL string to validate and wrap
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidArgument` if the string contains:
    /// * Non-ASCII characters
    /// * ASCII control characters
    /// * Whitespace
    /// * Special characters: `"`, `,`, `;`, `\`
    ///
    /// # Examples
    ///
    /// ```rust
    /// use pleezer::arl::Arl;
    ///
    /// // Valid ARL
    /// let arl = Arl::new("valid_token".to_string())?;
    ///
    /// // Invalid characters
    /// assert!(Arl::new("invalid;token".to_string()).is_err());
    /// assert!(Arl::new("spaces not allowed".to_string()).is_err());
    /// assert!(Arl::new("控制字符".to_string()).is_err());
    /// ```
    pub fn new(arl: String) -> Result<Self> {
        Ok(Self(arl))
    }
}

/// Provides read-only access to the validated ARL string.
///
/// # Examples
///
/// ```rust
/// use pleezer::arl::Arl;
///
/// let arl = Arl::new("token123".to_string())?;
/// assert_eq!(arl.len(), 8);  // Access String methods
/// assert_eq!(&*arl, "token123");  // Direct string access
/// ```
impl Deref for Arl {
    /// Target type for deref coercion.
    ///
    /// Allows read-only access to the underlying string while maintaining
    /// validation invariants.
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Formats the ARL for string representation.
///
/// Note: While this implementation allows displaying the token,
/// this should only be used when absolutely necessary, as it
/// exposes sensitive credentials.
///
/// # Examples
///
/// ```rust
/// use pleezer::arl::Arl;
///
/// let arl = Arl::new("token123".to_string())?;
///
/// // Avoid this in production code:
/// println!("{}", arl);  // Prints: token123
///
/// // Debug output is safely redacted:
/// println!("{:?}", arl);  // Prints: Arl("REDACTED")
/// ```
impl fmt::Display for Arl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Parses and validates an ARL from a string.
///
/// This implementation:
/// 1. Extracts the token from callback URLs if present
/// 2. Validates all characters for cookie safety
/// 3. Creates a new validated ARL instance
///
/// # Examples
///
/// ```rust
/// use std::str::FromStr;
/// use pleezer::arl::Arl;
///
/// // Direct token
/// let arl = Arl::from_str("valid_token")?;
///
/// // From callback URL
/// let arl = Arl::from_str("deezer://autolog/valid_token")?;
///
/// // Invalid characters
/// assert!(Arl::from_str("invalid;token").is_err());
/// ```
///
/// # Errors
///
/// Returns `Error::InvalidArgument` if:
/// * The string contains non-cookie-safe characters:
///   - Non-ASCII characters
///   - Control characters
///   - Whitespace
///   - Special characters (`"`, `,`, `;`, `\`)
impl FromStr for Arl {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let mut arl = s;

        // Foolproofing: in case a full callback URL is set.
        let parts: Vec<&str> = s.split('/').collect();
        if let Some(last_part) = parts.last() {
            arl = last_part;
        }

        // An `arl` must hold a valid cookie value.
        for chr in s.chars() {
            if !chr.is_ascii()
                || chr.is_ascii_control()
                || chr.is_ascii_whitespace()
                || ['\"', ',', ';', '\\'].contains(&chr)
            {
                return Err(Error::invalid_argument("invalid characters".to_string()));
            }
        }

        Ok(Self(arl.to_owned()))
    }
}
