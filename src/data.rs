use chrono::{DateTime, NaiveDateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize, de::Error};
use sqlx::Row;
use sqlx::{prelude::FromRow, sqlite::SqliteRow};

#[derive(Serialize, Deserialize, FromRow)]
pub struct Guild {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) tag: String,
}

#[derive(Serialize, Deserialize)]
pub struct Worlds {
    pub red: i32,
    pub green: i32,
    pub blue: i32,
}

#[derive(Serialize, Deserialize)]
pub struct VictoryPoints {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
}

#[derive(Serialize, Deserialize)]
pub struct Match {
    pub id: String,
    pub start_time: String,
    pub end_time: String,
    pub worlds: Worlds,
    pub victory_points: VictoryPoints,
}

impl<'r> FromRow<'r, SqliteRow> for Match {
    fn from_row(row: &'r SqliteRow) -> Result<Self, sqlx::Error> {
        Ok(Match {
            id: row.try_get("id")?,
            start_time: row.try_get("start_time")?,
            end_time: row.try_get("end_time")?,
            worlds: Worlds {
                red: row.try_get("red_world")?,
                green: row.try_get("green_world")?,
                blue: row.try_get("blue_world")?,
            },
            victory_points: VictoryPoints {
                red: row.try_get("red_vp")?,
                green: row.try_get("green_vp")?,
                blue: row.try_get("blue_vp")?,
            },
        })
    }
}

pub enum Tier {
    One,
    Two,
    Three,
    Four,
    Five,
}

impl Tier {
    pub fn as_id(&self) -> String {
        match self {
            Tier::One => "2-1".to_string(),
            Tier::Two => "2-2".to_string(),
            Tier::Three => "2-3".to_string(),
            Tier::Four => "2-4".to_string(),
            Tier::Five => "2-5".to_string(),
        }
    }

    pub fn all() -> Vec<Tier> {
        vec![Tier::One, Tier::Two, Tier::Three, Tier::Four, Tier::Five]
    }
}
