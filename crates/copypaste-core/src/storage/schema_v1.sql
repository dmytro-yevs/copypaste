CREATE TABLE IF NOT EXISTS clipboard_items (
    id              TEXT PRIMARY KEY NOT NULL,
    item_id         TEXT NOT NULL,
    content_type    TEXT NOT NULL,
    content         BLOB,
    content_nonce   BLOB,
    blob_ref        TEXT,
    is_sensitive    INTEGER NOT NULL DEFAULT 0,
    is_synced       INTEGER NOT NULL DEFAULT 0,
    lamport_ts      INTEGER NOT NULL,
    wall_time       INTEGER NOT NULL,
    expires_at      INTEGER,
    app_bundle_id   TEXT
);

CREATE INDEX IF NOT EXISTS idx_clipboard_wall_time ON clipboard_items(wall_time DESC);
CREATE INDEX IF NOT EXISTS idx_clipboard_expires ON clipboard_items(expires_at) WHERE expires_at IS NOT NULL;

CREATE VIRTUAL TABLE IF NOT EXISTS clipboard_fts
    USING fts5(id UNINDEXED, content_text);

CREATE TABLE IF NOT EXISTS devices (
    id              TEXT PRIMARY KEY NOT NULL,
    name            TEXT NOT NULL,
    platform        TEXT NOT NULL,
    public_key      TEXT NOT NULL,
    fingerprint     TEXT NOT NULL,
    verified        INTEGER NOT NULL DEFAULT 0,
    last_seen       INTEGER
);

CREATE TABLE IF NOT EXISTS settings (
    key             TEXT PRIMARY KEY NOT NULL,
    value           TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pending_uploads (
    item_id         TEXT PRIMARY KEY NOT NULL,
    tus_url         TEXT NOT NULL,
    bytes_uploaded  INTEGER NOT NULL DEFAULT 0,
    total_bytes     INTEGER NOT NULL,
    chunk_format_version INTEGER NOT NULL DEFAULT 1,
    created_at      INTEGER NOT NULL,
    expires_at      INTEGER NOT NULL
);
