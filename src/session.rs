use std::{
    num::NonZeroU32,
    sync::Arc,
    time::{Duration, Instant},
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
use sysinfo::{self, SystemExt};
use thiserror::Error;

use crate::{
    arl::Arl,
    config::Config,
    protocol::gateway::{self, UserData, UserDataResponse},
    token::{UserToken, UserTokenError, UserTokenProvider},
};

#[derive(Debug)]
pub struct Session {
    client_id: usize,
    http_client: reqwest::Client,
    rate_limiter: RateLimiter<NotKeyed, InMemoryState, MonotonicClock, NoOpMiddleware>,
    user_data: Option<UserData>,
    timestamp: Option<Instant>,
}

#[derive(Error, Debug)]
pub enum SessionError {
    #[error("assertion failed: {0}")]
    Assertion(String),
    #[error("http client error: {0}")]
    HttpClientError(#[from] reqwest::Error),
    #[error("parsing url failed: {0}")]
    UrlParseError(#[from] url::ParseError),
}

pub type SessionResult<T> = Result<T, SessionError>;

impl Session {
    pub fn new(config: &Config, arl: &Arl) -> SessionResult<Self> {
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
            return Err(SessionError::Assertion(format!(
                "application name, version and/or language invalid (\"{app_name}\"; \"{app_version}\"; \"{app_lang}\")"
            )));
        }

        let os_name = match std::env::consts::OS {
            "macos" => "osx",
            other => other,
        };
        let os_version = sysinfo::System::new()
            .os_version()
            .unwrap_or_else(|| String::from("0"));
        if os_name.is_empty()
            || os_name.contains(illegal_chars)
            || os_version.is_empty()
            || os_version.contains(illegal_chars)
        {
            return Err(SessionError::Assertion(format!(
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
            return Err(SessionError::Assertion(String::from(
                "cookie count invalid",
            )));
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
        const CALLS_PER_INTERVAL: usize = 50;
        let replenish_interval_ns = Duration::from_secs(5).as_nanos() / CALLS_PER_INTERVAL as u128;
        let quota = Quota::with_period(Duration::from_nanos(replenish_interval_ns as u64))
            .ok_or_else(|| SessionError::Assertion(String::from("quota time interval is zero")))?
            .allow_burst(NonZeroU32::new(CALLS_PER_INTERVAL as u32).ok_or_else(|| {
                SessionError::Assertion(String::from("calls per interval is zero"))
            })?);
        let rate_limiter = governor::RateLimiter::direct(quota);

        // Deezer on desktop uses a new `cid` on every start.
        let client_id = rand::thread_rng().gen_range(100000000..=999999999);
        debug!("client id: {client_id}");

        Ok(Self {
            client_id,
            http_client,
            rate_limiter,
            user_data: None,
            timestamp: None,
        })
    }

    pub async fn refresh(&mut self) -> SessionResult<UserData> {
        let timestamp = Instant::now();
        match self.request::<UserDataResponse>("{}").await {
            Ok(response) => {
                let user_data = self.set_user_data(response.results)?;

                self.timestamp = Some(timestamp);
                debug!(
                    "user data time to live: {} seconds",
                    self.time_to_live().as_secs()
                );

                Ok(user_data)
            }
            Err(SessionError::HttpClientError(e)) => {
                // For an invalid or expired `arl`, the response has some
                // fields as integer `0` which are normally typed as string,
                // which causes JSON deserialization to fail.
                if e.is_decode() {
                    return Err(SessionError::Assertion(format!("{e}: check your arl")));
                }
                Err(e.into())
            }
            Err(e) => Err(e),
        }
    }

    pub async fn request<T>(&mut self, body: impl Into<reqwest::Body>) -> SessionResult<T>
    where
        T: for<'a> gateway::Method<'a> + for<'de> Deserialize<'de>,
    {
        let method = format!("deezer.{}", T::METHOD);
        let api_token = self
            .user_data
            .as_ref()
            .map_or_else(|| String::from(""), |data| data.api_token.clone());
        let url_str = format!("https://www.deezer.com/ajax/gw-light.php?method={method}&input=3&api_version=1.0&api_token={api_token}&cid={}", self.client_id);

        // Check the URL early to not needlessly hit the rate limiter below.
        let url = url_str.parse::<reqwest::Url>()?;

        // No need to await with jitter because the level of concurrency is low.
        self.rate_limiter.until_ready().await;

        let response = self.http_client.post(url).body(body).send().await?;
        response.json::<T>().await.map_err(Into::into)
    }

    #[must_use]
    pub fn time_to_live(&self) -> Duration {
        self.user_data.as_ref().map_or(Duration::ZERO, |data| {
            let options = &data.user.options;

            // Account for clock skew between client and server.
            let time_to_live = Duration::from_secs(
                options
                    .expiration_timestamp
                    .saturating_sub(options.timestamp),
            );

            let timestamp = self.timestamp.unwrap_or_else(|| Instant::now());
            time_to_live.saturating_sub(timestamp.elapsed())
        })
    }

    #[must_use]
    pub fn expires_at(&self) -> Instant {
        let now = Instant::now();
        now.checked_add(self.time_to_live()).unwrap_or_else(|| now)
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.time_to_live() == Duration::ZERO
    }

    pub fn set_user_data(&mut self, data: UserData) -> SessionResult<gateway::UserData> {
        debug!("user id: {}", data.user.id);
        debug!("user plan: {}", data.plan);
        info!(
            "user casting quality: {}",
            data.user.audio_settings.connected_device_streaming_preset
        );
        self.user_data = Some(data.to_owned());

        Ok(data)
    }

    #[must_use]
    pub fn user_data(&self) -> Option<gateway::UserData> {
        self.user_data.clone()
    }
}

#[async_trait]
impl UserTokenProvider for Session {
    async fn user_token(&mut self) -> Result<UserToken, UserTokenError> {
        if self.is_expired() {
            self.refresh().await?;
        }

        let data = self
            .user_data
            .as_ref()
            .ok_or(UserTokenError::ProviderError(
                "user data unavailable".into(),
            ))?;
        if !data.gatekeeps.remote_control {
            return Err(UserTokenError::PermissionDenied(format!(
                "remote control is disabled for this account",
            )));
        }
        if data.user.options.too_many_devices {
            return Err(UserTokenError::PermissionDenied(format!(
                "too many devices; remove one or more in your account settings",
            )));
        }

        let expires_at = self.expires_at();
        UserToken::new(data.user.id, &data.user_token, expires_at)
    }

    fn flush_user_token(&mut self) {
        // Force refreshing user data, but do not set `user_data` to `None` so
        // so we can continue using the `api_token` it contains.
        self.timestamp = None;
    }
}

impl From<SessionError> for UserTokenError {
    fn from(e: SessionError) -> Self {
        Self::ProviderError(e.into())
    }
}
