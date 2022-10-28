use std::{fmt, fs, io};
use toml;

#[derive(Clone, Debug)]
pub struct Arl(String);

impl Arl {
    pub fn new(arl: &str) -> io::Result<Self> {
        let chars = arl.chars().count();
        if chars != 192 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("arl should be 192 characters long but is {chars}"),
            ));
        }

        // An `arl` must be a valid cookie value.
        for chr in arl.chars() {
            if !arl.is_ascii()
                || chr.is_ascii_control()
                || chr.is_ascii_whitespace()
                || vec!['\"', ',', ';', '\\'].contains(&chr)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("arl contains invalid characters"),
                ));
            }
        }

        Ok(Self(arl.to_owned()))
    }

    pub fn from_file(arl_file: &str) -> io::Result<Self> {
        // Prevent out-of-memory condition: `arl` file should be small.
        let attributes = fs::metadata(arl_file)?;
        let file_size = attributes.len();
        if file_size > 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "{arl_file} too large ({file_size} bytes)",
            ));
        }

        let contents = fs::read_to_string(arl_file)?;
        let contents = contents.parse::<toml::Value>().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{arl_file} format invalid: {e}"),
            )
        })?;

        let mut arl = contents["arl"].as_str().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "arl not found in {arl_file}")
        })?;

        // Foolproofing: in case a full callback URL is set.
        let parts: Vec<&str> = arl.split('/').collect();
        if let Some(last_part) = parts.last() {
            arl = last_part;
        }

        Self::new(arl)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Arl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
