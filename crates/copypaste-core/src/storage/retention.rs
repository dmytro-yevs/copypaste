//! What stays and what goes: the lookup and the restamp behind dedup, and the
//! sweeps that enforce the history cap, the age limit and the sensitive TTL.
//!
//! One rule spans all of it and is the reason they share a module: **a pinned
//! item is never removed by anything in here.** Dedup folds one into a bump
//! rather than away, and no sweep can select one.

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::connection::write_tx;
use super::model::{is_constraint_violation, item_columns, row_to_item, StoreError, StoredItem};
use super::store::Store;

/// Width of the UNIQUE-index dedup bucket, in milliseconds.
///
/// This is **not** a dedup window — [`Store::insert_or_bump`] matches across all
/// of history (manifest 01 I-23) and this only sizes the buckets of the index
/// that backstops it against a concurrent double-insert. Two captures further
/// apart than this land in different buckets and the index lets both through;
/// the application-level probe is what collapses them.
///
/// A genuine 60-second interval. v1 bucketed on `(wall_time / 60)` where
/// `wall_time` is *milliseconds*, so its "minute" bucket was really 60 ms; that
/// wart is reference-only now.
pub const DEDUP_WINDOW_MS: i64 = 60_000;

/// The same number, named for the schema's `created_at / 60000`. Kept equal to
/// [`DEDUP_WINDOW_MS`] so the index and the code that resolves its violations
/// agree about where a bucket starts.
const DEDUP_BUCKET_MS: i64 = DEDUP_WINDOW_MS;

impl Store {
    /// Most recent live item with this content hash at or after `since_ms`.
    ///
    /// A bounded probe, for callers that want "was this copied *recently*".
    /// Dedup itself does not use it — [`Store::insert_or_bump`] searches all of
    /// history — so passing `i64::MIN` is the unbounded question.
    pub fn find_recent_by_hash(
        &self,
        content_hash: &str,
        since_ms: i64,
    ) -> Result<Option<StoredItem>, StoreError> {
        let conn = self.conn()?;
        Ok(newest_live_with_hash(&conn, content_hash, since_ms)?)
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
        let tx = write_tx(&mut conn)?;

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
        let tx = write_tx(&mut conn)?;
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

    /// Is there anything the sensitive sweep could possibly delete?
    ///
    /// `CopyPaste-98ja`: on a machine that has never copied a secret, the sweep's
    /// select-and-delete transaction otherwise ran every few seconds forever for
    /// nothing. Answers `true` on a query error so a broken probe can never
    /// suppress the sweep — the TTL is a security guarantee and the probe is
    /// only an optimisation.
    #[must_use]
    pub(crate) fn has_wipeable_sensitive(&self) -> bool {
        let Ok(conn) = self.conn() else {
            return true;
        };
        conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM clipboard_items \
                            WHERE is_sensitive = 1 AND pinned = 0 AND deleted = 0)",
            [],
            |r| r.get::<_, bool>(0),
        )
        .unwrap_or(true)
    }

    /// Sensitive, unpinned, live rows whose capture is older than `cutoff_ms`.
    ///
    /// Candidates for the auto-wipe, not victims of it: whether a row may
    /// actually be deleted is decided from its plaintext against the confidence
    /// floor, in [`crate::sensitive::sweep_sensitive`]. Pinned rows are excluded
    /// here rather than at the delete, so no later edit can lose the exemption
    /// (manifest 03 I9).
    pub(crate) fn expired_sensitive(&self, cutoff_ms: i64) -> Result<Vec<StoredItem>, StoreError> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare_cached(concat!(
            "SELECT ",
            item_columns!(),
            " FROM clipboard_items \
              WHERE is_sensitive = 1 AND pinned = 0 AND deleted = 0 AND created_at < ?1 \
              ORDER BY created_at ASC, id ASC"
        ))?;
        let rows = stmt.query_map([cutoff_ms], row_to_item)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }
}

/// Newest live row carrying `content_hash`, at or after `since_ms`.
///
/// One body for both callers: the bounded probe [`Store::find_recent_by_hash`]
/// exposes, and the unbounded one [`Store::insert_or_bump`] runs inside its
/// transaction with `i64::MIN`. v1 grew a second dedup lookup here and the two
/// disagreed about tombstones.
///
/// `deleted = 0` is mandatory: tombstones keep their `content_hash`, and without
/// the filter a re-copy of a deleted item would match the tombstone and never
/// come back.
pub(super) fn newest_live_with_hash(
    conn: &Connection,
    content_hash: &str,
    since_ms: i64,
) -> rusqlite::Result<Option<StoredItem>> {
    if content_hash.is_empty() {
        return Ok(None);
    }
    let mut stmt = conn.prepare_cached(concat!(
        "SELECT ",
        item_columns!(),
        " FROM clipboard_items \
          WHERE content_hash = ?1 AND created_at >= ?2 AND deleted = 0 \
          ORDER BY created_at DESC, id DESC LIMIT 1"
    ))?;
    stmt.query_row(params![content_hash, since_ms], row_to_item)
        .optional()
}

/// Moves a row's version stamp to `created_at`, in the caller's transaction.
///
/// Forward only (T-37). `pinned` and `pin_order` are untouched, so a re-copied
/// pin keeps its slot instead of jumping to the top of the list (INV-31).
pub(super) fn bump_in_tx(
    tx: &Transaction<'_>,
    existing: &StoredItem,
    created_at: i64,
) -> rusqlite::Result<StoredItem> {
    if created_at <= existing.created_at {
        return Ok(existing.clone());
    }
    match tx.execute(
        "UPDATE clipboard_items SET created_at = ?2 WHERE id = ?1 AND deleted = 0",
        params![&existing.id, created_at],
    ) {
        Ok(_) => Ok(StoredItem {
            created_at,
            ..existing.clone()
        }),
        // The new stamp lands in a dedup bucket another live row already holds.
        // Leaving the row where it is costs a recency bump; failing here would
        // cost the capture, and data loss is the worse outcome.
        Err(e) if is_constraint_violation(&e) => Ok(existing.clone()),
        Err(e) => Err(e),
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

        let hit = s
            .find_recent_by_hash(&hash, (T0 + 30_000) - DEDUP_WINDOW_MS)
            .unwrap();
        assert_eq!(hit.map(|i| i.id), Some(first.id.clone()));

        let miss = s
            .find_recent_by_hash(&hash, (T0 + 90_000) - DEDUP_WINDOW_MS)
            .unwrap();
        assert!(miss.is_none());

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
    }

    /// Manifest 01 I-23 / T-36 / T-39: dedup has no window. A re-copy of
    /// something captured long ago promotes the original row; it does not make a
    /// second one. Until this landed, `Ingested::Duplicate` was only reachable
    /// inside 60 s and history grew a copy every time after that.
    #[test]
    fn a_recopy_far_outside_any_window_bumps_the_original_row() {
        let s = store();
        let first = s.insert(item("same content", T0)).unwrap();
        s.insert(item("something else", T0 + 60_000)).unwrap();

        let a_week_later = s
            .insert_or_bump(item("same content", T0 + 7 * 86_400_000))
            .unwrap();
        assert!(a_week_later.is_bump());
        assert_eq!(a_week_later.item().id, first.id);
        assert_eq!(a_week_later.item().created_at, T0 + 7 * 86_400_000);
        assert_eq!(s.count().unwrap(), 2, "history must not have grown");

        // ...and the bump is what puts it back on top (D9).
        assert_eq!(s.list(10, 0).unwrap()[0].id, first.id);
    }

    /// T-37: a bump never moves a stamp backwards. An out-of-order or
    /// clock-skewed capture must not demote the row it matched, or a peer that
    /// already holds the later version would win the merge and undo it.
    #[test]
    fn a_bump_only_ever_moves_the_stamp_forward() {
        let s = store();
        let first = s.insert(item("same content", T0 + 600_000)).unwrap();
        let earlier = s.insert_or_bump(item("same content", T0)).unwrap();
        assert!(earlier.is_bump());
        assert_eq!(earlier.item().created_at, T0 + 600_000);
        assert_eq!(s.get(&first.id).unwrap().unwrap().created_at, T0 + 600_000);
    }

    /// T-39 names pinned rows explicitly, and INV-31 says what a bump may not
    /// do to one: the stamp moves, the pinned slot does not.
    #[test]
    fn a_pinned_row_is_bumped_in_place_rather_than_jumping_to_the_top() {
        let s = store();
        let pinned = s.insert(item("worth keeping", T0)).unwrap();
        let other_pin = s.insert(item("also pinned", T0 + 1_000)).unwrap();
        s.insert(item("ordinary", T0 + 2_000)).unwrap();
        s.set_pinned(&pinned.id, true).unwrap();
        s.set_pinned(&other_pin.id, true).unwrap();

        let again = s
            .insert_or_bump(item("worth keeping", T0 + 900_000))
            .unwrap();
        assert!(again.is_bump());
        assert_eq!(again.item().id, pinned.id);
        assert!(again.item().pinned, "the pin must survive the bump");

        let ids: Vec<String> = s.list(10, 0).unwrap().into_iter().map(|i| i.id).collect();
        assert_eq!(
            ids[0], pinned.id,
            "pin order decides the pinned section, not recency"
        );
        assert_eq!(ids[1], other_pin.id);
    }

    /// A re-copy after a delete is a genuinely new item: the tombstone keeps its
    /// content hash, and matching against it would mean deleted content could
    /// never come back.
    #[test]
    fn a_recopy_after_a_delete_is_a_fresh_row_not_a_bump() {
        let s = store();
        let first = s.insert(item("gone", T0)).unwrap();
        s.delete(&first.id).unwrap();

        let again = s.insert_or_bump(item("gone", T0 + 120_000)).unwrap();
        assert!(!again.is_bump());
        assert_ne!(again.item().id, first.id);
        assert_eq!(s.count().unwrap(), 1);
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
        assert_eq!(compute_content_hash(b"abc"), compute_content_hash(b"abc"));
        assert_ne!(compute_content_hash(b"abc"), compute_content_hash(b"abd"));
    }
}
