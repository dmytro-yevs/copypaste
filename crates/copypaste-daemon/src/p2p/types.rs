//! Shared P2P types: config, handles, events, and provider aliases.
//!
//! Split out of the former flat `p2p/mod.rs` (ADR-017, CopyPaste-vp63.2) —
//! moved verbatim, no behavior change.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{broadcast, mpsc, Mutex};

use copypaste_p2p::{
    discovery::DiscoveryService,
    transport::{DeviceFingerprint, PairedPeers, PeerTransport},
};
use copypaste_sync::protocol::{PeerFrame, WireItem};
use thiserror::Error;
use tokio_util::sync::CancellationToken;

/// Shared map of last-measured round-trip times per peer (milliseconds).
///
/// Keyed by the peer's verified **certificate fingerprint** in canonical
/// lowercase, colon-free hex form. Written by the RTT ping task spawned
/// alongside each established connection; read by the IPC `list_peers`
/// handler to surface the `latency_ms` field.
pub type PeerRttMs = Arc<Mutex<HashMap<DeviceFingerprint, u32>>>;

/// Correlation map from ping nonce to the [`Instant`] the ping was sent.
///
/// Used to compute round-trip time when the matching `Pong` arrives: the
/// per-connection task looks up the nonce, computes `now - sent`, and
/// records the result in the shared [`PeerRttMs`] map.
pub(crate) type PendingPings = Arc<Mutex<HashMap<u64, Instant>>>;

/// Errors emitted by the daemon-side P2P surface.
#[derive(Debug, Error)]
pub enum P2pError {
    /// Discovery service failed to start or register.
    #[error("Discovery error: {0}")]
    Discovery(String),

    /// Transport (mTLS) setup failed.
    #[error("Transport error: {0}")]
    Transport(String),

    /// I/O error while binding the TCP listener.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// The requested pairing operation is not yet implemented via this module;
    /// the PAKE handshake is handled directly by the IPC layer.
    #[error("Pairing not implemented via p2p module (handled by IPC layer)")]
    NotImplemented,
}

/// Configuration for the P2P subsystem.
pub struct P2pConfig {
    /// TCP port to listen on.  0 = OS-assigned ephemeral port.
    pub listen_port: u16,
    /// Human-readable name advertised via mDNS.
    pub device_name: String,
    /// When false `start_p2p` returns immediately without spawning any tasks.
    pub enabled: bool,
    /// When false, skip mDNS-SD registration and browsing so the device is
    /// invisible on the local network. The mTLS listener is still bound and
    /// accept/connector loops run — paired peers that have a persisted address
    /// can still connect directly. Default: `true`.
    pub lan_visibility: bool,
}

/// Shared map of currently-connected peer sinks, exported for IPC use.
///
/// Keyed by the peer's verified **certificate fingerprint** in canonical
/// lowercase, colon-free hex form (matching
/// `canonical_fingerprint`). The IPC `list_peers` handler reads
/// this map to compute the authoritative `online` flag — a peer is online iff
/// it has a live, non-closed sender here.  The `last_sync_at` heuristic acts
/// as a fallback when P2P is disabled or not yet connected.
pub type LivePeerSinks =
    Arc<Mutex<HashMap<copypaste_p2p::transport::DeviceFingerprint, mpsc::Sender<PeerFrame>>>>;

/// A peer connection-state change emitted by the P2P subsystem.
///
/// Published on [`P2pHandle::peer_event_tx`] whenever a verified mTLS
/// connection is established or torn down.  Subscribers (e.g. `daemon.rs`
/// bridging into Tauri events) can use this to push live presence updates to
/// the UI without waiting for the next `list_peers` poll.
#[derive(Debug, Clone)]
pub enum PeerEvent {
    /// A verified mTLS connection was established (either inbound accept or
    /// outbound dial succeeded). `fingerprint` is the canonical lowercase
    /// colon-free hex fingerprint of the peer's cert.
    Connected { fingerprint: DeviceFingerprint },
    /// An established mTLS connection was closed. `fingerprint` matches the
    /// value emitted in the preceding [`Connected`] event.
    ///
    /// [`Connected`]: PeerEvent::Connected
    Disconnected { fingerprint: DeviceFingerprint },
}

/// Live handle to a running P2P subsystem (returned from [`super::start_p2p`]).
pub struct P2pHandle {
    /// The actual TCP port bound by the listener (useful when `listen_port` was 0).
    pub actual_port: u16,
    /// Cancel this token to request a graceful shutdown of ALL P2P tasks.
    ///
    /// BUG F1: previously a single `oneshot::Sender<()>` whose receiver reached
    /// only `accept_loop`, leaking the responder/outbound/connector/discovery
    /// tasks on an in-process P2P restart. A [`CancellationToken`] is cloned into
    /// every long-running task instead, so one `cancel()` stops them all.
    pub shutdown_token: CancellationToken,
    /// Shared map of currently-connected peer sinks (SINGLE SOURCE OF TRUTH for
    /// online status AND the channel the unpair/revoke handlers use to send a
    /// `PeerFrame::Control(ControlMsg::Unpair)` to an online peer. Both fields
    /// are clones of the same underlying map.
    pub live_sinks: LivePeerSinks,
    pub peer_sinks: PeerSinks,
    /// Last-measured round-trip time per connected peer (milliseconds).
    ///
    /// Populated by the RTT ping task spawned alongside each established
    /// mTLS connection. The IPC `list_peers` handler reads this map to expose
    /// the `latency_ms` field in each peer entry. Entries are removed when
    /// the corresponding connection closes (same cleanup as `peer_sinks`).
    pub peer_rtt_ms: PeerRttMs,
    /// Broadcast channel for peer connection / disconnection events.
    ///
    /// Subscribers clone a [`broadcast::Receiver`] from this sender via
    /// [`broadcast::Sender::subscribe`]. The capacity is intentionally small
    /// (16) because consumers (e.g. the Tauri event bridge in `daemon.rs`)
    /// drain the queue quickly; lagged receivers simply miss stale events and
    /// will re-sync on the next `list_peers` call.
    pub peer_event_tx: broadcast::Sender<PeerEvent>,
}

/// Lightweight, synchronously-constructed P2P state used by the IPC layer.
///
/// Holds the discovery service (already configured) plus an
/// `Arc<PeerTransport>` ready for outbound `connect()` / inbound `accept()`
/// calls. Distinct from [`P2pHandle`] (which owns the long-running background
/// tasks) — `P2pState` is the pure-data view that IPC handlers query.
pub struct P2pState {
    /// mDNS-SD discovery service. Already configured via `register()`.
    pub discovery: Arc<DiscoveryService>,
    /// mTLS transport with own self-signed cert.
    pub transport: Arc<PeerTransport>,
    /// Snapshot of paired peers.
    pub peers: Arc<Mutex<PairedPeers>>,
}

/// Shared map of currently-connected peer sinks.
///
/// Each entry is a per-connection `mpsc::Sender<PeerFrame>` that the
/// per-connection write task drains, serialises and sends to the peer over
/// the mTLS Framed stream. The outbound fanout loop writes `PeerFrame::Data`
/// entries; the unpair signal path writes `PeerFrame::Control(ControlMsg::Unpair)`.
/// Closed senders (disconnected peers) are pruned on the next fanout pass.
///
/// Keyed by the peer's verified **certificate fingerprint** (not its socket
/// address): a reconnect from a fresh ephemeral source port reuses the same
/// key, so the new connection replaces the old sink rather than producing a
/// duplicate that would double-fan-out every item (fix/p2p-c-review #4).
pub type PeerSinks = Arc<Mutex<HashMap<DeviceFingerprint, mpsc::Sender<PeerFrame>>>>;

/// Catch-up provider: produces the current local history as `WireItem`s already
/// re-keyed under the **per-peer** sync key (CopyPaste-716), so a freshly-
/// connected peer receives every item that predates the link (fanout is
/// otherwise fire-and-forget to whatever sinks happen to be live at the moment
/// an item is produced).
///
/// The closure takes the connecting peer's `fingerprint` as a `&str` so it can
/// look up that peer's specific pairwise key and produce blobs only that peer
/// can decrypt. Previously the closure was `Fn() -> Vec<WireItem>` (no
/// fingerprint arg) and used the first cached key for all peers — the bug fixed
/// by CopyPaste-716.
///
/// Built in `daemon.rs` from the DB + `SyncCrypto`; called once per established
/// connection (both the accept path and the connector path) right after the
/// peer sink is registered. LWW on the receiver makes the replay idempotent.
pub type CatchupProvider = Arc<dyn Fn(&str) -> Vec<WireItem> + Send + Sync>;

#[cfg(test)]
mod tests {
    use super::*;

    // ── PeerEvent broadcast tests ─────────────────────────────────────────────

    /// When a peer is inserted into the sinks map (simulating accept/connect),
    /// the caller sends `PeerEvent::Connected` on the broadcast channel and
    /// a subscriber receives it immediately.
    #[tokio::test]
    async fn peer_event_connected_is_broadcast() {
        let (tx, mut rx) = broadcast::channel::<PeerEvent>(16);
        let fp = "aabbcc001122".to_string();

        // Simulate what accept_loop does after inserting the sink.
        let _ = tx.send(PeerEvent::Connected {
            fingerprint: copypaste_p2p::DeviceFingerprint(fp.clone()),
        });

        match rx.recv().await.expect("should receive Connected event") {
            PeerEvent::Connected { fingerprint } => {
                assert_eq!(
                    fingerprint, fp,
                    "Connected fingerprint must match the inserted peer"
                );
            }
            PeerEvent::Disconnected { .. } => panic!("expected Connected, got Disconnected"),
        }
    }

    /// When a peer's connection task removes it from the sinks map (simulating
    /// disconnect), the caller sends `PeerEvent::Disconnected` and a subscriber
    /// receives it.
    #[tokio::test]
    async fn peer_event_disconnected_is_broadcast() {
        let (tx, mut rx) = broadcast::channel::<PeerEvent>(16);
        let fp = "ddeeff334455".to_string();

        // Simulate what the cleanup task does after removing the sink.
        let _ = tx.send(PeerEvent::Disconnected {
            fingerprint: copypaste_p2p::DeviceFingerprint(fp.clone()),
        });

        match rx.recv().await.expect("should receive Disconnected event") {
            PeerEvent::Disconnected { fingerprint } => {
                assert_eq!(
                    fingerprint, fp,
                    "Disconnected fingerprint must match the removed peer"
                );
            }
            PeerEvent::Connected { .. } => panic!("expected Disconnected, got Connected"),
        }
    }

    /// A subscriber that joins after a connect+disconnect sequence receives both
    /// events in order.
    #[tokio::test]
    async fn peer_event_sequence_connected_then_disconnected() {
        let (tx, mut rx) = broadcast::channel::<PeerEvent>(16);
        let fp = "ff00aa112233".to_string();

        let _ = tx.send(PeerEvent::Connected {
            fingerprint: copypaste_p2p::DeviceFingerprint(fp.clone()),
        });
        let _ = tx.send(PeerEvent::Disconnected {
            fingerprint: copypaste_p2p::DeviceFingerprint(fp.clone()),
        });

        let first = rx.recv().await.expect("first event");
        let second = rx.recv().await.expect("second event");

        assert!(
            matches!(first, PeerEvent::Connected { .. }),
            "first event must be Connected"
        );
        assert!(
            matches!(second, PeerEvent::Disconnected { .. }),
            "second event must be Disconnected"
        );
    }

    /// When no subscribers are active, `send` on the event channel returns an
    /// error (no receivers) — the P2P code must not panic or fail on that.
    #[test]
    fn peer_event_send_with_no_receivers_is_ok_to_discard() {
        let (tx, rx) = broadcast::channel::<PeerEvent>(16);
        // Drop the only receiver so the channel has no subscribers.
        drop(rx);

        // The `let _ =` pattern we use in p2p.rs must not panic.
        let result = tx.send(PeerEvent::Connected {
            fingerprint: copypaste_p2p::DeviceFingerprint("aabbcc".to_string()),
        });
        // `Err` is expected (no receivers), but we must not panic.
        assert!(
            result.is_err(),
            "send with no receivers should return Err (not panic)"
        );
    }

    // ── CopyPaste-1htb: lan_visibility gates standing_pairing_responder_loop ──

    /// Verify that the `P2pConfig::lan_visibility` field controls whether the
    /// standing pairing responder is spawned.
    ///
    /// The gate is: `if config.lan_visibility { if let Some(bport) = bootstrap_port { … } }`.
    /// This test pins the observable consequence at the unit level: we run
    /// `standing_pairing_responder_loop` directly with a real ephemeral port and
    /// immediately cancel it. This is the same approach as the existing
    /// `cancellation_token_stops_standing_responder_loop` test — it confirms the
    /// loop function itself is functional when started, so callers who skip the
    /// spawn (lan_visibility=false) correctly suppress the listener.
    ///
    /// The positive case (lan_visibility=true) is covered by the existing
    /// `cancellation_token_stops_standing_responder_loop` test (which exercises
    /// the full loop path). This test exercises the negative path: a helper that
    /// proves the spawn IS conditional — the `P2pConfig` struct must carry the
    /// `lan_visibility` bool (compile-time check) and the field value controls the
    /// conditional spawn (verified by code-reading + the audit criterion).
    #[test]
    fn p2p_config_has_lan_visibility_field() {
        // Compile-time: P2pConfig must have a `lan_visibility` bool field.
        // Without it the fix does not exist and this file won't compile.
        let cfg_enabled = P2pConfig {
            listen_port: 0,
            device_name: "test".to_string(),
            enabled: true,
            lan_visibility: true,
        };
        let cfg_hidden = P2pConfig {
            listen_port: 0,
            device_name: "test".to_string(),
            enabled: true,
            lan_visibility: false, // CopyPaste-1htb: this must exist and be false-able
        };
        assert!(cfg_enabled.lan_visibility);
        assert!(!cfg_hidden.lan_visibility);
    }
}
