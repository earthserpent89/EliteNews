use anyhow::Result;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{Pool, Row, Sqlite, SqlitePool};

use crate::types::{ChannelConfig, NewArticle, StoredArticle};

pub async fn connect_and_migrate(database_path: &str) -> Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(database_path)
        .create_if_missing(true);

    let pool = SqlitePool::connect_with(options).await?;

    let schema = include_str!("../sql/schema.sql");
    for statement in schema.split(";") {
        let stmt = statement.trim();
        if !stmt.is_empty() {
            sqlx::query(stmt).execute(&pool).await?;
        }
    }

    Ok(pool)
}

pub async fn upsert_channel_config(
    pool: &Pool<Sqlite>,
    channel_id: &str,
    guild_id: &str,
    content_type: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO channel_config (channel_id, guild_id, is_active, content_types, created_at, updated_at)
         VALUES (?1, ?2, 1, ?3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
         ON CONFLICT(channel_id) DO UPDATE SET
           is_active = 1,
           content_types = excluded.content_types,
           updated_at = CURRENT_TIMESTAMP"
    )
    .bind(channel_id)
    .bind(guild_id)
    .bind(content_type)
    .execute(pool)
    .await?;

    Ok(())
}

pub async fn active_channels(pool: &Pool<Sqlite>) -> Result<Vec<ChannelConfig>> {
    let rows = sqlx::query(
        "SELECT channel_id, content_types FROM channel_config WHERE is_active = 1"
    )
    .fetch_all(pool)
    .await?;

    let channels = rows
        .into_iter()
        .map(|row| ChannelConfig {
            channel_id: row.get::<String, _>("channel_id"),
            content_types: row.get::<String, _>("content_types"),
        })
        .collect();

    Ok(channels)
}

pub async fn article_exists(pool: &Pool<Sqlite>, content_hash: &str) -> Result<bool> {
    let exists = sqlx::query("SELECT 1 FROM article WHERE content_hash = ?1 LIMIT 1")
        .bind(content_hash)
        .fetch_optional(pool)
        .await?
        .is_some();

    Ok(exists)
}

pub async fn insert_article(pool: &Pool<Sqlite>, article: &NewArticle) -> Result<Option<StoredArticle>> {
    sqlx::query(
        "INSERT INTO article (
            external_id, title, summary, body, image_url, content_hash, api_source, published_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT(content_hash) DO NOTHING"
    )
    .bind(&article.external_id)
    .bind(&article.title)
    .bind(&article.summary)
    .bind(&article.body)
    .bind(&article.image_url)
    .bind(&article.content_hash)
    .bind(&article.api_source)
    .bind(article.published_at.to_rfc3339())
    .execute(pool)
    .await?;

    let row = sqlx::query(
        "SELECT id, title, summary, image_url, api_source
         FROM article WHERE content_hash = ?1 LIMIT 1"
    )
    .bind(&article.content_hash)
    .fetch_optional(pool)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    Ok(Some(StoredArticle {
        id: row.get("id"),
        title: row.get("title"),
        summary: row.get("summary"),
        image_url: row.get("image_url"),
        api_source: row.get("api_source"),
    }))
}

pub async fn record_published(
    pool: &Pool<Sqlite>,
    article_id: i64,
    channel_id: &str,
    message_id: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO published_article (article_id, channel_id, message_id, sent_at)
         VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
         ON CONFLICT(article_id, channel_id) DO NOTHING"
    )
    .bind(article_id)
    .bind(channel_id)
    .bind(message_id)
    .execute(pool)
    .await?;

    Ok(())
}
