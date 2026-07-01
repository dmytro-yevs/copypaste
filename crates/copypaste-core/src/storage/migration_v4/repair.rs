//! Mislabeled kv=2 blob repair cluster (closes writer bug).
//!
//! Before the writer fix, `daemon::handle_image` and `handle_file` encrypted
//! chunks with the RAW v1 seed key but stamped `key_version = 2` on the row
//! (the `ClipboardItem::new_image` / `new_file` constructors always stamp
//! `ITEM_KEY_VERSION_CURRENT = 2`). The `WHERE key_version = 1` predicate in
//! [`super::images::migrate_v1_image_chunks_to_v2`] never saw these rows, so
//! they stayed "mislabeled": encrypted-with-v1 but marked kv=2.
//!
//! After the writer fix every new image/file row is genuinely v2-encrypted.
//! But existing mislabeled rows need a one-time repair: try v1-decrypt; on
//! success the row is mislabeled — re-encrypt with v2 and bump `content` (the
//! `key_version` stays 2, matching the stamp). On v1-decrypt failure the row
//! is correctly v2-encrypted — skip it.
//!
//! The sweep is idempotent: after repair the row is truly v2-encrypted, so
//! the v1-decrypt probe fails on the next run and the row is skipped.

use super::images::{parse_file_id, v2_blob_from_chunks};
use super::{Database, MigrationV4Error, BATCH_SIZE, INTER_BATCH_SLEEP};
use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks};
use crate::image::{chunks_from_blob, IMAGE_CHUNK_SIZE};
use rusqlite::params;

/// Minimal projection of a candidate mislabeled kv=2 blob row.
pub(super) struct Kv2BlobRow {
    /// SQLite implicit integer rowid — used as a stable pagination cursor.
    /// Unlike `wall_time` (which may be duplicated) or `id` (a UUID string
    /// that is not numerically ordered), `rowid` is a unique monotonic integer
    /// that SQLite guarantees for every non-WITHOUT-ROWID table.  Using it as
    /// a `WHERE rowid > last_cursor` cursor is O(log N) on the primary-key
    /// B-tree and produces deterministic, non-overlapping pages.
    pub(super) rowid: i64,
    id: String,
    // item_id is read from the DB for structural consistency with V1ImageRow
    // but not used in the repair logic (file_id comes from blob_ref JSON).
    #[allow(dead_code)]
    item_id: String,
    content: Vec<u8>,
    blob_ref: Option<String>,
}

/// Fetch at most `BATCH_SIZE` candidate kv=2 blob rows whose `rowid` is
/// strictly greater than `after_rowid` (the pagination cursor).
///
/// `ORDER BY rowid ASC LIMIT BATCH_SIZE` guarantees:
/// * each page is a disjoint, non-overlapping window (cursor moves forward),
/// * no row can be skipped or repeated between pages, and
/// * the cursor is stable even if rows are UPDATEd between pages
///   (re-encryption writes the same `key_version = 2` and does not change
///   `rowid`; the predicate `WHERE rowid > last_cursor` will therefore
///   skip any already-visited rows on the next fetch).
///
/// `pub(super)` so the characterization test `repair_processes_multi_batch_in_pages`
/// can assert the page size directly.
pub(super) fn fetch_kv2_blob_batch(
    db: &Database,
    after_rowid: i64,
) -> Result<Vec<Kv2BlobRow>, MigrationV4Error> {
    let mut stmt = db.conn().prepare(
        "SELECT rowid, id, item_id, content, blob_ref \
         FROM clipboard_items \
         WHERE key_version = 2 \
           AND content IS NOT NULL \
           AND content_type IN ('image', 'file') \
           AND rowid > ?1 \
         ORDER BY rowid ASC \
         LIMIT ?2",
    )?;
    let rows = stmt
        .query_map(params![after_rowid, BATCH_SIZE as i64], |r| {
            Ok(Kv2BlobRow {
                rowid: r.get(0)?,
                id: r.get(1)?,
                item_id: r.get(2)?,
                content: r.get(3)?,
                blob_ref: r.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Try to repair a single mislabeled kv=2 blob row.
///
/// Attempts v1-decrypt of the chunk blob. If it succeeds the row was
/// encrypted with the v1 key but stamped kv=2 (the pre-fix writer bug) —
/// re-encrypt with the v2 key and update `content` in place (`key_version`
/// stays 2). If v1-decrypt fails the row is correctly v2-encrypted — skip.
///
/// Returns `Ok(true)` if the row was repaired, `Ok(false)` if it was
/// already correct (v1-decrypt failed), or `Err` for structural failures
/// (blob parse, metadata missing, re-encrypt failure).
fn maybe_repair_one_kv2_blob(
    db: &Database,
    row: &Kv2BlobRow,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
) -> Result<bool, MigrationV4Error> {
    let file_id = parse_file_id(&row.id, row.blob_ref.as_deref())?;

    let chunks = chunks_from_blob(&row.content).map_err(|e| MigrationV4Error::ImageBlob {
        id: row.id.clone(),
        source: e,
    })?;

    // Probe: try v1-decrypt. Failure → row is correctly v2 → skip.
    let plaintext = match decrypt_chunks(&chunks, v1_key, &file_id) {
        Ok(pt) => pt,
        Err(_) => return Ok(false), // correctly v2-encrypted, nothing to do
    };

    // v1-decrypt succeeded → row is mislabeled. Re-encrypt with v2 key.
    let v2_chunks =
        encrypt_chunks(&plaintext, v2_key, &file_id, IMAGE_CHUNK_SIZE).map_err(|e| {
            MigrationV4Error::ImageChunkEncrypt {
                id: row.id.clone(),
                source: e,
            }
        })?;
    let v2_blob = v2_blob_from_chunks(&row.id, &v2_chunks)?;

    // Update content only — key_version stays 2 (now accurate).
    // The WHERE re-asserts key_version=2 so a concurrent bump cannot be
    // silently overwritten.
    db.conn().execute(
        "UPDATE clipboard_items \
         SET content = ?1 \
         WHERE id = ?2 AND key_version = 2",
        params![v2_blob, row.id],
    )?;

    Ok(true)
}

/// Repair image/file rows that were mislabeled: encrypted with the v1 key
/// but stamped `key_version = 2` by the pre-fix writer.
///
/// For each candidate row (`content_type IN ('image','file') AND key_version = 2`):
/// * Try v1-decrypt (chunk decrypt with the v1 key).
/// * If it **succeeds**: the row is mislabeled — re-encrypt with the v2 key
///   and update `content` in place. `key_version` stays 2 (now accurate).
/// * If it **fails**: the row is correctly v2-encrypted — leave it alone.
///
/// Returns the number of rows that were actually repaired (re-encrypted).
/// The function is idempotent: repaired rows fail the v1-decrypt probe on
/// subsequent runs and are silently skipped.
///
/// ## Batching
///
/// The repair streams through all kv=2 blob rows in pages of [`BATCH_SIZE`]
/// using a `rowid`-based cursor (`WHERE rowid > last_cursor ORDER BY rowid
/// ASC LIMIT BATCH_SIZE`).  This bounds peak memory to `BATCH_SIZE × (row
/// overhead + encrypted content blob size)` regardless of how many image/file
/// rows the database contains, and yields [`INTER_BATCH_SLEEP`] between pages
/// so long-running repairs do not monopolise the SQLite write lock.
///
/// Rows that fail the v1-decrypt probe are skipped in-place; rows that fail
/// re-encryption are logged and left unchanged (they remain readable with the
/// v2 key if they were truly v2).
pub fn repair_mislabeled_kv2_blob_rows(
    db: &Database,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
) -> Result<usize, MigrationV4Error> {
    let mut total_repaired = 0usize;
    // `rowid` cursor — start before the first possible row (rowid >= 1).
    let mut cursor: i64 = 0;

    loop {
        let batch = fetch_kv2_blob_batch(db, cursor)?;
        if batch.is_empty() {
            break;
        }

        // Advance cursor to the last rowid in this batch so the next fetch
        // starts strictly after it.  The batch is ORDER BY rowid ASC so the
        // last element always has the highest rowid.
        //
        // Unwrap is safe: we just checked `!batch.is_empty()`.
        cursor = batch.last().unwrap().rowid;

        for row in &batch {
            match maybe_repair_one_kv2_blob(db, row, v1_key, v2_key) {
                Ok(true) => {
                    total_repaired += 1;
                    tracing::debug!(
                        row_id = %row.id,
                        "repair_mislabeled_kv2: repaired mislabeled kv2 blob row"
                    );
                }
                Ok(false) => {
                    // Correctly v2-encrypted; skip.
                }
                Err(e) => {
                    tracing::warn!(
                        row_id = %row.id,
                        error = %e,
                        "repair_mislabeled_kv2: could not repair row (left unchanged)"
                    );
                }
            }
        }

        // If the batch was smaller than BATCH_SIZE we've reached the end.
        if batch.len() < BATCH_SIZE {
            break;
        }

        // Yield between pages to avoid holding the write lock for an
        // unbounded duration on large databases.
        std::thread::sleep(INTER_BATCH_SLEEP);
    }

    if total_repaired > 0 {
        tracing::info!(
            repaired = total_repaired,
            "repair_mislabeled_kv2: repaired {total_repaired} mislabeled kv2 blob row(s)"
        );
    }
    Ok(total_repaired)
}
