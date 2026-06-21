use super::super::db::Database;
use super::super::pool::DbRead;
use super::types::{row_to_item, ClipboardItem, ItemsError};
use crate::crypto::encrypt::{decrypt_item_by_version, NONCE_SIZE};
use rusqlite::params;

pub fn get_page<D: DbRead + ?Sized>(
    db: &D,
    limit: usize,
    offset: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    // Fix 6: clamp before cast — a usize > i64::MAX would wrap negative, turning
    // LIMIT into "no limit" in SQLite (negative LIMIT means unlimited rows).
    let limit_i64 = limit.min(i64::MAX as usize) as i64;
    let offset_i64 = offset.min(i64::MAX as usize) as i64;
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted
         FROM clipboard_items WHERE deleted = 0 ORDER BY wall_time DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit_i64, offset_i64], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// Pinned-first variant of [`get_page`] for the history UI.
///
/// Returns items ordered by `pinned DESC, wall_time DESC` so pinned items
/// always appear at the top of the list regardless of when they were captured.
/// Within each group (pinned / unpinned) items are sorted newest-first.
/// Respects the same `limit`/`offset` semantics as [`get_page`] and is
/// capped to `MAX_PAGE` by the IPC layer before being called.
///
/// This is the function used by the `history_page` IPC verb. [`get_page`]
/// is kept for callers that need a pure-recency order (e.g. tests, sync).
pub fn get_page_pinned_first<D: DbRead + ?Sized>(
    db: &D,
    limit: usize,
    offset: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    // Fix 6: clamp before cast to avoid negative LIMIT/OFFSET in SQLite.
    let limit_i64 = limit.min(i64::MAX as usize) as i64;
    let offset_i64 = offset.min(i64::MAX as usize) as i64;
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted
         FROM clipboard_items
         WHERE deleted = 0
         ORDER BY
           CASE WHEN pinned = 1 THEN 0 ELSE 1 END ASC,
           pin_order IS NULL ASC,
           pin_order ASC,
           wall_time DESC
         LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit_i64, offset_i64], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// Lamport-ordered history page (pinned-first, unpinned by Lamport clock).
///
/// Returns items ordered by:
///   * Pinned items first (sorted by `pin_order`, then `pin_order IS NULL`).
///   * Unpinned items sorted by `lamport_ts DESC, wall_time DESC, origin_device_id ASC`.
///
/// Using `lamport_ts` as the primary ordering key for unpinned items provides
/// causal ordering that is correct after cross-device sync: a lamport clock
/// advances monotonically on every write/merge, so after sync the ordering
/// matches causal history rather than wall-clock skew between devices.
/// `wall_time` is the secondary key (tie-break for items with equal lamport
/// values, e.g. a batch import). `origin_device_id` is the final deterministic
/// tie-break so the sort is stable across devices.
///
/// This variant is intended for the Android FFI (PG-19 / CopyPaste-o0t3) and
/// any future caller that needs causally-correct ordering. The existing
/// [`get_page_pinned_first`] (wall-time order) is preserved for the daemon IPC
/// path to avoid a behaviour change for existing macOS users.
pub fn get_page_pinned_first_lamport<D: DbRead + ?Sized>(
    db: &D,
    limit: usize,
    offset: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    // Fix 6: clamp before cast to avoid negative LIMIT/OFFSET in SQLite.
    let limit_i64 = limit.min(i64::MAX as usize) as i64;
    let offset_i64 = offset.min(i64::MAX as usize) as i64;
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted
         FROM clipboard_items
         WHERE deleted = 0
         ORDER BY
           CASE WHEN pinned = 1 THEN 0 ELSE 1 END ASC,
           pin_order IS NULL ASC,
           pin_order ASC,
           lamport_ts DESC,
           wall_time DESC,
           origin_device_id ASC
         LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit_i64, offset_i64], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// Bump an existing item's recency fields to `now_ms` without changing its
/// content, sensitive-flag, or any other metadata.
///
/// Used by the dedup path in the clipboard capture pipeline: when the same
/// plaintext is copied again, the existing row is promoted to the top of the
/// history list rather than a duplicate row being inserted.
///
/// Specifically updates:
///   * `wall_time` — sets the visible recency to `now_ms`.
///   * `lamport_ts` — set to `new_lamport` so the sync layer recognises this
///     as a newer item than its previous version.
///   * `expires_at` — **recomputed** for sensitive items: if `sensitive_ttl_ms`
///     is `Some(t)` and the row has `is_sensitive = 1`, `expires_at` is updated
///     to `now_ms + t` so the new expiry tracks the re-copy, not the original
///     capture time. (CopyPaste-89ib: the stale `expires_at` would fire after the
///     bump and wipe a freshly-recopied sensitive item immediately.)
///     Non-sensitive items are unaffected; their `expires_at` is left unchanged.
///
/// Returns the number of rows actually updated (`0` if `id` does not exist).
pub fn bump_item_recency(
    db: &Database,
    id: &str,
    now_ms: i64,
    new_lamport: i64,
    sensitive_ttl_ms: Option<i64>,
) -> Result<usize, ItemsError> {
    // CopyPaste-89ib: recompute expires_at when a sensitive item is re-copied.
    // Sensitive items carry a fixed TTL relative to their most-recent wall_time,
    // so advancing wall_time without also advancing expires_at causes the stale
    // deadline to trigger immediately — wiping content the user just re-copied.
    if let Some(ttl) = sensitive_ttl_ms {
        let changed = db.conn().execute(
            "UPDATE clipboard_items
             SET wall_time = ?1,
                 lamport_ts = ?2,
                 expires_at = CASE WHEN is_sensitive = 1 THEN ?1 + ?4 ELSE expires_at END
             WHERE id = ?3",
            params![now_ms, new_lamport, id, ttl],
        )?;
        return Ok(changed);
    }
    let changed = db.conn().execute(
        "UPDATE clipboard_items SET wall_time = ?1, lamport_ts = ?2 WHERE id = ?3",
        params![now_ms, new_lamport, id],
    )?;
    Ok(changed)
}

/// List-view variant of [`get_page`] that omits the `content` blob.
///
/// Returns the same `ClipboardItem` shape but with `content = None`. Used by
/// the UI history list, which renders previews from `blob_ref` / type / hash
/// and only needs the ciphertext blob when the user actually pastes an item.
/// For image rows the blob can be hundreds of KB; skipping the SELECT shaves
/// substantial bytes off every history-page round trip.
///
/// SQL emits `NULL` in the `content` column so the existing `row_to_item`
/// mapper still works — only the read side changes, callers do not need a
/// new type.
pub fn get_page_meta(
    db: &Database,
    limit: usize,
    offset: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    // Fix 6: clamp before cast to avoid negative LIMIT/OFFSET in SQLite.
    let limit_i64 = limit.min(i64::MAX as usize) as i64;
    let offset_i64 = offset.min(i64::MAX as usize) as i64;
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, NULL AS content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted
         FROM clipboard_items WHERE deleted = 0 ORDER BY wall_time DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit_i64, offset_i64], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// Outcome of a graceful, decrypt-while-loading page fetch ([`decrypt_page`]).
///
/// `items` holds only the rows whose ciphertext successfully verified and
/// decrypted under the supplied keys (paired with their recovered plaintext).
/// `skipped` is the count of rows that failed AEAD verification or carried an
/// unsupported `key_version` and were therefore quarantined out of the result.
#[derive(Debug)]
pub struct DecryptedPage {
    /// Successfully-decrypted rows, each paired with its recovered plaintext.
    pub items: Vec<(ClipboardItem, Vec<u8>)>,
    /// Number of rows skipped because they could not be decrypted (wrong /
    /// rotated key, format drift, or an unsupported `key_version`).
    pub skipped: usize,
}

/// Load a page of clipboard items and decrypt each one, **degrading
/// gracefully** when an individual row cannot be decrypted.
///
/// # Why this exists (CopyPaste-00zz)
///
/// The startup item-load path used to treat every undecryptable legacy row as
/// a hard per-item error. After a key rotation / pairing change, hundreds of
/// old rows (encrypted under a now-mismatched key/format) each surfaced a
/// `DecryptionFailed` error — ~629 individual failures on a single launch —
/// spamming the log and degrading UX even though the items are simply dead
/// legacy ciphertext.
///
/// This function loads the page, then for each row attempts
/// [`decrypt_item_by_version`] (which keeps the AAD binding of
/// `(item_id, schema_version, key_version)` fully intact). A row that fails AEAD
/// verification — or whose `content` / `content_nonce` is missing or malformed,
/// or whose `key_version` is unknown — is **skipped, not surfaced and not
/// fatal**: it is counted in [`DecryptedPage::skipped`] so the caller can log a
/// single aggregate line ("skipped N undecryptable legacy items") instead of
/// one error per row.
///
/// # Security
///
/// "Graceful" means *skip*, never *bypass*. A failed auth tag is never accepted
/// as valid plaintext — the row is dropped from the result entirely. The AAD
/// binding is unchanged (it is computed inside `decrypt_item_by_version` from
/// the row's own `item_id` + `key_version`), so this path cannot be used to
/// swap or replay ciphertext across items.
///
/// Tombstone / blob rows (`content` is `None`, e.g. soft-deleted rows, image /
/// file rows whose nonces live per-chunk) carry no item-level
/// `(content, content_nonce)` pair and are counted as skipped here — this helper
/// is for the text-item list path; richer blob decoding stays with the
/// dedicated image/file decoders.
pub fn decrypt_page<D: DbRead + ?Sized>(
    db: &D,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
    limit: usize,
    offset: usize,
) -> Result<DecryptedPage, ItemsError> {
    let rows = get_page(db, limit, offset)?;
    let mut items = Vec::with_capacity(rows.len());
    let mut skipped = 0usize;
    for row in rows {
        match try_decrypt_row(&row, v1_key, v2_key) {
            Some(plaintext) => items.push((row, plaintext)),
            None => skipped = skipped.saturating_add(1),
        }
    }
    Ok(DecryptedPage { items, skipped })
}

/// Attempt to recover the plaintext of a single text row, returning `None`
/// (rather than erroring) on any failure so callers can skip-and-count.
///
/// Returns `None` when the row lacks an item-level `(content, content_nonce)`
/// pair (tombstone / blob row), when the nonce is the wrong length, or when the
/// AEAD auth tag does not verify (wrong / rotated key, format drift, unsupported
/// `key_version`). A failed auth tag is NEVER treated as success — the only
/// `Some` path is a fully-verified decrypt.
fn try_decrypt_row(row: &ClipboardItem, v1_key: &[u8; 32], v2_key: &[u8; 32]) -> Option<Vec<u8>> {
    let content = row.content.as_deref()?;
    let nonce_slice = row.content_nonce.as_deref()?;
    // A malformed nonce can never decrypt; skip rather than panic on the cast.
    let nonce: [u8; NONCE_SIZE] = nonce_slice.try_into().ok()?;
    decrypt_item_by_version(
        row.key_version,
        v1_key,
        v2_key,
        &row.item_id,
        &nonce,
        content,
    )
    .ok()
}

/// Fetch a single clipboard item by its primary-key `id`.
///
/// Returns `Ok(None)` when no row matches. Used by IPC verbs such as
/// `copy_item` that resolve an item directly by id — this avoids the
/// data-loss footgun of paging (`get_page`) and linear-scanning, which
/// silently misses any item beyond the fetched page window.
pub fn get_item_by_id<D: DbRead + ?Sized>(
    db: &D,
    id: &str,
) -> Result<Option<ClipboardItem>, ItemsError> {
    let result = db.conn().query_row(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted
         FROM clipboard_items WHERE id = ?1",
        params![id],
        row_to_item,
    );
    match result {
        Ok(item) => Ok(Some(item)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        // row_to_item emits IntegralValueOutOfRange(14, v) when key_version
        // does not fit in u8 — re-surface as the typed CorruptKeyVersion
        // variant so callers can distinguish schema corruption from other
        // SQLite errors.
        Err(rusqlite::Error::IntegralValueOutOfRange(14, v)) => {
            Err(ItemsError::CorruptKeyVersion(v))
        }
        Err(e) => Err(ItemsError::Sqlite(e)),
    }
}

/// Fetch a single clipboard item by its **cross-device** `item_id`.
///
/// `item_id` is the stable logical identity of an item across devices (it is
/// bound into the AEAD AAD and carries the `idx_clipboard_item_id` UNIQUE
/// index). The sync/merge layer resolves an incoming peer item against the
/// local row by `item_id` — NOT by the per-row primary key `id`, which is a
/// fresh `Uuid::new_v4()` on every device and so differs for the same logical
/// item. Returns `Ok(None)` when no row matches.
pub fn get_item_by_item_id(
    db: &Database,
    item_id: &str,
) -> Result<Option<ClipboardItem>, ItemsError> {
    // NOTE: no `deleted = 0` filter here — the merge layer must be able to see
    // tombstone rows (deleted = 1) so LWW can determine whether an incoming
    // remote version beats the local tombstone or vice-versa.
    let result = db.conn().query_row(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted
         FROM clipboard_items WHERE item_id = ?1",
        params![item_id],
        row_to_item,
    );
    match result {
        Ok(item) => Ok(Some(item)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(ItemsError::Sqlite(e)),
    }
}

/// Return `true` when a row with the given cross-device `item_id` already
/// exists locally. Used by the sync/cloud dedup path to decide between an
/// LWW resolve+replace (item already known) and a fresh insert.
pub fn exists_item_by_item_id(db: &Database, item_id: &str) -> Result<bool, ItemsError> {
    let count: i64 = db.conn().query_row(
        "SELECT COUNT(1) FROM clipboard_items WHERE item_id = ?1",
        params![item_id],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

pub fn count_items<D: DbRead + ?Sized>(db: &D) -> Result<i64, ItemsError> {
    Ok(db.conn().query_row(
        "SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0",
        [],
        |r| r.get(0),
    )?)
}

/// Find the id of an item with the given content hash stored within the last
/// `within_ms` milliseconds. Returns `None` if no such item exists.
///
/// Used by the daemon to skip inserting duplicate clipboard content.
pub fn find_recent_by_hash(
    db: &Database,
    hash: &str,
    now_ms: i64,
    within_ms: i64,
) -> Result<Option<String>, ItemsError> {
    // Use saturating_sub for consistency with `delete_sensitive_expired` and to
    // avoid a debug-mode panic when now_ms < within_ms (e.g. now_ms=0, within_ms=i64::MAX).
    let cutoff = now_ms.saturating_sub(within_ms);
    let result = db.conn().query_row(
        "SELECT id FROM clipboard_items
         WHERE content_hash = ?1 AND wall_time >= ?2
         ORDER BY wall_time DESC LIMIT 1",
        params![hash, cutoff],
        |row| row.get::<_, String>(0),
    );
    match result {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(ItemsError::Sqlite(e)),
    }
}
