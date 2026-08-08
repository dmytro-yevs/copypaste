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

const SCHEMA_VERSION: i64 = 6;

struct Column {
    name: &'static str,
    declared_type: &'static str,
    not_null: bool,
    primary_key: bool,
}

struct Table {
    name: &'static str,
    columns: &'static [Column],
    virtual_fts5: bool,
}

struct ActualColumn {
    name: String,
    declared_type: String,
    not_null: bool,
    primary_key: bool,
}

const CLIPBOARD_ITEMS_COLUMNS: &[Column] = &[
    Column {
        name: "id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: true,
    },
    Column {
        name: "content_ciphertext",
        declared_type: "BLOB",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "nonce",
        declared_type: "BLOB",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "content_type",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "content_hash",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "is_sensitive",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "pinned",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "pin_order",
        declared_type: "REAL",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "pin_updated_at",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "created_at",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "deleted",
        declared_type: "INTEGER",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "origin_device_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
    Column {
        name: "app_bundle_id",
        declared_type: "TEXT",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "app_name",
        declared_type: "TEXT",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "payload_metadata",
        declared_type: "TEXT",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "fts_rowid",
        declared_type: "INTEGER",
        not_null: false,
        primary_key: false,
    },
];

const CLIPBOARD_FTS_COLUMNS: &[Column] = &[
    Column {
        name: "id",
        declared_type: "",
        not_null: false,
        primary_key: false,
    },
    Column {
        name: "content_text",
        declared_type: "",
        not_null: false,
        primary_key: false,
    },
];

const SYNC_DEVICE_STATE_COLUMNS: &[Column] = &[
    Column {
        name: "key",
        declared_type: "TEXT",
        not_null: true,
        primary_key: true,
    },
    Column {
        name: "value",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
];

const SYNC_DEVICE_NAME_COLUMNS: &[Column] = &[
    Column {
        name: "device_id",
        declared_type: "TEXT",
        not_null: true,
        primary_key: true,
    },
    Column {
        name: "name",
        declared_type: "TEXT",
        not_null: true,
        primary_key: false,
    },
];

const TABLES: &[Table] = &[
    Table {
        name: "clipboard_items",
        columns: CLIPBOARD_ITEMS_COLUMNS,
        virtual_fts5: false,
    },
    Table {
        name: "clipboard_fts",
        columns: CLIPBOARD_FTS_COLUMNS,
        virtual_fts5: true,
    },
    Table {
        name: "sync_device_state",
        columns: SYNC_DEVICE_STATE_COLUMNS,
        virtual_fts5: false,
    },
    Table {
        name: "sync_device_name",
        columns: SYNC_DEVICE_NAME_COLUMNS,
        virtual_fts5: false,
    },
];

static MIGRATIONS: LazyLock<Migrations<'static>> = LazyLock::new(|| {
    Migrations::new(vec![
        M::up(SCHEMA_V1),
        M::up_with_hook("", repair_early_v2_schema),
        M::up_with_hook("", repair_early_v2_schema),
        M::up_with_hook("", repair_early_v2_schema),
        M::up_with_hook("", repair_dedup_index),
        M::up_with_hook("", add_fts_rowid_and_sync_indexes),
    ])
});

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
    Ok(())
}

pub(super) fn verify_schema(conn: &Connection) -> Result<(), StoreError> {
    let version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if version != SCHEMA_VERSION {
        return Err(StoreError::InvalidSchema);
    }

    for table in TABLES {
        verify_table(conn, table)?;
    }
    Ok(())
}

pub(super) fn is_current_table(name: &str) -> bool {
    TABLES.iter().any(|table| table.name == name)
}

fn verify_table(conn: &Connection, expected: &Table) -> Result<(), StoreError> {
    let sql: Option<String> = conn
        .query_row(
            "SELECT sql FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [expected.name],
            |row| row.get(0),
        )
        .optional()?;
    let Some(sql) = sql else {
        return Err(StoreError::InvalidSchema);
    };
    if expected.virtual_fts5 {
        let definition = sql.trim_start().to_ascii_lowercase();
        if !definition.starts_with("create virtual table") || !definition.contains("using fts5") {
            return Err(StoreError::InvalidSchema);
        }
    }

    let pragma = format!("PRAGMA table_info({})", quote_identifier(expected.name));
    let mut statement = conn.prepare(&pragma)?;
    let actual = statement
        .query_map([], |row| {
            Ok(ActualColumn {
                name: row.get(1)?,
                declared_type: row.get(2)?,
                not_null: row.get::<_, i64>(3)? != 0,
                primary_key: row.get::<_, i64>(5)? != 0,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if actual.len() != expected.columns.len()
        || expected.columns.iter().any(|expected_column| {
            actual
                .iter()
                .find(|actual| actual.name == expected_column.name)
                .is_none_or(|actual| {
                    actual.declared_type != expected_column.declared_type
                        || actual.not_null != expected_column.not_null
                        || actual.primary_key != expected_column.primary_key
                })
        })
    {
        return Err(StoreError::InvalidSchema);
    }
    Ok(())
}

fn quote_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::super::connection::run_pragma;
    use super::super::dbfile::open_validated;
    use super::super::test_support::KEY;
    use super::super::{Store, StoreError};
    use super::verify_schema;

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
    fn the_current_schema_passes_validation() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        let conn = store.conn().unwrap();

        verify_schema(&conn).unwrap();
    }

    #[test]
    fn schema_validation_rejects_a_column_with_the_wrong_type() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("copypaste-v2.db");
        let store = Store::open(&path, &KEY).unwrap();
        let conn = store.conn().unwrap();
        conn.execute_batch(
            "DROP TABLE sync_device_name;
             CREATE TABLE sync_device_name (
                 device_id BLOB PRIMARY KEY NOT NULL,
                 name TEXT NOT NULL
             );",
        )
        .unwrap();

        assert!(matches!(
            verify_schema(&conn),
            Err(StoreError::InvalidSchema)
        ));
    }

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
