use std::{
    fmt,
    num::NonZeroU64,
    time::{Duration, Instant},
};

use async_trait::async_trait;
use thiserror::Error;

#[derive(Clone, Eq, PartialEq, Debug)]
pub struct UserToken {
    user_id: NonZeroU64,
    token: String,
    expires_at: Instant,
}

#[derive(Error, Debug)]
pub enum UserTokenError {
    #[error("user token invalid: {0}")]
    Invalid(String),
    #[error("permission denied for user token: {0}")]
    PermissionDenied(String),
    #[error("user token provider error: {0}")]
    ProviderError(Box<dyn std::error::Error>),
}

#[async_trait]
pub trait UserTokenProvider {
    async fn user_token(&mut self) -> Result<UserToken, UserTokenError>;
    fn flush_user_token(&mut self);
}

impl UserToken {
    pub fn new(
        user_id: NonZeroU64,
        token: &str,
        expires_at: Instant,
    ) -> Result<Self, UserTokenError> {
        let chars = token.chars().count();
        if chars != 64 {
            return Err(UserTokenError::Invalid(format!(
                "user token should be 64 characters long but is {chars}"
            )));
        }

        Ok(Self {
            user_id,
            token: token.to_owned(),
            expires_at,
        })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.token
    }

    #[must_use]
    pub fn user_id(&self) -> NonZeroU64 {
        self.user_id
    }

    #[must_use]
    pub fn time_to_live(&self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }

    #[must_use]
    pub fn expires_at(&self) -> Instant {
        self.expires_at
    }

    #[must_use]
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

impl fmt::Display for UserToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.token)
    }
}
