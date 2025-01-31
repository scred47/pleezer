use std::{num::NonZeroU32, sync::Arc, time::Duration};

use governor::{DefaultDirectRateLimiter, Quota};
use http::header::CONTENT_TYPE;
use reqwest::{
    self,
    header::{HeaderValue, ACCEPT_LANGUAGE},
    Body, Method, Url,
};
use trust_dns_resolver::{
    config::{ResolverConfig, ResolverOpts},
    TokioAsyncResolver,
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

    /// Duration to wait for connection establishment.
    const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

    /// Duration to wait for individual network reads.
    ///
    /// Reads that take longer than 5 seconds will timeout to:
    /// * Prevent blocking operations
    /// * Allow faster recovery from network issues
    /// * Maintain responsive streaming
    ///
    /// The timeout needs to be long enough to allow for slow hardware and network conditions.
    const READ_TIMEOUT: Duration = Duration::from_secs(5);

    /// Content type for plain text requests.
    ///
    /// Used by `text()` method to set Content-Type header to "text/plain;charset=UTF-8"
    const CONTENT_TYPE_TEXT: HeaderValue = HeaderValue::from_static("text/plain;charset=UTF-8");

    /// Content type for JSON requests.
    ///
    /// Used by `json()` method to set Content-Type header to "application/json"
    const CONTENT_TYPE_JSON: HeaderValue = HeaderValue::from_static("application/json");

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

        // Configure the DNS resolver to only look up IPv4 addresses
        let resolver_config = ResolverConfig::from_parts(
            None,
            vec![],
            ResolverOpts {
                ipv4_only: true,
                ..Default::default()
            },
        );
        let resolver = TokioAsyncResolver::tokio_from_config(resolver_config)?;

        let mut http_client = reqwest::Client::builder()
            .tcp_keepalive(Self::KEEPALIVE_TIMEOUT)
            .connect_timeout(Self::CONNECT_TIMEOUT)
            .read_timeout(Self::READ_TIMEOUT)
            .default_headers(headers)
            .user_agent(&config.user_agent)
            .resolver(resolver);

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

    /// Builds a POST request with plain text body.
    ///
    /// Convenience method for `request()` with POST method.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL
    /// * `body` - Request body content
    #[inline]
    pub fn text<U, T>(&self, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        let mut request = self.request(Method::POST, url, body);
        request
            .headers_mut()
            .insert(CONTENT_TYPE, Self::CONTENT_TYPE_TEXT);
        request
    }

    /// Builds a POST request with JSON body.
    ///
    /// Convenience method for `request()` with POST method.
    ///
    /// # Arguments
    ///
    /// * `url` - Request URL
    /// * `body` - Request body content
    #[inline]
    pub fn json<U, T>(&self, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        let mut request = self.request(Method::POST, url, body);
        request
            .headers_mut()
            .insert(CONTENT_TYPE, Self::CONTENT_TYPE_JSON);
        request
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
    /// comply with API quotas. Automatically verifies that the response
    /// has a successful HTTP status code (2xx range).
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
    /// * Response status code is not successful (not 2xx)
    /// * Network error occurs
    pub async fn execute(&self, request: reqwest::Request) -> Result<reqwest::Response> {
        // No need to await with jitter because the level of concurrency is low.
        // TODO : use different rate limiter for each host.
        self.rate_limiter.until_ready().await;
        match self.unlimited.execute(request).await {
            Ok(response) => response.error_for_status().map_err(Into::into),
            Err(e) => Err(e.into()),
        }
    }
}

/// Builder for `Client`.
pub struct ClientBuilder {
    config: Config,
    cookie_jar: Option<reqwest_cookie_store::CookieStore>,
    keepalive_timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    read_timeout: Option<Duration>,
}

impl ClientBuilder {
    /// Creates a new `ClientBuilder` with default settings.
    pub fn new(config: Config) -> Self {
        Self {
            config,
            cookie_jar: None,
            keepalive_timeout: None,
            connect_timeout: None,
            read_timeout: None,
        }
    }

    /// Sets the cookie jar for session management.
    pub fn cookie_jar(mut self, cookie_jar: reqwest_cookie_store::CookieStore) -> Self {
        self.cookie_jar = Some(cookie_jar);
        self
    }

    /// Sets the keepalive timeout.
    pub fn keepalive_timeout(mut self, timeout: Duration) -> Self {
        self.keepalive_timeout = Some(timeout);
        self
    }

    /// Sets the connect timeout.
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Sets the read timeout.
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = Some(timeout);
        self
    }

    /// Builds the `Client`.
    pub fn build(self) -> Result<Client> {
        let keepalive_timeout = self.keepalive_timeout.unwrap_or(Client::KEEPALIVE_TIMEOUT);
        let connect_timeout = self.connect_timeout.unwrap_or(Client::CONNECT_TIMEOUT);
        let read_timeout = self.read_timeout.unwrap_or(Client::READ_TIMEOUT);

        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(lang) = HeaderValue::from_str(&self.config.app_lang) {
            headers.insert(ACCEPT_LANGUAGE, lang);
        }

        let cookie_jar = self
            .cookie_jar
            .map(|jar| Arc::new(reqwest_cookie_store::CookieStoreMutex::new(jar)));

        // Configure the DNS resolver to only look up IPv4 addresses
        let resolver_config = ResolverConfig::from_parts(
            None,
            vec![],
            ResolverOpts {
                ipv4_only: true,
                ..Default::default()
            },
        );
        let resolver = TokioAsyncResolver::tokio_from_config(resolver_config)?;

        let mut http_client = reqwest::Client::builder()
            .tcp_keepalive(keepalive_timeout)
            .connect_timeout(connect_timeout)
            .read_timeout(read_timeout)
            .default_headers(headers)
            .user_agent(&self.config.user_agent)
            .resolver(resolver);

        if let Some(ref jar) = cookie_jar {
            http_client = http_client.cookie_provider(Arc::clone(jar));
        }

        let replenish_interval =
            Client::RATE_LIMIT_INTERVAL / u32::from(Client::RATE_LIMIT_CALLS_PER_INTERVAL);
        let quota = Quota::with_period(replenish_interval)
            .expect("quota time interval is zero")
            .allow_burst(
                NonZeroU32::new(Client::RATE_LIMIT_CALLS_PER_INTERVAL.into())
                    .expect("calls per interval is zero"),
            );

        Ok(Client {
            unlimited: http_client.build()?,
            rate_limiter: governor::RateLimiter::direct(quota),
            cookie_jar,
        })
    }
}
