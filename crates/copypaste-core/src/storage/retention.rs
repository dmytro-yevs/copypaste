//! What stays and what goes: the dedup window that decides whether a capture is
//! a *new* clipboard event, and the two sweeps that enforce the history cap and
//! the TTL.
//!
//! One rule spans both halves and is the reason they share a module: **a pinned
//! item is never removed by anything in here.** Dedup will not fold one away,
//! and neither eviction path can select one.

use rusqlite::{params, OptionalExtension, Transaction};

use super::model::{item_columns, row_to_item, StoreError, StoredItem};
use super::store::Store;

/// Dedup window: two captures of identical content inside this interval are the
/// same clipboard event.
///
/// A genuine 60-second interval. v1 bucketed on `(wall_time / 60)` where
/// `wall_time` is *milliseconds*, so its "minute" bucket was really 60 ms; that
/// wart is reference-only now.
pub const DEDUP_WINDOW_MS: i64 = 60_000;

/// Width of the storage-level dedup bucket, in milliseconds. Kept equal to
/// [`DEDUP_WINDOW_MS`] so the UNIQUE-index backstop and the application probe
/// agree about what "the same clipboard event" means.
const DEDUP_BUCKET_MS: i64 = DEDUP_WINDOW_MS;

impl Store {
    /// Most recent live item with this content hash at or after `since_ms`.
    ///
    /// Callers pass `now_ms - DEDUP_WINDOW_MS`. `deleted = 0` is mandatory:
    /// tombstones keep their `content_hash`, and without the filter a re-copy of
    /// a deleted item would match the tombstone and never come back.
    pub fn find_recent_by_hash(
        &self,
        content_hash: &str,
        since_ms: i64,
    ) -> Result<Option<StoredItem>, StoreError> {
        if content_hash.is_empty() {
            return Ok(None);
        }
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items \
              WHERE content_hash = ?1 AND created_at >= ?2 AND deleted = 0 \
              ORDER BY created_at DESC, id DESC LIMIT 1"
        ))?;
        Ok(stmt
            .query_row(params![content_hash, since_ms], row_to_item)
            .optional()?)
    }

    /// Enforces the history cap: hard-deletes the oldest unpinned items until at
    /// most `max_items` live items remain. Returns how many were removed.
    ///
    /// Two rules carried from v1, both load-bearing:
    ///
    /// * **Pinned items are never evicted** (I9). If pins alone exceed the cap,
    ///   the cap is simply not reached.
    /// * **The newest unpinned item is never evicted**, so "user copies a huge
    ///   item and it instantly vanishes" cannot happen.
    ///
    /// Eviction is a hard delete, not a tombstone: it is local housekeeping, not
    /// a user-visible delete event to propagate.
    pub fn evict_over_cap(&self, max_items: u64) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        let live: i64 = tx.query_row(
            "SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0",
            [],
            |r| r.get(0),
        )?;
        let live = live.max(0) as u64;
        if live <= max_items {
            return Ok(0);
        }
        let excess = live - max_items;

        // Empty-string sentinel: `id <> ''` matches every real row.
        let keep_id: String = tx
            .query_row(
                "SELECT id FROM clipboard_items WHERE deleted = 0 AND pinned = 0 \
                  ORDER BY created_at DESC, id DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .optional()?
            .unwrap_or_default();

        let victims: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM clipboard_items \
                  WHERE deleted = 0 AND pinned = 0 AND id <> ?1 \
                  ORDER BY created_at ASC, id ASC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![keep_id, excess as i64], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let removed = hard_delete_in_tx(&tx, &victims)?;
        tx.commit()?;
        Ok(removed)
    }

    /// Enforces a TTL: hard-deletes unpinned live items created before
    /// `cutoff_ms`. Returns how many were removed.
    ///
    /// Pinned items are never TTL-deleted (I9).
    pub fn evict_older_than(&self, cutoff_ms: i64) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let victims: Vec<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM clipboard_items \
                  WHERE deleted = 0 AND pinned = 0 AND created_at < ?1",
            )?;
            let rows = stmt.query_map([cutoff_ms], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };
        let removed = hard_delete_in_tx(&tx, &victims)?;
        tx.commit()?;
        Ok(removed)
    }
}

/// Resolves the row that won a dedup race, inside the caller's transaction.
pub(super) fn find_in_bucket(
    tx: &Transaction<'_>,
    content_hash: &str,
    created_at: i64,
) -> rusqlite::Result<Option<StoredItem>> {
    if content_hash.is_empty() {
        return Ok(None);
    }
    let mut stmt = tx.prepare(concat!(
        "SELECT ",
        item_columns!(),
        " FROM clipboard_items \
          WHERE content_hash = ?1 AND created_at / ?2 = ?3 AND deleted = 0 \
          ORDER BY created_at DESC, id DESC LIMIT 1"
    ))?;
    stmt.query_row(
        params![content_hash, DEDUP_BUCKET_MS, created_at / DEDUP_BUCKET_MS],
        row_to_item,
    )
    .optional()
}

/// Hard-deletes rows and their FTS entries in the caller's transaction, so the
/// index can never drift from the table.
fn hard_delete_in_tx(tx: &Transaction<'_>, ids: &[String]) -> rusqlite::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    let mut del_fts = tx.prepare("DELETE FROM clipboard_fts WHERE id = ?1")?;
    let mut del_item = tx.prepare("DELETE FROM clipboard_items WHERE id = ?1")?;
    let mut removed = 0u64;
    for id in ids {
        del_fts.execute([id])?;
        removed += del_item.execute([id])? as u64;
    }
    Ok(removed)
}

/// The content hash used for dedup: the **full** 64-character lowercase
/// SHA-256 hex digest of the raw, pre-encryption bytes.
///
/// Never truncate it. A v1 daemon helper cut the digest to 16 bytes / 32 hex
/// chars, which threw away second-preimage resistance for nothing
/// (`CopyPaste-y4v1`). Hashing the content only — not the source app or any
/// other metadata — is what makes the same text copied from two apps dedup.
pub fn compute_content_hash(raw: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(raw))
}

#[cfg(test)]
mod tests {
    use super::super::test_support::{fts_row_count, hash_of, item, store, T0};
    use super::*;

    #[test]
    fn dedup_window_hit_and_miss() {
        let s = store();
        let hash = hash_of("repeated clipboard content");
        let first = s.insert(item("repeated clipboard content", T0)).unwrap();

        // Hit: the item is inside the window.
        let hit = s
            .find_recent_by_hash(&hash, (T0 + 30_000) - DEDUP_WINDOW_MS)
            .unwrap();
        assert_eq!(hit.map(|i| i.id), Some(first.id.clone()));

        // Miss: the item is older than the window.
        let miss = s
            .find_recent_by_hash(&hash, (T0 + 90_000) - DEDUP_WINDOW_MS)
            .unwrap();
        assert!(miss.is_none());

        // Miss: unknown hash.
        assert!(s
            .find_recent_by_hash(&hash_of("never copied"), T0 - DEDUP_WINDOW_MS)
            .unwrap()
            .is_none());

        // A tombstone must not satisfy the probe, or a re-copy of a deleted item
        // could never come back.
        s.delete(&first.id).unwrap();
        assert!(s
            .find_recent_by_hash(&hash, T0 - DEDUP_WINDOW_MS)
            .unwrap()
            .is_none());
        let recopy = s.insert(item("repeated clipboard content", T0)).unwrap();
        assert_ne!(recopy.id, first.id);
        assert_eq!(s.count().unwrap(), 1);
    }

    #[test]
    fn dedup_index_makes_a_same_bucket_reinsert_idempotent() {
        let s = store();
        let first = s.insert(item("same content", T0)).unwrap();
        // Same bucket (well inside 60s): no new row, the winner comes back.
        let again = s.insert(item("same content", T0 + 5_000)).unwrap();
        assert_eq!(again.id, first.id);
        assert_eq!(s.count().unwrap(), 1);

        // A later bucket is a genuinely new clipboard event.
        let later = s.insert(item("same content", T0 + 120_000)).unwrap();
        assert_ne!(later.id, first.id);
        assert_eq!(s.count().unwrap(), 2);
    }

    #[test]
    fn eviction_respects_pins() {
        let s = store();
        let mut ids = Vec::new();
        for n in 0..5 {
            ids.push(
                s.insert(item(&format!("item {n}"), T0 + n * 60_000))
                    .unwrap()
                    .id,
            );
        }
        // Pin the oldest — it must survive even though it is the first victim by
        // age.
        assert!(s.set_pinned(&ids[0], true).unwrap());

        assert_eq!(s.evict_over_cap(10).unwrap(), 0);

        let removed = s.evict_over_cap(2).unwrap();
        assert_eq!(removed, 3);
        assert_eq!(s.count().unwrap(), 2);

        // The pinned oldest survived; so did the newest unpinned item.
        assert!(s.get(&ids[0]).unwrap().is_some());
        assert!(s.get(&ids[4]).unwrap().is_some());
        for id in &ids[1..4] {
            assert!(s.get(id).unwrap().is_none());
            // Evicted rows leave no FTS orphans.
            assert_eq!(fts_row_count(&s, id), 0);
        }

        // Pins alone can hold the store over the cap; they are never evicted.
        assert_eq!(s.evict_over_cap(0).unwrap(), 0);
        assert!(s.get(&ids[0]).unwrap().is_some());
    }

    #[test]
    fn ttl_eviction_respects_pins() {
        let s = store();
        let old = s.insert(item("stale", T0)).unwrap();
        let old_pinned = s.insert(item("stale but pinned", T0 + 1)).unwrap();
        let fresh = s.insert(item("fresh", T0 + 600_000)).unwrap();
        assert!(s.set_pinned(&old_pinned.id, true).unwrap());

        assert_eq!(s.evict_older_than(T0 + 300_000).unwrap(), 1);
        assert!(s.get(&old.id).unwrap().is_none());
        assert!(s.get(&old_pinned.id).unwrap().is_some());
        assert!(s.get(&fresh.id).unwrap().is_some());
    }

    #[test]
    fn content_hash_is_the_full_sha256_hex() {
        let empty = compute_content_hash(b"");
        assert_eq!(empty.len(), 64);
        assert_eq!(
            empty,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            compute_content_hash(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Content-only: identical bytes hash identically, different bytes do not.
        assert_eq!(compute_content_hash(b"abc"), compute_content_hash(b"abc"));
        assert_ne!(compute_content_hash(b"abc"), compute_content_hash(b"abd"));
    }
}
