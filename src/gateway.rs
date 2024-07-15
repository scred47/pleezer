use std::{
    num::NonZeroU32,
    str::FromStr,
    sync::Arc,
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use governor::{
    clock::MonotonicClock,
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use rand::Rng;
use reqwest::{self, cookie::CookieStore};
use serde::Deserialize;
use sysinfo;
use thiserror::Error;

use crate::{
    arl::Arl,
    config::Config,
    protocol::{
        connect::AudioQuality,
        gateway::{self, UserData},
    },
    tokens::{UserToken, UserTokenError, UserTokenProvider},
};

#[derive(Debug)]
pub struct Gateway {
    client_id: usize,
    http_client: reqwest::Client,
    rate_limiter: RateLimiter<NotKeyed, InMemoryState, MonotonicClock, NoOpMiddleware>,
    user_data: Option<UserData>,
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("assertion failed: {0}")]
    Assertion(String),

    #[error("http client error: {0}")]
    HttpClient(#[from] reqwest::Error),

    #[error("parsing url failed: {0}")]
    UrlParse(#[from] url::ParseError),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Gateway {
    const RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(5);
    const RATE_LIMIT_CALLS_PER_INTERVAL: u8 = 50;

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - no valid `User-Agent` can be created out of the `config` fields
    /// - no valid OS name and/or version can be detected
    /// - no valid cookies can be created out of the `arl` and/or `config` fields
    pub fn new(config: &Config, arl: &Arl) -> Result<Self> {
        let app_name = &config.app_name;
        let app_version = &config.app_version;
        let app_lang = &config.app_lang;

        // Additional `User-Agent` string checks on top of `reqwest`.
        let illegal_chars = |chr| chr == '/' || chr == ';';
        if app_name.is_empty()
            || app_name.contains(illegal_chars)
            || app_version.is_empty()
            || app_version.contains(illegal_chars)
            || app_lang.chars().count() != 2
            || app_lang.contains(illegal_chars)
        {
            return Err(Error::Assertion(format!(
                "application name, version and/or language invalid (\"{app_name}\"; \"{app_version}\"; \"{app_lang}\")"
            )));
        }

        let os_name = match std::env::consts::OS {
            "macos" => "osx",
            other => other,
        };
        let os_version = sysinfo::System::os_version().unwrap_or_else(|| String::from("0"));
        if os_name.is_empty()
            || os_name.contains(illegal_chars)
            || os_version.is_empty()
            || os_version.contains(illegal_chars)
        {
            return Err(Error::Assertion(format!(
                "os name and/or version invalid (\"{os_name}\"; \"{os_version}\")"
            )));
        }

        // Set `User-Agent` to be served like Deezer on desktop.
        let user_agent =
            format!("{app_name}/{app_version} (Rust; {os_name}/{os_version}; Desktop; {app_lang})");
        debug!("user agent: {user_agent}");

        // `arl`s expire in about 190 days but users cannot simply copy & paste
        // the expiration from their browser into the `arl_file`, because there
        // they are displayed in human-readable and internationalized form.
        // Instead we will try to detect ARL expiration when API requests fail.
        let arl_cookie = format!("arl={arl}; Domain=deezer.com; Path=/; Secure; HttpOnly");
        let lang_cookie =
            format!("dz_lang={app_lang}; Domain=deezer.com; Path=/; Secure; HttpOnly");
        let cookie_origin =
            reqwest::Url::parse("https://www.deezer.com/desktop/login/electron/callback")?;

        // Create a new cookie jar and put the cookies in.
        let cookie_jar = reqwest::cookie::Jar::default();
        cookie_jar.add_cookie_str(&arl_cookie, &cookie_origin);
        cookie_jar.add_cookie_str(&lang_cookie, &cookie_origin);

        // The functions above are infallible. Check if the jar really contains
        // the cookies now.
        let cookie_check = cookie_jar.cookies(&cookie_origin);
        let cookie_count = cookie_check.map_or(0, |header_value| {
            header_value
                .to_str()
                .map_or(0, |result: &str| result.split(';').count())
        });
        if cookie_count != 2 {
            return Err(Error::Assertion(String::from("cookie count invalid")));
        }

        // `Arc` wrap the jar for use in a asynchronous context and build a
        // HTTP client with it.
        let cookie_jar = Arc::new(cookie_jar);
        let http_client = reqwest::Client::builder()
            .cookie_provider(Arc::clone(&cookie_jar))
            .tcp_keepalive(Duration::from_secs(60))
            .timeout(Duration::from_secs(60))
            .user_agent(user_agent)
            .build()?;

        // Rate limit own requests as to not DoS the Deezer infrastructure.
        let replenish_interval = Self::RATE_LIMIT_INTERVAL.as_secs_f32()
            / f32::from(Self::RATE_LIMIT_CALLS_PER_INTERVAL);
        let quota = Quota::with_period(Duration::from_secs_f32(replenish_interval))
            .ok_or_else(|| Error::Assertion("quota time interval is zero".to_string()))?
            .allow_burst(
                NonZeroU32::new(Self::RATE_LIMIT_CALLS_PER_INTERVAL.into())
                    .ok_or_else(|| Error::Assertion("calls per interval is zero".to_string()))?,
            );
        let rate_limiter = governor::RateLimiter::direct(quota);

        // Deezer on desktop uses a new `cid` on every start.
        let client_id = rand::thread_rng().gen_range(100_000_000..=999_999_999);
        debug!("client id: {client_id}");

        Ok(Self {
            client_id,
            http_client,
            rate_limiter,
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
        match self.request::<gateway::user_data::Response>("{}").await {
            Ok(response) => {
                let data = response.results;
                trace!("{data:#?}");
                self.set_user_data(data);
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
    pub async fn request<T>(&mut self, body: impl Into<reqwest::Body>) -> Result<T>
    where
        T: for<'a> gateway::Method<'a> + for<'de> Deserialize<'de>,
    {
        let method = format!("deezer.{}", T::METHOD);
        let api_token = self
            .user_data
            .as_ref()
            .map_or_else(String::new, |data| data.api_token.clone());
        let url_str = format!("https://www.deezer.com/ajax/gw-light.php?method={method}&input=3&api_version=1.0&api_token={api_token}&cid={}", self.client_id);

        // Check the URL early to not needlessly hit the rate limiter below.
        let url = url_str.parse::<reqwest::Url>()?;

        // No need to await with jitter because the level of concurrency is low.
        self.rate_limiter.until_ready().await;

        let response = self.http_client.post(url).body(body).send().await?;
        response.json::<T>().await.map_err(Into::into)
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

pub trait UserSettingsProvider {
    fn audio_quality(&self) -> Option<AudioQuality>;
}

impl UserSettingsProvider for Gateway {
    /// The [`AudioQuality`] that the user has set for casting.
    fn audio_quality(&self) -> Option<AudioQuality> {
        self.user_data.as_ref().and_then(|data| {
            AudioQuality::from_str(&data.user.audio_settings.connected_device_streaming_preset).ok()
        })
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
