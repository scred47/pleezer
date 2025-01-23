//! HTTP client with rate limiting and session management for Deezer APIs.
//!
//! This module provides a wrapper around `reqwest::Client` that adds:
//! * Cookie-based session management for authentication
//! * Persistent login across client restarts
//! * Request rate limiting to respect API quotas
//! * Consistent timeouts and headers
//!
//! # Session Management
//!
//! Handles authentication cookies for:
//! * ARL tokens for initial authentication
//! * Refresh tokens for session renewal
//! * Language preferences
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
//! // Create client with session management for authenticated endpoints
//! let client = Client::with_cookies(&config, cookie_jar)?;
//!
//! // Or without session management for public endpoints
//! let client = Client::without_cookies(&config)?;
//!
//! // Make authenticated requests
//! let request = client.post(auth_url, credentials);
//! let response = client.execute(request).await?;
//!
//! // Cookies are automatically managed for session persistence
//! ```

use std::{future::Future, num::NonZeroU32, sync::Arc, time::Duration};

use futures_util::{FutureExt, TryFutureExt};
use governor::{DefaultDirectRateLimiter, Quota};
use reqwest::{
    self,
    header::{HeaderValue, ACCEPT_LANGUAGE},
    Body, Method, Url,
};

use crate::{config::Config, error::Result};

/// HTTP client with session management and rate limiting.
///
/// Wraps `reqwest::Client` to provide:
/// * Cookie-based session persistence
/// * Rate limiting for API quotas
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

    /// Cookie store for session management.
    ///
    /// Stores authentication tokens and preferences:
    /// * ARL tokens for authentication
    /// * Refresh tokens for session renewal
    /// * Language settings
    ///
    /// Optional to support both authenticated and public endpoints.
    pub cookie_jar: Option<Arc<reqwest_cookie_store::CookieStoreMutex>>,
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

    /// Creates a new client with optional session management.
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration including user agent and language
    /// * `cookie_jar` - Optional cookie store for session management
    ///
    /// # Session Management
    ///
    /// If a cookie store is provided, the client will:
    /// * Store authentication tokens
    /// * Maintain persistent login
    /// * Handle automatic session renewal
    /// * Preserve language preferences
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
    pub fn new(
        config: &Config,
        cookie_jar: Option<reqwest_cookie_store::CookieStore>,
    ) -> Result<Self> {
        // Not having `Accept-Language` set is non-fatal.
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(lang) = HeaderValue::from_str(&config.app_lang) {
            headers.insert(ACCEPT_LANGUAGE, lang);
        }

        // Wrap any `cookie_jar` in an `Arc` for asynchronous use.
        let cookie_jar =
            cookie_jar.map(|jar| Arc::new(reqwest_cookie_store::CookieStoreMutex::new(jar)));

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
            cookie_jar,
        })
    }

    /// Creates a new client with session management.
    ///
    /// Convenience method for authenticated endpoints that require:
    /// * Cookie-based session persistence
    /// * Token storage and renewal
    /// * Language preference management
    ///
    /// # Arguments
    ///
    /// * `config` - Client configuration
    /// * `cookie_jar` - Cookie store for session management
    ///
    /// # Errors
    ///
    /// Returns error if client creation fails.
    pub fn with_cookies(
        config: &Config,
        cookie_jar: reqwest_cookie_store::CookieStore,
    ) -> Result<Self> {
        Self::new(config, Some(cookie_jar))
    }

    /// Creates a new client without session management.
    ///
    /// Convenience method for public endpoints that:
    /// * Don't require authentication
    /// * Access public resources (e.g., CDN)
    /// * Don't need persistent sessions
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
        Self::new(config, None)
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
    #[inline]
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
    #[inline]
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
    #[inline]
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
