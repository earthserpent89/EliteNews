CREATE TABLE IF NOT EXISTS article (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    external_id TEXT NOT NULL,
    title TEXT NOT NULL,
    summary TEXT,
    body TEXT,
    image_url TEXT,
    content_hash TEXT NOT NULL UNIQUE,
    api_source TEXT NOT NULL CHECK (api_source IN ('galnet', 'news')),
    published_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS published_article (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    article_id INTEGER NOT NULL,
    channel_id TEXT NOT NULL,
    message_id TEXT,
    sent_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (article_id) REFERENCES article(id) ON DELETE CASCADE,
    UNIQUE (article_id, channel_id)
);

CREATE TABLE IF NOT EXISTS channel_config (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id TEXT NOT NULL UNIQUE,
    guild_id TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 1,
    content_types TEXT NOT NULL DEFAULT 'both' CHECK (content_types IN ('galnet', 'news', 'both')),
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
