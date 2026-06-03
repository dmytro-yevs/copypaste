//! Sync orchestrator integration tests — beta-bonus.
//!
//! ## API gap (documented per task scope)
//!
//! `copypaste-daemon` is a **binary-only crate** (no `src/lib.rs`).  As a
//! result, `crate::sync_orch::run` cannot be imported from
//! `tests/*.rs` integration files — only `pub` items from a library crate are
//! reachable from external test binaries.
//!
//! Per the test scope ("DO NOT modify src/* or any other crate") we exercise
//! the **same contract** that `sync_orch::run` enforces, using the public
//! `copypaste-sync` and `copypaste-core` APIs directly:
//!
//! 1. The orchestrator owns three channels: a `broadcast::Receiver` for local
//!    items, an `mpsc::Receiver` for incoming wire items, and an
//!    `mpsc::Sender` for outbound wire items.
//! 2. Shutdown is signalled by **closing the upstream senders** (no explicit
//!    `Shutdown` message — `run()` exits when both `new_item_rx` and
//!    `incoming_rx` are closed).  These tests verify the same close-to-shutdown
//!    semantics in a hermetic, stand-in orchestrator built on the public APIs.
//! 3. Incoming `WireItem`s are merged into the DB via the LWW rules defined in
//!    `copypaste-sync::merge::{resolve, wire_to_local}` — the same helpers
//!    `sync_orch::merge_incoming` calls.
//!
//! ## Resolution path (post-beta)
//!
//! Promote `copypaste-daemon` to a hybrid binary+library crate by adding
//! `src/lib.rs` that re-exports `pub mod sync_orch;` and `pub mod p2p;` (the
//! existing `src/main.rs` would then `use copypaste_daemon::*`).  Until then
//! these tests exercise the contract, not the daemon-internal wiring.

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::timeout;

use copypaste_core::{insert_item, ClipboardItem, Database};
use copypaste_sync::{
    merge::{local_to_wire, resolve, wire_to_local, MergeOutcome},
    protocol::WireItem,
};

// ── helpers ──────────────────────────────────────────────────────────────────

fn make_db() -> Arc<Mutex<Database>> {
    Arc::new(Mutex::new(
        Database::open_in_memory().expect("in-memory DB must open"),
    ))
}

fn make_wire(id: &str, lamport: i64, content: u8) -> WireItem {
    WireItem {
        deleted: false,
        pinned: false,
        pin_order: None,
        id: id.to_string(),
        item_id: format!("{id}-iid"),
        content_type: "text".to_string(),
        content: Some(vec![content]),
        content_nonce: Some(vec![0u8; 24]),
        blob_ref: None,
        is_sensitive: false,
        lamport_ts: lamport,
        wall_time: 1_700_000_000_000 + lamport,
        expires_at: None,
        app_bundle_id: None,
        origin_device_id: "remote-device".to_string(),
        key_version: 2,
        file_name: None,
        mime: None,
    }
}

/// Stand-in orchestrator that mirrors `daemon::sync_orch::run` using only
/// public API surfaces.  Kept in the test file (not in production) because
/// the task scope forbids new production code.
async fn run_orch_stand_in(
    db: Arc<Mutex<Database>>,
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut incoming_rx: mpsc::Receiver<WireItem>,
    outbound_tx: mpsc::Sender<WireItem>,
    device_id: String,
) {
    let mut local_closed = false;
    let mut incoming_closed = false;

    while !(local_closed && incoming_closed) {
        tokio::select! {
            local = new_item_rx.recv(), if !local_closed => {
                match local {
                    Ok(item) => {
                        let wire = local_to_wire(&item, &device_id);
                        let _ = outbound_tx.send(wire).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => local_closed = true,
                }
            }
            incoming = incoming_rx.recv(), if !incoming_closed => {
                match incoming {
                    Some(wire) => {
                        // Mirror `sync_orch::merge_incoming` LWW path.
                        let g = db.lock().await;
                        let local: Vec<ClipboardItem> =
                            copypaste_core::get_page(&g, 10_000, 0).unwrap_or_default();
                        let take = match local.iter().find(|i| i.id == wire.id) {
                            Some(existing) => matches!(resolve(existing, &wire), MergeOutcome::TakeRemote),
                            None => true,
                        };
                        if take {
                            let existed = local.iter().any(|i| i.id == wire.id);
                            if existed {
                                let _ = copypaste_core::delete_item(&g, &wire.id);
                            }
                            let to_insert = wire_to_local(wire);
                            let _ = insert_item(&g, &to_insert);
                        }
                    }
                    None => incoming_closed = true,
                }
            }
        }
    }
}

// ── tests ────────────────────────────────────────────────────────────────────

/// Contract: closing **both** upstream senders causes the orchestrator task
/// to terminate within 1 s (the task spec's "Shutdown" semantic — the real
/// orchestrator has no explicit Shutdown enum, it exits on channel close).
#[tokio::test(flavor = "multi_thread")]
async fn shutdown_when_both_senders_dropped_joins_within_1s() {
    let db = make_db();
    let (local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
    let (incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
    let (outbound_tx, _outbound_rx) = mpsc::channel::<WireItem>(8);

    let handle = tokio::spawn(run_orch_stand_in(
        db,
        local_rx,
        incoming_rx,
        outbound_tx,
        "local-device".to_string(),
    ));

    // Drop both senders — the orchestrator must exit its select loop.
    drop(local_tx);
    drop(incoming_tx);

    timeout(Duration::from_secs(1), handle)
        .await
        .expect("orchestrator must join within 1s after senders dropped")
        .expect("task join clean");
}

/// Contract: a dummy incoming `WireItem` must be persisted via the merge
/// pipeline — this is the "handler invoked" assertion from the task spec.
#[tokio::test(flavor = "multi_thread")]
async fn dummy_incoming_wire_item_invokes_merge_handler() {
    let db = make_db();
    let (local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
    let (incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
    let (outbound_tx, _outbound_rx) = mpsc::channel::<WireItem>(8);

    let db_for_task = db.clone();
    let handle = tokio::spawn(run_orch_stand_in(
        db_for_task,
        local_rx,
        incoming_rx,
        outbound_tx,
        "local-device".to_string(),
    ));

    // "mock callback" → assert the merge handler actually wrote the row.
    let wire = make_wire("dummy-1", 7, 0x42);
    incoming_tx.send(wire).await.expect("send incoming");

    // Give the select loop a tick to drain.
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Verify handler invoked: row exists in DB.
    {
        let g = db.lock().await;
        let rows = copypaste_core::get_page(&g, 10, 0).expect("get_page");
        assert_eq!(rows.len(), 1, "merge handler must have inserted the item");
        assert_eq!(rows[0].id, "dummy-1");
        assert_eq!(rows[0].lamport_ts, 7);
        assert!(rows[0].is_synced, "incoming items must be marked is_synced");
    }

    // Clean shutdown.
    drop(local_tx);
    drop(incoming_tx);
    timeout(Duration::from_secs(1), handle)
        .await
        .expect("orchestrator must join")
        .expect("task join clean");
}

/// Contract: a locally-broadcast item is converted to a `WireItem` and pushed
/// to the outbound channel with the correct `origin_device_id` stamp.
#[tokio::test(flavor = "multi_thread")]
async fn local_broadcast_fans_out_to_outbound_with_device_id_stamp() {
    let db = make_db();
    let (local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
    let (incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<WireItem>(8);

    let handle = tokio::spawn(run_orch_stand_in(
        db,
        local_rx,
        incoming_rx,
        outbound_tx,
        "device-alpha".to_string(),
    ));

    let item = ClipboardItem::new_text(vec![0xCA, 0xFE], vec![0u8; 24], 12);
    let expected_id = item.id.clone();
    local_tx.send(item).expect("broadcast send");

    let wire = timeout(Duration::from_millis(200), outbound_rx.recv())
        .await
        .expect("outbound recv within 200ms")
        .expect("outbound channel must yield");

    assert_eq!(wire.id, expected_id, "wire id must equal local item id");
    assert_eq!(
        wire.origin_device_id, "device-alpha",
        "orchestrator must stamp the configured device_id"
    );
    assert_eq!(wire.lamport_ts, 12);

    drop(local_tx);
    drop(incoming_tx);
    timeout(Duration::from_secs(1), handle)
        .await
        .expect("orchestrator must join")
        .expect("task join clean");
}
