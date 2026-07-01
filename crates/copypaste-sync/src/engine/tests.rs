// size-exempt: test file per ADR-017
use super::*;
use crate::clock::LamportClock;
use crate::protocol::Message;
use copypaste_core::storage::items::ClipboardItem;
use tokio::io::AsyncWriteExt;

// ---------------------------------------------------------------------------
// Helpers — in-memory duplex stream simulation
// ---------------------------------------------------------------------------

/// Create a pair of in-memory duplex streams backed by tokio channels,
/// simulating a bidirectional TCP connection without network I/O.
fn make_duplex() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
    // 1 MiB buffer is plenty for test payloads.
    tokio::io::duplex(1024 * 1024)
}

fn make_item(id: &str, lamport: i64) -> ClipboardItem {
    ClipboardItem {
        deleted: false,
        id: id.to_string().into(),
        item_id: format!("{id}-item").into(),
        content_type: "text".to_string(),
        content: Some(vec![0xAA, 0xBB]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts: lamport,
        wall_time: 1_700_000_000_000 + lamport,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        origin_device_id: format!("dev-{id}"),
        key_version: 1,
        pinned: false,
        pin_order: None,
        thumb: None,
    }
}

// ---------------------------------------------------------------------------
// Unit tests for framing helpers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn send_recv_message_round_trips() {
    let (mut a, mut b) = make_duplex();
    let msg = Message::Hello {
        device_id: "test-dev".to_string(),
        clock: 5,
        item_count: 3,
    };
    send_message(&mut a, &msg).await.unwrap();
    let received = recv_message(&mut b).await.unwrap();
    assert_eq!(received, msg);
}

#[tokio::test]
async fn frame_too_large_is_rejected() {
    let (mut a, mut b) = make_duplex();
    // Write a fake frame with length > MAX_FRAME_SIZE.
    let huge_len: u32 = MAX_FRAME_SIZE + 1;
    a.write_all(&huge_len.to_le_bytes()).await.unwrap();
    drop(a);
    let err = recv_message(&mut b).await.unwrap_err();
    assert!(matches!(err, SyncError::FrameTooLarge(_)));
}

// ---------------------------------------------------------------------------
// Integration test: two engines exchange items
// ---------------------------------------------------------------------------

#[tokio::test]
async fn two_engines_exchange_disjoint_items() {
    let item_a = make_item("item-A", 1);
    let item_b = make_item("item-B", 2);

    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");

    let (mut stream_a, mut stream_b) = make_duplex();

    // Run both sessions concurrently.
    let items_a = [item_a.clone()];
    let items_b = [item_b.clone()];
    let (res_a, res_b) = tokio::join!(
        engine_a.run_session(&mut stream_a, &items_a),
        engine_b.run_session(&mut stream_b, &items_b),
    );

    let (result_a, upsert_a) = res_a.expect("engine A must succeed");
    let (result_b, upsert_b) = res_b.expect("engine B must succeed");

    // Each engine should have received the other's item.
    assert_eq!(result_a.items_received, 1, "A should receive item-B");
    assert_eq!(result_b.items_received, 1, "B should receive item-A");
    assert_eq!(result_a.items_sent, 1, "A should send item-A");
    assert_eq!(result_b.items_sent, 1, "B should send item-B");

    assert_eq!(upsert_a.len(), 1);
    assert_eq!(upsert_a[0].id, "item-B");
    assert!(upsert_a[0].is_synced);

    assert_eq!(upsert_b.len(), 1);
    assert_eq!(upsert_b[0].id, "item-A");
    assert!(upsert_b[0].is_synced);
}

#[tokio::test]
async fn already_synced_items_not_re_requested() {
    // Both engines have the same item — nothing should be exchanged.
    let shared = make_item("item-shared", 5);

    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");

    let (mut stream_a, mut stream_b) = make_duplex();

    let items_a = [shared.clone()];
    let items_b = [shared.clone()];
    let (res_a, res_b) = tokio::join!(
        engine_a.run_session(&mut stream_a, &items_a),
        engine_b.run_session(&mut stream_b, &items_b),
    );

    let (result_a, upsert_a) = res_a.unwrap();
    let (result_b, upsert_b) = res_b.unwrap();

    assert_eq!(result_a.items_received, 0);
    assert_eq!(result_b.items_received, 0);
    assert_eq!(result_a.items_sent, 0);
    assert_eq!(result_b.items_sent, 0);
    assert!(upsert_a.is_empty());
    assert!(upsert_b.is_empty());
}

#[tokio::test]
async fn lww_conflict_higher_lamport_wins() {
    // Same item ID, different lamport clocks — engine B's version wins.
    let item_a = make_item("item-conflict", 3); // local lamport = 3
    let mut item_b = make_item("item-conflict", 7); // remote lamport = 7 → wins
    item_b.content = Some(vec![0xFF]); // different content so we can verify

    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");

    let (mut stream_a, mut stream_b) = make_duplex();

    let items_a = [item_a];
    let items_b = [item_b.clone()];
    let (res_a, res_b) = tokio::join!(
        engine_a.run_session(&mut stream_a, &items_a),
        engine_b.run_session(&mut stream_b, &items_b),
    );

    let (_result_a, upsert_a) = res_a.unwrap();
    let (_result_b, _upsert_b) = res_b.unwrap();

    // Engine A should have accepted item-conflict from B (lamport 7 > 3).
    assert_eq!(upsert_a.len(), 1);
    assert_eq!(upsert_a[0].lamport_ts, 7);
    assert_eq!(upsert_a[0].content, Some(vec![0xFF]));
}

#[tokio::test]
async fn lamport_clock_advances_after_session() {
    let item_a = make_item("item-A", 10);

    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");

    // Set engine B's clock to something high.
    engine_b.clock = LamportClock::from_value(50);

    let (mut stream_a, mut stream_b) = make_duplex();

    let items_a = [item_a];
    let items_b: [ClipboardItem; 0] = [];
    let _ = tokio::join!(
        engine_a.run_session(&mut stream_a, &items_a),
        engine_b.run_session(&mut stream_b, &items_b),
    );

    // Engine A's clock should have advanced past B's initial value (50).
    // After observe(50): max(0, 50) + 1 = 51 (minimum).
    assert!(
        engine_a.clock.get() >= 51,
        "clock should advance past peer's value"
    );
}

#[tokio::test]
async fn peer_clock_recorded_after_session() {
    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");

    let (mut stream_a, mut stream_b) = make_duplex();

    let items: [ClipboardItem; 0] = [];
    let _ = tokio::join!(
        engine_a.run_session(&mut stream_a, &items),
        engine_b.run_session(&mut stream_b, &items),
    );

    // engine_a should know about device-B.
    assert!(engine_a.peer_clocks.contains_key("device-B"));
    assert!(engine_b.peer_clocks.contains_key("device-A"));
}

#[tokio::test]
async fn on_local_write_ticks_clock() {
    let mut engine = SyncEngine::new("device-A");
    let t1 = engine.on_local_write();
    let t2 = engine.on_local_write();
    assert_eq!(t1, 1);
    assert_eq!(t2, 2);
}

#[tokio::test]
async fn identical_everything_merge_is_idempotent() {
    // edge-cases MEDIUM #16: merging the exact same item twice must not
    // mutate local state on the second pass. Verifies LWW determinism
    // for fully-identical (lamport, wall, device, payload) items.
    let shared = make_item("item-shared", 5);

    // Round 1: A and B exchange and both end up holding `shared`.
    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");
    let (mut sa, mut sb) = make_duplex();
    let items_a = [shared.clone()];
    let items_b = [shared.clone()];
    let (r1_a, r1_b) = tokio::join!(
        engine_a.run_session(&mut sa, &items_a),
        engine_b.run_session(&mut sb, &items_b),
    );
    let (_, upsert_a_1) = r1_a.unwrap();
    let (_, upsert_b_1) = r1_b.unwrap();
    // First pass: both have it already, nothing to upsert.
    assert!(upsert_a_1.is_empty(), "round 1: A should not upsert");
    assert!(upsert_b_1.is_empty(), "round 1: B should not upsert");

    // Round 2: identical inputs again → still no-op (idempotent).
    let (mut sa2, mut sb2) = make_duplex();
    let (r2_a, r2_b) = tokio::join!(
        engine_a.run_session(&mut sa2, &items_a),
        engine_b.run_session(&mut sb2, &items_b),
    );
    let (res_a_2, upsert_a_2) = r2_a.unwrap();
    let (res_b_2, upsert_b_2) = r2_b.unwrap();

    assert!(upsert_a_2.is_empty(), "round 2 must be idempotent on A");
    assert!(upsert_b_2.is_empty(), "round 2 must be idempotent on B");
    assert_eq!(res_a_2.items_received, 0);
    assert_eq!(res_b_2.items_received, 0);
    assert_eq!(res_a_2.items_sent, 0);
    assert_eq!(res_b_2.items_sent, 0);
}

#[tokio::test]
async fn negative_lamport_does_not_panic() {
    // edge-cases LOW #34: a malicious/buggy peer sends a wire item with
    // negative lamport_ts; engine must clamp and not panic from the cast.
    let item = ClipboardItem {
        deleted: false,
        id: "neg".to_string().into(),
        item_id: "neg-item".to_string().into(),
        content_type: "text".to_string(),
        content: Some(vec![0x01]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        is_synced: false,
        lamport_ts: -1, // negative
        wall_time: 1_700_000_000_000,
        expires_at: None,
        app_bundle_id: None,
        content_hash: None,
        origin_device_id: "device-A".to_string(),
        key_version: 1,
        pinned: false,
        pin_order: None,
        thumb: None,
    };

    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");
    let (mut sa, mut sb) = make_duplex();
    let items_a: [ClipboardItem; 0] = [];
    let items_b = [item];
    let (res_a, res_b) = tokio::join!(
        engine_a.run_session(&mut sa, &items_a),
        engine_b.run_session(&mut sb, &items_b),
    );
    // Must not panic — both sides return Ok.
    let (_result_a, upsert_a) = res_a.expect("engine A must succeed");
    assert!(res_b.is_ok());

    // L1: A had no items, so it accepts "neg" from B. The stored item's
    // lamport_ts MUST be clamped to 0 at ingestion — a negative value must
    // never be persisted to the row.
    assert_eq!(upsert_a.len(), 1, "A should accept the new item from B");
    assert_eq!(upsert_a[0].id, "neg");
    assert_eq!(
        upsert_a[0].lamport_ts, 0,
        "negative inbound lamport_ts must be clamped to 0 before storage (L1)"
    );
}

#[tokio::test]
async fn empty_session_completes_without_error() {
    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");

    let (mut stream_a, mut stream_b) = make_duplex();

    let items: [ClipboardItem; 0] = [];
    let (res_a, res_b) = tokio::join!(
        engine_a.run_session(&mut stream_a, &items),
        engine_b.run_session(&mut stream_b, &items),
    );

    assert!(res_a.is_ok());
    assert!(res_b.is_ok());
}

/// CRDT stable-identity regression: the SAME logical item captured on two
/// devices has DIFFERENT per-row `id`s (each device runs `Uuid::new_v4()`)
/// but the SAME cross-device `item_id`. HAVE/WANT/LWW must key on `item_id`
/// so the two converge to ONE item — the higher-lamport version winning —
/// instead of each device treating the other's copy as a brand-new item and
/// accumulating a duplicate.
#[tokio::test]
async fn same_item_id_different_row_id_converges_to_one_row() {
    // Device A: row id A1, item_id X, lamport 5.
    let mut a = make_item("A1", 5);
    a.item_id = "X".to_string().into();
    a.content = Some(vec![0xAA]);
    // Device B: row id B9 (different!), item_id X (same logical item),
    // lamport 7, different content → B's version must win LWW.
    let mut b = make_item("B9", 7);
    b.item_id = "X".to_string().into();
    b.content = Some(vec![0xBB]);

    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");
    let (mut sa, mut sb) = make_duplex();
    let items_a = [a];
    let items_b = [b];
    let (res_a, res_b) = tokio::join!(
        engine_a.run_session(&mut sa, &items_a),
        engine_b.run_session(&mut sb, &items_b),
    );
    let (result_a, upsert_a) = res_a.expect("engine A ok");
    let (result_b, upsert_b) = res_b.expect("engine B ok");

    // A must accept B's newer version of item_id X (LWW: lamport 7 > 5).
    assert_eq!(upsert_a.len(), 1, "A converges to the single shared item");
    assert_eq!(upsert_a[0].item_id, "X");
    assert_eq!(upsert_a[0].lamport_ts, 7, "higher-lamport version wins");
    assert_eq!(upsert_a[0].content, Some(vec![0xBB]), "B's content wins");
    assert_eq!(result_a.items_received, 1);

    // B already holds the winning (higher-lamport) version of item_id X. The
    // HAVE/WANT exchange — now keyed on item_id — recognises A's copy as the
    // SAME item, so B never even requests A's older version: nothing is
    // transferred to B and nothing is upserted. This is the convergence
    // guarantee: B keeps its winner, A adopts it, ONE row results. (Pre-fix,
    // A's differently-`id`'d copy looked like a brand-new item to B and would
    // have been ingested as a duplicate.)
    assert!(
        upsert_b.is_empty(),
        "B must not ingest A's older same-item_id copy as a new row"
    );
    assert_eq!(
        result_b.items_received, 0,
        "B receives nothing — A's copy is the same logical item, not new"
    );
    assert_eq!(
        result_b.items_sent, 1,
        "B sends its winning version of item_id X to A"
    );
}

/// The inverse guarantee: two items with the SAME content but DISTINCT
/// `item_id`s (X1 / X2) are INTENTIONALLY-different captures and must BOTH
/// survive a sync. Identity is `item_id`, never the content hash — keying on
/// content would wrongly collapse deliberate duplicate captures.
#[tokio::test]
async fn intentional_duplicate_captures_stay_distinct() {
    // A holds item_id X1; B holds item_id X2 — identical content, distinct
    // logical items.
    let mut a = make_item("rowA", 3);
    a.item_id = "X1".to_string().into();
    a.content = Some(vec![0xEE]);
    let mut b = make_item("rowB", 3);
    b.item_id = "X2".to_string().into();
    b.content = Some(vec![0xEE]); // SAME content as A

    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");
    let (mut sa, mut sb) = make_duplex();
    let items_a = [a];
    let items_b = [b];
    let (res_a, res_b) = tokio::join!(
        engine_a.run_session(&mut sa, &items_a),
        engine_b.run_session(&mut sb, &items_b),
    );
    let (_ra, upsert_a) = res_a.expect("engine A ok");
    let (_rb, upsert_b) = res_b.expect("engine B ok");

    // Each side learns the OTHER's distinct item_id — both survive.
    assert_eq!(upsert_a.len(), 1, "A must receive X2");
    assert_eq!(upsert_a[0].item_id, "X2");
    assert_eq!(upsert_b.len(), 1, "B must receive X1");
    assert_eq!(upsert_b[0].item_id, "X1");
}

/// CopyPaste-5on regression: HAVE/WANT exchange MUST be keyed on the stable
/// `item_id`, never on the per-row `id`.  Each device mints its own
/// `Uuid::new_v4()` row id at capture time, so two devices that hold the same
/// logical item will have different row ids.  If the engine keyed HAVE/WANT on
/// `id` instead of `item_id` it would treat each copy as a distinct item and
/// accumulate duplicates on every sync.  This test verifies convergence by
/// using deliberately mismatched row ids and asserting that exactly ONE item
/// survives on each side after the session.
#[tokio::test]
async fn crdt_have_want_keyed_on_item_id_not_row_id() {
    // Device A captured item_id "shared-clip" and assigned row id "row-AAA".
    let mut item_a = make_item("row-AAA", 10);
    item_a.item_id = "shared-clip".to_string().into();
    item_a.content = Some(b"hello from A".to_vec());

    // Device B captured the SAME logical item (same item_id) but assigned a
    // completely different row id "row-BBB" and has a higher lamport clock.
    let mut item_b = make_item("row-BBB", 15);
    item_b.item_id = "shared-clip".to_string().into();
    item_b.content = Some(b"hello from B (newer)".to_vec());

    let mut engine_a = SyncEngine::new("device-A");
    let mut engine_b = SyncEngine::new("device-B");
    let (mut sa, mut sb) = make_duplex();
    let items_a = [item_a];
    let items_b = [item_b];
    let (res_a, res_b) = tokio::join!(
        engine_a.run_session(&mut sa, &items_a),
        engine_b.run_session(&mut sb, &items_b),
    );
    let (result_a, upsert_a) = res_a.expect("engine A ok");
    let (result_b, upsert_b) = res_b.expect("engine B ok");

    // A must receive B's newer version (lamport 15 > 10) — one upsert.
    assert_eq!(
        upsert_a.len(),
        1,
        "A must converge: exactly 1 upsert for shared-clip"
    );
    assert_eq!(upsert_a[0].item_id, "shared-clip");
    assert_eq!(upsert_a[0].lamport_ts, 15, "higher-lamport (B) wins LWW");
    assert_eq!(
        upsert_a[0].content,
        Some(b"hello from B (newer)".to_vec()),
        "B's content wins"
    );
    assert_eq!(result_a.items_received, 1);

    // B already holds the winning version — it must not ingest A's older copy
    // as a new row (which is what happens when HAVE/WANT keys on row `id`).
    assert!(
        upsert_b.is_empty(),
        "B must NOT treat A's row-AAA as a brand-new item (HAVE/WANT must key on item_id)"
    );
    assert_eq!(
        result_b.items_received, 0,
        "B receives nothing — it already has the winner"
    );
}

/// CopyPaste-5on regression: logical-clock unification.
///
/// When a remote item arrives with a large `lamport_ts` (e.g. an Android
/// device that historically stored wall-clock millis ≈ 1.7 × 10^12 in that
/// field), `observe()` must advance the LOCAL clock to `max(local, remote) + 1`
/// — the standard Lamport rule — so subsequent local events are causally
/// later than anything received.  The clock must NOT stay below the received
/// value (which would make future local writes appear "older").
///
/// The engine's upper-bound clamp (MAX_LAMPORT_SKEW) is intentionally set high
/// enough (10^12) that a legitimate wall-clock-millis value just above 1.7 × 10^12
/// is clamped before it jams the clock forever, while a value within the window
/// advances the local clock correctly.
#[tokio::test]
async fn logical_clock_observe_advances_to_max_plus_one() {
    // Simulate a peer whose lamport_ts reflects a small logical counter (42).
    // Our local clock starts at 0.  After the session our clock must be > 42.
    let mut peer_item = make_item("peer-row", 42);
    peer_item.item_id = "peer-item".to_string().into();

    let mut engine_local = SyncEngine::new("device-local");
    let mut engine_peer = SyncEngine::new("device-peer");
    assert_eq!(engine_local.clock.get(), 0, "local clock starts at 0");

    let (mut sl, mut sp) = make_duplex();
    let local_items: [ClipboardItem; 0] = [];
    let peer_items = [peer_item];
    let _ = tokio::join!(
        engine_local.run_session(&mut sl, &local_items),
        engine_peer.run_session(&mut sp, &peer_items),
    );

    // After observing peer's lamport=42, local clock must be >= 43
    // (Lamport rule: max(0, 42) + 1 = 43).
    assert!(
        engine_local.clock.get() >= 43,
        "local clock must advance to at least max(local, peer)+1 = 43, got {}",
        engine_local.clock.get()
    );
}
