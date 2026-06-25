//! Pairing, LAN/SAS discovery, and peer-management METHOD_* constants.

// ── Pairing ─────────────────────────────────────────────────────────────────

/// Generate a short-lived QR pairing payload.
pub const METHOD_PAIR_GENERATE_QR: &str = "pair_generate_qr";

// ── LAN/SAS discovery ────────────────────────────────────────────────────────

/// Return the list of peers currently visible via mDNS-SD, cross-referenced
/// against `peers.json` to mark each as paired or not.
///
/// Response shape: `{ devices: [{ device_id, device_name, ip_addrs, port,
/// bport, paired }] }`.  `paired` is `true` when the device's canonical
/// fingerprint matches an entry in `peers.json`.  `bport` is the bootstrap
/// port for SAS pairing (null on v1 peers); the UI should disable "Pair" when
/// `bport` is null.
pub const METHOD_LIST_DISCOVERED: &str = "list_discovered";

// ── LAN/SAS discovery-initiated pairing (Phase 2) ─────────────────────────────

/// Begin a discovery-initiated SAS pairing as the INITIATOR.
///
/// Takes `{ device_id }` (the discovered peer's mDNS `did`). The daemon resolves
/// the peer's bootstrap address (`bport`), generates an ephemeral random PAKE
/// password, runs the bootstrap handshake, and (on reaching the SAS step)
/// transitions the pairing state machine to `awaiting_sas`. The UI then polls
/// [`METHOD_PAIR_GET_SAS`] and calls [`METHOD_PAIR_CONFIRM_SAS`].
pub const METHOD_PAIR_WITH_DISCOVERED: &str = "pair_with_discovered";

/// Poll the discovery-pairing state machine.
///
/// Response: `{ state, sas?, role? }` where `state` is one of `idle`,
/// `initiating`, `awaiting_sas`, `confirmed`, `rejected`, `aborted`,
/// `timed_out`. `sas` (6 decimal digits) and `role` (`initiator`/`responder`)
/// are present only in `awaiting_sas`.
pub const METHOD_PAIR_GET_SAS: &str = "pair_get_sas";

/// Deliver the local user's SAS accept/reject decision.
///
/// Takes `{ accept: bool }`. Fires the in-flight handshake's confirmation
/// channel; the pairing succeeds (keys trusted + persisted) only when BOTH sides
/// accept. On reject the keys are dropped/zeroized and nothing is persisted.
pub const METHOD_PAIR_CONFIRM_SAS: &str = "pair_confirm_sas";

/// Abort an in-flight discovery pairing and reset the state machine to `idle`.
pub const METHOD_PAIR_ABORT: &str = "pair_abort";

/// Pair with a peer using a shared password (non-QR / non-SAS path).
///
/// Params: `{ peer_fingerprint: String, password: String }`.  Used when the
/// other device provides a fixed password instead of a QR / SAS code.
pub const METHOD_PAIR_PEER_WITH_PASSWORD: &str = "pair_peer_with_password";

// ── Peer management ──────────────────────────────────────────────────────────

/// Remove a paired peer (untrust, delete from `peers.json`, no key rotation).
///
/// Params: `{ fingerprint: String }`.  The peer is removed from the local trust
/// store; items it synced remain in history.  Use [`METHOD_REVOKE_PEER`] for a
/// stronger revoke that also logs the revocation timestamp.
pub const METHOD_UNPAIR_PEER: &str = "unpair_peer";

/// Revoke a paired peer with a logged revocation timestamp.
///
/// Params: `{ fingerprint: String }`.  More forceful than unpair: the peer's
/// entry is removed AND a `revoked_at` timestamp is persisted.
/// Returns `{ revoked_at: i64 }`.
pub const METHOD_REVOKE_PEER: &str = "revoke_peer";

/// Revoke ALL paired peers in one call.
///
/// Returns `{ revoked: u32 }` — the number of peers removed.
pub const METHOD_REVOKE_ALL_PEERS: &str = "revoke_all_peers";

/// List all paired devices.
///
/// Returns `{ peers: [PairedDevice] }` including online/offline status,
/// last-seen, latency, and sync timestamps.
pub const METHOD_LIST_PEERS: &str = "list_peers";

/// Reorder the pinned-item display sequence.
///
/// Params: `{ ids: [String] }` — complete ordered list of pinned item IDs.
/// The daemon stores the order and returns items sorted by it in subsequent
/// `history_page` responses.
pub const METHOD_REORDER_PINNED: &str = "reorder_pinned";

/// Drain all pending peer connect/disconnect events since the last call.
///
/// Returns `{ events: [{ kind: "connected" | "disconnected", fingerprint: String }] }`.
/// Used by the app-global peer-presence polling loop; individual UI components
/// subscribe to the derived presence store rather than calling this directly.
pub const METHOD_POLL_PEER_EVENTS: &str = "poll_peer_events";

/// Force an mDNS-SD rescan (restart-in-place re-browse) and return the
/// fresh discovered device list.  Same response shape as [`METHOD_LIST_DISCOVERED`].
pub const METHOD_RESCAN_DISCOVERED: &str = "rescan_discovered";

/// Transport kind reported per-peer in the `list_peers` response.
///
/// Indicates which sync transport was used for the most recent interaction with
/// this peer. Serialised as lowercase string on the wire (`"p2p"`, `"relay"`,
/// `"supabase"`) so the UI can render a 3-way chip. `None` / absent means the
/// transport is unknown (peer paired but never successfully synced, or pre-field
/// daemon).
///
/// Back-compat: the field is `#[serde(skip_serializing_if = "Option::is_none")]`
/// at the call site so older daemons that don't emit it produce JSON that
/// deserialises to `transport: None` on the UI side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PeerTransport {
    /// Synced via direct mTLS P2P connection.
    P2p,
    /// Synced via the HTTP relay (store-and-forward inbox).
    Relay,
    /// Synced via Supabase cloud backend.
    Supabase,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_list_discovered_has_correct_wire_name() {
        assert_eq!(METHOD_LIST_DISCOVERED, "list_discovered");
    }

    #[test]
    fn discovery_pairing_methods_have_correct_wire_names() {
        assert_eq!(METHOD_PAIR_WITH_DISCOVERED, "pair_with_discovered");
        assert_eq!(METHOD_PAIR_GET_SAS, "pair_get_sas");
        assert_eq!(METHOD_PAIR_CONFIRM_SAS, "pair_confirm_sas");
        assert_eq!(METHOD_PAIR_ABORT, "pair_abort");
    }

    #[test]
    fn pg62_peer_management_methods_have_correct_wire_names() {
        assert_eq!(METHOD_LIST_PEERS, "list_peers");
        assert_eq!(METHOD_POLL_PEER_EVENTS, "poll_peer_events");
        assert_eq!(METHOD_PAIR_PEER_WITH_PASSWORD, "pair_peer_with_password");
        assert_eq!(METHOD_UNPAIR_PEER, "unpair_peer");
        assert_eq!(METHOD_REVOKE_PEER, "revoke_peer");
        assert_eq!(METHOD_REVOKE_ALL_PEERS, "revoke_all_peers");
        assert_eq!(METHOD_REORDER_PINNED, "reorder_pinned");
        assert_eq!(METHOD_RESCAN_DISCOVERED, "rescan_discovered");
    }

    /// CopyPaste-1jms.32: `PeerTransport` must serialise to lowercase wire strings
    /// and round-trip through JSON (the format used by the `list_peers` response).
    #[test]
    fn peer_transport_serde_roundtrip() {
        // Serialise each variant to the expected lowercase wire string.
        assert_eq!(
            serde_json::to_string(&PeerTransport::P2p).unwrap(),
            "\"p2p\""
        );
        assert_eq!(
            serde_json::to_string(&PeerTransport::Relay).unwrap(),
            "\"relay\""
        );
        assert_eq!(
            serde_json::to_string(&PeerTransport::Supabase).unwrap(),
            "\"supabase\""
        );

        // Deserialise back from wire strings.
        assert_eq!(
            serde_json::from_str::<PeerTransport>("\"p2p\"").unwrap(),
            PeerTransport::P2p
        );
        assert_eq!(
            serde_json::from_str::<PeerTransport>("\"relay\"").unwrap(),
            PeerTransport::Relay
        );
        assert_eq!(
            serde_json::from_str::<PeerTransport>("\"supabase\"").unwrap(),
            PeerTransport::Supabase
        );
    }

    /// `PeerTransport` as `Option<PeerTransport>` serialises to `null` when `None`
    /// and is omitted only when the caller uses `skip_serializing_if = "Option::is_none"`.
    #[test]
    fn peer_transport_option_none_serialises_to_null() {
        let none: Option<PeerTransport> = None;
        assert_eq!(serde_json::to_string(&none).unwrap(), "null");
        let some = Some(PeerTransport::Relay);
        assert_eq!(serde_json::to_string(&some).unwrap(), "\"relay\"");
    }
}
