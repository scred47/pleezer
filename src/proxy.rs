// Adapted from https://chuxi.github.io/posts/websocket/ by chuxi

use std::{env, fmt::Display, str::FromStr};

use base64::prelude::*;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};
use url::{Position, Url};
use veil::Redact;

use crate::error::{Error, Result};

/// The configuration for an HTTP proxy.
#[derive(Redact, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Http {
    /// The authentication credentials for the proxy.
    #[redact]
    auth: Option<Vec<u8>>,

    /// The URL of the proxy.
    url: String,
}

impl Http {
    /// Create a new HTTP proxy configuration from the `HTTPS_PROXY` environment variable.
    ///
    /// This function will return `None` if the environment variable is not set or the proxy URL
    /// could not be parsed.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let proxy = env::var("HTTPS_PROXY")
            .or_else(|_| env::var("https_proxy"))
            .ok();

        proxy.and_then(|proxy| proxy.parse().ok())
    }

    /// Connect to a target host through the proxy.
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - the target URL could not be parsed
    /// - the TCP stream to the proxy could not be established
    /// - the tunnel to the target host could not be established
    pub async fn connect_async(&self, target: &str) -> Result<TcpStream> {
        let target_url = Url::parse(target)?;
        let host = target_url
            .host_str()
            .ok_or(Error::invalid_argument("target host not available"))?;
        let port = target_url.port().unwrap_or(443);
        let tcp_stream = TcpStream::connect(&self.url).await?;
        Self::tunnel(tcp_stream, host, port, &self.auth).await
    }

    /// Open a tunnel to the target host through the proxy.
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - the connection to the proxy could not be established
    /// - the authentication to the proxy failed
    /// - the response from the proxy could not be read
    /// - the response from the target host could not be read
    async fn tunnel(
        mut conn: TcpStream,
        host: &str,
        port: u16,
        auth: &Option<Vec<u8>>,
    ) -> Result<TcpStream> {
        let mut buf = format!(
            "\
         CONNECT {host}:{port} HTTP/1.1\r\n\
         Host: {host}:{port}\r\n\
         "
        )
        .into_bytes();

        if let Some(au) = auth {
            buf.extend_from_slice(b"Proxy-Authorization: ");
            buf.extend_from_slice(au.as_slice());
            buf.extend_from_slice(b"\r\n");
        }

        buf.extend_from_slice(b"\r\n");
        conn.write_all(&buf).await?;

        let mut buf = [0; 1024];
        let mut pos = 0;

        loop {
            let n = conn.read(&mut buf[pos..]).await?;
            if n == 0 {
                return Err(Error::data_loss("0 bytes in reading tunnel"));
            }
            pos += n;

            let recvd = &buf[..pos];
            if recvd.starts_with(b"HTTP/1.1 200") || recvd.starts_with(b"HTTP/1.0 200") {
                if recvd.ends_with(b"\r\n\r\n") {
                    return Ok(conn);
                }
                if pos == buf.len() {
                    return Err(Error::data_loss("proxy headers too long for tunnel"));
                }
            } else if recvd.starts_with(b"HTTP/1.1 407") {
                return Err(Error::permission_denied("proxy authentication required"));
            } else {
                return Err(Error::unknown("unsuccessful tunnel"));
            }
        }
    }
}

impl FromStr for Http {
    type Err = Error;

    /// Create a new `HttpProxy` from a string.
    fn from_str(proxy_str: &str) -> std::result::Result<Self, Self::Err> {
        let url = Url::parse(proxy_str)?;
        let addr = &url[Position::BeforeHost..Position::AfterPort];

        let scheme = url.scheme();
        match scheme {
            "http" | "https" => {
                let mut basic_bytes: Option<Vec<u8>> = None;
                if let Some(pwd) = url.password() {
                    let encoded_str = format!(
                        "Basic {}",
                        BASE64_STANDARD.encode(format!("{}:{pwd}", url.username()))
                    );
                    basic_bytes = Some(encoded_str.into_bytes());
                };

                Ok(Self {
                    auth: basic_bytes,
                    url: addr.to_string(),
                })
            }

            _ => Err(Error::unimplemented(format!(
                "unsupported proxy schema {scheme}"
            ))),
        }
    }
}

impl Display for Http {
    /// Format the `HttpProxy` as a string.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.url)
    }
}
