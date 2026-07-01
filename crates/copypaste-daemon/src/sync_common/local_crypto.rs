//! Shared local-v2 at-rest encrypt glue (ADR-017 Wave-2 dedup,
//! CopyPaste-vp63.52).
//!
//! Consolidates the identical 3-line block —
//! `build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, key_version=2)` +
//! `encrypt_item_with_aad(plaintext, v2_key, aad)` — that was duplicated
//! verbatim across `sync_common::rebuild::build_local_item`,
//! `sync_orch::rekey::inbound::rekey_inbound`,
//! `daemon::capture::text::encrypt_text_for_storage`, and the IPC
//! `import` handler (`ipc::handlers_transfer`).
//!
//! # Security
//! AAD-BINDING INVARIANT: this tuple `(item_id, AAD_SCHEMA_VERSION_V4,
//! key_version = 2)` MUST stay in lockstep with the `key_version = 2` the
//! caller stamps on the stored row — moved VERBATIM from the four call
//! sites, no crypto change.

use copypaste_core::{
    build_item_aad_v2, encrypt_item_with_aad, EncryptError, ItemId, AAD_SCHEMA_VERSION_V4,
    ITEM_KEY_VERSION_CURRENT,
};

/// Encrypt `plaintext` for local v2 at-rest storage under `v2_key`.
///
/// Builds the standard `(item_id, AAD_SCHEMA_VERSION_V4, key_version = 2)`
/// AAD and calls `encrypt_item_with_aad`. The caller MUST stamp the stored
/// row's `key_version` as `2` (`ITEM_KEY_VERSION_CURRENT`) — this function
/// does not touch storage, only the AEAD step.
///
/// `v2_key` accepts `&[u8; 32]` directly, so callers holding a
/// `zeroize::Zeroizing<[u8; 32]>` (the `derive_v2` return type) pass
/// `&v2_key` and rely on `Deref` coercion, exactly as the original call
/// sites did.
pub(crate) fn encrypt_v2_for_local_storage(
    item_id: &str,
    plaintext: &[u8],
    v2_key: &[u8; 32],
) -> Result<([u8; copypaste_core::NONCE_SIZE], Vec<u8>), EncryptError> {
    // ITEM_KEY_VERSION_CURRENT is i64 (storage convention); build_item_aad_v2
    // takes u32 — cast explicitly. Value is 2, which fits both.
    let aad = build_item_aad_v2(
        &ItemId::from(item_id),
        AAD_SCHEMA_VERSION_V4,
        ITEM_KEY_VERSION_CURRENT as u32,
    );
    encrypt_item_with_aad(plaintext, v2_key, &aad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{decrypt_item_by_version, derive_v2, V1Key, V2Key};

    /// Characterization test (CopyPaste-vp63.52): round-trips through the
    /// production read path (`decrypt_item_by_version` at key_version = 2),
    /// pinning the exact AAD tuple this helper builds.
    #[test]
    fn encrypt_v2_for_local_storage_round_trips_with_production_read_path() {
        let v1_key = [0x21u8; 32];
        let v2_key = derive_v2(&v1_key);
        let item_id = "item-vp63-52";
        let plaintext = b"hello from the shared helper";

        let (nonce, ciphertext) =
            encrypt_v2_for_local_storage(item_id, plaintext, &v2_key).expect("encrypt succeeds");

        let decrypted = decrypt_item_by_version(
            2,
            V1Key(&v1_key),
            V2Key(&v2_key),
            &ItemId::from(item_id),
            &nonce,
            &ciphertext,
        )
        .expect("decrypt with production read path must succeed");
        assert_eq!(decrypted, plaintext.to_vec());
    }

    /// Tampering the `item_id` used at decrypt time must break the AAD
    /// binding (auth failure), proving the ciphertext is bound to it.
    #[test]
    fn encrypt_v2_for_local_storage_binds_item_id_into_aad() {
        let v1_key = [0x22u8; 32];
        let v2_key = derive_v2(&v1_key);
        let plaintext = b"bound to item id";

        let (nonce, ciphertext) =
            encrypt_v2_for_local_storage("item-a", plaintext, &v2_key).expect("encrypt succeeds");

        let err = decrypt_item_by_version(
            2,
            V1Key(&v1_key),
            V2Key(&v2_key),
            &ItemId::from("item-b"),
            &nonce,
            &ciphertext,
        )
        .expect_err("decrypting with a different item_id must fail");
        assert!(matches!(err, copypaste_core::EncryptError::AuthFailed));
    }
}
