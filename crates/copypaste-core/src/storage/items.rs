use super::db::{Database, DbError, MigrationState};
use super::pool::DbRead;
use rusqlite::{params, OptionalExtension};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ItemsError {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("database error: {0}")]
    Db(#[from] DbError),
    /// A write was attempted while the v4 migration sweep is in progress.
    #[error("v4 key-version migration sweep is in progress; write rejected")]
    MigrationInProgress,
    /// An item carried a `key_version` outside the supported HKDF family set {1, 2}.
    #[error("unsupported key_version {0}; expected 1 or 2")]
    UnsupportedKeyVersion(u8),
    /// The `key_version` column contains a value that does not fit in `u8`
    /// (valid range 1–255). Surfaced instead of silently truncating so callers
    /// can distinguish a corrupt/forward-compat row from a known key version.
    #[error(
        "key_version {0} is out of range (must fit in u8); row is corrupt or from a future version"
    )]
    CorruptKeyVersion(i64),
}

/// Validate that an item's `key_version` is a known HKDF family before it is
/// persisted. Rows are written verbatim from `item.key_version`, so an
/// out-of-range value would later be undecryptable. We reject rather than
/// clamp so the caller learns the value was wrong instead of silently
/// mislabelling the ciphertext's key family.
fn validate_key_version(key_version: u8) -> Result<i64, ItemsError> {
    match key_version {
        1 | 2 => Ok(key_version as i64),
        other => Err(ItemsError::UnsupportedKeyVersion(other)),
    }
}

/// Compute the next monotonic-AND-time-ordered Lamport timestamp for a local
/// mutation.
///
/// Returns `max(prev_lamport + 1, now_ms)`.
///
/// # Why one unified value space (CopyPaste-ojhe)
///
/// Before this, the daemon stamped `lamport_ts` with three colliding
/// conventions in the same `i64` field: fresh capture = `0`, recopy/promote =
/// `now_ms` (~1.75e12), and pin/delete = `existing + 1` (small counter). The
/// cloud and relay transports do bare lamport-only LWW (`remote <= local ->
/// keep`), so a stale recopy (lamport ≈ `now_ms`) permanently outranked a newer
/// pin/delete (lamport ≈ small): pins were silently overwritten and deletes
/// resurrected.
///
/// Stamping *every* write with `max(prev + 1, now_ms)` makes the field both:
///   * **monotonic** — strictly greater than the row's previous value, so a
///     newer local edit always overtakes its own prior version even if two
///     edits land within the same wall-clock millisecond; and
///   * **time-ordered** — at least `now_ms`, so the newest *writer* across
///     devices wins under lamport-first LWW (wall_time / origin_device_id only
///     break exact ties).
///
/// Backward compatibility: existing rows carry `lamport_ts = 0` and older peers
/// emit small or `now_ms`-magnitude values; a fresh `now_ms`-based write
/// deterministically dominates a stale low value and loses to a strictly-larger
/// future value, so newest-writer-wins holds without a migration.
pub fn next_lamport_ts(prev_lamport: i64, now_ms: i64) -> i64 {
    prev_lamport.saturating_add(1).max(now_ms)
}

/// Current wall-clock time in milliseconds since the Unix epoch.
///
/// Degrades to `0` (epoch) on a pathological pre-epoch clock rather than
/// panicking — matching the `unwrap_or_default()` contract used by the
/// `ClipboardItem::new_*` constructors.
fn now_ms_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug, Clone)]
pub struct ClipboardItem {
    pub id: String,
    pub item_id: String,
    pub content_type: String,
    pub content: Option<Vec<u8>>,
    pub content_nonce: Option<Vec<u8>>,
    pub blob_ref: Option<String>,
    pub is_sensitive: bool,
    pub is_synced: bool,
    pub lamport_ts: i64,
    pub wall_time: i64,
    pub expires_at: Option<i64>,
    pub app_bundle_id: Option<String>,
    /// SHA-256 hex digest of the raw (pre-encryption) content bytes.
    /// Used for deduplication: skip insert if an identical hash was stored
    /// within the last 60 seconds.
    pub content_hash: Option<String>,
    /// UUID of the device that originated this item. Used as the deterministic
    /// tie-break in the LWW merge (see `copypaste-sync::merge::resolve`).
    /// Empty string for pre-v3 rows until backfilled via
    /// [`backfill_origin_device_id`].
    pub origin_device_id: String,
    /// HKDF key generation used to encrypt this item's content. Corresponds
    /// to the `key_version` column in `clipboard_items`.
    ///   1 = v1 HKDF family (legacy, HKDF-SHA256 + static salt)
    ///   2 = v2 HKDF family (HKDF-SHA512 + per-pair salt) — used for all new rows
    /// Rows at version 1 are swept to version 2 by `migration_v4_sweep_resumable`.
    pub key_version: u8,
    /// Whether the item has been explicitly pinned by the user.
    ///
    /// Pinned items are excluded from both the TTL prune (`delete_expired`,
    /// `delete_sensitive_expired`) and the history-limit prune
    /// (`prune_history` in the daemon). Set via `pin_item`; cleared via
    /// `unpin_item`. Schema version ≥ 7 stores this as `pinned INTEGER NOT
    /// NULL DEFAULT 0` in `clipboard_items`.
    pub pinned: bool,
    /// Explicit sort key for drag-to-reorder among pinned items (schema v8+).
    ///
    /// `None` for unpinned rows (the column holds SQL NULL).  When an item is
    /// pinned via `pin_item`, `pin_order` is set to
    /// `MAX(pin_order) + 1` so newly-pinned items land at the end of the
    /// pinned section.  The UI updates this by calling `reorder_pinned` with
    /// the desired id sequence; the daemon writes consecutive integers
    /// starting at 1.
    pub pin_order: Option<f64>,
    /// Small capture-time encrypted thumbnail blob for image items (schema v9).
    ///
    /// `None` for text rows and for image rows captured before the thumbnail
    /// pipeline existed (lazily backfillable via [`set_thumb`]). When present
    /// it is the serialized encrypted chunk blob produced by
    /// `image::encode_thumbnail` / `image::encode_image_full`, keyed by a
    /// distinct `thumb_file_id` (recorded in the image `blob_ref` meta JSON).
    /// Stored in the `thumb BLOB DEFAULT NULL` column.
    pub thumb: Option<Vec<u8>>,
    /// Whether this item has been soft-deleted (schema v10).
    ///
    /// A soft-deleted item is a tombstone: its content is wiped but the row
    /// remains so the LWW sync protocol can propagate the deletion to peer
    /// devices. `false` for all freshly-captured and synced items. Set to
    /// `true` by [`soft_delete_item`], which also NULLs `content`,
    /// `content_nonce`, and `thumb`. UI list queries filter `deleted = 0`;
    /// `get_item_by_item_id` intentionally does NOT filter so the merge
    /// layer can apply tombstone wins correctly.
    pub deleted: bool,
}

impl ClipboardItem {
    /// Create a brand-new text item.
    ///
    /// NOTE: `item_id` is the **cross-device identity** of the logical item. It
    /// is bound into the AEAD AAD and carries a UNIQUE index
    /// (`idx_clipboard_item_id`, schema v5), and the sync/merge layer keys
    /// HAVE/WANT/LWW/dedup on it — NOT on `id`, which is a fresh per-row primary
    /// key. The constructor seeds `item_id` with a fresh UUID for a genuinely
    /// new capture, but when **reconstructing a known item** (cloud/P2P
    /// download, sync replay) the caller MUST overwrite `item_id` with the
    /// originating device's value and MUST NEVER regenerate it — otherwise the
    /// same logical item lands under a different identity on each device, LWW
    /// never fires, and duplicate rows accumulate.
    pub fn new_text(encrypted_content: Vec<u8>, nonce: Vec<u8>, lamport_ts: i64) -> Self {
        // `duration_since(UNIX_EPOCH)` can only fail when the system clock is set
        // before the Unix epoch (1970-01-01). That is pathological on any correctly
        // configured host. `unwrap_or_default()` degrades to `wall_time = 0` (epoch)
        // rather than panicking, so a misconfigured clock produces a mis-ordered item
        // instead of crashing the daemon.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            id: Uuid::new_v4().to_string(),
            item_id: Uuid::new_v4().to_string(),
            content_type: "text".to_string(),
            content: Some(encrypted_content),
            content_nonce: Some(nonce),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts,
            wall_time: now,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: String::new(),
            key_version: ITEM_KEY_VERSION_CURRENT as u8,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        }
    }

    /// Create an image item whose content is an encrypted chunk blob.
    ///
    /// `encrypted_blob` is produced by `copypaste_core::chunks_to_blob`.
    /// `image_meta_json` stores width/height/chunk_count/file_id as JSON in `blob_ref`.
    /// The `content_nonce` field is left `None` because XChaCha20 nonces are stored
    /// per-chunk inside the blob itself (no single item-level nonce needed).
    ///
    /// NOTE: like [`new_text`](Self::new_text), `item_id` is the cross-device
    /// identity the sync/merge/dedup layer keys on. The constructor seeds it
    /// with a fresh UUID, but a capture pipeline that can derive a stable
    /// content identity (e.g. from the image `file_id`) SHOULD overwrite
    /// `item_id` once at capture so the same image converges to one row across
    /// devices, and a reconstructed item MUST preserve the originating
    /// `item_id` rather than regenerate it.
    ///
    /// `thumb` is the optional capture-time encrypted thumbnail blob
    /// (`image::encode_image_full` produces it alongside the full chunks). Pass
    /// `None` when no thumbnail was generated; it can be backfilled later via
    /// [`set_thumb`].
    pub fn new_image(
        encrypted_blob: Vec<u8>,
        image_meta_json: String,
        lamport_ts: i64,
        thumb: Option<Vec<u8>>,
    ) -> Self {
        // Same clock-before-epoch degradation contract as `new_text`: prefer
        // `unwrap_or_default()` over `unwrap()` so a pathological host clock
        // yields `wall_time = 0` rather than a daemon panic.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            id: Uuid::new_v4().to_string(),
            item_id: Uuid::new_v4().to_string(),
            content_type: "image".to_string(),
            content: Some(encrypted_blob),
            content_nonce: None,
            blob_ref: Some(image_meta_json),
            is_sensitive: false,
            is_synced: false,
            lamport_ts,
            wall_time: now,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: String::new(),
            key_version: ITEM_KEY_VERSION_CURRENT as u8,
            pinned: false,
            pin_order: None,
            thumb,
            deleted: false,
        }
    }

    /// Create a file item whose content is an encrypted chunk blob.
    ///
    /// Identical to [`new_image`](Self::new_image) except `content_type` is
    /// `"file"` and `thumb` is always `None` (files have no inline thumbnail).
    /// `encrypted_blob` is produced by `copypaste_core::chunks_to_blob` over the
    /// chunks returned by `copypaste_core::encode_file` (raw bytes — NO
    /// decode/re-encode). `file_meta_json` stores
    /// filename/mime/original_size/chunk_count/file_id as JSON in `blob_ref`.
    /// `content_nonce` is `None` because XChaCha20 nonces live per-chunk inside
    /// the blob itself.
    ///
    /// NOTE: like [`new_image`](Self::new_image), `item_id` is the cross-device
    /// identity the sync/merge/dedup layer keys on. The constructor seeds it
    /// with a fresh UUID, but a capture pipeline that can derive a stable
    /// content identity (e.g. from the file `file_id`) SHOULD overwrite
    /// `item_id` once at capture, and a reconstructed item MUST preserve the
    /// originating `item_id` rather than regenerate it.
    pub fn new_file(encrypted_blob: Vec<u8>, file_meta_json: String, lamport_ts: i64) -> Self {
        // Same clock-before-epoch degradation contract as `new_text` /
        // `new_image`: prefer `unwrap_or_default()` over `unwrap()` so a
        // pathological host clock yields `wall_time = 0` rather than a panic.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        Self {
            id: Uuid::new_v4().to_string(),
            item_id: Uuid::new_v4().to_string(),
            content_type: "file".to_string(),
            content: Some(encrypted_blob),
            content_nonce: None,
            blob_ref: Some(file_meta_json),
            is_sensitive: false,
            is_synced: false,
            lamport_ts,
            wall_time: now,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: String::new(),
            key_version: ITEM_KEY_VERSION_CURRENT as u8,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        }
    }
}

/// Current HKDF key generation written into the `key_version` column for
/// freshly-inserted rows. Pinned here (rather than re-exported from
/// `crypto::keys`) because the storage layer needs an i64 value matching the
/// column type and the on-disk meaning is "which key/AAD format to use at
/// decrypt time" — a storage concern, not a crypto-derivation concern.
///
/// Increase from 2 → N in lockstep with a future HKDF-v3 family + a
/// corresponding migration helper in `super::migration_v4`.
pub const ITEM_KEY_VERSION_CURRENT: i64 = 2;

pub fn insert_item(db: &Database, item: &ClipboardItem) -> Result<(), ItemsError> {
    // Gate: reject writes while the v4 key-version sweep is running so that
    // no key_version=2 row can corrupt the cursor-based resume (last_processed_id).
    if matches!(db.migration_state()?, MigrationState::InProgress { .. }) {
        return Err(ItemsError::MigrationInProgress);
    }
    let key_version = validate_key_version(item.key_version)?;
    db.conn().execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![
            item.id,
            item.item_id,
            item.content_type,
            item.content,
            item.content_nonce,
            item.blob_ref,
            item.is_sensitive as i64,
            item.is_synced as i64,
            item.lamport_ts,
            item.wall_time,
            item.expires_at,
            item.app_bundle_id,
            item.content_hash,
            item.origin_device_id,
            key_version,
            item.pinned as i64,
            item.pin_order,
            item.thumb,
            item.deleted as i64,
        ],
    )?;
    Ok(())
}

/// Read the `key_version` column for a single item row. Returns `None` if no
/// such row exists. Used by the migration sweep to spot-check that a row
/// landed on `key_version = 2` after re-encryption.
pub fn get_key_version(db: &Database, id: &str) -> Result<Option<i64>, ItemsError> {
    let result = db.conn().query_row(
        "SELECT key_version FROM clipboard_items WHERE id = ?1",
        params![id],
        |r| r.get::<_, i64>(0),
    );
    match result {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(ItemsError::Sqlite(e)),
    }
}

/// Atomically insert a clipboard item AND its FTS5 plaintext index
/// inside a single transaction.
///
/// Wraps `insert_item` + `upsert_fts` in `Connection::unchecked_transaction()`
/// so a crash between the two writes can't leave an orphan row with no FTS
/// entry (search would miss it forever).
///
/// Returns the `id` of the inserted row. On SQLITE_CONSTRAINT_UNIQUE from
/// the v5 dedup indexes (`idx_dedup_hash_minute`, `idx_clipboard_item_id`),
/// treats it as successful dedup: re-queries the existing row and returns
/// its id. Caller sees the same id it would have seen had
/// `find_recent_by_hash` won the race.
///
/// `plaintext_for_fts` is the already-decrypted text indexed for search.
/// Pass an empty string to skip FTS indexing (image items).
///
/// [P2 status] The daemon's `handle_text` and `handle_image` ingest paths
/// already call this atomic function directly, so the crash window is closed
/// on the primary capture path. The standalone `insert_item` + `upsert_fts`
/// two-step remains available only for callers that intentionally split the
/// insert and FTS update (e.g. post-decryption FTS backfill). No refactor of
/// other-crate callers is done here per the task constraint.
pub fn insert_item_with_fts(
    db: &Database,
    item: &ClipboardItem,
    plaintext_for_fts: &str,
) -> Result<String, ItemsError> {
    // Gate: reject writes while the v4 key-version sweep is running so that
    // no key_version=2 row can corrupt the cursor-based resume (last_processed_id).
    if matches!(db.migration_state()?, MigrationState::InProgress { .. }) {
        return Err(ItemsError::MigrationInProgress);
    }
    let key_version = validate_key_version(item.key_version)?;
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let insert_res = tx.execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)",
        params![
            item.id,
            item.item_id,
            item.content_type,
            item.content,
            item.content_nonce,
            item.blob_ref,
            item.is_sensitive as i64,
            item.is_synced as i64,
            item.lamport_ts,
            item.wall_time,
            item.expires_at,
            item.app_bundle_id,
            item.content_hash,
            item.origin_device_id,
            key_version,
            item.pinned as i64,
            item.pin_order,
            item.thumb,
            item.deleted as i64,
        ],
    );

    if let Err(e) = insert_res {
        let is_unique_violation = matches!(
            &e,
            rusqlite::Error::SqliteFailure(err, _)
                if err.code == rusqlite::ErrorCode::ConstraintViolation
        );
        if is_unique_violation {
            // Dedup SELECT runs inside the same transaction so it sees the
            // exact committed state that triggered the conflict — no TOCTOU
            // window between the failed INSERT and the fallback SELECT.
            if let Some(id) = lookup_existing_id_in_tx(&tx, item)? {
                return Ok(id);
            }
        }
        return Err(ItemsError::Sqlite(e));
    }

    if !plaintext_for_fts.is_empty() {
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![item.id])?;
        tx.execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
            params![item.id, plaintext_for_fts],
        )?;
    }
    tx.commit()?;
    Ok(item.id.clone())
}

/// Find the id of an existing row that conflicts with `item` on one of
/// the v5 UNIQUE indexes. Tries `content_hash` first (the more common
/// race), then falls back to `item_id` (sync replay).
///
/// Runs inside the provided transaction so the dedup SELECT is serialised
/// with the failed INSERT and sees no in-between commits.
fn lookup_existing_id_in_tx(
    tx: &rusqlite::Transaction<'_>,
    item: &ClipboardItem,
) -> Result<Option<String>, ItemsError> {
    if let Some(hash) = &item.content_hash {
        let minute_bucket = item.wall_time / 60;
        let by_hash = tx.query_row(
            "SELECT id FROM clipboard_items
             WHERE content_hash = ?1 AND (wall_time / 60) = ?2
             ORDER BY wall_time DESC LIMIT 1",
            params![hash, minute_bucket],
            |row| row.get::<_, String>(0),
        );
        match by_hash {
            Ok(id) => return Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => {}
            Err(e) => return Err(ItemsError::Sqlite(e)),
        }
    }
    let by_item_id = tx.query_row(
        "SELECT id FROM clipboard_items WHERE item_id = ?1",
        params![item.item_id],
        |row| row.get::<_, String>(0),
    );
    match by_item_id {
        Ok(id) => Ok(Some(id)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(ItemsError::Sqlite(e)),
    }
}

/// Stamp `origin_device_id` on every row that currently carries the empty
/// default (pre-v3 rows, or rows inserted before the daemon knew its device
/// id). Idempotent — rows with a non-empty origin are left alone so items
/// received from peers preserve their original origin.
///
/// Returns the number of rows updated.
pub fn backfill_origin_device_id(
    db: &Database,
    local_device_id: &str,
) -> Result<usize, ItemsError> {
    let changed = db.conn().execute(
        "UPDATE clipboard_items SET origin_device_id = ?1 WHERE origin_device_id = ''",
        params![local_device_id],
    )?;
    Ok(changed)
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
///
/// Returns the number of rows actually updated (`0` if `id` does not exist).
pub fn bump_item_recency(
    db: &Database,
    id: &str,
    now_ms: i64,
    new_lamport: i64,
) -> Result<usize, ItemsError> {
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

/// Fetch a single clipboard item by its primary-key `id`.
///
/// Returns `Ok(None)` when no row matches. Used by IPC verbs such as
/// `copy_item` that resolve an item directly by id — this avoids the
/// data-loss footgun of paging (`get_page`) and linear-scanning, which
/// silently misses any item beyond the fetched page window.
pub fn get_item_by_id<D: DbRead + ?Sized>(db: &D, id: &str) -> Result<Option<ClipboardItem>, ItemsError> {
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
fn delete_pending_uploads_for_ids(
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
fn delete_fts_for_ids(
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
pub fn has_sensitive_items(db: &Database) -> bool {
    db.conn()
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM clipboard_items WHERE is_sensitive = 1 AND pinned = 0)",
            [],
            |row| row.get::<_, bool>(0),
        )
        .unwrap_or(false)
}

/// Delete sensitive items whose `wall_time` is older than `sensitive_ttl_ms` milliseconds ago.
/// This enforces a local auto-wipe TTL for items marked `is_sensitive = 1`.
///
/// Pinned items are excluded (Fix 1). Threshold uses saturating_sub to avoid
/// underflow when sensitive_ttl_ms > now_ms (Fix 2). FTS rows are pruned in
/// the same transaction to avoid orphan FTS entries (Fix 4).
pub fn delete_sensitive_expired(
    db: &Database,
    now_ms: i64,
    sensitive_ttl_ms: i64,
) -> Result<usize, ItemsError> {
    // saturating_sub prevents underflow when ttl > now (e.g. in tests or on a
    // clock that has not advanced far past epoch).
    let threshold = now_ms.saturating_sub(sensitive_ttl_ms);
    // Collect ids first, then delete items + FTS in one transaction so no
    // orphan FTS entries accumulate (mirrors delete_expired).
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let mut stmt = tx.prepare(
        // `AND pinned = 0` mirrors `delete_expired`: pinned items are exempt from
        // every TTL prune (see ClipboardItem::pinned docs). Without this guard a
        // pinned+sensitive item is silently wiped after the sensitive TTL,
        // violating the pin contract.
        "SELECT id FROM clipboard_items WHERE is_sensitive = 1 AND wall_time < ?1 AND pinned = 0",
    )?;
    let ids: Vec<String> = stmt
        .query_map(params![threshold], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    drop(stmt);
    // CopyPaste-6fd: clean pending_uploads before the items delete.
    delete_pending_uploads_for_ids(&tx, &ids)?;
    let changed = tx.execute(
        "DELETE FROM clipboard_items WHERE is_sensitive = 1 AND wall_time < ?1 AND pinned = 0",
        params![threshold],
    )?;
    // CopyPaste-c1dd: batch FTS deletes into one statement (was N+1).
    delete_fts_for_ids(&tx, &ids)?;
    tx.commit()?;
    Ok(changed)
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
    let changed = tx.execute(
        "UPDATE clipboard_items
         SET deleted = 1,
             content = NULL,
             content_nonce = NULL,
             thumb = NULL,
             lamport_ts = ?2,
             wall_time = ?3
         WHERE id = ?1",
        params![id, lamport_ts, wall_time],
    )?;
    if changed > 0 {
        // Remove from FTS so the tombstone is not returned by search queries.
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    }
    tx.commit()?;
    Ok(changed)
}

/// Insert a fresh tombstone row for a cross-device `item_id` that is **not yet
/// known locally** (delete-before-create race — CopyPaste-bfiu).
///
/// When a delete arrives ahead of the original create (relay has no cross-push
/// ordering; cloud realtime/websocket can reorder vs the create), the receiver
/// previously dropped the tombstone because there was no row to soft-delete.
/// A later out-of-order create then resurrected the item with nothing to lose
/// LWW against.
///
/// Persisting the tombstone (deleted=1, content/nonce/thumb NULL, with the
/// incoming `lamport_ts` / `wall_time`) closes the window: the subsequent create
/// is routed through the normal LWW resolve and loses to this tombstone unless
/// it is *strictly newer*, honouring the [`soft_delete_item`] "an inbound delete
/// cannot resurrect the item" contract.
///
/// `origin_device_id` is preserved so the LWW tie-break (lamport → wall_time →
/// origin_device_id) stays deterministic across peers. The row is NOT indexed in
/// FTS (tombstones are never searchable). `id` is the local primary key to use —
/// callers typically seed it with the `item_id` for a fresh insert.
///
/// Returns the number of rows inserted (`1` on success). On a UNIQUE conflict
/// (`idx_clipboard_item_id`) the row already exists; the caller should have
/// taken the soft-delete-existing path instead, so a conflict is surfaced as an
/// error rather than silently ignored.
pub fn insert_tombstone(
    db: &Database,
    id: &str,
    item_id: &str,
    lamport_ts: i64,
    wall_time: i64,
    origin_device_id: &str,
) -> Result<usize, ItemsError> {
    // Honour the same write gate the core `insert_item` enforces.
    if matches!(db.migration_state()?, MigrationState::InProgress { .. }) {
        return Err(ItemsError::MigrationInProgress);
    }
    let inserted = db.conn().execute(
        "INSERT INTO clipboard_items
         (id, item_id, content_type, content, content_nonce, blob_ref,
          is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
          content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted)
         VALUES (?1, ?2, 'text', NULL, NULL, NULL,
                 0, 1, ?3, ?4, NULL, NULL,
                 NULL, ?5, ?6, 0, NULL, NULL, 1)",
        params![
            id,
            item_id,
            lamport_ts,
            wall_time,
            origin_device_id,
            ITEM_KEY_VERSION_CURRENT,
        ],
    )?;
    Ok(inserted)
}

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
/// Returns the number of rows updated (`0` when no row matches `id`).
pub fn set_thumb(db: &Database, id: &str, blob: Option<&[u8]>) -> Result<usize, ItemsError> {
    let changed = db.conn().execute(
        "UPDATE clipboard_items SET thumb = ?1 WHERE id = ?2",
        params![blob, id],
    )?;
    Ok(changed)
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

    // Fix (re-audit): collect the ids to evict first, then delete clipboard_items
    // AND clipboard_fts in a single transaction — mirrors the pattern used in
    // `delete_expired` / `delete_sensitive_expired`. Without the FTS sweep every
    // size-cap eviction leaves orphan FTS rows, causing unbounded FTS growth and
    // ghost search results.
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;

    // Single-pass window function: select the ids in the eviction prefix.
    // A row belongs to the prefix when cum_bytes - row_bytes < excess (i.e. the
    // running total before this row has not yet covered the excess).  The
    // "tipping" row (first one that brings the total to or past excess) is
    // included because cum_bytes[tipping-1] < excess by definition.
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

    let deleted = tx.execute(
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
         DELETE FROM clipboard_items
         WHERE id IN (
             SELECT id FROM ranked
             WHERE cum_bytes - row_bytes < ?1
         )",
        params![excess, keep_id],
    )?;
    // CopyPaste-c1dd: batch FTS deletes into one statement (was N+1).
    delete_fts_for_ids(&tx, &ids)?;
    tx.commit()?;
    Ok(deleted)
}

pub fn count_items<D: DbRead + ?Sized>(db: &D) -> Result<i64, ItemsError> {
    Ok(db.conn().query_row(
        "SELECT COUNT(*) FROM clipboard_items WHERE deleted = 0",
        [],
        |r| r.get(0),
    )?)
}

/// Maximum byte length of a text preview returned by [`fetch_text_preview`].
///
/// The UI history list renders one row per item. Sending more than 1 KiB per
/// row for a potentially-long list locks the UI rendering thread on large
/// clipboard entries. Full content is still stored encrypted; only the preview
/// is capped here. A proper rich-preview panel is planned for v0.4.
pub const MAX_PREVIEW_BYTES: usize = 1_024;

/// Fetch a clamped plaintext preview for `id` from the FTS5 index.
///
/// Returns `Some(text)` for text items that have an FTS entry, where `text`
/// is at most [`MAX_PREVIEW_BYTES`] bytes long (truncated at a UTF-8 char
/// boundary with an ellipsis appended when clamped).
///
/// Returns `None` when no FTS entry exists for the given id (image items or
/// pre-FTS rows). Callers should render an appropriate placeholder in that
/// case (e.g. `"[image — id:XXXXXXXX]"`).
pub fn fetch_text_preview<D: DbRead + ?Sized>(db: &D, id: &str) -> Result<Option<String>, ItemsError> {
    let result: Option<String> = db
        .conn()
        .query_row(
            "SELECT content_text FROM clipboard_fts WHERE id = ?1 LIMIT 1",
            params![id],
            |row| row.get(0),
        )
        .optional()
        .map_err(ItemsError::Sqlite)?;

    Ok(result.map(|text| clamp_preview(text, MAX_PREVIEW_BYTES)))
}

/// Batch variant of [`fetch_text_preview`]: fetch clamped previews for many ids
/// in a **single** `SELECT ... WHERE id IN (...)` round-trip instead of one
/// query per id.
///
/// `history_page` renders up to [`crate::storage`]'s page size of text items;
/// the per-item `fetch_text_preview` previously fired one SQL round-trip each
/// (a 50-item page = 51 round-trips). This collapses the preview fetch to one
/// statement and returns a `id → clamped preview` map. Ids with no FTS row are
/// simply absent from the map (callers render the usual placeholder).
///
/// Returns an empty map when `ids` is empty (no SQL issued).
pub fn fetch_text_previews_batch<D: DbRead + ?Sized>(
    db: &D,
    ids: &[&str],
) -> Result<std::collections::HashMap<String, String>, ItemsError> {
    if ids.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    // Build a `?,?,…` placeholder list sized to `ids`. Each id is bound as a
    // parameter (never interpolated), so this is injection-safe even though the
    // placeholder count is dynamic.
    let placeholders = std::iter::repeat_n("?", ids.len())
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!("SELECT id, content_text FROM clipboard_fts WHERE id IN ({placeholders})");
    let conn = db.conn();
    let mut stmt = conn.prepare(&sql)?;
    let params = rusqlite::params_from_iter(ids.iter());
    let rows = stmt.query_map(params, |row| {
        let id: String = row.get(0)?;
        let text: String = row.get(1)?;
        Ok((id, text))
    })?;
    let mut map = std::collections::HashMap::with_capacity(ids.len());
    for row in rows {
        let (id, text) = row.map_err(ItemsError::Sqlite)?;
        map.insert(id, clamp_preview(text, MAX_PREVIEW_BYTES));
    }
    Ok(map)
}

/// Clamp `text` to at most `max_bytes` bytes, truncating at a UTF-8 character
/// boundary and appending `…` when truncation occurs.
fn clamp_preview(text: String, max_bytes: usize) -> String {
    if text.len() <= max_bytes {
        return text;
    }
    // Walk back from max_bytes to find a valid UTF-8 char boundary.
    let boundary = (0..=max_bytes)
        .rev()
        .find(|&i| text.is_char_boundary(i))
        .unwrap_or(0);
    format!("{}…", &text[..boundary])
}

/// Insert or replace a plaintext snippet into the FTS5 index.
/// `plaintext` must already be decrypted by the caller.
/// Call this once per item after `insert_item`.
pub fn upsert_fts(db: &Database, id: &str, plaintext: &str) -> Result<(), ItemsError> {
    // FTS5 does not support ON CONFLICT; DELETE + INSERT is the correct upsert pattern.
    db.conn()
        .execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    db.conn().execute(
        "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
        params![id, plaintext],
    )?;
    Ok(())
}

/// Fetch a map of device UUID → device name from the `devices` table.
///
/// Used by `history_page` to resolve `origin_device_id` to a human-readable
/// name without requiring a per-item JOIN on every history query.  The map
/// is built once per page request; unknown device UUIDs (items captured
/// before the peer was paired, or orphaned rows) map to `None` at the call
/// site rather than appearing here.
///
/// Returns an empty map when the `devices` table is empty or when no paired
/// devices exist yet.
pub fn get_device_names<D: DbRead + ?Sized>(
    db: &D,
) -> Result<std::collections::HashMap<String, String>, ItemsError> {
    let mut stmt = db
        .conn()
        .prepare("SELECT id, name FROM devices")?;
    let pairs = stmt
        .query_map([], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            Ok((id, name))
        })?
        .collect::<Result<std::collections::HashMap<_, _>, _>>()?;
    Ok(pairs)
}

/// Remove an item's entry from the FTS5 index.
/// Call this after `delete_item` or `delete_expired`.
pub fn delete_fts(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn()
        .execute("DELETE FROM clipboard_fts WHERE id = ?1", params![id])?;
    Ok(())
}

/// Sanitize a user-supplied FTS5 query string, keeping only characters
/// that are safe to pass through the FTS5 MATCH operator:
///
/// Allowed:
///   - Unicode letters and digits (covers ASCII + Cyrillic, CJK, etc.)
///   - `_` and `-` (word-separator conventions)
///   - `"` (phrase-query delimiters, e.g. `"bar baz"`)
///   - `*` (explicit prefix operator)
///   - ASCII space
///
/// Stripped (FTS5 structural operators and SQL special chars):
///   - `:` (column filter, e.g. `col:term`)
///   - `^` (initial-token anchor)
///   - `;`, `'`, `\`, `\0` and other chars with no legitimate FTS use
///
/// Since the sanitized string is passed as a bound parameter (not
/// interpolated into SQL), SQL injection via MATCH is not possible even
/// Sanitize a raw user query into a safe FTS5 MATCH expression (S8 whitelist tokenizer).
///
/// Strategy:
/// - Strip every character that is not alphanumeric, `_`, `-`, `"`, `*`, or whitespace.
/// - If the cleaned query contains a quoted phrase (starts with `"` and ends with `"`),
///   pass it through as-is (FTS5 phrase queries are safe once other operators are stripped).
/// - Otherwise split on whitespace into individual tokens, discard empty tokens, join with
///   ` AND ` so all terms must appear, and append `*` to the last token for prefix search.
/// - Return `None` if no valid tokens remain after filtering (caller returns empty results).
///
/// This is a whitelist approach: only known-safe characters pass through, preventing
/// FTS5 operator injection (e.g. `NOT`, `OR`, `NEAR`, column filters).
fn sanitize_fts5_query(raw: &str) -> Option<String> {
    // Keep only alphanum, underscore, quote, asterisk, and whitespace.
    //
    // `-` (hyphen/minus) is an FTS5 operator: in a MATCH expression `foo -bar`
    // means "foo AND NOT column bar", so a hyphen-joined token like `foo-bar*`
    // makes FTS5 parse `-bar` as a column filter and error with
    // "no such column: bar". We therefore REWRITE `-` to whitespace (rather than
    // keeping or stripping it) so `foo-bar` splits into two AND-ed terms
    // (`foo* AND bar*`) before any per-token `*` prefix logic runs, and no raw
    // `-` ever reaches the MATCH operator.
    let cleaned: String = raw
        .chars()
        .map(|c| if c == '-' { ' ' } else { c })
        .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '"' | '*' | ' ' | '\t'))
        .collect();

    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Fix 5: count double-quotes; if the count is odd the phrase is unclosed and
    // FTS5 will return a syntax error.  Strip all double-quotes in that case so
    // the query degrades to a plain token search rather than an SQL error.
    let quote_count = trimmed.chars().filter(|&c| c == '"').count();
    let balanced = if quote_count % 2 == 0 {
        trimmed.to_string()
    } else {
        // Odd number of quotes — remove all quotes to avoid an unclosed FTS5 phrase.
        trimmed.chars().filter(|&c| c != '"').collect()
    };
    let trimmed = balanced.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Pass through quoted phrases and explicit prefix queries unchanged.
    // A quoted phrase looks like `"foo bar"` — starts and ends with a double-quote.
    if (trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() > 1)
        || trimmed.ends_with('*')
    {
        return Some(trimmed.to_string());
    }

    // Multi-word input: split into tokens, strip FTS5 reserved keywords
    // (NOT, OR, AND, NEAR) case-insensitively so a query like "secret NOT test"
    // degrades to a valid MATCH instead of an FTS5 operator-syntax error.
    let tokens: Vec<&str> = trimmed
        .split_whitespace()
        // CopyPaste-pbre: compare case-insensitively WITHOUT allocating a new
        // uppercased String per token (the old `t.to_ascii_uppercase()` heap-
        // allocated on every token just to feed a 4-way match).
        .filter(|t| {
            !["NOT", "OR", "AND", "NEAR"]
                .iter()
                .any(|kw| t.eq_ignore_ascii_case(kw))
        })
        .collect();
    // All tokens may have been stripped (e.g. query was "NOT AND") — return None
    // so the caller returns empty results rather than panicking on len()-1.
    if tokens.is_empty() {
        return None;
    }
    let last_idx = tokens.len() - 1;
    let parts: Vec<String> = tokens
        .iter()
        .enumerate()
        .map(|(i, tok)| {
            if i == last_idx {
                format!("{tok}*")
            } else {
                (*tok).to_string()
            }
        })
        .collect();

    Some(parts.join(" AND "))
}

/// Search clipboard items by full-text query.
///
/// Returns up to `limit` full `ClipboardItem` rows ordered by FTS5 rank (best match first).
///
/// Implementation: single SQL JOIN between `clipboard_fts` and `clipboard_items` — eliminates
/// the previous two-phase N+1 fetch (FTS ID list → dynamic IN-list → Rust re-sort).
/// `prepare_cached` reuses the compiled statement across repeated calls on the same connection.
///
/// The query is sanitized via `sanitize_fts5_query` (S8 whitelist tokenizer) before being
/// passed to the FTS5 MATCH operator to prevent operator injection.
pub fn search_items<D: DbRead + ?Sized>(
    db: &D,
    query: &str,
    limit: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let safe_query = match sanitize_fts5_query(query) {
        Some(q) => q,
        None => return Ok(vec![]),
    };

    // Single JOIN: FTS5 drives rank order; clipboard_items supplies full row data.
    // `fts.id` is the UNINDEXED text UUID column (matches `clipboard_items.id`).
    // `prepare_cached` avoids re-compiling the statement on every call.
    let mut stmt = db.conn().prepare_cached(
        "SELECT ci.id, ci.item_id, ci.content_type, ci.content, ci.content_nonce, ci.blob_ref,
                ci.is_sensitive, ci.is_synced, ci.lamport_ts, ci.wall_time, ci.expires_at,
                ci.app_bundle_id, ci.content_hash, ci.origin_device_id, ci.key_version,
                ci.pinned, ci.pin_order, ci.thumb, ci.deleted
         FROM clipboard_fts fts
         JOIN clipboard_items ci ON ci.id = fts.id
         WHERE clipboard_fts MATCH ?1 AND ci.deleted = 0
         ORDER BY rank
         LIMIT ?2",
    )?;

    // Fix 6: clamp before cast to avoid negative LIMIT in SQLite.
    let limit_i64 = limit.min(i64::MAX as usize) as i64;
    let rows: Vec<ClipboardItem> = stmt
        .query_map(params![safe_query, limit_i64], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(rows)
}

fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<ClipboardItem> {
    Ok(ClipboardItem {
        id: row.get(0)?,
        item_id: row.get(1)?,
        content_type: row.get(2)?,
        content: row.get(3)?,
        content_nonce: row.get(4)?,
        blob_ref: row.get(5)?,
        is_sensitive: row.get::<_, i64>(6)? != 0,
        is_synced: row.get::<_, i64>(7)? != 0,
        lamport_ts: row.get(8)?,
        wall_time: row.get(9)?,
        expires_at: row.get(10)?,
        app_bundle_id: row.get(11)?,
        content_hash: row.get(12)?,
        origin_device_id: row.get(13)?,
        key_version: {
            let kv: i64 = row.get(14)?;
            // Propagate a real error rather than silently truncating an
            // out-of-range value: `999i64 as u8` would yield 231, masking
            // corruption or a forward-compat row from a newer schema version.
            u8::try_from(kv).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(14, kv))?
        },
        pinned: row.get::<_, i64>(15)? != 0,
        pin_order: row.get(16)?,
        thumb: row.get(17)?,
        deleted: row.get::<_, i64>(18)? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Database;

    fn make_item(lamport: i64) -> ClipboardItem {
        ClipboardItem::new_text(vec![0xAA, 0xBB], vec![0u8; 24], lamport)
    }

    /// CopyPaste-bfiu: `insert_tombstone` persists a deleted row for an unknown
    /// item_id (delete-before-create race) so a later create loses LWW. The row
    /// is visible to the merge layer (`get_item_by_item_id`) but hidden from
    /// user-facing list queries (`get_page` filters deleted=0).
    #[test]
    fn insert_tombstone_persists_hidden_deleted_row() {
        let db = Database::open_in_memory().unwrap();
        let n = insert_tombstone(&db, "row-1", "iid-unknown", 42, 9000, "dev-X").unwrap();
        assert_eq!(n, 1, "one tombstone row inserted");

        // Visible to the merge layer.
        let row = get_item_by_item_id(&db, "iid-unknown")
            .unwrap()
            .expect("tombstone row exists");
        assert!(row.deleted, "row must be deleted");
        assert!(row.content.is_none(), "tombstone has no content");
        assert_eq!(row.lamport_ts, 42);
        assert_eq!(row.wall_time, 9000);
        assert_eq!(row.origin_device_id, "dev-X");

        // Hidden from the user-facing history list.
        let page = get_page(&db, 100, 0).unwrap();
        assert!(
            page.iter().all(|i| i.item_id != "iid-unknown"),
            "tombstone must not appear in the history list"
        );
    }

    #[test]
    fn new_image_carries_thumb_and_text_does_not() {
        let img = ClipboardItem::new_image(
            vec![0x01, 0x02],
            "{}".to_string(),
            1,
            Some(vec![0xAA, 0xBB, 0xCC]),
        );
        assert_eq!(img.thumb.as_deref(), Some(&[0xAA, 0xBB, 0xCC][..]));
        assert_eq!(img.content_type, "image");

        let txt = ClipboardItem::new_text(vec![0x00], vec![0u8; 24], 1);
        assert!(txt.thumb.is_none(), "text items must not carry a thumbnail");
    }

    #[test]
    fn new_file_has_file_content_type_and_no_thumb() {
        let item = ClipboardItem::new_file(vec![0x01, 0x02], "{\"k\":1}".to_string(), 3);
        assert_eq!(item.content_type, "file");
        assert!(
            item.thumb.is_none(),
            "file items must not carry a thumbnail"
        );
        assert!(
            item.content_nonce.is_none(),
            "file blob nonces live per-chunk"
        );
        assert_eq!(item.blob_ref.as_deref(), Some("{\"k\":1}"));
    }

    #[test]
    fn new_file_roundtrips_through_insert_and_select() {
        let db = Database::open_in_memory().unwrap();
        let blob = vec![0xCAu8, 0xFE, 0xBA, 0xBE];
        let meta_json =
            "{\"filename\":\"a.bin\",\"mime\":\"application/octet-stream\"}".to_string();
        let item = ClipboardItem::new_file(blob.clone(), meta_json.clone(), 5);
        let id = item.id.clone();
        insert_item(&db, &item).unwrap();

        let got = get_item_by_id(&db, &id).unwrap().expect("row must exist");
        assert_eq!(got.content_type, "file");
        assert_eq!(
            got.content.as_deref(),
            Some(blob.as_slice()),
            "encrypted blob must survive insert + select"
        );
        assert_eq!(
            got.blob_ref.as_deref(),
            Some(meta_json.as_str()),
            "blob_ref meta JSON must survive insert + select"
        );
    }

    #[test]
    fn thumb_roundtrips_through_insert_and_select() {
        let db = Database::open_in_memory().unwrap();
        let thumb = vec![0xDEu8, 0xAD, 0xBE, 0xEF];
        let item =
            ClipboardItem::new_image(vec![0x10, 0x20], "{}".to_string(), 1, Some(thumb.clone()));
        let id = item.id.clone();
        insert_item(&db, &item).unwrap();

        let got = get_item_by_id(&db, &id).unwrap().expect("row must exist");
        assert_eq!(
            got.thumb.as_deref(),
            Some(thumb.as_slice()),
            "thumb blob must survive insert + select"
        );
    }

    #[test]
    fn set_thumb_backfills_and_clears() {
        let db = Database::open_in_memory().unwrap();
        // Insert an image row with NO thumbnail (legacy / pre-pipeline row).
        let item = ClipboardItem::new_image(vec![0x10, 0x20], "{}".to_string(), 1, None);
        let id = item.id.clone();
        insert_item(&db, &item).unwrap();
        assert!(get_item_by_id(&db, &id).unwrap().unwrap().thumb.is_none());

        // Lazy backfill.
        let blob = vec![0x01u8, 0x02, 0x03];
        let changed = set_thumb(&db, &id, Some(&blob)).unwrap();
        assert_eq!(changed, 1);
        assert_eq!(
            get_item_by_id(&db, &id).unwrap().unwrap().thumb.as_deref(),
            Some(blob.as_slice())
        );

        // Clearing back to NULL.
        let changed = set_thumb(&db, &id, None).unwrap();
        assert_eq!(changed, 1);
        assert!(get_item_by_id(&db, &id).unwrap().unwrap().thumb.is_none());

        // No-op on an unknown id.
        let changed = set_thumb(&db, "00000000-0000-0000-0000-000000000000", Some(&blob)).unwrap();
        assert_eq!(changed, 0);
    }

    #[test]
    fn insert_and_count() {
        let db = Database::open_in_memory().unwrap();
        insert_item(&db, &make_item(1)).unwrap();
        insert_item(&db, &make_item(2)).unwrap();
        assert_eq!(count_items(&db).unwrap(), 2);
    }

    #[test]
    fn pagination_returns_correct_page() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..10 {
            insert_item(&db, &make_item(i)).unwrap();
        }
        let page1 = get_page(&db, 3, 0).unwrap();
        let page2 = get_page(&db, 3, 3).unwrap();
        assert_eq!(page1.len(), 3);
        assert_eq!(page2.len(), 3);
        let ids1: Vec<_> = page1.iter().map(|i| &i.id).collect();
        let ids2: Vec<_> = page2.iter().map(|i| &i.id).collect();
        assert!(ids1.iter().all(|id| !ids2.contains(id)));
    }

    #[test]
    fn get_page_meta_omits_content_blob_but_keeps_metadata() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.content_hash = Some("deadbeef".to_string());
        item.blob_ref = Some("blob://x".to_string());
        let id = item.id.clone();
        insert_item(&db, &item).unwrap();

        // Sanity: get_page returns the full blob.
        let full = get_page(&db, 10, 0).unwrap();
        assert_eq!(full.len(), 1);
        assert_eq!(full[0].content.as_deref(), Some(&[0xAA, 0xBB][..]));

        // get_page_meta drops the blob but preserves metadata.
        let meta = get_page_meta(&db, 10, 0).unwrap();
        assert_eq!(meta.len(), 1);
        assert_eq!(meta[0].id, id);
        assert!(
            meta[0].content.is_none(),
            "get_page_meta must NOT load content blob"
        );
        assert_eq!(meta[0].content_hash.as_deref(), Some("deadbeef"));
        assert_eq!(meta[0].blob_ref.as_deref(), Some("blob://x"));
        assert_eq!(meta[0].content_nonce.as_deref(), Some(&[0u8; 24][..]));
    }

    #[test]
    fn delete_expired_removes_old_items() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.expires_at = Some(1000);
        insert_item(&db, &item).unwrap();
        let mut item2 = make_item(2);
        item2.expires_at = None;
        insert_item(&db, &item2).unwrap();
        let removed = delete_expired(&db, 2000).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(count_items(&db).unwrap(), 1);
    }

    #[test]
    fn delete_item_removes_specific_row() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        let id = item.id.clone();
        insert_item(&db, &item).unwrap();
        let removed = delete_item(&db, &id).unwrap();
        assert_eq!(removed, 1, "exactly one row removed");
        assert_eq!(count_items(&db).unwrap(), 0);
    }

    #[test]
    fn delete_item_reports_zero_for_missing_row() {
        let db = Database::open_in_memory().unwrap();
        let removed = delete_item(&db, "00000000-0000-0000-0000-000000000000").unwrap();
        assert_eq!(removed, 0, "no row matched, nothing removed");
    }

    #[test]
    fn get_item_by_id_returns_matching_row() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(7);
        let id = item.id.clone();
        insert_item(&db, &item).unwrap();

        let found = get_item_by_id(&db, &id).unwrap();
        assert!(found.is_some(), "inserted row must be found by id");
        let found = found.unwrap();
        assert_eq!(found.id, id);
        assert_eq!(found.lamport_ts, 7);
        assert_eq!(found.content.as_deref(), Some(&[0xAA, 0xBB][..]));
    }

    #[test]
    fn get_item_by_id_returns_none_for_missing_row() {
        let db = Database::open_in_memory().unwrap();
        let found = get_item_by_id(&db, "00000000-0000-0000-0000-000000000000").unwrap();
        assert!(found.is_none(), "absent id must yield None, not an error");
    }

    #[test]
    fn get_item_by_id_finds_row_beyond_first_page() {
        // Regression: `copy_item` used to page get_page(1000, 0) and scan, so
        // any item past position 1000 was unreachable. get_item_by_id must
        // resolve a row regardless of how many other rows exist.
        let db = Database::open_in_memory().unwrap();
        let mut target_id = String::new();
        for i in 0..1200 {
            let item = make_item(i);
            if i == 0 {
                // Oldest row (sorts last under ORDER BY wall_time DESC) — would
                // fall outside a 1000-row window once 1200 rows exist.
                target_id = item.id.clone();
            }
            insert_item(&db, &item).unwrap();
        }
        let found = get_item_by_id(&db, &target_id).unwrap();
        assert!(
            found.is_some(),
            "row beyond the legacy 1000-row page window must still be found"
        );
        assert_eq!(found.unwrap().id, target_id);
    }

    // --- Task 1: upsert_fts ---

    #[test]
    fn upsert_fts_inserts_and_replaces() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();

        upsert_fts(&db, &item.id, "hello world").unwrap();

        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);

        // Upsert again with different text — must not duplicate
        upsert_fts(&db, &item.id, "updated text").unwrap();
        let count2: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count2, 1);
    }

    // --- Task 2: delete_fts ---

    #[test]
    fn delete_fts_removes_fts_entry() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "some text").unwrap();

        delete_fts(&db, &item.id).unwrap();

        let count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![item.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn delete_fts_nonexistent_id_is_ok() {
        let db = Database::open_in_memory().unwrap();
        // Should not error even if id doesn't exist
        delete_fts(&db, "nonexistent-id").unwrap();
    }

    // --- Task 3: search_items ---

    #[test]
    fn search_items_finds_matching_text() {
        let db = Database::open_in_memory().unwrap();
        let item1 = make_item(1);
        let item2 = make_item(2);
        insert_item(&db, &item1).unwrap();
        insert_item(&db, &item2).unwrap();
        upsert_fts(&db, &item1.id, "hello world clipboard").unwrap();
        upsert_fts(&db, &item2.id, "rust programming language").unwrap();

        let results = search_items(&db, "hello", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, item1.id);
    }

    #[test]
    fn search_items_empty_query_returns_empty() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "hello world").unwrap();

        let results = search_items(&db, "", 10).unwrap();
        assert_eq!(results.len(), 0);

        let results2 = search_items(&db, "   ", 10).unwrap();
        assert_eq!(results2.len(), 0);
    }

    #[test]
    fn search_items_no_match_returns_empty() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "hello world").unwrap();

        let results = search_items(&db, "nonexistentword", 10).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn search_items_respects_limit() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            let item = make_item(i);
            insert_item(&db, &item).unwrap();
            upsert_fts(&db, &item.id, "common search term").unwrap();
        }

        let results = search_items(&db, "common", 3).unwrap();
        assert_eq!(results.len(), 3);
    }

    /// Regression (P0): a hyphen-joined query like `foo-bar` must not reach the
    /// FTS5 MATCH operator with a raw `-`, otherwise FTS5 parses `-bar` as a
    /// column filter and errors with "no such column: bar". The sanitizer
    /// rewrites `-` to whitespace so these queries succeed (return Ok).
    #[test]
    fn search_items_hyphen_query_does_not_error() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "harmless content").unwrap();

        // Each of these previously triggered "no such column: ..." on real DBs.
        for q in [
            "foo-bar",
            "2026-06-02",
            "x86-64",
            "well-known",
            "co-op coffee",
        ] {
            let res = search_items(&db, q, 10);
            assert!(
                res.is_ok(),
                "hyphen query {q:?} must not error, got: {:?}",
                res.err()
            );
        }
    }

    /// A stored item containing a hyphenated word must be found when the user
    /// searches for that same hyphenated term: `well-known` → `well AND known*`.
    #[test]
    fn search_items_finds_hyphenated_term() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "this is a well-known endpoint").unwrap();

        let results = search_items(&db, "well-known", 10).unwrap();
        assert_eq!(results.len(), 1, "hyphenated term must match stored item");
        assert_eq!(results[0].id, item.id);
    }

    /// Direct unit check of the sanitizer: hyphens become whitespace-separated
    /// AND-ed terms and no raw `-` survives.
    #[test]
    fn sanitize_fts5_query_rewrites_hyphen_to_space() {
        let out = sanitize_fts5_query("foo-bar").expect("non-empty");
        assert!(!out.contains('-'), "no raw hyphen may remain: {out:?}");
        assert_eq!(out, "foo AND bar*");
    }

    #[test]
    fn delete_sensitive_expired_removes_old_sensitive_items() {
        let db = Database::open_in_memory().unwrap();

        // Sensitive item with old wall_time (should be deleted)
        let mut old_sensitive = make_item(1);
        old_sensitive.is_sensitive = true;
        old_sensitive.wall_time = 1_000; // very old
        insert_item(&db, &old_sensitive).unwrap();

        // Sensitive item with recent wall_time (should be kept)
        let mut new_sensitive = make_item(2);
        new_sensitive.is_sensitive = true;
        new_sensitive.wall_time = 100_000_000; // very recent relative to now_ms below
        insert_item(&db, &new_sensitive).unwrap();

        // Non-sensitive item with old wall_time (should NOT be deleted)
        let mut old_plain = make_item(3);
        old_plain.is_sensitive = false;
        old_plain.wall_time = 1_000;
        insert_item(&db, &old_plain).unwrap();

        // now_ms = 200_000, ttl = 30_000 → threshold = 170_000
        // old_sensitive.wall_time=1000 < 170_000 → deleted
        // new_sensitive.wall_time=100_000_000 > 170_000 → kept
        // old_plain.wall_time=1000 < 170_000 but not sensitive → kept
        let removed = delete_sensitive_expired(&db, 200_000, 30_000).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(count_items(&db).unwrap(), 2);
    }

    #[test]
    fn delete_sensitive_expired_keeps_pinned_items() {
        // Regression: a pinned + sensitive item past the sensitive cutoff must
        // NOT be auto-wiped — pinned rows are exempt from every TTL prune.
        let db = Database::open_in_memory().unwrap();

        // Pinned + sensitive + old wall_time → must survive the prune.
        let mut pinned_sensitive = make_item(1);
        pinned_sensitive.is_sensitive = true;
        pinned_sensitive.pinned = true;
        pinned_sensitive.wall_time = 1_000; // well past the cutoff below
        let pinned_id = pinned_sensitive.id.clone();
        insert_item(&db, &pinned_sensitive).unwrap();

        // Unpinned + sensitive + old wall_time → control row, must be deleted.
        let mut unpinned_sensitive = make_item(2);
        unpinned_sensitive.is_sensitive = true;
        unpinned_sensitive.pinned = false;
        unpinned_sensitive.wall_time = 1_000;
        insert_item(&db, &unpinned_sensitive).unwrap();

        // now_ms=200_000, ttl=30_000 → threshold=170_000; both wall_times qualify.
        let removed = delete_sensitive_expired(&db, 200_000, 30_000).unwrap();
        assert_eq!(removed, 1, "only the unpinned sensitive row is wiped");
        assert!(
            get_item_by_id(&db, &pinned_id).unwrap().is_some(),
            "pinned+sensitive item must survive the sensitive TTL prune"
        );
    }

    #[test]
    fn pin_item_removes_expiry() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.expires_at = Some(9999);
        insert_item(&db, &item).unwrap();
        pin_item(&db, &item.id).unwrap();
        // Verify expired returns 0 (pinned item not deleted)
        let removed = delete_expired(&db, 99999).unwrap();
        assert_eq!(removed, 0);
    }

    /// Regression: `pin_item` and `unpin_item` must bump `lamport_ts` so the
    /// pin-state change wins LWW merge on peers that already have the item.
    /// Without this bump a peer receiving the item via cloud backlog or P2P
    /// would silently discard the pin update because the timestamp tie-breaks
    /// in favour of the (unchanged) local copy.
    ///
    /// CopyPaste-ojhe: the bump now stamps the UNIFIED value space
    /// `MAX(lamport_ts + 1, now_ms)`, not a bare `+1`. A `make_item(10)` row
    /// pinned today lands on `now_ms` (~1.75e12), strictly greater than 10, so
    /// the pin remains monotonic AND time-ordered — and can overtake a stale
    /// now_ms-magnitude recopy of the same item (the bug this fixes).
    #[test]
    fn pin_unpin_bumps_lamport_ts() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(10);
        let id = item.id.clone();
        insert_item(&db, &item).unwrap();
        // Wall-clock floor: every unified stamp is at least this.
        let floor = now_ms_epoch() - 1000;

        // pin_item must advance lamport_ts to the unified value (>= now_ms).
        pin_item(&db, &id).unwrap();
        let after_pin = get_item_by_id(&db, &id).unwrap().expect("row must exist");
        assert!(
            after_pin.lamport_ts > 10,
            "pin_item must bump lamport_ts above the inserted value (was 10, got {})",
            after_pin.lamport_ts
        );
        assert!(
            after_pin.lamport_ts >= floor,
            "pin_item must stamp the unified now_ms-based value (got {}, floor {})",
            after_pin.lamport_ts,
            floor
        );
        assert!(after_pin.pinned, "item must be pinned after pin_item");
        assert!(
            after_pin.pin_order.is_some(),
            "pin_item must assign a non-null pin_order"
        );

        // unpin_item must advance lamport_ts strictly beyond the post-pin value.
        unpin_item(&db, &id).unwrap();
        let after_unpin = get_item_by_id(&db, &id).unwrap().expect("row must exist");
        assert!(
            after_unpin.lamport_ts >= after_pin.lamport_ts,
            "unpin_item must not regress lamport_ts (pin={}, unpin={})",
            after_pin.lamport_ts,
            after_unpin.lamport_ts
        );
        assert!(!after_unpin.pinned, "item must be unpinned after unpin_item");
        assert!(
            after_unpin.pin_order.is_none(),
            "unpin_item must clear pin_order back to NULL"
        );
    }

    /// CopyPaste-ojhe: `next_lamport_ts` is monotonic AND time-ordered.
    #[test]
    fn next_lamport_ts_is_monotonic_and_time_ordered() {
        // When now_ms dominates (fresh capture, prev=0), we get now_ms.
        assert_eq!(next_lamport_ts(0, 1_750_000_000_000), 1_750_000_000_000);
        // When prev+1 dominates (two edits in the same ms), we get prev+1 so the
        // value still strictly increases.
        assert_eq!(
            next_lamport_ts(1_750_000_000_005, 1_750_000_000_000),
            1_750_000_000_006
        );
        // Always strictly greater than prev.
        for prev in [0i64, 1, 1_750_000_000_000, i64::MAX - 1] {
            assert!(next_lamport_ts(prev, 0) > prev || prev == i64::MAX);
        }
    }

    /// CopyPaste-ojhe: a newer pin (unified) beats an older recopy (now_ms) when
    /// compared by raw lamport — the exact data-loss scenario from the audit.
    #[test]
    fn newer_pin_lamport_beats_older_recopy_lamport() {
        // Older recopy stamped at now_ms.
        let recopy_now = 1_750_000_000_000i64;
        let recopy_lamport = next_lamport_ts(0, recopy_now); // == recopy_now

        // The item is then pinned a few ms later: MAX(recopy + 1, pin_now).
        let pin_now = recopy_now + 5;
        let pin_lamport = next_lamport_ts(recopy_lamport, pin_now);

        assert!(
            pin_lamport > recopy_lamport,
            "the unified pin lamport ({pin_lamport}) must exceed the recopy \
             lamport ({recopy_lamport}) so lamport-first LWW keeps the pin"
        );
    }

    /// Regression: `reorder_pinned` must bump `lamport_ts` on each row so the
    /// new drag-to-reorder ordering wins LWW merge on peers.
    #[test]
    fn reorder_pinned_bumps_lamport_ts() {
        let db = Database::open_in_memory().unwrap();

        let item_a = make_item(5);
        let id_a = item_a.id.clone();
        insert_item(&db, &item_a).unwrap();
        pin_item(&db, &id_a).unwrap();

        let item_b = make_item(6);
        let id_b = item_b.id.clone();
        insert_item(&db, &item_b).unwrap();
        pin_item(&db, &id_b).unwrap();

        // Record lamport_ts values after pinning.
        let a_before = get_item_by_id(&db, &id_a)
            .unwrap()
            .expect("row must exist")
            .lamport_ts;
        let b_before = get_item_by_id(&db, &id_b)
            .unwrap()
            .expect("row must exist")
            .lamport_ts;

        // Reorder: put b first, a second.
        reorder_pinned(&db, &[&id_b, &id_a]).unwrap();

        let a_after = get_item_by_id(&db, &id_a)
            .unwrap()
            .expect("row must exist")
            .lamport_ts;
        let b_after = get_item_by_id(&db, &id_b)
            .unwrap()
            .expect("row must exist")
            .lamport_ts;

        assert!(
            a_after > a_before,
            "reorder_pinned must bump lamport_ts on item_a: before={a_before}, after={a_after}"
        );
        assert!(
            b_after > b_before,
            "reorder_pinned must bump lamport_ts on item_b: before={b_before}, after={b_after}"
        );
    }

    #[test]
    fn newly_inserted_items_land_on_key_version_2() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();

        let kv = get_key_version(&db, &item.id).unwrap();
        assert_eq!(
            kv,
            Some(ITEM_KEY_VERSION_CURRENT),
            "insert_item must stamp the current key_version on new rows"
        );
        assert_eq!(ITEM_KEY_VERSION_CURRENT, 2);
    }

    #[test]
    fn insert_persists_item_key_version_not_constant() {
        // Regression: insert must bind `item.key_version`, not the
        // ITEM_KEY_VERSION_CURRENT constant. A v1 item must persist as 1.
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.key_version = 1;
        insert_item(&db, &item).unwrap();
        assert_eq!(
            get_key_version(&db, &item.id).unwrap(),
            Some(1),
            "insert_item must persist item.key_version verbatim"
        );

        // Same contract for the FTS path.
        let mut item2 = make_item(2);
        item2.key_version = 1;
        let id2 = insert_item_with_fts(&db, &item2, "indexed text").unwrap();
        assert_eq!(get_key_version(&db, &id2).unwrap(), Some(1));
    }

    #[test]
    fn insert_rejects_out_of_range_key_version() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.key_version = 3; // outside the supported {1, 2} set
        let err = insert_item(&db, &item).unwrap_err();
        assert!(
            matches!(err, ItemsError::UnsupportedKeyVersion(3)),
            "out-of-range key_version must be rejected, not silently written: {err:?}"
        );
        // Nothing should have been persisted.
        assert_eq!(count_items(&db).unwrap(), 0);
    }

    #[test]
    fn get_key_version_missing_id_returns_none() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(get_key_version(&db, "nope").unwrap(), None);
    }

    #[test]
    fn insert_item_with_fts_writes_both_atomically() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        let id = item.id.clone();

        let returned = insert_item_with_fts(&db, &item, "hello clipboard world").unwrap();
        assert_eq!(returned, id, "fresh insert returns the supplied id");

        let row_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1, "item row must be present");

        let fts_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 1, "FTS row must be present");

        // Search round-trip — confirms the FTS index actually points at
        // the same id and is searchable.
        let results = search_items(&db, "clipboard", 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, id);
    }

    #[test]
    fn insert_item_with_fts_skips_fts_on_empty_text() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        let id = item.id.clone();

        let returned = insert_item_with_fts(&db, &item, "").unwrap();
        assert_eq!(returned, id);

        let row_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 1, "item row inserted even when FTS skipped");

        let fts_count: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_fts WHERE id = ?1",
                params![id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(fts_count, 0, "FTS row skipped for empty plaintext");
    }

    #[test]
    fn insert_item_with_fts_dedup_returns_existing_id_on_hash_race() {
        let db = Database::open_in_memory().unwrap();

        // First insert: stamped with a content_hash.
        let mut first = make_item(1);
        first.content_hash = Some("abc123".to_string());
        first.wall_time = 60_000; // bucket = 60_000 / 60 = 1000
        let first_id = insert_item_with_fts(&db, &first, "hello").unwrap();

        // Second insert: distinct logical id but same hash AND same
        // minute bucket → idx_dedup_hash_minute fires.
        let mut second = make_item(2);
        second.content_hash = Some("abc123".to_string());
        second.wall_time = 60_059; // 60_059 / 60 = 1000 (same bucket)
        let returned = insert_item_with_fts(&db, &second, "hello again").unwrap();

        assert_eq!(
            returned, first_id,
            "dedup race must return the existing row's id, not the new one"
        );
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1, "second insert must not create a duplicate row");
    }

    #[test]
    fn insert_item_with_fts_dedup_returns_existing_id_on_item_id_race() {
        let db = Database::open_in_memory().unwrap();

        let first = make_item(1);
        let first_id = insert_item_with_fts(&db, &first, "").unwrap();

        // Sync replay: peer re-broadcasts the same item_id with a new
        // logical id. idx_clipboard_item_id fires.
        let mut second = make_item(2);
        second.item_id = first.item_id.clone();
        let returned = insert_item_with_fts(&db, &second, "").unwrap();

        assert_eq!(returned, first_id);
        let count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn backfill_origin_device_id_only_touches_empty_rows() {
        let db = Database::open_in_memory().unwrap();

        // Row A: empty origin (pre-v3 default) → must be backfilled.
        let mut a = make_item(1);
        a.origin_device_id = String::new();
        insert_item(&db, &a).unwrap();

        // Row B: already-set origin (item received from peer "peer-xyz") →
        // must remain untouched so peer-origin items keep their provenance.
        let mut b = make_item(2);
        b.origin_device_id = "peer-xyz".to_string();
        insert_item(&db, &b).unwrap();

        let changed = backfill_origin_device_id(&db, "local-uuid").unwrap();
        assert_eq!(changed, 1, "only the empty-origin row must be updated");

        let got_a: String = db
            .conn()
            .query_row(
                "SELECT origin_device_id FROM clipboard_items WHERE id = ?1",
                params![a.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(got_a, "local-uuid");

        let got_b: String = db
            .conn()
            .query_row(
                "SELECT origin_device_id FROM clipboard_items WHERE id = ?1",
                params![b.id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(got_b, "peer-xyz", "peer origin must not be overwritten");
    }

    // --- T5: UI clipboard model — preview clamp + edge cases ---

    /// `clamp_preview` must return the text unchanged when it fits within the limit.
    #[test]
    fn clamp_preview_short_text_unchanged() {
        let text = "hello world".to_string();
        assert_eq!(clamp_preview(text.clone(), MAX_PREVIEW_BYTES), text);
    }

    /// `clamp_preview` must truncate at a UTF-8 boundary and append `…`.
    #[test]
    fn clamp_preview_long_text_truncated() {
        // Build a string that is longer than MAX_PREVIEW_BYTES (1024 bytes).
        let long_text: String = "a".repeat(MAX_PREVIEW_BYTES + 100);
        let result = clamp_preview(long_text, MAX_PREVIEW_BYTES);
        // Result must be at most MAX_PREVIEW_BYTES bytes (plus the 3-byte `…` ellipsis).
        // The truncated body is ≤ MAX_PREVIEW_BYTES and the appended ellipsis is "…" (3 bytes).
        assert!(
            result.len() <= MAX_PREVIEW_BYTES + "…".len(),
            "clamped preview too long: {} bytes",
            result.len()
        );
        assert!(
            result.ends_with('…'),
            "clamped preview must end with ellipsis"
        );
        assert!(
            result.is_char_boundary(result.len()),
            "result must be valid UTF-8"
        );
    }

    /// `clamp_preview` must not split a multi-byte character.
    #[test]
    fn clamp_preview_respects_utf8_boundary() {
        // Each '€' is 3 bytes (U+20AC).  Build a string where the naive byte
        // boundary would fall inside a character.
        let euros: String = "€".repeat(400); // 1200 bytes total
        let result = clamp_preview(euros, MAX_PREVIEW_BYTES);
        // Must be valid UTF-8 (would panic on index otherwise).
        assert!(std::str::from_utf8(result.as_bytes()).is_ok());
        assert!(result.ends_with('…'));
    }

    /// `fetch_text_preview` returns `None` for items with no FTS entry (e.g. images).
    #[test]
    fn fetch_text_preview_returns_none_for_no_fts_entry() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        // No FTS entry inserted — simulates an image item or pre-FTS row.
        let result = fetch_text_preview(&db, &item.id).unwrap();
        assert!(result.is_none(), "expected None when no FTS entry exists");
    }

    /// `fetch_text_preview` returns clamped plaintext for text items.
    #[test]
    fn fetch_text_preview_returns_short_text_unchanged() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        upsert_fts(&db, &item.id, "short snippet").unwrap();

        let result = fetch_text_preview(&db, &item.id).unwrap();
        assert_eq!(result, Some("short snippet".to_string()));
    }

    /// `fetch_text_preview` clamps text that exceeds MAX_PREVIEW_BYTES.
    #[test]
    fn fetch_text_preview_clamps_large_text() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        let big_text: String = "x".repeat(MAX_PREVIEW_BYTES + 500);
        upsert_fts(&db, &item.id, &big_text).unwrap();

        let result = fetch_text_preview(&db, &item.id).unwrap().unwrap();
        assert!(
            result.len() <= MAX_PREVIEW_BYTES + "…".len(),
            "preview must be clamped to ~{} bytes, got {}",
            MAX_PREVIEW_BYTES,
            result.len()
        );
        assert!(result.ends_with('…'));
    }

    /// CopyPaste-mnte: batch preview fetch returns clamped text for every id
    /// that has an FTS entry, in one round-trip, and omits ids without one.
    #[test]
    fn fetch_text_previews_batch_returns_map_for_present_ids() {
        let db = Database::open_in_memory().unwrap();
        let a = make_item(1);
        let b = make_item(2);
        let c = make_item(3); // no FTS entry — must be absent from the map
        insert_item(&db, &a).unwrap();
        insert_item(&db, &b).unwrap();
        insert_item(&db, &c).unwrap();
        upsert_fts(&db, &a.id, "alpha snippet").unwrap();
        upsert_fts(&db, &b.id, "beta snippet").unwrap();

        let ids = [a.id.as_str(), b.id.as_str(), c.id.as_str()];
        let map = fetch_text_previews_batch(&db, &ids).unwrap();

        assert_eq!(map.get(&a.id).map(String::as_str), Some("alpha snippet"));
        assert_eq!(map.get(&b.id).map(String::as_str), Some("beta snippet"));
        assert!(
            !map.contains_key(&c.id),
            "id with no FTS entry must be absent from the batch map"
        );
        // Parity with the per-item helper for both present ids.
        assert_eq!(map.get(&a.id).cloned(), fetch_text_preview(&db, &a.id).unwrap());
        assert_eq!(map.get(&b.id).cloned(), fetch_text_preview(&db, &b.id).unwrap());
    }

    /// CopyPaste-mnte: empty id slice issues no SQL and returns an empty map.
    #[test]
    fn fetch_text_previews_batch_empty_ids_is_noop() {
        let db = Database::open_in_memory().unwrap();
        let map = fetch_text_previews_batch(&db, &[]).unwrap();
        assert!(map.is_empty());
    }

    /// CopyPaste-mnte: batch preview clamps long text identically to the
    /// per-item path.
    #[test]
    fn fetch_text_previews_batch_clamps_large_text() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        let big_text: String = "y".repeat(MAX_PREVIEW_BYTES + 500);
        upsert_fts(&db, &item.id, &big_text).unwrap();

        let map = fetch_text_previews_batch(&db, &[item.id.as_str()]).unwrap();
        let got = map.get(&item.id).expect("present");
        assert!(got.len() <= MAX_PREVIEW_BYTES + "…".len());
        assert!(got.ends_with('…'));
    }

    /// CopyPaste-pvp4: the schema-v11 partial covering index used by the
    /// `prune_to_cap` size gate exists on a freshly migrated database.
    #[test]
    fn schema_has_unpinned_len_covering_index() {
        let db = Database::open_in_memory().unwrap();
        let found: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master \
                 WHERE type = 'index' AND name = 'idx_clipboard_unpinned_len'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(found, 1, "idx_clipboard_unpinned_len must exist");
    }

    /// CopyPaste-pvp4: the `prune_to_cap` size-gate SUM is planned as an
    /// index-only scan over the partial covering index (no full-table scan and
    /// no BLOB reads). We assert the query plan references the covering index.
    #[test]
    fn prune_to_cap_size_gate_uses_covering_index() {
        let db = Database::open_in_memory().unwrap();
        for i in 0..5 {
            insert_item(&db, &make_item(i)).unwrap();
        }
        let plan: Vec<String> = {
            let conn = db.conn();
            let mut stmt = conn
                .prepare(
                    "EXPLAIN QUERY PLAN \
                     SELECT COALESCE(SUM(LENGTH(COALESCE(content, ''))), 0) \
                     FROM clipboard_items WHERE pinned = 0",
                )
                .unwrap();
            stmt.query_map([], |r| r.get::<_, String>(3))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap()
        };
        let joined = plan.join(" | ");
        assert!(
            joined.contains("idx_clipboard_unpinned_len"),
            "size-gate SUM must use the covering index, plan was: {joined}"
        );
    }

    /// Empty history list — model correctly handles zero items.
    #[test]
    fn get_page_meta_empty_db_returns_empty_list() {
        let db = Database::open_in_memory().unwrap();
        let result = get_page_meta(&db, 50, 0).unwrap();
        assert!(result.is_empty(), "expected empty list for empty DB");
    }

    // --- FIX 1: get_page_pinned_first ---

    /// Pinned items must appear before unpinned items regardless of wall_time.
    #[test]
    fn get_page_pinned_first_pins_before_unpinned() {
        let db = Database::open_in_memory().unwrap();

        // Insert three items with ascending wall_time.
        // item_a: oldest, will be pinned
        // item_b: middle, unpinned
        // item_c: newest, unpinned
        let mut item_a = make_item(1);
        item_a.wall_time = 1_000;
        let id_a = item_a.id.clone();
        insert_item(&db, &item_a).unwrap();

        let mut item_b = make_item(2);
        item_b.wall_time = 2_000;
        insert_item(&db, &item_b).unwrap();

        let mut item_c = make_item(3);
        item_c.wall_time = 3_000;
        insert_item(&db, &item_c).unwrap();

        // Pin item_a (the oldest one).
        pin_item(&db, &id_a).unwrap();

        let page = get_page_pinned_first(&db, 10, 0).unwrap();
        assert_eq!(page.len(), 3);
        // Pinned item must be first, regardless of its wall_time.
        assert_eq!(
            page[0].id, id_a,
            "pinned item must be first regardless of age"
        );
        assert!(page[0].pinned, "first item must have pinned=true");
        // Remaining items must be sorted newest-first.
        assert!(
            page[1].wall_time >= page[2].wall_time,
            "unpinned items must be newest-first"
        );
    }

    /// Multiple pinned items are sorted newest-first within the pinned group.
    /// Pinned items appear before unpinned items; within the pinned group they
    /// are ordered by `pin_order ASC` (insertion order by default, since
    /// `pin_item` assigns `MAX(pin_order)+1`). This test verifies that the item
    /// pinned first has the lower `pin_order` and therefore appears first,
    /// regardless of its `wall_time`.
    #[test]
    fn get_page_pinned_first_multiple_pins_sorted_by_pin_order() {
        let db = Database::open_in_memory().unwrap();

        // old_pin: low wall_time, pinned first → pin_order = 1.0
        let mut old_pin = make_item(1);
        old_pin.wall_time = 100;
        let old_pin_id = old_pin.id.clone();
        insert_item(&db, &old_pin).unwrap();
        pin_item(&db, &old_pin_id).unwrap();

        // new_pin: high wall_time, pinned second → pin_order = 2.0
        let mut new_pin = make_item(2);
        new_pin.wall_time = 900;
        let new_pin_id = new_pin.id.clone();
        insert_item(&db, &new_pin).unwrap();
        pin_item(&db, &new_pin_id).unwrap();

        let mut unpinned = make_item(3);
        unpinned.wall_time = 500;
        insert_item(&db, &unpinned).unwrap();

        let page = get_page_pinned_first(&db, 10, 0).unwrap();
        assert_eq!(page.len(), 3);
        // Both pins appear first.
        assert!(
            page[0].pinned && page[1].pinned,
            "first two items must be pinned"
        );
        // Within the pinned group, order is by pin_order ASC (insertion order).
        // old_pin was pinned first (pin_order=1.0) so it appears before new_pin
        // (pin_order=2.0) even though old_pin has a lower wall_time.
        assert_eq!(
            page[0].id, old_pin_id,
            "item pinned first (lower pin_order) must appear first"
        );
        assert_eq!(
            page[1].id, new_pin_id,
            "item pinned second (higher pin_order) must appear second"
        );
        assert!(
            page[0].pin_order.unwrap() < page[1].pin_order.unwrap(),
            "pin_order must be ascending within the pinned group"
        );
        // Then unpinned.
        assert!(!page[2].pinned, "third item must not be pinned");
    }

    /// Defensive (HIGH): a sync-replaced pinned row whose `pin_order` became
    /// NULL must sort AFTER pinned rows with explicit `pin_order` values, not
    /// before them. SQLite sorts NULL first under plain `ASC`, so the ORDER BY
    /// adds `pin_order IS NULL ASC` to push NULLs to the end of the pinned group.
    #[test]
    fn get_page_pinned_first_null_pin_order_sorts_last_among_pins() {
        let db = Database::open_in_memory().unwrap();

        // Two normally-pinned items with explicit pin_order 1.0 and 2.0.
        let mut p1 = make_item(1);
        p1.wall_time = 100;
        let p1_id = p1.id.clone();
        insert_item(&db, &p1).unwrap();
        pin_item(&db, &p1_id).unwrap();

        let mut p2 = make_item(2);
        p2.wall_time = 200;
        let p2_id = p2.id.clone();
        insert_item(&db, &p2).unwrap();
        pin_item(&db, &p2_id).unwrap();

        // A pinned item whose pin_order is NULL (simulating a sync replace that
        // dropped pin_order). Insert directly with pinned=1, pin_order=None.
        let mut null_pin = make_item(3);
        null_pin.wall_time = 9_999; // newest, to prove ordering is by pin_order not wall_time
        null_pin.pinned = true;
        null_pin.pin_order = None;
        let null_pin_id = null_pin.id.clone();
        insert_item(&db, &null_pin).unwrap();

        let page = get_page_pinned_first(&db, 10, 0).unwrap();
        assert_eq!(page.len(), 3);
        assert!(
            page[0].pinned && page[1].pinned && page[2].pinned,
            "all three items are pinned"
        );
        // Explicit pin_order rows come first, in pin_order order.
        assert_eq!(page[0].id, p1_id, "pin_order=1.0 first");
        assert_eq!(page[1].id, p2_id, "pin_order=2.0 second");
        // The NULL pin_order row sorts LAST despite the newest wall_time.
        assert_eq!(
            page[2].id, null_pin_id,
            "NULL pin_order must sort after explicit pin_order values"
        );
        assert!(page[2].pin_order.is_none());
    }

    /// Unpinning an item moves it back into the unpinned group.
    #[test]
    fn pin_and_unpin_changes_sort_position() {
        let db = Database::open_in_memory().unwrap();

        let mut old = make_item(1);
        old.wall_time = 100;
        let old_id = old.id.clone();
        insert_item(&db, &old).unwrap();

        let mut new = make_item(2);
        new.wall_time = 200;
        insert_item(&db, &new).unwrap();

        // Pin the old item — it should appear first.
        pin_item(&db, &old_id).unwrap();
        let page = get_page_pinned_first(&db, 10, 0).unwrap();
        assert_eq!(page[0].id, old_id, "pinned old item must be first");

        // Unpin it — it should fall back to recency order (last).
        unpin_item(&db, &old_id).unwrap();
        let page2 = get_page_pinned_first(&db, 10, 0).unwrap();
        assert!(
            !page2[0].pinned,
            "after unpin, first item must not be pinned"
        );
        assert!(
            page2[0].wall_time >= page2[1].wall_time,
            "items must be newest-first after unpin"
        );
    }

    // --- FIX 2: bump_item_recency + content_hash dedup ---

    /// `bump_item_recency` updates wall_time and lamport_ts, returns 1 row changed.
    #[test]
    fn bump_item_recency_updates_wall_time_and_lamport() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.wall_time = 1_000;
        insert_item(&db, &item).unwrap();

        let changed = bump_item_recency(&db, &item.id, 99_000, 99_000).unwrap();
        assert_eq!(changed, 1, "one row must be updated");

        let fetched = get_item_by_id(&db, &item.id).unwrap().unwrap();
        assert_eq!(fetched.wall_time, 99_000, "wall_time must be bumped");
        assert_eq!(fetched.lamport_ts, 99_000, "lamport_ts must be bumped");
    }

    /// `bump_item_recency` returns 0 when id does not exist (no row updated).
    #[test]
    fn bump_item_recency_returns_zero_for_missing_id() {
        let db = Database::open_in_memory().unwrap();
        let changed = bump_item_recency(&db, "nonexistent-id", 999, 999).unwrap();
        assert_eq!(changed, 0, "no row matched; must return 0");
    }

    /// After `bump_item_recency`, the bumped item sorts to the top in
    /// `get_page_pinned_first` because its wall_time is now the newest.
    #[test]
    fn bumped_item_sorts_to_top() {
        let db = Database::open_in_memory().unwrap();

        // Three items, item_a is oldest.
        let mut item_a = make_item(1);
        item_a.wall_time = 100;
        let id_a = item_a.id.clone();
        insert_item(&db, &item_a).unwrap();

        let mut item_b = make_item(2);
        item_b.wall_time = 200;
        insert_item(&db, &item_b).unwrap();

        let mut item_c = make_item(3);
        item_c.wall_time = 300;
        insert_item(&db, &item_c).unwrap();

        // Bump item_a to wall_time=999 (the new highest).
        bump_item_recency(&db, &id_a, 999, 999).unwrap();

        let page = get_page_pinned_first(&db, 10, 0).unwrap();
        assert_eq!(page[0].id, id_a, "bumped item must appear at the top");
        assert_eq!(page[0].wall_time, 999);
    }

    /// Fix 4: `find_recent_by_hash` must not overflow when `now_ms < within_ms`
    /// (e.g. now_ms=0 and within_ms=i64::MAX). Before the fix, the subtraction
    /// `now_ms - within_ms` panics in debug builds.
    #[test]
    fn find_recent_by_hash_cutoff_no_overflow() {
        let db = Database::open_in_memory().unwrap();
        // now_ms=0, within_ms=i64::MAX → would overflow without saturating_sub.
        let result = find_recent_by_hash(&db, "anyhash", 0, i64::MAX);
        assert!(
            result.is_ok(),
            "must not panic or error on underflowing cutoff"
        );
        assert!(result.unwrap().is_none(), "empty db returns None");
    }

    /// Fix 3: `row_to_item` must return `CorruptKeyVersion` for out-of-range
    /// key_version values (e.g. 999 does not fit in u8 without silent truncation).
    #[test]
    fn row_to_item_corrupt_key_version_returns_error() {
        let db = Database::open_in_memory().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        // Insert a row with key_version=999 directly via SQL, bypassing insert_item's
        // ITEM_KEY_VERSION_CURRENT stamp.
        db.conn()
            .execute(
                "INSERT INTO clipboard_items
                 (id, item_id, content_type, content, content_nonce, blob_ref,
                  is_sensitive, is_synced, lamport_ts, wall_time, expires_at,
                  app_bundle_id, content_hash, origin_device_id, key_version, pinned)
                 VALUES (?1,?2,'text',NULL,NULL,NULL,0,0,1,1,NULL,NULL,NULL,'',999,0)",
                rusqlite::params![id, uuid::Uuid::new_v4().to_string()],
            )
            .unwrap();
        let result = get_item_by_id(&db, &id);
        assert!(
            matches!(result, Err(ItemsError::CorruptKeyVersion(999))),
            "expected CorruptKeyVersion(999), got: {result:?}"
        );
    }

    /// `find_recent_by_hash` finds a matching row when the window is wide open.
    #[test]
    fn find_recent_by_hash_finds_any_row_with_wide_window() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.content_hash = Some("aabbcc".to_string());
        item.wall_time = 1_000;
        insert_item(&db, &item).unwrap();

        // With i64::MAX window, any row with that hash should be found.
        let now_ms = i64::MAX / 2;
        let found = find_recent_by_hash(&db, "aabbcc", now_ms, i64::MAX).unwrap();
        assert_eq!(
            found,
            Some(item.id),
            "should find the row with matching hash"
        );
    }

    /// `find_recent_by_hash` returns None when no row has the given hash.
    #[test]
    fn find_recent_by_hash_returns_none_for_missing_hash() {
        let db = Database::open_in_memory().unwrap();
        let found = find_recent_by_hash(&db, "deadbeef", 99_000, i64::MAX).unwrap();
        assert!(found.is_none(), "no rows, must return None");
    }

    /// Dedup simulation: inserting the same content hash a second time via
    /// find_recent_by_hash + bump avoids a second row, and the bumped item
    /// sorts to the top.
    #[test]
    fn dedup_bump_prevents_duplicate_row_and_sorts_to_top() {
        let db = Database::open_in_memory().unwrap();

        // First capture: insert item with content_hash.
        let hash = "cafebabe".to_string();
        let mut item_first = make_item(1);
        item_first.wall_time = 1_000;
        item_first.content_hash = Some(hash.clone());
        let id_first = item_first.id.clone();
        insert_item(&db, &item_first).unwrap();

        // Insert a second, newer item so there are two rows total.
        let mut item_second = make_item(2);
        item_second.wall_time = 2_000;
        insert_item(&db, &item_second).unwrap();

        // "Second capture" of the same content: simulate the daemon dedup path.
        let now_ms: i64 = 9_999;
        let existing_id = find_recent_by_hash(&db, &hash, now_ms, i64::MAX)
            .unwrap()
            .expect("existing row must be found");
        assert_eq!(existing_id, id_first, "must find the original row");

        // Bump it.
        let changed = bump_item_recency(&db, &existing_id, now_ms, now_ms).unwrap();
        assert_eq!(changed, 1, "bump must affect one row");

        // Still only two rows total — no duplicate inserted.
        let total = count_items(&db).unwrap();
        assert_eq!(
            total, 2,
            "dedup must not insert a second row for the same hash"
        );

        // The bumped item now sorts to the top.
        let page = get_page_pinned_first(&db, 10, 0).unwrap();
        assert_eq!(
            page[0].id, id_first,
            "bumped item must appear first after recency update"
        );
        assert_eq!(page[0].wall_time, now_ms);
    }

    // ── prune_to_cap tests ────────────────────────────────────────────────────

    /// Build a ClipboardItem whose encrypted content is exactly `size` bytes,
    /// with a deterministic wall_time so tests can control eviction order.
    fn make_sized_item(lamport: i64, wall_time_ms: i64, size: usize) -> ClipboardItem {
        let mut item = make_item(lamport);
        item.wall_time = wall_time_ms;
        item.content = Some(vec![0xCC; size]);
        item
    }

    /// Under the quota: nothing deleted.
    #[test]
    fn prune_to_cap_no_op_when_under_quota() {
        let db = Database::open_in_memory().unwrap();
        // 3 items × 10 bytes = 30 bytes; quota = 100.
        for i in 0..3_i64 {
            insert_item(&db, &make_sized_item(i, i * 1_000, 10)).unwrap();
        }
        let deleted = prune_to_cap(&db, 100).unwrap();
        assert_eq!(deleted, 0, "no eviction when total < quota");
        assert_eq!(count_items(&db).unwrap(), 3);
    }

    /// Exactly at the quota: nothing deleted.
    #[test]
    fn prune_to_cap_no_op_when_exactly_at_quota() {
        let db = Database::open_in_memory().unwrap();
        // 5 items × 20 bytes = 100 bytes; quota = 100.
        for i in 0..5_i64 {
            insert_item(&db, &make_sized_item(i, i * 1_000, 20)).unwrap();
        }
        let deleted = prune_to_cap(&db, 100).unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(count_items(&db).unwrap(), 5);
    }

    /// Oldest items are evicted first (wall_time ASC ordering).
    #[test]
    fn prune_to_cap_evicts_oldest_first() {
        let db = Database::open_in_memory().unwrap();
        // Items ordered by wall_time: 1=oldest … 5=newest, each 20 bytes.
        // Total = 100, quota = 60 → excess = 40 → must remove 2 oldest (40 bytes).
        let mut ids = Vec::new();
        for i in 1..=5_i64 {
            let item = make_sized_item(i, i * 1_000, 20);
            ids.push(item.id.clone());
            insert_item(&db, &item).unwrap();
        }
        let deleted = prune_to_cap(&db, 60).unwrap();
        assert_eq!(deleted, 2, "exactly 2 oldest rows deleted");
        // Oldest two ids must be gone; newest three must remain.
        let conn = db.conn();
        let exists = |id: &str| -> bool {
            conn.query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
                params![id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
                > 0
        };
        assert!(!exists(&ids[0]), "oldest must be gone");
        assert!(!exists(&ids[1]), "second oldest must be gone");
        assert!(exists(&ids[2]), "third must remain");
        assert!(exists(&ids[3]), "fourth must remain");
        assert!(exists(&ids[4]), "newest must remain");
    }

    /// A `new_file` blob counts toward the byte cap exactly like text/image
    /// rows (`prune_to_cap` sums LENGTH(content) for all content types) and is
    /// evicted oldest-first.
    #[test]
    fn prune_to_cap_evicts_oldest_file_blob() {
        let db = Database::open_in_memory().unwrap();
        // Oldest row is a file blob (40 bytes); two newer text rows (20 each).
        // Total = 80, quota = 40 → must evict the oldest (the file) only.
        let mut file_item = ClipboardItem::new_file(vec![0xFFu8; 40], "{}".to_string(), 1);
        file_item.wall_time = 1_000;
        let file_id = file_item.id.clone();
        insert_item(&db, &file_item).unwrap();

        let mid = make_sized_item(2, 2_000, 20);
        let mid_id = mid.id.clone();
        insert_item(&db, &mid).unwrap();

        let newest = make_sized_item(3, 3_000, 20);
        let newest_id = newest.id.clone();
        insert_item(&db, &newest).unwrap();

        let deleted = prune_to_cap(&db, 40).unwrap();
        assert_eq!(deleted, 1, "only the oldest (file) row evicted");

        let conn = db.conn();
        let exists = |id: &str| -> bool {
            conn.query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
                params![id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
                > 0
        };
        assert!(!exists(&file_id), "oldest file blob must be evicted first");
        assert!(exists(&mid_id), "newer text row survives");
        assert!(exists(&newest_id), "newest text row survives");
    }

    /// The "tipping" row that crosses the byte threshold is evicted.
    #[test]
    fn prune_to_cap_tipping_row_is_evicted() {
        let db = Database::open_in_memory().unwrap();
        // 3 rows: 10 bytes, 10 bytes, 50 bytes (oldest → newest).
        // Total = 70, quota = 60 → excess = 10.
        // Row 1 (10 bytes): cum=10, cum-row=0 < 10 → DELETE (tipping).
        // Row 2 (10 bytes): cum=20, cum-row=10, 10 < 10 is FALSE → KEEP.
        let item1 = make_sized_item(1, 1_000, 10);
        let item2 = make_sized_item(2, 2_000, 10);
        let item3 = make_sized_item(3, 3_000, 50);
        let id1 = item1.id.clone();
        let id2 = item2.id.clone();
        let id3 = item3.id.clone();
        insert_item(&db, &item1).unwrap();
        insert_item(&db, &item2).unwrap();
        insert_item(&db, &item3).unwrap();

        let deleted = prune_to_cap(&db, 60).unwrap();
        assert_eq!(deleted, 1, "only the tipping row (oldest) deleted");
        let conn = db.conn();
        let exists = |id: &str| -> bool {
            conn.query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
                params![id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
                > 0
        };
        assert!(!exists(&id1), "tipping row deleted");
        assert!(exists(&id2), "row 2 kept");
        assert!(exists(&id3), "row 3 kept");
    }

    /// Pinned items are never evicted, even when they are the oldest.
    #[test]
    fn prune_to_cap_pinned_items_never_evicted() {
        let db = Database::open_in_memory().unwrap();
        // Pin the oldest item; its bytes must not count toward the quota.
        // 3 items × 20 bytes = 60 bytes. Quota = 30.
        // Unpinned bytes = 40 (rows 2 and 3). Excess = 10. Row 2 is evicted.
        let item1 = make_sized_item(1, 1_000, 20); // will be pinned
        let item2 = make_sized_item(2, 2_000, 20);
        let item3 = make_sized_item(3, 3_000, 20);
        let id1 = item1.id.clone();
        let id2 = item2.id.clone();
        let id3 = item3.id.clone();
        insert_item(&db, &item1).unwrap();
        insert_item(&db, &item2).unwrap();
        insert_item(&db, &item3).unwrap();
        pin_item(&db, &id1).unwrap();

        let deleted = prune_to_cap(&db, 30).unwrap();
        assert_eq!(deleted, 1, "one unpinned row evicted");
        let conn = db.conn();
        let exists = |id: &str| -> bool {
            conn.query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
                params![id],
                |r| r.get::<_, i64>(0),
            )
            .unwrap()
                > 0
        };
        assert!(exists(&id1), "pinned oldest must not be evicted");
        assert!(!exists(&id2), "oldest unpinned evicted");
        assert!(exists(&id3), "newest unpinned kept");
    }

    /// After `prune_to_cap` evicts rows, no orphan FTS rows must remain and
    /// a full-text search for a pruned term must return nothing.
    #[test]
    fn prune_to_cap_no_fts_orphans_after_eviction() {
        let db = Database::open_in_memory().unwrap();

        // Insert 3 items with FTS entries (oldest → newest, 20 bytes each).
        // Total = 60 bytes. Quota = 20 → excess = 40 → oldest 2 evicted.
        let mut ids = Vec::new();
        let terms = ["alpha unique term", "beta unique term", "gamma unique term"];
        for (i, term) in terms.iter().enumerate() {
            let item = make_sized_item(i as i64, (i as i64 + 1) * 1_000, 20);
            ids.push(item.id.clone());
            insert_item(&db, &item).unwrap();
            upsert_fts(&db, &item.id, term).unwrap();
        }

        let deleted = prune_to_cap(&db, 20).unwrap();
        assert_eq!(deleted, 2, "2 oldest items evicted");

        // No orphan FTS rows: count(fts) must equal count(items).
        let item_count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .unwrap();
        let fts_count: i64 = db
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_fts", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            fts_count, item_count,
            "clipboard_fts count ({fts_count}) must equal clipboard_items count \
             ({item_count}) — no orphan FTS rows after size-cap eviction"
        );

        // Ghost-search check: pruned terms must not appear in search results.
        let r_alpha = search_items(&db, "alpha", 10).unwrap();
        assert!(
            r_alpha.is_empty(),
            "pruned term 'alpha' must not appear in search results"
        );
        let r_beta = search_items(&db, "beta", 10).unwrap();
        assert!(
            r_beta.is_empty(),
            "pruned term 'beta' must not appear in search results"
        );

        // The surviving item must still be searchable.
        let r_gamma = search_items(&db, "gamma", 10).unwrap();
        assert_eq!(
            r_gamma.len(),
            1,
            "surviving item with term 'gamma' must still be found"
        );
        assert_eq!(r_gamma[0].id, ids[2]);
    }

    /// Empty database: prune is a no-op.
    #[test]
    fn prune_to_cap_empty_db_is_noop() {
        let db = Database::open_in_memory().unwrap();
        let deleted = prune_to_cap(&db, 1024).unwrap();
        assert_eq!(deleted, 0);
    }

    /// Items with NULL content (e.g. blob_ref-only rows) count as 0 bytes.
    #[test]
    fn prune_to_cap_null_content_counts_as_zero_bytes() {
        let db = Database::open_in_memory().unwrap();
        // One item with NULL content + one with 50 bytes. Total = 50. Quota = 40.
        // The NULL-content row is oldest; it contributes 0 bytes so cum_bytes
        // after it is 0, meaning cum-row=0 < 10 (excess) → it gets evicted first.
        // After evicting it: remaining = 50 bytes which still > 40. Then the
        // 50-byte row: cum=50, cum-row=0 < 10 → also evicted.
        // Wait — excess = 50 - 40 = 10. Row1 (0 bytes): cum=0, cum-row=0 < 10 → DELETE.
        // Row2 (50 bytes): cum=50, cum-row=0 < 10 → DELETE.
        // So both deleted, 0 remaining.  Let's redesign to make it meaningful:
        // NULL row (0b) at t=1, 50b at t=2. Total=50. Quota=50 → NO-OP.
        // NULL row (0b) at t=1, 50b at t=2. Total=50. Quota=49 → excess=1.
        // Row1: cum=0, 0-0=0 < 1 → DELETE; Row2: cum=50, 50-50=0 < 1 → DELETE.
        // Hmm — a NULL-content row always has cum-row=0 which is < any positive excess.
        // Use quota=50 for the no-op assertion:
        let mut item_null = make_item(1);
        item_null.wall_time = 1_000;
        item_null.content = None;
        let item_big = make_sized_item(2, 2_000, 50);
        insert_item(&db, &item_null).unwrap();
        insert_item(&db, &item_big).unwrap();
        // Quota exactly equals total (50). No prune.
        let deleted = prune_to_cap(&db, 50).unwrap();
        assert_eq!(deleted, 0, "no eviction when quota met");
        assert_eq!(count_items(&db).unwrap(), 2);
    }

    // --- CopyPaste-6fd: pending_uploads defensive cleanup ---

    /// Insert a `pending_uploads` row keyed by the given cross-device item_id.
    fn insert_pending_upload(db: &Database, item_id: &str) {
        db.conn()
            .execute(
                "INSERT INTO pending_uploads \
                 (item_id, tus_url, bytes_uploaded, total_bytes, chunk_format_version, \
                  created_at, expires_at) \
                 VALUES (?1, 'https://relay/tus/x', 0, 100, 1, 0, 0)",
                params![item_id],
            )
            .unwrap();
    }

    fn count_pending(db: &Database) -> i64 {
        db.conn()
            .query_row("SELECT COUNT(*) FROM pending_uploads", [], |r| r.get(0))
            .unwrap()
    }

    /// `delete_item` must also remove the matching `pending_uploads` row so a
    /// hard-deleted item can never strand a resumable-upload row.
    #[test]
    fn delete_item_cleans_pending_uploads() {
        let db = Database::open_in_memory().unwrap();
        let item = make_item(1);
        insert_item(&db, &item).unwrap();
        insert_pending_upload(&db, &item.item_id);
        // A second unrelated pending row must survive.
        insert_pending_upload(&db, "other-item-id");
        assert_eq!(count_pending(&db), 2);

        delete_item(&db, &item.id).unwrap();

        assert_eq!(
            count_pending(&db),
            1,
            "only the deleted item's pending_uploads row is removed"
        );
        let survivor: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM pending_uploads WHERE item_id = 'other-item-id'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(survivor, 1, "unrelated pending_uploads row must survive");
    }

    /// `prune_to_cap` eviction must also clean `pending_uploads` for evicted ids.
    #[test]
    fn prune_to_cap_cleans_pending_uploads() {
        let db = Database::open_in_memory().unwrap();
        // 3 × 20 bytes = 60. Quota = 20 → 2 oldest evicted.
        let mut items = Vec::new();
        for i in 0..3 {
            let item = make_sized_item(i, (i + 1) * 1_000, 20);
            insert_item(&db, &item).unwrap();
            insert_pending_upload(&db, &item.item_id);
            items.push(item);
        }
        assert_eq!(count_pending(&db), 3);

        let deleted = prune_to_cap(&db, 20).unwrap();
        assert_eq!(deleted, 2);

        // Only the surviving (newest) item keeps its pending_uploads row.
        assert_eq!(
            count_pending(&db),
            1,
            "evicted items' pending_uploads rows must be cleaned"
        );
        let surviving_iid = &items[2].item_id;
        let survivor: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM pending_uploads WHERE item_id = ?1",
                params![surviving_iid],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(survivor, 1, "surviving item keeps its pending_uploads row");
    }

    /// `delete_expired` (TTL prune) must also clean `pending_uploads`.
    #[test]
    fn delete_expired_cleans_pending_uploads() {
        let db = Database::open_in_memory().unwrap();
        let mut item = make_item(1);
        item.expires_at = Some(1_000); // already expired vs now=10_000
        insert_item(&db, &item).unwrap();
        insert_pending_upload(&db, &item.item_id);
        assert_eq!(count_pending(&db), 1);

        let removed = delete_expired(&db, 10_000).unwrap();
        assert_eq!(removed, 1);
        assert_eq!(
            count_pending(&db),
            0,
            "TTL-expired item's pending_uploads row must be cleaned"
        );
    }

    /// Large dataset (50 rows): window-function rewrite produces identical
    /// eviction to the reference naive algorithm (compute total, subtract quota,
    /// delete oldest prefix summing to ≥ excess).
    #[test]
    fn prune_to_cap_large_dataset_matches_naive_eviction() {
        use std::collections::HashSet;

        let db = Database::open_in_memory().unwrap();

        // Insert 50 items with varying sizes (5..=54 bytes) and distinct
        // wall_times so the ordering is deterministic.
        let mut items: Vec<ClipboardItem> = (0..50_i64)
            .map(|i| make_sized_item(i, (i + 1) * 1_000, 5 + i as usize))
            .collect();
        for item in &items {
            insert_item(&db, item).unwrap();
        }

        // Pin the 3 most-recent items so they survive unconditionally.
        let pinned_ids: HashSet<String> = items[47..].iter().map(|i| i.id.clone()).collect();
        for id in &pinned_ids {
            pin_item(&db, id).unwrap();
        }

        // Total bytes (items is sorted oldest-first, sizes 5..54 bytes).
        // Unpinned = items[0..47]; total_unpinned = sum(5..52) = 47*(5+51)/2 = 1316.
        let total_unpinned: i64 = items[..47]
            .iter()
            .map(|it| it.content.as_ref().map_or(0, |c| c.len() as i64))
            .sum();
        let quota: i64 = 800;
        let excess = total_unpinned - quota;
        assert!(excess > 0, "sanity: quota must be below total");

        // Naive reference: collect oldest-first ids until cumulative bytes >= excess.
        items[..47].sort_by_key(|it| (it.wall_time, it.id.clone()));
        let mut cum: i64 = 0;
        let mut naive_delete: HashSet<String> = HashSet::new();
        for it in &items[..47] {
            let row_bytes = it.content.as_ref().map_or(0, |c| c.len() as i64);
            if cum < excess {
                naive_delete.insert(it.id.clone());
                cum += row_bytes;
            }
        }

        // Run prune_to_cap.
        let deleted = prune_to_cap(&db, quota).unwrap();
        assert_eq!(
            deleted,
            naive_delete.len(),
            "window-fn and naive must delete the same number of rows"
        );

        // Verify each id matches.
        let conn = db.conn();
        for id in &naive_delete {
            let found: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(found, 0, "naive-evicted row {id} must be gone");
        }
        // Pinned items must still be present.
        for id in &pinned_ids {
            let found: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM clipboard_items WHERE id=?1",
                    params![id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(found, 1, "pinned row {id} must remain");
        }
    }
}
