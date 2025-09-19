use reqwest::Error;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Semaphore};
use tokio::time::sleep;

use crate::data::{Guild, Issue, Match, Tier};

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

pub async fn fetch_all_wvw_guild_ids() -> Result<HashMap<String, String>, Error> {
    fetch_json("https://api.guildwars2.com/v2/wvw/guilds/eu").await
}

pub async fn fetch_guild_info(guild_id: &str) -> Result<Option<Guild>, reqwest::Error> {
    let url = &format!("https://api.guildwars2.com/v2/guild/{guild_id}");

    let result: Result<Guild, reqwest::Error> = fetch_json::<Guild>(url).await;

    match result {
        Ok(guild) => Ok(Some(guild)),
        Err(_) => match fetch_json::<Issue>(url).await {
            Ok(_) => Ok(None),
            Err(err) => Err(err),
        },
    }
}

pub async fn fetch_match(tier: Tier) -> Result<Match, reqwest::Error> {
    let url = &format!("https://api.guildwars2.com/v2/wvw/matches/{}", tier.as_id());

    let m: Match = fetch_json(url).await?;
    Ok(m)
}

/* pub async fn fetch_all_guilds(client: &ApiClient, guild_ids: Vec<String>) -> Vec<Guild> {
    // Limit concurrency (optional, e.g., 10 tasks at a time)
    let concurrency_limit = 10;

    stream::iter(guild_ids)
        .map(|id| {
            //let client = client;
            async move {
                let url = format!("https://api.guildwars2.com/v2/guild/{}", id);
                client.fetch_json::<Guild>(&url).await
            }
        })
        .buffer_unordered(concurrency_limit)
        .filter_map(|res| async { res.ok() }) // discard failed requests
        .collect()
        .await
} */
