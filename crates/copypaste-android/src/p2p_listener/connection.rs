//! Per-connection duplex pump: push catch-up once, then read inbound frames
//! forever (no idle/deadline cutoff) until the peer drops, cancels, or an
//! inbound `Unpair` control frame arrives.

use std::sync::{Arc, Mutex};

use copypaste_core::SyncKey;
use copypaste_sync::protocol::{ControlMsg, PeerFrame, WireItem};
use tokio_util::sync::CancellationToken;

use crate::SyncedItem;

use super::codec::decrypt_wire_item;
use super::registry::PeerState;

/// Per-connection pump: push the catch-up history once, then read inbound
/// frames with NO idle/deadline cutoff (keep the link open like the daemon)
/// until the peer drops or `cancel` fires. Every decrypted item is appended to
/// `received`.
///
/// PG-1 (7d8x): now parses every inbound frame as a `PeerFrame` (the same
/// `#[serde(untagged)]` type the outbound `sync_with_peer` in lib.rs uses).
/// `PeerFrame::Data(WireItem)` is the normal case. `PeerFrame::Control(Unpair)`
/// triggers peer eviction from the live mTLS allowlist and closes the link —
/// exactly mirroring the desktop daemon's inbound `run_peer_connection` handling
/// and the existing outbound `sync_with_peer` path (lib.rs:1617-1636).
///
/// SECURITY: the eviction key is the mTLS-verified `peer_fingerprint` — never a
/// field inside the frame (mirrors the ControlMsg::Unpair doc comment). This
/// prevents a misbehaving peer from causing arbitrary evictions.
pub(super) async fn run_connection(
    mut framed: copypaste_p2p::transport::PeerStream,
    shared: SyncKey,
    catchup: Vec<WireItem>,
    received: Arc<Mutex<Vec<SyncedItem>>>,
    cancel: CancellationToken,
    // PG-1 (7d8x): peer identity for allowlist eviction on Unpair.
    peer_fingerprint: String,
    peer_state: Arc<Mutex<PeerState>>,
) {
    use bytes::Bytes;
    use futures_util::{SinkExt, StreamExt};

    // (1) Push the catch-up history once. A serialisation/write error just
    //     means the link is gone — stop, don't panic.
    for item in &catchup {
        match serde_json::to_vec(item) {
            Ok(payload) => {
                if framed.send(Bytes::from(payload)).await.is_err() {
                    return;
                }
            }
            Err(_) => return,
        }
    }

    // (2) Persistent read loop — no idle/deadline cutoff. Keep the link open
    //     like the daemon's `run_peer_connection`; exit only on peer EOF/error,
    //     listener shutdown, or an inbound Unpair control frame.
    loop {
        tokio::select! {
            frame = framed.next() => {
                match frame {
                    Some(Ok(bytes)) => {
                        // PG-1 (7d8x): parse as PeerFrame (untagged: Data first,
                        // then Control) — identical to the outbound read loop in
                        // sync_with_peer (lib.rs:1617). A parse failure is
                        // non-fatal (log-and-continue, same as the daemon).
                        match serde_json::from_slice::<PeerFrame>(&bytes) {
                            Ok(PeerFrame::Data(wire)) => {
                                if let Some(item) = decrypt_wire_item(&wire, &shared) {
                                    // Lock held only to push; never across an await.
                                    if let Ok(mut buf) = received.lock() {
                                        buf.push(item);
                                    }
                                }
                            }
                            Ok(PeerFrame::Control(ControlMsg::Unpair)) => {
                                // PG-1 (7d8x): the mTLS-authenticated peer has
                                // unilaterally unpaired. Evict it from the live
                                // allowlist immediately (defence-in-depth: the next
                                // handshake from this fingerprint is refused at TLS)
                                // and close this connection. The eviction key is the
                                // verified cert fingerprint — never a frame field.
                                //
                                // SECURITY: std::Mutex held only to mutate, never
                                // across an await — safe inside this async fn.
                                if let Ok(mut state) = peer_state.lock() {
                                    state.peers.remove(&peer_fingerprint);
                                    state.allowed.retain(|fp| fp != &peer_fingerprint);
                                    // Also add to denylist so the re-check at
                                    // accept (accept_loop) refuses a reconnect.
                                    if !state.revoked.iter().any(|r| r == &peer_fingerprint) {
                                        state.revoked.push(peer_fingerprint.clone());
                                    }
                                }
                                tracing::debug!(
                                    peer = %peer_fingerprint,
                                    "copypaste-android p2p_listener: inbound Unpair from \
                                     mTLS-verified peer — evicted from allowlist, closing link"
                                );
                                return;
                            }
                            Ok(PeerFrame::Control(_)) => {
                                // Other control frames (Ping/Pong) are not handled
                                // in the inbound listener yet — ignore and keep
                                // reading (matches sync_with_peer:1629-1634).
                            }
                            Err(_) => {
                                // Unparseable frame: skip, not fatal (mirrors daemon).
                            }
                        }
                    }
                    // Frame-level error or clean EOF: peer dropped the link.
                    Some(Err(_)) | None => return,
                }
            }
            _ = cancel.cancelled() => return,
        }
    }
}
