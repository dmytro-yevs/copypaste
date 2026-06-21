use super::super::db::Database;
use super::types::{now_ms_epoch, ItemsError};
use rusqlite::{params, OptionalExtension};

/// Pin an item so it is never auto-deleted by TTL or history-limit prunes.
///
/// Sets `pinned = 1`, clears `expires_at`, and assigns `pin_order` to
/// `MAX(pin_order) + 1` among currently-pinned rows so the newly-pinned item
/// lands at the **end** of the pinned section. This is done atomically in a
/// single UPDATE + subquery — no separate SELECT is needed.
pub fn pin_item(db: &Database, id: &str) -> Result<(), ItemsError> {
    // Bump lamport_ts so the pin change wins LWW merge on every peer that
    // already holds this item (same pattern as soft_delete_item).
    // wall_time is refreshed to now (ms since UNIX epoch) so peers can also
    // converge on wall-clock order when lamport clocks are tied.
    //
    // CopyPaste-ojhe: the new lamport is MAX(lamport_ts + 1, now_ms) — the same
    // unified value space `next_lamport_ts` produces — so a pin can overtake a
    // stale `now_ms`-magnitude recopy of the same item instead of staying a
    // small counter value that lamport-only LWW would discard. `now_ms` is bound
    // as a parameter (rather than strftime) so it equals the wall_time we stamp,
    // keeping the two clocks consistent for the LWW tie-break.
    let now_ms = now_ms_epoch();
    db.conn().execute(
        "UPDATE clipboard_items
         SET pinned = 1,
             expires_at = NULL,
             pin_order = (
                 SELECT COALESCE(MAX(pin_order), 0) + 1
                 FROM clipboard_items
                 WHERE pinned = 1
             ),
             lamport_ts = MAX(
                 (SELECT lamport_ts + 1 FROM clipboard_items WHERE id = ?1),
                 ?2
             ),
             wall_time = ?2
         WHERE id = ?1",
        rusqlite::params![id, now_ms],
    )?;
    Ok(())
}

/// Unpin a previously pinned item, restoring normal TTL and history-limit
/// behaviour. Sets `pinned = 0` and clears `pin_order` back to NULL;
/// `expires_at` remains `NULL` unless the caller explicitly sets a new expiry.
pub fn unpin_item(db: &Database, id: &str) -> Result<(), ItemsError> {
    // Bump lamport_ts so the unpin change wins LWW merge on every peer that
    // already holds this item (same pattern as soft_delete_item / pin_item).
    // CopyPaste-ojhe: MAX(lamport_ts + 1, now_ms) keeps the unified value space.
    let now_ms = now_ms_epoch();
    db.conn().execute(
        "UPDATE clipboard_items
         SET pinned = 0,
             pin_order = NULL,
             lamport_ts = MAX(
                 (SELECT lamport_ts + 1 FROM clipboard_items WHERE id = ?1),
                 ?2
             ),
             wall_time = ?2
         WHERE id = ?1",
        rusqlite::params![id, now_ms],
    )?;
    Ok(())
}

/// Reorder the pinned section by assigning consecutive `pin_order` values.
///
/// `ids` is a slice of primary-key `id` values (the per-row UUID, not
/// `item_id`) in the desired display order. Each `id` at index `i` receives
/// `pin_order = (i + 1) as f64` so the sequence starts at 1.0, 2.0, …
///
/// All updates run inside a single transaction. Non-pinned ids in the slice
/// are silently skipped (the UPDATE touches only rows where `pinned = 1`).
/// Unknown ids produce a no-op row-count of 0 and are not treated as errors,
/// matching the "idempotent reorder" contract.
///
/// Returns the number of rows whose `pin_order` was actually changed.
pub fn reorder_pinned(db: &Database, ids: &[&str]) -> Result<usize, ItemsError> {
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let mut changed = 0usize;
    let now_ms = now_ms_epoch();
    for (i, id) in ids.iter().enumerate() {
        let order = (i + 1) as f64;
        // Bump lamport_ts so the reorder wins LWW merge on every peer —
        // same pattern as pin_item / unpin_item / soft_delete_item.
        // CopyPaste-ojhe: MAX(lamport_ts + 1, now_ms) keeps the unified space.
        let rows = tx.execute(
            "UPDATE clipboard_items
             SET pin_order = ?1,
                 lamport_ts = MAX(
                     (SELECT lamport_ts + 1 FROM clipboard_items WHERE id = ?2),
                     ?3
                 ),
                 wall_time = ?3
             WHERE id = ?2 AND pinned = 1",
            rusqlite::params![order, id, now_ms],
        )?;
        changed += rows;
    }
    tx.commit()?;
    Ok(changed)
}

/// Set (or clear) the encrypted thumbnail blob for an item by primary-key `id`.
///
/// Used for lazy backfill: an image row captured before the thumbnail pipeline
/// existed (or downloaded via sync without a thumbnail) can have its `thumb`
/// column populated after the fact once a thumbnail is generated. Passing
/// `None` clears the column back to SQL NULL.
///
/// **Security (CopyPaste-44rq.49):** when `blob` is `Some(_)` this function
/// first reads `is_sensitive` for the target row inside the same connection.
/// If the row is sensitive the write is silently suppressed (returns `0`) so
/// the backfill path cannot be used to sneak a thumbnail in after a
/// sensitivity upgrade. A `None` (clear) is always allowed — clearing a
/// thumbnail is never harmful.
///
/// Returns the number of rows updated (`0` when no row matches `id` or when
/// the row is sensitive and a non-None blob was suppressed).
pub fn set_thumb(db: &Database, id: &str, blob: Option<&[u8]>) -> Result<usize, ItemsError> {
    // CopyPaste-44rq.49: suppress backfill for sensitive items.
    // Clearing (blob = None) is safe and always allowed; only non-None blobs
    // are gated so a future caller can still NULL-out a thumb on a sensitive row.
    if blob.is_some() {
        let is_sensitive: Option<i64> = db
            .conn()
            .query_row(
                "SELECT is_sensitive FROM clipboard_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .optional()?;
        // Row not found → 0 rows updated (same semantics as before).
        // Row is sensitive → suppress write, return 0.
        match is_sensitive {
            None => return Ok(0),
            Some(v) if v != 0 => return Ok(0),
            _ => {}
        }
    }
    let changed = db.conn().execute(
        "UPDATE clipboard_items SET thumb = ?1 WHERE id = ?2",
        params![blob, id],
    )?;
    Ok(changed)
}

/// Atomically mark an item as sensitive and remove any existing FTS entry.
///
/// Called when a post-insert sensitivity classification determines that an
/// item previously stored as `is_sensitive = 0` should now be treated as
/// sensitive. The two writes — `UPDATE clipboard_items SET is_sensitive = 1`
/// and `DELETE FROM clipboard_fts WHERE id = ?` — are wrapped in a single
/// `unchecked_transaction` so there is no window where the FTS row survives
/// a newly-sensitive item.
///
/// **Security (CopyPaste-44rq.45):** without this function the only way to
/// transition sensitivity was to re-insert the item, leaving a stale FTS row
/// that `search_items` would later suppress via its `AND ci.is_sensitive = 0`
/// filter. That filter is defense-in-depth; this function is the primary
/// enforcement layer for the transition-to-sensitive path.
///
/// Returns the number of `clipboard_items` rows updated (0 if `id` not found,
/// 1 on success). The FTS delete always runs inside the same transaction and
/// is not counted separately (a missing FTS row is not an error).
pub fn mark_sensitive(db: &Database, id: &str) -> Result<usize, ItemsError> {
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let changed = tx.execute(
        "UPDATE clipboard_items SET is_sensitive = 1 WHERE id = ?1",
        params![id],
    )?;
    // Always purge the FTS entry even if is_sensitive was already 1 —
    // an earlier partial failure could have left a stale FTS row.
    tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    tx.commit()?;
    Ok(changed)
}
