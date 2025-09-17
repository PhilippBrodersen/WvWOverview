use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::processing::Guild;

static API_BASE: &str = "https://api.guildwars2.com/v2";

#[derive(Debug, Clone)]
pub enum APIEndPoint {
    Guild(String),
    AllGuildIDs,
}

impl ToString for APIEndPoint {
    fn to_string(&self) -> String {
        match self {
            APIEndPoint::Guild(id) => format!("{API_BASE}/guild/{id}"),
            APIEndPoint::AllGuildIDs => format!("{API_BASE}/wvw/guilds/eu"),
        }
    }
}


enum Data {
    Guild(Guild),
    Team(Team),
    Match(Match),
}

enum ApiCall {
    AllGuilds,
    GuildInfo(String), //String = guild_id
}


enum Tier {
    One,
    Two,
    Three,
    Four,
    Five,
}

impl Tier {
    fn as_str(&self) -> &str {
        match self {
            Tier::One => "1",
            Tier::Two => "2",
            Tier::Three => "3",
            Tier::Four => "4",
            Tier::Five => "5",
        }
    }
}

// Enum for the three team colors
#[derive(Debug, Eq, PartialEq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")] // maps "red" -> Team::Red
enum TeamColor {
    Red,
    Green,
    Blue,
}

// Struct for just the fields you care about
#[derive(Debug, Deserialize)]
struct Match {
    worlds: HashMap<TeamColor, String>,
    victory_points: HashMap<TeamColor, String>,
}


struct Team {
    id: String,
    guilds: Vec<Guild>,
    score: String,
}

pub async fn fetch_all_wvw_guild_ids() -> Result<HashMap<String, String>, reqwest::Error> {
    let url = &format!("{API_BASE}/wvw/guilds/eu");

    let map: HashMap<String, String> = reqwest::get(url)
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    Ok(map)
}

pub async fn fetch_guild_info(guild_id: &str) -> Result<Guild, reqwest::Error> {
    let url = &format!("{API_BASE}/guild/{guild_id}");

    let guild: Guild = reqwest::get(url).await?.json::<Guild>().await?;
    Ok(guild)
}

pub async fn fetch_match(tier: Tier) -> Result<Match, reqwest::Error> {
    let url = &format!("{}/wvw/matches/2{}", API_BASE, tier.as_str());

    let m: Match = reqwest::get(url).await?.json::<Match>().await?;
    Ok(m)
}
