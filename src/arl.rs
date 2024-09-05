use std::{fmt, io, ops::Deref, str::FromStr};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("invalid ARL: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Arl(String);

impl Arl {
    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `arl` contains invalid characters
    pub fn new(arl: String) -> io::Result<Self> {
        Ok(Self(arl))
    }
}

impl Deref for Arl {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for Arl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for Arl {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        let mut arl = s;

        // Foolproofing: in case a full callback URL is set.
        let parts: Vec<&str> = s.split('/').collect();
        if let Some(last_part) = parts.last() {
            arl = last_part;
        }

        // An `arl` must hold a valid cookie value.
        for chr in s.chars() {
            if !chr.is_ascii()
                || chr.is_ascii_control()
                || chr.is_ascii_whitespace()
                || ['\"', ',', ';', '\\'].contains(&chr)
            {
                return Err(Error::Invalid("invalid characters".to_string()));
            }
        }

        Ok(Self(arl.to_owned()))
    }
}
