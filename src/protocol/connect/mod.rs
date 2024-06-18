//! Numbers are parsed and stored in 64-bit format, because [JSON] does not
//! distinguish between different sizes of numbers.

use thiserror::Error;

pub mod channel;
pub mod contents;
pub mod messages;
pub mod protos;

pub use channel::{Channel, Event, UserId};
pub use contents::{
    Body, Contents, DeviceId, Headers, Element, Percentage, AudioQuality, RepeatMode, Status,
};
pub use messages::Message;
pub use protos::queue;

/// A specialized [`Result`] for [Deezer Connect][Connect] websocket
/// operations.
///
/// This type is broadly used across [`pleezer::protocol::connect`] for any
/// operation which may produce an error.
///
/// This typedef is generally used to avoid writing out [`connect::Error`]
/// directly and is otherwise a direct mapping to [`Result`].
///
/// While usual Rust style is to import types directly, aliases of [`Result`]
/// often are not, to make it easier to distinguish between them. [`Result`] is
/// generally assumed to be [`std::result::Result`][`Result`], and so users of
/// this alias will generally use `connect::Result` instead of shadowing the
/// prelude's import of [`std::result::Result`][`Result`].
///
/// # Examples
///
/// A convenience function that bubbles an `connect::Result` to its caller:
///
/// ```
/// use protocol::connect::{self, Message};
///
/// fn get_message(s: &str) -> connect::Result<Message> {
///     s.parse::<Message>()
/// }
/// ```
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
/// [`connect::Error`]: enum.Error.html
/// [`pleezer::protocol::connect`]: index.html
/// [`Result`]: https://doc.rust-lang.org/stable/std/result/enum.Result.html
pub type Result<T> = std::result::Result<T, Error>;

/// The error type for [Deezer Connect][Connect] websocket operations.
///
/// [Connect]: https://en.deezercommunity.com/product-updates/try-our-remote-control-and-let-us-know-how-it-works-70079
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum Error {
    #[error("malformed base64: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error(transparent)]
    Deflate(#[from] flate2::DecompressError),

    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    #[error["invalid input: {0}"]]
    InvalidInput(String),

    #[error(transparent)]
    Json(#[from] serde_json::Error),

    #[error("malformed message: {0}")]
    Malformed(String),

    #[error("unsupported message: {0}")]
    Unsupported(String),

    #[error("write error")]
    Write(#[from] std::fmt::Error),
}
