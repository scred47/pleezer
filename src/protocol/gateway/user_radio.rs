use std::ops::Deref;

use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DisplayFromStr};

use super::{ListData, Method};
use crate::protocol::connect::UserId;

impl Method for UserRadio {
    const METHOD: &'static str = "radio.getUserRadio";
}

#[derive(Clone, PartialEq, Deserialize, Debug)]
pub struct UserRadio(pub ListData);

impl Deref for UserRadio {
    type Target = ListData;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[serde_as]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Serialize, Debug, Hash)]
pub struct Request {
    #[serde_as(as = "DisplayFromStr")]
    pub user_id: UserId,
}
