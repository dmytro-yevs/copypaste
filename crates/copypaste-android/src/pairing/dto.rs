//! FFI DTOs surfaced across UniFFI — `DiscoveredPeer` and `PairStatus`.
//!
//! # FFI-BOUNDARY FLAG
//! Both types are UDL `dictionary`s (`copypaste_android.udl`), re-exported at
//! `lib.rs`. Their FIELD names/types/order are FROZEN — do NOT rename/reorder/
//! retype any field.

use copypaste_p2p::discovery::PeerInfo;

use crate::SyncProvisioning;

use super::state::PairingState;

/// A peer discovered on the local network (FFI mirror of
/// [`copypaste_p2p::discovery::PeerInfo`] plus a `paired` flag the caller
/// computes by cross-referencing its persisted fingerprints).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// The peer's device id (hex cert fingerprint advertised as `did`).
    pub device_id: String,
    /// Human-readable device name.
    pub device_name: String,
    /// All resolved IP addresses for the peer (string form).
    pub ip_addrs: Vec<String>,
    /// TCP port of the peer's P2P sync listener.
    pub port: u16,
    /// TCP port of the peer's PAKE bootstrap listener (v2 peers only). `None`
    /// for v1 peers — the UI must disable "Pair" because the bootstrap handshake
    /// cannot be initiated without it.
    pub bport: Option<u16>,
    /// `true` when this peer's fingerprint is already in the caller's paired set.
    pub paired: bool,
}

impl DiscoveredPeer {
    /// Build from a [`PeerInfo`] snapshot, computing `paired` against a
    /// canonical-fingerprint set of already-paired device ids.
    pub fn from_peer_info(peer: PeerInfo, paired: bool) -> Self {
        DiscoveredPeer {
            device_id: peer.device_id,
            device_name: peer.device_name,
            ip_addrs: peer.ip_addrs.iter().map(|ip| ip.to_string()).collect(),
            port: peer.port,
            bport: peer.bport,
            paired,
        }
    }
}

/// Polled pairing status surfaced to Kotlin by [`crate::pair_get_sas`].
///
/// `state` is the lowercase machine token (`idle` / `initiating` /
/// `awaiting_sas` / `confirmed` / `rejected` / `aborted` / `timed_out`). `sas`
/// and `role` are populated while active; the `peer_*` outputs are populated
/// ONLY when `state == "confirmed"`.
///
/// # SECURITY NOTE — `session_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after deriving the content sync key (KEK-wrap into AndroidKeystore);
/// never log it.
#[derive(Debug, Clone, Default)]
pub struct PairStatus {
    pub state: String,
    pub sas: Option<String>,
    pub role: Option<String>,
    pub peer_fingerprint: Option<String>,
    pub peer_sync_addr: Option<String>,
    pub session_key: Option<Vec<u8>>,
    pub peer_provisioning: Option<SyncProvisioning>,
    /// HB-1b (ABI 14): the PEER's device metadata, set ONLY in the `confirmed`
    /// state (copied from [`super::state::ConfirmedPairing`]). Kotlin persists
    /// these on the `PairedPeer` for the Wave-3 device card.
    pub peer_model: Option<String>,
    pub peer_os: Option<String>,
    pub peer_app_version: Option<String>,
    pub peer_local_ip: Option<String>,
    pub peer_public_ip: Option<String>,
    /// ABI 17 (CopyPaste-3k6m): the PEER's stable device UUID, set ONLY in the
    /// `confirmed` state (copied from [`super::state::ConfirmedPairing`]).
    pub peer_device_id: Option<String>,
    /// ABI 19 (CopyPaste-gldr): the PEER's non-secret Supabase/cloud account
    /// id, set ONLY in the `confirmed` state (copied from
    /// [`super::state::ConfirmedPairing`]). `None` for legacy peers or peers
    /// with no cloud account configured.
    pub peer_supabase_account_id: Option<String>,
}

impl PairStatus {
    /// Build from a [`PairingState`] snapshot. `peer_*` fields are only set in
    /// the `Confirmed` terminal state.
    pub fn from_state(state: &PairingState) -> Self {
        let mut status = PairStatus {
            state: state.as_str().to_string(),
            sas: state.sas().map(|s| s.to_string()),
            role: state.role().map(|r| r.as_str().to_string()),
            ..PairStatus::default()
        };
        if let Some(c) = state.confirmed() {
            status.peer_fingerprint = Some(c.peer_fingerprint.clone());
            status.peer_sync_addr = Some(c.peer_sync_addr.clone());
            status.session_key = Some(c.session_key.clone());
            status.peer_provisioning = c.peer_provisioning.clone();
            // HB-1b: carry the peer's device metadata through to Kotlin.
            status.peer_model = c.peer_model.clone();
            status.peer_os = c.peer_os.clone();
            status.peer_app_version = c.peer_app_version.clone();
            status.peer_local_ip = c.peer_local_ip.clone();
            status.peer_public_ip = c.peer_public_ip.clone();
            status.peer_device_id = c.peer_device_id.clone();
            status.peer_supabase_account_id = c.peer_supabase_account_id.clone();
        }
        status
    }
}
