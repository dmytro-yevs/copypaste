//! Image-chunk v1 → v2 key rotation cluster (closes Cnew).
//!
//! Image items store their content as a per-chunk encrypted blob (see
//! [`crate::crypto::chunks`]); the per-chunk AAD binds `(CHUNK_FORMAT_V1,
//! file_id, chunk_index, total_chunks, is_final)` but NOT `key_version`, so
//! the row's `key_version` column is the authoritative record of which HKDF
//! key generation decrypts the chunks. This module fetches batches of
//! `key_version = 1` image rows, decrypts each chunk with the v1 key,
//! re-encrypts with the v2 key (fresh per-chunk nonces), re-serialises the
//! blob, and stamps `key_version = 2`.

use super::{Database, MigrationV4Error, BATCH_SIZE, INTER_BATCH_SLEEP, KEY_VERSION_V2};
use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks, ChunkError, EncryptedChunk};
use crate::image::{chunks_from_blob, chunks_to_blob, ImageError, IMAGE_CHUNK_SIZE};
use crate::storage::items::ItemId;
use rusqlite::params;

/// Minimal projection of a v1-key image row needed for chunk re-encryption.
/// Image rows have no item-level `content_nonce` (nonces live per-chunk inside
/// the `content` blob); the `file_id` AAD context is carried in `blob_ref`.
pub(super) struct V1ImageRow {
    id: String,
    item_id: ItemId,
    content: Vec<u8>,
    blob_ref: Option<String>,
}

/// Run the v1 → v2 chunk-key rotation across every image row still at
/// `key_version = 1`.
///
/// Mirrors [`super::migrate_v1_to_v2_keys`] (text sweep): batched, crash-safe,
/// idempotent (rows already at `key_version = 2` are filtered out by the
/// `WHERE key_version = 1` predicate). A row that fails to decrypt or whose
/// metadata is unusable is left at `key_version = 1` and logged — it must not
/// abort the sweep.
///
/// Returns the number of image rows successfully re-encrypted.
pub fn migrate_v1_image_chunks_to_v2(
    db: &Database,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
) -> Result<usize, MigrationV4Error> {
    let mut total_rotated = 0usize;
    let mut total_failed = 0usize;

    loop {
        let batch = fetch_v1_image_batch(db, BATCH_SIZE)?;
        if batch.is_empty() {
            break;
        }
        let batch_len = batch.len();
        let mut rotated_this_batch = 0usize;

        for row in batch {
            match rotate_one_image(db, &row, v1_key, v2_key) {
                Ok(()) => {
                    total_rotated += 1;
                    rotated_this_batch += 1;
                }
                Err(e) => {
                    total_failed += 1;
                    tracing::warn!(
                        item_id = %row.item_id,
                        row_id = %row.id,
                        error = %e,
                        "migration_v4: image row left at key_version=1 (decrypt or re-encrypt failed)"
                    );
                }
            }
        }

        // Termination guard (same defect as the text sweep): a full batch that
        // rotated ZERO rows would be re-fetched verbatim by the next
        // `WHERE key_version = 1` query, looping forever. Stop and leave them
        // at v1 rather than hang daemon startup.
        if rotated_this_batch == 0 && batch_len == BATCH_SIZE {
            tracing::warn!(
                stuck = batch_len,
                "migration_v4: a full batch of {batch_len} image rows all failed to rotate; \
                 leaving them at key_version=1 and stopping the image sweep to avoid an infinite loop"
            );
            break;
        }

        if batch_len < BATCH_SIZE {
            break;
        }
        std::thread::sleep(INTER_BATCH_SLEEP);
    }

    tracing::info!(
        rotated = total_rotated,
        failed = total_failed,
        "migration_v4: image-chunk sweep complete"
    );
    Ok(total_rotated)
}

fn fetch_v1_image_batch(db: &Database, limit: usize) -> Result<Vec<V1ImageRow>, MigrationV4Error> {
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content, blob_ref \
         FROM clipboard_items \
         WHERE key_version = 1 \
           AND content IS NOT NULL \
           AND content_type = 'image' \
         ORDER BY wall_time ASC \
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(V1ImageRow {
                id: r.get(0)?,
                item_id: r.get(1)?,
                content: r.get(2)?,
                blob_ref: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

/// Parse the 16-byte `file_id` out of an image row's `blob_ref` JSON.
///
/// The metadata shape is produced by `daemon::handle_image`:
/// `{"width":W,"height":H,"original_size":N,"chunk_count":C,"file_id":[u8;16]}`
/// where `file_id` is serialised as a JSON array of 16 unsigned integers via
/// Rust's `{:?}` debug format (e.g. `"file_id":[12, 34, ...]`).
///
/// Uses `serde_json` (a production dependency of this crate) to parse the full
/// JSON object and extract the `file_id` array. This is more robust than a
/// hand-rolled substring scanner: field reordering, extra whitespace, or new
/// metadata fields added to `blob_ref` in the future will not break extraction.
///
/// `pub(super)` because [`super::repair::maybe_repair_one_kv2_blob`] also
/// needs it to recover the AAD context for the mislabeled-kv2 repair probe.
pub(super) fn parse_file_id(id: &str, blob_ref: Option<&str>) -> Result<[u8; 16], MigrationV4Error> {
    let meta_json = blob_ref.ok_or_else(|| MigrationV4Error::ImageMeta {
        id: id.to_string(),
        reason: "missing blob_ref metadata".to_string(),
    })?;
    let err = |reason: String| MigrationV4Error::ImageMeta {
        id: id.to_string(),
        reason,
    };

    let v: serde_json::Value = serde_json::from_str(meta_json)
        .map_err(|e| err(format!("blob_ref is not valid JSON: {e}")))?;

    let arr = v
        .get("file_id")
        .and_then(|f| f.as_array())
        .ok_or_else(|| err("blob_ref missing 'file_id' array".to_string()))?;

    if arr.len() != 16 {
        return Err(err(format!(
            "'file_id' has wrong length: expected 16, got {}",
            arr.len()
        )));
    }

    let mut out = [0u8; 16];
    for (i, elem) in arr.iter().enumerate() {
        let n = elem
            .as_u64()
            .ok_or_else(|| err(format!("'file_id[{i}]' is not an unsigned integer: {elem}")))?;
        if n > 255 {
            return Err(err(format!("'file_id[{i}]' value {n} exceeds u8 range")));
        }
        out[i] = n as u8;
    }

    Ok(out)
}

/// Build the v2 on-disk chunk blob from freshly re-encrypted chunks, mapping
/// `chunks_to_blob`'s `ImageError` down to the `ChunkError` carried by
/// `MigrationV4Error::ImageChunkEncrypt`.
///
/// Extracted because the exact same error-mapping block was duplicated
/// verbatim in [`rotate_one_image`] and
/// [`super::repair::maybe_repair_one_kv2_blob`] (CopyPaste-vp63.21 dedup).
pub(super) fn v2_blob_from_chunks(
    id: &str,
    chunks: &[EncryptedChunk],
) -> Result<Vec<u8>, MigrationV4Error> {
    chunks_to_blob(chunks).map_err(|e| {
        // chunks_to_blob only errors with TooManyChunks; extract the inner ChunkError.
        let source = match e {
            ImageError::Chunk(ce) => ce,
            _ => ChunkError::TooManyChunks,
        };
        MigrationV4Error::ImageChunkEncrypt {
            id: id.to_string(),
            source,
        }
    })
}

fn rotate_one_image(
    db: &Database,
    row: &V1ImageRow,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
) -> Result<(), MigrationV4Error> {
    let file_id = parse_file_id(&row.id, row.blob_ref.as_deref())?;

    // Parse the on-disk chunk blob back into chunks.
    let chunks = chunks_from_blob(&row.content).map_err(|e| MigrationV4Error::ImageBlob {
        id: row.id.clone(),
        source: e,
    })?;

    // Decrypt all chunks with the v1 key. The per-chunk AAD binds
    // `(CHUNK_FORMAT_V1, file_id, chunk_index, total_chunks, is_final)` — the
    // `file_id` is preserved so the AAD re-binding holds across the rotation.
    let plaintext = decrypt_chunks(&chunks, v1_key, &file_id).map_err(|e| {
        MigrationV4Error::ImageChunkDecrypt {
            id: row.id.clone(),
            source: e,
        }
    })?;

    // Re-encrypt with the v2 key (fresh per-chunk nonces) and re-serialise.
    let v2_chunks =
        encrypt_chunks(&plaintext, v2_key, &file_id, IMAGE_CHUNK_SIZE).map_err(|e| {
            MigrationV4Error::ImageChunkEncrypt {
                id: row.id.clone(),
                source: e,
            }
        })?;
    let v2_blob = v2_blob_from_chunks(&row.id, &v2_chunks)?;

    // Atomically swap to v2. The WHERE re-asserts key_version=1 so a concurrent
    // writer bumping the version can't be silently overwritten. `content_nonce`
    // stays NULL (image nonces live per-chunk inside the blob); the row's
    // `key_version` column is the authoritative binding to the v2 key family.
    db.conn().execute(
        "UPDATE clipboard_items \
         SET content = ?1, key_version = ?2 \
         WHERE id = ?3 AND key_version = 1",
        params![v2_blob, KEY_VERSION_V2, row.id],
    )?;

    Ok(())
}
