

use std::{collections::HashMap, fmt, fs::OpenOptions, io::Write};

use chrono::{Duration, Utc};
use futures::{stream::FuturesUnordered, StreamExt};
use sqlx::SqlitePool;
use tokio::{sync::{mpsc::{self, Receiver}, Mutex, Semaphore}, time::sleep};

use crate::{data::Guild, database::{add_guild, get_last_guild_update, guild_exists, upsert_guild_team}, gw2api::{fetch_all_wvw_guild_ids, fetch_guild_info}};

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

pub async fn update_guilds(pool: &SqlitePool) {
    let mut tasks = FuturesUnordered::new();

    println!("HI");

    let result: HashMap<String, String> = match fetch_all_wvw_guild_ids().await {
        Ok(ids) => ids,
        Err(err) => {log_error(err); return;}
    };
    let guild_ids: Vec<String> = result.keys().cloned().collect();

    println!("Looping over ids");

    for (guild_id, team_id) in result {
        let pool: sqlx::Pool<sqlx::Sqlite> = pool.clone();
        tasks.push(tokio::spawn(async move {

            let exists = guild_exists(&pool, &guild_id).await.unwrap_or_else(|err| {log_error(err); false});
            let last_update = get_last_guild_update(&pool, &guild_id).await.unwrap_or_else(|err| {log_error(err); None});

            upsert_guild_team(&pool, &guild_id, &team_id).await; //does not do anythnig since it needs the guild to exist....

            if !exists || last_update.map_or(true, |ts| Utc::now() - ts > Duration::hours(24)) {
                match fetch_guild_info(&guild_id).await {
                    Ok(guild) => {
                        if let Err(err) = add_guild(&pool, guild).await {
                            log_error(err);
                        }
                    }
                    Err(err) => log_error(err)
                }
            }
        }));
    }

    // Wait for all tasks to finish
    while let Some(_) = tasks.next().await {}
}