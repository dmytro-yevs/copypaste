//! Durable `pending_unpair.json` delivery (Gap A).
//!
//! Split out of the former flat `p2p/connector.rs` (ADR-017,
//! CopyPaste-vp63.48) — moved verbatim, no behavior change.

use std::net::SocketAddr;

use bytes::Bytes;
use futures_util::SinkExt;

use copypaste_p2p::transport::{PairedPeers, PeerTransport};
use copypaste_sync::protocol::{ControlMsg, PeerFrame};

use super::super::framed_pump::WRITE_TIMEOUT;

/// Deliver any durable `pending_unpair.json` records (Gap A).
///
/// For each queued [`PendingUnpair`](crate::peers::PendingUnpair) that carries a
/// parseable dial address (and is not our own fingerprint), this:
///   1. TEMPORARILY allow-lists the peer's fingerprint on the live
///      [`PairedPeers`] so the outbound mTLS handshake is accepted by the peer
///      (the peer pins US, but we must pin THEM to connect);
///   2. dials the peer and sends ONE `PeerFrame::Control(ControlMsg::Unpair)`;
///   3. removes the peer from the live allowlist again (it must NOT resume sync);
///   4. removes the record from `pending_unpair.json` so it is delivered once.
///
/// Best-effort: a dial/connect/send failure leaves the record in place for a
/// retry on the next tick (the entry is removed from the live allowlist either
/// way, so a transient allow-list never lingers). Records with no address are
/// left untouched — there is nothing to dial.
pub(super) async fn deliver_pending_unpairs(
    transport: &PeerTransport,
    pending_path: &std::path::Path,
    own_fp: &str,
    live_peers: &PairedPeers,
) {
    let pending = crate::peers::load_pending_unpairs(pending_path);
    if pending.is_empty() {
        return;
    }

    for entry in pending {
        let canonical = crate::ipc::canonical_fingerprint(&entry.fingerprint);
        if canonical.is_empty() || canonical == own_fp {
            // Never dial ourselves; drop a degenerate record so it cannot wedge
            // the queue forever.
            let _ = crate::peers::remove_pending_unpair(pending_path, &entry.fingerprint);
            continue;
        }
        let Some(addr_str) = entry.address.as_deref() else {
            // No address — cannot dial. Leave it queued for a future improvement
            // that learns the address out-of-band.
            continue;
        };
        let addr: SocketAddr = match addr_str.parse() {
            Ok(a) => a,
            Err(e) => {
                tracing::debug!(
                    addr = %addr_str,
                    error = %e,
                    "pending-unpair: unparseable address — dropping record"
                );
                let _ = crate::peers::remove_pending_unpair(pending_path, &entry.fingerprint);
                continue;
            }
        };

        // Temporarily allow-list so our own client config will accept the peer's
        // pinned cert on the handshake. Removed again below regardless of outcome.
        live_peers.add(canonical.clone(), entry.name.clone());

        let dialed = transport.connect_with_retry(addr, &canonical).await;
        match dialed {
            Ok(mut stream) => {
                let frame = PeerFrame::Control(ControlMsg::Unpair);
                let sent = match serde_json::to_vec(&frame) {
                    Ok(payload) => {
                        tokio::time::timeout(WRITE_TIMEOUT, stream.send(Bytes::from(payload)))
                            .await
                            .map(|r| r.is_ok())
                            .unwrap_or(false)
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "pending-unpair: failed to serialise Unpair frame");
                        false
                    }
                };
                // Close our end promptly — we have nothing more to say.
                drop(stream);
                if sent {
                    tracing::info!(
                        peer = %canonical,
                        %addr,
                        "pending-unpair: delivered deferred Unpair to reconnected peer"
                    );
                    if let Err(e) =
                        crate::peers::remove_pending_unpair(pending_path, &entry.fingerprint)
                    {
                        tracing::warn!(
                            peer = %canonical,
                            error = %e,
                            "pending-unpair: delivered but failed to dequeue record"
                        );
                    }
                } else {
                    tracing::debug!(
                        peer = %canonical,
                        "pending-unpair: connect ok but send failed — will retry next tick"
                    );
                }
            }
            Err(e) => {
                tracing::debug!(
                    peer = %canonical,
                    %addr,
                    error = %e,
                    "pending-unpair: dial failed — will retry next tick"
                );
            }
        }

        // Always drop the transient allow-list entry so the peer can never
        // resume normal sync through this window.
        live_peers.remove(&canonical);
    }
}
