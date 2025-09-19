use std::{collections::HashMap, fmt, fs::OpenOptions, io::Write};

use chrono::{Duration, Utc};
use futures::{StreamExt, stream::FuturesUnordered};
use sqlx::SqlitePool;
use tokio::{
    sync::{
        Mutex, Semaphore,
        mpsc::{self, Receiver},
    },
    task::JoinHandle,
    time::{self, sleep},
};

use crate::{
    data::{Guild, Tier},
    database::{add_guild, get_last_guild_update, guild_exists, upsert_guild_team, upsert_match},
    gw2api::{fetch_all_wvw_guild_ids, fetch_guild_info, fetch_match},
};

fn log_error<E: fmt::Debug>(err: E) {
    let debug_str = format!("{err:?}\n");

    if let Ok(mut file) = OpenOptions::new()
        .create(true)
        .append(true)
        .open("error.log")
    {
        if let Err(e) = file.write_all(debug_str.as_bytes()) {
            eprintln!("Failed to write to log file: {e:?}");
        }
    } else {
        eprintln!("Failed to open error.log");
    }
}

pub async fn run_match_updater(pool: &SqlitePool) {
    let mut interval = time::interval(tokio::time::Duration::from_secs(60));

    let pool = pool.clone();
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            update_matches(&pool).await;
        }
    });
}

pub async fn update_matches(pool: &SqlitePool) {
    let mut tasks = FuturesUnordered::new();
    println!("updating matches");
    for tier in Tier::all() {
        let pool: sqlx::Pool<sqlx::Sqlite> = pool.clone();
        tasks.push(tokio::spawn(async move {
            match fetch_match(tier).await {
                Ok(m) => {
                    if let Err(err) = upsert_match(&pool, &m).await {
                        println!("AAAAAA");
                        log_error(err);
                    }
                }
                Err(err) => {
                    println!("BBBBB");
                    log_error(err);
                }
            };
        }));
    }

    while tasks.next().await.is_some() {}
}

pub async fn run_guild_updater(pool: &SqlitePool) {
    let mut interval = time::interval(tokio::time::Duration::from_secs(60));

    let pool = pool.clone();
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            update_guilds(&pool).await;
        }
    });
}

pub async fn update_guilds(pool: &SqlitePool) {
    let mut tasks = FuturesUnordered::new();
    println!("HI");

    let result: HashMap<String, String> = match fetch_all_wvw_guild_ids().await {
        Ok(ids) => ids,
        Err(err) => {
            log_error(err);
            return;
        }
    };
    let guild_ids: Vec<String> = result.keys().cloned().collect();

    println!("Looping over ids");

    for (guild_id, team_id) in result {
        let pool: sqlx::Pool<sqlx::Sqlite> = pool.clone();
        tasks.push(tokio::spawn(async move {
            let exists = guild_exists(&pool, &guild_id).await.unwrap_or_else(|err| {
                log_error(err);
                false
            });
            let last_update = get_last_guild_update(&pool, &guild_id)
                .await
                .unwrap_or_else(|err| {
                    log_error(err);
                    None
                });

            if !exists || last_update.map_or(true, |ts| Utc::now() - ts > Duration::hours(24)) {
                match fetch_guild_info(&guild_id).await {
                    Ok(guild) => {
                        if let Err(err) = add_guild(&pool, guild).await {
                            log_error(err);
                        }
                    }
                    Err(err) => log_error(err),
                }
            }

            let team_id: Option<&str> = if team_id.is_empty() { None } else { Some(&team_id) };
            if let Err(err) = upsert_guild_team(&pool, &guild_id, team_id).await {
                log_error(err);
            }
        }));
    }

    while tasks.next().await.is_some() {}
}
