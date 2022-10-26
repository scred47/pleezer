use std::{fmt, time::SystemTime};

use async_trait::async_trait;
use thiserror::Error;

#[derive(Clone, Debug)]
pub struct UserToken {
    user_id: u64,
    token: String,
    expires_at: SystemTime,
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
    fn expire_token(&mut self);
}

impl UserToken {
    pub fn new(user_id: u64, token: &str, expires_at: SystemTime) -> Result<Self, UserTokenError> {
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
    pub fn is_expired(&self) -> bool {
        SystemTime::now() >= self.expires_at
    }
}

impl fmt::Display for UserToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.token)
    }
}
