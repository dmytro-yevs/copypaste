//! Peer-to-peer IPC handlers.
//!
//! Each is [`copypaste_p2p::Node`] plus the things only the daemon knows: the
//! response envelope, event stream, device-name registry and cloud cursor. The
//! node holds the sentences, because what a user reads must not depend on which
//! client asked.
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
use std::time::Instant;

use copypaste_core::{now_ms, p2p_contract};
use copypaste_ipc::{
    DiscoveredData, ErrorCode, PairingData, PairingInviteData, Response, ResponseData, SyncResult,
};
#[cfg(test)]
use copypaste_ipc::{PairingProgressData, PeerInfo};
use copypaste_p2p::peers::Peer;
use copypaste_p2p::{NodeError, PairingPhase, PairingStatus};
use tracing::{info, warn};

use crate::sync::peer_source;
use crate::AppState;

/// Mint a pairing and hand back the code to read out to the other device.
pub async fn pair_create(state: &Arc<AppState>, id: u64, name: &str) -> Response {
    if !state.settings.get().sync_enabled {
        return sync_disabled(id);
    }
    let _ = name;
    let pairing = match state.p2p.node().pair_create_invite() {
        Ok(pairing) => pairing,
        Err(e) => return failed(id, e),
    };

    info!(pairing_id = %pairing.pairing_id, "minted a pairing invitation");
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

/// Refuse the legacy one-step operation, which cannot carry SAS confirmation.
pub async fn pair_accept(state: &Arc<AppState>, id: u64, code: &str, addr: &str) -> Response {
    let _ = (state, code, addr);
    Response::err(
        id,
        ErrorCode::InvalidRequest,
        "pair_accept requires the pairing confirmation API",
    )
}

pub async fn pair_create_invite(state: &Arc<AppState>, id: u64) -> Response {
    if !state.settings.get().sync_enabled {
        return sync_disabled(id);
    }
    match state.p2p.node().pair_create_invite() {
        Ok(invite) => Response::ok(
            id,
            ResponseData::PairingInvite(PairingInviteData {
                code: invite.code,
                pairing_id: invite.pairing_id,
                listen_addr: invite.listen_addr,
                expires_in_secs: invite.expires_in_secs,
            }),
        ),
        Err(error) => failed(id, error),
    }
}

pub async fn pair_join(state: &Arc<AppState>, id: u64, code: &str, addr: &str) -> Response {
    if !state.settings.get().sync_enabled {
        return sync_disabled(id);
    }
    let source = peer_source(state);
    match state.p2p.node().pair_join(code, addr, &source).await {
        Ok(status) => pairing_progress(state, id, status),
        Err(error) => failed(id, error),
    }
}

pub async fn pair_get_progress(state: &Arc<AppState>, id: u64) -> Response {
    pairing_progress(state, id, state.p2p.node().pair_progress())
}

pub async fn pair_confirm(state: &Arc<AppState>, id: u64, accept: bool) -> Response {
    if accept && !state.settings.get().sync_enabled {
        state.p2p.node().pair_cancel();
        return sync_disabled(id);
    }
    match state.p2p.node().pair_confirm(accept) {
        Ok(status) => pairing_progress(state, id, status),
        Err(error) => failed(id, error),
    }
}

pub async fn pair_cancel(state: &Arc<AppState>, id: u64) -> Response {
    pairing_progress(state, id, state.p2p.node().pair_cancel())
}

fn pairing_progress(state: &AppState, id: u64, status: PairingStatus) -> Response {
    if status.phase == PairingPhase::Confirmed {
        if let Some(peer) = &status.peer {
            if let Err(error) = state.meta.record_device_name(&peer.device_id, &peer.name) {
                warn!(%error, "could not record a paired device name");
            }
        }
        state.note_peers_changed();
    }
    let known_device = if status.phase == PairingPhase::Confirmed {
        status
            .pairing_id
            .as_deref()
            .and_then(|pairing_id| state.p2p.peers().get(pairing_id))
            .map(|peer| {
                let discovered = state.p2p.find(&peer.pairing_id);
                let authenticated = state.p2p.node().authenticated_profile(&peer.pairing_id);
                p2p_contract::peer_info(&peer, discovered.as_ref(), authenticated.as_ref())
            })
    } else {
        None
    };
    Response::ok(
        id,
        ResponseData::PairingProgress(p2p_contract::pairing_progress(status, known_device)),
    )
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

/// Cut a device off for good.
///
/// [`unpair`] drops the key; this drops the key *and* bars the pairing id, so a
/// code that was written down, a stale copy of the peer file, or a device that
/// keeps re-announcing cannot bring the pairing back. That is the difference
/// between "stop syncing with my laptop" and "I no longer have that phone".
///
/// It succeeds even when no peer was removed, and the revocation is still
/// recorded: barring an id this device has not yet seen is what "revoke the
/// device I lost before it reaches this one" needs. Reporting `not_found` there
/// would say nothing happened when the bar had in fact been written.
pub async fn revoke(state: &Arc<AppState>, id: u64, pairing_id: &str) -> Response {
    match state.p2p.peers().revoke(pairing_id, now_ms()) {
        Ok(removed) => {
            state.p2p.republish();
            state.note_peers_changed();
            info!(%pairing_id, removed, "revoked a pairing");
            Response::ok(id, ResponseData::Empty {})
        }
        Err(e) => {
            warn!(error = %e, "could not revoke a pairing");
            failed(id, NodeError::PeerStore)
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
            let discovered = state.p2p.find(&peer.pairing_id);
            let authenticated = state.p2p.node().authenticated_profile(&peer.pairing_id);
            p2p_contract::peer_info(peer, discovered.as_ref(), authenticated.as_ref())
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
    if !state.settings.get().sync_enabled {
        return sync_disabled(id);
    }
    let targets = match pairing_id {
        Some(pairing_id) => match state.p2p.peers().get(pairing_id) {
            Some(peer) => vec![peer],
            None => return failed(id, NodeError::NoPeer),
        },
        None => state.p2p.peers().list(),
    };

    // Queued behind any pass the poll loop is running rather than dialling the
    // same peers beside it: two sessions to one peer make the far side's
    // session limit refuse the second, and the user asked after the running
    // pass had already read its summaries.
    let _permit = state.p2p.begin_round().await;
    let mut results = Vec::with_capacity(targets.len());
    for peer in &targets {
        results.push(sync_one(state, peer).await);
    }
    if results.iter().any(|result| result.received > 0) {
        state.note_remote_change();
    }
    Response::ok(id, ResponseData::Sync(results))
}

fn sync_disabled(id: u64) -> Response {
    Response::err(id, ErrorCode::NotReady, "Sync is turned off.")
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
        .map(|found| {
            // Resolved locally rather than taken from the advertisement:
            // anyone on the LAN can claim any pairing id, and `paired` decides
            // whether the UI offers "pair" or "sync" (`CopyPaste-vgpy`).
            let paired = found
                .pairing_ids
                .iter()
                .any(|id| state.p2p.peers().get(id).is_some());
            p2p_contract::discovered_device(found, paired)
        })
        .collect();
    DiscoveredData { devices }
}

/// One peer, start to finish. Never returns `Err`: a failure is a field.
pub(super) async fn sync_one(state: &Arc<AppState>, peer: &Peer) -> SyncResult {
    let source = peer_source(state);
    let started = Instant::now();
    let result = state.p2p.node().sync_one(peer, &source).await;
    let duration = started.elapsed();
    if let Ok(outcome) = &result {
        crate::p2p::remember_device(state, outcome);
    }
    p2p_contract::sync_result(peer, result, duration)
}

/// Keep the daemon response envelope local; only the taxonomy is shared.
fn failed(id: u64, error: NodeError) -> Response {
    Response::err(id, p2p_contract::node_error_code(&error), error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;
    use copypaste_p2p::transport::{PairingToken, TOKEN_LEN};

    #[tokio::test]
    async fn creating_an_invite_returns_a_code_and_stores_no_peer() {
        let (state, _dir) = test_state("alpha");
        let response = pair_create_invite(&state, 1).await;
        assert!(response.ok);

        let pairing = match response.data {
            Some(ResponseData::PairingInvite(data)) => data,
            other => panic!("expected pairing data, got {other:?}"),
        };
        assert!(!pairing.code.is_empty());
        assert_ne!(pairing.code, pairing.pairing_id);

        let token = PairingToken::parse(&pairing.code).expect("a valid code");
        assert_eq!(token.pairing_id(), pairing.pairing_id);
        assert!(state.p2p.peers().get(&pairing.pairing_id).is_none());
        assert!(state.p2p.peers().psks().is_empty());

        let listed = peers(&state, 2).await;
        assert!(matches!(listed.data, Some(ResponseData::Peers(ref rows)) if rows.is_empty()));
    }

    #[test]
    fn known_device_data_appears_only_after_confirmation() {
        let (state, _dir) = test_state("alpha");
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        state
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
            })
            .unwrap();
        let progress = |phase| {
            pairing_progress(
                &state,
                1,
                PairingStatus {
                    pairing_id: Some(pairing_id.clone()),
                    role: Some(copypaste_p2p::PairingRole::Initiator),
                    phase,
                    sas: None,
                    peer: None,
                    error: None,
                },
            )
        };

        let awaiting = progress(PairingPhase::AwaitingConfirmation);
        assert!(matches!(
            awaiting.data,
            Some(ResponseData::PairingProgress(PairingProgressData {
                known_device: None,
                ..
            }))
        ));
        let confirmed = progress(PairingPhase::Confirmed);
        assert!(matches!(
            confirmed.data,
            Some(ResponseData::PairingProgress(PairingProgressData {
                known_device: Some(PeerInfo { pairing_id: id, .. }),
                ..
            })) if id == pairing_id
        ));
    }

    #[tokio::test]
    async fn a_second_ceremony_is_refused_until_the_first_is_cancelled() {
        let (state, _dir) = test_state("alpha");
        let first = match pair_create_invite(&state, 1).await.data {
            Some(ResponseData::PairingInvite(data)) => data,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            pair_create_invite(&state, 2).await.error_code,
            Some(ErrorCode::RateLimited)
        );
        assert!(pair_cancel(&state, 3).await.ok);
        let second = match pair_create_invite(&state, 4).await.data {
            Some(ResponseData::PairingInvite(data)) => data,
            other => panic!("{other:?}"),
        };
        assert_ne!(first.pairing_id, second.pairing_id);
        assert_eq!(state.p2p.peers().usable_count(), 0);
    }

    #[tokio::test]
    async fn a_malformed_code_is_rejected_without_touching_the_peer_list() {
        let (state, _dir) = test_state("alpha");
        let response = pair_join(&state, 1, "not-a-code", "127.0.0.1:1").await;
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::PairingCode));
        assert_eq!(
            response.error.as_deref(),
            Some(NodeError::BadCode.to_string().as_str())
        );
        assert_eq!(state.p2p.peers().usable_count(), 0);
    }

    /// A well-formed code for a device that is not listening must not leave a
    /// half-made pairing behind.
    #[tokio::test]
    async fn a_failed_handshake_stores_nothing() {
        let (state, _dir) = test_state("alpha");
        let code = PairingToken::generate().to_code();
        // Port 1 on loopback: nothing is listening, so the connect fails fast.
        let response = pair_join(&state, 1, &code, "127.0.0.1:1").await;
        assert!(!response.ok);
        assert_eq!(
            state.p2p.peers().usable_count(),
            0,
            "a pairing was persisted anyway"
        );
    }

    #[tokio::test]
    async fn an_unresolvable_address_is_a_client_error() {
        let (state, _dir) = test_state("alpha");
        let code = PairingToken::generate().to_code();
        let response = pair_join(&state, 1, &code, "no-port-here").await;
        assert_eq!(response.error_code, Some(ErrorCode::PairingAddress));
        assert_eq!(
            response.error.as_deref(),
            Some(NodeError::BadAddress.to_string().as_str())
        );
    }

    /// The code has to be the *device* one. `ErrorCode::NotFound` is rendered
    /// by every client as a sentence about the clipboard, so answering a
    /// vanished device with it told the user an item had gone.
    #[tokio::test]
    async fn unpairing_an_unknown_device_names_a_device_and_not_an_item() {
        let (state, _dir) = test_state("alpha");
        let response = unpair(&state, 1, "0123456789abcdef0123456789abcdef").await;
        assert_eq!(response.error_code, Some(ErrorCode::PeerNotFound));
        assert_ne!(response.error_code, Some(ErrorCode::NotFound));
        assert_eq!(
            response.error.as_deref(),
            Some(NodeError::NoPeer.to_string().as_str())
        );
    }

    #[tokio::test]
    async fn unpairing_removes_the_key_from_the_listeners_candidates() {
        let (state, _dir) = test_state("alpha");
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        state
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                name: "laptop".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
            })
            .unwrap();
        assert_eq!(state.p2p.peers().psks().len(), 1);

        let response = unpair(&state, 2, &pairing_id).await;
        assert!(response.ok);
        assert!(state.p2p.peers().psks().is_empty());
    }

    /// The whole of the difference between the two verbs, at the wire: after
    /// `unpair` the same code still enrols the pairing, and after `revoke` it
    /// never does again (`CopyPaste-gbo`).
    #[tokio::test]
    async fn revoking_bars_the_pairing_that_unpairing_leaves_reusable() {
        let (state, _dir) = test_state("alpha");
        let peers = state.p2p.peers();

        let unpaired = PairingToken::generate();
        let revoked = PairingToken::generate();
        for token in [&unpaired, &revoked] {
            peers
                .upsert(Peer {
                    pairing_id: token.pairing_id(),
                    name: "phone".into(),
                    psk: token.psk(),
                    last_addr: None,
                    last_seen_ms: 1_753_900_000_000,
                })
                .expect("upsert");
        }

        assert!(unpair(&state, 1, &unpaired.pairing_id()).await.ok);
        assert!(revoke(&state, 2, &revoked.pairing_id()).await.ok);
        assert!(
            peers.psks().is_empty(),
            "both keys must leave the candidates"
        );

        let re_add = |token: &PairingToken| {
            peers.upsert(Peer {
                pairing_id: token.pairing_id(),
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 0,
            })
        };
        re_add(&unpaired).expect("unpairing must leave the pairing id usable");
        assert!(
            re_add(&revoked).is_err(),
            "a revoked pairing came back, so the code someone kept still works"
        );
    }

    /// Barring a device that has not reached this one yet is the case the
    /// store's unconditional record exists for, so it must not answer as if
    /// nothing happened.
    #[tokio::test]
    async fn revoking_a_device_this_one_has_not_seen_succeeds_and_still_bars_it() {
        let (state, _dir) = test_state("alpha");
        let token = PairingToken::generate();

        let response = revoke(&state, 1, &token.pairing_id()).await;
        assert!(response.ok, "{:?}", response.error);
        assert!(state
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: token.pairing_id(),
                name: "the lost phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 0,
            })
            .is_err());
    }

    #[tokio::test]
    async fn peers_lists_what_was_paired() {
        let (state, _dir) = test_state("alpha");
        let token = PairingToken::generate();
        state
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: token.pairing_id(),
                name: "laptop".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
            })
            .unwrap();

        let response = peers(&state, 2).await;
        let listed = match response.data {
            Some(ResponseData::Peers(peers)) => peers,
            other => panic!("{other:?}"),
        };
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].name, "laptop");
        assert!(listed[0].last_addr.is_none());
        // Nothing is currently visible on the network in a unit test.
        assert!(!listed[0].online);
    }

    #[tokio::test]
    async fn syncing_a_named_peer_that_does_not_exist_is_not_found() {
        let (state, _dir) = test_state("alpha");
        let response = sync_now(&state, 1, Some("0123456789abcdef")).await;
        assert_eq!(response.error_code, Some(ErrorCode::PeerNotFound));
    }

    /// `copypaste sync` used to dial every peer while the poll loop was doing
    /// the same. Two sessions to one peer are not two syncs: the far side's
    /// session limit refuses the second, so the run reports a failure for a
    /// peer that is in fact reachable and syncing.
    #[tokio::test]
    async fn an_explicit_sync_waits_for_a_running_pass_rather_than_dialling_beside_it() {
        let (state, _dir) = test_state("alpha");
        let held = state.p2p.begin_round().await;
        assert!(state.p2p.round_in_flight());

        let queued = tokio::spawn({
            let state = Arc::clone(&state);
            async move { sync_now(&state, 1, None).await.ok }
        });
        tokio::task::yield_now().await;
        assert!(
            !queued.is_finished(),
            "an explicit sync dialled beside a running pass"
        );

        drop(held);
        assert!(queued.await.unwrap());
        assert!(state.p2p.try_begin_round().is_some(), "the permit leaked");
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

    #[tokio::test]
    async fn the_live_sync_switch_blocks_manual_pairing_and_sync() {
        let (state, _dir) = test_state("alpha");
        state
            .settings
            .apply(
                &state.meta,
                &copypaste_ipc::ConfigPatch {
                    sync_enabled: Some(false),
                    ..Default::default()
                },
            )
            .unwrap();

        assert_eq!(
            pair_create_invite(&state, 1).await.error_code,
            Some(ErrorCode::NotReady)
        );
        assert_eq!(
            pair_join(&state, 2, "not-a-code", "127.0.0.1:1")
                .await
                .error_code,
            Some(ErrorCode::NotReady)
        );
        assert_eq!(
            sync_now(&state, 3, None).await.error_code,
            Some(ErrorCode::NotReady)
        );
    }

    /// An established peer with no known address is reported per-peer, and the
    /// run still succeeds.
    #[tokio::test]
    async fn a_peer_with_no_known_address_fails_only_itself() {
        let (state, _dir) = test_state("alpha");
        let first = PairingToken::generate();
        state
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: first.pairing_id(),
                name: "unreachable".into(),
                psk: first.psk(),
                last_addr: None,
                last_seen_ms: 1,
            })
            .unwrap();
        state
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: "ffffffffffffffffffffffffffffffff".into(),
                name: "second".into(),
                psk: [3u8; TOKEN_LEN],
                last_addr: None,
                last_seen_ms: 1,
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
        assert!(results
            .iter()
            .all(|r| r.error_code == Some(ErrorCode::PeerUnreachable)));
    }

    /// Every variant the node can produce, with the code it arrives under.
    ///
    /// Exhaustive on purpose: this is the list a new `NodeError` has to be
    /// added to, while the compiler forces the canonical mapping to be total.
    const EVERY_FAILURE: &[(NodeError, ErrorCode)] = &[
        (NodeError::BadCode, ErrorCode::PairingCode),
        (NodeError::Handshake, ErrorCode::PairingCode),
        (NodeError::SelfPairing, ErrorCode::PairingCode),
        (NodeError::PairingBusy, ErrorCode::RateLimited),
        (NodeError::NoPairing, ErrorCode::NotReady),
        (NodeError::BadAddress, ErrorCode::PairingAddress),
        (NodeError::NoAddress, ErrorCode::PeerUnreachable),
        (NodeError::Timeout, ErrorCode::PeerUnreachable),
        (NodeError::TooManyPairings, ErrorCode::PairingLimit),
        (NodeError::Session, ErrorCode::PeerFailed),
        (NodeError::PeerStore, ErrorCode::PeerFailed),
        (NodeError::PeerVersion, ErrorCode::PeerVersion),
        (NodeError::NoPeer, ErrorCode::PeerNotFound),
    ];

    /// The node owns the sentences; this pins the taxonomy they arrive under,
    /// and that none of them picks up a path on the way through.
    #[test]
    fn node_failures_map_onto_the_ipc_taxonomy_without_a_path() {
        for (error, expected) in EVERY_FAILURE {
            let response = failed(1, *error);
            assert_eq!(response.error_code, Some(*expected), "{error:?}");
            let message = response.error.expect("a message");
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }

    /// The property `NotFound` failed: the sentence the node wrote is what
    /// reaches the client, so a code that carries a remedy still carries it.
    #[test]
    fn every_node_sentence_survives_the_mapping() {
        for (error, _) in EVERY_FAILURE {
            let response = failed(1, *error);
            assert_eq!(
                response.error.as_deref(),
                Some(error.to_string().as_str()),
                "{error:?}"
            );
        }
        let capped = failed(1, NodeError::TooManyPairings)
            .error
            .expect("a message");
        assert!(capped.contains("unpair one first"), "{capped}");
    }

    /// Nine sentences over six codes, and no two conditions that need
    /// different screens share one.
    #[test]
    fn no_pairing_condition_reaches_a_client_as_a_generic_failure() {
        for (error, code) in EVERY_FAILURE {
            assert!(
                !matches!(
                    code,
                    ErrorCode::Internal | ErrorCode::InvalidRequest | ErrorCode::NotFound
                ),
                "{error:?} has no code of its own"
            );
        }
    }
}
