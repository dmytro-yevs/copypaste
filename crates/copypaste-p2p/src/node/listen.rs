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
use super::Node;
use crate::sync::{run_responder, SyncOutcome, SyncSource};
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
pub async fn listen<S, F>(
    node: Arc<Node>,
    listener: TcpListener,
    source: Arc<S>,
    on_session: F,
    mut shutdown: watch::Receiver<bool>,
) where
    S: SyncSource + Send + Sync + 'static,
    F: Fn(&str, &SyncOutcome) + Send + Sync + Clone + 'static,
{
    match listener.local_addr() {
        Ok(addr) => node.set_listen_addr(addr),
        Err(error) => warn!(error = %error, "could not determine the peer listener address"),
    }
    info!(port = node.port(), "peer listener started");
    let mut sessions = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, addr)) => {
                    let Ok(permit) = Arc::clone(&node.sessions).try_acquire_owned() else {
                        debug!(%addr, "too many peer sessions in flight; dropping the connection");
                        continue;
                    };
                    let node = Arc::clone(&node);
                    let source = Arc::clone(&source);
                    let on_session = on_session.clone();
                    sessions.spawn(async move {
                        let _permit = permit;
                        serve_peer(&node, stream, addr, source.as_ref(), &on_session).await;
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
async fn serve_peer<S, F>(
    node: &Node,
    stream: TcpStream,
    addr: SocketAddr,
    source: &S,
    on_session: &F,
) where
    S: SyncSource,
    F: Fn(&str, &SyncOutcome),
{
    let candidates = node.peers().psks();
    if candidates.is_empty() {
        debug!(%addr, "a peer connected but this device has no pairings");
        return;
    }

    let (session, pairing_id) = match Session::accept_any(stream, &candidates).await {
        Ok(accepted) => accepted,
        Err(e) => {
            debug!(%addr, error = %e, "inbound peer handshake failed");
            return;
        }
    };

    let mut channel = NoiseChannel::new(session);
    let listen_addr = node.listen_addr();
    let outcome = tokio::time::timeout(
        SESSION_TIMEOUT,
        run_responder(&mut channel, source, listen_addr.as_deref()),
    )
    .await;
    channel.close().await;

    match outcome {
        Ok(Ok(outcome)) => {
            if outcome.peer_device_id == source.device_id() {
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
            if let Some(peer) = node.peers().get(&pairing_id) {
                node.touch_peer(
                    &peer,
                    outcome.peer_listen_addr,
                    Some(&outcome.peer_device_name),
                );
            }
            on_session(&pairing_id, &outcome);
        }
        Ok(Err(e)) => warn!(%pairing_id, error = %e, "peer sync session failed"),
        Err(_) => warn!(%pairing_id, "peer sync session timed out"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::{Peer, PeerStore};
    use crate::sync::testutil::{item, TestSource};
    use crate::transport::PairingToken;

    fn node(dir: &tempfile::TempDir, name: &str) -> Arc<Node> {
        let peers = PeerStore::open(&dir.path().join(format!("{name}-peers.json"))).unwrap();
        Arc::new(Node::new(peers, None, 0))
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
                name: "dialler".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 0,
            })
            .unwrap();
        tokio::spawn(listen(
            Arc::clone(node),
            listener,
            source,
            |_: &str, _: &SyncOutcome| {},
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
            rx,
        ));

        let token = PairingToken::generate();
        assert!(Session::connect(addr, &token.psk()).await.is_err());
        let _ = tx.send(true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_pairing_code_completes_a_session_and_syncs_both_histories() {
        let a_dir = tempfile::tempdir().unwrap();
        let b_dir = tempfile::tempdir().unwrap();
        let a = Arc::new(Node::new(
            PeerStore::open(&a_dir.path().join("a-peers.json")).unwrap(),
            None,
            crate::DEFAULT_PORT,
        ));
        let b = Node::new(
            PeerStore::open(&b_dir.path().join("b-peers.json")).unwrap(),
            None,
            crate::DEFAULT_PORT,
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
            shutdown_rx,
        ));

        let pairing = a.pair_create("phone").unwrap();
        let accepted = b
            .pair_accept(&pairing.code, &addr.to_string(), &b_source)
            .await
            .expect("the code and listener must establish a pairing");

        assert_eq!(a.peers().len(), 1);
        assert_eq!(b.peers().len(), 1);
        assert_eq!(accepted.peer.pairing_id, pairing.pairing_id);
        assert!(accepted.peer.last_addr.is_some());
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
}
