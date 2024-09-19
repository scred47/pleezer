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

/// A `reqwest::Client` wrapper with rate limiting and cookie support.
pub struct Client {
    /// The inner `reqwest::Client` instance.
    pub inner: reqwest::Client,

    /// A rate limiter to prevent Denial of Service attacks.
    rate_limiter: DefaultDirectRateLimiter,

    /// An optional cookie jar to store session cookies.
    pub cookie_jar: Option<Arc<dyn CookieStore>>,
}

impl Client {
    /// The rate limit interval.
    ///
    /// This is the last known value from the Deezer API documentation.
    const RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(5);

    /// The rate limit calls per interval.
    ///
    /// This is the last known value from the Deezer API documentation.
    const RATE_LIMIT_CALLS_PER_INTERVAL: u8 = 50;

    /// The request timeout.
    const TIMEOUT: Duration = Duration::from_secs(60);

    /// Creates a new `Client` with the given `Config` and optional
    /// `CookieStore`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the `reqwest::Client` cannot be built.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::http::Client;
    /// use deezer::config::Config;
    ///
    /// let config = Config::default();
    /// let client = Client::new(&config, None).unwrap();
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if the rate limit interval is zero or the calls per interval
    /// is zero.
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
            .tcp_keepalive(Self::TIMEOUT)
            .timeout(Self::TIMEOUT)
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
            inner: http_client.build()?,
            rate_limiter: governor::RateLimiter::direct(quota),
            cookie_jar: cookie_jar.map(|jar| jar as _), // coerce compiler to infer type
        })
    }

    /// Creates a new `Client` with the given `Config` and a `CookieStore`.
    ///
    /// # Errors
    ///
    /// Will return `Err` if the `reqwest::Client` cannot be built.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::http::Client;
    /// use deezer::config::Config;
    /// use reqwest::cookie::Jar;
    ///
    /// let config = Config::default();
    /// let cookie_jar = Jar::default();
    /// let client = Client::with_cookies(&config, cookie_jar).unwrap();
    /// ```
    pub fn with_cookies<C>(config: &Config, cookie_jar: C) -> Result<Self>
    where
        C: CookieStore + 'static,
    {
        Self::new(config, Some(cookie_jar))
    }

    /// Creates a new `Client` with the given `Config` and no cookies.
    ///
    /// This is useful for public endpoints that don't require authentication,
    /// such as the Deezer Content Delivery Network (CDN).
    ///
    /// # Errors
    ///
    /// Will return `Err` if the `reqwest::Client` cannot be built.
    ///
    /// # Example
    ///
    /// ```rust
    /// use deezer_metadata::http::Client;
    /// use deezer_metadata::config::Config;
    ///
    /// let config = Config::default();
    /// let client = Client::without_cookies(&config).unwrap();
    ///
    /// // Use the client to make requests...
    /// ```
    pub fn without_cookies(config: &Config) -> Result<Self> {
        // Need to specify a type that satisfies the trait bounds.
        Self::new(config, None::<reqwest::cookie::Jar>)
    }

    /// Builds a `reqwest::Request` with the given `Method`, `Url`, and `Body`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::http::Client;
    /// use reqwest::Method;
    /// use reqwest::Url;
    ///
    /// let client = Client::without_cookies(&config).unwrap();
    /// let url = Url::parse("https://api.deezer.com/track/3135556").unwrap();
    /// let request = client.request(Method::GET, url, None);
    ///
    /// // Use the request to make a call...
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

    /// Builds a `POST` request with the given `Url` and `Body`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::http::Client;
    /// use reqwest::Url;
    ///
    /// let client = Client::without_cookies(&config).unwrap();
    /// let url = Url::parse("https://api.deezer.com/track/3135556").unwrap();
    /// let request = client.post(url, None);
    ///
    /// // Execute the request...
    /// let response = client.execute(request).await.unwrap();
    /// ```
    pub fn post<U, T>(&self, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        self.request(Method::POST, url, body)
    }

    /// Builds a `GET` request with the given `Url` and `Body`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::http::Client;
    /// use reqwest::Url;
    ///
    /// let client = Client::without_cookies(&config).unwrap();
    /// let url = Url::parse("https://api.deezer.com/track/3135556").unwrap();
    /// let request = client.get(url, None);
    ///
    /// // Execute the request...
    /// let response = client.execute(request).await.unwrap();
    /// ```
    pub fn get<U, T>(&self, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        self.request(Method::GET, url, body)
    }

    /// Executes the given `Request` asynchronously.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use deezer::http::Client;
    /// use reqwest::Method;
    /// use reqwest::Url;
    ///
    /// let client = Client::without_cookies(&config).unwrap();
    /// let url = Url::parse("https://api.deezer.com/track/3135556").unwrap();
    /// let request = client.request(Method::GET, url, None);
    ///
    /// // Execute the request...
    /// let response = client.execute(request).await.unwrap();
    /// ```
    ///
    /// # Errors
    ///
    /// Will return `Err` if the request cannot be executed.
    pub fn execute(
        &self,
        request: reqwest::Request,
    ) -> impl Future<Output = Result<reqwest::Response>> + '_ {
        // No need to await with jitter because the level of concurrency is low.
        let throttle = self.rate_limiter.until_ready();
        throttle.then(|()| self.inner.execute(request).map_err(Into::into))
    }
}
