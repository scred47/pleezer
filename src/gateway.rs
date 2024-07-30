use std::{str::FromStr, sync::Arc, time::SystemTime};

use async_trait::async_trait;
use rand::Rng;
use reqwest::{
    self,
    cookie::CookieStore,
    header::{HeaderValue, CONTENT_TYPE},
};
use serde::Deserialize;
use thiserror::Error;

use crate::{
    arl::Arl,
    config::Config,
    http::Client as HttpClient,
    protocol::{
        connect::{queue, AudioQuality},
        gateway::{self, Queue, UserData},
    },
    tokens::{UserToken, UserTokenError, UserTokenProvider},
};

#[derive(Debug)]
pub struct Gateway {
    client_id: usize,
    http_client: HttpClient,
    user_data: Option<UserData>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("assertion failed: {0}")]
    Assertion(String),

    #[error("HTTP client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("parsing JSON error: {0}")]
    JsonParse(#[from] serde_json::Error),

    #[error("parsing URL failed: {0}")]
    UrlParse(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Gateway {
    const EMPTY_JSON_BODY: &'static str = "{}";

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - no valid `User-Agent` can be created out of the `config` fields
    /// - no valid OS name and/or version can be detected
    /// - no valid cookies can be created out of the `arl` and/or `config` fields
    pub fn new(config: &Config, arl: &Arl) -> Result<Self> {
        // Create a new cookie jar and put the cookies in.
        let cookie_jar = reqwest::cookie::Jar::default();
        let cookie_origin =
            reqwest::Url::parse("https://www.deezer.com/desktop/login/electron/callback")?;

        // `arl`s expire in about 190 days but users cannot simply copy & paste
        // the expiration from their browser into the `arl_file`, because there
        // they are displayed in human-readable and internationalized form.
        // Instead we will try to detect ARL expiration when API requests fail.
        let arl_cookie = format!("arl={arl}; Domain=deezer.com; Path=/; Secure; HttpOnly");
        cookie_jar.add_cookie_str(&arl_cookie, &cookie_origin);

        let lang_cookie = format!(
            "dz_lang={}; Domain=deezer.com; Path=/; Secure; HttpOnly",
            &config.app_lang
        );
        cookie_jar.add_cookie_str(&lang_cookie, &cookie_origin);

        // The function results above are infallible, but can reject invalid
        // cookies nonetheless. Check if the jar really contains our cookies.
        let cookie_check = cookie_jar.cookies(&cookie_origin);
        let cookie_count = cookie_check.map_or(0, |header_value| {
            header_value
                .to_str()
                .map_or(0, |result: &str| result.split(';').count())
        });
        if cookie_count != 2 {
            return Err(Error::Assertion("cookie count invalid".to_string()));
        }

        // `Arc` wrap the jar for use in a asynchronous context and build a
        // HTTP client with it.
        let cookie_jar = Arc::new(cookie_jar);
        let http_client = HttpClient::new(config, Some(cookie_jar))?;

        // Deezer on desktop uses a new `cid` on every start.
        let client_id = rand::thread_rng().gen_range(100_000_000..=999_999_999);
        debug!("client id: {client_id}");

        Ok(Self {
            client_id,
            http_client,
            user_data: None,
        })
    }

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - the `arl` is invalid or expired
    /// - the HTTP request failed
    pub async fn refresh(&mut self) -> Result<()> {
        match self
            .request::<gateway::UserData>(Self::EMPTY_JSON_BODY)
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
    ) -> Result<gateway::Response<T>>
    where
        T: std::fmt::Debug + gateway::Method + for<'de> Deserialize<'de>,
    {
        let api_token = self
            .user_data
            .as_ref()
            .map_or_else(String::new, |data| data.api_token.clone());
        let url_str = format!("https://www.deezer.com/ajax/gw-light.php?method={}&input=3&api_version=1.0&api_token={api_token}&cid={}", T::METHOD, self.client_id);

        // Check the URL early to not needlessly hit the rate limiter.
        let url = url_str.parse::<reqwest::Url>()?;
        let mut request = self.http_client.post(url, body);

        // Although all gateway requests are JSON, the `Content-Type` is not.
        let headers = request.headers_mut();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_static("text/plain;charset=UTF-8"),
        );

        let response = self.http_client.execute(request).await?;
        let result = response
            .json::<gateway::Response<T>>()
            .await
            .map_err(Into::into);

        if let Ok(ref body) = result {
            trace!("{}: {body:#?}", T::METHOD);
        }

        result
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
}

// TODO : move into Gateway
#[async_trait]
pub trait UserSettingsProvider {
    fn audio_quality(&self) -> Option<AudioQuality>;
    async fn list_to_queue(&mut self, list: queue::List) -> Result<Queue>;
}

#[async_trait]
impl UserSettingsProvider for Gateway {
    /// The [`AudioQuality`] that the user has set for casting.
    fn audio_quality(&self) -> Option<AudioQuality> {
        self.user_data.as_ref().and_then(|data| {
            AudioQuality::from_str(&data.user.audio_settings.connected_device_streaming_preset).ok()
        })
    }

    async fn list_to_queue(&mut self, list: queue::List) -> Result<Queue> {
        let track_list = gateway::list_data::Request {
            track_ids: list
                .tracks
                .into_iter()
                .map(|track| track.id.parse())
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(|_| Error::Assertion("track number must not be zero".to_string()))?,
        };

        let body = serde_json::to_string(&track_list)?;
        match self.request::<gateway::ListData>(body).await {
            Ok(response) => Ok(response.all().clone()),
            Err(e) => Err(e),
        }
    }
}

#[async_trait]
impl UserTokenProvider for Gateway {
    async fn user_token(&mut self) -> std::result::Result<UserToken, UserTokenError> {
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

    fn flush_user_token(&mut self) {
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
