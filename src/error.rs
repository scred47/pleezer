use std::fmt;
use thiserror::Error;

/// Generic error type for pleezer, wrapping the underlying error.
#[derive(Debug)]
pub struct Error {
    /// The kind of error that occurred.
    pub kind: ErrorKind,

    /// The underlying error that occurred.
    pub error: Box<dyn std::error::Error + Send + Sync>,
}

/// A specialized `Result` type with pleezer's `Error` type as the error
/// variant.
pub type Result<T> = std::result::Result<T, Error>;

/// The kind of error that occurred, inspired by [gRPC status codes](https://github.com/googleapis/googleapis/blob/master/google/rpc/code.proto).
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
    /// Create a new error with the specified kind and error.
    pub fn new<E>(kind: ErrorKind, error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind,
            error: error.into(),
        }
    }

    /// Create an aborted error with the specified error.
    pub fn aborted<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Aborted,
            error: error.into(),
        }
    }

    /// Create an already exists error with the specified error.
    pub fn already_exists<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::AlreadyExists,
            error: error.into(),
        }
    }

    /// Create a cancelled error with the specified error.
    pub fn cancelled<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Cancelled,
            error: error.into(),
        }
    }

    /// Create a data loss error with the specified error.
    pub fn data_loss<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::DataLoss,
            error: error.into(),
        }
    }

    /// Create a deadline exceeded error with the specified error.
    pub fn deadline_exceeded<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::DeadlineExceeded,
            error: error.into(),
        }
    }

    /// Create a failed precondition error with the specified error.
    pub fn failed_precondition<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::FailedPrecondition,
            error: error.into(),
        }
    }

    /// Create an internal error with the specified error.
    pub fn internal<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Internal,
            error: error.into(),
        }
    }

    /// Create an invalid argument error with the specified error.
    pub fn invalid_argument<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::InvalidArgument,
            error: error.into(),
        }
    }

    /// Create a not found error with the specified error.
    pub fn not_found<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::NotFound,
            error: error.into(),
        }
    }

    /// Create an out of range error with the specified error.
    pub fn out_of_range<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::OutOfRange,
            error: error.into(),
        }
    }

    /// Create a permission denied error with the specified error.
    pub fn permission_denied<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::PermissionDenied,
            error: error.into(),
        }
    }

    /// Create a resource exhausted error with the specified error.
    pub fn resource_exhausted<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::ResourceExhausted,
            error: error.into(),
        }
    }

    /// Create an unauthenticated error with the specified error.
    pub fn unauthenticated<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Unauthenticated,
            error: error.into(),
        }
    }

    /// Create an unavailable error with the specified error.
    pub fn unavailable<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Unavailable,
            error: error.into(),
        }
    }

    /// Create an unimplemented error with the specified error.
    pub fn unimplemented<E>(error: E) -> Self
    where
        E: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        Self {
            kind: ErrorKind::Unimplemented,
            error: error.into(),
        }
    }

    /// Create an unknown error with the specified error.
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
    /// The lower-level source of this error, if any.
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.error.source()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "{} {{ ", self.kind)?;
        self.error.fmt(fmt)?;
        write!(fmt, " }}")
    }
}

impl From<std::io::Error> for Error {
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
    fn from(err: semver::Error) -> Self {
        Self::invalid_argument(err)
    }
}

impl From<tokio_tungstenite::tungstenite::Error> for Error {
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
    fn from(err: serde_json::Error) -> Self {
        std::io::Error::from(err).into()
    }
}

impl From<http::header::MaxSizeReached> for Error {
    fn from(e: http::header::MaxSizeReached) -> Self {
        Self::out_of_range(e.to_string())
    }
}

impl From<http::header::InvalidHeaderValue> for Error {
    fn from(e: http::header::InvalidHeaderValue) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<url::ParseError> for Error {
    fn from(e: url::ParseError) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<http::uri::InvalidUri> for Error {
    fn from(e: http::uri::InvalidUri) -> Self {
        Self::internal(e.to_string())
    }
}

impl From<std::fmt::Error> for Error {
    fn from(e: std::fmt::Error) -> Self {
        Self::resource_exhausted(e.to_string())
    }
}

impl From<flate2::DecompressError> for Error {
    fn from(e: flate2::DecompressError) -> Self {
        Self::data_loss(e.to_string())
    }
}

impl From<base64::DecodeError> for Error {
    fn from(e: base64::DecodeError) -> Self {
        Self::invalid_argument(e.to_string())
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: std::num::ParseIntError) -> Self {
        Self::invalid_argument(e.to_string())
    }
}

impl<T> From<std::sync::PoisonError<std::sync::MutexGuard<'_, T>>> for Error {
    fn from(e: std::sync::PoisonError<std::sync::MutexGuard<'_, T>>) -> Self {
        Self::internal(e.to_string())
    }
}

impl<S> From<stream_download::StreamInitializationError<S>> for Error
where
    S: stream_download::source::SourceStream,
{
    fn from(e: stream_download::StreamInitializationError<S>) -> Self {
        Self::internal(e.to_string())
    }
}

impl<C> From<stream_download::http::HttpStreamError<C>> for Error
where
    C: stream_download::http::Client,
{
    fn from(e: stream_download::http::HttpStreamError<C>) -> Self {
        use stream_download::http::HttpStreamError::*;
        match e {
            FetchFailure(e) => Self::data_loss(e.to_string()),
            ResponseFailure(e) => Self::unavailable(e.to_string()),
        }
    }
}

impl From<rodio::StreamError> for Error {
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
    fn from(e: rodio::DevicesError) -> Self {
        Self::unknown(e.to_string())
    }
}

impl From<cpal::SupportedStreamConfigsError> for Error {
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
    fn from(e: rodio::PlayError) -> Self {
        use rodio::PlayError::*;
        match e {
            DecoderError(e) => Self::data_loss(e),
            NoDevice => Self::not_found(e),
        }
    }
}

impl From<rodio::source::SeekError> for Error {
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
    fn from(e: tokio::time::error::Elapsed) -> Self {
        Self::deadline_exceeded(e.to_string())
    }
}
