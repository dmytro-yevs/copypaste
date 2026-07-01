//! Inbound accept loop, ported from the daemon's `accept_loop` (no outbound
//! fanout — Android is receive-only).

use std::sync::{Arc, Mutex};

use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use copypaste_p2p::transport::PeerTransport;

use crate::{is_fingerprint_revoked, shared_sync_key_from_session, LocalItem, SyncedItem};

use super::codec::build_catchup_wire_items;
use super::connection::run_connection;
use super::registry::PeerState;

/// The accept loop, ported from the daemon's `accept_loop` (no outbound
/// fanout). `select!`s an accept against the cancel token; per accepted
/// connection re-checks the denylist BEFORE any catch-up, derives the per-peer
/// shared key, and spawns [`run_connection`].
pub(super) async fn accept_loop(
    listener: TcpListener,
    transport: Arc<PeerTransport>,
    peer_state: Arc<Mutex<PeerState>>,
    local_items: Arc<Vec<LocalItem>>,
    device_id: Arc<String>,
    received: Arc<Mutex<Vec<SyncedItem>>>,
    cancel: CancellationToken,
) {
    loop {
        tokio::select! {
            result = transport.accept(&listener) => {
                match result {
                    Ok((_peer_addr, peer_fp, framed)) => {
                        // ── SECURITY: re-check the denylist AT ACCEPT, before
                        //    catch-up or ANY frame. A revoked peer must never
                        //    receive the history push (inbound analog of the
                        //    dialer's revoked-peer refusal). ──
                        let (is_revoked, session_key) = {
                            // std::Mutex held briefly, never across an await.
                            let Ok(state) = peer_state.lock() else { continue };
                            let revoked = is_fingerprint_revoked(&peer_fp, &state.revoked);
                            let key = state.session_keys.get(peer_fp.as_str()).cloned();
                            (revoked, key)
                        };
                        if is_revoked {
                            // Drop the connection without sending anything.
                            drop(framed);
                            continue;
                        }

                        // Derive the per-peer shared content key from the
                        // VERIFIED peer fingerprint's session key. Without a
                        // session key we cannot decrypt/encrypt for this peer —
                        // drop the connection.
                        let Some(session_key) = session_key else {
                            drop(framed);
                            continue;
                        };
                        let shared = match shared_sync_key_from_session(&session_key) {
                            Ok(k) => k,
                            Err(_) => {
                                drop(framed);
                                continue;
                            }
                        };

                        // Build the catch-up history under THIS peer's key.
                        let catchup =
                            match build_catchup_wire_items(&local_items, &shared, &device_id) {
                                Ok(c) => c,
                                Err(_) => {
                                    drop(framed);
                                    continue;
                                }
                            };

                        let received = Arc::clone(&received);
                        let conn_cancel = cancel.clone();
                        // PG-1 (7d8x): pass peer_fingerprint + peer_state so
                        // run_connection can evict the peer on an inbound Unpair.
                        let conn_peer_state = Arc::clone(&peer_state);
                        tokio::spawn(async move {
                            run_connection(
                                framed,
                                shared,
                                catchup,
                                received,
                                conn_cancel,
                                peer_fp.into_string(),
                                conn_peer_state,
                            )
                            .await;
                        });
                    }
                    Err(_e) => {
                        // Accept/handshake error (unknown peer, TLS failure,
                        // handshake timeout). Not fatal — keep accepting.
                    }
                }
            }
            _ = cancel.cancelled() => break,
        }
    }
}
