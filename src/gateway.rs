//! Gateway API client for Deezer services.
//!
//! This module provides access to Deezer's gateway API, handling:
//! * Authentication (ARL tokens and user credentials)
//! * Session management
//! * User data retrieval
//! * Queue and track information
//! * Flow recommendations
//!
//! # Authentication
//!
//! Supports two authentication methods:
//! * Email/password login (preferred, allows token refresh)
//! * ARL token (requires manual renewal when expired)
//!
//! # Example
//!
//! ```rust
//! use pleezer::gateway::Gateway;
//!
//! let mut gateway = Gateway::new(&config)?;
//!
//! // Login with credentials
//! let arl = gateway.login("user@example.com", "password").await?;
//!
//! // Or use existing ARL
//! gateway.refresh().await?;
//! ```

use std::time::SystemTime;

use md5::{Digest, Md5};
use reqwest::{
    self,
    header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
};
use serde::Deserialize;
use url::Url;

use crate::{
    arl::Arl,
    config::{Config, Credentials},
    error::{Error, ErrorKind, Result},
    http::Client as HttpClient,
    protocol::{
        self, auth,
        connect::{
            queue::{self, TrackType},
            AudioQuality, UserId,
        },
        gateway::{self, MediaUrl, Queue, UserData},
    },
    tokens::UserToken,
};

/// Gateway client for Deezer API access.
///
/// Handles authentication, session management, and API requests to
/// Deezer's gateway endpoints. Maintains user data and authentication
/// state for continuous operation.
pub struct Gateway {
    /// HTTP client with cookie management.
    http_client: HttpClient,

    /// Cached user data from last refresh.
    ///
    /// Contains authentication tokens, preferences, and capabilities.
    // TODO : we probably don't need to retain all user data, all the time
    //       keep what we need here in the gateway, and send the rest off into
    //       a token object
    user_data: Option<UserData>,

    /// Client identifier for API requests.
    client_id: usize,
}

impl Gateway {
    /// Cookie domain for authentication.
    ///
    /// Note: This URL is not entirely correct, as the cookies could come from
    /// `connect.deezer.com` or `www.deezer.com` as well. What matters is
    /// that the domain matches with `deezer.com`.
    const COOKIE_ORIGIN: &'static str = "https://www.deezer.com";

    /// Gateway API endpoint URL.
    ///
    /// Base URL for all gateway API requests.
    const GATEWAY_URL: &'static str = "https://www.deezer.com/ajax/gw-light.php";

    /// Gateway API version string.
    ///
    /// Protocol version identifier included in all requests.
    /// Matches the version supported by official Deezer clients.
    const GATEWAY_VERSION: &'static str = "1.0";

    /// Gateway API input type identifier.
    ///
    /// Input type code that identifies the request format.
    /// Type 3 represents the standard gateway request format.
    const GATEWAY_INPUT: usize = 3;

    /// OAuth client ID for authentication.
    ///
    /// Application identifier used during OAuth authentication flow.
    /// Registered client ID for web application access.
    const OAUTH_CLIENT_ID: usize = 447_462;

    /// OAuth password hashing salt.
    ///
    /// Salt value used in password hash calculation during login.
    /// Combined with client ID and user credentials for secure authentication.
    const OAUTH_SALT: &'static str = "a83bf7f38ad2f137e444727cfc3775cf";

    /// OAuth session ID endpoint.
    ///
    /// URL for initiating OAuth authentication flow.
    /// Used to obtain a session ID before login.
    const OAUTH_SID_URL: &'static str = "https://connect.deezer.com/oauth/auth.php";

    /// OAuth login endpoint.
    ///
    /// URL for performing OAuth login with credentials.
    /// Returns access token on successful authentication.
    const OAUTH_LOGIN_URL: &'static str = "https://connect.deezer.com/oauth/user_auth.php";

    /// Content type for gateway requests.
    ///
    /// Although the bodies of all gateway requests are JSON, the
    /// `Content-Type` is not.
    const PLAIN_TEXT_CONTENT: HeaderValue = HeaderValue::from_static("text/plain;charset=UTF-8");

    /// Default empty JSON body for requests.
    ///
    /// Used when a request requires a body but has no parameters.
    /// Prevents having to create empty JSON objects repeatedly.
    const EMPTY_JSON_OBJECT: &'static str = "{}";

    /// Returns the cookie origin URL for Deezer services.
    ///
    /// # Panics
    ///
    /// Panics if the hardcoded URL is invalid, which should never happen
    /// as it's a compile-time constant.
    ///
    /// # Internal Use
    ///
    /// This method is used by cookie management functions to ensure
    /// all cookies are properly scoped to the Deezer domain.
    #[must_use]
    fn cookie_origin() -> reqwest::Url {
        reqwest::Url::parse(Self::COOKIE_ORIGIN).expect("invalid cookie origin")
    }

    /// Creates a cookie jar with authentication and language cookies.
    ///
    /// Sets up cookies required for Deezer API access:
    /// * Language preference cookie
    /// * ARL authentication cookie (if using ARL credentials)
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration containing credentials and language settings
    ///
    /// # Cookie Format
    ///
    /// Cookies are set with:
    /// * Domain: deezer.com
    /// * Path: /
    /// * Secure flag
    /// * `HttpOnly` flag
    #[must_use]
    fn cookie_jar(config: &Config) -> reqwest::cookie::Jar {
        let cookie_jar = reqwest::cookie::Jar::default();
        let cookie_origin = Self::cookie_origin();

        let lang_cookie = format!(
            "dz_lang={}; Domain=deezer.com; Path=/; Secure; HttpOnly",
            &config.app_lang
        );
        cookie_jar.add_cookie_str(&lang_cookie, &cookie_origin);

        if let Credentials::Arl(ref arl) = config.credentials {
            let arl_cookie = format!("arl={arl}; Domain=deezer.com; Path=/; Secure; HttpOnly");
            cookie_jar.add_cookie_str(&arl_cookie, &cookie_origin);
        }

        cookie_jar
    }

    /// Creates a new gateway client instance.
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration including credentials and client settings
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * User-Agent header cannot be created from config
    /// * OS information cannot be detected
    /// * Cookie creation fails
    pub fn new(config: &Config) -> Result<Self> {
        // Create a new cookie jar and put the cookies in.
        let cookie_jar = Self::cookie_jar(config);
        let http_client = HttpClient::with_cookies(config, cookie_jar)?;

        Ok(Self {
            client_id: config.client_id,
            http_client,
            user_data: None,
        })
    }

    /// Returns the current cookie header value, if available.
    ///
    /// Used for authentication in requests to Deezer services.
    #[must_use]
    pub fn cookies(&self) -> Option<HeaderValue> {
        self.http_client
            .cookie_jar
            .as_ref()
            .and_then(|jar| jar.cookies(&Self::cookie_origin()))
    }

    /// Refreshes user data and authentication state.
    ///
    /// Should be called when:
    /// * Starting a new session
    /// * After token expiration
    /// * When user data needs updating
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * ARL token is invalid or expired
    /// * Remote control is disabled
    /// * Too many devices are registered
    /// * Network request fails
    pub async fn refresh(&mut self) -> Result<()> {
        // Send an empty JSON map
        match self
            .request::<gateway::UserData>(Self::EMPTY_JSON_OBJECT, None)
            .await
        {
            Ok(response) => {
                if let Some(data) = response.first() {
                    if !data.gatekeeps.remote_control {
                        return Err(Error::permission_denied(
                            "remote control is disabled for this account; upgrade your Deezer subscription",
                        ));
                    }
                    if data.user.options.too_many_devices {
                        return Err(Error::resource_exhausted(
                            "too many devices; remove one or more in your account settings",
                        ));
                    }
                    if data.user.options.ads_audio {
                        return Err(Error::unimplemented(
                            "ads are not implemented; upgrade your Deezer subscription",
                        ));
                    }

                    self.set_user_data(data.clone());
                } else {
                    return Err(Error::not_found("no user data received".to_string()));
                }
                Ok(())
            }
            Err(e) => {
                if e.kind == ErrorKind::InvalidArgument {
                    // For an invalid or expired `arl`, the response has some
                    // fields as integer `0` which are normally typed as string,
                    // which causes JSON deserialization to fail.
                    return Err(Error::permission_denied(
                        "arl invalid or expired".to_string(),
                    ));
                }

                Err(e)
            }
        }
    }

    /// Sends a request to the Deezer gateway API.
    ///
    /// Handles:
    /// * API token inclusion
    /// * Request formatting
    /// * Response parsing
    /// * Error mapping
    ///
    /// # Type Parameters
    ///
    /// * `T` - Response type that implements `Method` and `Deserialize`
    ///
    /// # Arguments
    ///
    /// * `body` - Request body content
    /// * `headers` - Optional additional headers
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * URL construction fails
    /// * Network request fails
    /// * Response isn't valid JSON
    /// * Response can't be parsed as type T
    pub async fn request<T>(
        &mut self,
        body: impl Into<reqwest::Body>,
        headers: Option<HeaderMap>,
    ) -> Result<gateway::Response<T>>
    where
        T: std::fmt::Debug + gateway::Method + for<'de> Deserialize<'de>,
    {
        // Get the API token from the user data or use an empty string.
        let api_token = self
            .user_data
            .as_ref()
            .map(|data| data.api_token.as_str())
            .unwrap_or_default();

        // Check the URL early to not needlessly hit the rate limiter.
        let url_str = format!(
            "{}?method={}&input={}&api_version={}&api_token={api_token}&cid={}",
            Self::GATEWAY_URL,
            T::METHOD,
            Self::GATEWAY_INPUT,
            Self::GATEWAY_VERSION,
            self.client_id,
        );
        let url = url_str.parse::<reqwest::Url>()?;
        let mut request = self.http_client.post(url, body);

        let request_headers = request.headers_mut();
        request_headers.try_insert(CONTENT_TYPE, Self::PLAIN_TEXT_CONTENT)?;

        // Add any headers that were passed in.
        if let Some(headers) = headers {
            request_headers.extend(headers);
        }

        let response = self.http_client.execute(request).await?;
        let body = response.text().await?;

        protocol::json(&body, T::METHOD)
    }

    /// Returns the current license token if available.
    ///
    /// The license token is required for media access.
    #[must_use]
    pub fn license_token(&self) -> Option<&str> {
        self.user_data
            .as_ref()
            .map(|data| data.user.options.license_token.as_str())
    }

    /// Checks if the current session has expired.
    ///
    /// Returns `true` if:
    /// * No user data is available
    /// * Current time is past expiration time
    #[must_use]
    pub fn is_expired(&self) -> bool {
        self.expires_at() <= SystemTime::now()
    }

    /// Returns when the current session will expire.
    ///
    /// Returns UNIX epoch if no session is active.
    #[must_use]
    pub fn expires_at(&self) -> SystemTime {
        if let Some(data) = &self.user_data {
            return data.user.options.expiration_timestamp;
        }

        SystemTime::UNIX_EPOCH
    }

    /// Updates the cached user data.
    pub fn set_user_data(&mut self, data: UserData) {
        self.user_data = Some(data);
    }

    /// Returns a reference to the current user data if available.
    #[must_use]
    pub fn user_data(&self) -> Option<&gateway::UserData> {
        self.user_data.as_ref()
    }

    /// Returns the user's preferred streaming quality for connected devices.
    ///
    /// Returns the default quality if no preference is set.
    #[must_use]
    pub fn audio_quality(&self) -> AudioQuality {
        self.user_data
            .as_ref()
            .map_or(AudioQuality::default(), |data| {
                data.user.audio_settings.connected_device_streaming_preset
            })
    }

    /// Returns the target gain for volume normalization.
    ///
    /// The value is clamped to i8 range as the API might return
    /// out-of-bounds values.
    #[must_use]
    #[expect(clippy::cast_possible_truncation)]
    pub fn target_gain(&self) -> i8 {
        self.user_data
            .as_ref()
            .map(|data| data.gain)
            .unwrap_or_default()
            .target
            .clamp(i64::from(i8::MIN), i64::from(i8::MAX)) as i8
    }

    /// Returns the user's display name if available.
    #[must_use]
    pub fn user_name(&self) -> Option<&str> {
        self.user_data.as_ref().map(|data| data.user.name.as_str())
    }

    /// Returns the URL for media content requests.
    ///
    /// Returns the default URL if no custom URL is set.
    #[must_use]
    pub fn media_url(&self) -> Url {
        self.user_data
            .as_ref()
            .map_or(MediaUrl::default(), |data| data.media_url.clone())
            .into()
    }

    /// Converts a protocol buffer track list into a queue.
    ///
    /// Fetches detailed track information for each track in the list.
    ///
    /// # Arguments
    ///
    /// * `list` - Protocol buffer track list to convert
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * Track IDs are invalid
    /// * Track type is unsupported
    /// * Network request fails
    /// * Response parsing fails
    pub async fn list_to_queue(&mut self, list: &queue::List) -> Result<Queue> {
        let track_list = gateway::list_data::Request {
            track_ids: list
                .tracks
                .iter()
                .map(|track| {
                    let track_type = track.typ.enum_value_or_default();
                    if track_type == TrackType::TRACK_TYPE_SONG {
                        track.id.parse().map_err(Into::into)
                    } else {
                        Err(Error::unimplemented(format!(
                            "{track_type:?} not yet implemented"
                        )))
                    }
                })
                .collect::<std::result::Result<Vec<_>, _>>()?,
        };

        let body = serde_json::to_string(&track_list)?;
        match self.request::<gateway::ListData>(body, None).await {
            Ok(response) => Ok(response.all().clone()),
            Err(e) => Err(e),
        }
    }

    /// Fetches Flow recommendations for a user.
    ///
    /// Flow is Deezer's personalized radio feature.
    ///
    /// # Arguments
    ///
    /// * `user_id` - ID of user to get recommendations for
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * Network request fails
    /// * Response parsing fails
    pub async fn user_radio(&mut self, user_id: UserId) -> Result<Queue> {
        let request = gateway::user_radio::Request { user_id };
        let body = serde_json::to_string(&request)?;
        match self.request::<gateway::UserRadio>(body, None).await {
            Ok(response) => {
                // Transform the `UserRadio` response into a `Queue`. This is done to have
                // `UserRadio` re-use the `ListData` struct (for which `Queue` is an alias).
                Ok(response
                    .all()
                    .clone()
                    .into_iter()
                    .map(|item| item.0)
                    .collect())
            }
            Err(e) => Err(e),
        }
    }

    /// Retrieves an ARL token using an OAuth access token.
    ///
    /// # Arguments
    ///
    /// * `access_token` - OAuth access token from login
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * Network request fails
    /// * Response parsing fails
    /// * ARL parsing fails
    /// * No ARL is returned
    pub async fn get_arl(&mut self, access_token: &str) -> Result<Arl> {
        let mut headers = HeaderMap::new();
        headers.try_insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {access_token}"))?,
        )?;

        let arl = self
            .request::<gateway::Arl>(Self::EMPTY_JSON_OBJECT, Some(headers))
            .await
            .and_then(|response| {
                response
                    .first()
                    .map(|result| result.0.clone())
                    .ok_or_else(|| Error::not_found("no arl received".to_string()))
            })?;

        arl.parse::<Arl>()
    }

    /// Returns the user token for remote control functionality.
    ///
    /// Refreshes the session if expired.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * Session refresh fails
    /// * User data isn't available
    /// * Remote control is disabled
    /// * Too many devices are registered
    pub async fn user_token(&mut self) -> Result<UserToken> {
        if self.is_expired() {
            debug!("refreshing user token");
            self.refresh().await?;
        }

        match &self.user_data {
            Some(data) => Ok(UserToken {
                user_id: data.user.id,
                token: data.user_token.clone(),
                expires_at: self.expires_at(),
            }),
            None => Err(Error::unavailable("user data unavailable".to_string())),
        }
    }

    /// Invalidates the current user token.
    ///
    /// Forces a refresh on next token request while preserving
    /// other API functionality.
    pub fn flush_user_token(&mut self) {
        // Force refreshing user data, but do not set `user_data` to `None` so
        // so we can continue using the `api_token` it contains.
        if let Some(data) = self.user_data.as_mut() {
            data.user.options.expiration_timestamp = SystemTime::UNIX_EPOCH;
        }
    }

    /// Logs in with email and password to obtain an ARL token.
    ///
    /// # Arguments
    ///
    /// * `email` - User's email address
    /// * `password` - User's password
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// * Credentials are invalid
    /// * Email/password length is invalid
    /// * Network request fails
    /// * Response parsing fails
    /// * ARL parsing fails
    pub async fn login(&mut self, email: &str, password: &str) -> Result<Arl> {
        // Check email and password length to prevent out-of-memory conditions.
        const LENGTH_CHECK: std::ops::Range<usize> = 1..255;
        if !LENGTH_CHECK.contains(&email.len()) || !LENGTH_CHECK.contains(&password.len()) {
            return Err(Error::out_of_range(
                "email and password must be between 1 and 255 characters".to_string(),
            ));
        }

        // Hash the passwords.
        let password = Md5::digest(password);
        let hash = Md5::digest(format!(
            "{}{email}{password:x}{}",
            Self::OAUTH_CLIENT_ID,
            Self::OAUTH_SALT,
        ));

        // First get a session ID. The response can be ignored because the
        // session ID is stored in the cookie store.
        let request = self.http_client.get(Url::parse(Self::OAUTH_SID_URL)?, "");
        let _ = self.http_client.execute(request).await?;

        // Then login and get an access token.
        let query = Url::parse(&format!(
            "{}?app_id={}&login={email}&password={password:x}&hash={hash:x}",
            Self::OAUTH_LOGIN_URL,
            Self::OAUTH_CLIENT_ID,
        ))?;

        let request = self.http_client.get(query.clone(), "");
        let response = self.http_client.execute(request).await?;
        let body = response.text().await?;
        let result: auth::User = protocol::json(&body, query.path())
            .map_err(|_| Error::permission_denied("email or password incorrect"))?;

        // Finally use the access token to get an ARL.
        self.get_arl(&result.access_token).await
    }
}
