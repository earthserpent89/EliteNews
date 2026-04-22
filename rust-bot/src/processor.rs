use chrono::Utc;
use sha2::{Digest, Sha256};

use crate::types::{ApiArticleRaw, NewArticle};

pub fn transform_articles(galnet: Vec<ApiArticleRaw>, news: Vec<ApiArticleRaw>) -> Vec<NewArticle> {
    let mut out = Vec::new();

    for article in galnet {
        out.push(transform_one(article, "galnet"));
    }

    for article in news {
        out.push(transform_one(article, "news"));
    }

    out
}

fn transform_one(article: ApiArticleRaw, fallback_source: &str) -> NewArticle {
    let source = article.api_source_hint.as_deref().unwrap_or(fallback_source);
    let title = article.title.unwrap_or_else(|| "Untitled".to_string());
    let summary = article.summary.unwrap_or_default();
    let body = article.body.unwrap_or_default();
    let content_hash = hash_content(&title, &body);
    let image_url = extract_image_url(&body);

    let external_id = article
        .id
        .or(article.alt_id)
        .map(|v| v.to_string())
        .unwrap_or_else(|| format!("{}-{}", source, Utc::now().timestamp_millis()));

    NewArticle {
        external_id,
        title,
        summary,
        body,
        image_url,
        content_hash,
        api_source: source.to_string(),
        published_at: article.published_at.unwrap_or_else(Utc::now),
    }
}

fn hash_content(title: &str, body: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(title.as_bytes());
    hasher.update(b"|");
    hasher.update(body.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn extract_image_url(body: &str) -> Option<String> {
    let lower = body.to_lowercase();
    let img_pos = lower.find("<img")?;
    let src_pos = lower[img_pos..].find("src=")? + img_pos;
    let quote_start = body[src_pos..].find('"').map(|i| i + src_pos)
        .or_else(|| body[src_pos..].find('\'').map(|i| i + src_pos))?;

    let quote_char = body.chars().nth(quote_start)?;
    let remainder = &body[(quote_start + 1)..];
    let quote_end_rel = remainder.find(quote_char)?;
    let url = &remainder[..quote_end_rel];

    if url.starts_with("http://") || url.starts_with("https://") {
        Some(url.to_string())
    } else {
        None
    }
}
