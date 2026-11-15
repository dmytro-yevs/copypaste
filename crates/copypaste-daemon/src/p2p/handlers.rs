//! Peer operations through the daemon host.

#[allow(unused_imports)]
use crate::testutil::calls::*;

mod tests {
    use super::*;
    use crate::testutil::{peer_at, test_state};
    use copypaste_p2p::transport::{PairingToken, TOKEN_LEN};
    use tokio::io::AsyncReadExt;
    use tokio::sync::oneshot;

    fn set_sync_enabled(state: &AppState, enabled: bool) -> Response {
        crate::server::dispatch::dispatch_store(
            state,
            99,
            copypaste_ipc::Method::SetConfig {
                patch: copypaste_ipc::ConfigPatch {
                    sync_enabled: Some(enabled),
                    ..Default::default()
                },
            },
        )
    }

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

        let code = copypaste_p2p::PairingCode::parse(&pairing.code).expect("a valid short code");
        assert_eq!(code.to_code().len(), 8);
        assert_eq!(pairing.pairing_id.len(), 32);
        assert!(state.p2p.peers().get(&pairing.pairing_id).is_none());
        assert!(state.p2p.peers().psks().is_empty());

        let listed = peers(&state, 2).await;
        assert!(matches!(listed.data, Some(ResponseData::Peers(ref rows)) if rows.is_empty()));
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
        assert_eq!(state.p2p.peers().len(), 0);
    }

    #[tokio::test]
    async fn a_malformed_code_is_rejected_without_touching_the_peer_list() {
        let (state, _dir) = test_state("alpha");
        let response = pair_join(&state, 1, "not-a-code!", "127.0.0.1:1").await;
        assert!(!response.ok);
        assert_eq!(response.error_code, Some(ErrorCode::PairingCode));
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
        let response = pair_join(&state, 1, &code, "127.0.0.1:1").await;
        assert!(!response.ok);
        assert_eq!(state.p2p.peers().len(), 0, "a pairing was persisted anyway");
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
                device_id: None,
                name: "laptop".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
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
                    device_id: None,
                    name: "phone".into(),
                    psk: token.psk(),
                    last_addr: None,
                    last_seen_ms: 1_753_900_000_000,
                    profile: None,
                    profile_observed_at_ms: 0,
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
                device_id: None,
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
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
                device_id: None,
                name: "the lost phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
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
                device_id: None,
                name: "laptop".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
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
    async fn disabling_sync_cancels_an_active_pass_and_a_reenabled_pass_dials_again() {
        let (state, _dir) = test_state("cancel-active-peer-sync");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let peer = peer_at(
            &state,
            "blocked peer",
            &listener.local_addr().expect("address").to_string(),
        );
        let (first_tx, first_rx) = oneshot::channel();
        let (second_tx, second_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await.expect("first connection");
            first_tx.send(()).expect("record first connection");
            let mut byte = [0];
            let _ = first.read(&mut byte).await;
            let (mut second, _) = listener.accept().await.expect("second connection");
            second_tx.send(()).expect("record second connection");
            let _ = second.read(&mut byte).await;
        });

        let active = tokio::spawn({
            let state = Arc::clone(&state);
            let pairing_id = peer.pairing_id.clone();
            async move { sync_now(&state, 1, Some(&pairing_id)).await }
        });
        first_rx
            .await
            .expect("outbound pass entered the owned transport");

        assert!(
            set_sync_enabled(&state, false).ok,
            "the off transition failed"
        );
        let stopped = active.await.expect("sync task");
        let stopped_results = match stopped.data {
            Some(ResponseData::Sync(results)) => results,
            other => panic!("{other:?}"),
        };
        assert_eq!(stopped_results.len(), 1);
        assert_eq!(stopped_results[0].error_code, Some(ErrorCode::PeerFailed));
        assert_eq!(state.p2p.node().cursors().get(&peer.pairing_id).since_ms, 0);

        assert!(
            set_sync_enabled(&state, true).ok,
            "the on transition failed"
        );
        let replay = tokio::spawn({
            let state = Arc::clone(&state);
            let pairing_id = peer.pairing_id.clone();
            async move { sync_now(&state, 2, Some(&pairing_id)).await }
        });
        second_rx
            .await
            .expect("the fresh enabled cycle re-offered the same peer");
        assert!(set_sync_enabled(&state, false).ok);
        let _ = replay.await.expect("replayed sync task");
        server.await.expect("owned transport task");
    }

    #[tokio::test]
    async fn a_queued_sync_disabled_before_its_turn_never_dials() {
        let (state, _dir) = test_state("cancel-queued-peer-sync");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("listener");
        let peer = peer_at(
            &state,
            "queued peer",
            &listener.local_addr().expect("address").to_string(),
        );
        let (entered_tx, mut entered_rx) = oneshot::channel();
        let server = tokio::spawn(async move {
            let _ = listener.accept().await;
            let _ = entered_tx.send(());
        });
        let held = state.p2p.begin_round().await;
        let queued = tokio::spawn({
            let state = Arc::clone(&state);
            let pairing_id = peer.pairing_id.clone();
            async move { sync_now(&state, 1, Some(&pairing_id)).await }
        });
        tokio::task::yield_now().await;
        assert!(
            !queued.is_finished(),
            "the request skipped the running pass"
        );

        assert!(
            set_sync_enabled(&state, false).ok,
            "the off transition failed"
        );
        drop(held);
        let response = queued.await.expect("queued request task");
        assert_eq!(response.error_code, Some(ErrorCode::NotReady));
        assert!(
            entered_rx.try_recv().is_err(),
            "a request queued before the off acknowledgement dialled afterwards"
        );
        server.abort();
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
            .apply(&copypaste_ipc::ConfigPatch {
                sync_enabled: Some(false),
                ..Default::default()
            })
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
        assert_eq!(
            pair_nearby(&state, 4, "127.0.0.1:1").await.error_code,
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
                device_id: None,
                name: "unreachable".into(),
                psk: first.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();
        state
            .p2p
            .peers()
            .upsert(Peer {
                pairing_id: "ffffffffffffffffffffffffffffffff".into(),
                device_id: None,
                name: "second".into(),
                psk: [3u8; TOKEN_LEN],
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
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
        (NodeError::AlreadyPaired, ErrorCode::PairingAlreadyExists),
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
