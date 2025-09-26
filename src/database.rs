#![warn(clippy::pedantic)]

/*
Note: all functions in this file swallow errors by just passing to to log_error
*/

use std::{env, fs, path::PathBuf};

use chrono::{DateTime, Duration, Utc};
use sqlx::{Sqlite, SqlitePool, sqlite::SqlitePoolOptions};

use crate::{
    data::{Guild, Match, Tier},
    tasks::log_error,
};

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let default_path = "mydb.sqlite";
    let db_path: PathBuf = env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|dir| dir.join("mydb.sqlite")))
        .unwrap_or_else(|| PathBuf::from(default_path));

    let db_url = format!("sqlite://{}", db_path.to_str().unwrap_or(default_path));

    // Ensure the file exists
    if !db_path.exists() {
        fs::File::create(&db_path)?;
        println!("Created new database file: {}", db_path.display());
    }

    // Setup step: handle errors explicitly

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("PRAGMA journal_mode = WAL;")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("PRAGMA busy_timeout = 5000;")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&db_url)
        .await?;

    sqlx::query("PRAGMA journal_mode = WAL;")
        .execute(&pool)
        .await?;

    sqlx::query("PRAGMA busy_timeout = 5000;") // 5 seconds
        .execute(&pool)
        .await?;

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS guilds (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            tag TEXT
        );
        ",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS guild_last_updated (
            guild_id TEXT PRIMARY KEY,
            last_update TEXT NOT NULL
        );
        ",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS guild_team (
            guild_id TEXT PRIMARY KEY,     -- each guild belongs to only one team
            team_id TEXT
        );
        ",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS matches (
            id TEXT PRIMARY KEY,
            start_time TEXT NOT NULL,
            end_time TEXT NOT NULL,
            red_world INTEGER NOT NULL,
            green_world INTEGER NOT NULL,
            blue_world INTEGER NOT NULL,
            red_vp INTEGER NOT NULL,
            green_vp INTEGER NOT NULL,
            blue_vp INTEGER NOT NULL
        );
        ",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

pub async fn upsert_guild(pool: &SqlitePool, guild: Guild) {
    if let Err(err) = sqlx::query("INSERT OR REPLACE INTO guilds (id, name, tag) VALUES (?, ?, ?)")
        .bind(&guild.id)
        .bind(&guild.name)
        .bind(&guild.tag)
        .execute(pool)
        .await
    {
        log_error(err);
        return;
    }

    if let Err(err) = upsert_last_updated(pool, &guild.id, Utc::now()).await {
        log_error(err);
    }
}

pub async fn get_guild(pool: &SqlitePool, guild_id: &str) -> Result<Option<Guild>, sqlx::Error> {
    let guild = sqlx::query_as::<_, Guild>("SELECT id, name, tag FROM guilds WHERE id = ?")
        .bind(guild_id)
        .fetch_optional(pool)
        .await?;

    Ok(guild)
}

pub async fn guild_in_db(pool: &SqlitePool, guild_id: &str) -> bool {
    match sqlx::query_scalar::<_, i64>("SELECT 1 FROM guilds WHERE id = ? LIMIT 1")
        .bind(guild_id)
        .fetch_optional(pool)
        .await
    {
        Ok(a) => a.is_some(),
        Err(err) => {
            log_error(err);
            false
        }
    }
}

pub async fn upsert_last_updated(
    pool: &SqlitePool,
    guild_id: &str,
    timestamp: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT OR REPLACE  INTO guild_last_updated (guild_id, last_update) VALUES (?, ?)
        ",
    )
    .bind(guild_id)
    .bind(timestamp.to_rfc3339())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_last_guild_update(pool: &SqlitePool, guild_id: &str) -> Option<DateTime<Utc>> {
    match sqlx::query_scalar::<Sqlite, String>("SELECT last_update FROM guild_last_updated WHERE guild_id = ?")
            .bind(guild_id)
            .fetch_optional(pool)
            .await //.map(|ts: String| ts.parse::<DateTime<Utc>>().ok()).flatten()
        {
            Ok(Some(ts)) => ts.parse::<DateTime<Utc>>().ok(),
            Ok(None) => None,
            Err(err) => {
                log_error(err);
                None
            }
        }
}

pub async fn upsert_guild_teams_bulk(pool: &SqlitePool, guild_list: Vec<(String, String)>) {
    if guild_list.is_empty() {
        return;
    }

    let placeholders: Vec<String> = (0..guild_list.len())
        .map(|_| "(?, ?)".to_string())
        .collect();

    let sql = format!(
        "INSERT OR REPLACE INTO guild_team (guild_id, team_id) VALUES {};",
        placeholders.join(", ")
    );

    let mut query = sqlx::query(&sql);

    for (guild_id, team_id) in guild_list {
        query = query.bind(guild_id);
        query = query.bind(team_id);
    }

    if let Err(err) = query.execute(pool).await {
        log_error(err);
    }
}

pub async fn upsert_guild_team(pool: &SqlitePool, guild_id: &str, team_id: Option<&str>) {
    if let Err(err) = sqlx::query(
        r"
        INSERT OR REPLACE INTO guild_team (guild_id, team_id)
        VALUES (?, ?)
        ",
    )
    .bind(guild_id)
    .bind(team_id) // Option<&str> works; NULL if None
    .execute(pool)
    .await
    {
        let a: String = guild_id.to_string();
        let b: String = team_id.unwrap_or("none").to_string();

        log_error(err);
    }
}

pub async fn guilds_to_update(pool: &SqlitePool) -> Vec<String> {
    let cutoff = Utc::now() - Duration::hours(24);
    let cutoff_str = cutoff.to_rfc3339();

    match sqlx::query_scalar::<_, String>(
        r"
        SELECT guild_id
        FROM guild_last_updated
        WHERE last_update < ?
        ",
    )
    .bind(&cutoff_str)
    .fetch_all(pool)
    .await
    {
        Ok(ids) => ids,
        Err(err) => {
            log_error(err);
            Vec::new()
        }
    }
}

pub async fn upsert_guild_team_null(pool: &SqlitePool, excluded_guild_ids: Vec<String>) {
    let placeholders = excluded_guild_ids
        .iter()
        .enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>()
        .join(", ");

    let query =
        format!("UPDATE guild_team SET team_id = NULL WHERE guild_id NOT IN ({placeholders})");

    let mut q = sqlx::query(&query);
    for id in &excluded_guild_ids {
        q = q.bind(id);
    }

    if let Err(err) = q.execute(pool).await {
        log_error(err);
    }
}

pub async fn upsert_match(pool: &SqlitePool, m: &Match) {
    let query_result = sqlx::query(
        r"
        INSERT INTO matches (
            id, start_time, end_time,
            red_world, green_world, blue_world,
            red_vp, green_vp, blue_vp
        )
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(id) DO UPDATE SET
            start_time = excluded.start_time,
            end_time = excluded.end_time,
            red_world = excluded.red_world,
            green_world = excluded.green_world,
            blue_world = excluded.blue_world,
            red_vp = excluded.red_vp,
            green_vp = excluded.green_vp,
            blue_vp = excluded.blue_vp;
        ",
    )
    .bind(&m.id)
    .bind(&m.start_time)
    .bind(&m.end_time)
    .bind(m.worlds.red)
    .bind(m.worlds.green)
    .bind(m.worlds.blue)
    .bind(m.victory_points.red)
    .bind(m.victory_points.green)
    .bind(m.victory_points.blue)
    .execute(pool)
    .await;

    if let Err(err) = query_result {
        log_error(err);
    }
}

pub async fn get_match(pool: &SqlitePool, tier: Tier) -> Option<Match> {
    match sqlx::query_as::<_, Match>(
        r"
        SELECT *
        FROM matches
        WHERE id = ?
        ",
    )
    .bind(tier.as_id())
    .fetch_optional(pool)
    .await
    {
        Ok(m) => m,
        Err(err) => {
            log_error(err);
            None
        }
    }
}

pub async fn get_guild_team(
    pool: &SqlitePool,
    guild_id: &str,
) -> Result<Option<Option<String>>, sqlx::Error> {
    // Returns Some(team_id) if exists, Some(None) if explicitly NULL, None if guild not found
    let team_id: Option<String> =
        sqlx::query_scalar("SELECT team_id FROM guild_team WHERE guild_id = ?")
            .bind(guild_id)
            .fetch_optional(pool)
            .await?;

    Ok(Some(team_id))
}

pub async fn get_all_guild_teams(
    pool: &SqlitePool,
) -> Result<Vec<(String, Option<String>)>, sqlx::Error> {
    // Returns list of (guild_id, Option<team_id>)
    let rows =
        sqlx::query_as::<_, (String, Option<String>)>("SELECT guild_id, team_id FROM guild_team")
            .fetch_all(pool)
            .await?;

    Ok(rows)
}

pub async fn get_all_guilds(pool: &SqlitePool) -> Result<Vec<Guild>, sqlx::Error> {
    sqlx::query_as::<_, Guild>("SELECT id, name, tag FROM guilds")
        .fetch_all(pool)
        .await
}

pub async fn get_all_matches(pool: &SqlitePool) -> Result<Vec<Match>, sqlx::Error> {
    let m = sqlx::query_as::<_, Match>(
        r"
        SELECT 
            id,
            start_time,
            end_time,
            red_world,
            green_world,
            blue_world,
            red_vp,
            green_vp,
            blue_vp
        FROM matches
        ",
    )
    .fetch_all(pool)
    .await?;

    Ok(m)
}

pub async fn get_guilds_for_team(
    pool: &SqlitePool,
    team_id: &str,
) -> Result<Vec<Guild>, sqlx::Error> {
    let guilds: Vec<Guild> = sqlx::query_as::<_, Guild>(
        r"
        SELECT g.id, g.name, g.tag
        FROM guilds g
        JOIN guild_team gt ON gt.guild_id = g.id
        WHERE gt.team_id = ?
        ",
    )
    .bind(team_id)
    .fetch_all(pool)
    .await?;

    Ok(guilds)
}

pub async fn get_team_id_for_guild(
    pool: &SqlitePool,
    guild_name: &str,
) -> Result<Option<String>, sqlx::Error> {
    let team_id: Option<String> = sqlx::query_scalar(
        r"
        SELECT gt.team_id
        FROM guilds g
        JOIN guild_team gt ON gt.guild_id = g.id
        WHERE g.name = ?
        ",
    )
    .bind(guild_name)
    .fetch_optional(pool)
    .await?;

    Ok(team_id)
}
