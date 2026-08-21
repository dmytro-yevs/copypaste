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
//! * **One view of the history, for both sync transports.** The four merge keys
//!   are columns of `clipboard_items` and are projected into [`StoredItem`], so
//!   [`versions`] serves the peer and cloud paths from the rows the store
//!   already owns. v2 briefly shadowed `origin_device_id` in a side table on a
//!   second connection; two answers to "what is in this device's history" drift
//!   the first time an eviction lands on one and not the other.
//! * **One live row per distinct content.** [`Store::insert_or_bump`] promotes
//!   the existing row rather than writing a second one, across all of history
//!   and not only a recent window (manifest 01 I-23).
//! * **Errors never contain a filesystem path** (AGENTS.md rule 4 — the path
//!   discloses the local username). Nothing in [`StoreError`] formats a path,
//!   and the underlying `rusqlite` errors do not carry one either.
//!
//! The schema is created only for a newly reserved file. Every existing file
//! must match it exactly before connection pragmas or application queries run.

mod connection;
mod dbfile;
mod identity;
mod items;
mod merge_page;
mod model;
mod page;
mod pinning;
mod retention;
mod schema;
mod schema_upgrade;
mod schema_verify;
mod search;
mod state;
mod store;
mod versions;

pub use dbfile::{
    attach_key_literal, open_validated, verify_integrity, verify_schema, RestoreError,
};
pub use identity::DeviceIdentity;
pub(crate) use merge_page::MergePageError;
pub use model::{Ingest, NewItem, StoreError, StoredItem};
pub use page::{ItemCursor, Page};
pub use retention::{compute_content_hash, DEDUP_WINDOW_MS};
pub use search::IndexedText;
pub use store::Store;
pub use versions::{origin_or, IncomingItem, Version};

pub(crate) use search::{indexed_texts_in, purge_from_index_in, purge_index_of_unsearchable_in};

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

    /// Fails an `event` on `clipboard_items` whenever `when` holds, so a test
    /// can prove what a caller does when the database refuses rather than assume
    /// it. `event` is a trigger event such as `INSERT` or `UPDATE OF pinned`;
    /// `when` is a SQL predicate, `1` to refuse every one.
    pub(crate) fn reject_writes(store: &Store, name: &str, event: &str, when: &str) {
        store
            .conn()
            .unwrap()
            .execute_batch(&format!(
                "CREATE TRIGGER {name} BEFORE {event} ON clipboard_items WHEN {when} \
                 BEGIN SELECT RAISE(ABORT, 'injected write failure'); END;"
            ))
            .unwrap();
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
            app_bundle_id: None,
            app_name: None,
            payload_metadata: None,
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
        plant_fts_bytes(store, id, text.as_bytes());
    }

    /// The same, for bytes that are not valid UTF-8. `content_text` is a `TEXT`
    /// column in a file this process does not exclusively own, and a `String`
    /// read of one undecodable row used to abort the whole rescan.
    pub(crate) fn plant_fts_bytes(store: &Store, id: &str, bytes: &[u8]) {
        let conn = store.conn().unwrap();
        conn.execute(
            "INSERT INTO clipboard_fts (id, content_text) VALUES (?1, CAST(?2 AS TEXT))",
            rusqlite::params![id, bytes],
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

    /// `EXPLAIN QUERY PLAN` for a statement, one string per step.
    ///
    /// Parameters are bound to NULL: the plan is chosen from the statement, not
    /// from the values.
    pub(crate) fn plan_of(store: &Store, sql: &str) -> Vec<String> {
        let conn = store.conn().unwrap();
        let mut stmt = conn.prepare(&format!("EXPLAIN QUERY PLAN {sql}")).unwrap();
        let bound: Vec<rusqlite::types::Null> = (0..stmt.parameter_count())
            .map(|_| rusqlite::types::Null)
            .collect();
        stmt.query_map(rusqlite::params_from_iter(bound), |row| {
            row.get::<_, String>(3)
        })
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
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
