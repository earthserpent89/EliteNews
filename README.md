# Elite News Bot (Rust)

This repository now uses the Rust implementation located in `rust-bot/`.

## Quick Start

1. `cd rust-bot`
2. Copy env template:
   - PowerShell: `Copy-Item .env.example .env`
3. Edit `.env` and set:
   - `DISCORD_TOKEN`
   - `DISCORD_CLIENT_ID`
   - `DISCORD_GUILD_ID` (optional)
4. Run:
   - `cargo run`

## Notes

- The Rust bot is cross-platform and avoids Node native module issues.
- If slash command registration fails with **Missing Access**, re-invite the bot with `applications.commands` scope and ensure `DISCORD_GUILD_ID` points to a guild where the bot is present.
- Full implementation and source code are in `rust-bot/src`.
