//! The five peer-to-peer IPC operations.
//!
//! These are the only handlers in the daemon that do network I/O, which is why
//! they are `async` while every store handler in `server` is a blocking call on
//! a worker thread.
//!
//! **The pairing code is a secret.** `pair_create` returns it once, in the
//! response, and nothing here logs it — not at trace, not in an error, not in a
//! `Debug`. `PairingToken`'s own `Debug` is redacted, so the only way to render
//! one is `to_code`, and that is called exactly once below. The pairing *id* is
//! derived, non-secret and safe to log, and is what every message uses instead.
//!
//! Every client-visible string is a fixed sentence with no interpolation, for
//! the reason `server` gives: an error must never carry a filesystem path
//! (`CLAUDE.md` rule 4).

use std::net::SocketAddr;
use std::sync::Arc;

use copypaste_ipc::{ErrorCode, PairingData, PeerInfo, Response, ResponseData, SyncResult};
use copypaste_p2p::peers::Peer;
use copypaste_p2p::sync::{run_initiator, SyncOutcome};
use copypaste_p2p::transport::{PairingToken, Session};
use tracing::{debug, info, warn};

use crate::p2p::channel::{NoiseChannel, SESSION_TIMEOUT};
use crate::p2p::source::StoreSource;
use crate::AppState;

const MSG_BAD_CODE: &str = "that pairing code is not valid";
const MSG_BAD_ADDR: &str = "that address could not be resolved; expected host:port";
const MSG_HANDSHAKE: &str = "the other device did not accept this pairing code";
const MSG_NO_PEER: &str = "no such paired device";
const MSG_NO_ADDRESS: &str =
    "this peer has never been reached and is not visible on the network; sync from the other device, or re-pair with an address";
const MSG_SESSION: &str = "the sync session with the other device failed";
const MSG_TIMEOUT: &str = "the other device stopped responding";
const MSG_PEER_STORE: &str = "the paired-device list could not be updated";

/// Every client-visible string this module can produce. Asserted pathless.
#[cfg(test)]
const ALL_MESSAGES: &[&str] = &[
    MSG_BAD_CODE,
    MSG_BAD_ADDR,
    MSG_HANDSHAKE,
    MSG_NO_PEER,
    MSG_NO_ADDRESS,
    MSG_SESSION,
    MSG_TIMEOUT,
    MSG_PEER_STORE,
];

/// Mint a pairing and hand back the code to read out to the other device.
///
/// The peer is stored *now*, before the other device has ever been heard from,
/// because the pre-shared key has to be in the listener's candidate list before
/// the other half dials in. `name` is a placeholder until the first session,
/// which is where the peer's own device name arrives.
pub async fn pair_create(state: &Arc<AppState>, id: u64, name: &str) -> Response {
    let token = PairingToken::generate();
    let pairing_id = token.pairing_id();

    let peer = Peer {
        pairing_id: pairing_id.clone(),
        name: placeholder_name(name),
        psk: token.psk(),
        last_addr: None,
        last_seen_ms: 0,
    };
    if let Err(e) = state.p2p.peers().upsert(peer) {
        warn!(error = %e, "could not store a new pairing");
        return Response::err(id, ErrorCode::Internal, MSG_PEER_STORE);
    }
    state.p2p.republish();

    info!(%pairing_id, "minted a pairing");
    Response::ok(
        id,
        ResponseData::Pairing(PairingData {
            // The one and only rendering of the secret, straight into the
            // response. Not logged, not stored, not retrievable again.
            code: token.to_code(),
            pairing_id,
            listen_addr: state.p2p.listen_addr(),
        }),
    )
}

/// Consume a code from the other device and prove the pairing works.
///
/// The peer is persisted only after a complete session. A code that does not
/// parse, an address that does not answer, a handshake the other end refuses,
/// or a session that fails part-way all leave the paired-device list untouched:
/// a stored pairing means a pairing that has worked at least once.
pub async fn pair_accept(state: &Arc<AppState>, id: u64, code: &str, addr: &str) -> Response {
    let Ok(token) = PairingToken::parse(code) else {
        // No detail, and nothing logged: the input is a secret and saying *how*
        // it was wrong is a hint about what a valid one looks like.
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_CODE);
    };
    let pairing_id = token.pairing_id();

    let Some(addr) = resolve(addr).await else {
        return Response::err(id, ErrorCode::InvalidRequest, MSG_BAD_ADDR);
    };

    let session = match Session::connect(addr, &token.psk()).await {
        Ok(session) => session,
        Err(e) => {
            debug!(%pairing_id, error = %e, "pairing handshake failed");
            return Response::err(id, ErrorCode::InvalidRequest, MSG_HANDSHAKE);
        }
    };

    let outcome = match run_session(state, session).await {
        Ok(outcome) => outcome,
        Err(message) => {
            warn!(%pairing_id, "pairing session failed; not storing the peer");
            return Response::err(id, ErrorCode::Internal, message);
        }
    };

    let peer = Peer {
        pairing_id: pairing_id.clone(),
        name: placeholder_name(&outcome.peer_device_name),
        psk: token.psk(),
        last_addr: Some(addr),
        last_seen_ms: copypaste_core::now_ms(),
    };
    let info = peer_info(&peer, state.p2p.discovery().find(&pairing_id).is_some());
    if let Err(e) = state.p2p.peers().upsert(peer) {
        warn!(error = %e, "could not store a completed pairing");
        return Response::err(id, ErrorCode::Internal, MSG_PEER_STORE);
    }
    state.p2p.republish();

    info!(
        %pairing_id,
        sent = outcome.stats.sent,
        received = outcome.stats.received,
        "paired with a device"
    );
    Response::ok(id, ResponseData::Peers(vec![info]))
}

/// Forget a peer.
///
/// Local and unilateral: the other device keeps its half until it also unpairs,
/// which is why this cannot fail on an unreachable peer. What it does do is
/// remove the pre-shared key from the listener's candidate list, so that
/// pairing can no longer authenticate here.
pub async fn unpair(state: &Arc<AppState>, id: u64, pairing_id: &str) -> Response {
    match state.p2p.peers().remove(pairing_id) {
        Ok(true) => {
            state.p2p.republish();
            info!(%pairing_id, "unpaired a device");
            Response::ok(id, ResponseData::Empty {})
        }
        Ok(false) => Response::err(id, ErrorCode::NotFound, MSG_NO_PEER),
        Err(e) => {
            warn!(error = %e, "could not remove a pairing");
            Response::err(id, ErrorCode::Internal, MSG_PEER_STORE)
        }
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
            let online = state.p2p.discovery().find(&peer.pairing_id).is_some();
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
            None => return Response::err(id, ErrorCode::NotFound, MSG_NO_PEER),
        },
        None => state.p2p.peers().list(),
    };

    let mut results = Vec::with_capacity(targets.len());
    for peer in &targets {
        results.push(sync_one(state, peer).await);
    }
    Response::ok(id, ResponseData::Sync(results))
}

/// One peer, start to finish. Never returns `Err`: a failure is a field.
async fn sync_one(state: &Arc<AppState>, peer: &Peer) -> SyncResult {
    let failed = |message: &str| SyncResult {
        pairing_id: peer.pairing_id.clone(),
        name: peer.name.clone(),
        sent: 0,
        received: 0,
        error: Some(message.to_string()),
    };

    // The last address that worked, else whatever discovery has seen. Both are
    // hints: the handshake is what actually proves who is on the other end.
    let addr = peer.last_addr.or_else(|| {
        state
            .p2p
            .discovery()
            .find(&peer.pairing_id)
            .map(|found| found.addr)
    });
    let Some(addr) = addr else {
        return failed(MSG_NO_ADDRESS);
    };

    let session = match Session::connect(addr, &peer.psk).await {
        Ok(session) => session,
        Err(e) => {
            debug!(pairing_id = %peer.pairing_id, error = %e, "could not reach a peer");
            return failed(MSG_HANDSHAKE);
        }
    };

    match run_session(state, session).await {
        Ok(outcome) => {
            state
                .p2p
                .touch_peer(peer, Some(addr), Some(&outcome.peer_device_name));
            info!(
                pairing_id = %peer.pairing_id,
                sent = outcome.stats.sent,
                received = outcome.stats.received,
                skipped = outcome.stats.skipped,
                "synced with a peer"
            );
            SyncResult {
                pairing_id: peer.pairing_id.clone(),
                name: outcome.peer_device_name,
                sent: u32::try_from(outcome.stats.sent).unwrap_or(u32::MAX),
                received: u32::try_from(outcome.stats.received).unwrap_or(u32::MAX),
                error: None,
            }
        }
        Err(message) => failed(message),
    }
}

/// Drive the initiating half of one session over an established channel.
///
/// The whole session is bounded, on top of the channel's per-read deadline: a
/// peer that answers every message on time but never runs out of things to say
/// still has to finish inside `SESSION_TIMEOUT`.
async fn run_session(state: &Arc<AppState>, session: Session) -> Result<SyncOutcome, &'static str> {
    let mut channel = NoiseChannel::new(session);
    let source = StoreSource::new(Arc::clone(state));
    let outcome = tokio::time::timeout(SESSION_TIMEOUT, run_initiator(&mut channel, &source)).await;
    channel.close().await;

    match outcome {
        Ok(Ok(outcome)) => Ok(outcome),
        Ok(Err(e)) => {
            warn!(error = %e, "sync session failed");
            Err(MSG_SESSION)
        }
        Err(_) => Err(MSG_TIMEOUT),
    }
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

/// A peer always has *some* name: the list is unreadable otherwise, and the
/// real one arrives with the first hello.
fn placeholder_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "unnamed device".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

/// Resolve `host:port`, preferring IPv4.
///
/// A hostname is allowed — `copypaste.local:47654` is exactly what discovery
/// would have given the user — so this is a real lookup rather than a
/// `SocketAddr` parse.
async fn resolve(addr: &str) -> Option<SocketAddr> {
    let resolved: Vec<SocketAddr> = tokio::net::lookup_host(addr).await.ok()?.collect();
    resolved
        .iter()
        .find(|addr| addr.is_ipv4())
        .or(resolved.first())
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::p2p::tests::test_state;
    use copypaste_p2p::transport::TOKEN_LEN;

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
        assert_eq!(response.error.as_deref(), Some(MSG_BAD_CODE));
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
        assert_eq!(response.error.as_deref(), Some(MSG_BAD_ADDR));
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
            // An empty list is ambiguous in the untagged response encoding; the
            // CLI treats either as "nothing happened".
            Some(ResponseData::Items(items)) => assert!(items.is_empty()),
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

    #[test]
    fn client_visible_messages_disclose_no_path() {
        for message in ALL_MESSAGES {
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }

    #[test]
    fn a_missing_name_still_reads_as_something() {
        assert_eq!(placeholder_name("   "), "unnamed device");
        assert_eq!(placeholder_name(" laptop "), "laptop");
    }

    #[tokio::test]
    async fn an_address_without_a_port_does_not_resolve() {
        assert!(resolve("127.0.0.1").await.is_none());
        assert_eq!(
            resolve("127.0.0.1:47654").await,
            Some("127.0.0.1:47654".parse().unwrap())
        );
    }
}
