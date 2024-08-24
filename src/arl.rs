use crate::gateway::Gateway;
use std::{fmt, io, ops::Deref, str::FromStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("gateway error: {0}")]
    Gateway(#[from] crate::gateway::Error),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("invalid ARL: {0}")]
    Invalid(String),

    #[error("permission denied")]
    PermissionDenied,
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Arl(String);

impl Arl {
    /// The Deezer API client ID.
    const CLIENT_ID: usize = 447_462;

    /// The Deezer API client secret.
    const CLIENT_SECRET: &'static str = "a83bf7f38ad2f137e444727cfc3775cf";

    /// The Deezer API URL that will be used to get the session ID.
    const SID_URL: &'static str = "https://connect.deezer.com/oauth/auth.php";

    /// The Deezer API authentication URL.
    const AUTH_URL: &'static str = "https://connect.deezer.com/oauth/user_auth.php";

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `arl` contains invalid characters
    pub fn new(arl: String) -> io::Result<Self> {
        Ok(Self(arl))
    }

    pub async fn from_credentials(
        mut gateway: Gateway,
        email: &str,
        password: &str,
    ) -> Result<Self> {
        // Check email and password length to prevent out-of-memory conditions.
        const LENGTH_CHECK: std::ops::Range<usize> = 1..255;
        if !LENGTH_CHECK.contains(&email.len()) || !LENGTH_CHECK.contains(&password.len()) {
            return Err(Error::Invalid(
                "email and password must be between 1 and 255 characters".to_string(),
            ));
        }

        // Create a new HTTP client with a cookie store to keep the session ID.
        let http_client = gateway.http_client();

        // Hash the passwords.
        let password = md5::compute(password);
        let hash = md5::compute(format!(
            "{}{email}{password:x}{}",
            Self::CLIENT_ID,
            Self::CLIENT_SECRET
        ));

        // First get a session ID. The response can be ignored because the
        // session ID is stored in the cookie store.
        let _ = http_client.get(Self::SID_URL).send().await?;

        // Then login and get an access token.
        let query = format!(
            "{}?app_id={}&login={email}&password={password:x}&hash={hash:x}",
            Self::AUTH_URL,
            Self::CLIENT_ID
        );
        let response = http_client
            .get(query)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await?;
        let access_token = response
            .get("access_token")
            .and_then(|token| token.as_str())
            .ok_or_else(|| Error::PermissionDenied)?;

        // Finally use the access token to get an ARL.
        gateway.get_arl(access_token).await.map_err(Into::into)
    }
}

impl Deref for Arl {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Arl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

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
                return Err(Error::Invalid("invalid characters".to_string()));
            }
        }

        Ok(Self(arl.to_owned()))
    }
}
