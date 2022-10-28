pub mod user_data;

pub use user_data::{UserData, UserDataResponse};

pub trait Method<'a> {
    const METHOD: &'a str;
}
