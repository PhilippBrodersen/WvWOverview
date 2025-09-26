use std::collections::BTreeMap;
use std::fmt::Display;

use serde::{Deserialize, Serialize};
use sqlx::Row;
use sqlx::{prelude::FromRow, sqlite::SqliteRow};

#[derive(Serialize, Deserialize, FromRow)]
pub struct Issue {
    pub text: String,
}

const API_BASE: &str = "https://api.guildwars2.com/v2";

pub enum APIEndpoint {
    Match(Tier),
    Guild(String),
    AllWvWGuilds,
    GuildIDfromName(String),
}

impl Display for APIEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Match(tier) => write!(f, "{API_BASE}/wvw/matches/{}", tier.as_id()),
            Self::Guild(guild_id) => write!(f, "{API_BASE}/guild/{}", guild_id),
            Self::AllWvWGuilds => write!(f, "{API_BASE}/wvw/guilds/eu"),
            Self::GuildIDfromName(guild_name) => {
                write!(f, "{API_BASE}/guild/search?name={guild_name}")
            }
        }
    }
}

#[derive(Serialize, Deserialize, FromRow)]
pub struct Guild {
    pub id: String,
    pub name: String,
    pub tag: String,
}

impl Display for Guild {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} [{}]", self.name, self.tag)
    }
}

#[derive(Serialize, Deserialize)]
pub struct Worlds {
    pub red: u32,
    pub green: u32,
    pub blue: u32,
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
        Ok(Self {
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

#[derive(Clone, Copy)]
pub enum Tier {
    One,
    Two,
    Three,
    Four,
    Five,
}

impl Tier {
    pub fn as_id(self) -> String {
        match self {
            Self::One => "2-1".to_string(),
            Self::Two => "2-2".to_string(),
            Self::Three => "2-3".to_string(),
            Self::Four => "2-4".to_string(),
            Self::Five => "2-5".to_string(),
        }
    }

    pub fn all() -> Vec<Self> {
        vec![Self::One, Self::Two, Self::Three, Self::Four, Self::Five]
    }
}

impl Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::One => "1",
            Self::Two => "2",
            Self::Three => "3",
            Self::Four => "4",
            Self::Five => "5",
        };

        write!(f, "{s}")
    }
}

#[derive(Serialize, Default, Clone, Hash)]
pub struct MatchColor {
    pub team_name: String,
    pub victory_points: String,
    pub guilds: BTreeMap<char, Vec<String>>,
}

#[derive(Serialize, Default, Clone, Hash)]
pub struct MatchData {
    pub red: MatchColor,
    pub green: MatchColor,
    pub blue: MatchColor,
}

#[derive(Serialize, Default, Clone, Hash)]
pub struct Data {
    pub matches: BTreeMap<usize, MatchData>,
    pub important_guilds: Vec<String>,
    pub our_team: String,
}
