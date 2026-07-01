//! `merge_incoming[_with_crypto]` tests, relocated out of the former flat
//! `sync_orch/mod.rs` test module (ADR-017, CopyPaste-vp63.3).

use super::*;
use crate::sync_orch::test_support::{make_db, make_wire};
use copypaste_core::insert_item;

/// Op-propagation (v0.6.1): the wire now carries authoritative `pinned` /
/// `pin_order`, so on LWW TakeRemote the winning wire's pin state is applied
/// directly — this is what lets an explicit pin / UNPIN / reorder on one
/// device converge onto the others. (The previous semantic OR-merged the
/// local pin and was kept only because the wire had no pin field; that
/// would silently swallow a remote unpin, which the "sync all operations"
/// requirement forbids.)
///
/// Here a locally-pinned row receives a newer remote that is unpinned →
/// the row must become unpinned (remote unpin propagated). The lookup uses
/// `wire.id`, so the local row's `id` must equal `wire.id` for TakeRemote.
#[tokio::test]
async fn merge_incoming_takeremote_propagates_wire_pin_state() {
    let db = make_db();
    // Local row pinned=true; lamport 3 < wire lamport 9 → TakeRemote fires.
    let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 3);
    local.id = "shared-id".to_string().into();
    local.item_id = "shared-id-iid".to_string().into();
    local.pinned = true;
    {
        let g = db.lock().await;
        insert_item(&g, &local).unwrap();
        let stored = copypaste_core::get_item_by_id(&*g, "shared-id")
            .unwrap()
            .unwrap();
        assert!(stored.pinned, "setup: local row must be pinned");
    }

    // Newer remote, unpinned (make_wire sets pinned=false, pin_order=None).
    let wire = make_wire("shared-id", 9, 0xFF);
    assert_eq!(wire.id, "shared-id");
    assert!(!wire.pinned, "setup: incoming wire is unpinned");

    let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
    assert_eq!(upserted, 1, "newer remote must win LWW");

    let g = db.lock().await;
    let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
    assert_eq!(rows.len(), 1, "must remain ONE row");
    assert!(
        !rows[0].pinned,
        "remote unpin must propagate on TakeRemote — got pinned={}",
        rows[0].pinned
    );
}

/// LWW: a stale wire item (lower lamport) must NOT overwrite the local row.
/// Identity is matched on the cross-device `item_id`, so the local row and
/// the wire item share `item_id = "shared-iid"` (the local `id` is distinct,
/// as it always is across devices).
#[tokio::test]
async fn merge_incoming_keeps_local_on_older_remote() {
    let db = make_db();
    // Pre-insert a local row with a higher lamport clock. Its `item_id`
    // matches the incoming wire's so they are recognised as the SAME item.
    let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 50);
    local.id = "shared".to_string().into();
    local.item_id = "shared-iid".to_string().into();
    {
        let g = db.lock().await;
        insert_item(&g, &local).unwrap();
    }

    let wire = make_wire("shared", 5, 0xFF); // older; item_id = "shared-iid"
    let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
    assert_eq!(upserted, 0, "older remote must lose LWW");

    let g = db.lock().await;
    let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].content, Some(vec![0x11]), "local payload preserved");
}

/// CopyPaste-bfiu: a P2P tombstone for an UNKNOWN item_id (delete arrives
/// before the create, out-of-order over P2P) must persist a tombstone row so
/// a later lower-lamport create is LWW-rejected and the item stays deleted —
/// instead of the old behaviour that silently skipped the tombstone and let
/// the create resurrect the item.
#[tokio::test]
async fn merge_incoming_delete_before_create_does_not_resurrect() {
    let db = make_db();

    // 1) DELETE arrives first for an item we have never seen (lamport 20).
    let mut tomb = make_wire("ghost", 20, 0x00);
    tomb.item_id = "ghost-iid".to_string();
    tomb.deleted = true;
    tomb.content = None;
    tomb.content_nonce = None;
    let upserted = merge_incoming(&db, vec![tomb]).await.unwrap();
    assert_eq!(upserted, 1, "tombstone for unknown item must be persisted");

    {
        let g = db.lock().await;
        let row = copypaste_core::get_item_by_item_id(&g, "ghost-iid")
            .unwrap()
            .expect("tombstone row must exist");
        assert!(
            row.deleted,
            "unknown-item tombstone must persist as deleted"
        );
        // The user-facing list must not show it.
        assert!(
            copypaste_core::get_page(&*g, 10, 0).unwrap().is_empty(),
            "tombstone must not appear in the history list"
        );
    }

    // 2) CREATE arrives later with a LOWER lamport (10 < 20) → loses LWW.
    let mut create = make_wire("ghost", 10, 0xAB);
    create.item_id = "ghost-iid".to_string();
    let upserted2 = merge_incoming(&db, vec![create]).await.unwrap();
    assert_eq!(upserted2, 0, "late lower-lamport create must NOT resurrect");

    let g = db.lock().await;
    let row = copypaste_core::get_item_by_item_id(&g, "ghost-iid")
        .unwrap()
        .expect("row still present");
    assert!(
        row.deleted,
        "item must stay deleted after the racing create"
    );
    assert!(
        copypaste_core::get_page(&*g, 10, 0).unwrap().is_empty(),
        "item must remain hidden from history"
    );
}

/// CRDT identity + local-PK preservation: a TakeRemote (newer lamport) for
/// an item already present locally under a DIFFERENT row `id` must replace
/// the content in place while preserving the local primary key — so FTS,
/// `copy_item`, and pins (all keyed on `id`) keep pointing at the same row.
#[tokio::test]
async fn merge_incoming_replaces_by_item_id_preserving_local_pk() {
    let db = make_db();
    // Local row: PK "local-pk", item_id "X", lamport 5.
    let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 5);
    local.id = "local-pk".to_string().into();
    local.item_id = "X".to_string().into();
    {
        let g = db.lock().await;
        insert_item(&g, &local).unwrap();
    }

    // Incoming wire: peer's own PK "peer-pk", SAME item_id "X", newer
    // lamport 9, different content.
    let mut wire = make_wire("peer-pk", 9, 0xFF);
    wire.item_id = "X".to_string();

    let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
    assert_eq!(upserted, 1, "newer remote must win LWW");

    let g = db.lock().await;
    let rows = copypaste_core::get_page(&*g, 10, 0).unwrap();
    assert_eq!(
        rows.len(),
        1,
        "must remain ONE row (replace, not duplicate)"
    );
    assert_eq!(
        rows[0].id, "local-pk",
        "local primary key must be preserved"
    );
    assert_eq!(rows[0].item_id, "X");
    assert_eq!(rows[0].lamport_ts, 9, "remote (newer) lamport stored");
    assert_eq!(rows[0].content, Some(vec![0xFF]), "remote content stored");
    // The peer's row id must NOT have been adopted.
    assert!(
        copypaste_core::get_item_by_id(&*g, "peer-pk")
            .unwrap()
            .is_none(),
        "peer's row id must not leak into local storage"
    );
}

/// CopyPaste-kcf: inbound items must have `is_sensitive` set from the
/// decrypted plaintext, so cross-device sensitive items get the auto-wipe TTL.
///
/// Before the fix: `wire_to_local` always set `is_sensitive = false`, so a
/// password or API key copied on device A and synced to B was stored with
/// `is_sensitive = false` on B — bypassing the auto-wipe TTL entirely.
///
/// After the fix: `merge_incoming_with_crypto` runs `is_sensitive_for_autowipe`
/// on the recovered plaintext and stores the correct value.
#[tokio::test]
async fn rekey_inbound_sets_is_sensitive_from_plaintext() {
    use base64::Engine as _;
    use copypaste_core::encrypt_for_cloud;
    use copypaste_sync::protocol::WireItem;
    use tempfile::tempdir;

    // A single pairwise key between two devices.
    let k_shared: [u8; 32] = [0x55u8; 32];
    let k_shared_b64 = base64::engine::general_purpose::STANDARD.encode(k_shared);
    let fp_a = "aa:aa";
    let seed_b = [0x22u8; 32];

    let dir = tempdir().unwrap();
    let peers_path = dir.path().join("peers.json");
    std::fs::write(
        &peers_path,
        format!(
            r#"[{{"fingerprint":"{fp_a}","added_at":1,"address":"127.0.0.1:9","sync_key_b64":"{k_shared_b64}"}}]"#
        ),
    )
    .unwrap();
    let crypto = SyncCrypto::new(seed_b, peers_path);

    let db = make_db();
    let key_sync = copypaste_core::SyncKey::from_bytes(k_shared);

    // ── Test 1: sensitive plaintext (AWS key) → is_sensitive = true ──────
    let item_id_sensitive = "kcf-sensitive".to_string();
    // A real AWS access key triggers the detector at confidence 0.99.
    let sensitive_plaintext = "AKIAIOSFODNN7EXAMPLE";
    let blob_sensitive = encrypt_for_cloud(
        &key_sync,
        &item_id_sensitive,
        sensitive_plaintext.as_bytes(),
    )
    .expect("encrypt sensitive");

    let wire_sensitive = WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: "kcf-row-sens".to_string(),
        item_id: item_id_sensitive.clone(),
        content_type: "text".to_string(),
        content: Some(blob_sensitive),
        content_nonce: None, // sync-key-wrapped
        blob_ref: None,
        is_sensitive: false, // sender's flag — must be overridden by receiver
        lamport_ts: 1,
        wall_time: 1_700_000_000_001,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "device-A".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    };

    let quota = copypaste_core::AppConfig::default().storage_quota_bytes as i64;
    merge_incoming_with_crypto(&db, vec![wire_sensitive], Some(&crypto), quota, None)
        .await
        .expect("merge sensitive item");

    {
        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).expect("get_page");
        assert_eq!(rows.len(), 1, "sensitive item must be stored");
        assert!(
            rows[0].is_sensitive,
            "inbound sensitive item must have is_sensitive=true (CopyPaste-kcf fix); got false"
        );
    }

    // ── Test 2: non-sensitive plaintext → is_sensitive = false ───────────
    let item_id_plain = "kcf-plain".to_string();
    let plain_text = "hello world, nothing secret here";
    let blob_plain =
        encrypt_for_cloud(&key_sync, &item_id_plain, plain_text.as_bytes()).expect("encrypt plain");

    let wire_plain = WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: "kcf-row-plain".to_string(),
        item_id: item_id_plain.clone(),
        content_type: "text".to_string(),
        content: Some(blob_plain),
        content_nonce: None,
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: 2,
        wall_time: 1_700_000_000_002,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "device-A".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    };

    merge_incoming_with_crypto(&db, vec![wire_plain], Some(&crypto), quota, None)
        .await
        .expect("merge plain item");

    {
        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).expect("get_page");
        assert_eq!(rows.len(), 2, "both items must be stored");
        // The plain item should have is_sensitive=false.
        let plain_row = rows
            .iter()
            .find(|r| r.item_id == item_id_plain)
            .expect("plain item must be in DB");
        assert!(
            !plain_row.is_sensitive,
            "non-sensitive inbound item must have is_sensitive=false"
        );
    }
}
