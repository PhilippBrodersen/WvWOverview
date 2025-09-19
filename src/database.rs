use std::{fs, path::Path, sync::{Arc, LazyLock}};

use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use tokio::sync::{mpsc::{self, Sender}, Notify, OnceCell};

use crate::data::Guild;

static POOL: OnceCell<SqlitePool> = OnceCell::const_new();

pub async fn get_pool() -> &'static SqlitePool {
    POOL.get_or_init(|| async {
        SqlitePool::connect("sqlite://mydb.sqlite")
            .await
            .expect("Failed to connect to DB")
    })
    .await
}

pub async fn init_db() -> Result<SqlitePool, sqlx::Error> {

    let db_path = "mydb.sqlite";
    let db_url = format!("sqlite://{}", db_path);

    // Ensure the file exists
    if !Path::new(db_path).exists() {
        fs::File::create(db_path)?;
        println!("Created new database file: {}", db_path);
    }

    // Setup step: handle errors explicitly
    let pool = match SqlitePool::connect(&db_url).await {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("Failed to connect to database: {e}");
            return Err(e.into());
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
            team_id TEXT NOT NULL,
            FOREIGN KEY (guild_id) REFERENCES guilds(id) ON DELETE CASCADE
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

    upsert_last_updated(&pool, &guild.id, Utc::now()).await;

    Ok(())
}

pub async fn get_guild(pool: &SqlitePool, guild_id: &str) -> Result<Option<Guild>, sqlx::Error> {
    let pool = get_pool().await;

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
        r#"
        INSERT INTO guild_last_updated (guild_id, last_update)
        VALUES (?, ?)
        ON CONFLICT(guild_id) DO UPDATE SET last_update = excluded.last_update;
        "#
    )
    .bind(guild_id)
    .bind(&timestamp.to_rfc3339())
    .execute(pool)
    .await?;

    Ok(())
}


pub async fn get_last_guild_update(
    pool: &SqlitePool,
    guild_id: &str,
) -> Result<Option<DateTime<Utc>>, sqlx::Error> {
    let last_update_str: Option<String> = sqlx::query_scalar(
        "SELECT last_update FROM guild_last_updated WHERE guild_id = ?"
    )
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
    team_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO guild_team (guild_id, team_id)
        VALUES (?, ?)
        ON CONFLICT(guild_id) DO UPDATE SET team_id = excluded.team_id;
        "#
    )
    .bind(guild_id)
    .bind(team_id)
    .execute(pool)
    .await?;

    Ok(())
}

