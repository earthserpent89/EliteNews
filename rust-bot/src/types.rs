use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Clone, Deserialize)]
pub struct ApiArticleRaw {
    pub id: Option<Value>,
    #[serde(rename = "_id")]
    pub alt_id: Option<Value>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub body: Option<String>,
    #[serde(rename = "published_at")]
    pub published_at: Option<DateTime<Utc>>,
    #[serde(skip)]
    pub api_source_hint: Option<String>,
}

#[derive(Debug, Clone)]
pub struct NewArticle {
    pub external_id: String,
    pub title: String,
    pub summary: String,
    pub body: String,
    pub image_url: Option<String>,
    pub content_hash: String,
    pub api_source: String,
    pub published_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredArticle {
    pub id: i64,
    pub title: String,
    pub summary: String,
    pub image_url: Option<String>,
    pub api_source: String,
}

#[derive(Debug, Clone)]
pub struct ChannelConfig {
    pub channel_id: String,
    pub content_types: String,
}

#[derive(Debug, Clone, Default)]
pub struct PollResult {
    pub fetched: usize,
    pub new_articles: usize,
    pub inserted: usize,
    pub sent: usize,
    pub failed: usize,
    pub duration_ms: u128,
    pub error: Option<String>,
}
