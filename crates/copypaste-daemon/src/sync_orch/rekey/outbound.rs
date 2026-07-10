//! Outbound re-keying: decrypt at-rest ciphertext under this device's local
//! key, then re-encrypt the plaintext under a shared/pairwise sync key so a
//! paired peer can decrypt it (P2P Phase 3).
//!
//! Split out of the former flat `rekey.rs` (ADR-017, CopyPaste-vp63.9) — moved
//! verbatim, no behavior change.

use copypaste_core::{
    decrypt_item_by_version, encrypt_for_cloud, SyncKey, V1Key, V2Key, NONCE_SIZE,
};
use copypaste_sync::protocol::WireItem;
use tracing::warn;

use super::crypto_ctx::SyncCrypto;
use super::inbound::parse_file_name_mime;

/// Outcome of an attempt to re-key an outgoing item under the shared sync key.
#[derive(Debug, PartialEq, Eq)]
pub enum RekeyOutcome {
    /// The payload was successfully re-wrapped under the shared sync key — the
    /// wire item is decryptable by the paired peer and safe to forward.
    Rewrapped,
    /// The item is not a re-key candidate (non-text, no content/nonce, or no
    /// shared key is available). The wire item is left unchanged and follows
    /// the legacy path — it may carry raw at-rest ciphertext (a no-crypto /
    /// legacy peer expects that) or be an image chunk handled elsewhere.
    NotApplicable,
    /// A shared key WAS available and the item WAS a candidate, but re-keying
    /// failed (wrong nonce length, local-decrypt, or shared-encrypt error). The
    /// wire item still carries raw at-rest ciphertext that the peer can never
    /// decrypt — the caller MUST drop it rather than forward a permanently
    /// undecryptable row (sync H2).
    Failed,
}

/// Maximum reassembled image/file plaintext we will re-key onto the wire.
///
/// The P2P transport frames at 16 MiB (`transport.rs`) and the cloud relay
/// caps the request body, so an oversized blob would either be rejected by the
/// transport or land undecryptable on the peer. We enforce the ceiling here so
/// the item is *dropped with a warning* rather than silently corrupting sync.
///
/// Ceiling layering (one blob, four caps — see `defaults.rs::MAX_FILE_SIZE_BYTES`):
///   * STORABLE = 100 MiB — `copypaste_core::MAX_FILE_BYTES`, library hard cap on
///     a locally-stored file item. `max_file_size_bytes` is clamped to this.
///   * SYNC     =   8 MiB — *this* const: the largest plaintext re-keyed onto the
///     wire. Items 8–100 MiB are kept LOCALLY but skipped for sync (warned).
///   * P2P frame =  16 MiB — transport framing cap.
///   * Relay body = 10 MiB — relay request-body cap.
///
/// So a file can be storable yet un-syncable: local storage and sync are
/// deliberately decoupled, and the UI tells the user where the sync line sits.
///
/// Re-exported from [`copypaste_ipc::SYNC_MAX_BLOB_BYTES`] (CopyPaste-1d5l.58)
/// — the same canonical value `copypaste_relay::quota::Tier::max_item_bytes`
/// uses for its text-item quota, so the two crates (which do not depend on
/// each other) cannot drift.
pub const SYNC_MAX_BLOB_BYTES: usize = copypaste_ipc::SYNC_MAX_BLOB_BYTES;

/// Reassemble an image/file item's at-rest chunk blob back into plaintext.
///
/// The chunks were encrypted under this device's LOCAL v1 seed (`crypto.v1_key`)
/// with the 16-byte `file_id` as AEAD AAD — exactly as `daemon::handle_image`
/// (and the file pipeline) writes them. We parse `file_id` out of the
/// `blob_ref` meta JSON (shared parser for both image and file), deserialize the
/// chunks, and decode:
///   * image → [`copypaste_core::decode_image`] (PNG bytes)
///   * file  → [`copypaste_core::decode_file`] (verbatim bytes)
///
/// Returns the recovered plaintext, or `None` on any parse/decrypt failure
/// (logged). Callers map `None` to `RekeyOutcome::Failed` so a corrupt local
/// row is dropped, never forwarded.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn recover_blob_plaintext(crypto: &SyncCrypto, wire: &WireItem) -> Option<Vec<u8>> {
    let meta_json = wire.blob_ref.as_deref()?;
    let file_id = match crate::ipc::parse_image_file_id(meta_json) {
        Ok(id) => id,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob meta parse failed: {e}");
            return None;
        }
    };
    let content = wire.content.as_deref()?;
    let chunks = match copypaste_core::chunks_from_blob(content) {
        Ok(c) => c,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: chunks_from_blob failed: {e}");
            return None;
        }
    };
    // Dispatch on the wire item's key_version: v1 rows use the raw local
    // key (v1 seed); v2 rows use derive_v2(seed). After the writer fix
    // (handle_image / handle_file now use derive_v2), all freshly-captured
    // rows are kv=2. Legacy rows stamped kv=2 but encrypted with v1 (the
    // mislabeled rows) are repaired by repair_mislabeled_kv2_blob_rows at
    // startup, so by the time sync runs all kv=2 rows are truly v2.
    let blob_key: &[u8; 32] = if wire.key_version == 1 {
        &crypto.v1_key
    } else {
        &crypto.v2_key
    };
    let decoded = if wire.content_type == "image" {
        copypaste_core::decode_image(&chunks, blob_key, &file_id).map_err(|e| e.to_string())
    } else {
        copypaste_core::decode_file(&chunks, blob_key, &file_id).map_err(|e| e.to_string())
    };
    match decoded {
        Ok(pt) => Some(pt),
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob decode failed: {e}");
            None
        }
    }
}

/// Re-key an image/file wire item onto the shared sync key.
///
/// Reassembles the at-rest blob to plaintext ([`recover_blob_plaintext`]),
/// enforces [`SYNC_MAX_BLOB_BYTES`], then replaces `content` with a single
/// shared-key-wrapped blob (`encrypt_for_cloud`, same call the text arm uses),
/// clears `content_nonce` (the unwrap marker) and `blob_ref`, and keeps
/// `content_type`. Mirrors the text arm's `Failed`/`NotApplicable` contract.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
/// Inner implementation for blob (image/file) outbound re-keying under an
/// explicit `SyncKey` (CopyPaste-716: key now passed in by caller rather than
/// fetched from the first-peer-only cache).
pub(super) fn rekey_blob_outbound_with_key(
    crypto: &SyncCrypto,
    shared: &SyncKey,
    wire: &mut WireItem,
) -> RekeyOutcome {
    // A shared key IS present: from here any failure is `Failed` (drop), never
    // a silent forward of an undecryptable at-rest blob (sync H2).
    let Some(plaintext) = recover_blob_plaintext(crypto, wire) else {
        return RekeyOutcome::Failed;
    };
    if plaintext.len() > SYNC_MAX_BLOB_BYTES {
        warn!(
            item_id = %wire.item_id,
            size = plaintext.len(),
            max = SYNC_MAX_BLOB_BYTES,
            "sync_orch: blob exceeds sync ceiling, dropping (not forwarded)"
        );
        return RekeyOutcome::Failed;
    }
    match encrypt_for_cloud(shared, &wire.item_id, &plaintext) {
        Ok(blob) => {
            wire.content = Some(blob);
            // Self-framed blob → no item-level nonce; `None` is the receiver's
            // sync-key-wrapped marker.
            wire.content_nonce = None;
            // For file items: stash filename + mime into the dedicated wire
            // fields BEFORE clearing blob_ref, so the receiver can reconstruct
            // the local file meta JSON with the correct identity. blob_ref
            // itself must not travel (it is a local at-rest artefact; the
            // receiver rebuilds it from recovered plaintext + these fields).
            if wire.content_type == "file" {
                if let Some((fname, fmime)) =
                    wire.blob_ref.as_deref().and_then(parse_file_name_mime)
                {
                    wire.file_name = Some(fname);
                    wire.mime = Some(fmime);
                }
            }
            wire.blob_ref = None;
            RekeyOutcome::Rewrapped
        }
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: blob shared-encrypt failed: {e}");
            RekeyOutcome::Failed
        }
    }
}

pub(super) fn rekey_blob_outbound(crypto: &SyncCrypto, wire: &mut WireItem) -> RekeyOutcome {
    let Some(shared) = crypto.shared_sync_key() else {
        return RekeyOutcome::NotApplicable;
    };
    rekey_blob_outbound_with_key(crypto, &shared, wire)
}

/// Re-encrypt an outgoing item's payload under the shared content sync key so a
/// paired peer can decrypt it (P2P Phase 3).
///
/// Decrypts the row's at-rest ciphertext with this device's local key (by
/// `key_version`), then re-encrypts the plaintext under the shared sync key via
/// [`encrypt_for_cloud`] (XChaCha20-Poly1305, AAD bound to `item_id`). The
/// resulting self-framed blob (its own 24-byte nonce prefix + ciphertext+tag)
/// is placed in `wire.content` and `wire.content_nonce` is cleared to `None`,
/// which the receiver uses as the "sync-key-wrapped" marker.
///
/// Returns [`RekeyOutcome`]:
/// * [`RekeyOutcome::Rewrapped`] — payload re-wrapped, safe to forward.
/// * [`RekeyOutcome::NotApplicable`] — non-text, no content/nonce, or no shared
///   key: the wire item is left UNCHANGED and follows the legacy path.
/// * [`RekeyOutcome::Failed`] — a shared key was present but the crypto step
///   failed; the wire still carries raw at-rest ciphertext the peer cannot
///   decrypt, so the caller must DROP it (sync H2).
///
/// Image and file items are re-keyed by reassembling the at-rest chunk blob
/// into plaintext (decoded with the LOCAL v1 seed + `file_id` AAD), then
/// re-wrapping that whole plaintext under the shared sync key — identical wire
/// shape to text (`content_nonce = None`, `blob_ref = None`, `content_type`
/// preserved). See [`recover_blob_plaintext`] / [`rekey_blob_outbound`].
pub(super) fn rekey_outbound(crypto: &SyncCrypto, wire: &mut WireItem) -> RekeyOutcome {
    if wire.content_type == "image" || wire.content_type == "file" {
        return rekey_blob_outbound(crypto, wire);
    }
    if wire.content_type != "text" {
        return RekeyOutcome::NotApplicable;
    }
    let Some(shared) = crypto.shared_sync_key() else {
        return RekeyOutcome::NotApplicable;
    };
    rekey_outbound_text_with_key(crypto, &shared, wire)
}

/// Inner text re-key under an explicit `SyncKey` (CopyPaste-716: per-peer key).
///
/// Decrypts the at-rest ciphertext under `crypto`'s local key, then
/// re-encrypts under `peer_key`. Caller is responsible for passing the correct
/// per-peer key (via [`SyncCrypto::sync_key_for_peer`]).
pub(super) fn rekey_outbound_text_with_key(
    crypto: &SyncCrypto,
    peer_key: &SyncKey,
    wire: &mut WireItem,
) -> RekeyOutcome {
    let (Some(ciphertext), Some(nonce_vec)) = (wire.content.as_ref(), wire.content_nonce.as_ref())
    else {
        return RekeyOutcome::NotApplicable;
    };
    // From here on a shared key IS present and the item IS a re-key candidate,
    // so any failure must surface as `Failed` (drop), never silent forward.
    let mut nonce = [0u8; NONCE_SIZE];
    if nonce_vec.len() != NONCE_SIZE {
        warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound wrong nonce length, dropping");
        return RekeyOutcome::Failed;
    }
    nonce.copy_from_slice(nonce_vec);

    let plaintext = match decrypt_item_by_version(
        wire.key_version,
        V1Key(&crypto.v1_key),
        V2Key(&crypto.v2_key),
        &copypaste_core::ItemId::from(wire.item_id.as_str()),
        &nonce,
        ciphertext,
    ) {
        Ok(pt) => pt,
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound local-decrypt failed: {e}");
            return RekeyOutcome::Failed;
        }
    };

    match encrypt_for_cloud(peer_key, &wire.item_id, &plaintext) {
        Ok(blob) => {
            wire.content = Some(blob);
            // The cloud blob is self-framed (nonce prefix), so there is no
            // separate item-level nonce. `None` is the receiver's unwrap marker.
            wire.content_nonce = None;
            RekeyOutcome::Rewrapped
        }
        Err(e) => {
            warn!(item_id = %wire.item_id, "sync_orch: rekey_outbound shared-encrypt failed: {e}");
            RekeyOutcome::Failed
        }
    }
}

/// Re-encrypt an outgoing item under the pairwise sync key for `peer_fingerprint`.
///
/// CopyPaste-716: this is the correct per-peer fanout call. Unlike
/// `rekey_outbound` (which uses the first cached key for legacy/catchup
/// compatibility), this function looks up the sync key specific to
/// `peer_fingerprint` from the per-peer cache. The caller (fanout + catchup
/// paths) must clone the `WireItem` before calling so each peer gets its own
/// independently-encrypted copy.
///
/// Returns [`RekeyOutcome`]:
/// * [`RekeyOutcome::Rewrapped`] — payload re-wrapped under the peer's key.
/// * [`RekeyOutcome::NotApplicable`] — peer has no sync key, or item type is
///   not re-keyable (non-text/image/file). Wire item is left unchanged.
/// * [`RekeyOutcome::Failed`] — key present but crypto failed; caller must drop.
pub fn rekey_outbound_for_peer(
    crypto: &SyncCrypto,
    peer_fingerprint: &str,
    wire: &mut WireItem,
) -> RekeyOutcome {
    let Some(peer_key) = crypto.sync_key_for_peer(peer_fingerprint) else {
        return RekeyOutcome::NotApplicable;
    };
    if wire.content_type == "image" || wire.content_type == "file" {
        return rekey_blob_outbound_with_key(crypto, &peer_key, wire);
    }
    if wire.content_type != "text" {
        return RekeyOutcome::NotApplicable;
    }
    rekey_outbound_text_with_key(crypto, &peer_key, wire)
}
