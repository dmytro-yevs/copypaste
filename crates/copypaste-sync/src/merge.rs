//! Last-Write-Wins (LWW) merge logic for clipboard items.
//!
//! Conflict resolution rules (in priority order):
//!  1. Higher `lamport_ts` wins — the causally-later write takes precedence.
//!  2. On equal Lamport timestamps, higher `wall_time` (Unix ms) wins.
//!  3. On equal wall times, lexicographically larger `origin_device_id` wins
//!     (deterministic tie-break so both sides converge to the same item).
//!
//! This module is pure logic — no I/O, no database access.
use crate::protocol::WireItem;
use copypaste_core::storage::items::ClipboardItem;

/// Outcome of comparing two versions of the *same* logical item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeOutcome {
    /// Keep the local version unchanged.
    KeepLocal,
    /// Replace the local version with the remote one.
    TakeRemote,
}

/// Compare a locally-stored item against a remote version of the same item.
///
/// `local.item_id` and `remote.item_id` must be equal (same logical item).
/// Identity is the cross-device `item_id`, NOT the per-row `id` (which is a
/// fresh `Uuid::new_v4()` on every device and so differs for the same item).
/// Returns `TakeRemote` if the remote version should win, `KeepLocal` otherwise.
pub fn resolve(local: &ClipboardItem, remote: &WireItem) -> MergeOutcome {
    debug_assert_eq!(
        local.item_id, remote.item_id,
        "resolve called on different items (item_id mismatch)"
    );

    match remote.lamport_ts.cmp(&local.lamport_ts) {
        std::cmp::Ordering::Greater => MergeOutcome::TakeRemote,
        std::cmp::Ordering::Less => MergeOutcome::KeepLocal,
        std::cmp::Ordering::Equal => {
            // Tie-break by wall time.
            match remote.wall_time.cmp(&local.wall_time) {
                std::cmp::Ordering::Greater => MergeOutcome::TakeRemote,
                std::cmp::Ordering::Less => MergeOutcome::KeepLocal,
                std::cmp::Ordering::Equal => {
                    // Final tie-break by `origin_device_id` (lexicographic,
                    // larger wins). Before schema v3 this branch compared
                    // `remote.origin_device_id` against `local.id` (the row
                    // UUID) — two completely different identifier spaces.
                    // Row UUIDs are random per-write while device ids are
                    // stable per-peer, so the result was non-deterministic
                    // and frequently bogus: two peers could pick different
                    // winners, causing CRDT divergence (merge.rs:39 BUG).
                    // v3 added `ClipboardItem::origin_device_id` so we now
                    // compare the same field on both sides, matching the
                    // module contract above and converging every peer to
                    // the same winner.
                    if remote.origin_device_id > local.origin_device_id {
                        MergeOutcome::TakeRemote
                    } else {
                        MergeOutcome::KeepLocal
                    }
                }
            }
        }
    }
}

/// Convert a `WireItem` received from a peer into a `ClipboardItem` ready to
/// be persisted locally, marking it as synced.
pub fn wire_to_local(wire: WireItem) -> ClipboardItem {
    ClipboardItem {
        id: wire.id,
        item_id: wire.item_id,
        content_type: wire.content_type,
        content: wire.content,
        content_nonce: wire.content_nonce,
        blob_ref: wire.blob_ref,
        is_sensitive: wire.is_sensitive,
        is_synced: true,
        lamport_ts: wire.lamport_ts,
        wall_time: wire.wall_time,
        expires_at: wire.expires_at,
        app_bundle_id: wire.app_bundle_id,
        content_hash: None,
        // Preserve the peer's origin so future tie-breaks remain
        // deterministic regardless of which peer replays the merge.
        origin_device_id: wire.origin_device_id,
        // Preserve the sender's key_version. The `content` ciphertext + AAD
        // were produced under THIS version on the sending device; the local
        // read path (`decrypt_item_by_version`) dispatches on it to pick the
        // matching key + AAD. Hard-coding 1 here (the prior bug) made every
        // v2-encrypted synced item undecryptable on the receiver — the v1 key
        // + v3 AAD never matched the v2 ciphertext, so reads failed with
        // `EncryptError::AuthFailed`.
        key_version: wire.key_version,
        // Received items are never pinned by default; the user must pin them
        // explicitly on this device after syncing.
        pinned: false,
        // pin_order is a local UI concept — it is not synced across devices.
        // Received items start with NULL and get a position only if the user
        // explicitly pins them (via pin_item, which assigns MAX+1).
        pin_order: None,
        // thumb is a local-only, capture-time derived image thumbnail (schema
        // v9). It is NOT part of the WireItem sync payload; received items start
        // with no thumbnail and can be backfilled later via `set_thumb`.
        thumb: None,
    }
}

/// Convert a local `ClipboardItem` into a `WireItem` for transmission.
///
/// If the item already carries an `origin_device_id` (received from a peer or
/// stamped on local creation) that value is preserved; otherwise
/// `local_device_id` is used. Preserving the existing origin is essential for
/// the LWW tie-break: every peer must see the same origin for the same item
/// regardless of who relays it.
pub fn local_to_wire(item: &ClipboardItem, local_device_id: &str) -> WireItem {
    let origin = if item.origin_device_id.is_empty() {
        local_device_id.to_string()
    } else {
        item.origin_device_id.clone()
    };

    // For file items, extract filename + mime from the at-rest blob_ref meta
    // JSON (produced by `build_file_meta_json`) so the receiver can reconstruct
    // the correct file identity without having to inspect blob_ref (which the
    // daemon's rekey path clears before forwarding — see `rekey_blob_outbound`).
    let (file_name, mime) = if item.content_type == "file" {
        parse_file_meta_name_mime(item.blob_ref.as_deref())
    } else {
        (None, None)
    };

    WireItem {
        id: item.id.clone(),
        item_id: item.item_id.clone(),
        content_type: item.content_type.clone(),
        content: item.content.clone(),
        content_nonce: item.content_nonce.clone(),
        blob_ref: item.blob_ref.clone(),
        is_sensitive: item.is_sensitive,
        lamport_ts: item.lamport_ts,
        wall_time: item.wall_time,
        expires_at: item.expires_at,
        app_bundle_id: item.app_bundle_id.clone(),
        origin_device_id: origin,
        // Carry the row's real key_version so the receiver can select the
        // correct key + AAD when decrypting `content` (see wire_to_local).
        key_version: item.key_version,
        file_name,
        mime,
    }
}

/// Parse `filename` and `mime` from a file blob_ref meta JSON string.
///
/// The meta JSON is produced by `clipboard::build_file_meta_json`; its shape
/// carries a `"filename"` and a `"mime"` field at the top level.  Returns
/// `(None, None)` on any parse failure so callers degrade gracefully.
fn parse_file_meta_name_mime(meta_json: Option<&str>) -> (Option<String>, Option<String>) {
    let Some(json) = meta_json else {
        return (None, None);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(json) else {
        return (None, None);
    };
    let filename = value
        .get("filename")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let mime = value
        .get("mime")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    (filename, mime)
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::storage::items::ClipboardItem;

    fn make_local(lamport: i64, wall: i64) -> ClipboardItem {
        ClipboardItem {
            id: "item-001".to_string(),
            item_id: "iid-001".to_string(),
            content_type: "text".to_string(),
            content: Some(vec![1, 2, 3]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts: lamport,
            wall_time: wall,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "device-local".to_string(),
            key_version: 1,
            pinned: false,
            pin_order: None,
            thumb: None,
        }
    }

    fn make_remote(lamport: i64, wall: i64, device_id: &str) -> WireItem {
        WireItem {
            id: "item-001".to_string(),
            item_id: "iid-001".to_string(),
            content_type: "text".to_string(),
            content: Some(vec![4, 5, 6]),
            content_nonce: Some(vec![0u8; 24]),
            blob_ref: None,
            is_sensitive: false,
            lamport_ts: lamport,
            wall_time: wall,
            expires_at: None,
            app_bundle_id: None,
            origin_device_id: device_id.to_string(),
            key_version: 2,
            file_name: None,
            mime: None,
        }
    }

    // --- Lamport clock ordering ---

    #[test]
    fn higher_remote_lamport_wins() {
        let local = make_local(5, 1000);
        let remote = make_remote(10, 500, "peer-A"); // higher lamport, lower wall
        assert_eq!(resolve(&local, &remote), MergeOutcome::TakeRemote);
    }

    #[test]
    fn higher_local_lamport_keeps_local() {
        let local = make_local(15, 500);
        let remote = make_remote(3, 9999, "peer-A"); // lower lamport, higher wall
        assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);
    }

    // --- Wall-time tie-break ---

    #[test]
    fn equal_lamport_higher_remote_wall_wins() {
        let local = make_local(5, 1000);
        let remote = make_remote(5, 2000, "peer-A");
        assert_eq!(resolve(&local, &remote), MergeOutcome::TakeRemote);
    }

    #[test]
    fn equal_lamport_higher_local_wall_keeps_local() {
        let local = make_local(5, 9000);
        let remote = make_remote(5, 1000, "peer-A");
        assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);
    }

    // --- Device-ID tie-break (determinism) ---

    #[test]
    fn equal_lamport_equal_wall_larger_device_id_wins() {
        // local.origin_device_id == "device-local"; remote "zzz" > "device-local"
        let local = make_local(5, 1000);
        let remote_wins = make_remote(5, 1000, "zzz");
        assert_eq!(resolve(&local, &remote_wins), MergeOutcome::TakeRemote);

        // remote "aaa" < "device-local" → local keeps
        let local_wins = make_remote(5, 1000, "aaa");
        assert_eq!(resolve(&local, &local_wins), MergeOutcome::KeepLocal);
    }

    #[test]
    fn equal_lamport_equal_wall_equal_device_keeps_local() {
        // Two peers with the same origin (e.g. same item replayed) converge
        // to KeepLocal — the comparison is a strict `>`.
        let local = make_local(5, 1000);
        let remote = make_remote(5, 1000, "device-local");
        assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);
    }

    // --- wire_to_local ---

    #[test]
    fn wire_to_local_marks_synced_and_preserves_origin() {
        let wire = make_remote(7, 2000, "dev-X");
        let local = wire_to_local(wire.clone());
        assert!(local.is_synced);
        assert_eq!(local.lamport_ts, 7);
        assert_eq!(local.content, wire.content);
        assert_eq!(
            local.origin_device_id, "dev-X",
            "peer origin must survive wire_to_local so tie-breaks stay \
             deterministic across hops"
        );
    }

    // --- local_to_wire ---

    #[test]
    fn local_to_wire_preserves_existing_origin() {
        // Item already has an origin (received from a peer earlier and now
        // being relayed). local_to_wire must keep the original origin, not
        // overwrite it with the local device id.
        let mut item = make_local(3, 500);
        item.origin_device_id = "peer-A".to_string();
        let wire = local_to_wire(&item, "my-device");
        assert_eq!(wire.origin_device_id, "peer-A");
    }

    #[test]
    fn local_to_wire_stamps_local_when_origin_empty() {
        // Pre-backfill row (or legacy v2 row migrated up). local_to_wire
        // stamps the local device id as a safe default.
        let mut item = make_local(3, 500);
        item.origin_device_id = String::new();
        let wire = local_to_wire(&item, "my-device");
        assert_eq!(wire.id, item.id);
        assert_eq!(wire.lamport_ts, 3);
        assert_eq!(wire.origin_device_id, "my-device");
        assert_eq!(wire.content, item.content);
    }

    // --- key_version round-trip (crypto correctness regression) ---

    /// v0.4 sync key-version regression (HIGH, crypto). The sending side
    /// stamps the row's real `key_version` (2 for every freshly-captured
    /// item) and the ciphertext/AAD are bound to that version. The receiver
    /// MUST persist the same `key_version` so its production read path
    /// (`decrypt_item_by_version`) selects the matching key + AAD and recovers
    /// the original plaintext.
    ///
    /// This test reproduces the REAL data flow end-to-end:
    ///   encrypt @ key_version=2 (v2 key + v4 AAD, exactly as `handle_text`)
    ///     → `local_to_wire` → `wire_to_local` (the conversion under test)
    ///     → `decrypt_item_by_version` (the production read path)
    /// and asserts the original bytes survive.
    ///
    /// Before the fix `wire_to_local` HARD-CODED `key_version = 1`, so the
    /// receiver decrypted a v2 ciphertext with the v1 key + v3 AAD → the AEAD
    /// auth tag rejected it (`EncryptError::AuthFailed`). Net effect: every
    /// item received via sync was undecryptable on the receiver.
    #[test]
    fn wire_round_trip_preserves_key_version_so_receiver_can_decrypt() {
        use copypaste_core::{
            build_item_aad_v2, decrypt_item_by_version, derive_v2, encrypt_item_with_aad,
            AAD_SCHEMA_VERSION_V4, NONCE_SIZE,
        };

        // The device's v1 storage seed (stands in for `load_local_key()`).
        let seed = [0x42u8; 32];
        let item_id = "iid-roundtrip".to_string();
        let plaintext = b"sensitive clipboard payload synced from a peer";

        // SENDER: encrypt exactly as `encrypt_text_for_storage` does —
        // v2 key + v4 AAD bound to (item_id, schema_version=4, key_version=2).
        let v2_key = derive_v2(&seed);
        let aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
        let (nonce, ciphertext) =
            encrypt_item_with_aad(plaintext, &v2_key, &aad).expect("encrypt at key_version=2");

        // The stored row is stamped key_version = 2 (ClipboardItem::new_text).
        let mut sent = make_local(7, 2000);
        sent.item_id = item_id.clone();
        sent.content = Some(ciphertext.clone());
        sent.content_nonce = Some(nonce.to_vec());
        sent.key_version = 2;
        sent.origin_device_id = "sender-device".to_string();

        // local_to_wire → wire_to_local: the real conversion the daemon runs.
        let wire = local_to_wire(&sent, "sender-device");
        let received = wire_to_local(wire);

        // The received row must carry the SAME key_version the ciphertext was
        // produced under, otherwise the read path picks the wrong key/AAD.
        assert_eq!(
            received.key_version, 2,
            "wire_to_local must preserve the sender's key_version (2), not \
             hard-code 1 — otherwise the v2 ciphertext is decrypted with the \
             v1 key + v3 AAD and fails with AuthFailed"
        );

        // RECEIVER read path: dispatch on the stored key_version exactly as
        // `ipc::write_to_pasteboard` does (v1_key = seed, v2_key = derive_v2).
        let v1_key = seed;
        let stored_nonce = received.content_nonce.expect("nonce present");
        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&stored_nonce);
        let stored_content = received.content.expect("content present");

        let recovered = decrypt_item_by_version(
            received.key_version,
            &v1_key,
            &v2_key,
            &received.item_id,
            &nonce_arr,
            &stored_content,
        )
        .expect("receiver read path must decrypt the synced item");

        assert_eq!(
            recovered, plaintext,
            "synced item must read back as the original plaintext on the receiver"
        );
    }

    // --- file_name / mime population (#21b) ---

    /// `local_to_wire` must extract filename and mime from the at-rest blob_ref
    /// meta JSON for `content_type = "file"` items, so `rekey_blob_outbound`
    /// can stamp them on the wire before clearing blob_ref.
    #[test]
    fn local_to_wire_file_item_extracts_filename_and_mime_from_blob_ref() {
        // Minimal file meta JSON produced by `build_file_meta_json`.
        let meta_json = r#"{"file_id":[0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0],"filename":"report.pdf","mime":"application/pdf","size":1024}"#;

        let mut item = make_local(5, 1000);
        item.content_type = "file".to_string();
        item.blob_ref = Some(meta_json.to_string());

        let wire = local_to_wire(&item, "my-device");

        assert_eq!(
            wire.file_name.as_deref(),
            Some("report.pdf"),
            "local_to_wire must extract filename from file blob_ref meta JSON"
        );
        assert_eq!(
            wire.mime.as_deref(),
            Some("application/pdf"),
            "local_to_wire must extract mime from file blob_ref meta JSON"
        );
    }

    /// For non-file items (`content_type = "text"`) the new wire fields must stay None.
    #[test]
    fn local_to_wire_text_item_leaves_filename_mime_none() {
        let item = make_local(1, 100);
        let wire = local_to_wire(&item, "my-device");
        assert!(
            wire.file_name.is_none(),
            "text items must not set file_name"
        );
        assert!(wire.mime.is_none(), "text items must not set mime");
    }
}
