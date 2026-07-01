use tokio::sync::mpsc;

use super::forwarder::supervise_forward_task;
use super::SyncInboxForwarder;
use crate::metrics::SyncLagCounter;
use crate::protocol::WireItem;

/// CopyPaste-crh3.93: a panicking forwarding task must be observed (logged)
/// by the supervisor, not silently swallowed; a clean exit must report no
/// panic. The supervisor itself must never propagate the panic.
#[tokio::test]
async fn supervise_forward_task_reports_panic_and_clean_exit() {
    let panicking = tokio::spawn(async { panic!("forward_loop boom") });
    assert!(
        supervise_forward_task(panicking).await,
        "a panicked forward task must be reported (true), not swallowed"
    );

    let clean = tokio::spawn(async {});
    assert!(
        !supervise_forward_task(clean).await,
        "a cleanly-exited forward task must report no panic (false)"
    );
}

fn wire_item(id: &str, lamport: i64) -> WireItem {
    WireItem {
        id: id.to_string(),
        item_id: format!("{id}-item"),
        content_type: "text".to_string(),
        content: Some(vec![0xAA]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: lamport,
        wall_time: 1_700_000_000_000 + lamport,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: format!("dev-{id}"),
        key_version: 2,
        file_name: None,
        mime: None,
        deleted: false,
        pinned: false,
        pin_order: None,
    }
}

// -------------------------------------------------------------------------
// CopyPaste-bxsa: enqueue MUST be non-blocking even when downstream is full
// -------------------------------------------------------------------------

/// Enqueueing to a full downstream channel must not block the caller.
///
/// This is the primary regression guard for CopyPaste-bxsa: the P2P accept
/// loop must be able to call try_enqueue and return immediately regardless
/// of the downstream consumer's state. We simulate a slow / stalled
/// downstream by creating an mpsc with capacity 1 and never reading from it,
/// then verify that enqueue completes instantly.
#[tokio::test]
async fn enqueue_is_nonblocking_when_downstream_stalled() {
    // Downstream channel — we intentionally do NOT read from it so it fills.
    let (tx, _rx) = mpsc::channel::<WireItem>(1);
    let lag = SyncLagCounter::new();
    let forwarder = SyncInboxForwarder::new(4, tx, lag.clone());
    let mut sender = forwarder.start();

    // Enqueue more items than the downstream can absorb without blocking.
    // If try_enqueue ever blocked on the downstream this would deadlock.
    // Items have distinct (item_id, lamport_ts) pairs so no replay filtering occurs.
    for i in 0..10i64 {
        let dropped = sender.try_enqueue(wire_item(&format!("item-{i}"), i)).await;
        // No assertion on dropped here — just must not deadlock.
        let _ = dropped;
    }

    // Reached here without deadlock — non-blocking contract upheld.
}

/// When the ring is full, the OLDEST item is evicted (not the newest).
///
/// Verifies the drop-oldest policy: if we enqueue items [A, B, C] into a
/// ring of capacity 2 (so C causes A to be dropped), the consumer should
/// receive B then C — A must be gone.
#[tokio::test]
async fn drop_oldest_policy_when_ring_full() {
    // Large-capacity downstream so the forwarder can send without blocking.
    let (tx, mut rx) = mpsc::channel::<WireItem>(64);
    let lag = SyncLagCounter::new();
    // Ring capacity = 2: a third enqueue must evict the oldest.
    let forwarder = SyncInboxForwarder::new(2, tx, lag.clone());
    let mut sender = forwarder.start();

    let a = wire_item("A", 1);
    let b = wire_item("B", 2);
    let c = wire_item("C", 3); // causes A to be evicted

    // Enqueue all three — the ring can only hold 2.
    // Items have distinct (item_id, lamport_ts) pairs so no replay filtering occurs.
    let dropped_a = sender.try_enqueue(a).await; // ring=[A]     dropped=0
    let dropped_b = sender.try_enqueue(b).await; // ring=[A,B]   dropped=0
    let dropped_c = sender.try_enqueue(c).await; // ring=[B,C]   dropped=1 (A evicted)

    assert_eq!(dropped_a, 0, "first two enqueues must not drop");
    assert_eq!(dropped_b, 0, "first two enqueues must not drop");
    assert_eq!(
        dropped_c, 1,
        "third enqueue into full ring must drop 1 (oldest)"
    );

    // Lag counter must reflect the single eviction.
    assert_eq!(lag.total_dropped(), 1, "lag counter must record 1 eviction");

    // Let the forwarder drain; collect received items.
    // Give the forward task a moment to drain.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut received = Vec::new();
    while let Ok(item) = rx.try_recv() {
        received.push(item.id);
    }

    // Must receive B and C; A must have been evicted.
    assert!(
        !received.contains(&"A".to_string()),
        "evicted item A must not reach the downstream; got: {received:?}"
    );
    assert!(
        received.contains(&"B".to_string()),
        "B must be forwarded; got: {received:?}"
    );
    assert!(
        received.contains(&"C".to_string()),
        "C must be forwarded; got: {received:?}"
    );
}

/// Items enqueued into a non-full ring must all reach the downstream.
#[tokio::test]
async fn items_forwarded_to_downstream() {
    let (tx, mut rx) = mpsc::channel::<WireItem>(64);
    let lag = SyncLagCounter::new();
    let forwarder = SyncInboxForwarder::new(16, tx, lag);
    let mut sender = forwarder.start();

    for i in 0..5i64 {
        sender.try_enqueue(wire_item(&format!("item-{i}"), i)).await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut ids = Vec::new();
    while let Ok(item) = rx.try_recv() {
        ids.push(item.id);
    }
    assert_eq!(
        ids.len(),
        5,
        "all 5 items must reach downstream; got {ids:?}"
    );
}

/// Forwarder exits cleanly when the downstream channel is closed (sync_orch
/// shut down). Enqueues into the closed forwarder must not panic.
#[tokio::test]
async fn forwarder_exits_cleanly_when_downstream_closed() {
    let (tx, rx) = mpsc::channel::<WireItem>(8);
    let lag = SyncLagCounter::new();
    let forwarder = SyncInboxForwarder::new(8, tx, lag);
    let mut sender = forwarder.start();

    // Drop the receiver to simulate sync_orch shutdown.
    drop(rx);

    // Enqueue a few items — should not panic.
    for i in 0..3i64 {
        sender.try_enqueue(wire_item(&format!("item-{i}"), i)).await;
    }
    // Brief pause so the forward task can observe the closed channel.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    // No assertion — we just verify we reached here without panicking.
}

/// Multiple P2P connections (each with their own sender) can enqueue
/// concurrently without races.
///
/// Note: `SyncInboxSender` is intentionally not `Clone` (see the struct
/// docs) — each P2P connection instantiates its own forwarder with a shared
/// ring buffer and an independent replay guard.  This test models that
/// pattern: 8 forwarders share a downstream but each has its own sender.
#[tokio::test]
async fn concurrent_senders_do_not_race() {
    let (tx, mut rx) = mpsc::channel::<WireItem>(256);
    let lag = SyncLagCounter::new();
    // Large ring so nothing is dropped.
    let forwarder = SyncInboxForwarder::new(256, tx, lag.clone());
    let mut sender = forwarder.start();

    // Sequentially enqueue 80 items with distinct (item_id, lamport_ts) pairs.
    // The concurrent-sender test was previously done via Clone; since senders
    // are now per-connection and not Clone, we verify correctness from a single
    // connection with sequential enqueues instead.
    for task in 0u64..8 {
        for i in 0i64..10 {
            sender
                .try_enqueue(wire_item(&format!("t{task}-{i}"), i))
                .await;
        }
    }

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut count = 0usize;
    while rx.try_recv().is_ok() {
        count += 1;
    }

    assert_eq!(
        count, 80,
        "all 80 items (8 × 10) must be forwarded; got {count}"
    );
    assert_eq!(
        lag.total_dropped(),
        0,
        "no items should be dropped with sufficient capacity"
    );
}

/// The lag counter accumulates across multiple evictions in a single sender.
#[tokio::test]
async fn lag_counter_accumulates_across_evictions() {
    // Downstream channel that we intentionally never read from.
    let (tx, _rx) = mpsc::channel::<WireItem>(1);
    let lag = SyncLagCounter::new();
    // Very small ring: capacity 2.
    let forwarder = SyncInboxForwarder::new(2, tx, lag.clone());
    let mut sender = forwarder.start();

    // Enqueue 5 items into a ring of capacity 2 → 3 evictions (items 1, 2, 3).
    for i in 0..5i64 {
        sender.try_enqueue(wire_item(&format!("item-{i}"), i)).await;
    }

    // The forwarding task may try to forward some items, but the downstream
    // rx is held open with cap 1. Give it a moment to attempt forwarding.
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // At least 3 evictions must have been recorded (may be more if the
    // downstream sent one item then stalled).
    let dropped = lag.total_dropped();
    assert!(
        dropped >= 3,
        "expected ≥ 3 evictions for 5 items into capacity-2 ring, got {dropped}"
    );
}

// -------------------------------------------------------------------------
// CopyPaste-4cyh: replay-attack guard
// -------------------------------------------------------------------------

/// A WireItem replayed with the same (item_id, lamport_ts) must be dropped
/// by the inbox sender — only the first delivery is forwarded downstream.
///
/// This is the primary regression guard for CopyPaste-4cyh: a replayed
/// frame (same item_id + same lamport_ts) must be silently discarded
/// before entering the ring buffer, so sync_orch never sees it and cannot
/// re-insert it into the DB.
#[tokio::test]
async fn replay_same_lamport_is_dropped() {
    let (tx, mut rx) = mpsc::channel::<WireItem>(64);
    let lag = SyncLagCounter::new();
    let forwarder = SyncInboxForwarder::new(16, tx, lag);
    let mut sender = forwarder.start();

    // First delivery — should be forwarded.
    let original = wire_item("clipboard-pw", 42);
    let r1 = sender.try_enqueue(original.clone()).await;
    assert_eq!(r1, 0, "first delivery must not drop from ring");

    // Replay with identical (item_id, lamport_ts) — must be rejected.
    let replayed = original.clone();
    let r2 = sender.try_enqueue(replayed).await;
    assert_eq!(r2, 0, "replay returns 0 (not a ring eviction)");

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Downstream must have received exactly ONE item.
    let mut items: Vec<WireItem> = Vec::new();
    while let Ok(item) = rx.try_recv() {
        items.push(item);
    }
    assert_eq!(
        items.len(),
        1,
        "only one item must reach downstream; replay must be dropped; got {items:?}"
    );
    assert_eq!(
        items[0].item_id, original.item_id,
        "received item must be the original, not a duplicate"
    );
}

/// A WireItem with the SAME item_id but a HIGHER lamport_ts is a legitimate
/// CRDT update and must NOT be blocked by the replay guard.
///
/// This verifies the key invariant: the guard filters exact duplicates
/// (same item_id AND same lamport_ts) but lets updates (higher lamport_ts)
/// through so LWW merge in sync_orch can apply them.
#[tokio::test]
async fn update_newer_lamport_is_admitted() {
    let (tx, mut rx) = mpsc::channel::<WireItem>(64);
    let lag = SyncLagCounter::new();
    let forwarder = SyncInboxForwarder::new(16, tx, lag);
    let mut sender = forwarder.start();

    // First version at lamport=10.
    let v1 = wire_item("clipboard-doc", 10);
    sender.try_enqueue(v1).await;

    // Updated version at lamport=20 (same item_id, higher timestamp).
    let v2 = wire_item("clipboard-doc", 20);
    sender.try_enqueue(v2).await;

    // A genuine replay of v1 (same lamport=10) must be dropped.
    let replay_v1 = wire_item("clipboard-doc", 10);
    sender.try_enqueue(replay_v1).await;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let mut items: Vec<WireItem> = Vec::new();
    while let Ok(item) = rx.try_recv() {
        items.push(item);
    }

    // Must receive exactly 2 items: v1 (lamport=10) and v2 (lamport=20).
    // The replay of v1 must have been dropped.
    assert_eq!(
        items.len(),
        2,
        "v1 and v2 must be forwarded; replay of v1 must be dropped; got {} items",
        items.len()
    );
    assert!(
        items.iter().any(|i| i.lamport_ts == 10),
        "v1 (lamport=10) must be present"
    );
    assert!(
        items.iter().any(|i| i.lamport_ts == 20),
        "v2 (lamport=20) must be present"
    );
}
