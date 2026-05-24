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
//! Text items only. Image items use the per-chunk encryption in
//! [`crate::crypto::chunks`] which has its own format-version field; image
//! migration is tracked separately and out of scope for T5.
//!
//! ## TODO(v0.4): Cnew — image chunks not migrated
//!
//! Image clipboard items captured before the v4 migration retain their
//! original key derivation (v1 HKDF family). They remain accessible but are
//! **not** re-encrypted as part of this sweep: the `WHERE content_type = 'text'`
//! predicate explicitly excludes them. Full re-encryption of image chunks is
//! deferred to v0.4. See `docs/known-issues.md` for the documented scope.

use super::db::Database;
use crate::crypto::encrypt::{
    build_item_aad, build_item_aad_v2, decrypt_item_with_aad, encrypt_item_with_aad, EncryptError,
    NONCE_SIZE,
};
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
}

/// Run the v1 → v2 key rotation across every text row still at
/// `key_version = 1`.
///
/// * Returns the number of rows successfully re-encrypted.
/// * If a single row fails to decrypt, it is **left at `key_version = 1`**
///   and the function continues with the next row (a corrupt row should not
///   block the rest of the sweep). The error count is logged via `tracing`.
/// * Rows without a `content_nonce` (image rows or pre-baked migration rows)
///   are skipped entirely — they don't carry an item-level AEAD nonce.
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

    tracing::info!(
        rotated = total_rotated,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::encrypt::encrypt_item_with_aad;
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
}
