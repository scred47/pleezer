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

#[derive(Debug)]
pub struct Client {
    inner: reqwest::Client,
    rate_limiter: DefaultDirectRateLimiter,
}

impl Client {
    const RATE_LIMIT_INTERVAL: Duration = Duration::from_secs(5);
    const RATE_LIMIT_CALLS_PER_INTERVAL: u8 = 50;

    const TIMEOUT: Duration = Duration::from_secs(60);

    pub fn new<C>(config: &Config, cookie_jar: Option<Arc<C>>) -> Result<Self>
    where
        C: CookieStore + 'static,
    {
        // Not having `Accept-Language` set is non-fatal.
        let mut headers = reqwest::header::HeaderMap::new();
        if let Ok(lang) = HeaderValue::from_str(&config.app_lang) {
            headers.insert(ACCEPT_LANGUAGE, lang);
        }

        let mut http_client = reqwest::Client::builder()
            .tcp_keepalive(Self::TIMEOUT)
            .timeout(Self::TIMEOUT)
            .default_headers(headers)
            .user_agent(&config.user_agent);

        if let Some(jar) = cookie_jar {
            http_client = http_client.cookie_provider(Arc::clone(&jar));
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
        })
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
