//! Outbound fanout loop and catch-up replay.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use copypaste_core::ClipboardItem;
use copypaste_p2p::transport::DeviceFingerprint;
use copypaste_sync::protocol::{PeerFrame, WireItem};

use super::unpair::stamp_peer_sync;
use super::{CatchupProvider, PeerSinks};

/// Minimum interval between `stamp_peer_sync` calls triggered by a successful
/// outbound send to a given peer (CopyPaste-dkwl) — mirrors
/// `INBOUND_STAMP_THROTTLE` in `framed_pump.rs`. Without this, fanning out a
/// burst of items would rewrite `peers.json` once per item per peer.
const OUTBOUND_STAMP_THROTTLE: Duration = Duration::from_secs(60);

/// Last successful-outbound-send stamp time per peer fingerprint.
///
/// In-process only (reset on daemon restart) — purely a write-rate limiter
/// for `peers.json`, not itself a source of truth. A `std::sync::Mutex` is
/// fine here: the critical section is a single map lookup/insert with no
/// `.await` inside it.
fn last_outbound_stamp_map() -> &'static Mutex<HashMap<DeviceFingerprint, Instant>> {
    static MAP: OnceLock<Mutex<HashMap<DeviceFingerprint, Instant>>> = OnceLock::new();
    MAP.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Stamp `last_sync_at` for `peer` following a successful outbound send,
/// throttled to at most once per [`OUTBOUND_STAMP_THROTTLE`] per peer.
///
/// CopyPaste-dkwl: called only when the item was actually re-keyed
/// successfully (or forwarded via the legacy no-pairwise-key path) AND
/// handed off to the peer's local sink — i.e. NOT on `RekeyOutcome::Failed`.
/// This closes the specific gap that motivated this change: a peer whose
/// pairwise key is broken stops advancing `last_sync_at` from the outbound
/// side, so it correctly goes stale instead of looking healthy forever off
/// the one-time connection-establishment stamp.
fn stamp_outbound_success(peer: &DeviceFingerprint) {
    // Lock poisoning here would only mean a prior panic while holding the
    // map — recover the inner map rather than propagate, since this stamp is
    // best-effort and must never take down the fanout loop.
    let mut map = last_outbound_stamp_map()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let due = map
        .get(peer)
        .is_none_or(|t| t.elapsed() >= OUTBOUND_STAMP_THROTTLE);
    if due {
        map.insert(peer.clone(), Instant::now());
        drop(map); // release before the (blocking, best-effort) file write
        stamp_peer_sync(&crate::ipc::peers_file_path(), peer);
    }
}

/// Outbound fanout loop.
///
/// Receives `WireItem`s from the sync orchestrator via `outbound_rx` and
/// sends each one to every currently-connected peer, re-encrypting once per
/// peer under that peer's pairwise sync key (CopyPaste-716).
///
/// Also drains the `new_item_rx` broadcast channel (previously handled by
/// `subscriber_loop`) so broadcast items are also fanned out.
///
/// Peer sinks whose channel is closed (peer disconnected) are removed from
/// `peer_sinks` on the next fanout pass.
pub(super) async fn outbound_loop(
    mut new_item_rx: broadcast::Receiver<ClipboardItem>,
    mut outbound_rx: mpsc::Receiver<WireItem>,
    peer_sinks: PeerSinks,
    sync_crypto: Option<crate::sync_orch::SyncCrypto>,
    core_config: std::sync::Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    shutdown: CancellationToken,
) {
    tracing::debug!("P2P outbound fanout loop running");

    let mut new_item_closed = false;
    let mut outbound_closed = false;

    loop {
        if new_item_closed && outbound_closed {
            tracing::info!("P2P outbound loop: both upstream channels closed, shutting down");
            break;
        }

        tokio::select! {
            // New clipboard item from the local monitor (broadcast channel).
            result = new_item_rx.recv(), if !new_item_closed => {
                match result {
                    Ok(_item) => {
                        // The clipboard item is stored in the DB; the sync orchestrator
                        // converts it to a WireItem and sends it via outbound_rx.
                        // We log only at debug to avoid double-counting.
                        tracing::debug!("P2P: new local clipboard item (sync_orch will forward)");
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("P2P outbound loop lagged by {n} items");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        tracing::info!("P2P outbound loop: broadcast channel closed");
                        new_item_closed = true;
                    }
                }
            }
            // Outbound WireItem from sync_orch — fan out to all connected peers,
            // re-encrypting once per peer under that peer's pairwise sync key.
            item_opt = outbound_rx.recv(), if !outbound_closed => {
                match item_opt {
                    Some(item) => {
                        // CopyPaste-7ub: honour `sync_on_wifi_only` on the P2P
                        // outbound path, exactly like the relay (relay/push.rs)
                        // and cloud paths. When the user opted into Wi-Fi-only
                        // sync and the device is on cellular, skip the fanout —
                        // the item is already persisted locally and the
                        // sync-on-connect catch-up replay reconciles it with
                        // peers once back on Wi-Fi. Read the flag live so a
                        // runtime `set_config` change takes effect immediately.
                        let sync_on_wifi_only = core_config
                            .read()
                            .map(|g| g.sync_on_wifi_only)
                            .unwrap_or(false);
                        if sync_on_wifi_only {
                            // fail-open: if the Wi-Fi probe errors, assume Wi-Fi.
                            let on_wifi = tokio::task::spawn_blocking(crate::platform::is_on_wifi)
                                .await
                                .unwrap_or(true);
                            if crate::sync_common::should_skip_on_cellular(sync_on_wifi_only, on_wifi)
                            {
                                tracing::debug!(
                                    "P2P outbound: sync_on_wifi_only=true and not on Wi-Fi — \
                                     skipping fanout (catch-up replay reconciles on reconnect)"
                                );
                                continue;
                            }
                        }
                        fanout_to_peers(&item, &peer_sinks, sync_crypto.as_ref()).await;
                    }
                    None => {
                        tracing::info!("P2P outbound loop: outbound_rx channel closed");
                        outbound_closed = true;
                    }
                }
            }
            // BUG F1: graceful shutdown — break out even while channels are open.
            _ = shutdown.cancelled() => {
                tracing::info!("P2P outbound loop shutting down");
                break;
            }
        }
    }
}

/// Send `item` to every currently-connected peer sink, re-encrypting once per
/// peer under that peer's pairwise sync key (CopyPaste-716).
///
/// Peers whose sender has been closed (disconnected) are removed from
/// `peer_sinks`.
///
/// CopyPaste-716: `sync_crypto` is now used to re-encrypt the raw at-rest wire
/// item once per peer under the correct pairwise key before sending. The old
/// path cloned the same pre-encrypted blob to all peers — breaking >2 device
/// sync because peer C received a K_AB-encrypted blob it could not decrypt.
///
/// M2: the `peer_sinks` lock is held only long enough to *snapshot* the
/// senders (each `mpsc::Sender` is cheap to clone) — never across the actual
/// send. The previous implementation held the lock across `tx.send().await`,
/// so a single slow/backpressured peer stalled all connection management
/// (accept/dial loops insert and remove sinks under the same lock). We now use
/// the non-blocking `try_send` on the dropped-guard snapshot: a `Closed`
/// channel means the peer is gone (pruned), while a transiently `Full` channel
/// (bounded at 64) just drops this best-effort fanout item for that peer — the
/// sync-on-connect catch-up replay reconciles it on the next reconnect, and we
/// must not evict a live peer merely for being momentarily behind.
pub(super) async fn fanout_to_peers(
    item: &WireItem,
    peer_sinks: &PeerSinks,
    sync_crypto: Option<&crate::sync_orch::SyncCrypto>,
) {
    // Snapshot (key, sender) pairs under the lock, then release it before sending.
    let snapshot: Vec<(DeviceFingerprint, mpsc::Sender<PeerFrame>)> = {
        let sinks = peer_sinks.lock().await;
        sinks
            .iter()
            .map(|(key, tx)| (key.clone(), tx.clone()))
            .collect()
    };

    let mut dead_keys: Vec<DeviceFingerprint> = Vec::new();
    for (key, tx) in snapshot {
        // CopyPaste-716: re-encrypt the at-rest wire item under this peer's
        // specific pairwise sync key. Each peer gets its own independently-
        // encrypted clone, so K_AB is never sent to peer C (which needs K_AC).
        let peer_item = if let Some(crypto) = sync_crypto {
            let mut cloned = item.clone();
            let outcome =
                crate::sync_orch::rekey_outbound_for_peer(crypto, key.as_str(), &mut cloned);
            match outcome {
                crate::sync_orch::RekeyOutcome::Rewrapped => cloned,
                crate::sync_orch::RekeyOutcome::Failed => {
                    // sync H2: a key was present but re-keying failed — drop
                    // this item for this peer rather than forwarding an
                    // undecryptable blob. The catch-up replay will retry on
                    // the next reconnect once the root cause is resolved.
                    //
                    // CopyPaste-dkwl (fixed the `last_sync_at` half of
                    // CopyPaste-8ebg.26's gap): deliberately do NOT call
                    // `stamp_outbound_success` here. `last_sync_at` is now
                    // stamped from real successful application-level
                    // exchanges — a successful inbound `Data` frame
                    // (`framed_pump.rs`) or a successful outbound send that
                    // did NOT hit this `Failed` arm (`stamp_outbound_success`
                    // below) — rather than only once at mTLS
                    // handshake (`listener.rs`/`connector/mod.rs`). So a peer
                    // whose key is permanently broken now correctly stops
                    // advancing `last_sync_at` from this device's outbound
                    // side and goes stale, instead of looking healthy forever
                    // off the one-time connection stamp.
                    //
                    // Still deferred (CopyPaste-dkwl notes): a REAL per-peer
                    // rekey-failure counter/flag surfaced to `list_peers`/
                    // Devices, so a stuck peer can be flagged immediately
                    // rather than waiting out `PEER_STALL_THRESHOLD_MS` (30
                    // min) for `last_sync_at` to go stale. That needs a
                    // shared counter threaded in from `p2p/mod.rs::start_p2p`
                    // alongside `peer_sinks`, PLUS a decision on how to
                    // surface it (new IPC/`list_peers` field vs. folding into
                    // an existing one) and a client (SyncStatusChip.tsx)
                    // change to consume it — out of scope for this fix.
                    tracing::warn!(
                        peer = %key,
                        item_id = %item.item_id,
                        "fanout: rekey failed for peer, dropping item (catch-up will reconcile)"
                    );
                    continue;
                }
                crate::sync_orch::RekeyOutcome::NotApplicable => {
                    // No pairwise key for this peer (legacy peer / P2P disabled):
                    // forward the raw at-rest ciphertext as the legacy path did.
                    item.clone()
                }
            }
        } else {
            // No SyncCrypto (P2P crypto disabled): legacy path — clone as-is.
            item.clone()
        };

        match tx.try_send(PeerFrame::Data(peer_item)) {
            Ok(()) => {
                // CopyPaste-dkwl: only reached when rekey did NOT fail for
                // this peer and the frame was actually enqueued — see
                // `stamp_outbound_success`.
                stamp_outbound_success(&key);
            }
            Err(mpsc::error::TrySendError::Full(_)) => {
                tracing::warn!(
                    peer = %key,
                    "peer sink full — dropping fanout item (catch-up will reconcile)"
                );
            }
            Err(mpsc::error::TrySendError::Closed(_)) => {
                tracing::debug!(peer = %key, "peer sink closed — will prune");
                dead_keys.push(key);
            }
        }
    }

    if !dead_keys.is_empty() {
        let mut sinks = peer_sinks.lock().await;
        for key in dead_keys {
            sinks.remove(&key);
        }
    }
}

/// Push the catch-up history for `peer_fingerprint` into a freshly-connected
/// peer's sink.
///
/// CopyPaste-716: the `peer_fingerprint` is forwarded to the `CatchupProvider`
/// so it can look up the pairwise sync key for this specific peer and re-encrypt
/// each item under that key. Previously the provider had no fingerprint arg and
/// used the first cached key for all peers, causing 3rd+ peers to receive blobs
/// encrypted under the wrong key (silent AEAD failure on the receiver).
pub(super) async fn push_catchup(
    catchup: &CatchupProvider,
    peer_fingerprint: &str,
    sink: &mpsc::Sender<PeerFrame>,
) {
    let items = catchup(peer_fingerprint);
    if items.is_empty() {
        return;
    }
    tracing::debug!(
        count = items.len(),
        peer = %peer_fingerprint,
        "P2P sync-on-connect: replaying local history to peer"
    );
    for item in items {
        if sink.send(PeerFrame::Data(item)).await.is_err() {
            tracing::debug!("P2P sync-on-connect: peer sink closed mid-replay");
            return;
        }
    }
}

/// Spawn [`outbound_loop`] as a background task.
///
/// Thin glue extracted from `start_p2p` (ADR-017, CopyPaste-vp63.2) — every
/// argument is already the exact clone `start_p2p` used to build before
/// spawning inline, so this call is behaviourally identical to the former
/// inline `tokio::spawn` block. CopyPaste-716: `sync_crypto` lets
/// `outbound_loop` re-encrypt once per peer under that peer's pairwise key
/// inside `fanout_to_peers`.
pub(super) fn spawn_outbound_loop(
    new_item_rx: broadcast::Receiver<ClipboardItem>,
    outbound_rx: mpsc::Receiver<WireItem>,
    peer_sinks: PeerSinks,
    sync_crypto: Option<crate::sync_orch::SyncCrypto>,
    core_config: std::sync::Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    shutdown: CancellationToken,
) {
    tokio::spawn(async move {
        outbound_loop(
            new_item_rx,
            outbound_rx,
            peer_sinks,
            sync_crypto,
            core_config,
            shutdown,
        )
        .await;
    });
}
