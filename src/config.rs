use machine_uid;
use uuid::Uuid;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Config {
    pub app_name: String,
    pub app_version: String,
    pub app_lang: String,
    pub device_name: String,
    pub device_uuid: Uuid,
}

impl Default for Config {
    fn default() -> Self {
        let device_uuid = match machine_uid::get() {
            Ok(machine_id) => Uuid::new_v5(&Uuid::nil(), &machine_id.as_bytes()),
            Err(e) => {
                warn!("could not get machine id, using random device uuid: {e}");
                Uuid::new_v4()
            }
        };
        debug!("device uuid: {device_uuid}");

        Self {
            app_name: env!("CARGO_PKG_NAME").to_owned(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            app_lang: "en".to_owned(),
            device_name: env!("CARGO_PKG_NAME").to_owned(),
            device_uuid,
        }
    }
}
