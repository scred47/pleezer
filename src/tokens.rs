use std::{
    fmt,
    num::NonZeroU64,
    time::{Duration, SystemTime},
};

use async_trait::async_trait;
use thiserror::Error;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UserToken {
    pub user_id: NonZeroU64,
    pub token: String,
    pub expires_at: SystemTime,
}

#[derive(Error, Debug)]
pub enum UserTokenError {
    #[error("user token requires refresh")]
    Refresh,

    #[error("permission denied for user token: {0}")]
    PermissionDenied(String),

    #[error("user token provider error: {0}")]
    Provider(Box<dyn std::error::Error>),
}

#[async_trait]
pub trait UserTokenProvider {
    async fn user_token(&mut self) -> Result<UserToken, UserTokenError>;
    fn flush_user_token(&mut self);
}

impl UserToken {
    #[must_use]
    pub fn time_to_live(&self) -> Duration {
        self.expires_at
            .duration_since(SystemTime::now())
            .unwrap_or(Duration::ZERO)
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
