//! Peer sync through the daemon host: two daemons over loopback.

pub mod handlers;

#[allow(unused_imports)]
use crate::testutil::calls::*;

mod tests {
    use super::*;
    use crate::testutil::{add, contents, test_state};
    use copypaste_ipc::{EventKind, ResponseData};
    use copypaste_p2p::peers::Peer;
    use std::net::SocketAddr;

    /// Pair two states over loopback and hand back the address A listens on.
    ///
    /// Both halves get the same pre-shared key, which is the whole of the
    /// pairing: the initiator dials, the listener recognises the key, and
    /// neither side needs anything else.
    async fn pair(
        a: &Arc<AppState>,
        b: &Arc<AppState>,
        shutdown: watch::Receiver<bool>,
    ) -> (String, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(listen(listener, Arc::clone(a), shutdown));

        let token = copypaste_p2p::transport::PairingToken::generate();
        let pairing_id = token.pairing_id();
        a.p2p
            .peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "b".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .expect("store the pairing on A");
        b.p2p
            .peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "a".into(),
                psk: token.psk(),
                last_addr: Some(addr),
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .expect("store the pairing on B");
        (pairing_id, addr)
    }

    #[tokio::test]
    async fn inbound_pairing_forwards_a_peer_event_before_trust_is_committed() {
        let (responder, _responder_dir) = test_state("responder");
        let (initiator, _initiator_dir) = test_state("initiator");
        let mut events = responder.subscribe_events();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("listener address");
        tokio::spawn(listen(
            listener,
            Arc::clone(&responder),
            shutdown_rx.clone(),
        ));

        let bridge = tokio::spawn(responder.runtime.forward_changes(shutdown_rx.clone()));
        let invite = match crate::testutil::calls::pair_create_invite(&responder, 1)
            .await
            .data
        {
            Some(ResponseData::PairingInvite(invite)) => invite,
            other => panic!("expected pairing invite, got {other:?}"),
        };
        assert_eq!(
            tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
                .await
                .expect("invite event was not published")
                .expect("event stream closed")
                .event,
            EventKind::Peers,
        );

        assert!(
            crate::testutil::calls::pair_join(&initiator, 2, &invite.code, &addr.to_string(),)
                .await
                .ok
        );
        tokio::time::timeout(std::time::Duration::from_secs(1), async {
            loop {
                assert_eq!(
                    events.recv().await.expect("event stream closed").event,
                    EventKind::Peers
                );
                if responder.p2p.node().pair_progress().phase
                    == copypaste_p2p::PairingPhase::AwaitingConfirmation
                {
                    return;
                }
            }
        })
        .await
        .expect("inbound AwaitingConfirmation event was not published");
        assert!(responder.p2p.peers().list().is_empty());

        let _ = shutdown_tx.send(true);
        bridge.await.expect("pairing event bridge stopped");
    }

    #[tokio::test]
    async fn inbound_probe_publishes_one_peer_event() {
        let (receiver, _receiver_dir) = test_state("receiver");
        let (sender, _sender_dir) = test_state("sender");
        let mut events = receiver.subscribe_events();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let bridge = tokio::spawn(receiver.runtime.forward_changes(shutdown_rx.clone()));
        let (pairing_id, addr) = pair(&receiver, &sender, shutdown_rx).await;

        let peer = sender.p2p.peers().get(&pairing_id).unwrap();
        sender.p2p.node().probe_one(&peer).await.unwrap();
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), events.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(event.event, EventKind::Peers);
        assert!(event.sync_status.is_some());
        let observed = receiver.p2p.peers().get(&pairing_id).unwrap();
        assert!(observed.profile.is_some());
        assert!(observed.profile_observed_at_ms > 0);
        assert!(observed.last_seen_ms > 0);
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(25), events.recv())
                .await
                .is_err()
        );
        assert_eq!(receiver.p2p.node().listen_addr(), Some(addr.to_string()));
        let _ = shutdown_tx.send(true);
        bridge.await.expect("peer event bridge stopped");
    }

    /// The whole thing, in process: a listener, a dialler, two databases with
    /// two different device secrets, and the three properties that matter.
    ///
    /// Every assertion runs the instant `sync_now` returns, with no sleep. That
    /// is deliberate and it is a real guarantee: the initiator's last act is a
    /// `Done` the responder has yet to *apply*, so a session that returned as
    /// soon as the bytes left would make this test — and any script that syncs
    /// and then reads — a race. `NoiseChannel::wait_for_close` is what closes
    /// it. This test failed roughly one run in ten before that existed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_daemons_converge_all_captured_items() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (pairing_id, _addr) = pair(&a, &b, shutdown_rx).await;

        add(&a, "from a");
        add(&b, "from b");
        add(&a, "third item");

        let response = crate::testutil::calls::sync_now(&b, 1, Some(&pairing_id)).await;
        assert!(response.ok, "{:?}", response.error);

        let on_a = contents(&a);
        let on_b = contents(&b);
        for item in ["from a", "from b"] {
            assert!(
                on_a.iter().any(|c| c == item),
                "A is missing {item}: {on_a:?}"
            );
            assert!(
                on_b.iter().any(|c| c == item),
                "B is missing {item}: {on_b:?}"
            );
        }
        assert!(on_a.iter().any(|c| c == "third item"));
        assert!(on_b.iter().any(|c| c == "third item"));

        // The same item ids on both sides, which is what makes the second
        // session free rather than a second copy of everything.
        let ids_a: std::collections::HashSet<String> = a
            .store
            .list(100, 0)
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect();
        let ids_b: std::collections::HashSet<String> = b
            .store
            .list(100, 0)
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect();
        assert_eq!(ids_a, ids_b);

        let _ = shutdown_tx.send(true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_reenabled_cycle_applies_a_real_peer_item_once() {
        let (a, _da) = test_state("cycle-source");
        let (b, _db) = test_state("cycle-target");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (pairing_id, _addr) = pair(&a, &b, shutdown_rx).await;
        add(&a, "off then on");

        crate::server::dispatch::dispatch_store(
            &b,
            1,
            copypaste_ipc::Method::SetConfig {
                patch: copypaste_ipc::ConfigPatch {
                    sync_enabled: Some(false),
                    ..Default::default()
                },
            },
        );
        let stopped = crate::testutil::calls::sync_now(&b, 1, Some(&pairing_id)).await;
        assert!(!stopped.ok);
        assert!(contents(&b).is_empty());
        assert_eq!(b.p2p.node().cursors().get(&pairing_id).since_ms, 0);

        crate::server::dispatch::dispatch_store(
            &b,
            2,
            copypaste_ipc::Method::SetConfig {
                patch: copypaste_ipc::ConfigPatch {
                    sync_enabled: Some(true),
                    ..Default::default()
                },
            },
        );
        let resumed = crate::testutil::calls::sync_now(&b, 2, Some(&pairing_id)).await;
        assert!(resumed.ok);
        assert_eq!(contents(&b), ["off then on"]);
        let cursor = b.p2p.node().cursors().get(&pairing_id).since_ms;
        assert!(
            cursor > 0,
            "the completed session did not commit its cursor"
        );

        let replay = crate::testutil::calls::sync_now(&b, 3, Some(&pairing_id)).await;
        assert!(replay.ok);
        assert_eq!(contents(&b), ["off then on"]);
        assert_eq!(b.p2p.node().cursors().get(&pairing_id).since_ms, cursor);
        let _ = shutdown_tx.send(true);
    }

    /// A repeated session must move nothing: `plan` skips a tie, and `apply`
    /// re-checks the merge, so replay is a no-op at two layers.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_second_session_transfers_nothing() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (pairing_id, _addr) = pair(&a, &b, shutdown_rx).await;

        add(&a, "from a");
        add(&b, "from b");

        let first = crate::testutil::calls::sync_now(&b, 1, Some(&pairing_id)).await;
        assert!(first.ok);
        let second = crate::testutil::calls::sync_now(&b, 2, Some(&pairing_id)).await;

        let results = match second.data {
            Some(copypaste_ipc::ResponseData::Sync(results)) => results,
            other => panic!("{other:?}"),
        };
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].error, None);
        assert_eq!(
            (results[0].sent, results[0].received),
            (0, 0),
            "a replayed session moved data"
        );

        let _ = shutdown_tx.send(true);
    }

    /// The listener must refuse a dialler holding a key it does not know, and
    /// say nothing about which keys it does know.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_unknown_pairing_key_cannot_open_a_session() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_pairing_id, addr) = pair(&a, &b, shutdown_rx).await;
        add(&a, "not yours");

        // A different pairing entirely: well-formed, and unknown to A.
        let stranger = copypaste_p2p::transport::PairingToken::generate();
        b.p2p
            .peers()
            .upsert(Peer {
                pairing_id: stranger.pairing_id(),
                device_id: None,
                name: "stranger".into(),
                psk: stranger.psk(),
                last_addr: Some(addr),
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();

        let response = crate::testutil::calls::sync_now(&b, 1, Some(&stranger.pairing_id())).await;
        let results = match response.data {
            Some(copypaste_ipc::ResponseData::Sync(results)) => results,
            other => panic!("{other:?}"),
        };
        assert!(results[0].error.is_some(), "the handshake must fail");
        assert!(
            contents(&b).is_empty(),
            "nothing may be transferred over a refused handshake"
        );

        let _ = shutdown_tx.send(true);
    }

    #[test]
    fn two_states_have_distinct_device_identities() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        assert_ne!(a.meta.device_id(), b.meta.device_id());
        assert_eq!(a.meta.device_name(), "alpha");
    }

    #[test]
    fn a_listen_address_is_never_loopback() {
        let (state, _dir) = test_state("alpha");
        if let Some(addr) = state.p2p.listen_addr() {
            let parsed: SocketAddr = addr.parse().expect("a socket address");
            assert!(!parsed.ip().is_loopback(), "{addr}");
        }
    }
}
