use std::{
    num::NonZeroU64,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use serde_with::{formats::Flexible, serde_as, DisplayFromStr, DurationSeconds, TimestampSeconds};

use super::Method;

impl Method for ListData {
    const METHOD: &'static str = "song.getListData";
}

pub type Queue = Vec<ListData>;

#[serde_as]
#[derive(Clone, PartialEq, Deserialize, Debug)]
#[serde(rename_all = "UPPERCASE")]
pub struct ListData {
    #[serde(rename = "SNG_ID")]
    #[serde_as(as = "DisplayFromStr")]
    pub track_id: NonZeroU64,
    #[serde(rename = "ART_NAME")]
    pub artist: String,
    #[serde_as(as = "DurationSeconds<String>")]
    pub duration: Duration,
    #[serde(rename = "SNG_TITLE")]
    pub title: String,
    #[serde(flatten)]
    pub file_size: FileSize,
    #[serde_as(as = "DisplayFromStr")]
    pub gain: f32,
    pub track_token: String,
    #[serde(rename = "TRACK_TOKEN_EXPIRE")]
    #[serde_as(as = "TimestampSeconds<String, Flexible>")]
    pub expiry: SystemTime,
}

#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug, Hash)]
pub struct Request {
    #[serde(rename = "sng_ids")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub track_ids: Vec<NonZeroU64>,
}

// 0 byte file sizes mean: not available in that quality. Note that sometimes
// higher quality files *are* available. Particularly, few if any tracks seem
// available in basic 64 kbps MP3. 256 kbps MP3 seems always unavailable also.
#[serde_as]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Debug, Hash)]
pub struct FileSize {
    #[serde(rename = "FILESIZE_MP3_64")]
    #[serde_as(as = "DisplayFromStr")]
    pub mp3_64: u64,
    #[serde(rename = "FILESIZE_MP3_128")]
    #[serde_as(as = "DisplayFromStr")]
    pub mp3_128: u64,
    #[serde(rename = "FILESIZE_MP3_256")]
    #[serde_as(as = "DisplayFromStr")]
    pub mp3_256: u64,
    #[serde(rename = "FILESIZE_MP3_320")]
    #[serde_as(as = "DisplayFromStr")]
    pub mp3_320: u64,
    #[serde(rename = "FILESIZE_FLAC")]
    #[serde_as(as = "DisplayFromStr")]
    pub flac: u64,
}
