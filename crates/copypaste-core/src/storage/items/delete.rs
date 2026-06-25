use super::super::db::Database;
use super::types::ItemsError;
use rusqlite::{params, OptionalExtension};

/// CopyPaste-6fd: defensively delete any `pending_uploads` rows whose
/// cross-device `item_id` belongs to clipboard rows that are about to be hard
/// deleted, identified by their primary-key `id`s.
///
/// `pending_uploads` has no `ON DELETE CASCADE` foreign key (and even if it did,
/// `PRAGMA foreign_keys` is connection-scoped and easy to miss on a fresh
/// connection — see `CONNECTION_PRAGMAS` in `db.rs`). Every hard-delete /
/// prune / evict path therefore calls this inside its own transaction so a
/// dropped clipboard item can never strand a resumable-upload row. The DELETE
/// resolves `item_id` from `clipboard_items` while those rows still exist, so it
/// MUST run before the corresponding `clipboard_items` delete.
///
/// No-op when `ids` is empty.
pub(super) fn delete_pending_uploads_for_ids(
    tx: &rusqlite::Transaction<'_>,
    ids: &[String],
) -> Result<(), ItemsError> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "DELETE FROM pending_uploads WHERE item_id IN \
         (SELECT item_id FROM clipboard_items WHERE id IN ({placeholders}))"
    );
    tx.execute(&sql, rusqlite::params_from_iter(ids.iter()))?;
    Ok(())
}

/// CopyPaste-c1dd: delete the FTS5 rows for `ids` in a SINGLE
/// `DELETE FROM clipboard_fts WHERE id IN (...)` statement instead of one
/// `tx.execute(... WHERE id = ?)` round-trip per id (an N+1 pattern in
/// `delete_expired` / `delete_sensitive_expired` / `prune_to_cap`).
///
/// All ids are already materialised by the callers before the delete, so a
/// single batched statement is a pure win with identical semantics. No-op when
/// `ids` is empty.
pub(super) fn delete_fts_for_ids(
    tx: &rusqlite::Transaction<'_>,
    ids: &[String],
) -> Result<(), ItemsError> {
    if ids.is_empty() {
        return Ok(());
    }
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("DELETE FROM clipboard_fts WHERE id IN ({placeholders})");
    tx.execute(&sql, rusqlite::params_from_iter(ids.iter()))?;
    Ok(())
}

pub fn delete_expired(db: &Database, now_ms: i64) -> Result<usize, ItemsError> {
    // Fix 4: delete matching FTS rows in the same transaction so no orphan FTS
    // entries accumulate when items are TTL-pruned.
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    // Collect ids before deleting so we can prune FTS in the same tx.
    let mut stmt = tx.prepare(
        "SELECT id FROM clipboard_items WHERE expires_at IS NOT NULL AND expires_at < ?1 AND pinned = 0",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![now_ms], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    // CopyPaste-6fd: clean pending_uploads BEFORE the items delete (it resolves
    // item_id from the rows that are about to vanish).
    delete_pending_uploads_for_ids(&tx, &ids)?;
    let changed = tx.execute(
        "DELETE FROM clipboard_items WHERE expires_at IS NOT NULL AND expires_at < ?1 AND pinned = 0",
        params![now_ms],
    )?;
    // CopyPaste-c1dd: batch FTS deletes into one statement (was N+1).
    delete_fts_for_ids(&tx, &ids)?;
    tx.commit()?;
    Ok(changed)
}

/// Return `true` when there is at least one non-pinned sensitive item in the
/// database, `false` otherwise.
///
/// This is a cheap `SELECT EXISTS` probe used as a pre-flight guard by
/// `run_ttl_cleanup` (CopyPaste-98ja): when the table has no sensitive rows at
/// all there is nothing to prune, so the full `delete_sensitive_expired` scan
/// is skipped entirely.  The query touches only the `is_sensitive` + `pinned`
/// columns which are covered by the primary-key/clustered index and completes
/// in O(1) on an empty result.
///
/// # Fail-closed security guarantee (CopyPaste-ny0g)
///
/// On query error this function returns `true` (not `false`). Returning `false`
/// on error would silently suppress the TTL sweep, allowing sensitive items to
/// outlive their configured TTL indefinitely — a silent data-retention violation.
/// Returning `true` causes an unnecessary `delete_sensitive_expired` call (a
/// cheap no-op when nothing is actually expired), which is always safe. Prefer
/// false-positive over false-negative on the security gate.
pub fn has_sensitive_items(db: &Database) -> bool {
    db.conn()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM clipboard_items WHERE is_sensitive = 1 AND pinned = 0)",
            [],
            |row| row.get::<_, bool>(0),
        )
        // SECURITY: fail-closed — treat a query error as "sensitive items present"
        // so the caller always runs the TTL sweep. See doc comment above.
        .unwrap_or(true)
}

/// Delete sensitive items whose TTL has expired.
///
/// # Unified TTL path (CopyPaste-3e7y)
///
/// Previously this function used a divergent `wall_time < now_ms - ttl_ms`
/// predicate while `delete_expired` used `expires_at < now_ms`. Two separate
/// code paths with different semantics meant a sensitive item without an
/// explicit `expires_at` (e.g. inserted before the bump fix) was invisible to
/// `delete_expired` and could outlive its intended TTL if the wall_time path
/// was skipped.
///
/// The unified approach:
///   1. **Backfill** any sensitive row that lacks `expires_at` by computing it
///      from `wall_time + sensitive_ttl_ms`, clamping to avoid overflow.
///   2. **Delegate** to `delete_expired(db, now_ms)` which handles both plain
///      and sensitive items via the single `expires_at` predicate.
///
/// This makes `delete_expired` the canonical, only TTL sweep path.
///
/// Pinned items are excluded by both the backfill (`AND pinned = 0`) and by
/// `delete_expired` itself.
pub fn delete_sensitive_expired(
    db: &Database,
    now_ms: i64,
    sensitive_ttl_ms: i64,
) -> Result<usize, ItemsError> {
    // CopyPaste-44rq.62: the backfill UPDATE and the expiry DELETE must be
    // atomic. Previously the UPDATE ran in autocommit and `delete_expired`
    // opened a separate transaction; a crash between them left sensitive rows
    // with `expires_at` set but not yet deleted.  We now open a single
    // `unchecked_transaction` that covers both operations.
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;

    // Step 1: backfill expires_at for sensitive rows that pre-date the bump fix.
    // We use saturating_add to avoid overflow on pathologically large TTL values.
    // The backfill is idempotent — rows that already carry expires_at are left alone.
    tx.execute(
        "UPDATE clipboard_items
         SET expires_at = MIN(wall_time + ?1, 9223372036854775807)
         WHERE is_sensitive = 1 AND expires_at IS NULL AND pinned = 0",
        params![sensitive_ttl_ms],
    )?;

    // Step 2: unified expiry sweep — inline the delete_expired logic so that
    // the UPDATE above and the DELETE below share the same transaction.
    // Predicate: `expires_at IS NOT NULL AND expires_at < now_ms AND pinned = 0`.
    let mut stmt = tx.prepare(
        "SELECT id FROM clipboard_items WHERE expires_at IS NOT NULL AND expires_at < ?1 AND pinned = 0",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![now_ms], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    // Clean pending_uploads BEFORE deleting the items rows (CopyPaste-6fd).
    delete_pending_uploads_for_ids(&tx, &ids)?;
    let changed = tx.execute(
        "DELETE FROM clipboard_items WHERE expires_at IS NOT NULL AND expires_at < ?1 AND pinned = 0",
        params![now_ms],
    )?;
    // Batch-delete FTS entries for all pruned ids (CopyPaste-c1dd).
    delete_fts_for_ids(&tx, &ids)?;

    tx.commit()?;
    Ok(changed)
}

/// Run `PRAGMA incremental_vacuum(pages)` to reclaim up to `pages` free SQLite
/// pages without a full blocking VACUUM.
///
/// # Why incremental vacuum (CopyPaste-kexs)
///
/// SQLite reclaims deleted-row space into its free-list but does NOT shrink the
/// database file until `VACUUM` runs.  A full `VACUUM` rebuilds the entire
/// database in a single serialised write and can hold the write lock for
/// hundreds of milliseconds on large histories — unacceptable in the clipboard
/// hot path.
///
/// `PRAGMA incremental_vacuum(N)` reclaims at most `N` free pages per call,
/// returning them to the OS in small, bounded increments.  The caller controls
/// the budget via `max_pages`:
///   * A small value (e.g. `10`) adds a few microseconds of latency each call;
///     called periodically (e.g. after every TTL sweep) it keeps the file size
///     near its minimum without ever blocking.
///   * `0` reclaims ALL free pages — equivalent to a full VACUUM but still
///     WAL-safe; use only at low-traffic moments (startup, explicit user action).
///
/// For `incremental_vacuum` to have any effect the database MUST be opened with
/// `PRAGMA auto_vacuum = INCREMENTAL` (value 2). The schema migration sets this
/// on the first open. Pre-existing databases that were opened without it keep
/// `auto_vacuum = NONE` (0) and the pragma is a no-op — harmless, but also
/// silent. Callers should not depend on a specific freed-page count.
///
/// # Returns
///
/// `Ok(())` on success (including the no-op case). `Err(ItemsError::Sqlite(_))`
/// on any SQLite error.
pub fn incremental_vacuum(db: &Database, max_pages: u32) -> Result<(), ItemsError> {
    db.conn()
        .execute_batch(&format!("PRAGMA incremental_vacuum({max_pages});"))?;
    Ok(())
}

/// Delete the clipboard item with the given primary-key `id`.
///
/// Returns the number of rows actually removed (`0` when no row matched).
/// Callers can use this to distinguish a real deletion from a no-op against a
/// non-existent id.
///
/// Fix 4: also removes the matching `clipboard_fts` row in the same transaction
/// so callers (daemon prune-by-id paths) don't need to call `delete_fts` separately.
pub fn delete_item(db: &Database, id: &str) -> Result<usize, ItemsError> {
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    // CopyPaste-6fd: clean any resumable-upload row for this item BEFORE the
    // items delete resolves the item_id away.
    let id_owned = id.to_string();
    delete_pending_uploads_for_ids(&tx, std::slice::from_ref(&id_owned))?;
    let removed = tx.execute("DELETE FROM clipboard_items WHERE id=?1", params![id])?;
    tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    tx.commit()?;
    Ok(removed)
}

/// Soft-delete an item: wipe its content/nonce/thumb blobs, set `deleted = 1`,
/// and stamp the supplied `lamport_ts` / `wall_time` so the resulting tombstone
/// wins LWW resolution on every peer it reaches.
///
/// Unlike [`delete_item`] (hard DELETE), the row is **kept** in the table as a
/// tombstone so:
///   1. The sync layer can broadcast it as a deletion event.
///   2. An inbound delete from another device cannot resurrect the item (the
///      tombstone absorbs the re-insert via LWW).
///
/// Also removes the FTS entry so tombstones are never returned by search.
///
/// Returns the number of rows modified (0 means the id was not found).
pub fn soft_delete_item(
    db: &Database,
    id: &str,
    lamport_ts: i64,
    wall_time: i64,
) -> Result<usize, ItemsError> {
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let changed = soft_delete_item_in_tx(&tx, id, lamport_ts, wall_time)?;
    tx.commit()?;
    Ok(changed)
}

/// Soft-delete one item INSIDE a caller-provided transaction (CopyPaste-jvzm.3).
///
/// Identical work to [`soft_delete_item`] (tombstone: `deleted=1`,
/// `is_synced=0`, content/nonce/thumb wiped, LWW clocks bumped; plus the
/// in-flight `pending_uploads` row and the FTS index entry are cleaned in the
/// SAME transaction), but it does NOT open or commit a transaction itself. This
/// lets a batch caller (e.g. the `delete_all` IPC handler) tombstone many items
/// in ONE transaction while reusing the single canonical tombstone definition —
/// so the two paths cannot drift, and the previous O(n) "FTS orphan purge"
/// cross-table scan per call is no longer needed.
pub fn soft_delete_item_in_tx(
    tx: &rusqlite::Transaction<'_>,
    id: &str,
    lamport_ts: i64,
    wall_time: i64,
) -> Result<usize, ItemsError> {
    let changed = tx.execute(
        "UPDATE clipboard_items
         SET deleted = 1,
             is_synced = 0,
             content = NULL,
             content_nonce = NULL,
             thumb = NULL,
             lamport_ts = ?2,
             wall_time = ?3
         WHERE id = ?1",
        params![id, lamport_ts, wall_time],
    )?;
    if changed > 0 {
        // CopyPaste-bhm9: clean any in-flight resumable-upload row BEFORE the
        // item's content is gone — same defensive cleanup the hard-delete paths
        // apply (CopyPaste-6fd). Must run inside the same transaction so the
        // item_id → pending_uploads join still resolves.
        let id_owned = id.to_string();
        delete_pending_uploads_for_ids(tx, std::slice::from_ref(&id_owned))?;
        // Remove from FTS so the tombstone is not returned by search queries.
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    }
    Ok(changed)
}

/// Remove an item's entry from the FTS5 index.
///
/// CopyPaste-jvzm.5: `delete_item`, `delete_expired`, and the soft-delete paths
/// ALREADY clean the FTS index inside their own transactions — do NOT call this
/// after them (it would be a redundant no-op). This standalone helper is only
/// for callers that remove a row WITHOUT going through those functions (e.g. an
/// ad-hoc raw `DELETE FROM clipboard_items`) and must keep the FTS index in sync.
pub fn delete_fts(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn()
        .execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    Ok(())
}

/// Prune the oldest unpinned clipboard items so that the total byte size of
/// all unpinned `content` blobs does not exceed `max_bytes`.
///
/// # Eviction semantics
///
/// * **Pinned items are never evicted** — only rows with `pinned = 0` are
///   considered for deletion or counted towards the quota.
/// * **Oldest-first ordering** — rows are sorted by `(wall_time ASC, id ASC)`
///   before eviction. When two items share the same millisecond timestamp,
///   the lexicographically smaller UUID is evicted first (deterministic).
/// * **The "tipping" row is evicted** — the first row whose inclusion brings
///   the running cumulative byte total to or past the excess is deleted, not
///   kept. After the prune, remaining unpinned bytes ≤ `max_bytes`.
/// * **Images are counted** — `content` stores the encrypted blob for both
///   text and image items in the same `clipboard_items` table; `LENGTH(content)`
///   includes image bytes correctly. There is no separate image store at this
///   layer, so the quota is byte-accurate across all content types.
///
/// # Performance
///
/// Uses a single-pass SQLite window function
/// `SUM(LENGTH(COALESCE(content,''))) OVER (ORDER BY wall_time ASC, id ASC
/// ROWS UNBOUNDED PRECEDING)` to compute a running cumulative byte total in
/// O(n log n). The previous correlated-subquery approach (O(n²)) was
/// prohibitively slow on large databases after a cloud backfill batch.
///
/// SQLite ≥ 3.25 is required for window functions. The bundled SQLCipher
/// version shipping with `rusqlite = "0.32" / bundled-sqlcipher` includes
/// SQLite ≥ 3.47, which satisfies this requirement.
///
/// # Returns
///
/// The number of rows deleted (0 when the quota is already satisfied).
pub fn prune_to_cap(db: &Database, max_bytes: i64) -> Result<usize, ItemsError> {
    // Fast path: if total unpinned bytes are within the quota nothing to do.
    // This avoids constructing the window-function query on every insert when
    // the DB is well under the cap (the common case).
    //
    // CopyPaste-pvp4: the `LENGTH(COALESCE(content, ''))` expression and the
    // `WHERE pinned = 0` predicate match the partial covering index
    // `idx_clipboard_unpinned_len` (schema v11) verbatim, so SQLite serves this
    // SUM from an index-only scan — no full-table scan and no decrypted-BLOB
    // reads on every clipboard write.
    let total_unpinned: i64 = db.conn().query_row(
        "SELECT COALESCE(SUM(LENGTH(COALESCE(content, ''))), 0) \
         FROM clipboard_items WHERE pinned = 0",
        [],
        |r| r.get(0),
    )?;
    if total_unpinned <= max_bytes {
        return Ok(0);
    }

    // Compute excess = bytes that must be freed.
    // Cast is safe: total_unpinned > max_bytes >= 0, so excess > 0 and fits in i64.
    let excess = total_unpinned - max_bytes;

    // Defense-in-depth: never evict the single most-recent unpinned row in the
    // same tick that inserted it. If a fresh capture alone exceeds the cap (a
    // large image, or a mis-set sub-floor quota that the clamp somehow missed),
    // pruning would otherwise delete the row we just stored — the user copies
    // something and it instantly vanishes. Protecting the newest row guarantees
    // the just-captured item always survives; the next-oldest rows still absorb
    // the cap. Ordering matches the eviction order (wall_time ASC, id ASC), so
    // the "newest" is the max (wall_time, id) row.
    let newest_unpinned_id: Option<String> = db
        .conn()
        .query_row(
            "SELECT id FROM clipboard_items WHERE pinned = 0 \
             ORDER BY wall_time DESC, id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?;
    // Empty string is never a valid UUID id, so it is a safe "no row to keep"
    // sentinel for the `id <> ?` filters below.
    let keep_id = newest_unpinned_id.unwrap_or_default();

    // CopyPaste-yfm8: single-pass — collect eviction ids via the window CTE once,
    // then DELETE directly by the collected id list.  The previous implementation
    // computed the window CTE twice (once for SELECT, once inside the DELETE's
    // sub-SELECT), doubling the O(n log n) sort work for large tables and risking
    // a stale result if rows changed between the two executions (unlikely but
    // possible if locks were released).  Using the materialised id list for the
    // DELETE removes the second CTE entirely.
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;

    // Select the eviction prefix via a single window-function pass.
    // A row belongs to the prefix when cum_bytes - row_bytes < excess (i.e. the
    // running total BEFORE adding this row has not yet covered the excess). The
    // "tipping" row (first one whose cum_bytes meets or exceeds `excess`) is
    // also included because its pre-row total is still < excess by definition.
    let mut stmt = tx.prepare(
        "WITH ranked AS (
             SELECT
                 id,
                 LENGTH(COALESCE(content, '')) AS row_bytes,
                 SUM(LENGTH(COALESCE(content, ''))) OVER (
                     ORDER BY wall_time ASC, id ASC
                     ROWS UNBOUNDED PRECEDING
                 ) AS cum_bytes
             FROM clipboard_items
             WHERE pinned = 0 AND id <> ?2
         )
         SELECT id FROM ranked
         WHERE cum_bytes - row_bytes < ?1",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![excess, keep_id], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    if ids.is_empty() {
        return Ok(0);
    }

    // CopyPaste-6fd: clean pending_uploads for the evicted ids before the items
    // delete (resolves item_id while those rows still exist).
    delete_pending_uploads_for_ids(&tx, &ids)?;

    // DELETE by the already-materialised id list — no second CTE needed.
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let delete_sql = format!("DELETE FROM clipboard_items WHERE id IN ({placeholders})");
    let deleted = tx.execute(&delete_sql, rusqlite::params_from_iter(ids.iter()))?;

    // CopyPaste-c1dd: batch FTS deletes into one statement (was N+1).
    delete_fts_for_ids(&tx, &ids)?;
    tx.commit()?;
    Ok(deleted)
}
