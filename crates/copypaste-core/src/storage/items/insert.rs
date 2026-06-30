use super::super::db::{Database, MigrationState};
use super::types::{validate_key_version, ClipboardItem, ItemsError};
use super::ITEM_KEY_VERSION_CURRENT;
use rusqlite::params;

/// CopyPaste-crh3.83: single source of truth for the 19-column clipboard_items
/// INSERT column list, previously duplicated verbatim in three insert functions
/// (a missed edit when adding a column silently corrupts positional writes). The
/// column ORDER must stay aligned with each call's `params!`/VALUES list and with
/// `row_to_item`'s SELECT order.
const ITEM_INSERT_COLUMNS: &str = "(id, item_id, content_type, content, content_nonce, blob_ref, \
     is_sensitive, is_synced, lamport_ts, wall_time, expires_at, app_bundle_id, \
     content_hash, origin_device_id, key_version, pinned, pin_order, thumb, deleted)";

pub fn insert_item(db: &Database, item: &ClipboardItem) -> Result<(), ItemsError> {
    // Gate: reject writes while the v4 key-version sweep is running so that
    // no key_version=2 row can corrupt the cursor-based resume (last_processed_id).
    if matches!(db.migration_state()?, MigrationState::InProgress { .. }) {
        return Err(ItemsError::MigrationInProgress);
    }
    let key_version = validate_key_version(item.key_version)?;
    // CopyPaste-44rq.49 (SECURITY): never persist a thumbnail for a sensitive item.
    // A thumbnail is a recognisable, scaled-down preview of the captured image.
    // Storing it — even encrypted — means that a future UI bug or an AAD mismatch
    // between thumb_file_id values could expose a visual preview of sensitive
    // content (e.g. a screenshot from a password-manager screen).
    // Erring toward NOT storing is the correct policy; the thumb can never be
    // backfilled for a sensitive item either (see `set_thumb`).
    let thumb: Option<&[u8]> = if item.is_sensitive {
        None
    } else {
        item.thumb.as_deref()
    };
    db.conn().execute(
        &format!(
            "INSERT INTO clipboard_items {ITEM_INSERT_COLUMNS} \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)"
        ),
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
            thumb,
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
    // CopyPaste-44rq.49 (SECURITY): same sensitive-thumb suppression as `insert_item`.
    // Both insert paths must apply the guard so neither can be used to sneak a
    // thumbnail past the policy (e.g. via a sync-reconstructed item whose caller
    // sets is_sensitive=true but forgets to clear thumb before calling this function).
    let thumb: Option<&[u8]> = if item.is_sensitive {
        None
    } else {
        item.thumb.as_deref()
    };
    let conn = db.conn();
    let tx = conn.unchecked_transaction()?;
    let insert_res = tx.execute(
        &format!(
            "INSERT INTO clipboard_items {ITEM_INSERT_COLUMNS} \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)"
        ),
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
            thumb,
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

    // CopyPaste-i6pp: never index sensitive items into the FTS table.
    // Sensitive items contain secrets (passwords, tokens, PII); storing them
    // as plaintext in clipboard_fts — even under SQLCipher encryption — widens
    // the attack surface and leaks secrets via search results to any caller
    // that invokes `search_items`. The guard here is defense-in-depth: callers
    // (the daemon's `handle_text`) are expected to pass `""` for sensitive
    // items, but we enforce the policy regardless of what `plaintext_for_fts`
    // contains so a future caller cannot accidentally index secret content.
    if !plaintext_for_fts.is_empty() && !item.is_sensitive {
        tx.execute("DELETE FROM clipboard_fts WHERE id = ?1", params![item.id])?;
        tx.execute(
            "INSERT INTO clipboard_fts(id, content_text) VALUES (?1, ?2)",
            params![item.id, plaintext_for_fts],
        )?;
    }
    tx.commit()?;
    Ok(item.id.as_str().to_owned())
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
            // CopyPaste-fuxl: `AND deleted = 0` — never dedup against a soft-deleted
            // tombstone, so a re-copy within the same minute bucket inserts a fresh
            // live row (the UNIQUE index now also excludes deleted=1 rows).
            "SELECT id FROM clipboard_items
             WHERE content_hash = ?1 AND (wall_time / 60) = ?2 AND deleted = 0
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
/// it is *strictly newer*, honouring the [`crate::soft_delete_item`] "an inbound delete
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
        &format!(
            "INSERT INTO clipboard_items {ITEM_INSERT_COLUMNS} \
             VALUES (?1, ?2, 'text', NULL, NULL, NULL, \
                     0, 1, ?3, ?4, NULL, NULL, \
                     NULL, ?5, ?6, 0, NULL, NULL, 1)"
        ),
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
