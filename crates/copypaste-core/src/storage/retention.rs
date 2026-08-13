//! What stays and what goes: the lookup and the restamp behind dedup, and the
//! sweeps that enforce the history cap, the age limit and the sensitive TTL.
//!
//! One rule spans all of it and is the reason they share a module: **a pinned
//! item is never removed by anything in here.** Dedup folds one into a bump
//! rather than away, and no sweep can select one.

use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::connection::write_tx;
use super::model::{
    is_constraint_violation, item_columns, row_to_item, ItemColumns, StoreError, StoredItem,
};
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

pub(super) fn live_count(conn: &Connection) -> rusqlite::Result<u64> {
    let live: i64 = conn.query_row(LIVE_COUNT_SQL, [], |r| r.get(0))?;
    Ok(live.max(0) as u64)
}

const LIVE_COUNT_SQL: &str = "SELECT live FROM clipboard_live_count";

fn unpinned_bytes(conn: &Connection) -> rusqlite::Result<u64> {
    let total: i64 = conn.query_row(BYTE_CAP_TOTAL_SQL, [], |r| r.get(0))?;
    Ok(total.max(0) as u64)
}

const BYTE_CAP_TOTAL_SQL: &str =
    "SELECT COALESCE(SUM(LENGTH(COALESCE(content_ciphertext, X''))), 0) \
    FROM clipboard_items INDEXED BY idx_items_unpinned_bytes \
    WHERE deleted = 0 AND pinned = 0";

// `INDEXED BY idx_items_evictable` throughout: nothing writes `sqlite_stat1`
// (`connection::OptimizeOnRelease`), so the planner guesses `idx_items_history`
// and sorts — 1.147 ms against 0.003 ms forced at 10 000 rows.
//
// Each also gates on its own connection before promoting it to an IMMEDIATE
// transaction and re-checks inside, so what gets deleted is unchanged and a
// no-op sweep stops taking the write lock — three per imported item. Sweeping
// one capture later can only delete less, later.

/// The newest unpinned live row — the one [`Store::evict_over_cap`] documents as
/// never evictable. Empty-string sentinel, so `id <> ?1` matches every real row.
fn keep_newest(conn: &Connection) -> rusqlite::Result<String> {
    Ok(conn
        .query_row(KEEP_NEWEST_SQL, [], |r| r.get::<_, String>(0))
        .optional()?
        .unwrap_or_default())
}

const KEEP_NEWEST_SQL: &str = concat!(
    "SELECT id FROM ",
    "clipboard_items INDEXED BY idx_items_evictable",
    " WHERE deleted = 0 AND pinned = 0 ORDER BY created_at DESC, id DESC LIMIT 1"
);

/// Is there a second unpinned live row behind the protected newest one? Without
/// one the quota is unreachable and the victim scan can only come back empty —
/// so an item larger than the quota re-ranked the history on every capture.
fn has_byte_victim(conn: &Connection) -> rusqlite::Result<bool> {
    let found: i64 = conn.query_row(BYTE_CAP_HAS_VICTIM_SQL, [], |r| r.get(0))?;
    Ok(found >= 2)
}

fn has_expired(conn: &Connection, cutoff_ms: i64) -> rusqlite::Result<bool> {
    conn.query_row(AGE_GATE_SQL, [cutoff_ms], |r| r.get(0))
}

const BYTE_CAP_HAS_VICTIM_SQL: &str = concat!(
    "SELECT COUNT(*) FROM (SELECT 1 FROM ",
    "clipboard_items INDEXED BY idx_items_evictable",
    " WHERE deleted = 0 AND pinned = 0 LIMIT 2)"
);

const CAP_VICTIMS_SQL: &str = concat!(
    "SELECT id FROM ",
    "clipboard_items INDEXED BY idx_items_evictable",
    " WHERE deleted = 0 AND pinned = 0 AND id <> ?1 \
      ORDER BY created_at ASC, id ASC LIMIT ?2"
);

/// Ranking runs in index order, so the window needs no sort. Row lengths still
/// come from the table: index-only needs a stored byte column, which `dbfile`'s
/// `INSERT ... SELECT *` restore cannot carry.
const BYTE_VICTIMS_SQL: &str = concat!(
    "WITH ranked AS ( \
        SELECT id, \
               LENGTH(COALESCE(content_ciphertext, X'')) AS row_bytes, \
               SUM(LENGTH(COALESCE(content_ciphertext, X''))) OVER ( \
                   ORDER BY created_at ASC, id ASC ROWS UNBOUNDED PRECEDING \
               ) AS cumulative_bytes \
          FROM ",
    "clipboard_items INDEXED BY idx_items_evictable",
    " WHERE deleted = 0 AND pinned = 0 AND id <> ?1 \
     ) \
     SELECT id FROM ranked WHERE cumulative_bytes - row_bytes < ?2"
);

const AGE_GATE_SQL: &str = concat!(
    "SELECT EXISTS(SELECT 1 FROM ",
    "clipboard_items INDEXED BY idx_items_evictable",
    " WHERE deleted = 0 AND pinned = 0 AND created_at < ?1)"
);

const AGE_VICTIMS_SQL: &str = concat!(
    "SELECT id FROM ",
    "clipboard_items INDEXED BY idx_items_evictable",
    " WHERE deleted = 0 AND pinned = 0 AND created_at < ?1"
);

/// `idx_items_sensitive_wipe`'s partial predicate is these two WHERE clauses, so
/// both cost what the sensitive set costs and it is empty where no secret was
/// ever copied — which is what makes the `CopyPaste-98ja` probe free. Written
/// differently SQLite declines it (`CopyPaste-crh3.3`); `INDEXED BY` errors.
const SENSITIVE_EXISTS_SQL: &str = concat!(
    "SELECT EXISTS(SELECT 1 FROM ",
    "clipboard_items INDEXED BY idx_items_sensitive_wipe",
    " WHERE is_sensitive = 1 AND pinned = 0 AND deleted = 0)"
);

const EXPIRED_SENSITIVE_SQL: &str = concat!(
    "SELECT ",
    item_columns!(),
    " FROM clipboard_items INDEXED BY idx_items_sensitive_wipe \
      WHERE is_sensitive = 1 AND pinned = 0 AND deleted = 0 AND created_at < ?1 \
      ORDER BY created_at ASC, id ASC"
);

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
        if live_count(&conn)? <= max_items {
            return Ok(0);
        }
        let tx = write_tx(&mut conn)?;
        let live = live_count(&tx)?;
        if live <= max_items {
            return Ok(0);
        }
        let excess = live - max_items;

        let keep_id = keep_newest(&tx)?;
        let victims: Vec<String> = {
            let mut stmt = tx.prepare_cached(CAP_VICTIMS_SQL)?;
            let rows = stmt.query_map(params![keep_id, excess as i64], |r| r.get(0))?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let removed = hard_delete_in_tx(&tx, &victims)?;
        tx.commit()?;
        Ok(removed)
    }

    /// Hard-deletes the oldest unpinned live rows until their ciphertext fits
    /// within `max_bytes`, while preserving the newest unpinned live row.
    ///
    /// The quota covers the bytes SQLite stores for the encrypted payload, not
    /// FTS plaintext or database pages. Tombstones carry no ciphertext and are
    /// deliberately retained for sync.
    pub fn evict_over_byte_cap(&self, max_bytes: u64) -> Result<u64, StoreError> {
        let mut conn = self.conn()?;
        if !has_byte_victim(&conn)? || unpinned_bytes(&conn)? <= max_bytes {
            return Ok(0);
        }
        let tx = write_tx(&mut conn)?;
        let total = unpinned_bytes(&tx)?;
        if total <= max_bytes {
            return Ok(0);
        }

        let keep_id = keep_newest(&tx)?;
        let excess =
            i64::try_from(total).unwrap_or(i64::MAX) - i64::try_from(max_bytes).unwrap_or(i64::MAX);
        let victims: Vec<String> = {
            let mut stmt = tx.prepare_cached(BYTE_VICTIMS_SQL)?;
            let rows = stmt.query_map(params![keep_id, excess], |r| r.get(0))?;
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
        if !has_expired(&conn, cutoff_ms)? {
            return Ok(0);
        }
        let tx = write_tx(&mut conn)?;
        let victims: Vec<String> = {
            let mut stmt = tx.prepare_cached(AGE_VICTIMS_SQL)?;
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
        conn.query_row(SENSITIVE_EXISTS_SQL, [], |r| r.get::<_, bool>(0))
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
        let mut stmt = conn.prepare_cached(EXPIRED_SENSITIVE_SQL)?;
        let columns = ItemColumns::resolve(&stmt)?;
        let rows = stmt.query_map([cutoff_ms], |row| row_to_item(row, &columns))?;
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
    let columns = ItemColumns::resolve(&stmt)?;
    stmt.query_row(params![content_hash, since_ms], |row| {
        row_to_item(row, &columns)
    })
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
    app_bundle_id: &Option<String>,
    app_name: &Option<String>,
) -> rusqlite::Result<StoredItem> {
    if created_at <= existing.created_at {
        return Ok(existing.clone());
    }
    match tx.execute(
        "UPDATE clipboard_items \
         SET created_at = ?2, app_bundle_id = ?3, app_name = ?4 \
         WHERE id = ?1 AND deleted = 0",
        params![&existing.id, created_at, app_bundle_id, app_name],
    ) {
        Ok(_) => Ok(StoredItem {
            created_at,
            app_bundle_id: app_bundle_id.clone(),
            app_name: app_name.clone(),
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
    let columns = ItemColumns::resolve(&stmt)?;
    stmt.query_row(
        params![content_hash, DEDUP_BUCKET_MS, created_at / DEDUP_BUCKET_MS],
        |row| row_to_item(row, &columns),
    )
    .optional()
}

/// Hard-deletes rows and their FTS entries in the caller's transaction, so the
/// index can never drift from the table.
fn hard_delete_in_tx(tx: &Transaction<'_>, ids: &[String]) -> rusqlite::Result<u64> {
    if ids.is_empty() {
        return Ok(0);
    }
    // By rowid, and while the item row still holds it. Keyed by `id` this was
    // one full scan of the plaintext index per victim, so an eviction was
    // quadratic in history and held the write lock for all of it.
    let mut del_fts = tx.prepare(
        "DELETE FROM clipboard_fts \
          WHERE rowid = (SELECT fts_rowid FROM clipboard_items WHERE id = ?1)",
    )?;
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
pub use copypaste_p2p::protocol::plaintext_content_hash as compute_content_hash;

#[cfg(test)]
mod tests {
    use super::super::test_support::{fts_row_count, hash_of, item, plan_of, store, T0};
    use super::*;

    /// Every sweep query must reach its index. `INDEXED BY` makes a mismatched
    /// partial predicate an error rather than a silent scan, so these assert the
    /// half `INDEXED BY` cannot: that the index supplies the order too, and that
    /// the byte scan never leaves the index to weigh a payload.
    #[test]
    fn every_eviction_query_seeks_its_index_and_needs_no_sort() {
        let s = store();
        s.insert(item("payload", T0)).unwrap();

        for (label, sql) in [
            ("keep newest", KEEP_NEWEST_SQL),
            ("cap victims", CAP_VICTIMS_SQL),
            ("byte victims", BYTE_VICTIMS_SQL),
            ("age gate", AGE_GATE_SQL),
            ("age victims", AGE_VICTIMS_SQL),
            ("byte victim probe", BYTE_CAP_HAS_VICTIM_SQL),
        ] {
            let plan = plan_of(&s, sql);
            assert!(
                plan.iter().any(|d| d.contains("idx_items_evictable")),
                "{label} must use idx_items_evictable, got {plan:?}"
            );
            assert!(
                !plan.iter().any(|d| d.contains("TEMP B-TREE")),
                "{label} must take its order from the index, got {plan:?}"
            );
        }
    }

    #[test]
    fn both_sensitive_queries_use_the_partial_wipe_index() {
        let s = store();
        s.insert(item("payload", T0)).unwrap();
        for (label, sql) in [
            ("probe", SENSITIVE_EXISTS_SQL),
            ("sweep", EXPIRED_SENSITIVE_SQL),
        ] {
            let plan = plan_of(&s, sql);
            assert!(
                plan.iter().any(|d| d.contains("idx_items_sensitive_wipe")),
                "the sensitive {label} must use its partial index, got {plan:?}"
            );
            assert!(
                !plan.iter().any(|d| d.contains("TEMP B-TREE")),
                "the sensitive {label} must not sort, got {plan:?}"
            );
        }
    }

    /// One item bigger than the whole quota is permanently over it, and it is
    /// also the protected newest row, so nothing may be deleted. That state used
    /// to re-rank the entire history on every capture, for ever.
    #[test]
    fn a_lone_oversized_item_is_kept_and_stops_the_rescan() {
        let s = store();
        let only = s.insert(item(&"a".repeat(100), T0)).unwrap();

        for _ in 0..3 {
            assert_eq!(s.evict_over_byte_cap(10).unwrap(), 0);
        }
        assert!(s.get(&only.id).unwrap().is_some(), "the sole row is kept");

        // A second row makes the first evictable again, and the sweep resumes.
        s.insert(item(&"b".repeat(100), T0 + 60_000)).unwrap();
        assert_eq!(s.evict_over_byte_cap(10).unwrap(), 1);
        assert!(s.get(&only.id).unwrap().is_none());
    }

    /// A sweep with nothing to do must not take the write lock. Held open by
    /// another connection, an IMMEDIATE transaction it does not need is the
    /// difference between returning and waiting out `busy_timeout` — and an
    /// import ran three of them per item.
    ///
    /// On a file store, so the WAL writer lock is the real one — a shared-cache
    /// in-memory database locks per table and would prove something else.
    #[test]
    fn a_sweep_with_nothing_to_do_never_takes_the_write_lock() {
        let dir = tempfile::TempDir::new().unwrap();
        let s = Store::open(
            &dir.path().join("copypaste-v2.db"),
            &super::super::test_support::KEY,
        )
        .unwrap();
        s.insert(item("only", T0)).unwrap();

        let mut blocker = s.conn().unwrap();
        let held = super::write_tx(&mut blocker).unwrap();
        held.execute("UPDATE clipboard_live_count SET live = live", [])
            .unwrap();

        // Each of these would otherwise open its own IMMEDIATE transaction,
        // wait out `busy_timeout`, and fail.
        assert_eq!(s.evict_over_cap(1_000).unwrap(), 0);
        assert_eq!(s.evict_over_byte_cap(u64::MAX).unwrap(), 0);
        assert_eq!(s.evict_older_than(T0 - 1).unwrap(), 0);

        held.rollback().unwrap();
    }

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
    fn the_live_count_tracks_every_liveness_change() {
        let s = store();
        assert_counter_matches(&s, "empty store");

        let ids: Vec<String> = (0..4)
            .map(|n| {
                s.insert(item(&format!("item {n}"), T0 + n * 60_000))
                    .unwrap()
                    .id
            })
            .collect();
        assert_counter_matches(&s, "inserts");

        s.insert_or_bump(item("item 0", T0 + 7 * 86_400_000))
            .unwrap();
        assert_counter_matches(&s, "dedup bump");

        assert!(s.delete(&ids[1]).unwrap());
        assert!(!s.delete(&ids[1]).unwrap());
        assert_counter_matches(&s, "soft delete");

        assert!(s.set_pinned(&ids[0], true).unwrap());
        assert_eq!(s.evict_over_cap(2).unwrap(), 1);
        assert_counter_matches(&s, "cap purge");

        s.delete_all().unwrap();
        assert_eq!(s.count().unwrap(), 1, "the pinned item survives");
        assert_counter_matches(&s, "bulk delete");

        s.insert(item("after the sweep", T0 + 8 * 86_400_000))
            .unwrap();
        assert_counter_matches(&s, "insert after purge");
    }

    #[test]
    fn the_history_cap_gate_reads_only_the_counter() {
        let s = store();
        s.insert(item("payload", T0)).unwrap();
        let conn = s.conn().unwrap();
        let plan = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {LIVE_COUNT_SQL}"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            !plan.iter().any(|detail| detail.contains("clipboard_items")),
            "the cap gate must not read the items table: {plan:?}"
        );
    }

    #[track_caller]
    fn assert_counter_matches(s: &Store, after: &str) {
        let scanned: i64 = s
            .conn()
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(s.count().unwrap(), scanned as u64, "drift after {after}");
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
    fn byte_eviction_removes_oldest_unpinned_ciphertexts_and_their_fts_rows() {
        let s = store();
        let oldest = s.insert(item(&"a".repeat(10), T0)).unwrap();
        let middle = s.insert(item(&"b".repeat(20), T0 + 60_000)).unwrap();
        let newest = s.insert(item(&"c".repeat(30), T0 + 120_000)).unwrap();

        assert_eq!(s.evict_over_byte_cap(45).unwrap(), 2);
        assert!(s.get(&oldest.id).unwrap().is_none());
        assert!(s.get(&middle.id).unwrap().is_none());
        assert!(s.get(&newest.id).unwrap().is_some());
        assert_eq!(fts_row_count(&s, &oldest.id), 0);
        assert_eq!(fts_row_count(&s, &middle.id), 0);
        assert_eq!(fts_row_count(&s, &newest.id), 1);
    }

    #[test]
    fn byte_eviction_keeps_pins_and_the_newest_unpinned_item_over_quota() {
        let s = store();
        let pinned = s.insert(item(&"a".repeat(10), T0)).unwrap();
        let old = s.insert(item(&"b".repeat(20), T0 + 60_000)).unwrap();
        let newest = s.insert(item(&"c".repeat(30), T0 + 120_000)).unwrap();
        assert!(s.set_pinned(&pinned.id, true).unwrap());

        assert_eq!(s.evict_over_byte_cap(40).unwrap(), 1);
        assert!(s.get(&pinned.id).unwrap().is_some());
        assert!(s.get(&old.id).unwrap().is_none());
        assert!(s.get(&newest.id).unwrap().is_some());

        assert_eq!(s.evict_over_byte_cap(1).unwrap(), 0);
        assert!(s.get(&pinned.id).unwrap().is_some());
        assert!(s.get(&newest.id).unwrap().is_some());
    }

    #[test]
    fn byte_cap_gate_uses_the_unpinned_bytes_index() {
        let s = store();
        s.insert(item("payload", T0)).unwrap();
        let conn = s.conn().unwrap();
        let plan = conn
            .prepare(&format!("EXPLAIN QUERY PLAN {BYTE_CAP_TOTAL_SQL}"))
            .unwrap()
            .query_map([], |row| row.get::<_, String>(3))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();
        assert!(
            plan.iter()
                .any(|detail| detail.contains("idx_items_unpinned_bytes")),
            "byte quota gate must use its expression index, got {plan:?}"
        );
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
