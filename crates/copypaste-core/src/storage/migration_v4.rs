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
//! ## Why batched
//!
//! The sweep runs on daemon startup and must not block the event loop. We
//! process at most `BATCH_SIZE` rows per transaction and sleep
//! `INTER_BATCH_SLEEP` between batches. The migration is idempotent: if
//! the daemon is killed mid-sweep, the next startup picks up where it left
//! off (any rows already at `key_version = 2` are filtered out by the
//! `WHERE key_version = 1` predicate).
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

/// AAD schema version stamped into the v4 (key-versioned) AAD format. Same
/// caveat as `AAD_SCHEMA_V3` re: pinning.
const AAD_SCHEMA_V4: u32 = 4;

/// `key_version` value written into newly-rotated rows. Must match
/// [`super::items::ITEM_KEY_VERSION_CURRENT`].
const KEY_VERSION_V2: i64 = 2;

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

        for row in batch {
            match rotate_one(db, &row, v1_key, v2_key) {
                Ok(()) => total_rotated += 1,
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

        for row in batch {
            match rotate_one_image(db, &row, v1_key, v2_key) {
                Ok(()) => total_rotated += 1,
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

/// Parse the 16-byte `file_id` out of an image row's `blob_ref` JSON. The
/// metadata shape is produced by `daemon::handle_image`
/// (`{"width":...,"file_id":[u8; 16]}` — Rust `{:?}` debug-format of the byte
/// array, which serialises as a JSON array of 16 numbers, e.g.
/// `"file_id":[12, 34, ...]`).
///
/// We parse the single `file_id` array directly rather than pulling
/// `serde_json` into the production build of `copypaste-core` (it is only a
/// dev-dependency here). The format is fixed and emitted by our own daemon, so
/// a targeted extractor is sufficient and avoids a new runtime dependency.
fn parse_file_id(id: &str, blob_ref: Option<&str>) -> Result<[u8; 16], MigrationV4Error> {
    let meta_json = blob_ref.ok_or_else(|| MigrationV4Error::ImageMeta {
        id: id.to_string(),
        reason: "missing blob_ref metadata".to_string(),
    })?;
    let err = |reason: String| MigrationV4Error::ImageMeta {
        id: id.to_string(),
        reason,
    };

    // Locate the `"file_id"` key, then the opening `[` and matching `]`.
    let key_pos = meta_json
        .find("\"file_id\"")
        .ok_or_else(|| err("blob_ref missing 'file_id' field".to_string()))?;
    let after_key = &meta_json[key_pos + "\"file_id\"".len()..];
    let open = after_key
        .find('[')
        .ok_or_else(|| err("'file_id' value is not an array".to_string()))?;
    let rest = &after_key[open + 1..];
    let close = rest
        .find(']')
        .ok_or_else(|| err("'file_id' array is not closed".to_string()))?;
    let inner = &rest[..close];

    let mut out = [0u8; 16];
    let mut count = 0usize;
    for tok in inner.split(',') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        if count >= 16 {
            return Err(err("'file_id' has more than 16 elements".to_string()));
        }
        out[count] = tok
            .parse::<u8>()
            .map_err(|_| err(format!("'file_id[{count}]' is not a u8: {tok:?}")))?;
        count += 1;
    }
    if count != 16 {
        return Err(err(format!(
            "'file_id' has wrong length: expected 16, got {count}"
        )));
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
    let v2_chunks = encrypt_chunks(&plaintext, v2_key, &file_id, IMAGE_CHUNK_SIZE);
    let v2_blob = chunks_to_blob(&v2_chunks);

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

        let chunks = encrypt_chunks(plaintext, v1_key, &file_id, chunk_size);
        let blob = chunks_to_blob(&chunks);

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
}
