use std::{num::NonZeroU64, time::SystemTime};

use serde::Deserialize;
use serde_with::{formats::Flexible, serde_as, DisplayFromStr, TimestampSeconds};
use veil::Redact;

use super::Method;

// TODO: implement defaults, options
// TODO: check private fields

impl Method for UserData {
    const METHOD: &'static str = "deezer.getUserData";
}

// TODO : #[serde(rename_all = "UPPERCASE")]
#[serde_as]
#[derive(Clone, PartialEq, Deserialize, Redact)]
pub struct UserData {
    #[serde(rename = "USER")]
    pub user: User,
    #[serde(rename = "SESSION_ID")]
    session_id: String,
    #[serde(rename = "USER_TOKEN")]
    #[redact]
    pub user_token: String,
    #[serde(rename = "OFFER_NAME")]
    pub plan: String,
    #[serde_as(as = "TimestampSeconds<i64, Flexible>")]
    #[serde(rename = "SERVER_TIMESTAMP")]
    timestamp: SystemTime,
    #[serde(rename = "PLAYER_TOKEN")]
    #[redact]
    player_token: String,
    #[serde(rename = "checkForm")]
    #[redact]
    pub api_token: String,
    #[serde(rename = "__DZR_GATEKEEPS__")]
    pub gatekeeps: Gatekeeps,
    #[serde(rename = "URL_MEDIA")]
    pub media_url: String,
    #[serde(rename = "GAIN")]
    pub gain: Gain,
}

#[derive(Clone, Eq, PartialEq, Deserialize, Debug, Hash)]
pub struct User {
    #[serde(rename = "USER_ID")]
    // TODO : replace with UserId
    pub id: NonZeroU64,
    #[serde(rename = "BLOG_NAME")]
    pub name: String,
    #[serde(rename = "OPTIONS")]
    pub options: Options,
    #[serde(rename = "AUDIO_SETTINGS")]
    pub audio_settings: AudioSettings,
    #[serde(rename = "SETTING")]
    pub settings: Settings,
}

// TODO: find out how to register our own device.

#[serde_as]
#[derive(Clone, Eq, PartialEq, Deserialize, Redact, Hash)]
pub struct Options {
    #[redact]
    pub license_token: String,
    audio_quality_default_preset: String,
    pub too_many_devices: bool,
    #[serde_as(as = "TimestampSeconds<i64, Flexible>")]
    pub expiration_timestamp: SystemTime,
    #[serde_as(as = "TimestampSeconds<i64, Flexible>")]
    pub timestamp: SystemTime,
    // TODO: are these used anywhere in the API?
    // license_country: String,
    // radio_skips: bool,
    // business: bool,
    // streaming_group: String,
    // queuelist_edition: bool,
}

#[derive(Clone, Eq, PartialEq, Deserialize, Debug, Hash)]
pub struct AudioSettings {
    presets: Vec<AudioPreset>,
    default_preset: String,
    pub connected_device_streaming_preset: String,
}

#[derive(Clone, Eq, PartialEq, Deserialize, Debug, Hash)]
pub struct AudioPreset {
    id: String,
    #[serde(rename = "wifi_download")]
    audio_quality: String,
}

#[derive(Clone, Eq, PartialEq, Deserialize, Debug, Hash)]
pub struct Settings {
    pub site: SiteSettings,
    adjust: AdjustSettings,
    audio_quality_settings: AudioQualitySettings,
}

#[derive(Clone, Eq, PartialEq, Deserialize, Debug, Hash)]
pub struct SiteSettings {
    player_hq: bool,
    player_audio_quality: String,
    player_repeat: i64, // TODO: use repeat enum
    pub player_normalize: bool,
    cast_audio_quality: String,
}

#[derive(Clone, Eq, PartialEq, Deserialize, Debug, Hash)]
pub struct AdjustSettings {
    // TODO: what do these do?
    d0_stream: String,
    d7_stream: String,
}

#[derive(Clone, Eq, PartialEq, Deserialize, Debug, Hash)]
pub struct AudioQualitySettings {
    preset: String,
    connected_device_streaming_preset: String,
}

#[derive(Copy, Clone, Eq, PartialEq, Deserialize, Debug, Hash)]
#[expect(clippy::struct_excessive_bools)]
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
    pub volume_normalization: bool,
    pub remote_control: bool,
    pub remote_control_release: bool,
    free_on_cast: bool,
}

#[serde_as]
#[derive(Copy, Clone, PartialEq, Deserialize, Debug)]
pub struct Gain {
    #[serde(rename = "TARGET")]
    #[serde_as(as = "DisplayFromStr")]
    pub target: i8,
}
