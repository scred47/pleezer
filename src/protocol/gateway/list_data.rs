use std::{
    num::NonZeroU64,
    time::{Duration, SystemTime},
};

use serde::{Deserialize, Serialize};
use serde_with::{formats::Flexible, serde_as, DisplayFromStr, DurationSeconds, TimestampSeconds};
use veil::Redact;

use super::Method;

impl Method for ListData {
    const METHOD: &'static str = "song.getListData";
}

pub type Queue = Vec<ListData>;

#[serde_as]
#[derive(Clone, PartialEq, Deserialize, Redact)]
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
    #[serde_as(as = "Option<DisplayFromStr>")]
    pub gain: Option<f32>,
    #[redact]
    pub track_token: String,
    #[serde(rename = "TRACK_TOKEN_EXPIRE")]
    #[serde_as(as = "TimestampSeconds<i64, Flexible>")]
    pub expiry: SystemTime,
}

#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug, Hash)]
pub struct Request {
    #[serde(rename = "sng_ids")]
    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub track_ids: Vec<NonZeroU64>,
}
