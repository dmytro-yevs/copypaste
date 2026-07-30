//! The five peer-to-peer IPC operations.
//!
//! Each is [`copypaste_p2p::Node`] plus the two things only the daemon knows:
//! which `ErrorCode` a failure carries, and what else has to be told about it —
//! the event stream, the device-name registry, the cloud cursor. The node holds
//! the sentences, because what a user reads must not depend on which client
//! asked.
//!
//! These are the only handlers in the daemon that do network I/O, which is why
//! they are `async` while every store handler in `server` is a blocking call on
//! a worker thread.
//!
//! **The pairing code is a secret.** `pair_create` returns it once, in the
//! response, and nothing here logs it — not at trace, not in an error, not in a
//! `Debug`. `NewPairing`'s own `Debug` is redacted, and the pairing *id* is
//! derived, non-secret and safe to log; it is what every message uses instead.

use std::sync::Arc;

use copypaste_ipc::{
    DiscoveredData, DiscoveredDevice, ErrorCode, PairingData, PeerInfo, Response, ResponseData,
    SyncResult,
};
use copypaste_p2p::peers::Peer;
use copypaste_p2p::NodeError;
use tracing::info;

use crate::sync::peer_source;
use crate::AppState;

/// Mint a pairing and hand back the code to read out to the other device.
pub async fn pair_create(state: &Arc<AppState>, id: u64, name: &str) -> Response {
    let pairing = match state.p2p.node().pair_create(name) {
        Ok(pairing) => pairing,
        Err(e) => return failed(id, e),
    };

    state.note_peers_changed();
    info!(pairing_id = %pairing.pairing_id, "minted a pairing");
    Response::ok(
        id,
        ResponseData::Pairing(PairingData {
            // The one and only rendering of the secret, straight into the
            // response. Not logged, not stored, not retrievable again.
            code: pairing.code,
            pairing_id: pairing.pairing_id,
            listen_addr: pairing.listen_addr,
        }),
    )
}

/// Consume a code from the other device and prove the pairing works.
///
/// The peer is persisted only after a complete session — the node's rule, and
/// the reason a failed pairing leaves the paired-device list untouched.
pub async fn pair_accept(state: &Arc<AppState>, id: u64, code: &str, addr: &str) -> Response {
    let source = peer_source(state);
    let accepted = match state.p2p.node().pair_accept(code, addr, &source).await {
        Ok(accepted) => accepted,
        Err(e) => return failed(id, e),
    };

    crate::p2p::remember_device(state, &accepted.outcome);
    let online = state.p2p.find(&accepted.peer.pairing_id).is_some();
    let info = peer_info(&accepted.peer, online);

    state.note_peers_changed();
    if accepted.outcome.stats.received > 0 {
        state.note_remote_change();
    }
    Response::ok(id, ResponseData::Peers(vec![info]))
}

/// Forget a peer.
///
/// Local and unilateral: the other device keeps its half until it also unpairs,
/// which is why this cannot fail on an unreachable peer.
pub async fn unpair(state: &Arc<AppState>, id: u64, pairing_id: &str) -> Response {
    match state.p2p.node().unpair(pairing_id) {
        Ok(true) => {
            state.note_peers_changed();
            info!(%pairing_id, "unpaired a device");
            Response::ok(id, ResponseData::Empty {})
        }
        Ok(false) => failed(id, NodeError::NoPeer),
        Err(e) => failed(id, e),
    }
}

/// Known peers, with a best-effort liveness flag from discovery.
///
/// `online: false` means "not seen on the network", never "unreachable" — a
/// peer on a network without multicast, or on a different subnet, is reachable
/// by address and still reads as offline.
pub async fn peers(state: &Arc<AppState>, id: u64) -> Response {
    let infos = state
        .p2p
        .peers()
        .list()
        .iter()
        .map(|peer| {
            let online = state.p2p.find(&peer.pairing_id).is_some();
            peer_info(peer, online)
        })
        .collect();
    Response::ok(id, ResponseData::Peers(infos))
}

/// Sync with one peer, or with every known peer.
///
/// One peer failing must not abort the rest: each gets its own [`SyncResult`],
/// and a failure is reported in that peer's `error` field while the run
/// continues. The whole request only fails when a named peer does not exist.
pub async fn sync_now(state: &Arc<AppState>, id: u64, pairing_id: Option<&str>) -> Response {
    let targets = match pairing_id {
        Some(pairing_id) => match state.p2p.peers().get(pairing_id) {
            Some(peer) => vec![peer],
            None => return failed(id, NodeError::NoPeer),
        },
        None => state.p2p.peers().list(),
    };

    let mut results = Vec::with_capacity(targets.len());
    for peer in &targets {
        results.push(sync_one(state, peer).await);
    }
    if results.iter().any(|result| result.received > 0) {
        state.note_remote_change();
    }
    Response::ok(id, ResponseData::Sync(results))
}

/// LAN devices, paired or not.
///
/// Never an error, and an empty list is a normal answer: discovery may be
/// switched off, or the network may filter multicast. A client must offer
/// "enter an address" as an equal path rather than as a fallback (manifest 04
/// §4.12).
pub async fn discovered(state: &Arc<AppState>, id: u64) -> Response {
    Response::ok(id, ResponseData::Discovered(devices(state)))
}

/// Re-advertise and answer as [`discovered`] does.
///
/// Best-effort, exactly like every other discovery call: a republish that fails
/// is logged and the current table is still returned, because what the user
/// asked for was "show me what is out there".
pub async fn rescan(state: &Arc<AppState>, id: u64) -> Response {
    state.p2p.republish();
    Response::ok(id, ResponseData::Discovered(devices(state)))
}

fn devices(state: &Arc<AppState>) -> DiscoveredData {
    let devices = state
        .p2p
        .seen()
        .into_iter()
        .map(|found| DiscoveredDevice {
            // Resolved locally rather than taken from the advertisement:
            // anyone on the LAN can claim any pairing id, and `paired` decides
            // whether the UI offers "pair" or "sync" (`CopyPaste-vgpy`).
            paired: state.p2p.peers().get(&found.pairing_id).is_some(),
            addr: found.addr.to_string(),
            pairing_id: found.pairing_id,
            name: found.name,
            last_seen_ms: found.last_seen_ms,
        })
        .collect();
    DiscoveredData { devices }
}

/// One peer, start to finish. Never returns `Err`: a failure is a field.
pub(super) async fn sync_one(state: &Arc<AppState>, peer: &Peer) -> SyncResult {
    let source = peer_source(state);
    match state.p2p.node().sync_one(peer, &source).await {
        Ok(outcome) => {
            crate::p2p::remember_device(state, &outcome);
            SyncResult {
                pairing_id: peer.pairing_id.clone(),
                name: outcome.peer_device_name,
                sent: u32::try_from(outcome.stats.sent).unwrap_or(u32::MAX),
                received: u32::try_from(outcome.stats.received).unwrap_or(u32::MAX),
                error: None,
            }
        }
        Err(e) => SyncResult {
            pairing_id: peer.pairing_id.clone(),
            name: peer.name.clone(),
            sent: 0,
            received: 0,
            error: Some(e.to_string()),
        },
    }
}

/// The one mapping from the node's failures onto the IPC taxonomy.
fn failed(id: u64, error: NodeError) -> Response {
    let code = match error {
        NodeError::NoPeer => ErrorCode::NotFound,
        _ if error.is_client_error() => ErrorCode::InvalidRequest,
        _ => ErrorCode::Internal,
    };
    Response::err(id, code, error.to_string())
}

fn peer_info(peer: &Peer, online: bool) -> PeerInfo {
    PeerInfo {
        pairing_id: peer.pairing_id.clone(),
        name: peer.name.clone(),
        last_addr: peer.last_addr.map(|addr| addr.to_string()),
        last_seen_ms: peer.last_seen_ms,
        online,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_p2p::transport::{PairingToken, TOKEN_LEN};

    #[tokio::test]
    async fn creating_a_pairing_returns_a_code_and_stores_the_peer() {
        let (state, _dir) = test_state("alpha");
        let response = pair_create(&state, 1, "laptop").await;
        assert!(response.ok);

        let pairing = match response.data {
            Some(ResponseData::Pairing(data)) => data,
            other => panic!("expected pairing data, got {other:?}"),
        };
        assert!(!pairing.code.is_empty());
        assert_ne!(pairing.code, pairing.pairing_id);

        // The code must reconstruct the stored key, or the other device could
        // never authenticate.
        let token = PairingToken::parse(&pairing.code).expect("a valid code");
        assert_eq!(token.pairing_id(), pairing.pairing_id);
        let stored = state
            .p2p
            .peers()
            .get(&pairing.pairing_id)
            .expect("the peer is stored before the other device dials in");
        assert!(stored.psk_matches(&token.psk()));
        assert_eq!(stored.name, "laptop");
    }

    #[tokio::test]
    async fn two_pairings_are_independent() {
        let (state, _dir) = test_state("alpha");
        let first = pair_create(&state, 1, "a").await;
        let second = pair_create(&state, 2, "b").await;
        let ids: Vec<String> = [first, second]
            .into_iter()
            .map(|r| match r.data {
                Some(ResponseData::Pairing(data)) => data.pairing_id,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_ne!(ids[0], ids[1]);
        assert_eq!(state.p2p.peers().len(), 2);
    }

    #[tokio::test]
    async fn a_malformed_code_is_rejected_without_touching_the_peer_list() {
        let (state, _dir) = test_state("alpha");
        let response = pair_accept(&state, 1, "not-a-code", "127.0.0.1:1").await;
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(
            response.error.as_deref(),
            Some(NodeError::BadCode.to_string().as_str())
        );
        assert_eq!(state.p2p.peers().len(), 0);
    }

    /// A well-formed code for a device that is not listening must not leave a
    /// half-made pairing behind.
    #[tokio::test]
    async fn a_failed_handshake_stores_nothing() {
        let (state, _dir) = test_state("alpha");
        let code = PairingToken::generate().to_code();
        // Port 1 on loopback: nothing is listening, so the connect fails fast.
        let response = pair_accept(&state, 1, &code, "127.0.0.1:1").await;
        assert!(!response.ok);
        assert_eq!(state.p2p.peers().len(), 0, "a pairing was persisted anyway");
    }

    #[tokio::test]
    async fn an_unresolvable_address_is_a_client_error() {
        let (state, _dir) = test_state("alpha");
        let code = PairingToken::generate().to_code();
        let response = pair_accept(&state, 1, &code, "no-port-here").await;
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(
            response.error.as_deref(),
            Some(NodeError::BadAddress.to_string().as_str())
        );
    }

    #[tokio::test]
    async fn unpairing_an_unknown_device_is_not_found() {
        let (state, _dir) = test_state("alpha");
        let response = unpair(&state, 1, "0123456789abcdef0123456789abcdef").await;
        assert_eq!(response.error_code, Some(ErrorCode::NotFound));
    }

    #[tokio::test]
    async fn unpairing_removes_the_key_from_the_listeners_candidates() {
        let (state, _dir) = test_state("alpha");
        let pairing = match pair_create(&state, 1, "laptop").await.data {
            Some(ResponseData::Pairing(data)) => data,
            other => panic!("{other:?}"),
        };
        assert_eq!(state.p2p.peers().psks().len(), 1);

        let response = unpair(&state, 2, &pairing.pairing_id).await;
        assert!(response.ok);
        assert!(state.p2p.peers().psks().is_empty());
    }

    #[tokio::test]
    async fn peers_lists_what_was_paired() {
        let (state, _dir) = test_state("alpha");
        pair_create(&state, 1, "laptop").await;

        let response = peers(&state, 2).await;
        let listed = match response.data {
            Some(ResponseData::Peers(peers)) => peers,
            other => panic!("{other:?}"),
        };
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "laptop");
        assert!(listed[0].last_addr.is_none());
        // Nothing has been seen on the network in a unit test.
        assert!(!listed[0].online);
    }

    #[tokio::test]
    async fn syncing_a_named_peer_that_does_not_exist_is_not_found() {
        let (state, _dir) = test_state("alpha");
        let response = sync_now(&state, 1, Some("0123456789abcdef")).await;
        assert_eq!(response.error_code, Some(ErrorCode::NotFound));
    }

    #[tokio::test]
    async fn syncing_with_no_peers_reports_an_empty_run() {
        let (state, _dir) = test_state("alpha");
        let response = sync_now(&state, 1, None).await;
        assert!(response.ok);
        match response.data {
            Some(ResponseData::Sync(results)) => assert!(results.is_empty()),
            other => panic!("{other:?}"),
        }
    }

    /// A peer that has never been reached and is not on the network is reported
    /// per-peer, and the run still succeeds.
    #[tokio::test]
    async fn a_peer_with_no_known_address_fails_only_itself() {
        let (state, _dir) = test_state("alpha");
        pair_create(&state, 1, "unreachable").await;
        state
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: "ffffffffffffffffffffffffffffffff".into(),
                name: "second".into(),
                psk: [3u8; TOKEN_LEN],
                last_addr: None,
                last_seen_ms: 0,
            })
            .unwrap();

        let response = sync_now(&state, 2, None).await;
        assert!(response.ok, "one unreachable peer must not fail the run");
        let results = match response.data {
            Some(ResponseData::Sync(results)) => results,
            other => panic!("{other:?}"),
        };
        assert_eq!(results.len(), 2);
        assert!(results.iter().all(|r| r.error.is_some()));
    }

    /// The node owns the sentences; this pins the taxonomy they arrive under,
    /// and that none of them picks up a path on the way through.
    #[test]
    fn node_failures_map_onto_the_ipc_taxonomy_without_a_path() {
        for (error, expected) in [
            (NodeError::BadCode, ErrorCode::InvalidRequest),
            (NodeError::BadAddress, ErrorCode::InvalidRequest),
            (NodeError::Handshake, ErrorCode::InvalidRequest),
            (NodeError::NoPeer, ErrorCode::NotFound),
            (NodeError::NoAddress, ErrorCode::Internal),
            (NodeError::Session, ErrorCode::Internal),
            (NodeError::Timeout, ErrorCode::Internal),
            (NodeError::PeerStore, ErrorCode::Internal),
        ] {
            let response = failed(1, error);
            assert_eq!(response.error_code, Some(expected), "{error:?}");
            let message = response.error.expect("a message");
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }
}
