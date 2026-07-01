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
//!
//! ## Module layout (CopyPaste-vp63.21)
//!
//! This module is split into 3 independent sweep clusters sharing the
//! constants + error type defined here:
//! * [`text`] — text-item item-level AEAD rotation.
//! * [`images`] — image-chunk rotation (closes Cnew).
//! * [`repair`] — mislabeled-kv2-blob repair (closes the pre-fix writer bug).

use super::db::Database;
use crate::crypto::encrypt::EncryptError;
use crate::image::ImageError;
use thiserror::Error;

mod images;
mod repair;
mod text;

#[cfg(test)]
mod tests;

pub use images::migrate_v1_image_chunks_to_v2;
pub use repair::repair_mislabeled_kv2_blob_rows;

use crate::crypto::chunks::ChunkError;

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
        let batch = text::fetch_v1_batch(db, BATCH_SIZE)?;
        if batch.is_empty() {
            break;
        }
        let batch_len = batch.len();
        let mut rotated_this_batch = 0usize;

        for row in batch {
            match text::rotate_one(db, &row, v1_key, v2_key) {
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
