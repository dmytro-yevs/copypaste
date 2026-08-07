//! The one v2 schema, created in one transaction. `rusqlite_migration` owns
//! the version marker and refuses a database written by a newer build.

use std::sync::LazyLock;

use rusqlite::{Connection, OptionalExtension, Transaction};
use rusqlite_migration::{HookResult, Migrations, M};

use super::model::StoreError;

/// Schema v1 — the whole schema, created in one step on a fresh database.
///
/// No `IF NOT EXISTS` guards and no `pragma_table_info` probes: the version
/// marker is `rusqlite_migration`'s business now, and every statement here runs
/// exactly once, inside its transaction.
const SCHEMA_V1: &str = r#"
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

-- Serves the eviction scans (oldest unpinned live rows first).
CREATE INDEX idx_items_evictable
    ON clipboard_items(created_at, id)
    WHERE deleted = 0 AND pinned = 0;

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

-- The history cap is tested on every capture, so the test may not be a scan.
-- SQLite has no O(1) row count; this row is one. The triggers below are the
-- only thing that maintains it, and `seed_live_count` recomputes it from the
-- table on every open so a drift cannot outlive a restart.
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

-- Liveness is this column and nothing else, so `UPDATE OF deleted` is the whole
-- surface: a soft delete, and the resurrect a merge performs when a peer's live
-- version outranks a local tombstone.
CREATE TRIGGER clipboard_live_count_update AFTER UPDATE OF deleted ON clipboard_items
WHEN OLD.deleted <> NEW.deleted
BEGIN
    UPDATE clipboard_live_count
       SET live = live + (CASE WHEN NEW.deleted = 0 THEN 1 ELSE -1 END);
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

/// The same objects as the tail of [`SCHEMA_V1`], written idempotently for a v2
/// history that predates them.
const LIVE_COUNT_DDL: &str = r#"
CREATE TABLE IF NOT EXISTS clipboard_live_count (
    only_row INTEGER PRIMARY KEY NOT NULL CHECK (only_row = 0),
    live     INTEGER NOT NULL
);
INSERT OR IGNORE INTO clipboard_live_count (only_row, live) VALUES (0, 0);

CREATE TRIGGER IF NOT EXISTS clipboard_live_count_insert AFTER INSERT ON clipboard_items
WHEN NEW.deleted = 0
BEGIN
    UPDATE clipboard_live_count SET live = live + 1;
END;

CREATE TRIGGER IF NOT EXISTS clipboard_live_count_delete AFTER DELETE ON clipboard_items
WHEN OLD.deleted = 0
BEGIN
    UPDATE clipboard_live_count SET live = live - 1;
END;

CREATE TRIGGER IF NOT EXISTS clipboard_live_count_update AFTER UPDATE OF deleted ON clipboard_items
WHEN OLD.deleted <> NEW.deleted
BEGIN
    UPDATE clipboard_live_count
       SET live = live + (CASE WHEN NEW.deleted = 0 THEN 1 ELSE -1 END);
END;
"#;

/// One past the last migration in [`MIGRATIONS`]. `super::schema_verify` reads
/// it to refuse a file this ladder has not produced.
pub(super) const SCHEMA_VERSION: i64 = 7;

static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(SCHEMA_V1),
        M::up_with_hook("", repair_early_v2_schema),
        M::up_with_hook("", repair_early_v2_schema),
        M::up_with_hook("", repair_early_v2_schema),
        M::up_with_hook("", repair_dedup_index),
        M::up_with_hook("", add_fts_rowid_and_sync_indexes),
        M::up_with_hook("", add_live_count),
    ])
});

/// Idempotent for the same reason [`add_fts_rowid_and_sync_indexes`] is: a
/// fresh database already carries all of this from [`SCHEMA_V1`].
fn add_live_count(tx: &Transaction<'_>) -> HookResult {
    tx.execute_batch(LIVE_COUNT_DDL)?;
    seed_live_count(tx)?;
    Ok(())
}

fn repair_early_v2_schema(tx: &Transaction<'_>) -> HookResult {
    for (name, statement) in [
        (
            "origin_device_id",
            "ALTER TABLE clipboard_items ADD COLUMN origin_device_id TEXT NOT NULL DEFAULT '';",
        ),
        (
            "app_bundle_id",
            "ALTER TABLE clipboard_items ADD COLUMN app_bundle_id TEXT;",
        ),
        (
            "app_name",
            "ALTER TABLE clipboard_items ADD COLUMN app_name TEXT;",
        ),
        (
            "payload_metadata",
            "ALTER TABLE clipboard_items ADD COLUMN payload_metadata TEXT;",
        ),
        (
            "pin_updated_at",
            "ALTER TABLE clipboard_items ADD COLUMN pin_updated_at INTEGER NOT NULL DEFAULT 0;",
        ),
    ] {
        if !has_column(tx, name)? {
            tx.execute_batch(statement)?;
        }
    }
    tx.execute_batch(
        "CREATE TABLE IF NOT EXISTS sync_device_state (
             key TEXT PRIMARY KEY NOT NULL,
             value TEXT NOT NULL
         );
         CREATE TABLE IF NOT EXISTS sync_device_name (
             device_id TEXT PRIMARY KEY NOT NULL,
             name TEXT NOT NULL
         );",
    )?;
    Ok(())
}

fn repair_dedup_index(tx: &Transaction<'_>) -> HookResult {
    tx.execute_batch(
        "DROP INDEX IF EXISTS idx_items_dedup;
         CREATE UNIQUE INDEX idx_items_dedup
             ON clipboard_items(content_hash, created_at / 60000, origin_device_id)
             WHERE deleted = 0 AND content_hash <> '';",
    )?;
    Ok(())
}

/// Idempotent like the repair hooks above, because [`SCHEMA_V1`] already
/// carries all of this on a fresh database and only a v2 history written
/// before it needs the work done.
///
/// The backfill goes through a keyed temporary table rather than a correlated
/// subquery: `clipboard_fts.id` is UNINDEXED, so the direct form is one full
/// scan of the plaintext index per item row.
fn add_fts_rowid_and_sync_indexes(tx: &Transaction<'_>) -> HookResult {
    if !has_column(tx, "fts_rowid")? {
        tx.execute_batch("ALTER TABLE clipboard_items ADD COLUMN fts_rowid INTEGER;")?;
        tx.execute_batch(
            "CREATE TEMP TABLE fts_rowid_map (id TEXT PRIMARY KEY, rowid_value INTEGER);
             INSERT OR REPLACE INTO fts_rowid_map (id, rowid_value)
                 SELECT id, rowid FROM clipboard_fts;
             UPDATE clipboard_items SET fts_rowid =
                 (SELECT rowid_value FROM fts_rowid_map m WHERE m.id = clipboard_items.id)
               WHERE id IN (SELECT id FROM fts_rowid_map);
             DROP TABLE fts_rowid_map;",
        )?;
    }
    tx.execute_batch(
        "CREATE INDEX IF NOT EXISTS idx_items_syncable
             ON clipboard_items(created_at, id, content_hash, deleted, origin_device_id,
                                pinned, pin_order, pin_updated_at, is_sensitive)
             WHERE deleted = 1 OR is_sensitive = 0;
         CREATE INDEX IF NOT EXISTS idx_items_sync_cursor
             ON clipboard_items(MAX(created_at, pin_updated_at), id)
             WHERE deleted = 1 OR is_sensitive = 0;",
    )?;
    Ok(())
}

fn seed_live_count(conn: &Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE clipboard_live_count \
            SET live = (SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0)",
        [],
    )?;
    Ok(())
}

fn has_column(tx: &Transaction<'_>, name: &str) -> Result<bool, rusqlite::Error> {
    tx.query_row(
        "SELECT 1 FROM pragma_table_info('clipboard_items') WHERE name = ?1 LIMIT 1",
        [name],
        |_| Ok(()),
    )
    .optional()
    .map(|found| found.is_some())
}

/// Creates the v2 schema. A database written by a newer schema is refused.
pub(super) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    MIGRATIONS.to_latest(conn)?;
    // `clipboard_live_count` is derived state, so it is recomputed rather than
    // trusted. This is the one full `COUNT(*)` v2 pays, and paying it per open
    // is what lets the history cap gate be a single-row read per capture.
    seed_live_count(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::super::connection::run_pragma;
    use super::super::dbfile::open_validated;
    use super::super::schema_verify::verify_schema;
    use super::super::test_support::{item, KEY, T0};
    use super::super::{Store, StoreError};

    fn rewrite_as_early_v2(path: &std::path::Path, version: i64, columns: &str) {
        let store = Store::open(path, &KEY).unwrap();
        let conn = store.conn().unwrap();
        conn.execute_batch(&format!(
            "DROP TABLE sync_device_name;
             DROP TABLE sync_device_state;
             DROP TABLE clipboard_fts;
             DROP TABLE clipboard_items;
             CREATE TABLE clipboard_items ({columns});
             CREATE VIRTUAL TABLE clipboard_fts USING fts5(id UNINDEXED, content_text);
             INSERT INTO clipboard_items (id, content_type, content_hash, created_at)
             VALUES ('historic-item', 'text/plain', 'historic-hash', 1);
             PRAGMA user_version = {version};"
        ))
        .unwrap();
    }

    const EARLY_COLUMNS: &str = "
        id TEXT PRIMARY KEY NOT NULL,
        content_ciphertext BLOB,
        nonce BLOB,
        content_type TEXT NOT NULL,
        content_hash TEXT NOT NULL DEFAULT '',
        is_sensitive INTEGER NOT NULL DEFAULT 0,
        pinned INTEGER NOT NULL DEFAULT 0,
        pin_order REAL,
        created_at INTEGER NOT NULL,
        deleted INTEGER NOT NULL DEFAULT 0";

    const PRE_PIN_COLUMNS: &str = "
        id TEXT PRIMARY KEY NOT NULL,
        content_ciphertext BLOB,
        nonce BLOB,
        content_type TEXT NOT NULL,
        content_hash TEXT NOT NULL DEFAULT '',
        is_sensitive INTEGER NOT NULL DEFAULT 0,
        pinned INTEGER NOT NULL DEFAULT 0,
        pin_order REAL,
        created_at INTEGER NOT NULL,
        deleted INTEGER NOT NULL DEFAULT 0,
        origin_device_id TEXT NOT NULL DEFAULT '',
        app_bundle_id TEXT,
        payload_metadata TEXT";

    #[test]
    fn repairs_a_v2_history_from_before_the_schema_ladder_was_complete() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        rewrite_as_early_v2(&path, 1, EARLY_COLUMNS);

        let store = Store::open(&path, &KEY).unwrap();
        let conn = store.conn().unwrap();
        verify_schema(&conn).unwrap();
        drop(conn);
        assert_eq!(
            store.list_from(None, 10).unwrap().items[0].id,
            "historic-item"
        );
    }

    #[test]
    fn repairs_a_v2_history_that_already_has_the_old_second_migration() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        rewrite_as_early_v2(&path, 2, PRE_PIN_COLUMNS);

        let store = Store::open(&path, &KEY).unwrap();
        let conn = store.conn().unwrap();
        verify_schema(&conn).unwrap();
        drop(conn);
        assert_eq!(
            store.list_from(None, 10).unwrap().items[0].id,
            "historic-item"
        );
    }

    /// The `fts_rowid` back-pointer is the only key a delete now uses, so a
    /// history written before the column existed has to be mapped onto it. An
    /// unmapped row would leave the plaintext of a deleted item in the index.
    #[test]
    fn index_rows_written_before_the_back_pointer_are_mapped_onto_it() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        rewrite_as_early_v2(&path, 1, EARLY_COLUMNS);
        // Planted on a connection that does not migrate, so these rows carry
        // FTS5's own rowids and no back-pointer — the state an upgrade finds.
        open_validated(&path, &KEY)
            .unwrap()
            .execute_batch(
                "INSERT INTO clipboard_fts (id, content_text) \
                 VALUES ('decoy', 'unrelated text');
                 INSERT INTO clipboard_fts (id, content_text) \
                 VALUES ('historic-item', 'historic plaintext');",
            )
            .unwrap();

        let store = Store::open(&path, &KEY).unwrap();
        assert!(store.delete("historic-item").unwrap());

        let conn = store.conn().unwrap();
        let left: Vec<String> = conn
            .prepare("SELECT id FROM clipboard_fts")
            .unwrap()
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert_eq!(
            left,
            vec!["decoy".to_string()],
            "the delete must take its own index row and only its own"
        );
    }

    /// The live count is derived state, so a value that has drifted — by a
    /// crash between the row write and the trigger, or by a tool that wrote
    /// rows behind them — must not survive a restart. The cap evicts from this
    /// number, and a wrong one either over-deletes or stops enforcing.
    #[test]
    fn a_drifted_live_count_is_recomputed_on_open() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        {
            let store = Store::open(&path, &KEY).unwrap();
            store.insert(item("first", T0)).unwrap();
            store.insert(item("second", T0 + 60_000)).unwrap();
            store
                .conn()
                .unwrap()
                .execute("UPDATE clipboard_live_count SET live = 99", [])
                .unwrap();
            assert_eq!(store.count().unwrap(), 99, "the drift must be real");
        }

        assert_eq!(Store::open(&path, &KEY).unwrap().count().unwrap(), 2);
    }

    /// A repaired v2 history has to get the *triggers* back, not only the
    /// table. Without them the counter freezes at whatever the open computed
    /// and the history cap silently stops enforcing.
    #[test]
    fn a_repaired_schema_regains_the_live_count_triggers() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        rewrite_as_early_v2(&path, 1, EARLY_COLUMNS);

        let store = Store::open(&path, &KEY).unwrap();
        assert_eq!(store.count().unwrap(), 1, "the planted row is live");

        let added = store.insert(item("after the repair", T0)).unwrap();
        assert_eq!(store.count().unwrap(), 2);
        assert!(store.delete(&added.id).unwrap());
        assert_eq!(store.count().unwrap(), 1);
    }

    #[test]
    fn a_future_schema_version_is_refused() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        {
            let s = Store::open(&path, &KEY).unwrap();
            let conn = s.conn().unwrap();
            run_pragma(&conn, "PRAGMA user_version = 999").unwrap();
        }
        let err = Store::open(&path, &KEY).unwrap_err();
        assert!(
            matches!(err, StoreError::Migration(_)),
            "expected a migration error, got {err:?}"
        );
    }
}
