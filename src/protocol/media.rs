use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use serde_with::{serde_as, DisplayFromStr};
use url::Url;

use super::connect::AudioQuality;

#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug, Hash)]
pub struct Request {
    pub license_token: String,
    pub media: Vec<Media>,
    pub track_tokens: Vec<String>,
}

#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug, Hash)]
pub struct Media {
    #[serde(rename = "type")]
    #[serde_with(as = "DisplayFromStr")]
    pub typ: Type,
    #[serde(rename = "formats")]
    pub cipher_formats: Vec<CipherFormat>,
}

#[derive(
    Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash,
)]
pub enum Type {
    #[default]
    FULL,
    PREVIEW,
}

#[serde_as]
#[derive(
    Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash,
)]
pub struct CipherFormat {
    #[serde_with(as = "DisplayFromStr")]
    pub cipher: Cipher,
    #[serde_with(as = "DisplayFromStr")]
    pub format: Format,
}

#[derive(
    Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize, Debug, Hash,
)]
#[expect(non_camel_case_types)]
pub enum Cipher {
    #[default]
    BF_CBC_STRIPE,
    NONE,
}

#[derive(
    Copy,
    Clone,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Deserialize_repr,
    Serialize_repr,
    Debug,
    Hash,
)]
#[expect(non_camel_case_types)]
#[repr(i64)]
pub enum Format {
    EXTERNAL = -1,
    FLAC = 9,
    MP3_64 = 10,
    #[default]
    MP3_128 = 1,
    MP3_320 = 3,
    MP3_MISC = 0,
}

impl From<Format> for AudioQuality {
    fn from(format: Format) -> Self {
        match format {
            Format::MP3_64 => AudioQuality::Basic,
            Format::MP3_128 => AudioQuality::Standard,
            Format::MP3_320 => AudioQuality::High,
            Format::FLAC => AudioQuality::Lossless,
            _ => AudioQuality::Unknown,
        }
    }
}

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Debug, Hash)]
pub struct Response {
    pub media: Vec<Medium>,
}

#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Debug, Hash)]
pub struct Medium {
    #[serde_with(as = "DisplayFromStr")]
    pub media_type: Type,
    pub cipher_type: CipherType,
    #[serde_with(as = "DisplayFromStr")]
    pub format: Format,
    pub sources: Vec<Source>,
    #[serde(rename = "nbf")]
    pub not_before: SystemTime,
    #[serde(rename = "exp")]
    pub expiry: SystemTime,
}

#[serde_as]
#[derive(Copy, Clone, Default, Eq, PartialEq, Ord, PartialOrd, Deserialize, Debug, Hash)]
pub struct CipherType {
    #[serde(rename = "type")]
    #[serde_with(as = "DisplayFromStr")]
    pub typ: Cipher,
}

#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Debug, Hash)]
pub struct Source {
    #[serde_as(as = "DisplayFromStr")]
    pub url: Url,
    pub provider: String,
}
