//! The inbound half: accept, authenticate against every stored pairing,
//! respond.
//!
//! An unknown pre-shared key fails the handshake and the connection is dropped
//! without a reply. That is deliberate and it is the whole authentication
//! story: `accept_any` reports nothing about which pairings this device holds,
//! and neither does the logging here.

use std::net::SocketAddr;
use std::sync::Arc;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;
use tracing::{debug, info, warn};

use super::channel::{NoiseChannel, SESSION_TIMEOUT};
use super::probe::{respond as respond_to_probe, AuthenticatedReachability};
use super::Node;
use crate::protocol::SyncMessage;
use crate::sync::{run_responder_from_first, SyncChannel, SyncOutcome, SyncSource};
use crate::transport::Session;

/// Bind the peer port.
///
/// `0.0.0.0`: the channel refuses anyone without a stored pre-shared key, so an
/// open port discloses only that CopyPaste is running.
pub fn bind(port: u16) -> std::io::Result<std::net::TcpListener> {
    let listener = std::net::TcpListener::bind(("0.0.0.0", port))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Accept peer connections until shutdown.
///
/// `on_session` is called after each completed session with the pairing id and
/// what it moved, so the caller can wake whatever it keeps in step with the
/// history. It is deliberately not part of [`SyncSource`]: applying an item and
/// noticing that a round happened are different jobs, and only the second one
/// needs to reach a UI.
pub async fn listen<S, F, P>(
    node: Arc<Node>,
    listener: TcpListener,
    source: Arc<S>,
    on_session: F,
    on_probe: P,
    mut shutdown: watch::Receiver<bool>,
) where
    S: SyncSource + Send + Sync + 'static,
    F: Fn(&str, &SyncOutcome) + Send + Sync + Clone + 'static,
    P: Fn() + Send + Sync + Clone + 'static,
{
    match listener.local_addr() {
        Ok(addr) => node.set_listen_addr(addr),
        Err(error) => warn!(error = %error, "could not determine the peer listener address"),
    }
    info!(port = node.port(), "peer listener started");
    let mut sessions = tokio::task::JoinSet::new();
    let monitor = Arc::clone(&node).monitor_reachability(on_probe.clone(), shutdown.clone());
    tokio::pin!(monitor);

    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            _ = &mut monitor => break,
            Some(_) = sessions.join_next() => {}
            accepted = listener.accept() => match accepted {
                Ok((stream, addr)) => {
                    let Ok(permit) = Arc::clone(&node.sessions).try_acquire_owned() else {
                        debug!(%addr, "too many peer sessions in flight; dropping the connection");
                        continue;
                    };
                    let node = Arc::clone(&node);
                    let source = Arc::clone(&source);
                    let on_session = on_session.clone();
                    let on_probe = on_probe.clone();
                    sessions.spawn(async move {
                        let _permit = permit;
                        serve_peer(&node, stream, addr, source.as_ref(), &on_session, &on_probe).await;
                    });
                }
                Err(e) => warn!(error = %e, "could not accept a peer connection"),
            },
        }
    }

    sessions.shutdown().await;
    info!("peer listener stopped");
}

/// One inbound peer, start to finish.
async fn serve_peer<S, F, P>(
    node: &Arc<Node>,
    stream: TcpStream,
    addr: SocketAddr,
    source: &S,
    on_session: &F,
    on_probe: &P,
) where
    S: SyncSource,
    F: Fn(&str, &SyncOutcome),
    P: Fn(),
{
    let mut candidates = node.peers().psks();
    // Tentative Stored slots cannot admit another connection before commit.
    candidates.retain(|candidate| !node.pairing.is_pending_pairing(&candidate.pairing_id));
    let pending = node.pairing_candidate();
    let pending_pairing_id = pending
        .as_ref()
        .map(|candidate| candidate.token.pairing_id.clone());
    if let Some(candidate) = &pending {
        candidates.push(crate::PskCandidate {
            pairing_id: candidate.token.pairing_id.clone(),
            psk: candidate.token.psk,
        });
    }
    let mut nearby_id = None;
    let (session, pairing_id) = match Session::accept_with_membership(
        stream,
        &candidates,
        pending.as_ref(),
        || {
            if !node.lan_visible.load(std::sync::atomic::Ordering::Relaxed)
                || node.peers.len() >= crate::peers::MAX_PAIRINGS
            {
                return None;
            }
            let candidate = node.pairing.nearby_candidate()?;
            nearby_id = Some(candidate.token.pairing_id.clone());
            Some(candidate)
        },
        || node.peers().metadata_psks(),
    )
    .await
    {
        Ok(accepted) => accepted,
        Err(e) => {
            if let Some(id) = &nearby_id {
                node.pairing.cancel_reservation(id);
            }
            debug!(%addr, error = %e, "inbound peer handshake failed");
            return;
        }
    };

    if nearby_id.as_deref() == Some(pairing_id.as_str())
        || pending_pairing_id.as_deref() == Some(pairing_id.as_str())
    {
        if let Err(error) = Arc::clone(node)
            .accept_pairing(session, &pairing_id, source)
            .await
        {
            debug!(%pairing_id, %error, "inbound pairing ceremony failed");
        }
        return;
    }

    node.note_authenticated(&pairing_id);
    let membership_only = session.is_membership_only();
    let listen_addr = node.session_listen_addr(session.local_addr());
    let mut channel = NoiseChannel::new(session);
    let mut first = match channel.recv().await {
        Ok(message) => message,
        Err(error) => {
            let condition = super::dial::session_error(error.clone());
            if condition.sync_issue().is_some_and(|code| code.blocking()) {
                node.sync_activity.failure(&pairing_id, condition);
            }
            debug!(%pairing_id, error = %error, "peer sent no valid application message");
            channel.close().await;
            return;
        }
    };
    if matches!(
        first,
        SyncMessage::Group { .. }
            | SyncMessage::Probe { .. }
            | SyncMessage::Hello { .. }
            | SyncMessage::FileRequest { .. }
    ) {
        node.sync_activity.evidence(
            &pairing_id,
            copypaste_ipc::sync_issue::SyncEvidence::CompatibleProtocol,
        );
    }
    if membership_only {
        if let SyncMessage::Group { snapshot, .. } = first {
            match tokio::time::timeout(
                SESSION_TIMEOUT,
                node.respond_group(&mut channel, source, &pairing_id, snapshot, false),
            )
            .await
            {
                Ok(Ok(())) => on_probe(),
                Ok(Err(error)) => node.sync_activity.failure(&pairing_id, error),
                Err(_) => node
                    .sync_activity
                    .failure(&pairing_id, super::NodeError::Timeout),
            }
        } else {
            node.sync_activity
                .failure(&pairing_id, super::NodeError::InvalidData);
        }
        channel.close().await;
        return;
    }
    if let SyncMessage::Probe {
        protocol_version,
        nonce,
        profile,
    } = first
    {
        let result = respond_to_probe(&mut channel, protocol_version, nonce).await;
        channel.close().await;
        match result {
            Ok(()) => {
                if let Some(peer) = node.peers().get(&pairing_id) {
                    node.record_reachability(
                        &pairing_id,
                        AuthenticatedReachability::online(None, crate::now_ms()),
                    );
                    node.record_authenticated_profile(&pairing_id, profile.as_ref());
                    // The inbound source port belongs to this probe, not to
                    // the peer listener. Preserve the authenticated endpoint.
                    node.touch_peer(&peer, None, None, None);
                    on_probe();
                }
            }
            Err(error) => debug!(%pairing_id, %error, "peer liveness probe failed"),
        }
        return;
    }
    if matches!(first, SyncMessage::FileRequest { .. }) {
        let _ = node
            .serve_file(&pairing_id, source, &mut channel, first)
            .await;
        channel.close().await;
        return;
    }
    if let SyncMessage::Group { snapshot, .. } = first {
        let exchanged = tokio::time::timeout(
            SESSION_TIMEOUT,
            node.respond_group(&mut channel, source, &pairing_id, snapshot, false),
        )
        .await;
        match exchanged {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                node.sync_activity.failure(&pairing_id, error);
                channel.close().await;
                return;
            }
            Err(_) => {
                node.sync_activity
                    .failure(&pairing_id, super::NodeError::Timeout);
                channel.close().await;
                return;
            }
        }
        on_probe();
        first = match channel.recv().await {
            Ok(message) => message,
            Err(error) => {
                node.sync_activity
                    .failure(&pairing_id, super::dial::session_error(error));
                channel.close().await;
                return;
            }
        };
    } else {
        // Current nodes always exchange membership before history. A bare
        // legacy Hello must not silently bypass the group protocol.
        node.sync_activity
            .failure(&pairing_id, super::NodeError::InvalidData);
        channel.close().await;
        return;
    }
    // Register after the first message distinguishes a history session from a
    // probe, but before any summary or payload reaches the source. If removal
    // won the race after Noise authenticated, do not let this old transport
    // begin a history exchange.
    let Some(active) = node.begin_active_sync(&pairing_id) else {
        channel.close().await;
        return;
    };
    let active_cancel = active.cancelled();
    let guarded_source = super::ActiveSyncSource {
        source,
        state: Arc::clone(&active.state),
    };
    let observation = node.sync_activity.begin(&pairing_id);
    let activity = Arc::clone(&node.sync_activity);
    let transfer_pairing_id = pairing_id.clone();
    let token = active.id;
    let _transfer = activity.transfer_scope(&pairing_id, token);
    channel = channel.on_transfer(move || activity.transferring(&transfer_pairing_id, token));
    let cursor = node.cursors().get(&pairing_id);
    let outcome = tokio::select! {
        _ = active_cancel.cancelled() => None,
        outcome = tokio::time::timeout(
            SESSION_TIMEOUT,
            run_responder_from_first(&mut channel, &guarded_source, listen_addr.as_deref(), cursor, first),
        ) => Some(outcome),
    };
    channel.close().await;

    match outcome {
        None => {
            observation.finish(None, Some(super::NodeError::Cancelled));
        }
        Some(Ok(Ok(outcome))) => {
            if outcome.peer_device_id == source.device_id() {
                // The self-pairing path removes the same pairing; release its
                // active-session lease before it takes the removal lock.
                drop(active);
                if let Err(error) = node.unpair(&pairing_id) {
                    warn!(%pairing_id, %error, "could not remove a self-pairing");
                }
                return;
            }
            info!(
                %pairing_id,
                sent = outcome.stats.sent,
                received = outcome.stats.received,
                skipped = outcome.stats.skipped,
                "served a peer sync session"
            );
            if node
                .commit_active_sync(&active, || {
                    node.record_cursor(&pairing_id, &outcome);
                    node.record_authenticated_profile(&pairing_id, outcome.peer_profile.as_ref());
                    if let Some(peer) = node.peers().get(&pairing_id) {
                        node.touch_peer(
                            &peer,
                            Some(&outcome.peer_device_id),
                            outcome
                                .peer_listen_addr
                                .and_then(super::ListenerEndpoint::advertised),
                            Some(&outcome.peer_device_name),
                        );
                    }
                })
                .is_some()
            {
                on_session(&pairing_id, &outcome);
                observation.finish(Some(&outcome.stats), None);
            } else {
                observation.finish(None, Some(super::NodeError::Cancelled));
            }
        }
        Some(Ok(Err(e))) => {
            warn!(%pairing_id, error = %e, "peer sync session failed");
            observation.finish(None, Some(super::dial::session_error(e)));
        }
        Some(Err(_)) => {
            warn!(%pairing_id, "peer sync session timed out");
            observation.finish(None, Some(super::NodeError::Timeout));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::{Peer, PeerStore};
    use crate::sync::testutil::{item, TestSource};
    use crate::transport::PairingToken;
    use crate::NodeError;

    #[tokio::test]
    async fn inbound_probes_preserve_the_authenticated_listener_endpoint() {
        let dir = tempfile::tempdir().unwrap();
        let receiver = node(&dir, "receiver");
        let sender = node(&dir, "sender");
        let receiver_listener = receiver.bind_listener().unwrap();
        let sender_listener = sender.bind_listener().unwrap();
        let address = SocketAddr::new(
            "127.0.0.1".parse().unwrap(),
            receiver_listener.local_addr().unwrap().port(),
        );
        let expected = SocketAddr::new(
            "127.0.0.1".parse().unwrap(),
            sender_listener.local_addr().unwrap().port(),
        );
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        let peer = Peer {
            pairing_id: pairing_id.clone(),
            device_id: None,
            name: "probe peer".into(),
            psk: token.psk(),
            last_addr: Some(expected),
            last_seen_ms: 0,
            profile: None,
            profile_observed_at_ms: 0,
        };
        receiver.peers().upsert(peer.clone()).unwrap();
        let mut outgoing_peer = peer.clone();
        outgoing_peer.last_addr = Some(address);
        sender.peers().upsert(outgoing_peer.clone()).unwrap();
        let (shutdown, rx) = watch::channel(false);
        let (completed, mut receipts) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(listen(
            Arc::clone(&receiver),
            TcpListener::from_std(receiver_listener).unwrap(),
            Arc::new(TestSource::new("receiver", vec![])),
            |_, _| {},
            move || {
                let _ = completed.send(());
            },
            rx,
        ));
        for _ in 0..2 {
            sender.probe_one(&outgoing_peer).await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(1), receipts.recv())
                .await
                .unwrap()
                .unwrap();
            assert_eq!(
                receiver.peers().get(&pairing_id).unwrap().last_addr,
                Some(expected)
            );
        }
        assert_eq!(
            receiver.sync_status(true).phase,
            copypaste_ipc::SyncPhase::Waiting
        );
        let _ = shutdown.send(true);
    }

    fn node(dir: &tempfile::TempDir, name: &str) -> Arc<Node> {
        let peers = PeerStore::open(
            &dir.path().join(format!("{name}-peers.json")),
            &crate::peers::testutil::KEY,
        )
        .unwrap();
        Arc::new(Node::new(peers, None, 0, true))
    }

    /// A listener plus the pairing both halves share. Returns the address to
    /// dial and the pairing id.
    async fn listening(
        node: &Arc<Node>,
        source: Arc<TestSource>,
        shutdown: watch::Receiver<bool>,
    ) -> (SocketAddr, String) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        node.peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "dialler".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();
        tokio::spawn(listen(
            Arc::clone(node),
            listener,
            source,
            |_: &str, _: &SyncOutcome| {},
            || {},
            shutdown,
        ));
        (addr, token.to_code())
    }

    /// The property the whole listener exists for: a dialler holding a key this
    /// device does not know gets nothing, and is told nothing.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_unknown_pairing_key_cannot_open_a_session() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir, "listener");
        let source = Arc::new(TestSource::new(
            "listener",
            vec![item("mine", 1_000, "secret of mine", "listener")],
        ));
        let (tx, rx) = watch::channel(false);
        let (addr, _code) = listening(&node, Arc::clone(&source), rx).await;

        let stranger = PairingToken::generate();
        assert!(
            Session::connect(addr, &stranger.psk()).await.is_err(),
            "an unknown key must not complete a handshake"
        );
        assert_eq!(source.snapshot().len(), 1, "nothing may have been taken");
        let _ = tx.send(true);
    }

    /// A device with no pairings at all must not even try to authenticate.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_device_with_no_pairings_serves_nobody() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir, "empty");
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (tx, rx) = watch::channel(false);
        tokio::spawn(listen(
            Arc::clone(&node),
            listener,
            Arc::new(TestSource::new("empty", Vec::new())),
            |_: &str, _: &SyncOutcome| {},
            || {},
            rx,
        ));

        let token = PairingToken::generate();
        assert!(Session::connect(addr, &token.psk()).await.is_err());
        let _ = tx.send(true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_known_peer_session_syncs_both_histories() {
        let a_dir = tempfile::tempdir().unwrap();
        let b_dir = tempfile::tempdir().unwrap();
        let a = Arc::new(Node::new(
            PeerStore::open(
                &a_dir.path().join("a-peers.json"),
                &crate::peers::testutil::KEY,
            )
            .unwrap(),
            None,
            crate::DEFAULT_PORT,
            true,
        ));
        let b = Node::new(
            PeerStore::open(
                &b_dir.path().join("b-peers.json"),
                &crate::peers::testutil::KEY,
            )
            .unwrap(),
            None,
            crate::DEFAULT_PORT,
            true,
        );
        let a_source = Arc::new(TestSource::new(
            "desktop",
            vec![item("from-desktop", 1_000, "desktop item", "desktop")],
        ));
        let b_source = TestSource::new(
            "phone",
            vec![item("from-phone", 2_000, "phone item", "phone")],
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        tokio::spawn(listen(
            Arc::clone(&a),
            listener,
            Arc::clone(&a_source),
            |_: &str, _: &SyncOutcome| {},
            || {},
            shutdown_rx,
        ));

        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        a.peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();
        let b_peer = Peer {
            pairing_id: pairing_id.clone(),
            device_id: None,
            name: "desktop".into(),
            psk: token.psk(),
            last_addr: Some(addr),
            last_seen_ms: 1,
            profile: None,
            profile_observed_at_ms: 0,
        };
        b.peers().upsert(b_peer.clone()).unwrap();
        let outcome = b
            .sync_one(&b_peer, &b_source)
            .await
            .expect("known peers must establish a session");

        assert_eq!(a.peers().len(), 1);
        assert_eq!(b.peers().len(), 1);
        assert_eq!(outcome.peer_device_id, "desktop");
        assert_eq!(
            a.peers()
                .get(&pairing_id)
                .expect("server peer")
                .device_id
                .as_deref(),
            Some("phone")
        );
        assert_eq!(
            b.peers()
                .get(&pairing_id)
                .expect("client peer")
                .device_id
                .as_deref(),
            Some("desktop")
        );
        assert!(a_source
            .snapshot()
            .iter()
            .any(|entry| entry.item_id == "from-phone"));
        assert!(b_source
            .snapshot()
            .iter()
            .any(|entry| entry.item_id == "from-desktop"));

        let _ = shutdown_tx.send(true);
    }

    /// A dialler `b`, a listening `a`, and the pairing they share. `b` holds
    /// the `Peer` a sync pass would have read before the user touched anything.
    async fn dialling_pair(
        a_dir: &tempfile::TempDir,
        b_dir: &tempfile::TempDir,
        shutdown: watch::Receiver<bool>,
    ) -> (Node, Peer) {
        let a = Arc::new(Node::new(
            PeerStore::open(
                &a_dir.path().join("a-peers.json"),
                &crate::peers::testutil::KEY,
            )
            .unwrap(),
            None,
            crate::DEFAULT_PORT,
            true,
        ));
        let b = Node::new(
            PeerStore::open(
                &b_dir.path().join("b-peers.json"),
                &crate::peers::testutil::KEY,
            )
            .unwrap(),
            None,
            crate::DEFAULT_PORT,
            true,
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(listen(
            Arc::clone(&a),
            listener,
            Arc::new(TestSource::new("desktop", Vec::new())),
            |_: &str, _: &SyncOutcome| {},
            || {},
            shutdown,
        ));

        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        a.peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();
        let b_peer = Peer {
            pairing_id,
            device_id: None,
            name: "desktop".into(),
            psk: token.psk(),
            last_addr: Some(addr),
            last_seen_ms: 1,
            profile: None,
            profile_observed_at_ms: 0,
        };
        b.peers().upsert(b_peer.clone()).unwrap();
        (b, b_peer)
    }

    /// A stale peer snapshot cannot start a new transfer after the user
    /// unpairs. The remote device may still retain its half, but this node must
    /// neither authenticate a fresh session nor write the stale pairing back.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_session_that_outlives_an_unpair_does_not_restore_the_device() {
        let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (b, b_peer) = dialling_pair(&a_dir, &b_dir, shutdown_rx).await;
        let source = TestSource::new("phone", Vec::new());

        // The click lands while the pass holds `b_peer`.
        assert!(b.unpair(&b_peer.pairing_id).expect("unpair"));

        assert_eq!(b.sync_one(&b_peer, &source).await, Err(NodeError::NoPeer));

        assert!(
            b.peers().get(&b_peer.pairing_id).is_none(),
            "the unpaired device came back"
        );
        assert!(b.peers().list().is_empty());
        assert!(
            b.peers().psks().is_empty(),
            "the key must not authenticate here again"
        );

        let _ = shutdown_tx.send(true);
    }

    /// Revocation also rejects a peer snapshot that was read before the click.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_session_that_outlives_a_revoke_does_not_restore_the_device() {
        let (a_dir, b_dir) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (b, b_peer) = dialling_pair(&a_dir, &b_dir, shutdown_rx).await;
        let source = TestSource::new("phone", Vec::new());

        assert!(b
            .revoke(&b_peer.pairing_id, crate::now_ms())
            .expect("revoke"));

        assert_eq!(b.sync_one(&b_peer, &source).await, Err(NodeError::NoPeer));

        assert!(b.peers().get(&b_peer.pairing_id).is_none());
        assert!(b.peers().psks().is_empty());

        let _ = shutdown_tx.send(true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_listener_stops_on_shutdown() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir, "stopper");
        let (tx, rx) = watch::channel(false);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let task = tokio::spawn(listen(
            node,
            listener,
            Arc::new(TestSource::new("stopper", Vec::new())),
            |_: &str, _: &SyncOutcome| {},
            || {},
            rx,
        ));
        tx.send(true).unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await
            .expect("the listener must observe shutdown")
            .expect("no panic");
    }

    #[test]
    fn binding_the_peer_port_listens_on_every_interface() {
        let listener = bind(0).expect("bind an ephemeral port");
        assert!(listener.local_addr().unwrap().ip().is_unspecified());
    }

    #[tokio::test]
    async fn finished_session_tasks_are_drained_from_the_join_set() {
        let mut sessions = tokio::task::JoinSet::new();
        for _ in 0..8 {
            sessions.spawn(async {});
        }
        for _ in 0..20 {
            if sessions.is_empty() {
                break;
            }
            tokio::task::yield_now().await;
            while sessions.try_join_next().is_some() {}
        }
        assert!(
            sessions.is_empty(),
            "completed JoinSet tasks must be joinable so they do not leak"
        );
    }
}
