use std::env;

use anyhow::{Context, Result};

#[derive(Clone, Debug)]
pub struct Config {
    pub discord_token: String,
    pub discord_client_id: u64,
    pub discord_guild_id: Option<u64>,
    pub poll_interval_ms: u64,
    pub database_path: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let discord_token = clean_env(env::var("DISCORD_TOKEN").context("DISCORD_TOKEN is required")?);
        let discord_client_id = env::var("DISCORD_CLIENT_ID")
            .context("DISCORD_CLIENT_ID is required")?
            .pipe(clean_env)
            .parse::<u64>()
            .context("DISCORD_CLIENT_ID must be a valid u64")?;

        let discord_guild_id = match env::var("DISCORD_GUILD_ID") {
            Ok(v) if !v.trim().is_empty() => Some(
                clean_env(v)
                    .parse::<u64>()
                    .context("DISCORD_GUILD_ID must be a valid u64")?,
            ),
            _ => None,
        };

        let poll_interval_ms = env::var("POLL_INTERVAL")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3_600_000);

        let database_path = clean_env(
            env::var("DATABASE_PATH").unwrap_or_else(|_| "./database.sqlite".to_string()),
        );

        Ok(Self {
            discord_token,
            discord_client_id,
            discord_guild_id,
            poll_interval_ms,
            database_path,
        })
    }
}

fn clean_env(value: String) -> String {
    value.trim().trim_matches('"').trim_matches('\'').to_string()
}

trait Pipe: Sized {
    fn pipe<R>(self, f: impl FnOnce(Self) -> R) -> R {
        f(self)
    }
}

impl<T> Pipe for T {}
