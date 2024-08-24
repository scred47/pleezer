use std::{str::FromStr, time::SystemTime};

use http::header::{InvalidHeaderValue, MaxSizeReached};
use reqwest::{
    self,
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
};
use serde::Deserialize;
use thiserror::Error;

use crate::{
    arl::{self, Arl},
    config::Config,
    http::Client as HttpClient,
    protocol::{
        connect::{queue, AudioQuality},
        gateway::{self, Method, Queue, UserData},
    },
    // TODO : move into gateway
    tokens::{UserToken, UserTokenError},
};

pub struct Gateway {
    http_client: HttpClient,
    user_data: Option<UserData>,
    client_id: usize,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("assertion failed: {0}")]
    Assertion(String),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("HTTP header error: {0}")]
    HttpHeader(String),

    #[error("parsing JSON error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("parsing URL failed: {0}")]
    UrlParse(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Gateway {
    /// The URL of the Deezer cookie origin.
    ///
    /// This URL is not entirely correct, as the cookies could come from
    /// `connect.deezer.com` or `www.deezer.com` as well. What
    /// matters is that the domain matches with `deezer.com`.
    const COOKIE_ORIGIN: &'static str = "https://www.deezer.com";

    /// The URL of the Deezer gateway.
    const GATEWAY_URL: &'static str = "https://www.deezer.com/ajax/gw-light.php";

    /// The Deezer gateway version.
    const GATEWAY_VERSION: &'static str = "1.0";

    /// The Deezer gateway input type.
    const GATEWAY_INPUT: usize = 3;

    /// The `Content-Type` header value for the Deezer gateway requests.
    ///
    /// Although the bodies of all gateway requests are JSON, the
    /// `Content-Type` is not.
    const PLAIN_TEXT_CONTENT: HeaderValue = HeaderValue::from_static("text/plain;charset=UTF-8");

    /// An empty JSON object that is used as the default body for the Deezer
    /// API gateway requests.
    const EMPTY_JSON_OBJECT: &'static str = "{}";

    /// The cookie origin for the Deezer API as a `reqwest::Url`.
    ///
    /// # Panics
    ///
    /// Will panic if the URL is invalid.
    fn cookie_origin() -> reqwest::Url {
        reqwest::Url::parse(Self::COOKIE_ORIGIN).expect("invalid cookie origin")
    }

    /// Creates a new `reqwest::cookie::Jar` containing the necessary cookies
    /// for the Deezer API.
    fn cookie_jar(config: &Config) -> reqwest::cookie::Jar {
        let cookie_jar = reqwest::cookie::Jar::default();
        let cookie_origin = Self::cookie_origin();

        let lang_cookie = format!(
            "dz_lang={}; Domain=deezer.com; Path=/; Secure; HttpOnly",
            &config.app_lang
        );
        cookie_jar.add_cookie_str(&lang_cookie, &cookie_origin);

        if let Some(ref arl) = config.arl {
            let arl_cookie = format!("arl={arl}; Domain=deezer.com; Path=/; Secure; HttpOnly");
            cookie_jar.add_cookie_str(&arl_cookie, &cookie_origin);
        }

        cookie_jar
    }

    pub fn http_client(&self) -> reqwest::Client {
        self.http_client.inner.clone()
    }

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - no valid `User-Agent` can be created out of the `config` fields
    /// - no valid OS name and/or version can be detected
    /// - no valid cookies can be created out of the `arl` and/or `config` fields
    pub fn new(config: &Config) -> Result<Self> {
        // Create a new cookie jar and put the cookies in.
        let cookie_jar = Self::cookie_jar(config);
        let http_client = HttpClient::with_cookies(config, cookie_jar)?;

        Ok(Self {
            client_id: config.client_id,
            http_client,
            user_data: None,
        })
    }

    pub fn cookies(&self) -> Option<HeaderValue> {
        self.http_client
            .cookie_jar
            .as_ref()
            .and_then(|jar| jar.cookies(&Self::cookie_origin()))
    }

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - the `arl` is invalid or expired
    /// - the HTTP request failed
    pub async fn refresh(&mut self) -> Result<()> {
        // Send an empty JSON map
        match self
            .request::<gateway::UserData>(Self::EMPTY_JSON_OBJECT, None)
            .await
        {
            Ok(response) => {
                if let Some(data) = response.first() {
                    self.set_user_data(data.clone());
                } else {
                    return Err(Error::Assertion("no user data received".to_string()));
                }
                Ok(())
            }
            Err(Error::HttpClient(e)) => {
                // For an invalid or expired `arl`, the response has some
                // fields as integer `0` which are normally typed as string,
                // which causes JSON deserialization to fail.
                if e.is_decode() {
                    return Err(Error::Assertion(format!("{e}: please refresh your arl")));
                }
                Err(e.into())
            }
            Err(e) => Err(e),
        }
    }

    /// todo
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - no valid [`Url`] can be created out of the session data
    /// - the HTTP request fails
    /// - the HTTP response cannot be parsed as [JSON]
    pub async fn request<T>(
        &mut self,
        body: impl Into<reqwest::Body>,
        headers: Option<HeaderMap>,
    ) -> Result<gateway::Response<T>>
    where
        T: std::fmt::Debug + gateway::Method + for<'de> Deserialize<'de>,
    {
        // Get the API token from the user data or use an empty string.
        let api_token = self
            .user_data
            .as_ref()
            .map(|data| data.api_token.as_str())
            .unwrap_or_default();

        // Check the URL early to not needlessly hit the rate limiter.
        let url_str = format!(
            "{}?method={}&input={}&api_version={}&api_token={api_token}&cid={}",
            Self::GATEWAY_URL,
            T::METHOD,
            Self::GATEWAY_INPUT,
            Self::GATEWAY_VERSION,
            self.client_id,
        );
        let url = url_str.parse::<reqwest::Url>()?;
        let mut request = self.http_client.post(url, body);

        let request_headers = request.headers_mut();
        request_headers.try_insert(CONTENT_TYPE, Self::PLAIN_TEXT_CONTENT)?;

        // Add any headers that were passed in.
        if let Some(headers) = headers {
            request_headers.extend(headers);
        }

        let response = self.http_client.execute(request).await?;
        let result = response.json::<gateway::Response<T>>().await;

        let redacted = T::METHOD == gateway::get_arl::GetArl::METHOD;
        if let Ok(ref body) = result {
            if redacted {
                trace!("{}: {{ ... }}", T::METHOD);
            } else {
                trace!("{}: {body:#?}", T::METHOD);
            }
        }

        result.map_err(Into::into)
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        if let Some(data) = &self.user_data {
            return data.user.options.expiration_timestamp >= data.user.options.timestamp;
        }

        true
    }

    #[must_use]
    pub fn expires_at(&self) -> SystemTime {
        if let Some(data) = &self.user_data {
            return data.user.options.expiration_timestamp;
        }

        SystemTime::now()
    }

    pub fn set_user_data(&mut self, data: UserData) {
        self.user_data = Some(data);
    }

    #[must_use]
    pub fn user_data(&self) -> Option<&gateway::UserData> {
        self.user_data.as_ref()
    }

    /// The [`AudioQuality`] that the user has set for casting.
    pub fn audio_quality(&self) -> Option<AudioQuality> {
        self.user_data.as_ref().and_then(|data| {
            AudioQuality::from_str(&data.user.audio_settings.connected_device_streaming_preset).ok()
        })
    }

    pub async fn list_to_queue(&mut self, list: queue::List) -> Result<Queue> {
        let track_list = gateway::list_data::Request {
            track_ids: list
                .tracks
                .into_iter()
                .map(|track| track.id.parse())
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| Error::Assertion("track number must not be zero".to_string()))?,
        };

        let body = serde_json::to_string(&track_list)?;
        match self.request::<gateway::ListData>(body, None).await {
            Ok(response) => Ok(response.all().clone()),
            Err(e) => Err(e),
        }
    }

    pub async fn get_arl(&mut self, access_token: &str) -> Result<Arl> {
        let mut headers = HeaderMap::new();
        headers.try_insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {access_token}"))?,
        )?;

        let arl = self
            .request::<gateway::GetArl>(Self::EMPTY_JSON_OBJECT, Some(headers))
            .await
            .and_then(|response| {
                response
                    .first()
                    .map(|result| result.0.clone())
                    .ok_or_else(|| Error::Assertion("no arl received".to_string()))
            })?;

        arl.parse::<Arl>().map_err(Into::into)
    }

    pub async fn user_token(&mut self) -> std::result::Result<UserToken, UserTokenError> {
        if self.is_expired() {
            self.refresh().await?;
        }

        match &self.user_data {
            Some(data) => {
                if !data.gatekeeps.remote_control {
                    return Err(UserTokenError::PermissionDenied(
                        "remote control is disabled for this account".to_string(),
                    ));
                }
                if data.user.options.too_many_devices {
                    return Err(UserTokenError::PermissionDenied(
                        "too many devices; remove one or more in your account settings".to_string(),
                    ));
                }

                let expires_at = self.expires_at();
                Ok(UserToken {
                    user_id: data.user.id,
                    token: data.user_token.clone(),
                    expires_at,
                })
            }
            None => Err(UserTokenError::Provider("user data unavailable".into())),
        }
    }

    pub fn flush_user_token(&mut self) {
        // Force refreshing user data, but do not set `user_data` to `None` so
        // so we can continue using the `api_token` it contains.
        if let Some(ref mut data) = self.user_data {
            data.user.options.expiration_timestamp = SystemTime::now();
        }
    }
}

impl From<Error> for UserTokenError {
    fn from(e: Error) -> Self {
        Self::Provider(e.into())
    }
}

impl From<MaxSizeReached> for Error {
    fn from(e: MaxSizeReached) -> Self {
        Self::HttpHeader(e.to_string())
    }
}

impl From<InvalidHeaderValue> for Error {
    fn from(e: InvalidHeaderValue) -> Self {
        Self::HttpHeader(e.to_string())
    }
}

impl From<arl::Error> for Error {
    fn from(e: arl::Error) -> Self {
        Self::Assertion(e.to_string())
    }
}
