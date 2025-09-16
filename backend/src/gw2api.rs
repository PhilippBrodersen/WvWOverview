use reqwest::Client;
use serde_json::{Map, Value};
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

/// Fetch all WvW guilds (just IDs)
pub async fn fetch_all_wvw_guilds() -> Value {
    fetch_json(&format!("{}/wvw/guilds/eu", API_BASE)).await
}

/// Fetch detailed guild info
pub async fn fetch_guild_info(guild_id: &str) -> Value {
    fetch_json(&format!("{}/guild/{}", API_BASE, guild_id)).await
}

/// Test: fetch all guilds and then details
pub async fn fetch_all_wvw_guilds_test() -> Vec<Value> {
    let data = fetch_all_wvw_guilds().await;
    let mut guilds: Vec<Value> = Vec::new();

    if let Some(obj) = data.as_object() {
        for guild_id in obj.keys() {
            let guild = fetch_guild_info(guild_id).await;
            guilds.push(guild);
        }
    }
    guilds
}

/// Fetch a match by tier
pub async fn fetch_match(tier: u32) -> Value {
    let data = fetch_json(&format!("{}/wvw/matches/2-{}", API_BASE, tier)).await;

    fn normalize_team_id(team_id: i64) -> String {
        let mut tid = team_id;
        if tid == 2101 {
            tid = 2015;
        }
        format!("1{}", tid)
    }

    if data.is_null() {
        return Value::Null;
    }

    let mut match_info = Map::new();
    if let (Some(worlds), Some(points)) = (data.get("worlds"), data.get("victory_points")) {
        for color in ["red", "blue", "green"] {
            let team_id = worlds[color].as_i64().unwrap_or(0);
            let score = points[color].as_i64().unwrap_or(0);

            let mut team_map = Map::new();
            team_map.insert(
                "team_id".to_string(),
                Value::String(normalize_team_id(team_id)),
            );
            team_map.insert("score".to_string(), Value::Number(score.into()));

            match_info.insert(color.to_string(), Value::Object(team_map));
        }
    }

    let result = Value::Object(match_info);
    println!("{}", result);
    result
}

#[tokio::main]
async fn main() {
    let guilds = fetch_all_wvw_guilds().await;
    println!("Guild IDs: {}", guilds);

    let match_info = fetch_match(1).await;
    println!("Match Info: {}", match_info);
}
