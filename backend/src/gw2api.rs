use reqwest::Client;
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tokio::time::sleep;

static API_BASE: &str = "https://api.guildwars2.com/v2";

lazy_static::lazy_static! {
    static ref SEMAPHORE: Arc<Semaphore> = Arc::new(Semaphore::new(1));
}

/// Fetch JSON with semaphore and rate-limiting.
async fn fetch_json(url: &str) -> Value {
    let permit = match SEMAPHORE.acquire().await {
        Ok(p) => p,
        Err(_) => return Value::Null,
    };

    let client = Client::new();
    let resp = client
        .get(url)
        .timeout(Duration::from_secs(10))
        .send()
        .await;
    let data = match resp {
        Ok(r) => match r.json::<Value>().await {
            Ok(json) => json,
            Err(_) => Value::Null,
        },
        Err(_) => Value::Null,
    };

    // enforce >=0.2s between requests
    sleep(Duration::from_millis(210)).await;
    data
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

#[derive(serde::Deserialize)]
struct Guild {
    id: String,
    name: String,
    tag: String,
}

struct Team {
    id: String,
    guilds: Vec<Guild>,
    score: String
}

pub async fn fetch_all_wvw_guild_ids() -> Result<HashMap<String, String>, reqwest::Error> {
    let url = &format!("{}/wvw/guilds/eu", API_BASE);

    let map: HashMap<String, String> = reqwest::get(url)
        .await?
        .json::<HashMap<String, String>>()
        .await?;
    return Ok(map)
}

pub async fn fetch_guild_info(guild_id: &str) -> Result<Guild, reqwest::Error> {
    let url = &format!("{}/guild/{}", API_BASE, guild_id);

    let guild: Guild = reqwest::get(url)
        .await?
        .json::<Guild>()
        .await?;
    return Ok(guild)
}

pub async fn fetch_match(tier: Tier) -> Result<Match, reqwest::Error> {
    let url = &format!("{}/wvw/matches/2{}", API_BASE, tier.as_str());

    let m: Match = reqwest::get(url)
        .await?
        .json::<Match>()
        .await?;
    return Ok(m)
}