//! Error handling for pleezer.
//!
//! Provides a unified error handling system based on gRPC status codes,
//! with mapping from various underlying errors to appropriate categories.
//!
//! # Error Categories
//!
//! Errors are categorized into standard types that map to HTTP status codes:
//! * Authentication/authorization (401, 403)
//! * Resource state (404, 409)
//! * Client errors (400, 429)
//! * Server errors (500, 501, 503)
//! * Timeouts and cancellation (499, 504)
//!
//! # Example
//!
//! ```rust
//! use pleezer::error::{Error, ErrorKind, Result};
//!
//! fn do_something() -> Result<()> {
//!     // Create typed errors
//!     if condition {
//!         return Err(Error::not_found("resource doesn't exist"));
//!     }
//!
//!     // Convert from standard errors
//!     let file = std::fs::File::open("file.txt")?;
//!
//!     Ok(())
//! }
//! ```

#![allow(clippy::enum_glob_use)]

use std::fmt;
use thiserror::Error;

/// Main error type combining error kind and details.
///
/// Provides:
/// * Categorized error types ([`ErrorKind`])
/// * Underlying error details
/// * Conversion from common error types
/// * HTTP status code mapping
#[derive(Debug)]
pub struct Error {
    /// Classification of the error
    pub kind: ErrorKind,

    /// Details of the underlying error
    pub error: Box<dyn std::error::Error + Send + Sync>,
}

impl Error {
    /// Attempts to downcast the underlying error to a concrete type.
    ///
    /// Allows accessing the original error when its concrete type is known.
    ///
    /// # Arguments
    /// * `E` - The target error type to downcast to
    ///
    /// # Returns
    /// * `Some(&E)` - If the underlying error is of type `E`
    /// * `None` - If the underlying error is not of type `E`
    ///
    /// # Example
    /// ```
    /// use std::io;
    ///
    /// let io_error = io::Error::new(io::ErrorKind::Other, "oh no!");
    /// let error = Error::from(io_error);
    ///
    /// if let Some(io_err) = error.downcast::<io::Error>() {
    ///     println!("IO error kind: {:?}", io_err.kind());
    /// }
    /// ```
    #[must_use]
    pub fn downcast<E>(&self) -> Option<&E>
    where
        E: std::error::Error + 'static,
    {
        self.error.downcast_ref::<E>()
    }
}

/// Standard result type for pleezer operations.
///
/// Wraps the standard `Result` type with our custom [`struct@Error`] type.
pub type Result<T> = std::result::Result<T, Error>;

/// Error categories based on gRPC status codes.
///
/// Each variant:
/// * Maps to a specific HTTP status code
/// * Represents a distinct failure category
/// * Carries a standard error message
///
/// See [gRPC status codes](https://github.com/googleapis/googleapis/blob/master/google/rpc/code.proto)
/// for the original definitions.
#[expect(clippy::module_name_repetitions)]
#[derive(Clone, Copy, Debug, Eq, Error, Hash, Ord, PartialEq, PartialOrd)]
pub enum ErrorKind {
    /// HTTP Mapping: 499 Client Closed Request
    #[error("Operation was cancelled")]
    Cancelled = 1,

    /// HTTP Mapping: 500 Internal Server Error
    #[error("Unknown error")]
    Unknown = 2,

    /// HTTP Mapping: 400 Bad Request
    #[error("Invalid argument specified")]
    InvalidArgument = 3,

    /// HTTP Mapping: 504 Gateway Timeout
    #[error("Operation timed out")]
    DeadlineExceeded = 4,

    /// HTTP Mapping: 404 Not Found
    #[error("Not found")]
    NotFound = 5,

    /// HTTP Mapping: 409 Conflict
    #[error("Attempt to create what already exists")]
    AlreadyExists = 6,

    /// HTTP Mapping: 403 Forbidden
    #[error("Permission denied")]
    PermissionDenied = 7,

    /// HTTP Mapping: 401 Unauthorized
    #[error("No valid authentication credentials")]
    Unauthenticated = 16,

    /// HTTP Mapping: 429 Too Many Requests
    #[error("Resource has been exhausted")]
    ResourceExhausted = 8,

    /// HTTP Mapping: 400 Bad Request
    #[error("Invalid state")]
    FailedPrecondition = 9,

    /// HTTP Mapping: 409 Conflict
    #[error("Operation aborted")]
    Aborted = 10,

    /// HTTP Mapping: 400 Bad Request
    #[error("Out of range")]
    OutOfRange = 11,

    /// HTTP Mapping: 501 Not Implemented
    #[error("Not implemented")]
    Unimplemented = 12,

    /// HTTP Mapping: 500 Internal Server Error
    #[error("Internal error")]
    Internal = 13,

    /// HTTP Mapping: 503 Service Unavailable
    #[error("Service unavailable")]
    Unavailable = 14,

    /// HTTP Mapping: 500 Internal Server Error
    #[error("Unrecoverable data loss or corruption")]
    DataLoss = 15,
}

impl Error {
    /// Creates a new error with specified kind and details.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::new(ErrorKind::NotFound, "user profile not found");
    /// assert_eq!(err.kind, ErrorKind::NotFound);
    /// ```
    pub fn new<E>(kind: ErrorKind, error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind,
            error: error.into(),
        }
    }

    /// Creates an error for operations that were interrupted mid-execution.
    ///
    /// Maps to HTTP 409 Conflict. Use when an operation couldn't complete
    /// due to conflicting changes or state.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::aborted("download interrupted");
    /// assert_eq!(err.kind, ErrorKind::Aborted);
    /// ```
    pub fn aborted<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Aborted,
            error: error.into(),
        }
    }

    /// Creates an error for duplicate resource creation attempts.
    ///
    /// Maps to HTTP 409 Conflict. Use when attempting to create
    /// a resource that already exists.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::already_exists("user account already registered");
    /// assert_eq!(err.kind, ErrorKind::AlreadyExists);
    /// ```
    pub fn already_exists<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::AlreadyExists,
            error: error.into(),
        }
    }

    /// Creates an error for cancelled operations.
    ///
    /// Maps to HTTP 499 Client Closed Request. Use when an operation
    /// was cancelled before completion.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::cancelled("user cancelled download");
    /// assert_eq!(err.kind, ErrorKind::Cancelled);
    /// ```
    pub fn cancelled<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Cancelled,
            error: error.into(),
        }
    }

    /// Creates an error for data corruption or loss.
    ///
    /// Maps to HTTP 500 Internal Server Error. Use when data has been
    /// corrupted or lost in an unrecoverable way.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::data_loss("track data corrupted");
    /// assert_eq!(err.kind, ErrorKind::DataLoss);
    /// ```
    pub fn data_loss<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::DataLoss,
            error: error.into(),
        }
    }

    /// Creates an error for operations that exceeded their deadline.
    ///
    /// Maps to HTTP 504 Gateway Timeout. Use when an operation took
    /// longer than its allowed time.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::deadline_exceeded("download timed out");
    /// assert_eq!(err.kind, ErrorKind::DeadlineExceeded);
    /// ```
    pub fn deadline_exceeded<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::DeadlineExceeded,
            error: error.into(),
        }
    }

    /// Creates an error for operations that failed due to current state.
    ///
    /// Maps to HTTP 400 Bad Request. Use when an operation cannot proceed
    /// due to the current system state.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::failed_precondition("must be logged in first");
    /// assert_eq!(err.kind, ErrorKind::FailedPrecondition);
    /// ```
    pub fn failed_precondition<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::FailedPrecondition,
            error: error.into(),
        }
    }

    /// Creates an error for internal errors.
    ///
    /// Maps to HTTP 500 Internal Server Error. Use for unexpected internal
    /// errors that shouldn't occur during normal operation.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::internal("unexpected null pointer");
    /// assert_eq!(err.kind, ErrorKind::Internal);
    /// ```
    pub fn internal<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Internal,
            error: error.into(),
        }
    }

    /// Creates an error for invalid arguments.
    ///
    /// Maps to HTTP 400 Bad Request. Use when provided arguments
    /// don't meet validation requirements.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::invalid_argument("email address malformed");
    /// assert_eq!(err.kind, ErrorKind::InvalidArgument);
    /// ```
    pub fn invalid_argument<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::InvalidArgument,
            error: error.into(),
        }
    }

    /// Creates an error for missing resources.
    ///
    /// Maps to HTTP 404 Not Found. Use when a requested resource
    /// doesn't exist.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::not_found("track does not exist");
    /// assert_eq!(err.kind, ErrorKind::NotFound);
    /// ```
    pub fn not_found<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::NotFound,
            error: error.into(),
        }
    }

    /// Creates an error for values outside valid range.
    ///
    /// Maps to HTTP 400 Bad Request. Use when a value exceeds
    /// its allowed bounds.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::out_of_range("volume must be between 0 and 100");
    /// assert_eq!(err.kind, ErrorKind::OutOfRange);
    /// ```
    pub fn out_of_range<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::OutOfRange,
            error: error.into(),
        }
    }

    /// Creates an error for permission denied conditions.
    ///
    /// Maps to HTTP 403 Forbidden. Use when the caller lacks
    /// necessary permissions.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::permission_denied("premium subscription required");
    /// assert_eq!(err.kind, ErrorKind::PermissionDenied);
    /// ```
    pub fn permission_denied<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::PermissionDenied,
            error: error.into(),
        }
    }

    /// Creates an error for exhausted resources.
    ///
    /// Maps to HTTP 429 Too Many Requests. Use when a resource
    /// limit has been reached.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::resource_exhausted("too many concurrent downloads");
    /// assert_eq!(err.kind, ErrorKind::ResourceExhausted);
    /// ```
    pub fn resource_exhausted<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::ResourceExhausted,
            error: error.into(),
        }
    }

    /// Creates an error for authentication failures.
    ///
    /// Maps to HTTP 401 Unauthorized. Use when valid credentials
    /// are required but not provided.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::unauthenticated("login required");
    /// assert_eq!(err.kind, ErrorKind::Unauthenticated);
    /// ```
    pub fn unauthenticated<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Unauthenticated,
            error: error.into(),
        }
    }

    /// Creates an error for unavailable services.
    ///
    /// Maps to HTTP 503 Service Unavailable. Use when the service
    /// is temporarily unavailable.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::unavailable("service is down for maintenance");
    /// assert_eq!(err.kind, ErrorKind::Unavailable);
    /// ```
    pub fn unavailable<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Unavailable,
            error: error.into(),
        }
    }

    /// Creates an error for unimplemented features.
    ///
    /// Maps to HTTP 501 Not Implemented. Use when the requested
    /// operation isn't implemented.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::unimplemented("feature not yet available");
    /// assert_eq!(err.kind, ErrorKind::Unimplemented);
    /// ```
    pub fn unimplemented<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Unimplemented,
            error: error.into(),
        }
    }

    /// Creates an error for unknown errors.
    ///
    /// Maps to HTTP 500 Internal Server Error. Use when the error
    /// doesn't fit any other category.
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::unknown("unexpected error occurred");
    /// assert_eq!(err.kind, ErrorKind::Unknown);
    /// ```
    pub fn unknown<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Unknown,
            error: error.into(),
        }
    }
}

impl std::error::Error for Error {
    /// Returns the underlying error source.
    ///
    /// This allows error chains to be examined for root causes.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

impl fmt::Display for Error {
    /// Formats the error for display, showing both kind and details.
    ///
    /// Format: "{kind}: {details}"
    ///
    /// # Examples
    ///
    /// ```rust
    /// let err = Error::not_found("user not found");
    /// assert_eq!(err.to_string(), "Not found: user not found");
    /// ```
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{}: ", self.kind)?;
        self.error.fmt(fmt)
    }
}

impl From<std::io::Error> for Error {
    /// Converts IO errors into appropriate error kinds.
    ///
    /// Maps standard IO errors to their logical equivalents:
    /// * `NotFound` -> `NotFound`
    /// * `PermissionDenied` -> `PermissionDenied`
    /// * `ConnectionReset` -> `Aborted`
    /// * etc.
    fn from(err: std::io::Error) -> Self {
        use std::io::ErrorKind::*;
        match err.kind() {
            NotFound => Self::not_found(err),
            PermissionDenied => Self::permission_denied(err),
            AddrInUse | AlreadyExists => Self::already_exists(err),
            AddrNotAvailable | ConnectionRefused | NotConnected => Self::unavailable(err),
            BrokenPipe | ConnectionReset | ConnectionAborted => Self::aborted(err),
            Interrupted | WouldBlock => Self::cancelled(err),
            UnexpectedEof => Self::data_loss(err),
            TimedOut => Self::deadline_exceeded(err),
            InvalidInput | InvalidData => Self::invalid_argument(err),
            WriteZero => Self::resource_exhausted(err),
            _ => Self::unknown(err),
        }
    }
}

impl From<reqwest::Error> for Error {
    /// Converts HTTP client errors into appropriate error kinds.
    ///
    /// Maps HTTP errors based on their nature:
    /// * Body errors -> `DataLoss`
    /// * Decode errors -> `InvalidArgument`
    /// * Connect errors -> `Unavailable`
    /// * Timeout errors -> `DeadlineExceeded`
    /// * etc.
    fn from(err: reqwest::Error) -> Self {
        if err.is_body() {
            return Self::data_loss(err);
        }

        if err.is_decode() {
            return Self::invalid_argument(err);
        }

        if err.is_builder() | err.is_builder() {
            return Self::internal(err);
        }

        if err.is_connect() || err.is_redirect() {
            return Self::unavailable(err);
        }

        if err.is_redirect() {
            return Self::resource_exhausted(err);
        }

        if err.is_status() {
            return Self::failed_precondition(err);
        }

        if err.is_timeout() {
            return Self::deadline_exceeded(err);
        }

        Self::unknown(err)
    }
}

impl From<semver::Error> for Error {
    /// Converts version parsing errors to `InvalidArgument`.
    fn from(err: semver::Error) -> Self {
        Self::invalid_argument(err)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
    /// Converts WebSocket errors into appropriate error kinds.
    ///
    /// Maps WebSocket errors based on their type:
    /// * `ConnectionClosed` -> `Cancelled`
    /// * `AlreadyClosed` -> `Unavailable`
    /// * `Capacity` -> `OutOfRange`
    /// * `Utf8` -> `InvalidArgument`
    /// * etc.
    fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
        use tokio_tungstenite::tungstenite::Error::*;
        match err {
            ConnectionClosed => Self::cancelled(err),
            AlreadyClosed => Self::unavailable(err),
            Io(err) => Self::data_loss(err),
            Http(_) => Self::unknown(err),
            Tls(err) => Self::unknown(err),
            Capacity(err) => Self::out_of_range(err),
            HttpFormat(err) => Self::unknown(err),
            Protocol(err) => Self::unknown(err),
            Url(err) => Self::unknown(err),
            Utf8 => Self::invalid_argument(err),
            WriteBufferFull(err) => Self::resource_exhausted(err.to_string()),
            AttackAttempt => Self::permission_denied(err),
        }
    }
}

impl From<serde_json::Error> for Error {
    /// Converts JSON errors through IO error mapping.
    ///
    /// JSON errors are first converted to IO errors, then mapped
    /// using the IO error conversion rules.
    fn from(err: serde_json::Error) -> Self {
        std::io::Error::from(err).into()
    }
}

impl From<http::header::MaxSizeReached> for Error {
    /// Converts header size errors to `OutOfRange`.
    fn from(e: http::header::MaxSizeReached) -> Self {
        Self::out_of_range(e.to_string())
    }
}

impl From<http::header::InvalidHeaderValue> for Error {
    /// Converts invalid header errors to `Internal`.
    fn from(e: http::header::InvalidHeaderValue) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<url::ParseError> for Error {
    /// Converts URL parsing errors to `Internal`.
    fn from(e: url::ParseError) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<http::uri::InvalidUri> for Error {
    /// Converts URI parsing errors to `Internal`.
    fn from(e: http::uri::InvalidUri) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<std::fmt::Error> for Error {
    /// Converts formatting errors to `Unknown`.
    fn from(e: std::fmt::Error) -> Self {
        Self::unknown(e.to_string())
    }
}

impl From<flate2::DecompressError> for Error {
    /// Converts decompression errors to `DataLoss`.
    fn from(e: flate2::DecompressError) -> Self {
        Self::data_loss(e.to_string())
    }
}

impl From<base64::DecodeError> for Error {
    /// Converts Base64 decoding errors to `InvalidArgument`.
    fn from(e: base64::DecodeError) -> Self {
        Self::invalid_argument(e.to_string())
    }
}

impl From<std::num::ParseIntError> for Error {
    /// Converts integer parsing errors to `InvalidArgument`.
    fn from(e: std::num::ParseIntError) -> Self {
        Self::invalid_argument(e.to_string())
    }
}

impl<T> From<std::sync::PoisonError<std::sync::MutexGuard<'_, T>>> for Error {
    /// Converts mutex poisoning errors to `Internal`.
    fn from(e: std::sync::PoisonError<std::sync::MutexGuard<'_, T>>) -> Self {
        Self::internal(e.to_string())
    }
}

impl<S> From<stream_download::StreamInitializationError<S>> for Error
where
    S: stream_download::source::SourceStream,
{
    /// Converts stream initialization errors to `Internal`.
    fn from(e: stream_download::StreamInitializationError<S>) -> Self {
        Self::internal(e.to_string())
    }
}

impl<C> From<stream_download::http::HttpStreamError<C>> for Error
where
    C: stream_download::http::Client,
{
    /// Converts HTTP stream errors based on their type.
    ///
    /// Maps stream errors:
    /// * `FetchFailure` -> `DataLoss`
    /// * `ResponseFailure` -> `Unavailable`
    fn from(e: stream_download::http::HttpStreamError<C>) -> Self {
        use stream_download::http::HttpStreamError::*;
        match e {
            FetchFailure(e) => Self::data_loss(e.to_string()),
            ResponseFailure(e) => Self::unavailable(e.to_string()),
        }
    }
}

impl From<rodio::StreamError> for Error {
    /// Converts audio stream errors into appropriate error kinds.
    ///
    /// Maps audio errors:
    /// * `PlayStreamError` -> `Unavailable`
    /// * `NoDevice` -> `NotFound`
    /// * etc.
    fn from(e: rodio::StreamError) -> Self {
        use rodio::StreamError::*;
        match e {
            PlayStreamError(e) => Self::unavailable(e),
            DefaultStreamConfigError(e) => Self::unavailable(e),
            BuildStreamError(e) => Self::unavailable(e),
            SupportedStreamConfigsError(e) => Self::not_found(e),
            NoDevice => Self::not_found(e),
        }
    }
}

impl From<rodio::DevicesError> for Error {
    /// Converts audio device errors to `Unknown`.
    fn from(e: rodio::DevicesError) -> Self {
        Self::unknown(e.to_string())
    }
}

impl From<cpal::SupportedStreamConfigsError> for Error {
    /// Converts audio config errors into appropriate error kinds.
    ///
    /// Maps config errors:
    /// * `DeviceNotAvailable` -> `Unavailable`
    /// * `InvalidArgument` -> `InvalidArgument`
    /// * `BackendSpecific` -> `Unknown`
    fn from(e: cpal::SupportedStreamConfigsError) -> Self {
        use cpal::SupportedStreamConfigsError::*;
        match e {
            DeviceNotAvailable => Self::unavailable(e),
            InvalidArgument => Self::invalid_argument(e),
            BackendSpecific { err } => Self::unknown(err),
        }
    }
}

impl From<rodio::PlayError> for Error {
    /// Converts playback errors into appropriate error kinds.
    ///
    /// Maps playback errors:
    /// * `DecoderError` -> `DataLoss`
    /// * `NoDevice` -> `NotFound`
    fn from(e: rodio::PlayError) -> Self {
        use rodio::PlayError::*;
        match e {
            DecoderError(e) => Self::data_loss(e),
            NoDevice => Self::not_found(e),
        }
    }
}

impl From<rodio::source::SeekError> for Error {
    /// Converts seek errors into appropriate error kinds.
    ///
    /// Maps seek errors:
    /// * `NotSupported` -> `Unimplemented`
    /// * `SymphoniaDecoder` -> `DataLoss`
    /// * Others -> `Unknown`
    fn from(e: rodio::source::SeekError) -> Self {
        use rodio::source::SeekError::*;
        match e {
            NotSupported { underlying_source } => Self::unimplemented(underlying_source),
            SymphoniaDecoder(e) => Self::data_loss(e),
            _ => Self::unknown(e.to_string()),
        }
    }
}

impl From<rodio::decoder::DecoderError> for Error {
    /// Converts decoder errors into appropriate error kinds.
    ///
    /// Maps decoder errors:
    /// * `UnrecognizedFormat` -> `Unknown`
    /// * `IoError` -> `DataLoss`
    /// * `DecodeError` -> `DataLoss`
    /// * `LimitError` -> `ResourceExhausted`
    /// * `NoStreams` -> `NotFound`
    /// * etc.
    fn from(e: rodio::decoder::DecoderError) -> Self {
        use rodio::decoder::DecoderError::*;
        match e {
            UnrecognizedFormat => Self::unknown("format not recognized"),
            IoError(e) => Self::data_loss(e),
            DecodeError(e) => Self::data_loss(e),
            LimitError(e) => Self::resource_exhausted(e),
            ResetRequired => Self::internal(e),
            NoStreams => Self::not_found("no streams found"),
        }
    }
}

impl From<tokio::time::error::Elapsed> for Error {
    /// Converts timeout errors to `DeadlineExceeded`.
    fn from(e: tokio::time::error::Elapsed) -> Self {
        Self::deadline_exceeded(e.to_string())
    }
}

impl From<uuid::Error> for Error {
    /// Converts UUID errors to `InvalidArgument`.
    fn from(e: uuid::Error) -> Self {
        Self::invalid_argument(e.to_string())
    }
}
