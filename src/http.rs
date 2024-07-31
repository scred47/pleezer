use std::{future::Future, num::NonZeroU32, sync::Arc, time::Duration};

use futures_util::FutureExt;
use governor::{DefaultDirectRateLimiter, Quota};
use reqwest::{
    self,
    cookie::CookieStore,
    header::{HeaderValue, ACCEPT_LANGUAGE},
    Body, Method, Url,
};

use crate::config::Config;

pub type Result<T> = std::result::Result<T, reqwest::Error>;

pub struct Client {
    inner: reqwest::Client,
    rate_limiter: DefaultDirectRateLimiter,
    pub cookie_jar: Option<Arc<dyn CookieStore>>,
}

impl Client {
    const RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(5);
    const RATE_LIMIT_CALLS_PER_INTERVAL: u8 = 50;
    const TIMEOUT: Duration = Duration::from_secs(60);

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

    pub fn with_cookies<C>(config: &Config, cookie_jar: C) -> Result<Self>
    where
        C: CookieStore + 'static,
    {
        Self::new(config, Some(cookie_jar))
    }

    pub fn without_cookies(config: &Config) -> Result<Self> {
        // Need to specify a type that satisfies the trait bounds.
        Self::new(config, None::<reqwest::cookie::Jar>)
    }

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

    pub fn post<U, T>(&self, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        self.request(Method::POST, url, body)
    }

    pub fn get<U, T>(&self, url: U, body: T) -> reqwest::Request
    where
        U: Into<Url>,
        T: Into<Body>,
    {
        self.request(Method::GET, url, body)
    }

    pub fn execute(
        &self,
        request: reqwest::Request,
    ) -> impl Future<Output = Result<reqwest::Response>> + '_ {
        // No need to await with jitter because the level of concurrency is low.
        let throttle = self.rate_limiter.until_ready();
        throttle.then(|()| self.inner.execute(request))
    }
}
