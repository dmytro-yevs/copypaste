//! Text rekey round-trip + per-peer key isolation tests, relocated out of the
//! former flat `sync_orch/mod.rs` test module (ADR-017, CopyPaste-vp63.3).
//! Image/file blob round-trips live in `blob_tests.rs`.

use super::*;

/// P2P Phase 3 (cross-device readability): an item encrypted at rest under
/// device A's per-device local key must, after `rekey_outbound` (shared key
/// re-wrap) → `rekey_inbound` (shared key unwrap + re-wrap under device B's
/// local key), be readable on B via the production read path
/// (`decrypt_item_by_version`). A and B have DIFFERENT local seeds — the
/// whole point of the shared sync key.
#[test]
fn rekey_round_trip_makes_item_readable_on_peer_with_different_local_key() {
    use base64::Engine as _;
    use copypaste_core::{
        build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
    };
    use copypaste_sync::protocol::WireItem;
    use tempfile::tempdir;

    // Two distinct devices, two distinct local seeds.
    let seed_a = [0x11u8; 32];
    let seed_b = [0x22u8; 32];
    assert_ne!(seed_a, seed_b);

    // The shared content sync key both sides persisted at pairing (same 32
    // bytes on both — here a fixed test value).
    let shared = [0x33u8; 32];
    let shared_b64 = base64::engine::general_purpose::STANDARD.encode(shared);

    let item_id = "iid-rekey-001".to_string();
    let plaintext = b"the answer is 42 and it travelled over real P2P";

    // SENDER A: item is stored encrypted under A's v2 local key (exactly as
    // a freshly-captured text item is).
    let a_v2 = derive_v2(&seed_a);
    let aad_a = build_item_aad_v2(
        &copypaste_core::ItemId::from(item_id.as_str()),
        AAD_SCHEMA_VERSION_V4,
        2,
    );
    let (nonce_a, ct_a) = encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

    // A's wire item carries A's at-rest ciphertext + nonce.
    let mut wire = WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: "row-1".to_string(),
        item_id: item_id.clone(),
        content_type: "text".to_string(),
        content: Some(ct_a),
        content_nonce: Some(nonce_a.to_vec()),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: 7,
        wall_time: 1_700_000_000_000,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "device-A".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    };

    // A's peers.json holds the shared key (peer = B).
    let dir_a = tempdir().unwrap();
    let peers_a = dir_a.path().join("peers.json");
    std::fs::write(
        &peers_a,
        format!(
            r#"[{{"fingerprint":"bb:bb","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
        ),
    )
    .unwrap();
    let crypto_a = SyncCrypto::new(seed_a, peers_a);

    // OUTBOUND: re-key under the shared sync key.
    rekey_outbound(&crypto_a, &mut wire);
    assert!(
        wire.content_nonce.is_none(),
        "sync-key-wrapped payload must clear the item nonce (self-framed blob)"
    );
    // The wire content is no longer A's at-rest ciphertext.
    assert_ne!(wire.content.as_deref(), Some(plaintext.as_slice()));

    // RECEIVER B: different local seed, same shared key in its peers.json.
    let dir_b = tempdir().unwrap();
    let peers_b = dir_b.path().join("peers.json");
    std::fs::write(
        &peers_b,
        format!(
            r#"[{{"fingerprint":"aa:aa","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{shared_b64}"}}]"#
        ),
    )
    .unwrap();
    let crypto_b = SyncCrypto::new(seed_b, peers_b);

    // INBOUND: unwrap with shared key, re-wrap under B's local v2 key.
    let (stored, recovered_pt) = rekey_inbound(
        &crypto_b,
        wire,
        copypaste_core::config::MAX_DECODED_IMAGE_MB,
    )
    .expect("B must unwrap the sync-key-wrapped item");
    assert_eq!(
        recovered_pt.as_deref(),
        Some(plaintext.as_slice()),
        "B recovers A's original plaintext"
    );
    assert_eq!(stored.key_version, 2);

    // B's production read path: decrypt the stored row with B's keys.
    let b_v2 = derive_v2(&seed_b);
    let stored_nonce = stored.content_nonce.expect("nonce");
    let mut narr = [0u8; copypaste_core::NONCE_SIZE];
    narr.copy_from_slice(&stored_nonce);
    let read_back = copypaste_core::decrypt_item_by_version(
        stored.key_version,
        copypaste_core::V1Key(&seed_b),
        copypaste_core::V2Key(&b_v2),
        &stored.item_id,
        &narr,
        &stored.content.expect("content"),
    )
    .expect("B read path must decrypt the stored synced item");
    assert_eq!(
        read_back, plaintext,
        "synced item reads back as A's original plaintext on B"
    );
}

/// CopyPaste-716: 3-device topology (A paired with B and C under DIFFERENT
/// pairwise keys) must produce per-peer ciphertext blobs.
///
/// Before the fix: `rekey_outbound` used `shared_sync_key()` (first peer
/// only) so fanout to C produced a K_AB-encrypted blob that C could never
/// decrypt. After the fix: `rekey_outbound_for_peer` uses the per-peer key
/// from the HashMap cache, so each peer gets a blob it can actually decrypt.
///
/// This test simulates device A sending to peers B and C:
/// - K_AB = [0x33; 32], K_AC = [0x44; 32] (distinct pairwise keys)
/// - Fanout to B must produce a blob decryptable under K_AB (not K_AC)
/// - Fanout to C must produce a blob decryptable under K_AC (not K_AB)
#[test]
fn three_device_fanout_uses_per_peer_key_not_first_peer_key() {
    use base64::Engine as _;
    use copypaste_core::{
        build_item_aad_v2, decrypt_from_cloud, derive_v2, encrypt_item_with_aad,
        AAD_SCHEMA_VERSION_V4,
    };
    use copypaste_sync::protocol::WireItem;
    use tempfile::tempdir;

    // Device A's local seed.
    let seed_a = [0x11u8; 32];
    let k_ab: [u8; 32] = [0x33u8; 32];
    let k_ac: [u8; 32] = [0x44u8; 32];
    assert_ne!(k_ab, k_ac, "pairwise keys must be distinct");
    let k_ab_b64 = base64::engine::general_purpose::STANDARD.encode(k_ab);
    let k_ac_b64 = base64::engine::general_purpose::STANDARD.encode(k_ac);
    let fp_b = "bb:bb";
    let fp_c = "cc:cc";

    // Device A's peers.json: two peers with different pairwise keys.
    let dir_a = tempdir().unwrap();
    let peers_a = dir_a.path().join("peers.json");
    std::fs::write(
        &peers_a,
        format!(
            r#"[
                {{"fingerprint":"{fp_b}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_ab_b64}"}},
                {{"fingerprint":"{fp_c}","added_at":1,"address":"127.0.0.1:8","sync_key_b64":"{k_ac_b64}"}}
            ]"#
        ),
    )
    .unwrap();
    let crypto_a = SyncCrypto::new(seed_a, peers_a);

    // Build a wire item (A's at-rest ciphertext).
    let item_id = "fanout-716-item".to_string();
    let plaintext = b"the shared secret content for 3-device test";
    let a_v2 = derive_v2(&seed_a);
    let aad_a = build_item_aad_v2(
        &copypaste_core::ItemId::from(item_id.as_str()),
        AAD_SCHEMA_VERSION_V4,
        2,
    );
    let (nonce_a, ct_a) = encrypt_item_with_aad(plaintext, &a_v2, &aad_a).expect("A local encrypt");

    let wire_template = WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: "row-716".to_string(),
        item_id: item_id.clone(),
        content_type: "text".to_string(),
        content: Some(ct_a),
        content_nonce: Some(nonce_a.to_vec()),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: 1,
        wall_time: 1_700_000_000_000,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "device-A".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    };

    // ── Fanout to peer B (should use K_AB) ───────────────────────────────
    let mut wire_for_b = wire_template.clone();
    let outcome_b = rekey_outbound_for_peer(&crypto_a, fp_b, &mut wire_for_b);
    assert_eq!(
        outcome_b,
        RekeyOutcome::Rewrapped,
        "fanout to B must succeed (K_AB present)"
    );
    assert!(
        wire_for_b.content_nonce.is_none(),
        "sync-key-wrapped payload clears item nonce"
    );
    let blob_b = wire_for_b.content.as_ref().unwrap().clone();

    // ── Fanout to peer C (should use K_AC) ───────────────────────────────
    let mut wire_for_c = wire_template.clone();
    let outcome_c = rekey_outbound_for_peer(&crypto_a, fp_c, &mut wire_for_c);
    assert_eq!(
        outcome_c,
        RekeyOutcome::Rewrapped,
        "fanout to C must succeed (K_AC present)"
    );
    let blob_c = wire_for_c.content.as_ref().unwrap().clone();

    // ── Verify: B's blob decrypts under K_AB ─────────────────────────────
    let key_b = copypaste_core::SyncKey::from_bytes(k_ab);
    let decrypted_b =
        decrypt_from_cloud(&key_b, &item_id, &blob_b).expect("blob_b must decrypt under K_AB");
    assert_eq!(
        decrypted_b, plaintext,
        "B recovers A's original plaintext from its blob"
    );

    // ── Verify: C's blob decrypts under K_AC ─────────────────────────────
    let key_c = copypaste_core::SyncKey::from_bytes(k_ac);
    let decrypted_c =
        decrypt_from_cloud(&key_c, &item_id, &blob_c).expect("blob_c must decrypt under K_AC");
    assert_eq!(
        decrypted_c, plaintext,
        "C recovers A's original plaintext from its blob"
    );

    // ── Key isolation: B's blob must NOT decrypt under K_AC ──────────────
    // (This is what was silently broken before the fix: fanout used K_AB for
    // all peers, so C received blob_b which it cannot decrypt.)
    let result_wrong = decrypt_from_cloud(&key_c, &item_id, &blob_b);
    assert!(
        result_wrong.is_err(),
        "blob_b (encrypted under K_AB) must NOT decrypt under K_AC — \
         this would be the CopyPaste-716 bug if it succeeded"
    );

    // ── Key isolation: C's blob must NOT decrypt under K_AB ──────────────
    let result_wrong2 = decrypt_from_cloud(&key_b, &item_id, &blob_c);
    assert!(
        result_wrong2.is_err(),
        "blob_c (encrypted under K_AC) must NOT decrypt under K_AB"
    );

    // ── Blobs differ (per-peer encryption with fresh nonces) ─────────────
    assert_ne!(
        blob_b, blob_c,
        "each peer must receive a distinct (independently encrypted) blob"
    );
}

/// CopyPaste-kw2: 3-device topology — inbound item from peer C (encrypted
/// under K_AC) must be decrypted correctly even when K_AB is the first
/// entry in the peer key map.
///
/// Before the fix: `rekey_inbound` called `shared_sync_key()` which returned
/// `values().next()` — an arbitrary HashMap entry.  If K_AB was first, the
/// item from C (encrypted under K_AC) failed to decrypt and was silently
/// dropped.
///
/// After the fix: `all_sync_keys()` returns all keys; `rekey_inbound` tries
/// each until AEAD succeeds, so K_AC is always found regardless of iteration
/// order.
#[test]
fn rekey_inbound_3_device_tries_all_keys() {
    use base64::Engine as _;
    use copypaste_core::{decrypt_from_cloud, encrypt_for_cloud};
    use copypaste_sync::protocol::WireItem;
    use tempfile::tempdir;

    // Distinct pairwise keys: K_AB and K_AC.
    let k_ab: [u8; 32] = [0x33u8; 32];
    let k_ac: [u8; 32] = [0x44u8; 32];
    assert_ne!(k_ab, k_ac);
    let k_ab_b64 = base64::engine::general_purpose::STANDARD.encode(k_ab);
    let k_ac_b64 = base64::engine::general_purpose::STANDARD.encode(k_ac);

    let fp_b = "bb:bb";
    let fp_c = "cc:cc";
    let seed_b_recv = [0x22u8; 32]; // Device B's local seed (the receiver in this test)

    // Device B's peers.json: both A and C, each with their own pairwise key.
    // B shares K_AB with A and K_BC with C — but for this test B receives
    // a blob from C encrypted under K_AC (the key A and C share).
    // We simulate a direct A→B case: B holds K_AB (to reach A) and K_AC (wrong).
    // The payload we produce is encrypted under K_AC and B should find it by
    // trying all keys.
    //
    // To make the scenario concrete: pretend this device IS A and receives a
    // blob from C (encrypted under K_AC).  A's peer map has K_AB (for B) first
    // and K_AC (for C) second.  The fix ensures K_AC is tried.
    let dir = tempdir().unwrap();
    let peers_path = dir.path().join("peers.json");
    std::fs::write(
        &peers_path,
        format!(
            r#"[
                {{"fingerprint":"{fp_b}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_ab_b64}"}},
                {{"fingerprint":"{fp_c}","added_at":1,"address":"127.0.0.1:8","sync_key_b64":"{k_ac_b64}"}}
            ]"#
        ),
    )
    .unwrap();
    let crypto = SyncCrypto::new(seed_b_recv, peers_path);

    // Build a wire item encrypted under K_AC (as if sent by peer C).
    let item_id = "kw2-test-item".to_string();
    let plaintext = b"secret payload from peer C";
    let key_ac = copypaste_core::SyncKey::from_bytes(k_ac);
    let blob_ac = encrypt_for_cloud(&key_ac, &item_id, plaintext).expect("encrypt under K_AC");

    let wire = WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: "kw2-row".to_string(),
        item_id: item_id.clone(),
        content_type: "text".to_string(),
        content: Some(blob_ac),
        content_nonce: None, // sync-key-wrapped (no local nonce)
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: 1,
        wall_time: 1_700_000_000_000,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "device-C".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    };

    // rekey_inbound must succeed even if K_AB is iterated before K_AC.
    let (stored, fts_plaintext) =
        rekey_inbound(&crypto, wire, copypaste_core::config::MAX_DECODED_IMAGE_MB)
            .expect("must decrypt under K_AC (CopyPaste-kw2 fix)");

    assert_eq!(
        fts_plaintext.as_deref(),
        Some(plaintext.as_slice()),
        "recovered plaintext must match original"
    );
    // The stored row must be re-encrypted under this device's v2 key.
    assert!(
        stored.content_nonce.is_some(),
        "stored row must have a local nonce after rekey"
    );
    assert_eq!(stored.key_version, 2, "stored row must be keyed at v2");

    // Sanity: the wrong key (K_AB) alone would have failed.
    let key_ab = copypaste_core::SyncKey::from_bytes(k_ab);
    let blob_from_stored = stored.content.as_ref().unwrap();
    // The stored ciphertext is under the device's v2 key, not K_AB — just
    // confirm K_AB cannot decrypt the original blob (key isolation).
    // We reconstruct the original blob to check isolation.
    let original_blob = encrypt_for_cloud(&key_ac, &item_id, plaintext).unwrap();
    assert!(
        decrypt_from_cloud(&key_ab, &item_id, &original_blob).is_err(),
        "K_AB must not decrypt a blob encrypted under K_AC — key isolation"
    );
    let _ = blob_from_stored; // silence unused warning
}
