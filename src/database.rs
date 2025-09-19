#![warn(clippy::pedantic)]

use std::{env, fs, path::Path};

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::data::{Guild, Match, Tier};

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {
    let db_path = "mydb.sqlite";

    if let Ok(exe_path) = env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let db_path = Some(exe_dir.join("mydb.sqlite"));
        }
    }

    let db_url = format!("sqlite://{db_path}");

    // Ensure the file exists
    if !Path::new(db_path).exists() {
        fs::File::create(db_path)?;
        println!("Created new database file: {db_path}");
    }

    // Setup step: handle errors explicitly
    let pool = match SqlitePool::connect(&db_url).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("Failed to connect to database: {e}");
            return Err(e);
        }
    };

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
            last_update TEXT NOT NULL,
            FOREIGN KEY (guild_id) REFERENCES guilds(id) ON DELETE CASCADE
        );
        ",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS guild_team (
            guild_id TEXT PRIMARY KEY,     -- each guild belongs to only one team
            team_id TEXT,
            FOREIGN KEY (guild_id) REFERENCES guilds(id) ON DELETE CASCADE
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

pub async fn add_guild(pool: &SqlitePool, guild: Guild) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT OR REPLACE INTO guilds (id, name, tag) VALUES (?, ?, ?)")
        .bind(&guild.id)
        .bind(&guild.name)
        .bind(&guild.tag)
        .execute(pool)
        .await?;

    upsert_last_updated(pool, &guild.id, Utc::now()).await?;

    Ok(())
}

pub async fn get_guild(pool: &SqlitePool, guild_id: &str) -> Result<Option<Guild>, sqlx::Error> {
    let guild = sqlx::query_as::<_, Guild>("SELECT id, name, tag FROM guilds WHERE id = ?")
        .bind(guild_id)
        .fetch_optional(pool)
        .await?;

    Ok(guild)
}

pub async fn guild_exists(pool: &SqlitePool, guild_id: &str) -> Result<bool, sqlx::Error> {
    let exists: bool = sqlx::query_scalar::<_, i64>("SELECT 1 FROM guilds WHERE id = ? LIMIT 1")
        .bind(guild_id)
        .fetch_optional(pool)
        .await?
        .is_some();

    Ok(exists)
}

pub async fn upsert_last_updated(
    pool: &SqlitePool,
    guild_id: &str,
    timestamp: DateTime<Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT INTO guild_last_updated (guild_id, last_update)
        VALUES (?, ?)
        ON CONFLICT(guild_id) DO UPDATE SET last_update = excluded.last_update;
        ",
    )
    .bind(guild_id)
    .bind(timestamp.to_rfc3339())
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn get_last_guild_update(
    pool: &SqlitePool,
    guild_id: &str,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    let last_update_str: Option<String> =
        sqlx::query_scalar("SELECT last_update FROM guild_last_updated WHERE guild_id = ?")
            .bind(guild_id)
            .fetch_optional(pool)
            .await?;

    if let Some(ts) = last_update_str {
        match ts.parse::<DateTime<Utc>>() {
            Ok(dt) => Ok(Some(dt)),
            Err(_) => Ok(None), // parse error treated as missing
        }
    } else {
        Ok(None)
    }
}

pub async fn upsert_guild_team(
    pool: &SqlitePool,
    guild_id: &str,
    team_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        INSERT INTO guild_team (guild_id, team_id)
        VALUES (?, ?)
        ON CONFLICT(guild_id) DO UPDATE SET team_id = excluded.team_id;
        ",
    )
    .bind(guild_id)
    .bind(team_id) // Option<&str> works; NULL if None
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn upsert_guild_team_null(
    pool: &SqlitePool,
    excluded_guild_ids: Vec<String>,
) -> Result<(), sqlx::Error> {
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

    q.execute(pool).await?;

    Ok(())
}

pub async fn upsert_match(pool: &SqlitePool, m: &Match) -> Result<(), sqlx::Error> {
    sqlx::query(
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
    .await?;

    Ok(())
}

pub async fn get_match(pool: &SqlitePool, tier: Tier) -> Result<Option<Match>, sqlx::Error> {
    sqlx::query_as::<_, Match>(
        r"
        SELECT *
        FROM matches
        WHERE id = ?
        ",
    )
    .bind(tier.as_id())
    .fetch_optional(pool)
    .await
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
