//! v3 → v4 ciphertext migration: re-encrypt every text item still stored
//! under the v1 HKDF key family using the v2 key family.
//!
//! ## Background
//!
//! Schema v4 adds a `key_version` column to `clipboard_items` (see
//! `super::schema::V4_ALTER_SQL`). Rows that existed prior to the migration
//! are marked `key_version = 1`; rows freshly inserted via
//! [`super::items::insert_item`] are marked `key_version = 2`.
//!
//! The on-disk binding of `key_version` into the AEAD AAD (see
//! [`crate::crypto::encrypt::build_item_aad_v2`]) means a v1 ciphertext can
//! NOT be decrypted with a v2 key — the auth tag rejects it. To finish the
//! key rotation we have to actually read each `key_version = 1` row,
//! decrypt it with the v1 key + v3-format AAD, re-encrypt it with the v2
//! key + v4-format AAD, and write the new ciphertext + nonce + bumped
//! `key_version` back.
//!
//! ## How the sweep works
//!
//! The sweep runs on daemon startup and must not block the event loop.
//! Each row is re-encrypted individually in its own implicit autocommit
//! transaction (`rotate_one` issues a single `UPDATE` outside any
//! explicit `BEGIN`). There is no multi-row batch transaction: the name
//! `BATCH_SIZE` refers only to the fetch page size — how many row ids are
//! loaded into memory per `SELECT` before processing them one-by-one.
//!
//! Progress tracking: `last_processed_id` is NOT written per row or per
//! batch. Instead, crash-safety is achieved by the `WHERE key_version = 1`
//! predicate itself — on restart the `SELECT` cursor skips every row that
//! was already successfully rotated to `key_version = 2`. The predicate is
//! therefore load-bearing for crash-safety and MUST NOT be removed or
//! weakened; removing it would cause a restarted sweep to re-process
//! already-rotated rows, which would fail AEAD authentication and leave
//! them logged as undecryptable.
//!
//! `INTER_BATCH_SLEEP` is applied between fetch pages (not between
//! individual rows) to yield the write lock to the daemon's hot path.
//! The migration is idempotent: if the daemon is killed mid-sweep, the
//! next startup picks up automatically because all already-rotated rows
//! are excluded by `WHERE key_version = 1`.
//!
//! ## Scope
//!
//! Both text and image items are migrated. Text items carry an item-level
//! AEAD (`content` + `content_nonce`) keyed by the row's `key_version` via the
//! `build_item_aad{,_v2}` AAD format. Image items store their content as a
//! per-chunk encrypted blob (see [`crate::crypto::chunks`]); the per-chunk AAD
//! binds `(CHUNK_FORMAT_V1, file_id, chunk_index, total_chunks, is_final)` but
//! NOT `key_version`, so the row's `key_version` column is the authoritative
//! record of which HKDF key generation decrypts the chunks. Image rows are
//! re-encrypted chunk-by-chunk with the v2 key and bumped to `key_version = 2`.
//!
//! ## Cnew (closed in v0.4)
//!
//! Earlier builds left image clipboard items captured before the v4 migration
//! under their original key derivation (v1 HKDF family): the text sweep's
//! `WHERE content_type = 'text'` predicate explicitly excluded them. That gap
//! is now closed — [`migrate_v1_to_v2_keys`] rotates image rows too (via
//! [`migrate_v1_image_chunks_to_v2`]), decrypting each chunk with the v1 key,
//! re-encrypting with the v2 key (fresh per-chunk nonces), re-serialising the
//! blob, and stamping `key_version = 2`. The `file_id` AAD context is read
//! from the row's `blob_ref` JSON and preserved across the rotation.

use super::db::Database;
use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks, ChunkError};
use crate::crypto::encrypt::{
    build_item_aad, build_item_aad_v2, decrypt_item_with_aad, encrypt_item_with_aad, EncryptError,
    NONCE_SIZE,
};
use crate::image::{chunks_from_blob, chunks_to_blob, ImageError, IMAGE_CHUNK_SIZE};
use rusqlite::params;
use thiserror::Error;

/// Re-encryption batch size. Picked at 100 to keep individual write
/// transactions short enough to not block readers on a hot DB while still
/// amortising the WAL fsync cost.
pub const BATCH_SIZE: usize = 100;

/// Sleep between batches. Yields the SQLite write lock so the daemon's
/// hot-path inserts (new clipboard items) can interleave.
pub const INTER_BATCH_SLEEP: std::time::Duration = std::time::Duration::from_millis(50);

/// AAD schema version stamped into the v3 (legacy) AAD format. Local
/// constant — kept in sync with [`crate::crypto::encrypt::AAD_SCHEMA_VERSION`]
/// but pinned here so the migration can never be silently desynced by an
/// unrelated bump of that constant.
const AAD_SCHEMA_V3: u32 = 3;
// Compile-time guard: if the canonical const ever changes, this migration
// file must be revisited before it will compile again.
const _: () = assert!(
    AAD_SCHEMA_V3 == crate::crypto::encrypt::AAD_SCHEMA_VERSION,
    "AAD_SCHEMA_V3 is out of sync with crate::crypto::encrypt::AAD_SCHEMA_VERSION"
);

/// AAD schema version stamped into the v4 (key-versioned) AAD format. Same
/// caveat as `AAD_SCHEMA_V3` re: pinning.
const AAD_SCHEMA_V4: u32 = 4;
// Compile-time guard: same rationale as above.
const _: () = assert!(
    AAD_SCHEMA_V4 == crate::crypto::encrypt::AAD_SCHEMA_VERSION_V4,
    "AAD_SCHEMA_V4 is out of sync with crate::crypto::encrypt::AAD_SCHEMA_VERSION_V4"
);

/// `key_version` value written into newly-rotated rows. Must match
/// [`super::items::ITEM_KEY_VERSION_CURRENT`].
const KEY_VERSION_V2: i64 = 2;
// Compile-time guard: if ITEM_KEY_VERSION_CURRENT is ever bumped this assert
// will fail, prompting a review of whether the migration constant is still correct.
const _: () = assert!(
    KEY_VERSION_V2 == super::items::ITEM_KEY_VERSION_CURRENT,
    "KEY_VERSION_V2 is out of sync with super::items::ITEM_KEY_VERSION_CURRENT"
);

#[derive(Debug, Error)]
pub enum MigrationV4Error {
    #[error("SQLite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("Decryption failed for item {id}: {source}")]
    Decrypt {
        id: String,
        #[source]
        source: EncryptError,
    },
    #[error("Re-encryption failed for item {id}: {source}")]
    Reencrypt {
        id: String,
        #[source]
        source: EncryptError,
    },
    #[error("Item {id} has an unexpected nonce length (got {got}, expected {expected})")]
    BadNonceLength {
        id: String,
        got: usize,
        expected: usize,
    },
    /// An image row's chunk blob could not be parsed back into chunks
    /// (corrupt/truncated `content` column).
    #[error("Image blob parse failed for item {id}: {source}")]
    ImageBlob {
        id: String,
        #[source]
        source: ImageError,
    },
    /// An image row's `blob_ref` metadata was missing or its `file_id` field
    /// could not be read. Without the `file_id` we cannot rebuild the per-chunk
    /// AAD, so the row is left at `key_version = 1`.
    #[error("Image metadata (blob_ref/file_id) invalid for item {id}: {reason}")]
    ImageMeta { id: String, reason: String },
    /// Per-chunk decryption with the v1 key failed for an image row.
    #[error("Image chunk decrypt failed for item {id}: {source}")]
    ImageChunkDecrypt {
        id: String,
        #[source]
        source: ChunkError,
    },
    /// Per-chunk re-encryption with the v2 key failed for an image row.
    #[error("Image chunk re-encrypt failed for item {id}: {source}")]
    ImageChunkEncrypt {
        id: String,
        #[source]
        source: ChunkError,
    },
}

/// Run the v1 → v2 key rotation across every row still at `key_version = 1`,
/// covering BOTH text items (item-level AEAD) and image items (per-chunk
/// encrypted blob).
///
/// * Returns the total number of rows successfully re-encrypted (text +
///   image).
/// * If a single row fails to decrypt, it is **left at `key_version = 1`**
///   and the function continues with the next row (a corrupt row should not
///   block the rest of the sweep). The error count is logged via `tracing`.
/// * Text rows are matched by `content_type = 'text'` with a non-NULL
///   `content_nonce`; image rows by `content_type = 'image'` (no item-level
///   nonce — nonces live per-chunk inside the `content` blob).
///
/// The caller is responsible for providing the two keys derived from the
/// same device IKM:
///   * `v1_key = DeviceKeypair::local_enc_key()` (or
///     [`crate::crypto::derive_storage_key_v1`])
///   * `v2_key = crate::crypto::derive_storage_key_v2(ikm, pair_id)`
///
/// See `crates/copypaste-daemon/src/daemon.rs` for the wiring at startup
/// (deferred to a follow-up task per the T5 hard-constraint not to touch
/// the daemon crate from this scope).
pub fn migrate_v1_to_v2_keys(
    db: &Database,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
) -> Result<usize, MigrationV4Error> {
    let mut total_rotated = 0usize;
    let mut total_failed = 0usize;

    loop {
        let batch = fetch_v1_batch(db, BATCH_SIZE)?;
        if batch.is_empty() {
            break;
        }
        let batch_len = batch.len();
        let mut rotated_this_batch = 0usize;

        for row in batch {
            match rotate_one(db, &row, v1_key, v2_key) {
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
                        "migration_v4: row left at key_version=1 (decrypt or re-encrypt failed)"
                    );
                }
            }
        }

        // Termination guard: a full batch that rotated ZERO rows means every
        // one of these `key_version = 1` rows failed (e.g. all undecryptable /
        // corrupt). Since the `WHERE key_version = 1` predicate would re-fetch
        // the exact same rows on the next iteration, continuing would loop
        // forever and hang daemon startup. Stop and leave them at v1.
        if rotated_this_batch == 0 && batch_len == BATCH_SIZE {
            tracing::warn!(
                stuck = batch_len,
                "migration_v4: a full batch of {batch_len} rows all failed to rotate; \
                 leaving them at key_version=1 and stopping the text sweep to avoid an infinite loop"
            );
            break;
        }

        // If the batch was full, more rows likely remain — sleep to yield
        // the write lock to the daemon's hot path, then loop. If the batch
        // was short, we're done.
        if batch_len < BATCH_SIZE {
            break;
        }
        std::thread::sleep(INTER_BATCH_SLEEP);
    }

    // Image rows are rotated in a second pass: they use a different on-disk
    // representation (per-chunk blob, no item-level nonce) so they're handled
    // by a dedicated sweep that reuses the same batching / crash-safe stepping.
    let image_rotated = migrate_v1_image_chunks_to_v2(db, v1_key, v2_key)?;
    total_rotated += image_rotated;

    tracing::info!(
        rotated = total_rotated,
        image_rotated,
        failed = total_failed,
        "migration_v4: sweep complete"
    );
    Ok(total_rotated)
}

/// Minimal projection of a v1-key row needed for re-encryption.
struct V1Row {
    id: String,
    item_id: String,
    content: Vec<u8>,
    content_nonce: Vec<u8>,
}

fn fetch_v1_batch(db: &Database, limit: usize) -> Result<Vec<V1Row>, MigrationV4Error> {
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content, content_nonce \
         FROM clipboard_items \
         WHERE key_version = 1 \
           AND content IS NOT NULL \
           AND content_nonce IS NOT NULL \
           AND content_type = 'text' \
         ORDER BY wall_time ASC \
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![limit as i64], |r| {
            Ok(V1Row {
                id: r.get(0)?,
                item_id: r.get(1)?,
                content: r.get(2)?,
                content_nonce: r.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn rotate_one(
    db: &Database,
    row: &V1Row,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
) -> Result<(), MigrationV4Error> {
    if row.content_nonce.len() != NONCE_SIZE {
        return Err(MigrationV4Error::BadNonceLength {
            id: row.id.clone(),
            got: row.content_nonce.len(),
            expected: NONCE_SIZE,
        });
    }
    let mut nonce_v1 = [0u8; NONCE_SIZE];
    nonce_v1.copy_from_slice(&row.content_nonce);

    // Decrypt with v1 key + v3-format AAD.
    let aad_v1 = build_item_aad(&row.item_id, AAD_SCHEMA_V3);
    let plaintext =
        decrypt_item_with_aad(&row.content, &nonce_v1, v1_key, &aad_v1).map_err(|e| {
            MigrationV4Error::Decrypt {
                id: row.id.clone(),
                source: e,
            }
        })?;

    // Re-encrypt with v2 key + v4-format AAD (key_version = 2).
    let aad_v2 = build_item_aad_v2(&row.item_id, AAD_SCHEMA_V4, KEY_VERSION_V2 as u32);
    let (nonce_v2, ciphertext_v2) =
        encrypt_item_with_aad(&plaintext, v2_key, &aad_v2).map_err(|e| {
            MigrationV4Error::Reencrypt {
                id: row.id.clone(),
                source: e,
            }
        })?;

    // Update in a single statement — the row is atomically swapped to v2.
    // The WHERE clause re-asserts key_version=1 so a concurrent writer
    // bumping the version can't be silently overwritten.
    db.conn().execute(
        "UPDATE clipboard_items \
         SET content = ?1, content_nonce = ?2, key_version = ?3 \
         WHERE id = ?4 AND key_version = 1",
        params![ciphertext_v2, nonce_v2.to_vec(), KEY_VERSION_V2, row.id],
    )?;

    Ok(())
}

// ── Image-chunk rotation (closes Cnew) ────────────────────────────────────

/// Minimal projection of a v1-key image row needed for chunk re-encryption.
/// Image rows have no item-level `content_nonce` (nonces live per-chunk inside
/// the `content` blob); the `file_id` AAD context is carried in `blob_ref`.
struct V1ImageRow {
    id: String,
    item_id: String,
    content: Vec<u8>,
    blob_ref: Option<String>,
}

/// Run the v1 → v2 chunk-key rotation across every image row still at
/// `key_version = 1`.
///
/// Mirrors [`migrate_v1_to_v2_keys`]: batched, crash-safe, idempotent (rows
/// already at `key_version = 2` are filtered out by the `WHERE key_version = 1`
/// predicate). A row that fails to decrypt or whose metadata is unusable is
/// left at `key_version = 1` and logged — it must not abort the sweep.
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
fn parse_file_id(id: &str, blob_ref: Option<&str>) -> Result<[u8; 16], MigrationV4Error> {
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
    let v2_blob = chunks_to_blob(&v2_chunks).map_err(|e| {
        // chunks_to_blob only errors with TooManyChunks; extract the inner ChunkError.
        let source = match e {
            crate::image::ImageError::Chunk(ce) => ce,
            _ => crate::crypto::chunks::ChunkError::TooManyChunks,
        };
        MigrationV4Error::ImageChunkEncrypt {
            id: row.id.clone(),
            source,
        }
    })?;

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

// ── Mislabeled kv=2 blob repair (closes writer bug) ───────────────────────
//
// Before the writer fix, `daemon::handle_image` and `handle_file` encrypted
// chunks with the RAW v1 seed key but stamped `key_version = 2` on the row
// (the `ClipboardItem::new_image` / `new_file` constructors always stamp
// `ITEM_KEY_VERSION_CURRENT = 2`). The `WHERE key_version = 1` predicate in
// `migrate_v1_image_chunks_to_v2` never saw these rows, so they stayed
// "mislabeled": encrypted-with-v1 but marked kv=2.
//
// After the writer fix every new image/file row is genuinely v2-encrypted.
// But existing mislabeled rows need a one-time repair: try v1-decrypt; on
// success the row is mislabeled — re-encrypt with v2 and bump `content` (the
// `key_version` stays 2, matching the stamp). On v1-decrypt failure the row
// is correctly v2-encrypted — skip it.
//
// The function is idempotent: after repair the row is truly v2-encrypted, so
// the v1-decrypt probe fails on the next run and the row is skipped.

/// Minimal projection of a candidate mislabeled kv=2 blob row.
struct Kv2BlobRow {
    id: String,
    // item_id is read from the DB for structural consistency with V1ImageRow
    // but not used in the repair logic (file_id comes from blob_ref JSON).
    #[allow(dead_code)]
    item_id: String,
    content: Vec<u8>,
    blob_ref: Option<String>,
}

fn fetch_kv2_blob_batch(db: &Database, limit: usize) -> Result<Vec<Kv2BlobRow>, MigrationV4Error> {
    // Clamp to i64::MAX to avoid overflow when the caller passes usize::MAX
    // or i64::MAX as usize.
    let sql_limit = i64::try_from(limit).unwrap_or(i64::MAX);
    let mut stmt = db.conn().prepare(
        "SELECT id, item_id, content, blob_ref \
         FROM clipboard_items \
         WHERE key_version = 2 \
           AND content IS NOT NULL \
           AND content_type IN ('image', 'file') \
         ORDER BY wall_time ASC \
         LIMIT ?1",
    )?;
    let rows = stmt
        .query_map(params![sql_limit], |r| {
            Ok(Kv2BlobRow {
                id: r.get(0)?,
                item_id: r.get(1)?,
                content: r.get(2)?,
                blob_ref: r.get(3)?,
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
    let v2_blob = chunks_to_blob(&v2_chunks).map_err(|e| {
        let source = match e {
            crate::image::ImageError::Chunk(ce) => ce,
            _ => crate::crypto::chunks::ChunkError::TooManyChunks,
        };
        MigrationV4Error::ImageChunkEncrypt {
            id: row.id.clone(),
            source,
        }
    })?;

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
/// The repair fetches all kv=2 blob rows in one pass. The expected set is
/// small (only rows captured before the writer fix was deployed), so a
/// single unbounded fetch is acceptable. Rows that fail the v1-decrypt probe
/// are skipped in-place; rows that fail re-encryption are logged and left
/// unchanged (they remain readable with the v2 key if they were truly v2).
pub fn repair_mislabeled_kv2_blob_rows(
    db: &Database,
    v1_key: &[u8; 32],
    v2_key: &[u8; 32],
) -> Result<usize, MigrationV4Error> {
    // Fetch all candidates in one shot — the set is expected to be tiny.
    // i64::MAX safely caps the SQL LIMIT to "all rows".
    let candidates = fetch_kv2_blob_batch(db, i64::MAX as usize)?;
    let mut total_repaired = 0usize;

    for row in &candidates {
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

    if total_repaired > 0 {
        tracing::info!(
            repaired = total_repaired,
            "repair_mislabeled_kv2: repaired {total_repaired} mislabeled kv2 blob row(s)"
        );
    }
    Ok(total_repaired)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::chunks::{decrypt_chunks, encrypt_chunks};
    use crate::crypto::encrypt::encrypt_item_with_aad;
    use crate::image::{chunks_from_blob, chunks_to_blob, IMAGE_CHUNK_SIZE};
    use crate::storage::db::Database;
    use rusqlite::params;
    use uuid::Uuid;

    /// Seed a row that looks exactly like a v1-key-encrypted text item:
    /// `key_version = 1`, AEAD built with the legacy 2-arg AAD format
    /// `"{item_id}|3"`. Returns `(row_id, item_id, plaintext)`.
    fn seed_v1_row(
        db: &Database,
        v1_key: &[u8; 32],
        plaintext: &[u8],
    ) -> (String, String, Vec<u8>) {
        let row_id = Uuid::new_v4().to_string();
        let item_id = Uuid::new_v4().to_string();
        let aad = build_item_aad(&item_id, AAD_SCHEMA_V3);
        let (nonce, ciphertext) = encrypt_item_with_aad(plaintext, v1_key, &aad).unwrap();

        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'text',?3,?4,0,0,?5,?5,1)",
                params![row_id, item_id, ciphertext, nonce.to_vec(), 1i64],
            )
            .unwrap();

        (row_id, item_id, plaintext.to_vec())
    }

    #[test]
    fn migrate_50_rows_all_land_on_key_version_2() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x11u8; 32];
        let v2_key = [0x22u8; 32];

        let mut originals: Vec<(String, Vec<u8>)> = Vec::with_capacity(50);
        for i in 0..50u8 {
            let pt = format!("plaintext-{}", i).into_bytes();
            let (row_id, _item_id, _) = seed_v1_row(&db, &v1_key, &pt);
            originals.push((row_id, pt));
        }

        let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(rotated, 50, "all 50 v1 rows must be re-encrypted");

        // Every row must now be at key_version=2.
        let remaining_v1: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(remaining_v1, 0);

        // Every row must decrypt cleanly with the v2 key + v4 AAD AND yield
        // the original plaintext (proves migration preserved content).
        for (row_id, expected_pt) in &originals {
            let (item_id, content, nonce_blob): (String, Vec<u8>, Vec<u8>) = db
                .conn()
                .query_row(
                    "SELECT item_id, content, content_nonce \
                     FROM clipboard_items WHERE id = ?1",
                    params![row_id],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();

            let mut nonce = [0u8; NONCE_SIZE];
            nonce.copy_from_slice(&nonce_blob);
            let aad_v2 = build_item_aad_v2(&item_id, AAD_SCHEMA_V4, 2);
            let pt = decrypt_item_with_aad(&content, &nonce, &v2_key, &aad_v2).unwrap();
            assert_eq!(&pt, expected_pt, "v2 plaintext must match v1 plaintext");
        }
    }

    #[test]
    fn migration_is_idempotent() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x33u8; 32];
        let v2_key = [0x44u8; 32];

        for i in 0..5u8 {
            seed_v1_row(&db, &v1_key, &[i, i, i, i]);
        }
        let first = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
        let second = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();

        assert_eq!(first, 5);
        assert_eq!(second, 0, "second run must find no v1 rows");
    }

    #[test]
    fn migration_with_no_v1_rows_returns_zero() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x55u8; 32];
        let v2_key = [0x66u8; 32];

        let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(rotated, 0);
    }

    #[test]
    fn migrated_row_is_undecryptable_with_v1_key() {
        // The whole point of the rotation: a v2-encrypted row must NOT be
        // decryptable with the v1 key even by an attacker who knows the
        // item_id and tries every plausible AAD format.
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x77u8; 32];
        let v2_key = [0x88u8; 32];

        let (row_id, item_id, _plain) = seed_v1_row(&db, &v1_key, b"super secret");
        migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();

        let (content, nonce_blob): (Vec<u8>, Vec<u8>) = db
            .conn()
            .query_row(
                "SELECT content, content_nonce FROM clipboard_items WHERE id = ?1",
                params![row_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        let mut nonce = [0u8; NONCE_SIZE];
        nonce.copy_from_slice(&nonce_blob);

        // Attempt #1: v1 key + v3 AAD (legacy combo)
        let aad_v3 = build_item_aad(&item_id, AAD_SCHEMA_V3);
        assert!(decrypt_item_with_aad(&content, &nonce, &v1_key, &aad_v3).is_err());

        // Attempt #2: v1 key + v4 AAD with key_version=2
        let aad_v4 = build_item_aad_v2(&item_id, AAD_SCHEMA_V4, 2);
        assert!(decrypt_item_with_aad(&content, &nonce, &v1_key, &aad_v4).is_err());
    }

    #[test]
    fn corrupt_v1_row_does_not_abort_the_sweep() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x99u8; 32];
        let v2_key = [0xAAu8; 32];

        // Seed one good v1 row + one row that was encrypted under a
        // *different* v1 key (simulating an undecryptable-under-current-key
        // row — could happen after a key rotation race).
        let (good_id, _item, _pt) = seed_v1_row(&db, &v1_key, b"good");
        let other_v1_key = [0xBBu8; 32];
        let (bad_id, _item2, _pt2) = seed_v1_row(&db, &other_v1_key, b"bad");

        let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(rotated, 1, "sweep must rotate the one decryptable row");

        // Good row is now at key_version=2; bad row is still at 1.
        let good_kv: i64 = db
            .conn()
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = ?1",
                params![good_id],
                |r| r.get(0),
            )
            .unwrap();
        let bad_kv: i64 = db
            .conn()
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = ?1",
                params![bad_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(good_kv, 2);
        assert_eq!(bad_kv, 1, "undecryptable row must be left at key_version=1");
    }

    // ── Image-chunk migration (Cnew / TODO(v0.4)) ──────────────────────────
    //
    // Image rows store their content as a chunk blob (no item-level
    // `content_nonce`; nonces live per-chunk inside the blob). The per-chunk
    // AEAD AAD binds `(CHUNK_FORMAT_V1, file_id, chunk_index, total_chunks,
    // is_final)` but NOT `key_version`. The row's `key_version` column is the
    // binding that records which HKDF key generation the chunks were encrypted
    // under. To carry an image row through the v1→v2 rotation we must decrypt
    // every chunk with the v1 key, re-encrypt with the v2 key (fresh nonces),
    // re-serialise the blob, and bump `key_version` to 2.

    /// Seed a row that looks exactly like a v1-key-encrypted image item:
    /// `content_type = 'image'`, `key_version = 1`, `content` holding a chunk
    /// blob produced with the v1 key, `content_nonce = NULL`, and `blob_ref`
    /// carrying the JSON metadata (`file_id` as a 16-element byte array, the
    /// same shape `daemon::handle_image` writes).
    ///
    /// Returns `(row_id, file_id, plaintext)`.
    fn seed_v1_image_row(
        db: &Database,
        v1_key: &[u8; 32],
        plaintext: &[u8],
        chunk_size: usize,
    ) -> (String, [u8; 16], Vec<u8>) {
        let row_id = Uuid::new_v4().to_string();
        let item_id = Uuid::new_v4().to_string();
        let file_id: [u8; 16] = *Uuid::new_v4().as_bytes();

        let chunks = encrypt_chunks(plaintext, v1_key, &file_id, chunk_size).unwrap();
        let blob = chunks_to_blob(&chunks).unwrap();

        // Mirror the JSON shape from daemon::handle_image: a `file_id` array
        // of 16 numbers (Rust `{:?}` debug-format of the byte array).
        let meta_json = format!(
            r#"{{"width":2,"height":2,"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
            plaintext.len(),
            chunks.len(),
            file_id
        );

        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, blob_ref, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'image',?3,NULL,?4,0,0,?5,?5,1)",
                params![row_id, item_id, blob, meta_json, 1i64],
            )
            .unwrap();

        (row_id, file_id, plaintext.to_vec())
    }

    /// RED→GREEN: a v1-key image-chunk row must be carried through the v4
    /// rotation — decrypted with the v1 key, re-encrypted with the v2 key, and
    /// landed on `key_version = 2`, while remaining decryptable to the original
    /// plaintext under the v2 key (and the preserved `file_id` AAD).
    #[test]
    fn image_chunk_row_migrates_to_key_version_2() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x11u8; 32];
        let v2_key = [0x22u8; 32];

        // Multi-chunk payload to exercise the per-chunk re-encryption loop.
        let plaintext: Vec<u8> = (0..(IMAGE_CHUNK_SIZE + 137))
            .map(|i| (i % 251) as u8)
            .collect();
        let (row_id, file_id, expected) =
            seed_v1_image_row(&db, &v1_key, &plaintext, IMAGE_CHUNK_SIZE);

        let rotated = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(rotated, 1, "the one v1 image row must be rotated");

        // Row must now be at key_version=2.
        let kv: i64 = db
            .conn()
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = ?1",
                params![row_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kv, 2, "image row must land on key_version=2");

        // The blob must now decrypt with the v2 key + preserved file_id AAD,
        // yielding the original plaintext.
        let blob: Vec<u8> = db
            .conn()
            .query_row(
                "SELECT content FROM clipboard_items WHERE id = ?1",
                params![row_id],
                |r| r.get(0),
            )
            .unwrap();
        let chunks = chunks_from_blob(&blob).unwrap();
        let recovered = decrypt_chunks(&chunks, &v2_key, &file_id).unwrap();
        assert_eq!(
            recovered, expected,
            "v2 chunk decrypt must match v1 plaintext"
        );

        // And it must NOT decrypt with the old v1 key anymore.
        assert!(
            decrypt_chunks(&chunks, &v1_key, &file_id).is_err(),
            "migrated image chunks must not decrypt with the v1 key"
        );
    }

    /// The image migration must be idempotent: a second run finds no v1 image
    /// rows (the first run bumped them all to key_version=2).
    #[test]
    fn image_chunk_migration_is_idempotent() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x33u8; 32];
        let v2_key = [0x44u8; 32];

        for i in 0..3u8 {
            seed_v1_image_row(&db, &v1_key, &[i; 64], 16);
        }
        let first = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
        let second = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(first, 3);
        assert_eq!(second, 0, "second run must find no v1 image rows");
    }

    /// A corrupt/undecryptable image row (encrypted under a different v1 key)
    /// must be left at key_version=1 and must not abort the sweep.
    #[test]
    fn corrupt_image_row_does_not_abort_the_sweep() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x55u8; 32];
        let v2_key = [0x66u8; 32];
        let other_v1_key = [0x77u8; 32];

        let (good_id, _fid, _pt) = seed_v1_image_row(&db, &v1_key, b"good image bytes", 8);
        let (bad_id, _fid2, _pt2) = seed_v1_image_row(&db, &other_v1_key, b"bad image bytes", 8);

        let rotated = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(rotated, 1, "only the decryptable image row must rotate");

        let good_kv: i64 = db
            .conn()
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = ?1",
                params![good_id],
                |r| r.get(0),
            )
            .unwrap();
        let bad_kv: i64 = db
            .conn()
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = ?1",
                params![bad_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(good_kv, 2);
        assert_eq!(
            bad_kv, 1,
            "undecryptable image row must be left at key_version=1"
        );
    }

    /// `parse_file_id` must extract the 16-byte array from the exact JSON shape
    /// that `daemon::handle_image` writes (Rust `{:?}` debug-format of a byte
    /// array, embedded among other metadata fields).
    #[test]
    fn parse_file_id_reads_daemon_json_shape() {
        let file_id: [u8; 16] = [0, 255, 1, 2, 3, 200, 17, 99, 16, 44, 78, 123, 5, 6, 7, 250];
        let json = format!(
            r#"{{"width":2,"height":2,"original_size":42,"chunk_count":1,"file_id":{:?}}}"#,
            file_id
        );
        let parsed = parse_file_id("row-1", Some(&json)).unwrap();
        assert_eq!(parsed, file_id);
    }

    /// `parse_file_id` must reject malformed / missing metadata with
    /// `ImageMeta` rather than panicking.
    #[test]
    fn parse_file_id_rejects_bad_metadata() {
        // Missing blob_ref entirely.
        assert!(matches!(
            parse_file_id("r", None),
            Err(MigrationV4Error::ImageMeta { .. })
        ));
        // No file_id field.
        assert!(matches!(
            parse_file_id("r", Some(r#"{"width":2}"#)),
            Err(MigrationV4Error::ImageMeta { .. })
        ));
        // Wrong length (15 elements).
        let short = r#"{"file_id":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15]}"#;
        assert!(matches!(
            parse_file_id("r", Some(short)),
            Err(MigrationV4Error::ImageMeta { .. })
        ));
        // Non-u8 element.
        let bad = r#"{"file_id":[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,999]}"#;
        assert!(matches!(
            parse_file_id("r", Some(bad)),
            Err(MigrationV4Error::ImageMeta { .. })
        ));
    }

    // ── Termination guard regression (HIGH) ───────────────────────────────
    //
    // If an ENTIRE batch (BATCH_SIZE rows) all fail to rotate, the
    // `WHERE key_version = 1` predicate would re-fetch the exact same rows on
    // the next iteration and the sweep would loop forever, hanging daemon
    // startup. The guard breaks out once a full batch produces zero
    // successful rotations. These tests seed a full batch of undecryptable
    // rows and assert the sweep TERMINATES (and leaves them at v1).
    //
    // The sweep itself is already bounded by the fix, but to guarantee the
    // test can never hang the whole suite if the guard ever regresses, we arm
    // a watchdog thread before the call: if the sweep hasn't returned (and
    // cleared the flag) within a generous budget, the watchdog aborts the
    // process with a clear message instead of letting CI block forever. A
    // worker-thread approach isn't usable here because rusqlite's in-memory
    // `Connection` is per-connection and `!Send`, so the sweep must run inline
    // on this thread against the borrowed `&Database`.

    #[test]
    fn full_batch_of_undecryptable_text_rows_terminates() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0xC1u8; 32];
        let v2_key = [0xC2u8; 32];
        // Every seeded row is encrypted under a DIFFERENT key, so none of them
        // decrypt with `v1_key` — a full batch of guaranteed failures.
        let other_v1_key = [0xCEu8; 32];

        for i in 0..BATCH_SIZE {
            let pt = format!("undecryptable-{i}").into_bytes();
            seed_v1_row(&db, &other_v1_key, &pt);
        }

        let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let flag = timed_out.clone();
        let watchdog = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(10));
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                // The sweep never set the flag to false → it hung.
                eprintln!(
                    "full_batch_of_undecryptable_text_rows_terminates: \
                     sweep hung (>10s) — termination guard regressed"
                );
                std::process::abort();
            }
        });

        let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
        timed_out.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = watchdog.join();

        assert_eq!(rotated, 0, "no row was decryptable, so none may rotate");

        // All BATCH_SIZE rows must still be at key_version=1.
        let remaining_v1: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining_v1 as usize, BATCH_SIZE,
            "every stuck row must be left at key_version=1"
        );
    }

    #[test]
    fn full_batch_of_undecryptable_image_rows_terminates() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0xD1u8; 32];
        let v2_key = [0xD2u8; 32];
        let other_v1_key = [0xDEu8; 32];

        for i in 0..BATCH_SIZE {
            // Distinct payloads, all encrypted under a key the sweep won't have.
            let pt = vec![(i % 256) as u8; 32];
            seed_v1_image_row(&db, &other_v1_key, &pt, 8);
        }

        let timed_out = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
        let flag = timed_out.clone();
        let watchdog = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_secs(10));
            if flag.load(std::sync::atomic::Ordering::SeqCst) {
                eprintln!(
                    "full_batch_of_undecryptable_image_rows_terminates: \
                     sweep hung (>10s) — termination guard regressed"
                );
                std::process::abort();
            }
        });

        let rotated = migrate_v1_image_chunks_to_v2(&db, &v1_key, &v2_key).unwrap();
        timed_out.store(false, std::sync::atomic::Ordering::SeqCst);
        let _ = watchdog.join();

        assert_eq!(
            rotated, 0,
            "no image row was decryptable, so none may rotate"
        );

        let remaining_v1: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items \
                 WHERE key_version = 1 AND content_type = 'image'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining_v1 as usize, BATCH_SIZE,
            "every stuck image row must be left at key_version=1"
        );
    }

    /// The combined `migrate_v1_to_v2_keys` sweep must rotate BOTH text and
    /// image rows so the v4 migration has no remaining `key_version = 1` rows
    /// of either type. This is the regression that closes the documented Cnew
    /// gap (image chunks previously skipped).
    #[test]
    fn full_sweep_rotates_text_and_image_rows() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0x88u8; 32];
        let v2_key = [0x99u8; 32];

        // One text row, one image row, both at key_version=1.
        let (text_id, _text_item, _text_pt) = {
            let row_id = Uuid::new_v4().to_string();
            let item_id = Uuid::new_v4().to_string();
            let aad = build_item_aad(&item_id, AAD_SCHEMA_V3);
            let (nonce, ct) = encrypt_item_with_aad(b"text payload", &v1_key, &aad).unwrap();
            db.conn()
                .execute(
                    "INSERT INTO clipboard_items \
                     (id, item_id, content_type, content, content_nonce, \
                      is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                     VALUES (?1,?2,'text',?3,?4,0,0,1,1,1)",
                    params![row_id, item_id, ct, nonce.to_vec()],
                )
                .unwrap();
            (row_id, item_id, b"text payload".to_vec())
        };
        let (image_id, _fid, _pt) = seed_v1_image_row(&db, &v1_key, b"image payload bytes", 8);

        let rotated = migrate_v1_to_v2_keys(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(rotated, 2, "both text and image rows must be rotated");

        let remaining_v1: i64 = db
            .conn()
            .query_row(
                "SELECT COUNT(*) FROM clipboard_items WHERE key_version = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            remaining_v1, 0,
            "no key_version=1 rows of any type may remain"
        );

        // Sanity: both rows are at key_version=2.
        for id in [&text_id, &image_id] {
            let kv: i64 = db
                .conn()
                .query_row(
                    "SELECT key_version FROM clipboard_items WHERE id = ?1",
                    params![id],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(kv, 2);
        }
    }

    // ── Mislabeled kv=2 blob repair ──────────────────────────────────────
    //
    // Before the writer fix, handle_image/handle_file encrypted chunks with
    // the v1 key but stamped key_version=2 on the row. The repair function
    // must detect these "mislabeled" rows (v1-decrypt succeeds) and
    // re-encrypt them with the v2 key. Correctly v2-encrypted rows (v1-
    // decrypt fails) must be left unchanged.

    /// Seed a mislabeled kv=2 row: content_type='image', key_version=2,
    /// BUT the chunk blob was encrypted with the v1 key (the old writer bug).
    /// Returns `(row_id, file_id, plaintext)`.
    fn seed_mislabeled_kv2_image_row(
        db: &Database,
        v1_key: &[u8; 32],
        plaintext: &[u8],
    ) -> (String, [u8; 16], Vec<u8>) {
        let row_id = Uuid::new_v4().to_string();
        let item_id = Uuid::new_v4().to_string();
        let file_id: [u8; 16] = *Uuid::new_v4().as_bytes();

        // Encrypt with v1 key (the bug) but stamp key_version=2 (the lie).
        let chunks = encrypt_chunks(plaintext, v1_key, &file_id, IMAGE_CHUNK_SIZE).unwrap();
        let blob = chunks_to_blob(&chunks).unwrap();

        let meta_json = format!(
            r#"{{"width":4,"height":4,"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
            plaintext.len(),
            chunks.len(),
            file_id
        );

        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, blob_ref, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'image',?3,NULL,?4,0,0,?5,?5,2)",
                params![row_id, item_id, blob, meta_json, 1i64],
            )
            .unwrap();

        (row_id, file_id, plaintext.to_vec())
    }

    /// A mislabeled kv=2 image row (encrypted with v1, stamped kv=2) must be
    /// re-encrypted with the v2 key by `repair_mislabeled_kv2_blob_rows`.
    /// After repair: repaired_count=1, row is genuinely v2-decryptable, and
    /// v1-decrypt fails.
    #[test]
    fn kv2_mislabeled_image_row_repairs_via_migration() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0xA1u8; 32];
        let v2_key = [0xA2u8; 32];

        let plaintext = b"mislabeled image bytes for repair test";
        let (row_id, file_id, expected_pt) =
            seed_mislabeled_kv2_image_row(&db, &v1_key, plaintext);

        let repaired = repair_mislabeled_kv2_blob_rows(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(repaired, 1, "exactly one mislabeled row must be repaired");

        // Retrieve the updated blob.
        let blob: Vec<u8> = db
            .conn()
            .query_row(
                "SELECT content FROM clipboard_items WHERE id = ?1",
                params![row_id],
                |r| r.get(0),
            )
            .unwrap();
        let chunks = chunks_from_blob(&blob).unwrap();

        // Must now decrypt with v2 key.
        let recovered = decrypt_chunks(&chunks, &v2_key, &file_id)
            .expect("repaired row must decrypt with v2 key");
        assert_eq!(recovered, expected_pt, "v2 plaintext must match original");

        // Must NOT decrypt with v1 key anymore.
        assert!(
            decrypt_chunks(&chunks, &v1_key, &file_id).is_err(),
            "repaired row must NOT decrypt with v1 key"
        );

        // key_version must still be 2 (stamp unchanged).
        let kv: i64 = db
            .conn()
            .query_row(
                "SELECT key_version FROM clipboard_items WHERE id = ?1",
                params![row_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(kv, 2, "key_version stamp must remain 2 after repair");
    }

    /// A correctly v2-encrypted kv=2 row (v1-decrypt fails) must be left
    /// completely unchanged by `repair_mislabeled_kv2_blob_rows`.
    /// repaired_count must be 0.
    #[test]
    fn kv2_correctly_encrypted_row_not_touched_by_repair_migration() {
        let db = Database::open_in_memory().unwrap();
        let v1_key = [0xB1u8; 32];
        let v2_key = [0xB2u8; 32];

        let plaintext = b"genuinely v2-encrypted image bytes";
        let file_id: [u8; 16] = *Uuid::new_v4().as_bytes();
        let row_id = Uuid::new_v4().to_string();
        let item_id = Uuid::new_v4().to_string();

        // Encrypt with v2 key (correct).
        let chunks = encrypt_chunks(plaintext, &v2_key, &file_id, IMAGE_CHUNK_SIZE).unwrap();
        let blob = chunks_to_blob(&chunks).unwrap();
        let original_blob = blob.clone();

        let meta_json = format!(
            r#"{{"width":2,"height":2,"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
            plaintext.len(),
            chunks.len(),
            file_id
        );

        db.conn()
            .execute(
                "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, blob_ref, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?1,?2,'image',?3,NULL,?4,0,0,?5,?5,2)",
                params![row_id, item_id, blob, meta_json, 1i64],
            )
            .unwrap();

        let repaired = repair_mislabeled_kv2_blob_rows(&db, &v1_key, &v2_key).unwrap();
        assert_eq!(repaired, 0, "correctly v2-encrypted row must NOT be repaired");

        // Content blob must be byte-for-byte identical (untouched).
        let stored_blob: Vec<u8> = db
            .conn()
            .query_row(
                "SELECT content FROM clipboard_items WHERE id = ?1",
                params![row_id],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            stored_blob, original_blob,
            "content must be unchanged for a correctly v2-encrypted row"
        );
    }
}
