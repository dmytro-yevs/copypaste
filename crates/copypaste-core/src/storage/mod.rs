//! Storage — the single durable store of clipboard history on a device.
//!
//! One SQLCipher-encrypted SQLite file, one schema version, one r2d2 pool.
//!
//! # Carried over from v1 (port manifest 03)
//!
//! * **Sensitive items never reach the search index** (ADR-015). Three
//!   enforcement layers, all required: the write guard in [`Store::insert`]
//!   ([`items`]), the in-transaction re-read in `upsert_fts_in_tx` and the
//!   `is_sensitive = 0` predicate on the search JOIN (both [`search`]). v1
//!   shipped databases containing plaintext passwords in FTS because one layer
//!   was missing. All three decide at capture, which is why
//!   [`crate::sensitive::purge_indexed_secrets`] exists to revisit rows a later
//!   ruleset would have caught.
//! * **Tombstones.** [`Store::delete`] soft-deletes: the ciphertext and nonce
//!   are wiped, the FTS row is removed in the same transaction, and the row
//!   survives as a delete event that a later sync/LWW layer can propagate.
//!   `content_hash` is deliberately kept on the tombstone, which is why every
//!   dedup query filters `deleted = 0` — without that a re-copy of a deleted
//!   item could never come back (`CopyPaste-crh3.67`, `CopyPaste-fuxl`).
//! * **Pinned items are never auto-deleted** — every eviction path filters
//!   `pinned = 0`.
//! * **Total ordering.** Every list query ends in an `id` tiebreak so the sort
//!   is total; that is what [`Store::list_from`]'s keyset seek requires
//!   ([`page`]) and what keeps offset pages from duplicating or skipping rows
//!   that tie on `created_at`.
//! * **One live row per distinct content.** [`Store::insert_or_bump`] promotes
//!   the existing row rather than writing a second one, across all of history
//!   and not only a recent window (manifest 01 I-23).
//! * **Errors never contain a filesystem path** (CLAUDE.md rule 4 — the path
//!   discloses the local username). Nothing in [`StoreError`] formats a path,
//!   and the underlying `rusqlite` errors do not carry one either.
//!
//! v2 drops backward compatibility (`CLAUDE.md` rule 3), so the v1→v15
//! migration ladder, `migration_state` and `key_version` dispatch are gone: the
//! schema starts clean at version 1 and `rusqlite_migration` owns the ladder.
//! The one obligation that creates is [`legacy`]: a v0.4.x file is identified
//! and refused with [`StoreError::LegacyDatabase`], never opened and never
//! reported as corrupt.

mod connection;
mod dbfile;
mod items;
mod legacy;
mod model;
mod page;
mod pinning;
mod retention;
mod schema;
mod search;
mod store;

pub use dbfile::{attach_key_literal, open_validated, verify_integrity};
pub use legacy::{is_v1_database, v1_database_in, V1_DATABASE_FILENAME};
pub use model::{Ingest, NewItem, StoreError, StoredItem};
pub use page::{ItemCursor, Page};
pub use retention::{compute_content_hash, DEDUP_WINDOW_MS};
pub use search::IndexedText;
pub use store::Store;

/// Fixtures shared by every test module under `storage`. The direct-SQL helpers
/// let a test assert what is *in* the FTS table rather than only what `search`
/// returns — that is how a missing ADR-015 layer would be caught.
#[cfg(test)]
pub(crate) mod test_support {
    use super::{NewItem, Store};

    pub(super) const KEY: [u8; 32] = [7u8; 32];
    pub(super) const OTHER_KEY: [u8; 32] = [9u8; 32];
    pub(crate) const T0: i64 = 1_700_000_000_000;

    /// Rows with this id, tombstones included — the difference between a soft
    /// delete and a hard one.
    pub(crate) fn raw_row_count(store: &Store, id: &str) -> i64 {
        let conn = store.conn().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    }

    pub(crate) fn store() -> Store {
        Store::open_in_memory(&KEY).expect("in-memory store")
    }

    /// Distinct content gets a distinct hash; that is all the dedup index needs.
    pub(super) fn hash_of(text: &str) -> String {
        hex::encode(text.as_bytes())
    }

    pub(crate) fn item(text: &str, created_at: i64) -> NewItem {
        NewItem {
            id: uuid::Uuid::new_v4().to_string(),
            content_ciphertext: format!("ct:{text}").into_bytes(),
            nonce: vec![1u8; 24],
            content_type: "text".to_string(),
            content_hash: hash_of(text),
            is_sensitive: false,
            search_text: Some(text.to_string()),
            created_at,
        }
    }

    pub(super) fn sensitive_item(text: &str, created_at: i64) -> NewItem {
        NewItem {
            is_sensitive: true,
            search_text: None,
            ..item(text, created_at)
        }
    }

    /// An index row written straight past every write-time guard — the state a
    /// database from before a guard existed is actually in.
    pub(crate) fn plant_fts_row(store: &Store, id: &str, text: &str) {
        let conn = store.conn().unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, ?2)",
            rusqlite::params![id, text],
        )
        .unwrap();
    }

    pub(crate) fn fts_row_count(store: &Store, id: &str) -> i64 {
        let conn = store.conn().unwrap();
        conn.query_row(
            "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .unwrap()
    }

    pub(super) fn fts_dump(store: &Store) -> String {
        let conn = store.conn().unwrap();
        let mut stmt = conn
            .prepare("SELECT content_text FROM clipboard_fts")
            .unwrap();
        let rows: Vec<String> = stmt
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        rows.join("\n")
    }
}
