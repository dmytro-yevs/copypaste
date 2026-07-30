//! The schema and the migration ladder: one version, one `M::up`. A
//! `Migrations` value rather than a hand-rolled `user_version` runner, so
//! adding version 2 later is an append and a database written by a *newer*
//! build is refused rather than silently downgraded.

use std::sync::LazyLock;

use rusqlite::Connection;
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
    -- Milliseconds since the Unix epoch. Every timestamp in this schema is ms.
    created_at         INTEGER NOT NULL,
    deleted            INTEGER NOT NULL DEFAULT 0
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
"#;

static MIGRATIONS: LazyLock<Migrations<'static>> =
    LazyLock::new(|| Migrations::new(vec![M::up(SCHEMA_V1)]));

/// Runs the ladder. `rusqlite_migration` owns `PRAGMA user_version`; a database
/// written by a *newer* schema is refused rather than silently downgraded.
pub(super) fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    MIGRATIONS.to_latest(conn)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::super::connection::run_pragma;
    use super::super::test_support::KEY;
    use super::super::{Store, StoreError};

    #[test]
    fn a_future_schema_version_is_refused() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("clipboard.db");
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
