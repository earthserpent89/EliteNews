use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use std::collections::HashSet;

use anyhow::Result;
use regex::Regex;
use reqwest::header::{ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};
use serenity::all::{
    ChannelId, Command, CommandDataOptionValue, CommandOptionType, Context, CreateCommand,
    CreateCommandOption, CreateEmbed, CreateInteractionResponse,
    CreateInteractionResponseMessage, CreateMessage, EditInteractionResponse, EventHandler,
    GatewayIntents, GuildId, Interaction, Permissions, Ready,
};
use sqlx::SqlitePool;
use tracing::{error, info, warn};

use crate::config::Config;
use crate::{db, elite_api, processor, types::PollResult};

pub struct AppState {
    pub config: Config,
    pub db: SqlitePool,
    pub http_client: reqwest::Client,
    pub poller_started: AtomicBool,
}

pub struct Handler {
    state: Arc<AppState>,
}

impl Handler {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[serenity::async_trait]
impl EventHandler for Handler {
    async fn ready(&self, ctx: Context, ready: Ready) {
        info!("Logged in as {}", ready.user.tag());

        if let Err(err) = register_commands(&ctx, &self.state.config).await {
            error!("Failed to register commands: {err:#}");
        }

        if !self.state.poller_started.swap(true, Ordering::SeqCst) {
            let state = Arc::clone(&self.state);
            tokio::spawn(async move {
                run_polling_loop(ctx, state).await;
            });
        }
    }

    async fn interaction_create(&self, ctx: Context, interaction: Interaction) {
        let Interaction::Command(command) = interaction else {
            return;
        };

        match command.data.name.as_str() {
            "setnewschannel" => {
                if let Err(err) = handle_set_channel(&ctx, &self.state, &command).await {
                    error!("setnewschannel failed: {err:#}");
                }
            }
            "checknews" => {
                if let Err(err) = handle_checknews(&ctx, &self.state, &command).await {
                    error!("checknews failed: {err:#}");
                }
            }
            _ => {}
        }
    }
}

pub fn intents() -> GatewayIntents {
    GatewayIntents::GUILDS
}

async fn register_commands(ctx: &Context, config: &Config) -> Result<()> {
    let commands = vec![setnewschannel_command(), checknews_command()];

    if let Some(guild_id) = config.discord_guild_id {
        match GuildId::new(guild_id).set_commands(&ctx.http, commands.clone()).await {
            Ok(_) => {
                info!("Registered guild commands for {}", guild_id);

                // If we previously registered global commands (fallback mode),
                // they can appear as duplicates alongside guild commands.
                // Clear global commands once guild registration succeeds.
                if let Err(err) = Command::set_global_commands(&ctx.http, Vec::new()).await {
                    warn!(
                        "Failed to clear stale global commands after guild registration: {}",
                        err
                    );
                } else {
                    info!("Cleared stale global commands to avoid duplicate command entries");
                }
            }
            Err(err) => {
                warn!(
                    "Guild command registration failed for {} ({}). Falling back to global commands.",
                    guild_id,
                    err
                );
                Command::set_global_commands(&ctx.http, commands).await?;
                info!("Registered global commands (fallback)");
            }
        }
    } else {
        Command::set_global_commands(&ctx.http, commands).await?;
        info!("Registered global commands");
    }

    Ok(())
}

fn setnewschannel_command() -> CreateCommand {
    CreateCommand::new("setnewschannel")
        .description("Set the Discord channel where news articles will be posted")
        .default_member_permissions(Permissions::MANAGE_GUILD)
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::Channel,
                "channel",
                "The channel to post articles to",
            )
            .required(true),
        )
        .add_option(
            CreateCommandOption::new(
                CommandOptionType::String,
                "content_type",
                "Type of content to post",
            )
            .add_string_choice("Galnet Only", "galnet")
            .add_string_choice("News Only", "news")
            .add_string_choice("Both", "both"),
        )
}

fn checknews_command() -> CreateCommand {
    CreateCommand::new("checknews")
        .description("Manually trigger a news polling cycle")
        .default_member_permissions(Permissions::MANAGE_GUILD)
}

async fn handle_set_channel(
    ctx: &Context,
    state: &Arc<AppState>,
    command: &serenity::all::CommandInteraction,
) -> Result<()> {
    // Acknowledge immediately to avoid Discord's 3-second interaction timeout.
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Defer(
                CreateInteractionResponseMessage::new().ephemeral(true),
            ),
        )
        .await?;

    let mut channel_id: Option<u64> = None;
    let mut content_type = "both".to_string();

    for option in &command.data.options {
        match (&option.name[..], &option.value) {
            ("channel", CommandDataOptionValue::Channel(id)) => channel_id = Some(id.get()),
            ("content_type", CommandDataOptionValue::String(value)) => {
                content_type = value.to_string()
            }
            _ => {}
        }
    }

    let Some(channel_id) = channel_id else {
        command
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content("Missing channel option"),
            )
            .await?;
        return Ok(());
    };

    let guild_id = command
        .guild_id
        .map(|g| g.get().to_string())
        .unwrap_or_else(|| "0".to_string());

    if let Err(err) = db::upsert_channel_config(
        &state.db,
        &channel_id.to_string(),
        &guild_id,
        &content_type,
    )
    .await
    {
        command
            .edit_response(
                &ctx.http,
                EditInteractionResponse::new().content(format!(
                    "Failed to save channel configuration: {}",
                    err
                )),
            )
            .await?;
        return Ok(());
    }

    let msg = format!(
        "Configured channel <#{}> with content type {}",
        channel_id, content_type
    );

    command
        .edit_response(
            &ctx.http,
            EditInteractionResponse::new().content(msg),
        )
        .await?;

    Ok(())
}

async fn handle_checknews(
    ctx: &Context,
    state: &Arc<AppState>,
    command: &serenity::all::CommandInteraction,
) -> Result<()> {
    command
        .create_response(
            &ctx.http,
            CreateInteractionResponse::Defer(CreateInteractionResponseMessage::new()),
        )
        .await?;

    match fetch_latest_website_article(&state.http_client).await {
        Ok(Some(article)) => {
            let mut embed = CreateEmbed::new()
                .title(truncate(&article.title, 256))
                .url(article.url)
                .description(truncate(&article.description, 4096))
                .color(if article.is_galnet { 0xF0E000 } else { 0x3498DB });

            if let Some(image_url) = article.image_url {
                embed = embed.image(image_url);
            }

            if let Some(published) = article.published_at {
                embed = embed.field("Published", published, false);
            }

            embed = embed.field(
                "Section",
                if article.is_galnet { "Galnet" } else { "General News" },
                true,
            );

            command
                .edit_response(&ctx.http, EditInteractionResponse::new().embed(embed))
                .await?;
        }
        Ok(None) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(
                        "Could not find a latest article from the Elite Dangerous website right now.",
                    ),
                )
                .await?;
        }
        Err(err) => {
            command
                .edit_response(
                    &ctx.http,
                    EditInteractionResponse::new().content(format!(
                        "Failed to fetch latest website article: {}",
                        err
                    )),
                )
                .await?;
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct WebsiteArticle {
    title: String,
    description: String,
    url: String,
    image_url: Option<String>,
    published_at: Option<String>,
    is_galnet: bool,
}

const BROWSER_USER_AGENT: &str =
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36";
const DEFAULT_NEWS_IMAGE: &str = "https://www.elitedangerous.com/images/og_image.jpg";

async fn fetch_latest_website_article(client: &reqwest::Client) -> Result<Option<WebsiteArticle>> {
    let html = client
        .get("https://www.elitedangerous.com/news/rss")
        .header(USER_AGENT, BROWSER_USER_AGENT)
        .header(ACCEPT, "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
        .header(ACCEPT_LANGUAGE, "en-GB,en;q=0.9")
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    let articles = extract_articles_from_payload(&html)?;

    if let Some(article) = articles.into_iter().next() {
        return Ok(Some(article));
    }

    Ok(None)
}

fn extract_articles_from_payload(html: &str) -> Result<Vec<WebsiteArticle>> {
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

        // Wider look-back captures body/image that precedes the slug field.
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

        let summary = extract_js_string(context, "summary").filter(|s| s.len() > 5);
        let image_url = extract_zaonce_image(context);
        let title = extract_js_string(context, "title")
            .filter(|t| t.len() > 2 && !t.eq_ignore_ascii_case("news") && !t.eq_ignore_ascii_case("galnet"))
            .unwrap_or_else(|| title_from_news_path(&format!("/news/{}", slug)));

        out.push(WebsiteArticle {
            title,
            description: summary.unwrap_or_else(|| "Open the article link to view full details.".to_string()),
            url: format!("https://www.elitedangerous.com/news/{}", slug),
            image_url,
            published_at: None,
            is_galnet: false,
        });
    }

    if out.is_empty() {
        // Fallback: path-pattern extraction.
        let path_re = Regex::new(r"\\u002Fnews\\u002F(?:galnet\\u002F)?[a-z0-9\\-]+")?;
        for m in path_re.find_iter(html) {
            let path = m
                .as_str()
                .replace("\\u002F", "/")
                .trim_end_matches('\\')
                .to_string();
            if path == "/news/rss" || path == "/news/galnet/rss" {
                continue;
            }
            if seen.insert(path.clone()) {
                out.push(WebsiteArticle {
                    title: title_from_news_path(&path),
                    description: "Open the article link to view full details.".to_string(),
                    url: format!("https://www.elitedangerous.com{}", path),
                    image_url: Some(DEFAULT_NEWS_IMAGE.to_string()),
                    published_at: None,
                    is_galnet: path.contains("/galnet/"),
                });
            }
        }
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

fn title_from_news_path(path: &str) -> String {
    let slug = path
        .trim_start_matches("/news/")
        .trim_start_matches("galnet/")
        .trim_matches('/');

    if slug.is_empty() {
        return "Elite Dangerous News".to_string();
    }

    slug.split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut out = first.to_ascii_uppercase().to_string();
                    out.push_str(chars.as_str());
                    out
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

async fn run_polling_loop(ctx: Context, state: Arc<AppState>) {
    info!(
        "Starting polling loop every {} minutes",
        (state.config.poll_interval_ms as f64) / 60000.0
    );

    loop {
        let result = poll_once(&ctx, &state).await;
        if let Some(err) = result.error {
            error!("Polling cycle failed: {err}");
        }

        tokio::time::sleep(std::time::Duration::from_millis(state.config.poll_interval_ms)).await;
    }
}

pub async fn poll_once(ctx: &Context, state: &Arc<AppState>) -> PollResult {
    let started = Instant::now();
    let mut result = PollResult::default();

    let fetched = elite_api::fetch_all_articles(&state.http_client).await;
    let (galnet, news) = match fetched {
        Ok(v) => v,
        Err(err) => {
            result.error = Some(format!("fetch error: {err:#}"));
            result.duration_ms = started.elapsed().as_millis();
            return result;
        }
    };

    result.fetched = galnet.len() + news.len();

    let transformed = processor::transform_articles(galnet, news);
    let mut inserted = Vec::new();

    for article in transformed {
        match db::article_exists(&state.db, &article.content_hash).await {
            Ok(true) => continue,
            Ok(false) => {}
            Err(err) => {
                warn!("Failed dedupe check: {err:#}");
                continue;
            }
        }

        match db::insert_article(&state.db, &article).await {
            Ok(Some(row)) => inserted.push(row),
            Ok(None) => {}
            Err(err) => warn!("Insert failed: {err:#}"),
        }
    }

    result.new_articles = inserted.len();
    result.inserted = inserted.len();

    let channels = match db::active_channels(&state.db).await {
        Ok(c) => c,
        Err(err) => {
            result.error = Some(format!("channel query error: {err:#}"));
            result.duration_ms = started.elapsed().as_millis();
            return result;
        }
    };

    for channel in channels {
        for article in &inserted {
            if channel.content_types != "both" && channel.content_types != article.api_source {
                continue;
            }

            let channel_id_num = match channel.channel_id.parse::<u64>() {
                Ok(v) => v,
                Err(_) => {
                    result.failed += 1;
                    continue;
                }
            };

            let embed = build_embed(article);
            let send_result = ChannelId::new(channel_id_num)
                .send_message(&ctx.http, CreateMessage::new().embed(embed))
                .await;

            match send_result {
                Ok(message) => {
                    if let Err(err) = db::record_published(
                        &state.db,
                        article.id,
                        &channel.channel_id,
                        &message.id.get().to_string(),
                    )
                    .await
                    {
                        warn!("Failed to record published row: {err:#}");
                    }
                    result.sent += 1;
                }
                Err(err) => {
                    warn!("Failed send to channel {}: {}", channel.channel_id, err);
                    result.failed += 1;
                }
            }
        }
    }

    result.duration_ms = started.elapsed().as_millis();
    result
}

fn build_embed(article: &crate::types::StoredArticle) -> CreateEmbed {
    let source_color = if article.api_source == "galnet" {
        0xF0E000
    } else {
        0x3498DB
    };

    let mut embed = CreateEmbed::new()
        .title(truncate(&article.title, 256))
        .description(truncate(&article.summary, 4096))
        .color(source_color);

    if let Some(image_url) = &article.image_url {
        embed = embed.thumbnail(image_url);
    }

    embed
}

fn truncate(input: &str, max: usize) -> String {
    if input.chars().count() <= max {
        return input.to_string();
    }

    input.chars().take(max.saturating_sub(3)).collect::<String>() + "..."
}
