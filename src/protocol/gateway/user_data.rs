use std::{collections::HashMap, io};

use serde::Deserialize;
use serde_with::{serde_as, DisplayFromStr};

use super::Method;

// TODO: implement ads and free accounts

// TODO: implement defaults, options

#[derive(Clone, Deserialize, Debug)]
pub struct UserDataResponse {
    pub results: UserData,
}

impl<'a> Method<'a> for UserDataResponse {
    const METHOD: &'a str = "getUserData";
}

#[derive(Clone, Deserialize, Debug)]
pub struct UserData {
    #[serde(rename = "USER")]
    pub user: User,
    #[serde(rename = "SESSION_ID")]
    session_id: String,
    #[serde(rename = "USER_TOKEN")]
    pub user_token: String,
    #[serde(rename = "OFFER_NAME")]
    pub plan: String,
    #[serde(rename = "SERVER_TIMESTAMP")]
    timestamp: u64,
    #[serde(rename = "PLAYER_TOKEN")]
    player_token: String,
    #[serde(rename = "checkForm")]
    pub api_token: String,
    #[serde(rename = "__DZR_GATEKEEPS__")]
    pub gatekeeps: Gatekeeps,
    #[serde(rename = "URL_MEDIA")]
    media_url: String,
    #[serde(rename = "GAIN")]
    gain: Gain,
}

#[derive(Clone, Deserialize, Debug)]
pub struct User {
    #[serde(rename = "USER_ID")]
    pub id: u64,
    #[serde(rename = "OPTIONS")]
    pub options: Options,
    #[serde(rename = "AUDIO_SETTINGS")]
    pub audio_settings: AudioSettings,
    #[serde(rename = "SETTING")]
    settings: Settings,
}

#[serde_as]
#[derive(Clone, Deserialize, Debug)]
pub struct Options {
    license_token: String,
    audio_quality_default_preset: String,
    pub too_many_devices: bool,
    pub expiration_timestamp: u64,
    pub timestamp: u64,
    // TODO: are these used anywhere in the API?
    // license_country: String,
    // radio_skips: bool,
    // business: bool,
    // streaming_group: String,
    // queuelist_edition: bool,
}

#[derive(Clone, Deserialize, Debug)]
pub struct AudioSettings {
    presets: Vec<AudioPreset>,
    default_preset: String,
    pub connected_device_streaming_preset: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct AudioPreset {
    id: String,
    #[serde(rename = "wifi_download")]
    audio_quality: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Settings {
    site: SiteSettings,
    adjust: AdjustSettings,
    audio_quality_settings: AudioQualitySettings,
}

#[derive(Clone, Deserialize, Debug)]
pub struct SiteSettings {
    player_hq: bool,
    player_audio_quality: String,
    player_repeat: i64,
    player_normalize: bool,
    cast_audio_quality: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct AdjustSettings {
    // TODO: what do these do?
    d0_stream: String,
    d7_stream: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct AudioQualitySettings {
    preset: String,
    connected_device_streaming_preset: String,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Gatekeeps {
    disable_device_limitation: bool,
    #[serde(rename = "metric.timetoplay")]
    metric_timetoplay: bool,
    #[serde(rename = "metric.remote_control")]
    metric_remote_control: bool,
    #[serde(rename = "metric.media_request_errors")]
    metric_media_request_errors: bool,
    cdn_metrics: bool,
    #[serde(rename = "metric.playback_errors")]
    metric_playback_errors: bool,
    volume_normalization: bool,
    pub remote_control: bool,
    pub remote_control_release: bool,
    free_on_cast: bool,
}

#[serde_as]
#[derive(Clone, Deserialize, Debug)]
pub struct Gain {
    #[serde(rename = "TARGET")]
    #[serde_as(as = "DisplayFromStr")]
    target: i64,
}

// pub async fn get(arl: &str, api_token: &str) -> io::Result<UserData> {
//     let response = super::request(arl, api_token, "deezer.getUserData").await?;
//     response.json::<UserData>().await.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
// }
