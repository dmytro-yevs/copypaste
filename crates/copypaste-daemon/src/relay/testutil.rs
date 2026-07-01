//! Shared test-only builders for the relay push/receive/registration/pasteboard
//! test suites (CopyPaste-vp63.25).
//!
//! Centralizing these here (instead of copy-pasting into each submodule's own
//! `#[cfg(test)] mod tests`) is the reuse win flagged by the relay/mod.rs split:
//! every submodule that exercises the ingest/push/registration pipeline needs
//! the SAME sync-key derivation, HTTP test client, in-memory DB, and
//! locally-encrypted / relay-envelope item builders.
//!
//! This whole module only compiles under `#[cfg(test)]` (see the `mod
//! testutil;` declaration in `relay/mod.rs`), so every item here is a plain
//! `pub(super)` — visible to `relay` and all of its descendant test modules
//! (`push`, `pasteboard`, `registration`, `receive`, `receive::ingest`, ...).

use base64::Engine as _;
use copypaste_core::{
    build_item_aad_v2, derive_sync_key, derive_v2, encrypt_for_cloud, encrypt_item_with_aad,
    ClipboardItem, Database, SyncKey, AAD_SCHEMA_VERSION_V4, ITEM_KEY_VERSION_CURRENT,
};

use super::types::{PullItem, RelayEnvelope};

/// Derive a stable 32-byte sync key from a test passphrase. The relay takes
/// raw key bytes; the per-account derivation just needs a stable, non-empty
/// account id for the test.
pub(super) fn skey(p: &str) -> [u8; 32] {
    *derive_sync_key(p, "proj_test|00000000-0000-0000-0000-000000000001")
        .expect("derive")
        .as_bytes()
}

/// A `reqwest::Client` with the same timeout the production push/receive
/// loops use, for tests that drive `register` / `push_item` / `pull_page`
/// directly against a mockito server.
pub(super) fn test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(crate::sync_common::SYNC_HTTP_TIMEOUT)
        .build()
        .expect("client")
}

/// A fresh in-memory SQLCipher DB wrapped for the async ingest call sites.
pub(super) fn open_mem_db() -> std::sync::Arc<tokio::sync::Mutex<Database>> {
    let db = Database::open_in_memory().expect("open in-memory db");
    std::sync::Arc::new(tokio::sync::Mutex::new(db))
}

/// Build a locally-stored text `ClipboardItem` (v2 key path) so the upload
/// pipeline's `decrypt_item_plaintext` can read it back.
pub(super) fn make_local_text_item(
    item_id: &str,
    plaintext: &[u8],
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    lamport_ts: i64,
    wall_time: i64,
) -> ClipboardItem {
    let v1: [u8; 32] = **local_key;
    let v2 = derive_v2(&v1);
    let aad = build_item_aad_v2(
        &copypaste_core::ItemId::from(item_id),
        AAD_SCHEMA_VERSION_V4,
        ITEM_KEY_VERSION_CURRENT as u32,
    );
    let (nonce, ct) = encrypt_item_with_aad(plaintext, &v2, &aad).expect("encrypt");
    ClipboardItem {
        deleted: false,
        id: item_id.to_owned().into(),
        item_id: item_id.to_owned().into(),
        content_type: "text".to_owned(),
        content: Some(ct),
        content_nonce: Some(nonce.to_vec()),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts,
        wall_time,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        origin_device_id: "dev-local".to_owned(),
        key_version: ITEM_KEY_VERSION_CURRENT as u8,
        pinned: false,
        pin_order: None,
        thumb: None,
    }
}

/// Build a relay `PullItem` carrying a text payload encrypted for the cloud.
pub(super) fn make_pull_item(
    id: i64,
    item_id: &str,
    plaintext: &[u8],
    sync_key: &SyncKey,
    lamport_ts: i64,
    wall_time: u64,
) -> PullItem {
    let blob = encrypt_for_cloud(sync_key, item_id, plaintext).expect("cloud encrypt");
    let ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
    let env = RelayEnvelope {
        item_id: item_id.to_owned(),
        lamport_ts,
        ct_b64,
        deleted: false,
        pinned: false,
        pin_order: None,
        wall_time: wall_time as i64,
        origin_device_id: "dev-remote".to_owned(),
    };
    envelope_to_pull(id, "text", &env, wall_time)
}

/// Wrap a `RelayEnvelope` into a `PullItem` (the relay-wire row shape).
pub(super) fn envelope_to_pull(
    id: i64,
    content_type: &str,
    env: &RelayEnvelope,
    wall_time: u64,
) -> PullItem {
    let content_b64 = base64::engine::general_purpose::STANDARD
        .encode(serde_json::to_vec(env).expect("env json"));
    PullItem {
        id,
        content_type: content_type.to_owned(),
        content_b64,
        wall_time,
    }
}

/// Build a relay `PullItem` carrying a TOMBSTONE (deleted=true, empty ct).
pub(super) fn make_tombstone_pull(
    id: i64,
    item_id: &str,
    lamport_ts: i64,
    wall_time: u64,
) -> PullItem {
    let env = RelayEnvelope {
        item_id: item_id.to_owned(),
        lamport_ts,
        ct_b64: String::new(),
        deleted: true,
        pinned: false,
        pin_order: None,
        wall_time: wall_time as i64,
        origin_device_id: "dev-remote".to_owned(),
    };
    envelope_to_pull(id, "text", &env, wall_time)
}

/// Build a relay `PullItem` carrying a PINNED text item.
pub(super) fn make_pinned_pull(
    id: i64,
    item_id: &str,
    plaintext: &[u8],
    sync_key: &SyncKey,
    lamport_ts: i64,
    wall_time: u64,
    pin_order: f64,
) -> PullItem {
    let blob = encrypt_for_cloud(sync_key, item_id, plaintext).expect("cloud encrypt");
    let ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
    let env = RelayEnvelope {
        item_id: item_id.to_owned(),
        lamport_ts,
        ct_b64,
        deleted: false,
        pinned: true,
        pin_order: Some(pin_order),
        wall_time: wall_time as i64,
        origin_device_id: "dev-remote".to_owned(),
    };
    envelope_to_pull(id, "text", &env, wall_time)
}
