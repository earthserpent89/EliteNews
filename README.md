# EliteNews — Elite Dangerous Discord News Bot

A cross-platform Discord bot written in Rust that automatically polls the [Elite Dangerous website](https://www.elitedangerous.com/news) for new articles and posts them as rich embeds to configured Discord channels. Supports both general news and Galnet articles with per-channel content filtering, deduplication, and slash command controls.

---

## Features

- **Automatic polling** — checks for new articles on a configurable interval (default: 60 minutes)
- **Rich Discord embeds** — posts article title, summary, header image, and direct link
- **Dual content streams** — tracks general News and Galnet separately; channels can subscribe to either or both
- **Deduplication** — content-hashes every article so the same story is never posted twice, even if the upstream page changes
- **Publish history** — records every message sent per channel to avoid re-posting on restart
- **Slash commands** — `/setnewschannel` and `/checknews` with proper defer/edit interaction flow
- **Guild-scoped registration** — commands register instantly to a specific guild (no 1-hour global propagation delay); falls back to global if no guild is set
- **Stale command cleanup** — automatically clears leftover global commands after successful guild registration to prevent duplicates
- **SQLite persistence** — zero-dependency local database, created automatically on first run
- **Cross-platform** — runs natively on Windows, macOS, and Linux with no native module issues

---

## Project Structure

```
EliteNews/
├── rust-bot/
│   ├── src/
│   │   ├── main.rs          # Entry point, tokio runtime, serenity client setup
│   │   ├── bot.rs           # Event handler, slash commands, polling loop, embed builder
│   │   ├── elite_api.rs     # Fetches and parses articles from elitedangerous.com/news/rss
│   │   ├── processor.rs     # Transforms raw API data into normalised NewArticle structs
│   │   ├── db.rs            # SQLite helpers (connect, migrate, insert, query)
│   │   ├── config.rs        # Environment variable loading and validation
│   │   └── types.rs         # Shared data types
│   ├── sql/
│   │   └── schema.sql       # SQLite table definitions (auto-applied on startup)
│   ├── Cargo.toml
│   └── .env.example
├── README.md
├── SETUP.md
└── VERIFICATION.md
```

---

## Prerequisites

| Requirement | Version | Notes |
|---|---|---|
| Rust toolchain | stable (1.75+) | Install via [rustup.rs](https://rustup.rs) |
| Discord bot token | — | See [Bot Setup](#discord-bot-setup) |

No Node.js, npm, or native modules required.

---

## Discord Bot Setup

1. Go to the [Discord Developer Portal](https://discord.com/developers/applications) and create a new application.
2. Under **Bot**, create a bot user and copy the **Token**.
3. Under **OAuth2 → URL Generator**, select scopes:
   - `bot`
   - `applications.commands`
4. Under **Bot Permissions**, select:
   - `Send Messages`
   - `Embed Links`
   - `View Channels`
5. Use the generated URL to invite the bot to your server.
6. Copy the **Application ID** (shown on the General Information page) — this is your `DISCORD_CLIENT_ID`.
7. Enable **Developer Mode** in Discord (User Settings → Advanced) to right-click and copy your server ID for `DISCORD_GUILD_ID`.

---

## Installation

```bash
# Clone the repository
git clone https://github.com/your-username/EliteNews.git
cd EliteNews/rust-bot

# Copy the environment template
# Windows (PowerShell)
Copy-Item .env.example .env

# macOS / Linux
cp .env.example .env
```

Edit `.env` and fill in your values (see [Configuration](#configuration) below).

```bash
# Build and run
cargo run
```

On first run, the SQLite database file is created automatically at the path specified by `DATABASE_PATH`.

---

## Configuration

All configuration is done via environment variables in `rust-bot/.env`.

| Variable | Required | Default | Description |
|---|---|---|---|
| `DISCORD_TOKEN` | ✅ | — | Bot token from the Discord Developer Portal |
| `DISCORD_CLIENT_ID` | ✅ | — | Application ID from the Developer Portal |
| `DISCORD_GUILD_ID` | ❌ | — | Guild (server) ID for instant command registration. Omit for global registration (up to 1 hour propagation). |
| `POLL_INTERVAL` | ❌ | `3600000` | Polling interval in **milliseconds**. Default is 60 minutes. |
| `DATABASE_PATH` | ❌ | `./database.sqlite` | Path to the SQLite database file. |

**Example `.env`:**

```env
DISCORD_TOKEN=your_bot_token_here
DISCORD_CLIENT_ID=123456789012345678
DISCORD_GUILD_ID=987654321098765432
POLL_INTERVAL=3600000
DATABASE_PATH=./database.sqlite
```

---

## Slash Commands

### `/setnewschannel`

Configures a channel to receive automatic news posts.

**Required permission:** Manage Server

| Option | Type | Required | Description |
|---|---|---|---|
| `channel` | Channel | ✅ | The text channel to post articles to |
| `content_type` | Choice | ❌ | `Galnet Only`, `News Only`, or `Both` (default: `Both`) |

You can run this command again at any time to change the channel or content type. The old configuration is overwritten automatically.

---

### `/checknews`

Manually fetches the latest article from the Elite Dangerous website and displays it as an embed, without triggering the full polling cycle or sending to configured channels.

**Required permission:** Manage Server

Useful for verifying the bot can reach the website and testing embed formatting.

---

## How It Works

### Article Fetching

The bot fetches `https://www.elitedangerous.com/news/rss`, which returns a Nuxt SSR page containing a full inline JavaScript payload (`window.__NUXT__`). Individual article pages are blocked by the site's WAF for non-browser clients, so all article data — title, summary, images, and slug — is extracted directly from the embedded JS payload using regex.

Key extraction logic:
- Articles are identified by `"field-slug":"slug-name"` markers in the payload
- A 3 000-character look-back window around each slug captures the article body
- Summary text is extracted from `summary:"..."` JS object literals
- Header images are extracted from `cms-cdn.zaonce.net` URLs (forward slashes are unicode-escaped as `\u002F` in the payload and decoded on extraction)
- Article ordering preserves the server's natural order, so the newest article is always processed first

### Polling Loop

1. Fetch the `/news/rss` payload
2. Extract all article candidates in order
3. For each article, compute a SHA-256 content hash of the title + body
4. Check the database — skip if the hash already exists (deduplication)
5. Insert new articles into the `article` table
6. For each configured active channel, send an embed and record the sent message ID in `published_article`

### Database Schema

```sql
-- Stores every article ever seen
article (id, external_id, title, summary, body, image_url, content_hash, api_source, published_at, created_at)

-- Records each Discord message sent per article per channel
published_article (id, article_id, channel_id, message_id, sent_at)

-- Channel subscription configuration
channel_config (channel_id, guild_id, is_active, content_types, created_at, updated_at)
```

---

## Troubleshooting

### Commands not appearing in Discord

- Ensure `DISCORD_CLIENT_ID` and `DISCORD_GUILD_ID` are set correctly in `.env`
- Make sure the bot was invited with the `applications.commands` OAuth2 scope
- If you see duplicate commands, set `DISCORD_GUILD_ID` — the bot automatically clears stale global commands after a successful guild registration

### Bot posts to a channel but shows "Missing Access"

The bot lacks permission to view or send messages in the target channel. Fix channel permissions in Discord server settings to grant the bot **View Channel**, **Send Messages**, and **Embed Links**.

### `/checknews` returns only a link (no image or description)

The site WAF blocks direct article page fetches. If this occurs, the Nuxt payload structure at `/news/rss` may have changed. Check bot logs for extraction warnings.

### The bot crashes immediately on startup

- Confirm `DISCORD_TOKEN` is valid and not expired
- Confirm the SQLite database path is writable
- Run with `RUST_LOG=debug cargo run` for verbose output

### Exit code `0xffffffff` on Windows

This is normal Windows process termination — not a crash. It appears when the process is killed externally (e.g. closing the terminal or `Stop-Process`).

---

## Development

```bash
# Check for compile errors without producing a binary
cargo check

# Run with debug logging
$env:RUST_LOG="debug"; cargo run        # PowerShell
RUST_LOG=debug cargo run                # bash/zsh

# Build an optimised release binary
cargo build --release
# Binary: rust-bot/target/release/elite-news-rust-bot(.exe)
```

---

## Dependencies

| Crate | Purpose |
|---|---|
| `serenity` | Discord API client and slash command framework |
| `sqlx` | Async SQLite with compile-time query checking |
| `reqwest` | HTTP client for fetching the Elite Dangerous website |
| `regex` | Nuxt payload parsing and article field extraction |
| `tokio` | Async runtime |
| `tracing` / `tracing-subscriber` | Structured logging |
| `anyhow` | Ergonomic error propagation |
| `chrono` | Timestamp handling |
| `sha2` | SHA-256 content hashing for deduplication |
| `dotenvy` | `.env` file loading |
| `serde` / `serde_json` | JSON deserialization |

---

## License

MIT
