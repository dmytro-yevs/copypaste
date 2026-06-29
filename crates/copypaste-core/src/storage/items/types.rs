use super::super::db::DbError;
use super::ids::{ItemId, RowId};
use thiserror::Error;

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
pub(super) fn validate_key_version(key_version: u8) -> Result<i64, ItemsError> {
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
pub(crate) fn now_ms_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

#[derive(Debug, Clone)]
pub struct ClipboardItem {
    /// Local DB row primary key. See [`RowId`].
    pub id: RowId,
    /// Cross-device logical identity, bound into the AEAD AAD. See [`ItemId`].
    pub item_id: ItemId,
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
            id: RowId(uuid::Uuid::new_v4().to_string()),
            item_id: ItemId(uuid::Uuid::new_v4().to_string()),
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
            key_version: super::ITEM_KEY_VERSION_CURRENT as u8,
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
            id: RowId(uuid::Uuid::new_v4().to_string()),
            item_id: ItemId(uuid::Uuid::new_v4().to_string()),
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
            key_version: super::ITEM_KEY_VERSION_CURRENT as u8,
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
            id: RowId(uuid::Uuid::new_v4().to_string()),
            item_id: ItemId(uuid::Uuid::new_v4().to_string()),
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
            key_version: super::ITEM_KEY_VERSION_CURRENT as u8,
            pinned: false,
            pin_order: None,
            thumb: None,
            deleted: false,
        }
    }
}

pub(super) fn row_to_item(row: &rusqlite::Row) -> rusqlite::Result<ClipboardItem> {
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
