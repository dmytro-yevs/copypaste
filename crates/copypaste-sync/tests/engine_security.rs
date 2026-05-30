//! Security and correctness tests for the sync engine fixes.
//!
//! Tests for:
//!  1. [MED] Upper bound clamp on incoming lamport_ts / wall_time — a hostile/buggy
//!     peer sending a sky-high lamport_ts must not jam the local Lamport clock to
//!     saturation and permanently win all LWW decisions.
//!  2. [MED] Duplicate item_id keys in a peer's HAVE list — must take the MAX
//!     lamport_ts rather than silently discarding all but the last entry.
//!  3. [LOW] `items_peer_wants.contains()` lookup — validated via integration
//!     path (no direct O(n) exposure, but the code change is exercised here).
//!  4. [LOW] Centralised negative→0 clamp of lamport_ts — clamping is now done
//!     at decode time (WireItem::clamp_timestamps) so no consumer ever sees
//!     a raw negative value.

use copypaste_core::storage::items::ClipboardItem;
use copypaste_sync::engine::SyncEngine;
use copypaste_sync::protocol::{Message, WireItem};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

fn make_item(id: &str, lamport: i64) -> ClipboardItem {
    ClipboardItem {
        id: id.to_string(),
        item_id: format!("{id}-iid"),
        content_type: "text".to_string(),
        content: Some(vec![0xAA]),
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
        key_version: 2,
        pinned: false,
    }
}

fn make_wire(id: &str, lamport: i64) -> WireItem {
    WireItem {
        id: id.to_string(),
        item_id: format!("{id}-iid"),
        content_type: "text".to_string(),
        content: Some(vec![0xBB]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: lamport,
        // Use saturating_add so the helper doesn't overflow when lamport is i64::MAX.
        wall_time: 1_700_000_000_000_i64.saturating_add(lamport),
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: format!("dev-{id}"),
        key_version: 2,
    }
}

fn make_duplex() -> (tokio::io::DuplexStream, tokio::io::DuplexStream) {
    tokio::io::duplex(4 * 1024 * 1024)
}

async fn peer_send(stream: &mut tokio::io::DuplexStream, msg: &Message) {
    let frame = msg.encode().unwrap();
    stream.write_all(&frame).await.unwrap();
}

async fn peer_recv(stream: &mut tokio::io::DuplexStream) -> Message {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await.unwrap();
    let len = u32::from_le_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await.unwrap();
    Message::decode(&payload).unwrap()
}

/// Drive the "peer" side of the full HELLO/HAVE/WANT/ITEMS/DONE exchange using
/// a pre-crafted adversarial payload. Used to inject scenarios that a real
/// `SyncEngine` would never produce.
async fn run_adversarial_peer(
    stream: &mut tokio::io::DuplexStream,
    peer_device_id: &str,
    peer_clock: u64,
    have_items: Vec<(String, i64)>,
    items_to_deliver: Vec<WireItem>,
) {
    // 1. Receive HELLO from engine under test.
    let _ = peer_recv(stream).await;

    // 2. Send our HELLO.
    peer_send(
        stream,
        &Message::Hello {
            device_id: peer_device_id.to_string(),
            clock: peer_clock,
            item_count: have_items.len() as u64,
        },
    )
    .await;

    // 3. Receive HAVE from engine under test.
    let _ = peer_recv(stream).await;

    // 4. Send our HAVE.
    peer_send(stream, &Message::Have { items: have_items }).await;

    // 5. Receive WANT from engine under test.
    let _ = peer_recv(stream).await;

    // 6. Send our WANT (we want nothing).
    peer_send(stream, &Message::Want { item_ids: vec![] }).await;

    // 7. Receive ITEMS from engine under test (we ignore them).
    let _ = peer_recv(stream).await;

    // 8. Send our ITEMS (the adversarial payload).
    peer_send(
        stream,
        &Message::Items {
            items: items_to_deliver,
        },
    )
    .await;

    // 9. Receive DONE.
    let _ = peer_recv(stream).await;

    // 10. Send DONE.
    peer_send(stream, &Message::Done).await;
}

// ---------------------------------------------------------------------------
// Fix #1 — Upper bound clamp on inbound lamport_ts / wall_time
// ---------------------------------------------------------------------------

/// A peer sending an implausibly large `lamport_ts` (far beyond what any real
/// peer's clock could have reached) must have that value clamped before it is
/// fed into `observe()`. The local Lamport clock must NOT jump to saturation
/// (or anywhere near it).
///
/// Without the fix: `clock.observe(i64::MAX as u64)` drives the local clock
/// toward u64::MAX, permanently making every future local write appear
/// causally "older" than any peer item (i.e. that peer wins ALL future LWW
/// conflicts forever).
#[tokio::test]
async fn hostile_peer_cannot_jam_clock_with_huge_lamport_ts() {
    let local_item = make_item("item-local", 5);

    let mut engine = SyncEngine::new("device-local");
    let (mut engine_stream, mut peer_stream) = make_duplex();

    // Hostile item with the maximum possible i64 lamport_ts.
    let mut hostile = make_wire("item-hostile", i64::MAX);
    hostile.wall_time = i64::MAX;

    let peer_future = run_adversarial_peer(
        &mut peer_stream,
        "hostile-peer",
        999_999_999, // peer's HELLO clock — large but not i64::MAX
        vec![("item-hostile".to_string(), i64::MAX)],
        vec![hostile],
    );

    let local_items = [local_item];
    let engine_future = engine.run_session(&mut engine_stream, &local_items);

    let (_, engine_res) = tokio::join!(peer_future, engine_future);
    let (_result, upserts) = engine_res.expect("engine must complete without error");

    // The local Lamport clock must NOT have jumped to anywhere near u64::MAX.
    // After the fix, the inbound lamport_ts is clamped to a sane ceiling before
    // being passed to observe(), so the clock stays in a normal range.
    let clock_after = engine.clock.get();
    assert!(
        clock_after < u64::MAX / 2,
        "clock must not be near saturation after hostile peer; got clock={}",
        clock_after
    );

    let _ = upserts;
}

/// wall_time upper bound — a peer with wall_time=i64::MAX must have the
/// accepted item's wall_time clamped so all future LWW wall-time comparisons
/// remain meaningful (i.e. local items would not always appear "older").
#[tokio::test]
async fn hostile_peer_wall_time_is_clamped_on_accepted_item() {
    let mut engine = SyncEngine::new("device-local");
    let (mut engine_stream, mut peer_stream) = make_duplex();

    // Item with sane lamport_ts but malicious wall_time.
    let mut wire = make_wire("item-wt", 1);
    wire.wall_time = i64::MAX;

    let peer_future = run_adversarial_peer(
        &mut peer_stream,
        "hostile-peer",
        1,
        vec![("item-wt".to_string(), 1)],
        vec![wire],
    );

    let local_items: [ClipboardItem; 0] = [];
    let engine_future = engine.run_session(&mut engine_stream, &local_items);
    let (_, engine_res) = tokio::join!(peer_future, engine_future);
    let (_result, upserts) = engine_res.expect("engine must complete");

    // Accepted item's wall_time must not be i64::MAX.
    if let Some(item) = upserts.first() {
        assert!(
            item.wall_time < i64::MAX,
            "wall_time on accepted item must be clamped; got wall_time={}",
            item.wall_time
        );
    }
}

// ---------------------------------------------------------------------------
// Fix #2 — Duplicate keys in HAVE list: take MAX lamport_ts
// ---------------------------------------------------------------------------

/// When the peer sends a HAVE list containing the same `item_id` twice, the
/// engine must use the MAXIMUM lamport_ts for that id, not silently discard
/// one entry (which `HashMap::collect` does — "last wins" is undefined order).
///
/// Scenario:
///   - Local has "item-shared" at lamport=5.
///   - Peer's HAVE sends ("item-shared", 3) AND ("item-shared", 8) — two
///     entries for the same id. The effective ts must be max(3, 8) = 8.
///   - Because peer has ts=8 > local ts=5, the engine MUST request the item
///     and accept the higher version.
///
/// Without the fix, `collect` collapses duplicates with last-wins (undefined
/// iteration order), so if ts=3 happens to be last the engine incorrectly
/// skips the request, causing the newer version from the peer to be lost.
#[tokio::test]
async fn have_duplicate_keys_take_max_lamport() {
    let local_item = make_item("item-shared", 5); // local at ts=5
    let mut engine = SyncEngine::new("device-local");
    let (mut engine_stream, mut peer_stream) = make_duplex();

    // Duplicate HAVE entries: ts=3 AND ts=8 for the same id.
    // Correct effective value: max(3, 8) = 8 > 5 → engine must WANT it.
    let have_items = vec![
        ("item-shared".to_string(), 3_i64),
        ("item-shared".to_string(), 8_i64), // duplicate, higher ts
    ];

    // Peer delivers the item at lamport=8 with distinct content.
    let mut peer_item = make_wire("item-shared", 8);
    peer_item.content = Some(vec![0xFF]);

    let peer_future =
        run_adversarial_peer(&mut peer_stream, "peer-dup", 8, have_items, vec![peer_item]);

    let local_items = [local_item];
    let engine_future = engine.run_session(&mut engine_stream, &local_items);
    let (_, engine_res) = tokio::join!(peer_future, engine_future);
    let (result, upserts) = engine_res.expect("engine must complete");

    // Engine must have accepted the peer's item (ts=8 > ts=5).
    assert_eq!(
        result.items_received, 1,
        "engine must accept peer item with max(dup lamport)=8 > local 5"
    );
    assert_eq!(upserts.len(), 1);
    assert_eq!(upserts[0].lamport_ts, 8);
    assert_eq!(upserts[0].content, Some(vec![0xFF]));
}

/// Mirror test: both duplicate HAVE entries are below local ts — engine must
/// NOT request the item (max is still below local).
#[tokio::test]
async fn have_duplicate_keys_both_below_local_does_not_request() {
    let local_item = make_item("item-shared", 10); // local at ts=10
    let mut engine = SyncEngine::new("device-local");
    let (mut engine_stream, mut peer_stream) = make_duplex();

    // Both duplicate entries have ts < 10.
    let have_items = vec![
        ("item-shared".to_string(), 3_i64),
        ("item-shared".to_string(), 7_i64), // max is 7 < 10
    ];

    let peer_future = run_adversarial_peer(
        &mut peer_stream,
        "peer-dup",
        7,
        have_items,
        vec![], // peer delivers nothing — engine should not have asked
    );

    let local_items = [local_item];
    let engine_future = engine.run_session(&mut engine_stream, &local_items);
    let (_, engine_res) = tokio::join!(peer_future, engine_future);
    let (result, upserts) = engine_res.expect("engine must complete");

    assert_eq!(
        result.items_received, 0,
        "local ts=10 > max(dup)=7 — engine must not request item"
    );
    assert!(upserts.is_empty());
}

// ---------------------------------------------------------------------------
// Fix #4 — Centralised negative→0 clamp (decode-time, WireItem::clamp_timestamps)
// ---------------------------------------------------------------------------

/// `WireItem::clamp_timestamps()` must zero out negative `lamport_ts` and
/// `wall_time` values.
#[test]
fn wire_item_clamp_timestamps_zeroes_negative_lamport() {
    let mut wire = make_wire("id-neg", -42);
    wire.wall_time = -999;

    wire.clamp_timestamps();

    assert_eq!(
        wire.lamport_ts, 0,
        "negative lamport_ts must be clamped to 0"
    );
    assert_eq!(wire.wall_time, 0, "negative wall_time must be clamped to 0");
}

/// Non-negative values must be left unchanged by the clamp.
#[test]
fn wire_item_clamp_timestamps_leaves_positive_values_unchanged() {
    let mut wire = make_wire("id-pos", 42);
    wire.wall_time = 1_700_000_000_000;

    wire.clamp_timestamps();

    assert_eq!(wire.lamport_ts, 42);
    assert_eq!(wire.wall_time, 1_700_000_000_000);
}

/// Zero values must remain zero.
#[test]
fn wire_item_clamp_timestamps_zero_is_unchanged() {
    let mut wire = make_wire("id-zero", 0);
    wire.wall_time = 0;

    wire.clamp_timestamps();

    assert_eq!(wire.lamport_ts, 0);
    assert_eq!(wire.wall_time, 0);
}

// ---------------------------------------------------------------------------
// Regression: negative lamport is still clamped end-to-end (engine path)
// ---------------------------------------------------------------------------

/// End-to-end regression: engine receives a WireItem with negative lamport_ts
/// over the wire. The stored item must have lamport_ts=0, not a wrapped huge u64.
#[tokio::test]
async fn engine_clamps_negative_lamport_end_to_end() {
    let mut engine = SyncEngine::new("device-local");
    let (mut engine_stream, mut peer_stream) = make_duplex();

    let hostile = make_wire("item-neg", -1);

    let peer_future = run_adversarial_peer(
        &mut peer_stream,
        "buggy-peer",
        1,
        vec![("item-neg".to_string(), -1_i64)],
        vec![hostile],
    );

    let local_items: [ClipboardItem; 0] = [];
    let engine_future = engine.run_session(&mut engine_stream, &local_items);
    let (_, engine_res) = tokio::join!(peer_future, engine_future);
    let (_result, upserts) = engine_res.expect("engine must complete");

    assert_eq!(upserts.len(), 1, "new item from peer must be accepted");
    assert_eq!(
        upserts[0].lamport_ts, 0,
        "negative lamport_ts must be clamped to 0 in stored item"
    );
}
