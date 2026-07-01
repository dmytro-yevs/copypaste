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

pub(crate) mod catchup;
pub(crate) mod merge;
pub(crate) mod pasteboard;
pub(crate) mod poison;
pub(crate) mod rekey;
// Shared test fixtures (ADR-017, CopyPaste-vp63.3) used by this module's own
// `run()` tests plus `merge_tests.rs` and `rekey`'s test submodules.
#[cfg(test)]
mod test_support;

// ── Public re-exports (keep the flat public surface identical to the old file)

pub use catchup::{catchup_items, catchup_read_raw, rekey_catchup_items};
pub use merge::{merge_incoming, merge_incoming_with_crypto};
pub use poison::{is_poison_wire, sweep_poison_rows};
pub use rekey::{
    rekey_outbound_for_peer, AutoApplyCtx, RekeyOutcome, SyncCrypto, SYNC_MAX_BLOB_BYTES,
};

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use copypaste_core::ClipboardItem;
use copypaste_sync::{merge::local_to_wire_owned, protocol::WireItem};

/// Run the sync orchestrator until both upstream channels close or `shutdown`
/// is cancelled.
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
/// * `storage_quota_bytes` — byte cap passed to `prune_to_cap` after each
///   successful P2P merge so the local DB stays bounded (mirrors the cloud path).
/// * `auto_apply` — when `Some`, enables the Universal Clipboard feature: a
///   genuinely fresh incoming item (newer than the current local clipboard) is
///   written to NSPasteboard immediately after merge, with the self-write guard
///   armed to prevent re-capture by the poller.
/// * `shutdown` — D2: token cancelled by the daemon on SIGINT/SIGTERM so the
///   orchestrator exits promptly instead of waiting for channels to drain.
///
/// Returns `Ok(())` once both channels close or `shutdown` fires.
// `run` takes: db, new_item_rx, incoming_rx, outbound_tx, device_id, crypto,
// storage_quota_bytes, auto_apply, and shutdown — each a distinct runtime
// dependency; no struct without pulling daemon internals into copypaste-sync.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    db: Arc<Mutex<copypaste_core::Database>>,
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut incoming_rx: mpsc::Receiver<WireItem>,
    outbound_tx: mpsc::Sender<WireItem>,
    device_id: String,
    crypto: Option<SyncCrypto>,
    storage_quota_bytes: i64,
    auto_apply: Option<AutoApplyCtx>,
    shutdown: CancellationToken,
) -> anyhow::Result<()> {
    info!(%device_id, has_crypto = crypto.is_some(), "sync orchestrator started");

    let mut local_closed = false;
    let mut incoming_closed = false;

    while !(local_closed && incoming_closed) {
        tokio::select! {
            // D2: exit promptly on daemon-wide shutdown signal.
            _ = shutdown.cancelled() => {
                info!("sync orchestrator: shutdown signal received, stopping");
                break;
            }
            // Local clipboard → forward to transport for fan-out.
            local = new_item_rx.recv(), if !local_closed => {
                match local {
                    Ok(item) => {
                        // tke7 (PG-30): master sync gate — checked on every outbound
                        // item so a runtime set_config toggle takes effect immediately.
                        // Reads from AutoApplyCtx.core_config (shared Arc) when
                        // available; defaults to enabled when ctx is absent (P2P off
                        // anyway, so the gate is moot).
                        let sync_enabled = auto_apply
                            .as_ref()
                            .and_then(|ctx| ctx.core_config.read().ok().map(|g| g.sync_enabled))
                            .unwrap_or(true);
                        if !sync_enabled {
                            debug!(
                                item_id = %item.item_id,
                                "sync_orch: sync_enabled=false; not forwarding to P2P peers"
                            );
                            continue;
                        }

                        // P1-1: honour the "sensitive items are NEVER uploaded" guarantee.
                        // Block P2P transport just like relay and cloud paths.
                        if item.is_sensitive {
                            debug!(
                                item_id = %item.item_id,
                                "sync_orch: skipping sensitive item (never forwarded to P2P peers)"
                            );
                            continue;
                        }
                        // CopyPaste-ux2i: `item` is owned here and unused after the
                        // wire item is built, so move its content blobs instead of
                        // cloning them.
                        let wire = local_to_wire_owned(item, &device_id);
                        // CopyPaste-716: per-peer re-keying now happens in the
                        // transport's fanout_to_peers (p2p.rs) so each peer
                        // receives a blob encrypted under its own pairwise sync
                        // key. Sending the raw at-rest wire here is safe because
                        // the outbound_loop holds a SyncCrypto and re-encrypts
                        // once per peer at send time. When crypto is None (P2P
                        // disabled) the raw ciphertext is forwarded as before.
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
                        // tke7 (PG-30): gate inbound storage behind sync_enabled.
                        // When sync is off, we accept the wire frame from the
                        // transport layer (to keep the channel alive) but discard
                        // the payload rather than merging it into the local DB.
                        let sync_enabled_inbound = auto_apply
                            .as_ref()
                            .and_then(|ctx| ctx.core_config.read().ok().map(|g| g.sync_enabled))
                            .unwrap_or(true);
                        if !sync_enabled_inbound {
                            debug!(
                                item_id = %wire.id,
                                "sync_orch: sync_enabled=false; discarding inbound P2P item"
                            );
                            continue;
                        }
                        if let Err(e) = merge_incoming_with_crypto(
                            &db,
                            vec![wire],
                            crypto.as_ref(),
                            storage_quota_bytes,
                            auto_apply.as_ref(),
                        ).await {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync_orch::test_support::{make_db, make_wire};

    /// PG-30 gap characterization test (CopyPaste-vp63.3): the `sync_enabled`
    /// master toggle must short-circuit BOTH the outbound leg (local item ->
    /// transport) and the inbound leg (incoming wire -> DB merge). Added
    /// FIRST (before any structural extraction) per the sketch's flagged gap
    /// — `run()`'s own logic is the part of this file NOT already covered by
    /// the relocated rekey/merge/catchup/poison tests.
    #[tokio::test]
    async fn run_drops_outbound_and_inbound_when_sync_disabled() {
        let db = make_db();

        let (local_tx, local_rx) = broadcast::channel::<ClipboardItem>(8);
        let (incoming_tx, incoming_rx) = mpsc::channel::<WireItem>(8);
        let (outbound_tx, mut outbound_rx) = mpsc::channel::<WireItem>(8);

        let core_config = Arc::new(std::sync::RwLock::new(copypaste_core::AppConfig {
            sync_enabled: false,
            ..copypaste_core::AppConfig::default()
        }));
        let auto_apply = AutoApplyCtx {
            self_write_change_count: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            local_key: Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            core_config,
        };

        let db_for_task = db.clone();
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
                None,
                500_000_000, // storage_quota_bytes: 500 MB (test default)
                Some(auto_apply),
                shutdown,
            )
            .await
            .expect("orchestrator must finish cleanly");
        });

        // Outbound leg: a local item must NOT reach the transport channel.
        let item = ClipboardItem::new_text(vec![0xEE], vec![0u8; 24], 1);
        local_tx.send(item).expect("broadcast send");

        // Inbound leg: an incoming wire item must NOT be merged into the DB.
        let wire = make_wire("gated-item", 1, 0xAA);
        incoming_tx.send(wire).await.expect("send incoming");

        // Give the orchestrator a moment to (not) process either leg.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        assert!(
            outbound_rx.try_recv().is_err(),
            "sync_enabled=false must drop the outbound item, not forward it to transport"
        );

        drop(local_tx);
        drop(incoming_tx);
        handle.await.expect("task join");

        let g = db.lock().await;
        let rows = copypaste_core::get_page(&*g, 10, 0).expect("get_page");
        assert!(
            rows.is_empty(),
            "sync_enabled=false must discard the inbound item, not merge it into the DB"
        );
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
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
                None,
                500_000_000, // storage_quota_bytes: 500 MB (test default)
                None,        // auto_apply: disabled in tests (no NSPasteboard)
                shutdown,
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
        let rows = copypaste_core::get_page(&*db_guard, 10, 0).expect("get_page");
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
        let shutdown = CancellationToken::new();
        let handle = tokio::spawn(async move {
            run(
                db_for_task,
                local_rx,
                incoming_rx,
                outbound_tx,
                "local-device".to_string(),
                None,
                500_000_000, // storage_quota_bytes: 500 MB (test default)
                None,        // auto_apply: disabled in tests (no NSPasteboard)
                shutdown,
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
}
