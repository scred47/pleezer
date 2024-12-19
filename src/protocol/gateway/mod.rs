//! Gateway API types and functionality for Deezer's web services.
//!
//! This module provides type-safe interfaces to Deezer's gateway API endpoints,
//! handling:
//! * Authentication tokens ([`arl`])
//! * User data and settings ([`user_data`])
//! * Content listings ([`list_data`])
//! * Radio stations ([`user_radio`])
//!
//! # Number Handling
//!
//! All numeric values are stored as 64-bit integers because the JSON protocol
//! doesn't distinguish between number sizes. This ensures safe handling of all
//! possible values from the API.
//!
//! # Response Types
//!
//! The API returns two types of responses:
//! * Paginated lists ([`Response::Paginated`])
//! * Simple results ([`Response::Unpaginated`])
//!
//! # Example
//!
//! ```rust
//! use deezer::gateway::{Response, UserData};
//!
//! // Parse a user data response
//! let response: Response<UserData> = serde_json::from_str(json)?;
//!
//! // Access the first result
//! if let Some(user) = response.first() {
//!     println!("User: {}", user.name);
//! }
//! ```

pub mod arl;
pub mod list_data;
pub mod user_data;
pub mod user_radio;

pub use arl::Arl;
pub use list_data::{ListData, Queue};
pub use user_data::{MediaUrl, UserData};
pub use user_radio::UserRadio;

use std::{collections::HashMap, convert::Infallible, ops::Deref, str::FromStr};

use serde::Deserialize;
use serde_with::serde_as;

/// Defines a gateway API method identifier.
///
/// Each type implementing this trait represents a specific Deezer gateway API
/// endpoint, identified by a method name string.
///
/// # Examples
///
/// ```rust
/// use deezer::gateway::{Method, Arl};
///
/// // ARL endpoint
/// assert_eq!(Arl::METHOD, "user.getArl");
///
/// // Generic function using method name
/// fn call_api<T: Method>(params: &str) {
///     println!("Calling {}: {}", T::METHOD, params);
/// }
/// ```
pub trait Method {
    /// The gateway API method name.
    ///
    /// This constant identifies the specific API endpoint, using Deezer's
    /// dot-notation format (e.g., "user.getArl").
    const METHOD: &'static str;
}

/// Response from a Deezer gateway API endpoint.
///
/// Responses can be either paginated (with total counts and filtered results)
/// or unpaginated (direct result lists). Both formats include an error map
/// for API status information.
///
/// # Response Formats
///
/// Paginated format:
/// ```json
/// {
///     "error": {},
///     "results": {
///         "data": [...],
///         "count": 10,
///         "total": 100,
///         "filtered_count": 10
///     }
/// }
/// ```
///
/// Unpaginated format:
/// ```json
/// {
///     "error": {},
///     "results": [...]  // Direct array or single item
/// }
/// ```
///
/// # Examples
///
/// ```rust
/// use deezer::gateway::Response;
///
/// // Working with paginated data
/// let response: Response<Track> = serde_json::from_str(json)?;
/// if let Some(first_track) = response.first() {
///     println!("First track: {}", first_track.title);
/// }
///
/// // Getting all results regardless of pagination
/// for item in response.all() {
///     println!("Item: {:?}", item);
/// }
/// ```
#[serde_as]
#[derive(Clone, PartialEq, Deserialize, Debug)]
#[serde(untagged)]
pub enum Response<T> {
    /// Paginated response with result counts
    Paginated {
        /// API status information
        #[serde_as(as = "serde_with::Seq<(_, _)>")]
        error: HashMap<String, serde_json::Value>,
        /// Paginated result set
        results: Paginated<T>,
    },

    /// Direct response with results array
    Unpaginated {
        /// API status information
        #[serde_as(as = "serde_with::Seq<(_, _)>")]
        error: HashMap<String, serde_json::Value>,
        /// Result items (single item or array)
        #[serde_as(as = "serde_with::OneOrMany<_>")]
        results: Vec<T>,
    },
}

impl<T> Response<T> {
    /// Returns the first result item, if any.
    ///
    /// Works consistently for both paginated and unpaginated responses.
    ///
    /// # Examples
    ///
    /// ```rust
    /// if let Some(item) = response.first() {
    ///     println!("First item: {:?}", item);
    /// }
    /// ```
    #[must_use]
    pub fn first(&self) -> Option<&T> {
        self.all().first()
    }

    /// Returns all result items as a slice.
    ///
    /// Works consistently for both paginated and unpaginated responses.
    ///
    /// # Examples
    ///
    /// ```rust
    /// for item in response.all() {
    ///     println!("Item: {:?}", item);
    /// }
    /// ```
    #[must_use]
    pub fn all(&self) -> &Vec<T> {
        match self {
            Self::Paginated { results, .. } => &results.data,
            Self::Unpaginated { results, .. } => results,
        }
    }
}

/// Paginated result set from the Deezer gateway API.
///
/// Contains both the actual data items and metadata about the total
/// number of available items and filtering.
///
/// # Example Response
///
/// ```json
/// {
///     "data": [...],           // Actual items
///     "count": 10,            // Items in this page
///     "total": 100,           // Total available items
///     "filtered_count": 10    // Items matching filters
/// }
/// ```
#[serde_as]
#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct Paginated<T> {
    /// Items in this page of results
    pub data: Vec<T>,
    /// Number of items in this page
    pub count: u64,
    /// Total number of items available
    pub total: u64,
    /// Number of items matching applied filters
    pub filtered_count: u64,
}

/// String value that defaults to "UNKNOWN" when parsing fails.
///
/// Used for API fields that might return unexpected or invalid values,
/// ensuring robust handling of responses while maintaining type safety.
///
/// # Examples
///
/// ```rust
/// use deezer::gateway::StringOrUnknown;
///
/// // Normal string
/// let value: StringOrUnknown = "value".parse()?;
/// assert_eq!(&*value, "value");
///
/// // Default value
/// let unknown = StringOrUnknown::default();
/// assert_eq!(&*unknown, "UNKNOWN");
/// ```
///
/// # Deref Behavior
///
/// Derefs to `String` for convenient access to string methods:
/// ```rust
/// use deezer::gateway::StringOrUnknown;
///
/// let value = StringOrUnknown::default();
/// assert_eq!(value.to_uppercase(), "UNKNOWN");
/// ```
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Debug, Hash)]
pub struct StringOrUnknown(pub String);

/// Provides read-only access to the underlying string.
///
/// # Examples
///
/// ```rust
/// use deezer::gateway::StringOrUnknown;
///
/// let value = StringOrUnknown("test".to_string());
/// assert_eq!(value.len(), 4);  // Uses String's len() method
/// assert_eq!(&*value, "test"); // Direct access to string content
/// ```
impl Deref for StringOrUnknown {
    /// Target type for deref coercion.
    ///
    /// Allows `StringOrUnknown` to be used anywhere a `String` reference is expected.
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Creates a `StringOrUnknown` from a string slice.
///
/// Simply wraps the input in a new `String`. Cannot fail.
///
/// # Examples
///
/// ```rust
/// use std::str::FromStr;
/// use deezer::gateway::StringOrUnknown;
///
/// let value = StringOrUnknown::from_str("test")?;
/// assert_eq!(&*value, "test");
///
/// // Also works with string literals
/// let value: StringOrUnknown = "test".parse()?;
/// assert_eq!(&*value, "test");
/// ```
impl FromStr for StringOrUnknown {
    /// This implementation never fails, ensuring robust parsing.
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

/// Creates a new `StringOrUnknown` with the value "UNKNOWN".
///
/// Used when a string value cannot be properly parsed or is missing.
///
/// # Examples
///
/// ```rust
/// use deezer::gateway::StringOrUnknown;
///
/// let value = StringOrUnknown::default();
/// assert_eq!(&*value, "UNKNOWN");
/// ```
impl Default for StringOrUnknown {
    fn default() -> Self {
        Self(String::from("UNKNOWN"))
    }
}
