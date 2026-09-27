//! The outbound half: consuming a pairing code, and syncing with a peer.
//!
//! Both end in [`run_session`], so a pairing proves itself by running exactly
//! the session every later round runs.

use std::net::SocketAddr;

use tokio::net::lookup_host;
use tracing::{debug, info, warn};

use super::channel::{NoiseChannel, SESSION_TIMEOUT};
use super::{Node, NodeError, SyncCycle};
use crate::peers::Peer;
use crate::protocol::ProtocolError;
use crate::sync::{run_initiator, SyncCursor, SyncError, SyncOutcome, SyncSource};
use crate::transport::Session;

impl Node {
    /// Sync with one peer, start to finish.
    ///
    /// The last address that worked, else whatever discovery has seen. Both are
    /// hints: the handshake is what actually proves who is on the other end.
    pub async fn sync_one<S: SyncSource>(
        &self,
        peer: &Peer,
        source: &S,
    ) -> Result<SyncOutcome, NodeError> {
        self.sync_one_in_cycle(peer, source, &SyncCycle::new())
            .await
    }

    pub async fn sync_one_in_cycle<S: SyncSource>(
        &self,
        peer: &Peer,
        source: &S,
        cycle: &SyncCycle,
    ) -> Result<SyncOutcome, NodeError> {
        let cancel = cycle.cancel_token();
        let addr = peer
            .last_addr
            .or_else(|| self.find(&peer.pairing_id).map(|found| found.addr))
            .ok_or(NodeError::NoAddress)?;

        let session = match Session::connect(addr, &peer.psk).await {
            Ok(session) => session,
            Err(e) => {
                debug!(pairing_id = %peer.pairing_id, error = %e, "could not reach a peer");
                return Err(NodeError::Handshake);
            }
        };

        let cursor = self.cursors().get(&peer.pairing_id);
        let outcome = self.run_session(session, source, cursor).await?;
        #[cfg(test)]
        cycle.wait_before_commit().await;
        if outcome.peer_device_id == source.device_id() {
            self.unpair(&peer.pairing_id)?;
            return Err(NodeError::SelfPairing);
        }
        let committed = cycle.commit(&cancel, || -> Result<(), NodeError> {
            self.record_cursor(&peer.pairing_id, &outcome);
            self.record_authenticated_profile(&peer.pairing_id, outcome.peer_profile.as_ref());
            self.touch_peer(
                peer,
                outcome.peer_listen_addr.or(Some(addr)),
                Some(&outcome.peer_device_name),
            );
            Ok(())
        });
        match committed {
            Some(Ok(())) => {}
            Some(Err(error)) => return Err(error),
            None => return Err(NodeError::Session),
        }
        info!(
            pairing_id = %peer.pairing_id,
            sent = outcome.stats.sent,
            received = outcome.stats.received,
            skipped = outcome.stats.skipped,
            "synced with a peer"
        );
        Ok(outcome)
    }

    /// Drive the initiating half of one session over an established channel.
    ///
    /// The whole session is bounded, on top of the channel's per-read deadline:
    /// a peer that answers every message on time but never runs out of things
    /// to say still has to finish inside [`SESSION_TIMEOUT`].
    async fn run_session<S: SyncSource>(
        &self,
        session: Session,
        source: &S,
        cursor: SyncCursor,
    ) -> Result<SyncOutcome, NodeError> {
        let mut channel = NoiseChannel::new(session);
        let listen_addr = self.listen_addr();
        // The wait for the peer's close is inside the same budget on purpose:
        // see `NoiseChannel::wait_for_close` for why a session is not over when
        // `run_initiator` returns.
        let outcome = tokio::time::timeout(SESSION_TIMEOUT, async {
            let outcome =
                run_initiator(&mut channel, source, listen_addr.as_deref(), cursor).await?;
            channel.wait_for_close().await;
            Ok::<_, SyncError>(outcome)
        })
        .await;
        channel.close().await;

        match outcome {
            Ok(Ok(outcome)) => Ok(outcome),
            Ok(Err(e)) => {
                warn!(error = %e, "sync session failed");
                Err(session_error(&e))
            }
            Err(_) => Err(NodeError::Timeout),
        }
    }
}

fn session_error(e: &SyncError) -> NodeError {
    match e {
        SyncError::Protocol(ProtocolError::VersionMismatch { .. }) => NodeError::PeerVersion,
        _ => NodeError::Session,
    }
}

/// Resolve `host:port`, preferring IPv4.
///
/// A hostname is allowed — `copypaste.local:47654` is exactly what discovery
/// would have given the user — so this is a real lookup rather than a
/// `SocketAddr` parse.
pub(super) async fn resolve(addr: &str) -> Option<SocketAddr> {
    let resolved: Vec<SocketAddr> = lookup_host(addr).await.ok()?.collect();
    resolved
        .iter()
        .find(|addr| addr.is_ipv4())
        .or(resolved.first())
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::{Peer, PeerStore};
    use crate::sync::testutil::{item, TestSource};
    use crate::transport::PairingToken;
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::sync::watch;

    fn node(dir: &tempfile::TempDir) -> Node {
        let peers = PeerStore::open(&dir.path().join("peers.json")).unwrap();
        Node::new(peers, None, 0, true)
    }

    /// A peer that has never been reached and is not on the network fails on
    /// its own account, before any socket is opened.
    #[tokio::test]
    async fn a_peer_with_no_known_address_fails_before_it_dials() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let source = TestSource::new("me", Vec::new());
        let peer = Peer {
            pairing_id: "ffffffffffffffffffffffffffffffff".into(),
            name: "unreachable".into(),
            psk: [3u8; crate::transport::TOKEN_LEN],
            last_addr: None,
            last_seen_ms: 1,
        };

        let err = node.sync_one(&peer, &source).await.expect_err("no address");
        assert_eq!(err, NodeError::NoAddress);
        assert!(!err.is_client_error(), "this is not the caller's mistake");
    }

    #[tokio::test]
    async fn an_address_without_a_port_does_not_resolve() {
        assert!(resolve("127.0.0.1").await.is_none());
        assert_eq!(
            resolve("127.0.0.1:47654").await,
            Some("127.0.0.1:47654".parse().unwrap())
        );
    }

    #[tokio::test]
    async fn a_cancelled_cycle_never_commits_a_completed_sessions_cursor_or_profile() {
        let a_dir = tempfile::tempdir().unwrap();
        let b_dir = tempfile::tempdir().unwrap();
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        let a = Arc::new(node(&a_dir));
        a.peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                name: "b".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
            })
            .unwrap();
        let b = node(&b_dir);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let peer = Peer {
            pairing_id: pairing_id.clone(),
            name: "a".into(),
            psk: token.psk(),
            last_addr: Some(addr),
            last_seen_ms: 1,
        };
        b.peers().upsert(peer.clone()).unwrap();
        let source_a = Arc::new(TestSource::new(
            "a",
            vec![item("item", 1_000, "payload", "a")],
        ));
        let source_b = TestSource::new("b", Vec::new());
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let listener_task = tokio::spawn(crate::node::listen(
            Arc::clone(&a),
            listener,
            source_a,
            |_pairing_id, _outcome| {},
            shutdown_rx,
        ));
        let cycle = SyncCycle::new();
        let pause = cycle.pause_before_commit();
        let session_cycle = cycle.clone();
        let session = async { b.sync_one_in_cycle(&peer, &source_b, &session_cycle).await };
        tokio::pin!(session);
        tokio::select! {
            result = &mut session => panic!("session completed early: {result:?}"),
            () = pause.wait_entered() => {},
        }

        assert_eq!(
            source_b.snapshot().len(),
            1,
            "the completed session lost its payload"
        );
        cycle.set_enabled(false);
        pause.release();
        assert_eq!(session.await, Err(NodeError::Session));
        assert_eq!(b.cursors().get(&pairing_id), SyncCursor::default());
        assert!(b.authenticated_profile(&pairing_id).is_none());
        assert_eq!(b.peers().get(&pairing_id).unwrap().last_seen_ms, 1);

        cycle.set_enabled(true);
        let replay = b.sync_one_in_cycle(&peer, &source_b, &cycle).await.unwrap();
        assert_eq!(
            replay.stats.received, 0,
            "the applied payload was duplicated"
        );
        assert_eq!(source_b.snapshot().len(), 1);
        assert!(b.cursors().get(&pairing_id).since_ms > 0);
        let _ = shutdown_tx.send(true);
        listener_task.await.unwrap();
    }
}
