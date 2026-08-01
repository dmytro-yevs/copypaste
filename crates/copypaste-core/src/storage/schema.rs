//! The schema and the migration ladder: one version, one `M::up`. A
//! `Migrations` value rather than a hand-rolled `user_version` runner, so
//! adding version 2 later is an append and a database written by a *newer*
//! build is refused rather than silently downgraded.

use std::sync::LazyLock;

use rusqlite::{Connection, OptionalExtension};
use rusqlite_migration::{Migrations, M};

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
    origin_device_id   TEXT    NOT NULL DEFAULT ''
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
-- so two ingest events with identical content can both observe "no recent
-- row". This makes the second INSERT fail; insert() then re-reads the winner
-- inside the same transaction and returns it, which is what makes dedup
-- idempotent. Excludes empty hashes so hash-less rows do not all collide, and
-- excludes tombstones so a re-copy after a delete is a fresh live row.
CREATE UNIQUE INDEX idx_items_dedup
    ON clipboard_items(content_hash, created_at / 60000)
    WHERE deleted = 0 AND content_hash <> '';

-- Serves the eviction scans (oldest unpinned live rows first).
CREATE INDEX idx_items_evictable
    ON clipboard_items(created_at, id)
    WHERE deleted = 0 AND pinned = 0;

-- External-content mode is NOT used: there is no cascade from clipboard_items,
-- so every delete path must remove the FTS row explicitly, in the same
-- transaction as the row change.
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

const SCHEMA_VERSION: i64 = 1;

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

static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::new(vec![M::up(SCHEMA_V1)]));

/// Runs the ladder. `rusqlite_migration` owns `PRAGMA user_version`; a database
/// written by a *newer* schema is refused rather than silently downgraded.
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
        || actual
            .iter()
            .zip(expected.columns)
            .any(|(actual, expected)| {
                actual.name != expected.name
                    || actual.declared_type != expected.declared_type
                    || actual.not_null != expected.not_null
                    || actual.primary_key != expected.primary_key
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
    use super::super::test_support::KEY;
    use super::super::{Store, StoreError};
    use super::verify_schema;

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
