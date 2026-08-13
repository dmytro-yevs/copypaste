//! The one v2 schema, created in one transaction on a newly reserved file.

use rusqlite::Connection;

use super::model::StoreError;

pub(super) const SCHEMA: &str = r#"
CREATE TABLE clipboard_items (
    id                 TEXT    PRIMARY KEY NOT NULL,
    -- NULL only on a tombstone: soft delete wipes the payload.
    content_ciphertext BLOB,
    nonce              BLOB,
    content_type       TEXT    NOT NULL,
    -- SHA-256 hex of the pre-encryption bytes. Kept on tombstones on purpose.
    content_hash       TEXT    NOT NULL DEFAULT '',
    is_sensitive       INTEGER NOT NULL DEFAULT 0,
    pinned             INTEGER NOT NULL DEFAULT 0,
    -- REAL so a reorder can insert between two neighbours without renumbering.
    pin_order          REAL,
    -- A separate version for P2P-only pin state. Pinning must not restamp the
    -- content version, because cloud does not carry pin metadata.
    pin_updated_at     INTEGER NOT NULL DEFAULT 0,
    -- Milliseconds since the Unix epoch. Every timestamp in this schema is ms.
    created_at         INTEGER NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0,
    -- Which device first captured this version. Merge key 4, and the reason it
    -- is a column rather than a side table: the sync view used to shadow it in
    -- `sync_item_origin` on a second connection, which is two answers to "what
    -- is in this device's history". Empty means "captured here" — a reader
    -- substitutes its own device id (`storage::versions::origin_or`), so a
    -- local capture costs no extra write.
    origin_device_id   TEXT    NOT NULL DEFAULT '',
    app_bundle_id      TEXT,
    app_name           TEXT,
    payload_metadata   TEXT,
    -- `clipboard_fts.rowid` of this row's index entry, NULL when it has none.
    -- FTS5 seeks on nothing but its rowid: `WHERE id = ?` against the
    -- UNINDEXED column is filtered after a full scan of the plaintext index.
    fts_rowid          INTEGER
);

-- Serves the list query verbatim: pinned first, then pin order, then newest
-- first, with an id tiebreak that makes the order total.
CREATE INDEX idx_items_history
    ON clipboard_items(pinned DESC, pin_order, created_at DESC, id DESC)
    WHERE deleted = 0;

-- Serves find_recent_by_hash.
CREATE INDEX idx_items_hash
    ON clipboard_items(content_hash, created_at DESC)
    WHERE deleted = 0;

-- TOCTOU backstop for dedup: the application probe is a SELECT-before-INSERT,
-- so two local ingest events with identical content can both observe "no recent
-- row". This makes the second INSERT fail; insert() then re-reads the winner
-- inside the same transaction and returns it, which is what makes dedup
-- idempotent. Origin keeps simultaneous copies on different devices as their
-- own sync identities. Excludes empty hashes so hash-less rows do not all
-- collide, and excludes tombstones so a re-copy after a delete is a fresh live
-- row.
CREATE UNIQUE INDEX idx_items_dedup
    ON clipboard_items(content_hash, created_at / 60000, origin_device_id)
    WHERE deleted = 0 AND content_hash <> '';

-- Serves the eviction scans (oldest unpinned live rows first) and the unpinned
-- half of the history page, whose seek predicate is this partial one exactly.
CREATE INDEX idx_items_evictable
    ON clipboard_items(created_at, id)
    WHERE deleted = 0 AND pinned = 0;

-- Serves the sensitive TTL sweep and the probe that keeps it off the write
-- path. Empty on a machine that has never copied a secret, which is what makes
-- that probe free.
CREATE INDEX idx_items_sensitive_wipe
    ON clipboard_items(created_at, id)
    WHERE is_sensitive = 1 AND pinned = 0 AND deleted = 0;

-- The byte quota's hot gate reads only this expression and its partial
-- predicate. Keeping ciphertext out of the table scan avoids touching every
-- encrypted payload on each accepted capture.
CREATE INDEX idx_items_unpinned_bytes
    ON clipboard_items(LENGTH(COALESCE(content_ciphertext, X'')))
    WHERE deleted = 0 AND pinned = 0;

-- Serves the sync read, covering. The partial predicate must stay written
-- exactly as the query writes it or SQLite silently declines the index
-- (`CopyPaste-crh3.3`); here that predicate is also what keeps a live sensitive
-- item out of an advertisement, so a drift is a disclosure, not a slow query.
CREATE INDEX idx_items_syncable
    ON clipboard_items(created_at, id, content_hash, deleted, origin_device_id,
                       pinned, pin_order, pin_updated_at, is_sensitive)
    WHERE deleted = 1 OR is_sensitive = 0;

-- Serves the incremental variant. Pin state moves on `pin_updated_at`, not on
-- `created_at`, so a cursor over `created_at` alone silently stops propagating
-- pin and unpin to old items (manifest 05 §3.6).
CREATE INDEX idx_items_sync_cursor
    ON clipboard_items(MAX(created_at, pin_updated_at), id)
    WHERE deleted = 1 OR is_sensitive = 0;

-- SQLite has no constant-time row count. This singleton is derived state;
-- triggers maintain it in the item transaction.
CREATE TABLE clipboard_live_count (
    only_row INTEGER PRIMARY KEY NOT NULL CHECK (only_row = 0),
    live     INTEGER NOT NULL
);
INSERT INTO clipboard_live_count (only_row, live) VALUES (0, 0);

CREATE TRIGGER clipboard_live_count_insert AFTER INSERT ON clipboard_items
WHEN NEW.deleted = 0
BEGIN
    UPDATE clipboard_live_count SET live = live + 1;
END;

CREATE TRIGGER clipboard_live_count_delete AFTER DELETE ON clipboard_items
WHEN OLD.deleted = 0
BEGIN
    UPDATE clipboard_live_count SET live = live - 1;
END;

CREATE TRIGGER clipboard_live_count_update AFTER UPDATE OF deleted ON clipboard_items
WHEN OLD.deleted <> NEW.deleted
BEGIN
    UPDATE clipboard_live_count
       SET live = live + CASE WHEN NEW.deleted = 0 THEN 1 ELSE -1 END;
END;

-- External-content mode is NOT used: there is no cascade from clipboard_items,
-- so every delete path must remove the FTS row explicitly, in the same
-- transaction as the row change. Rows are keyed by `rowid`, mirrored in
-- `clipboard_items.fts_rowid`, because that is the only key FTS5 can seek on.
CREATE VIRTUAL TABLE clipboard_fts USING fts5(id UNINDEXED, content_text);

-- This device's identity and every cursor, token and setting either transport
-- keeps. Inside the encrypted database rather than a file beside it: a refresh
-- token or a sync key in plaintext next to an encrypted history would be the
-- weakest link. `server::dbadmin` deliberately does not restore it — a restore
-- brings back history, not another device's identity.
CREATE TABLE sync_device_state (
    key   TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

-- What each device id calls itself, so `origin_device_id` can be shown to a
-- user as a name rather than a UUID. Cosmetic and untrusted: it is whatever a
-- peer said in its hello, and nothing keys off it.
CREATE TABLE sync_device_name (
    device_id TEXT PRIMARY KEY NOT NULL,
    name      TEXT NOT NULL
);
"#;

pub(super) fn create(conn: &mut Connection) -> Result<(), StoreError> {
    let tx = conn.transaction()?;
    tx.execute_batch(SCHEMA)?;
    tx.commit()?;
    super::schema_verify::verify_schema(conn)
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{KEY, OTHER_KEY};
    use super::super::{Store, StoreError};

    #[test]
    fn fresh_open_creates_the_canonical_schema_and_reopens_it() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");

        let store = Store::open(&path, &KEY).unwrap();
        super::super::schema_verify::verify_schema(&store.conn().unwrap()).unwrap();
        drop(store);
        Store::open(&path, &KEY).unwrap();
    }

    #[test]
    fn an_incompatible_database_is_refused_without_repairing_it() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        store
            .conn()
            .unwrap()
            .execute_batch("DROP INDEX idx_items_sync_cursor")
            .unwrap();
        drop(store);

        let error = Store::open(&path, &KEY).unwrap_err();
        assert!(matches!(error, StoreError::InvalidSchema));
        assert!(!error.to_string().contains(&*path.to_string_lossy()));

        let conn = rusqlite::Connection::open(&path).unwrap();
        super::super::connection::apply_key(&conn, &KEY).unwrap();
        super::super::connection::validate_key(&conn).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_schema WHERE name = 'idx_items_sync_cursor'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "open must not repair an incompatible schema");
    }

    #[test]
    fn wrong_key_still_fails_closed() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        drop(Store::open(&path, &KEY).unwrap());

        assert!(matches!(
            Store::open(&path, &OTHER_KEY),
            Err(StoreError::InvalidKey)
        ));
    }

    #[test]
    fn source_has_one_schema_and_no_compatibility_ladder() {
        let production = include_str!("schema.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        for forbidden in [
            concat!("rusqlite", "_migration"),
            concat!("Migration", "s"),
            concat!("M::", "up"),
            concat!("up_with_", "hook"),
            concat!("repair", "_early_v2_schema"),
            concat!("pragma_", "table_info"),
            concat!("user_", "version"),
            "ALTER TABLE",
        ] {
            assert!(!production.contains(forbidden), "forbidden: {forbidden}");
        }
        assert_eq!(production.matches("pub(super) const SCHEMA").count(), 1);
        assert_eq!(production.matches("tx.execute_batch(SCHEMA)").count(), 1);

        let manifests = concat!(
            include_str!("../../Cargo.toml"),
            include_str!("../../../../Cargo.toml")
        );
        assert!(!manifests.contains(concat!("rusqlite", "_migration")));
    }
}
