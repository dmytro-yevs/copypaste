//! Storage — the single durable store of clipboard history on a device.
//!
//! One SQLCipher-encrypted SQLite file, one schema version, one r2d2 pool.
//!
//! # What this module carries over from v1 (port manifest 03)
//!
//! * **Sensitive items never reach the search index** (ADR-015). Three
//!   enforcement layers, all required: the write guard in [`Store::insert`]
//!   ([`items`]), the in-transaction re-read in `upsert_fts_in_tx` and the
//!   `is_sensitive = 0` predicate on the search JOIN (both [`search`]). v1
//!   shipped databases containing plaintext passwords in FTS because one layer
//!   was missing.
//! * **Tombstones.** [`Store::delete`] soft-deletes: the ciphertext and nonce
//!   are wiped, the FTS row is removed in the same transaction, and the row
//!   survives as a delete event that a later sync/LWW layer can propagate.
//!   `content_hash` is deliberately kept on the tombstone, which is why every
//!   dedup query filters `deleted = 0` — without that a re-copy of a deleted
//!   item could never come back (`CopyPaste-crh3.67`, `CopyPaste-fuxl`).
//! * **Pinned items are never auto-deleted** — every eviction path filters
//!   `pinned = 0`.
//! * **Total ordering.** Every list query ends in an `id` tiebreak so the sort
//!   is total; that is what keyset pagination requires and what keeps offset
//!   pages from duplicating or skipping rows that tie on `created_at`.
//! * **Errors never contain a filesystem path** (CLAUDE.md rule 4 — the path
//!   discloses the local username). Nothing in [`StoreError`] formats a path,
//!   and the underlying `rusqlite` errors do not carry one either.
//!
//! # What this module deliberately does *not* carry over
//!
//! v2 drops backward compatibility (`docs/rewrite/port-manifest/README.md`), so
//! the v1→v15 migration ladder, `migration_state`, `key_version` dispatch and
//! the `(wall_time / 60)` millisecond dedup bucket are gone. The schema starts
//! clean at version 1 and the ladder is owned by `rusqlite_migration` rather
//! than by a hand-rolled `user_version` runner with a retry loop that
//! string-matched `"duplicate column name"`.
//!
//! # Layout
//!
//! [`Store`] is one handle with one pool; its methods are grouped by what they
//! are *for*, because that is what changes together:
//!
//! * [`model`] — the row types, the shared column projection and its mapper.
//! * [`schema`] — the DDL and the migration ladder.
//! * [`connection`] — `PRAGMA key`, the connection pragmas, the pool.
//! * [`store`] — the handle itself: open, open-in-memory, checkout.
//! * [`items`] — insert / read / pin / soft-delete.
//! * [`search`] — FTS5 and the three-layer sensitive exclusion.
//! * [`retention`] — the dedup bucket and both eviction sweeps.

mod connection;
mod items;
mod model;
mod retention;
mod schema;
mod search;
mod store;

pub use model::{NewItem, StoreError, StoredItem};
pub use retention::{compute_content_hash, DEDUP_WINDOW_MS};
pub use store::Store;

/// Fixtures shared by every test module under `storage`.
///
/// Not compiled into a shipping build. The direct-SQL helpers exist so a test
/// can assert what is *in* the FTS table rather than only what `search`
/// returns — that is how a missing ADR-015 layer would be caught.
#[cfg(test)]
pub(super) mod test_support {
    use super::{NewItem, Store};

    pub(super) const KEY: [u8; 32] = [7u8; 32];
    pub(super) const OTHER_KEY: [u8; 32] = [9u8; 32];
    pub(super) const T0: i64 = 1_700_000_000_000;

    pub(super) fn store() -> Store {
        Store::open_in_memory(&KEY).expect("in-memory store")
    }

    /// Distinct content gets a distinct hash; that is all the dedup index needs.
    pub(super) fn hash_of(text: &str) -> String {
        hex::encode(text.as_bytes())
    }

    pub(super) fn item(text: &str, created_at: i64) -> NewItem {
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

    pub(super) fn fts_row_count(store: &Store, id: &str) -> i64 {
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
