//! HTTP client with rate limiting and cookie management for Deezer APIs.
//!
//! This module provides a wrapper around `reqwest::Client` that adds:
//! * Request rate limiting to respect Deezer's API quotas
//! * Cookie management for authentication
//! * Consistent timeouts and headers
//!
//! # Rate Limiting
//!
//! Implements Deezer's rate limits:
//! * 50 calls per 5-second interval
//! * Automatic request throttling
//! * Allows bursts up to the maximum calls per interval
//! * Requests that would exceed the limit are delayed
//!
//! # Example
//!
//! ```rust
//! use pleezer::http::Client;
//! use reqwest::Url;
//!
//! // Create client with cookies for authenticated endpoints
//! let client = Client::with_cookies(&config, cookie_jar)?;
//!
//! // Or without cookies for public endpoints
//! let client = Client::without_cookies(&config)?;
//!
//! // Make rate-limited requests
//! let request = client.get(url, body);
//! let response = client.execute(request).await?;
//! ```

use std::{future::Future, num::NonZeroU32, sync::Arc, time::Duration};

use futures_util::{FutureExt, TryFutureExt};
use governor::{DefaultDirectRateLimiter, Quota};
use reqwest::{
    self,
    cookie::CookieStore,
    header::{HeaderValue, ACCEPT_LANGUAGE},
    Body, Method, Url,
};

use crate::{config::Config, error::Result};

/// HTTP client with built-in rate limiting and cookie support.
///
/// Wraps `reqwest::Client` to provide:
/// * Rate limiting for API quotas
/// * Optional cookie storage
/// * Consistent configuration
pub struct Client {
    /// Unlimited request client for special cases.
    ///
    /// Direct access to underlying client without rate limiting.
    pub unlimited: reqwest::Client,

    /// Rate limiter for API quota compliance.
    ///
    /// Implements Deezer's 50 calls per 5-second limit.
    rate_limiter: DefaultDirectRateLimiter,

    /// Cookie storage for authentication.
    ///
    /// Optional to support both authenticated and public endpoints.
    pub cookie_jar: Option<Arc<dyn CookieStore>>,
}

impl Client {
    /// Standard rate limit interval for Deezer's API.
    ///
    /// The API enforces a rolling window of 5 seconds during which
    /// a maximum number of calls can be made.
    const RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(5);

    /// Maximum allowed API calls per interval.
    ///
    /// Deezer's API allows up to 50 calls within each 5-second window.
    /// Requests beyond this limit will be automatically delayed.
    const RATE_LIMIT_CALLS_PER_INTERVAL: u8 = 50;

    /// Duration to keep idle connections alive.
    ///
    /// Prevents frequent reconnection overhead for subsequent requests.
    const KEEPALIVE_TIMEOUT: Duration = Duration::from_secs(60);

    /// Duration to wait for individual network reads.
    ///
    /// Reads that take longer than 2 seconds will timeout to:
    /// * Prevent blocking operations
    /// * Allow faster recovery from network issues
    /// * Maintain responsive streaming
    const READ_TIMEOUT: Duration = Duration::from_secs(2);

    /// Creates a new client with optional cookie storage.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration including user agent and language
    /// * `cookie_jar` - Optional cookie storage implementation
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * HTTP client creation fails
    /// * Header values are invalid
    ///
    /// # Panics
    ///
    /// Panics if rate limit parameters are zero.
    pub fn new<C>(config: &Config, cookie_jar: Option<C>) -> Result<Self>
    where
        C: CookieStore + 'static,
    {
        // Not having `Accept-Language` set is non-fatal.
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(lang) = HeaderValue::from_str(&config.app_lang) {
            headers.insert(ACCEPT_LANGUAGE, lang);
        }

        // Wrap `cookie_jar` in an `Arc` for asynchronous use.
        let cookie_jar = cookie_jar.map(|jar| Arc::new(jar));

        let mut http_client = reqwest::Client::builder()
            .tcp_keepalive(Self::KEEPALIVE_TIMEOUT)
            .read_timeout(Self::READ_TIMEOUT)
            .default_headers(headers)
            .user_agent(&config.user_agent);

        if let Some(ref jar) = cookie_jar {
            http_client = http_client.cookie_provider(Arc::clone(jar));
        }

        // Rate limit own requests as to not DoS the Deezer infrastructure.
        let replenish_interval =
            Self::RATE_LIMIT_INTERVAL / u32::from(Self::RATE_LIMIT_CALLS_PER_INTERVAL);
        let quota = Quota::with_period(replenish_interval)
            .expect("quota time interval is zero")
            .allow_burst(
                NonZeroU32::new(Self::RATE_LIMIT_CALLS_PER_INTERVAL.into())
                    .expect("calls per interval is zero"),
            );

        Ok(Self {
            unlimited: http_client.build()?,
            rate_limiter: governor::RateLimiter::direct(quota),
            cookie_jar: cookie_jar.map(|jar| jar as _), // coerce compiler to infer type
        })
    }

    /// Creates a new client with cookie storage.
    ///
    /// Convenience method for authenticated endpoints that
    /// require cookie-based session management.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration
    /// * `cookie_jar` - Cookie storage implementation
    ///
    /// # Errors
    ///
    /// Returns error if client creation fails.
    pub fn with_cookies<C>(config: &Config, cookie_jar: C) -> Result<Self>
    where
        C: CookieStore + 'static,
    {
        Self::new(config, Some(cookie_jar))
    }

    /// Creates a new client without cookie storage.
    ///
    /// Convenience method for public endpoints that don't
    /// require authentication (e.g., CDN access).
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration
    ///
    /// # Errors
    ///
    /// Returns error if client creation fails.
    pub fn without_cookies(config: &Config) -> Result<Self> {
        // Need to specify a type that satisfies the trait bounds.
        Self::new(config, None::<reqwest::cookie::Jar>)
    }

    /// Builds a request with specified method, URL and body.
    ///
    /// Creates a raw request that can be executed with `execute()`.
    ///
    /// # Arguments
    ///
    /// * `method` - HTTP method to use
    /// * `url` - Request URL
    /// * `body` - Request body content
    ///
    /// # Examples
    ///
    /// ```rust
    /// let request = client.request(
    ///     Method::POST,
    ///     "https://api.deezer.com/user/me",
    ///     json,
    /// );
    /// let response = client.execute(request).await?;
    /// ```
    pub fn request<U, T>(&self, method: Method, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        let mut request = reqwest::Request::new(method, url.into());
        let body_mut = request.body_mut();
        *body_mut = Some(body.into());

        request
    }

    /// Builds a POST request.
    ///
    /// Convenience method for `request()` with POST method.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL
    /// * `body` - Request body content
    pub fn post<U, T>(&self, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        self.request(Method::POST, url, body)
    }

    /// Builds a GET request.
    ///
    /// Convenience method for `request()` with GET method.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL
    /// * `body` - Request body content (usually empty)
    pub fn get<U, T>(&self, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        self.request(Method::GET, url, body)
    }

    /// Executes a request with rate limiting.
    ///
    /// Applies rate limiting before executing the request to
    /// comply with API quotas.
    ///
    /// # Arguments
    ///
    /// * `request` - Request to execute
    ///
    /// # Errors
    ///
    /// Returns error if:
    /// * Rate limiting fails
    /// * Request execution fails
    /// * Network error occurs
    pub fn execute(
        &self,
        request: reqwest::Request,
    ) -> impl Future<Output = Result<reqwest::Response>> + '_ {
        // No need to await with jitter because the level of concurrency is low.
        // TODO : use different rate limiter for each host.
        let throttle = self.rate_limiter.until_ready();
        throttle.then(|()| self.unlimited.execute(request).map_err(Into::into))
    }
}
