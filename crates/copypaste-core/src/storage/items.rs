use super::db::{Database, DbError, MigrationState};
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
    pub fn new_image(encrypted_blob: Vec<u8>, image_meta_json: String, lamport_ts: i64) -> Self {
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
          content_hash, origin_device_id, key_version, pinned)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
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
/// TODO(daemon-owner): existing daemon ingest paths still call
/// `insert_item` + `upsert_fts` as two separate steps. Switch to this new
/// fn to close the crash window.
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
          content_hash, origin_device_id, key_version, pinned)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16)",
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
    let cutoff = now_ms - within_ms;
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

pub fn get_page(
    db: &Database,
    limit: usize,
    offset: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned
         FROM clipboard_items ORDER BY wall_time DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit as i64, offset as i64], row_to_item)?
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
pub fn get_page_pinned_first(
    db: &Database,
    limit: usize,
    offset: usize,
) -> Result<Vec<ClipboardItem>, ItemsError> {
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned
         FROM clipboard_items ORDER BY pinned DESC, wall_time DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit as i64, offset as i64], row_to_item)?
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
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content_type, NULL AS content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned
         FROM clipboard_items ORDER BY wall_time DESC LIMIT ?1 OFFSET ?2",
    )?;
    let items = stmt
        .query_map(params![limit as i64, offset as i64], row_to_item)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(items)
}

/// Fetch a single clipboard item by its primary-key `id`.
///
/// Returns `Ok(None)` when no row matches. Used by IPC verbs such as
/// `copy_item` that resolve an item directly by id — this avoids the
/// data-loss footgun of paging (`get_page`) and linear-scanning, which
/// silently misses any item beyond the fetched page window.
pub fn get_item_by_id(db: &Database, id: &str) -> Result<Option<ClipboardItem>, ItemsError> {
    let result = db.conn().query_row(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned
         FROM clipboard_items WHERE id = ?1",
        params![id],
        row_to_item,
    );
    match result {
        Ok(item) => Ok(Some(item)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
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
    let result = db.conn().query_row(
        "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id,
                content_hash, origin_device_id, key_version, pinned
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

pub fn delete_expired(db: &Database, now_ms: i64) -> Result<usize, ItemsError> {
    let changed = db.conn().execute(
        "DELETE FROM clipboard_items WHERE expires_at IS NOT NULL AND expires_at < ?1 AND pinned = 0",
        params![now_ms],
    )?;
    Ok(changed)
}

/// Delete sensitive items whose `wall_time` is older than `sensitive_ttl_ms` milliseconds ago.
/// This enforces a local auto-wipe TTL for items marked `is_sensitive = 1`.
pub fn delete_sensitive_expired(
    db: &Database,
    now_ms: i64,
    sensitive_ttl_ms: i64,
) -> Result<usize, ItemsError> {
    let threshold = now_ms - sensitive_ttl_ms;
    let changed = db.conn().execute(
        // `AND pinned = 0` mirrors `delete_expired`: pinned items are exempt from
        // every TTL prune (see ClipboardItem::pinned docs). Without this guard a
        // pinned+sensitive item is silently wiped after the sensitive TTL,
        // violating the pin contract.
        "DELETE FROM clipboard_items WHERE is_sensitive = 1 AND wall_time < ?1 AND pinned = 0",
        params![threshold],
    )?;
    Ok(changed)
}

/// Delete the clipboard item with the given primary-key `id`.
///
/// Returns the number of rows actually removed (`0` when no row matched).
/// Callers can use this to distinguish a real deletion from a no-op against a
/// non-existent id.
pub fn delete_item(db: &Database, id: &str) -> Result<usize, ItemsError> {
    let removed = db
        .conn()
        .execute("DELETE FROM clipboard_items WHERE id=?1", params![id])?;
    Ok(removed)
}

/// Pin an item so it is never auto-deleted by TTL or history-limit prunes.
///
/// Sets `pinned = 1` and clears `expires_at` so the item survives both
/// `delete_expired` and `prune_history`.
pub fn pin_item(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn().execute(
        "UPDATE clipboard_items SET pinned = 1, expires_at = NULL WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}

/// Unpin a previously pinned item, restoring normal TTL and history-limit
/// behaviour. Sets `pinned = 0`; `expires_at` remains `NULL` unless the
/// caller explicitly sets a new expiry.
pub fn unpin_item(db: &Database, id: &str) -> Result<(), ItemsError> {
    db.conn().execute(
        "UPDATE clipboard_items SET pinned = 0 WHERE id = ?1",
        rusqlite::params![id],
    )?;
    Ok(())
}

pub fn count_items(db: &Database) -> Result<i64, ItemsError> {
    Ok(db
        .conn()
        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))?)
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
pub fn fetch_text_preview(db: &Database, id: &str) -> Result<Option<String>, ItemsError> {
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
    // Keep only alphanum, underscore, hyphen, quote, asterisk, and whitespace.
    let cleaned: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '"' | '*' | ' ' | '\t'))
        .collect();

    let trimmed = cleaned.trim();
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

    // Multi-word input: split into tokens, join with AND, suffix-prefix the last token.
    let tokens: Vec<&str> = trimmed.split_whitespace().collect();
    // tokens is non-empty because trimmed is non-empty.
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
pub fn search_items(
    db: &Database,
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
                ci.app_bundle_id, ci.content_hash, ci.origin_device_id, ci.key_version, ci.pinned
         FROM clipboard_fts fts
         JOIN clipboard_items ci ON ci.id = fts.id
         WHERE clipboard_fts MATCH ?1
         ORDER BY rank
         LIMIT ?2",
    )?;

    let rows: Vec<ClipboardItem> = stmt
        .query_map(params![safe_query, limit as i64], row_to_item)?
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
        key_version: row.get::<_, i64>(14)? as u8,
        pinned: row.get::<_, i64>(15)? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::db::Database;

    fn make_item(lamport: i64) -> ClipboardItem {
        ClipboardItem::new_text(vec![0xAA, 0xBB], vec![0u8; 24], lamport)
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
    #[test]
    fn get_page_pinned_first_multiple_pins_sorted_by_recency() {
        let db = Database::open_in_memory().unwrap();

        let mut old_pin = make_item(1);
        old_pin.wall_time = 100;
        let old_pin_id = old_pin.id.clone();
        insert_item(&db, &old_pin).unwrap();
        pin_item(&db, &old_pin_id).unwrap();

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
        // Both pins first, sorted newest-first within the pinned group.
        assert!(
            page[0].pinned && page[1].pinned,
            "first two items must be pinned"
        );
        assert!(
            page[0].wall_time >= page[1].wall_time,
            "pins must be newest-first within pin group"
        );
        // Then unpinned.
        assert!(!page[2].pinned, "third item must not be pinned");
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
}
