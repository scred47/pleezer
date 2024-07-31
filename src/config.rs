use machine_uid;
use sysinfo;
use uuid::Uuid;

use crate::arl::Arl;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Config {
    pub app_name: String,
    pub app_version: String,
    pub app_lang: String,

    pub device_name: String,
    pub device_id: Uuid,

    pub interruptions: bool,

    pub user_agent: String,

    pub arl: Arl,
}

impl Config {
    #[must_use]
    pub fn with_arl(arl: Arl) -> Self {
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

        // Additional `User-Agent` string checks on top of `reqwest::HeaderValue`.
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
        let user_agent =
            format!("{app_name}/{app_version} (Rust; {os_name}/{os_version}; Desktop; {app_lang})");
        trace!("user agent: {user_agent}");

        Self {
            app_name,
            app_version,
            app_lang,

            device_name: env!("CARGO_PKG_NAME").to_owned(),
            device_id,

            interruptions: true,

            user_agent,

            arl,
        }
    }
}
