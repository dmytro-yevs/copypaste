//! Text-item v1 → v2 key rotation cluster.
//!
//! Text items carry an item-level AEAD (`content` + `content_nonce`) keyed by
//! the row's `key_version` via the `build_item_aad{,_v2}` AAD format. This
//! module fetches batches of `key_version = 1` text rows and rotates each one
//! individually (`rotate_one`); the batching/looping/termination-guard logic
//! lives in the top orchestrator [`super::migrate_v1_to_v2_keys`].

use super::{Database, MigrationV4Error, AAD_SCHEMA_V3, AAD_SCHEMA_V4, KEY_VERSION_V2};
use crate::crypto::encrypt::{
    build_item_aad, build_item_aad_v2, decrypt_item_with_aad, encrypt_item_with_aad, NONCE_SIZE,
};
use crate::storage::items::ItemId;
use rusqlite::params;

/// Minimal projection of a v1-key row needed for re-encryption.
pub(super) struct V1Row {
    pub(super) id: String,
    pub(super) item_id: ItemId,
    content: Vec<u8>,
    content_nonce: Vec<u8>,
}

pub(super) fn fetch_v1_batch(db: &Database, limit: usize) -> Result<Vec<V1Row>, MigrationV4Error> {
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

pub(super) fn rotate_one(
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
