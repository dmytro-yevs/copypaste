//! Local decrypt (upload side): turn a locally-stored [`ClipboardItem`]'s
//! ciphertext back into plaintext so cloud/relay can re-wrap the SAME
//! plaintext under the sync key.
//!
//! Split out of the former flat `sync_common.rs` (ADR-017, CopyPaste-vp63.7)
//! — moved verbatim, no behavior change.

use copypaste_core::{decrypt_item_by_version, derive_v2, ClipboardItem, V1Key, V2Key};

/// Decrypt a locally-stored [`ClipboardItem`]'s `content` field to plaintext
/// using the daemon's local key and the item's `key_version`.
///
/// Returns the raw plaintext bytes on success, or an error string for logging.
/// Never logs the plaintext or the key.
pub(crate) fn decrypt_item_plaintext(
    item: &ClipboardItem,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<Vec<u8>, String> {
    // v0.6: image/file items store a multi-chunk blob encrypted under the LOCAL
    // v1 seed with `file_id` AAD (NOT the per-item v2 AAD). Reassemble them into
    // plaintext here so the cloud upload path re-wraps the SAME plaintext under
    // the sync key (identical wire contract to the P2P re-key path), then
    // enforce the sync ceiling so an oversized blob is rejected, not corrupted.
    if item.content_type == "image" || item.content_type == "file" {
        let meta_json = item
            .blob_ref
            .as_deref()
            .ok_or("blob item has no blob_ref")?;
        let file_id = crate::ipc::parse_image_file_id(meta_json)?;
        let content = item.content.as_deref().ok_or("blob item has no content")?;
        let chunks = copypaste_core::chunks_from_blob(content).map_err(|e| e.to_string())?;
        let v1_key: [u8; 32] = **local_key;
        let plaintext = if item.content_type == "image" {
            copypaste_core::decode_image(&chunks, &v1_key, &file_id).map_err(|e| e.to_string())?
        } else {
            copypaste_core::decode_file(&chunks, &v1_key, &file_id).map_err(|e| e.to_string())?
        };
        // NOTE: the cloud sync ceiling is enforced on the WRAPPED plaintext (after
        // `wrap_cloud_upload_plaintext` prepends the file name/MIME header), NOT on
        // this raw plaintext. The DOWNLOAD side (`build_local_blob_item`) checks the
        // same header-INCLUSIVE buffer, so checking the wrapped quantity keeps upload
        // and download symmetric — see `wrap_and_check_cloud_upload_plaintext`.
        return Ok(plaintext);
    }
    let content = item.content.as_deref().ok_or("item has no content")?;
    let nonce_vec = item
        .content_nonce
        .as_deref()
        .ok_or("item has no content_nonce")?;
    let nonce: &[u8; 24] = nonce_vec
        .try_into()
        .map_err(|_| format!("content_nonce wrong length: {}", nonce_vec.len()))?;
    let v1_key: [u8; 32] = **local_key;
    let v2_key = derive_v2(&v1_key);
    decrypt_item_by_version(
        item.key_version,
        V1Key(&v1_key),
        V2Key(&v2_key),
        &item.item_id,
        nonce,
        content,
    )
    .map_err(|e| e.to_string())
}

/// Async wrapper around [`decrypt_item_plaintext`] that runs the CPU-bound
/// decrypt/decode on the blocking thread pool (CopyPaste-z1xt).
///
/// The push/relay loops are async tasks on the tokio executor; `decode_image` /
/// `decode_file` / `decrypt_item_by_version` are synchronous, potentially
/// multi-MB crypto operations that previously ran inline and stalled the
/// executor thread. This consumes the `ClipboardItem` (so it can move into the
/// `'static` closure without cloning the heavy `content` blob) and returns it
/// back (as `Some`) alongside the decrypt result, so the caller can keep using
/// the item.
///
/// On the (effectively unreachable) JoinError path — a panic inside the
/// blocking task or runtime shutdown — the item cannot be recovered, so the
/// first tuple element is `None` and the second is `Err`. Callers handle the
/// error path by logging + skipping, so no panic is raised here.
pub(crate) async fn decrypt_item_plaintext_blocking(
    item: ClipboardItem,
    local_key: zeroize::Zeroizing<[u8; 32]>,
) -> (Option<ClipboardItem>, Result<Vec<u8>, String>) {
    match tokio::task::spawn_blocking(move || {
        let result = decrypt_item_plaintext(&item, &local_key);
        (item, result)
    })
    .await
    {
        Ok((item, result)) => (Some(item), result),
        Err(join_err) => (
            None,
            Err(format!("decrypt blocking task failed: {join_err}")),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{
        build_item_aad_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4, ITEM_KEY_VERSION_CURRENT,
    };

    /// Characterization test (CopyPaste-vp63.7 gap): `decrypt_item_plaintext`
    /// round-trips a v2 text item encrypted exactly the way the local capture
    /// path stores it (`build_item_aad_v2` + `encrypt_item_with_aad`).
    #[test]
    fn decrypt_item_plaintext_text_round_trip() {
        let local_key = zeroize::Zeroizing::new([0x42u8; 32]);
        let v1_key: [u8; 32] = *local_key;
        let v2_key = derive_v2(&v1_key);
        let item_id = copypaste_core::ItemId::from("decrypt-test-item");
        let plaintext = b"round trip me";
        let aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce, ciphertext) =
            encrypt_item_with_aad(plaintext, &v2_key, &aad).expect("encrypt must succeed");

        let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 1);
        item.item_id = item_id;
        item.key_version = ITEM_KEY_VERSION_CURRENT as u8;

        let decrypted = decrypt_item_plaintext(&item, &local_key).expect("decrypt must succeed");
        assert_eq!(decrypted, plaintext);
    }

    /// Characterization test (CopyPaste-vp63.7 gap): a text item missing
    /// `content` must return a descriptive `Err`, not panic.
    #[test]
    fn decrypt_item_plaintext_text_missing_content_errors() {
        let local_key = zeroize::Zeroizing::new([0x11u8; 32]);
        let mut item = ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], 1);
        item.content = None;
        let err = decrypt_item_plaintext(&item, &local_key).expect_err("must error");
        assert!(err.contains("no content"), "unexpected error: {err}");
    }

    /// Characterization test (CopyPaste-vp63.7 gap): a text item with a
    /// wrong-length nonce must return a descriptive `Err`, not panic.
    #[test]
    fn decrypt_item_plaintext_text_wrong_nonce_length_errors() {
        let local_key = zeroize::Zeroizing::new([0x11u8; 32]);
        let mut item = ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], 1);
        item.content_nonce = Some(vec![0u8; 4]); // wrong length
        let err = decrypt_item_plaintext(&item, &local_key).expect_err("must error");
        assert!(err.contains("wrong length"), "unexpected error: {err}");
    }

    /// Characterization test (CopyPaste-vp63.7 gap): an image item with a
    /// missing `blob_ref` must return a descriptive `Err`, not panic.
    #[test]
    fn decrypt_item_plaintext_image_missing_blob_ref_errors() {
        let local_key = zeroize::Zeroizing::new([0x11u8; 32]);
        let mut item = ClipboardItem::new_text(vec![1, 2, 3], vec![0u8; 24], 1);
        item.content_type = "image".to_string();
        item.blob_ref = None;
        let err = decrypt_item_plaintext(&item, &local_key).expect_err("must error");
        assert!(err.contains("blob_ref"), "unexpected error: {err}");
    }

    /// Characterization test (CopyPaste-vp63.7 gap): the blocking wrapper
    /// hands the item back on success alongside the decrypted plaintext.
    #[tokio::test]
    async fn decrypt_item_plaintext_blocking_returns_item_and_plaintext() {
        let local_key = zeroize::Zeroizing::new([0x77u8; 32]);
        let v1_key: [u8; 32] = *local_key;
        let v2_key = derive_v2(&v1_key);
        let item_id = copypaste_core::ItemId::from("decrypt-blocking-item");
        let plaintext = b"blocking round trip";
        let aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce, ciphertext) =
            encrypt_item_with_aad(plaintext, &v2_key, &aad).expect("encrypt must succeed");

        let mut item = ClipboardItem::new_text(ciphertext, nonce.to_vec(), 1);
        item.item_id = item_id;
        item.key_version = ITEM_KEY_VERSION_CURRENT as u8;
        let expected_id = item.id.clone();

        let (returned_item, result) = decrypt_item_plaintext_blocking(item, local_key).await;
        let returned_item = returned_item.expect("item must be returned on success");
        assert_eq!(returned_item.id, expected_id);
        assert_eq!(result.expect("decrypt must succeed"), plaintext);
    }
}
