//! Outbound fanout loop and catch-up replay.

use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use copypaste_core::ClipboardItem;
use copypaste_p2p::transport::DeviceFingerprint;
use copypaste_sync::protocol::{PeerFrame, WireItem};

use super::{CatchupProvider, PeerSinks};

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
            Ok(()) => {}
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
