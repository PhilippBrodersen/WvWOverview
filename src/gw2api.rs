#![warn(clippy::pedantic)]

use reqwest::Error;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;

use crate::data::{Guild, Match, Tier};
use crate::tasks::log_error;

struct RateLimiter {
    semaphore: Semaphore,
    last_request: Mutex<Instant>,
    delay: Duration,
}

static RATE_LIMITER: OnceLock<Arc<RateLimiter>> = OnceLock::new();

fn get_rate_limiter() -> Arc<RateLimiter> {
    RATE_LIMITER
        .get_or_init(|| {
            Arc::new(RateLimiter {
                semaphore: Semaphore::new(1),
                last_request: Mutex::new(
                    Instant::now()
                        .checked_sub(Duration::from_millis(200))
                        .unwrap(),
                ),
                delay: Duration::from_millis(200),
            })
        })
        .clone()
}

pub async fn fetch_json<T: serde::de::DeserializeOwned>(url: &str) -> Result<T, Error> {
    let limiter = get_rate_limiter();
    let _permit = limiter.semaphore.acquire().await.unwrap();
    let mut last = limiter.last_request.lock().await;
    let now = Instant::now();
    if let Some(remaining) = limiter.delay.checked_sub(now.duration_since(*last)) {
        sleep(remaining).await;
    }
    let result = reqwest::get(url).await?.json::<T>().await;
    *last = Instant::now();
    result
}

pub async fn fetch_all_wvw_guild_ids() -> Option<HashMap<String, String>> {
    match fetch_json("https://api.guildwars2.com/v2/wvw/guilds/eu").await {
        Ok(map) => Some(map),
        Err(err) => {
            log_error(err);
            None
        }
    }
}

pub async fn fetch_guild_info(guild_id: &str) -> Option<Guild> {
    let url = &format!("https://api.guildwars2.com/v2/guild/{guild_id}");

    let raw_json: Value = match fetch_json::<Value>(url).await {
        Ok(v) => v,
        Err(err) => {
            log_error(err);
            return None;
        }
    };

    serde_json::from_value::<Guild>(raw_json).ok()
}

pub async fn fetch_match(tier: Tier) -> Option<Match> {
    let url = &format!("https://api.guildwars2.com/v2/wvw/matches/{}", tier.as_id());

    match fetch_json(url).await {
        Ok(m) => Some(m),
        Err(err) => {
            log_error(err);
            None
        }
    }
}

pub async fn fetch_guild_id_by_name(guild_name: String) -> Option<String> {
    let url = &format!("https://api.guildwars2.com/v2/guild/search?name={guild_name}");

   let ids: Vec<String> = match fetch_json(url).await {
        Ok(v) => v,
        Err(err) => {
            log_error(err);
            return None;
        }
    };

    ids.into_iter().next()
}
