use machine_uid;
use uuid::Uuid;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Config {
    pub app_name: String,
    pub app_version: String,
    pub app_lang: String,

    pub device_name: String,
    pub device_id: Uuid,

    pub interruptions: bool,
}

impl Default for Config {
    fn default() -> Self {
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
        debug!("device uuid: {device_id}");

        Self {
            app_name: env!("CARGO_PKG_NAME").to_owned(),
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            app_lang: "en".to_owned(),

            device_name: env!("CARGO_PKG_NAME").to_owned(),
            device_id,

            interruptions: true,
        }
    }
}
