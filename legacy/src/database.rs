use sqlx::SqlitePool;
use tokio::sync::OnceCell;

use crate::processing::Guild;

static POOL: OnceCell<SqlitePool> = OnceCell::const_new();

pub async fn get_pool() -> &'static SqlitePool {
    POOL.get_or_init(|| async {
        SqlitePool::connect("sqlite://mydb.sqlite")
            .await
            .expect("Failed to connect to DB")
    })
    .await
}

async fn init_db(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r"
        CREATE TABLE IF NOT EXISTS guilds (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            tag TEXT
            normalized_first_letter TEXT
        );
        ",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_guild(guild: Guild) -> Result<(), sqlx::Error> {
    let pool = get_pool().await;

    sqlx::query("INSERT OR REPLACE INTO guilds (id, name, tag) VALUES (?, ?, ?)")
        .bind(&guild.id)
        .bind(&guild.name)
        .bind(&guild.tag)
        .execute(pool)
        .await?;

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

pub async fn guild_exists(guild_id: &str) -> Result<bool, sqlx::Error> {
    let pool: &'static sqlx::Pool<sqlx::Sqlite> = get_pool().await;

    let exists: bool = sqlx::query_scalar::<_, i64>("SELECT 1 FROM guilds WHERE id = ? LIMIT 1")
        .bind(guild_id)
        .fetch_optional(pool)
        .await?
        .is_some();

    Ok(exists)
}
