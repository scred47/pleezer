use super::Method;
use serde::Deserialize;

impl Method for GetArl {
    const METHOD: &'static str = "user.getArl";
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct GetArl(pub String);
