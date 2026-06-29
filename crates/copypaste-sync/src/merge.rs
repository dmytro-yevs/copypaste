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

/// The three total-order sort keys a remote version carries, in priority order:
/// `lamport_ts` → `wall_time` → `origin_device_id`.
///
/// Cloud (Supabase poll + websocket) and relay ingest paths decode these from
/// their own wire formats and feed them to [`remote_wins`] so all three
/// transports break ties by the SAME deterministic order that the P2P path
/// already uses via [`resolve`]. Without this they diverged: cloud/relay used a
/// bare `remote_lamport <= local -> keep`, which on EQUAL lamport ALWAYS kept
/// the local copy (decided by arrival locality, not content) — so two devices
/// holding the same `item_id` at equal lamport with different content would each
/// keep their own copy and never converge (CopyPaste-ayvs).
#[derive(Debug, Clone)]
pub struct RemoteMeta<'a> {
    /// Remote Lamport timestamp.
    pub lamport_ts: i64,
    /// Remote wall-clock time (Unix ms) — the second-priority tie-break.
    pub wall_time: i64,
    /// Remote originating device id — the final, deterministic tie-break.
    pub origin_device_id: &'a str,
}

/// Decide whether a remote version should overwrite the local one, using the
/// SAME total order as [`resolve`] (`lamport_ts` → `wall_time` →
/// `origin_device_id`, larger wins).
///
/// Returns `true` when the remote wins (the caller should replace/apply the
/// remote version), `false` when the local copy is kept. This is the
/// transport-agnostic primitive the cloud and relay ingest paths share so every
/// transport converges identically (CopyPaste-ayvs). The P2P path uses
/// [`resolve`] against a full [`WireItem`]; this variant takes only the three
/// sort keys because cloud/relay decode them out of distinct wire shapes.
pub fn remote_wins(
    local_lamport: i64,
    local_wall: i64,
    local_origin_device_id: &str,
    remote: &RemoteMeta<'_>,
) -> bool {
    match remote.lamport_ts.cmp(&local_lamport) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => match remote.wall_time.cmp(&local_wall) {
            std::cmp::Ordering::Greater => true,
            std::cmp::Ordering::Less => false,
            std::cmp::Ordering::Equal => remote.origin_device_id > local_origin_device_id,
        },
    }
}

/// Convert a `WireItem` received from a peer into a `ClipboardItem` ready to
/// be persisted locally, marking it as synced.
pub fn wire_to_local(wire: WireItem) -> ClipboardItem {
    ClipboardItem {
        id: wire.id.into(),
        item_id: wire.item_id.into(),
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
        // Propagate the sender's deleted flag so tombstones apply correctly
        // on the receiver. A tombstone win (higher lamport) must land as
        // deleted=true in the local DB, wiping the content from this device.
        deleted: wire.deleted,
        // Propagate pinned/pin_order from the wire so pin and reorder
        // operations converge across devices via LWW.
        pinned: wire.pinned,
        pin_order: wire.pin_order,
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
        id: item.id.to_string(),
        item_id: item.item_id.to_string(),
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
        // Propagate deleted/pinned/pin_order so tombstones, pin changes, and
        // reorder operations travel to peers and win/lose via normal LWW.
        deleted: item.deleted,
        pinned: item.pinned,
        pin_order: item.pin_order,
        file_name,
        mime,
    }
}

/// By-value variant of [`local_to_wire`]: consumes the `ClipboardItem` and
/// **moves** its heap-allocated fields (notably the `content` /
/// `content_nonce` ciphertext blobs, which can be megabytes for file/image
/// items) into the `WireItem` instead of cloning them.
///
/// CopyPaste-ux2i: callers that already own the item and do not need it after
/// building the wire item (e.g. the daemon sync orchestrator draining a
/// broadcast receiver) should use this to avoid an avoidable full-blob copy.
/// The borrowing [`local_to_wire`] is kept for callers that only hold a `&`
/// (e.g. the engine's `filter().map()` over a shared `&[ClipboardItem]` slice).
pub fn local_to_wire_owned(item: ClipboardItem, local_device_id: &str) -> WireItem {
    let origin = if item.origin_device_id.is_empty() {
        local_device_id.to_string()
    } else {
        item.origin_device_id
    };

    let (file_name, mime) = if item.content_type == "file" {
        parse_file_meta_name_mime(item.blob_ref.as_deref())
    } else {
        (None, None)
    };

    WireItem {
        id: item.id.into_string(),
        item_id: item.item_id.into_string(),
        content_type: item.content_type,
        content: item.content,
        content_nonce: item.content_nonce,
        blob_ref: item.blob_ref,
        is_sensitive: item.is_sensitive,
        lamport_ts: item.lamport_ts,
        wall_time: item.wall_time,
        expires_at: item.expires_at,
        app_bundle_id: item.app_bundle_id,
        origin_device_id: origin,
        key_version: item.key_version,
        deleted: item.deleted,
        pinned: item.pinned,
        pin_order: item.pin_order,
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
            id: "item-001".to_string().into(),
            item_id: "iid-001".to_string().into(),
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
            deleted: false,
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
            deleted: false,
            pinned: false,
            pin_order: None,
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

    /// CopyPaste-ux2i: the by-value `local_to_wire_owned` produces a `WireItem`
    /// byte-for-byte identical to the borrowing `local_to_wire`, including the
    /// moved `content` blob and the empty-origin → local-device stamping.
    #[test]
    fn local_to_wire_owned_matches_borrowing_variant() {
        // Case 1: existing origin preserved.
        let mut item = make_local(7, 900);
        item.origin_device_id = "peer-Z".to_string();
        let borrowed = local_to_wire(&item, "my-device");
        let owned = local_to_wire_owned(item, "my-device");
        assert_eq!(owned.id, borrowed.id);
        assert_eq!(owned.item_id, borrowed.item_id);
        assert_eq!(owned.content, borrowed.content);
        assert_eq!(owned.content_nonce, borrowed.content_nonce);
        assert_eq!(owned.origin_device_id, borrowed.origin_device_id);
        assert_eq!(owned.origin_device_id, "peer-Z");
        assert_eq!(owned.key_version, borrowed.key_version);

        // Case 2: empty origin → stamped with the local device id.
        let mut item2 = make_local(3, 500);
        item2.origin_device_id = String::new();
        let borrowed2 = local_to_wire(&item2, "my-device");
        let owned2 = local_to_wire_owned(item2, "my-device");
        assert_eq!(owned2.origin_device_id, "my-device");
        assert_eq!(owned2.content, borrowed2.content);
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
            ItemId, AAD_SCHEMA_VERSION_V4, NONCE_SIZE,
        };

        // The device's v1 storage seed (stands in for `load_local_key()`).
        let seed = [0x42u8; 32];
        let item_id = ItemId::from("iid-roundtrip");
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
            copypaste_core::V1Key(&v1_key),
            copypaste_core::V2Key(&v2_key),
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

    // --- Tombstone LWW semantics ---

    /// A remote tombstone (deleted=true) with a higher lamport_ts must beat
    /// a live local item. The normal `resolve` path does not need to know
    /// about deleted — it just compares lamport/wall/device and the winner
    /// carries whatever deleted flag it has.
    #[test]
    fn tombstone_with_higher_lamport_beats_live_local() {
        let local = make_local(5, 1000); // live item
        let mut remote = make_remote(10, 500, "peer-A");
        remote.deleted = true;
        remote.content = None;
        remote.content_nonce = None;
        // Higher lamport → TakeRemote regardless of deleted flag.
        assert_eq!(resolve(&local, &remote), MergeOutcome::TakeRemote);
    }

    /// A live remote item with a lower lamport must NOT beat a local tombstone.
    #[test]
    fn live_remote_with_lower_lamport_keeps_local_tombstone() {
        let mut local = make_local(10, 1000);
        local.deleted = true;
        local.content = None;
        let remote = make_remote(5, 2000, "peer-A"); // higher wall but lower lamport
        assert_eq!(resolve(&local, &remote), MergeOutcome::KeepLocal);
    }

    // --- deleted / pinned / pin_order round-trip ---

    /// local_to_wire copies deleted=true; wire_to_local restores it.
    #[test]
    fn tombstone_survives_local_to_wire_and_back() {
        let mut item = make_local(7, 2000);
        item.deleted = true;
        item.content = None;
        item.content_nonce = None;

        let wire = local_to_wire(&item, "my-device");
        assert!(wire.deleted, "local_to_wire must copy deleted=true");
        assert!(wire.content.is_none());

        let restored = wire_to_local(wire);
        assert!(restored.deleted, "wire_to_local must restore deleted=true");
        assert!(restored.content.is_none());
    }

    /// local_to_wire copies pinned=true and pin_order; wire_to_local restores them.
    #[test]
    fn pinned_and_pin_order_survive_round_trip() {
        let mut item = make_local(3, 500);
        item.pinned = true;
        item.pin_order = Some(2.5);

        let wire = local_to_wire(&item, "my-device");
        assert!(wire.pinned, "local_to_wire must copy pinned=true");
        assert_eq!(
            wire.pin_order,
            Some(2.5),
            "local_to_wire must copy pin_order"
        );

        let restored = wire_to_local(wire);
        assert!(restored.pinned, "wire_to_local must restore pinned=true");
        assert_eq!(
            restored.pin_order,
            Some(2.5),
            "wire_to_local must restore pin_order"
        );
    }

    // --- Tie-break parity: resolve() vs remote_wins() (CopyPaste-ayvs) ---

    /// `remote_wins` must agree with `resolve` on the same inputs across the
    /// whole decision space (lower / equal / higher lamport, wall, device id),
    /// proving cloud/relay (which call `remote_wins`) converge identically to
    /// P2P (which calls `resolve`).
    #[test]
    fn remote_wins_matches_resolve_across_decision_space() {
        let lamports = [3i64, 5, 7];
        let walls = [100i64, 200, 300];
        let devices = ["aaa", "device-local", "zzz"];
        for &ll in &lamports {
            for &lw in &walls {
                for ld in &devices {
                    let mut local = make_local(ll, lw);
                    local.origin_device_id = (*ld).to_string();
                    for &rl in &lamports {
                        for &rw in &walls {
                            for rd in &devices {
                                let remote = make_remote(rl, rw, rd);
                                let via_resolve =
                                    resolve(&local, &remote) == MergeOutcome::TakeRemote;
                                let via_remote_wins = remote_wins(
                                    local.lamport_ts,
                                    local.wall_time,
                                    &local.origin_device_id,
                                    &RemoteMeta {
                                        lamport_ts: remote.lamport_ts,
                                        wall_time: remote.wall_time,
                                        origin_device_id: &remote.origin_device_id,
                                    },
                                );
                                assert_eq!(
                                    via_resolve, via_remote_wins,
                                    "resolve and remote_wins disagree for \
                                     local=({ll},{lw},{ld}) remote=({rl},{rw},{rd})"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    /// Equal lamport, equal wall, larger remote device id → remote wins (this is
    /// the exact case the old cloud/relay `remote <= local -> keep` got wrong:
    /// it kept local regardless of content).
    #[test]
    fn remote_wins_breaks_equal_lamport_tie_by_device_id() {
        // remote device "zzz" > local "device-local" → remote wins.
        assert!(remote_wins(
            5,
            1000,
            "device-local",
            &RemoteMeta {
                lamport_ts: 5,
                wall_time: 1000,
                origin_device_id: "zzz"
            }
        ));
        // remote device "aaa" < local "device-local" → local wins.
        assert!(!remote_wins(
            5,
            1000,
            "device-local",
            &RemoteMeta {
                lamport_ts: 5,
                wall_time: 1000,
                origin_device_id: "aaa"
            }
        ));
    }

    // --- Lamport unification regression (CopyPaste-ojhe) ---

    /// A newer pin/delete stamped under the unified value space
    /// (`next_lamport_ts`) beats an older recopy stamped at `now_ms`.
    ///
    /// Repro from the audit: device copies X via promote-on-copy (lamport ≈
    /// now_ms, pinned=false); a peer then pins/deletes X. Under the unified
    /// stamping the pin/delete is `max(now_ms_recopy + 1, now_ms_pin)` which is
    /// strictly greater, so `resolve` returns TakeRemote and the pin/delete wins
    /// instead of being discarded.
    #[test]
    fn unified_pin_delete_beats_older_recopy() {
        use copypaste_core::next_lamport_ts;

        // Older recopy: stamped at an earlier now_ms.
        let recopy_now = 1_750_000_000_000i64;
        let recopy_lamport = next_lamport_ts(0, recopy_now); // == recopy_now
        let local = make_local(recopy_lamport, recopy_now);

        // Newer pin/delete on the SAME item: a few ms later, derived from the
        // recopy's lamport via the unified helper.
        let pin_now = recopy_now + 5;
        let pin_lamport = next_lamport_ts(recopy_lamport, pin_now);
        assert!(
            pin_lamport > recopy_lamport,
            "unified pin lamport must strictly exceed the recopy"
        );
        let mut remote = make_remote(pin_lamport, pin_now, "peer-A");
        remote.deleted = true; // model a delete; pin is the same lamport story

        assert_eq!(
            resolve(&local, &remote),
            MergeOutcome::TakeRemote,
            "a newer unified pin/delete must beat an older recopy"
        );
    }

    /// Unpinned item (pin_order=None) round-trips without acquiring a pin_order.
    #[test]
    fn unpinned_item_pin_order_stays_none_on_round_trip() {
        let item = make_local(1, 100); // pinned=false, pin_order=None
        let wire = local_to_wire(&item, "my-device");
        assert!(!wire.pinned);
        assert!(wire.pin_order.is_none());

        let restored = wire_to_local(wire);
        assert!(!restored.pinned);
        assert!(restored.pin_order.is_none());
    }
}
