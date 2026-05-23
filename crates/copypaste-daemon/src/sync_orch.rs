//! Sync orchestrator — wires `copypaste-sync` into the daemon.
//!
//! Responsibilities:
//!
//! 1. Subscribe to the daemon's local `new_item_tx` broadcast channel and
//!    convert each freshly-inserted [`ClipboardItem`] into a [`WireItem`],
//!    forwarding it on `outbound_tx` for the transport layer to deliver.
//! 2. Consume incoming [`WireItem`]s pushed by the transport layer via
//!    `incoming_rx` and merge them into the local SQLite database using the
//!    Last-Write-Wins rules defined in `copypaste-sync::merge`.
//!
//! ## Why channels instead of a Transport trait?
//!
//! The actual peer transports (mTLS-over-TCP from `copypaste-p2p`, the
//! Supabase relay from `cloud.rs`, or a future WebRTC channel) live in
//! sibling modules. We expose two `tokio::sync` channels — outbound and
//! inbound — so the orchestrator stays pure I/O-free merge logic and the
//! tests remain hermetic. The transport layer owns the network side and just
//! forwards bytes through these channels.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{debug, info, warn};

use copypaste_core::{insert_item, ClipboardItem, Database};
use copypaste_sync::{
    merge::{local_to_wire, resolve, wire_to_local, MergeOutcome},
    protocol::WireItem,
};

/// Run the sync orchestrator until both upstream channels close.
///
/// * `db` — shared handle to the local SQLite store.
/// * `new_item_rx` — broadcast receiver from `daemon::run`; carries items
///   produced by the local clipboard monitor.
/// * `incoming_rx` — `mpsc` receiver fed by the transport layer with items
///   received from remote peers.
/// * `outbound_tx` — `mpsc` sender drained by the transport layer to push
///   locally-produced items to peers. A closed receiver is logged and
///   ignored — peers may simply not be connected.
/// * `device_id` — UUID stamped as `origin_device_id` on outgoing items.
///
/// Returns `Ok(())` once both `new_item_rx` and `incoming_rx` are closed
/// (typically during daemon shutdown).
pub async fn run(
    db: Arc<Mutex<Database>>,
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut incoming_rx: mpsc::Receiver<WireItem>,
    outbound_tx: mpsc::Sender<WireItem>,
    device_id: String,
) -> anyhow::Result<()> {
    info!(%device_id, "sync orchestrator started");

    let mut local_closed = false;
    let mut incoming_closed = false;

    while !(local_closed && incoming_closed) {
        tokio::select! {
            // Local clipboard → forward to transport for fan-out.
            local = new_item_rx.recv(), if !local_closed => {
                match local {
                    Ok(item) => {
                        let wire = local_to_wire(&item, &device_id);
                        debug!(item_id = %wire.id, "sync_orch: forwarding local item to transport");
                        if let Err(e) = outbound_tx.send(wire).await {
                            // No transport listening — normal when P2P/cloud disabled.
                            debug!("sync_orch: outbound channel closed: {e}");
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        warn!("sync_orch: broadcast lagged by {n} items");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("sync_orch: local channel closed");
                        local_closed = true;
                    }
                }
            }
            // Incoming peer item → LWW merge into DB.
            incoming = incoming_rx.recv(), if !incoming_closed => {
                match incoming {
                    Some(wire) => {
                        if let Err(e) = merge_incoming(&db, vec![wire]).await {
                            warn!("sync_orch: merge_incoming failed: {e}");
                        }
                    }
                    None => {
                        info!("sync_orch: incoming channel closed");
                        incoming_closed = true;
                    }
                }
            }
        }
    }

    info!("sync orchestrator stopped");
    Ok(())
}

/// Apply LWW conflict resolution and persist any items that should win.
///
/// For each incoming [`WireItem`]:
///
/// * If the local row is missing, insert the wire version (marked synced).
/// * If the local row exists, [`resolve`] picks the winner; on `TakeRemote`
///   we delete the stale local row and insert the wire version.
///
/// Returns the number of rows that were actually upserted (i.e. winners
/// that replaced or supplemented local state). The orchestrator itself
/// ignores the count — it is exposed for tests and telemetry.
pub async fn merge_incoming(
    db: &Arc<Mutex<Database>>,
    items: Vec<WireItem>,
) -> anyhow::Result<usize> {
    if items.is_empty() {
        return Ok(0);
    }

    let db_guard = db.lock().await;
    // Snapshot local rows once so we can compare every incoming item without
    // re-querying. History is bounded by the daemon's `history_limit`, so
    // this is cheap (low thousands of rows in practice).
    let local: Vec<ClipboardItem> = copypaste_core::get_page(&db_guard, 10_000, 0)
        .map_err(|e| anyhow::anyhow!("sync_orch: get_page: {e}"))?;
    let local_by_id: std::collections::HashMap<&str, &ClipboardItem> =
        local.iter().map(|i| (i.id.as_str(), i)).collect();

    let mut upserted = 0usize;
    for wire in items {
        let exists = local_by_id.contains_key(wire.id.as_str());
        let take_remote = match local_by_id.get(wire.id.as_str()) {
            Some(existing) => matches!(resolve(existing, &wire), MergeOutcome::TakeRemote),
            None => true,
        };

        if !take_remote {
            debug!(item_id = %wire.id, "sync_orch: LWW kept local");
            continue;
        }

        // `clipboard_items.id` is the PK and `insert_item` uses plain INSERT
        // (not REPLACE), so existing rows must be deleted first.
        if exists {
            if let Err(e) = copypaste_core::delete_item(&db_guard, &wire.id) {
                warn!(item_id = %wire.id, "sync_orch: delete before reinsert failed: {e}");
                continue;
            }
        }

        let to_insert = wire_to_local(wire);
        match insert_item(&db_guard, &to_insert) {
            Ok(()) => {
                debug!(item_id = %to_insert.id, "sync_orch: upserted incoming item");
                upserted += 1;
            }
            Err(e) => warn!(item_id = %to_insert.id, "sync_orch: insert failed: {e}"),
        }
    }
    Ok(upserted)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_db() -> Arc<Mutex<Database>> {
        Arc::new(Mutex::new(
            Database::open_in_memory().expect("in-memory DB must open"),
        ))
    }

    fn make_wire(id: &str, lamport: i64, content: u8) -> WireItem {
        WireItem {
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
        }
    }

    /// W2.2: an incoming WireItem from the transport must be persisted to the
    /// local DB via the LWW merge path.
    #[tokio::test]
    async fn sync_orch_inserts_incoming_wire_item() {
        let db = make_db();

        let (_local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
        let (incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
        let (outbound_tx, _outbound_rx) = mpsc::channel::<WireItem>(8);

        let db_for_task = db.clone();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
            )
            .await
            .expect("orchestrator must finish cleanly");
        });

        // Push one wire item from the "transport".
        let wire = make_wire("new-item", 5, 0xAB);
        incoming_tx.send(wire).await.expect("send incoming");

        // Let the orchestrator merge.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Drop senders so the orchestrator exits.
        drop(incoming_tx);
        drop(_local_tx);
        handle.await.expect("task join");

        let db_guard = db.lock().await;
        let rows = copypaste_core::get_page(&db_guard, 10, 0).expect("get_page");
        assert_eq!(rows.len(), 1, "incoming item must be persisted");
        assert_eq!(rows[0].id, "new-item");
        assert!(rows[0].is_synced, "item from peer must be marked synced");
        assert_eq!(rows[0].lamport_ts, 5);
    }

    /// W2.2: a locally-produced item arriving on the broadcast channel must
    /// be forwarded to the transport's outbound channel.
    #[tokio::test]
    async fn sync_orch_broadcasts_local_item() {
        let db = make_db();

        let (local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
        let (_incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<WireItem>(8);

        let db_for_task = db.clone();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
            )
            .await
            .expect("orchestrator must finish cleanly");
        });

        // Push a local item through the broadcast channel.
        let item = ClipboardItem::new_text(vec![0xCC, 0xDD], vec![0u8; 24], 9);
        let item_id = item.id.clone();
        local_tx.send(item).expect("broadcast send");

        // Receive on the transport side.
        let received =
            tokio::time::timeout(std::time::Duration::from_millis(200), outbound_rx.recv())
                .await
                .expect("must receive within 200ms")
                .expect("outbound channel must yield item");

        assert_eq!(received.id, item_id, "wire id must match local id");
        assert_eq!(
            received.origin_device_id, "local-device",
            "origin_device_id must be stamped by the orchestrator"
        );
        assert_eq!(received.lamport_ts, 9);

        // Tear down and join.
        drop(local_tx);
        drop(_incoming_tx);
        handle.await.expect("task join");
    }

    /// LWW: a stale wire item (lower lamport) must NOT overwrite the local row.
    #[tokio::test]
    async fn merge_incoming_keeps_local_on_older_remote() {
        let db = make_db();
        // Pre-insert a local row with a higher lamport clock.
        let mut local = ClipboardItem::new_text(vec![0x11], vec![0u8; 24], 50);
        local.id = "shared".to_string();
        {
            let g = db.lock().await;
            insert_item(&g, &local).unwrap();
        }

        let wire = make_wire("shared", 5, 0xFF); // older
        let upserted = merge_incoming(&db, vec![wire]).await.unwrap();
        assert_eq!(upserted, 0, "older remote must lose LWW");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&g, 10, 0).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].content, Some(vec![0x11]), "local payload preserved");
    }
}
