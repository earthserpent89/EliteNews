mod bot;
mod config;
mod db;
mod elite_api;
mod processor;
mod types;

use std::sync::{atomic::AtomicBool, Arc};

use anyhow::Result;
use serenity::Client;
use tracing::info;
use tracing_subscriber::EnvFilter;

use crate::bot::{intents, AppState, Handler};
use crate::config::Config;

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_logging();

    let config = Config::from_env()?;
    info!(
        "Starting Rust Elite News bot for application {}",
        config.discord_client_id
    );

    let db = db::connect_and_migrate(&config.database_path).await?;
    let http_client = reqwest::Client::builder().build()?;

    let state = Arc::new(AppState {
        config: config.clone(),
        db,
        http_client,
        poller_started: AtomicBool::new(false),
    });

    let mut client = Client::builder(config.discord_token.clone(), intents())
        .event_handler(Handler::new(state))
        .await?;

    client.start().await?;
    Ok(())
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .compact()
        .init();
}
