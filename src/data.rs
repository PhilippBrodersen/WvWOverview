use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer, de::Error};
use sqlx::prelude::FromRow;



#[derive(Deserialize, FromRow)]
pub struct Guild {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) tag: String,
}
