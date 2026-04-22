use std::collections::HashSet;

use anyhow::Result;
use regex::Regex;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};

use crate::types::ApiArticleRaw;

const NEWS_RSS_ROUTE: &str = "https://www.elitedangerous.com/news/rss";
const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const DEFAULT_NEWS_IMAGE: &str = "https://www.elitedangerous.com/images/og_image.jpg";

pub async fn fetch_all_articles(client: &reqwest::Client) -> Result<(Vec<ApiArticleRaw>, Vec<ApiArticleRaw>)> {
    let html = client
        .get(NEWS_RSS_ROUTE)
        .header(USER_AGENT, BROWSER_USER_AGENT)
        .header(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header(ACCEPT_LANGUAGE, "en-GB,en;q=0.9")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let articles = extract_articles_from_payload(&html)?;

    let mut galnet = Vec::new();
    let mut news = Vec::new();

    for article in articles {
        if article.api_source_hint.as_deref() == Some("galnet") {
            galnet.push(article);
        } else {
            news.push(article);
        }
    }

    Ok((galnet, news))
}

fn extract_articles_from_payload(html: &str) -> Result<Vec<ApiArticleRaw>> {
    let slug_re = Regex::new(r#""field-slug":"([a-z0-9\-]+)""#)?;
    let mut seen = HashSet::new();
    let mut out = Vec::new();

    for caps in slug_re.captures_iter(html) {
        let Some(m) = caps.get(0) else { continue; };
        let Some(slug_m) = caps.get(1) else { continue; };
        let slug = slug_m.as_str();

        if matches!(slug, "rss" | "news" | "galnet") {
            continue;
        }

        let raw_start = m.start().saturating_sub(3000);
        let ctx_start = (0..=raw_start).rev().find(|&i| html.is_char_boundary(i)).unwrap_or(0);
        let raw_end = (m.end() + 400).min(html.len());
        let ctx_end = (raw_end..=html.len()).find(|&i| html.is_char_boundary(i)).unwrap_or(html.len());
        let context = &html[ctx_start..ctx_end];

        if !(context.contains("summary:") && context.contains("\"field-category\":")) {
            continue;
        }

        if !seen.insert(slug.to_string()) {
            continue;
        }

        let is_galnet = context.contains("/news/galnet/") || context.contains("galnet");

        let summary = extract_js_string(context, "summary").filter(|s| s.len() > 5);
        let image_url = extract_zaonce_image(context);
        let title = extract_js_string(context, "title")
            .filter(|t| t.len() > 2 && !t.eq_ignore_ascii_case("news") && !t.eq_ignore_ascii_case("galnet"))
            .unwrap_or_else(|| title_from_slug(slug));

        let summary_text = summary.clone().unwrap_or_default();
        let body = match image_url {
            Some(ref img) => format!("<img src=\"{}\" />\n<p>{}</p>", img, summary_text),
            None => format!("<img src=\"{}\" />\n<p>{}</p>", DEFAULT_NEWS_IMAGE, summary_text),
        };

        out.push(ApiArticleRaw {
            id: None,
            alt_id: None,
            title: Some(title),
            summary: Some(summary_text),
            body: Some(body),
            published_at: None,
            api_source_hint: Some(if is_galnet { "galnet".to_string() } else { "news".to_string() }),
        });
    }

    Ok(out)
}

fn extract_js_string(context: &str, field: &str) -> Option<String> {
    let pattern = format!("{}:\"((?:[^\"\\\\]|\\\\.)*)\"" , regex::escape(field));
    let re = Regex::new(&pattern).ok()?;
    let caps = re.captures(context)?;
    let raw = caps.get(1)?.as_str();
    Some(
        raw.replace("\\\"", "\"")
            .replace("\\'", "'")
            .replace("\\n", " ")
            .replace("\\t", " ")
            .replace("\\u003C", "<")
            .replace("\\u003E", ">")
            .replace("\\u0026", "&"),
    )
}

fn extract_zaonce_image(context: &str) -> Option<String> {
    let re = Regex::new(r#"https:\\u002F\\u002Fcms-cdn\.zaonce\.net[^\s"<]+"#).ok()?;
    let m = re.find(context)?;
    let raw = m.as_str().trim_end_matches('\\');
    Some(raw.replace("\\u002F", "/"))
}

fn title_from_slug(slug: &str) -> String {
    slug.split('-')
        .filter(|p| !p.is_empty())
        .map(|p| {
            let mut c = p.chars();
            match c.next() {
                Some(f) => f.to_ascii_uppercase().to_string() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

