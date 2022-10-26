use thiserror::Error;

#[derive(Clone, Debug)]
pub struct Config {
    pub app_name: String,
    pub app_version: String,
    pub app_lang: String,
    pub device_name: String,
}

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("invalid configuration: {0}")]
    Invalid(String),
}

impl Default for Config {
    fn default() -> Self {
        Self {
            app_name: env!("CARGO_PKG_NAME").to_owned(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            app_lang: "en".to_owned(),
            device_name: env!("CARGO_PKG_NAME").to_owned(),
        }
    }
}
