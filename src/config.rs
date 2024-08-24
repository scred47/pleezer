use machine_uid;
use rand::Rng;
use sysinfo;
use uuid::Uuid;

use crate::arl::Arl;

/// The configuration of pleezer.
// TODO: implement Debug manually to avoid leaking the arl.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Config {
    /// The name of the application.
    ///
    /// By default this is retrieved from `Cargo.toml`, used in the
    /// `User-Agent` string, and the fallback device name if not provided and
    /// the system hostname is not available.
    pub app_name: String,

    /// The version of the application.
    ///
    /// By default this is retrieved from `Cargo.toml` used in the `User-Agent`
    /// string.
    pub app_version: String,

    /// The language of the application in ISO 639-1 format.
    ///
    /// By default this is "en" for English, used in the `User-Agent` string,
    /// as well as `Accept-Language`header in API requests.
    pub app_lang: String,

    /// The player's name as it appears to Deezer clients.
    ///
    /// By default this is equal to `app_name`.
    pub device_name: String,

    /// The ID that uniquely identifies the device.
    ///
    /// By default this is the machine ID, or a random UUID if the machine ID
    /// could not be retrieved.
    pub device_id: Uuid,

    /// Whether other clients may take over an existing connection.
    ///
    /// By default this is `true`.
    pub interruptions: bool,

    /// The client ID used in API requests.
    ///
    /// By default this is a random number of 9 digits.
    pub client_id: usize,

    /// The `User-Agent` string used in API requests.
    ///
    /// By default this is a combination of the application name, version, and
    /// language, to be like the official Deezer Desktop client.
    pub user_agent: String,

    /// The `arl` token used in API requests.
    ///
    /// By default this is `None` and must be set before making authenticated
    /// requests.
    pub arl: Option<Arl>,
}

impl Default for Config {
    /// Creates a new configuration from `Cargo.toml` and system information.
    ///
    /// # Panics
    ///
    /// Panics if the application name, version, language, OS name and/or
    /// version are invalid.
    fn default() -> Self {
        let app_name = env!("CARGO_PKG_NAME").to_owned();
        let app_version = env!("CARGO_PKG_VERSION").to_owned();
        let app_lang = "en".to_owned();

        let device_id = match machine_uid::get() {
            Ok(machine_id) => {
                let namespace = Uuid::new_v5(&Uuid::NAMESPACE_DNS, b"deezer.com");
                Uuid::new_v5(&namespace, machine_id.as_bytes())
            }
            Err(e) => {
                warn!("could not get machine id, using random device id: {e}");
                Uuid::new_v4()
            }
        };
        trace!("device uuid: {device_id}");

        // Additional `User-Agent` string checks on top of what
        // `reqwest::HeaderValue` already checks.
        let illegal_chars = |chr| chr == '/' || chr == ';';
        if app_name.is_empty()
            || app_name.contains(illegal_chars)
            || app_version.is_empty()
            || app_version.contains(illegal_chars)
            || app_lang.chars().count() != 2
            || app_lang.contains(illegal_chars)
        {
            panic!(
                "application name, version and/or language invalid (\"{app_name}\"; \"{app_version}\"; \"{app_lang}\")"
            );
        }

        let os_name = match std::env::consts::OS {
            "macos" => "osx",
            other => other,
        };
        let os_version = sysinfo::System::os_version().unwrap_or_else(|| String::from("0"));
        if os_name.is_empty()
            || os_name.contains(illegal_chars)
            || os_version.is_empty()
            || os_version.contains(illegal_chars)
        {
            panic!("os name and/or version invalid (\"{os_name}\"; \"{os_version}\")");
        }

        // Set `User-Agent` to be served like Deezer on desktop.
        let user_agent = format!(
            "{app_name}/{app_version} (Rust; {os_name}/{os_version}; like Desktop; {app_lang})"
        );
        trace!("user agent: {user_agent}");

        // Deezer on desktop uses a new `cid` on every start.
        let client_id = rand::thread_rng().gen_range(100_000_000..=999_999_999);
        debug!("client id: {client_id}");

        Self {
            app_name: app_name.clone(),
            app_version,
            app_lang,

            device_name: app_name,
            device_id,

            interruptions: true,

            client_id,
            user_agent,

            arl: None,
        }
    }
}
