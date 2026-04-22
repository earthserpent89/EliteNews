# Elite News Rust Bot (Cross-Platform Refactor)

This is a Rust rewrite path for the Elite News Discord bot focused on stronger cross-platform reliability.

## Implemented in this version

- Discord client startup with slash command registration
- SQLite schema creation on startup
- Elite API polling (Galnet + News)
- Content hashing and deduplication
- Article insertion and publish history tracking
- Channel configuration command: /setnewschannel
- Manual poll trigger command: /checknews
- Background polling loop

## Environment

Copy .env.example to .env and set values:

- DISCORD_TOKEN
- DISCORD_CLIENT_ID
- DISCORD_GUILD_ID (optional for guild-scoped command registration)
- POLL_INTERVAL
- DATABASE_PATH

## Run

1. Install Rust toolchain (stable)
2. From this folder: cargo run

## Notes

- This refactor currently ships the core polling and channel configuration flow.
- searchnews and articlehistory parity can be added next using the same DB schema.
- The Node implementation remains in the root src folder during migration.
