use uuid::Uuid;

use crate::{arl::Arl, decrypt::Key};

/// Methods that can be used to authenticate with Deezer.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Credentials {
    /// The user's email and password.
    Login {
        /// The user's email.
        email: String,
        /// The user's password.
        password: String,
    },

    /// The user's `arl` token.
    Arl(Arl),
}

/// The configuration of pleezer.
// TODO: implement Debug manually to avoid leaking the arl.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Config {
    /// The name of the application.
    ///
    /// By default this is retrieved from `Cargo.toml`, used in the
    /// `User-Agent` string, and the fallback device name if not provided and
    /// the system hostname is not available.
    pub app_name: String,

    /// The version of the application.
    ///
    /// By default this is retrieved from `Cargo.toml` used in the `User-Agent`
    /// string.
    pub app_version: String,

    /// The language of the application in ISO 639-1 format.
    ///
    /// By default this is "en" for English, used in the `User-Agent` string,
    /// as well as `Accept-Language`header in API requests.
    pub app_lang: String,

    /// The player's name as it appears to Deezer clients.
    ///
    /// By default this is equal to `app_name`.
    pub device_name: String,

    /// The ID that uniquely identifies the device.
    ///
    /// By default this is the machine ID, or a random UUID if the machine ID
    /// could not be retrieved.
    pub device_id: Uuid,

    /// Whether other clients may take over an existing connection.
    ///
    /// By default this is `true`.
    pub interruptions: bool,

    /// The client ID used in API requests.
    ///
    /// By default this is a random number of 9 digits.
    pub client_id: usize,

    /// The `User-Agent` string used in API requests.
    ///
    /// By default this is a combination of the application name, version, and
    /// language, to be like the official Deezer Desktop client.
    pub user_agent: String,

    /// The credentials used to authenticate with Deezer.
    pub credentials: Credentials,

    /// Secret for computing the track decryption key.
    pub bf_secret: Key,
}
