use std::{fmt, fs, io, ops::Deref};
use toml;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Arl(String);

impl Arl {
    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `arl` contains invalid characters
    pub fn new(arl: &str) -> io::Result<Self> {
        // An `arl` must hold a valid cookie value.
        for chr in arl.chars() {
            if !arl.is_ascii()
                || chr.is_ascii_control()
                || chr.is_ascii_whitespace()
                || vec!['\"', ',', ';', '\\'].contains(&chr)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "arl contains invalid characters".to_string(),
                ));
            }
        }

        Ok(Self(arl.to_owned()))
    }

    /// TODO
    ///
    /// # Errors
    ///
    /// Will return `Err` if:
    /// - `arl_file` can not be accessed
    /// - `arl_file` is too large
    /// - `arl_file` does not contain an `arl`
    pub fn from_file(arl_file: &str) -> io::Result<Self> {
        // Prevent out-of-memory condition: `arl` file should be small.
        let attributes = fs::metadata(arl_file)?;
        let file_size = attributes.len();
        if file_size > 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "{arl_file} too large: {file_size} bytes",
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
