//! The outbound half: consuming a pairing code, and syncing with a peer.
//!
//! Both end in [`run_session`], so a pairing proves itself by running exactly
//! the session every later round runs.

use std::net::SocketAddr;

use tokio::net::lookup_host;
use tracing::{debug, info, warn};

use super::channel::{NoiseChannel, SESSION_TIMEOUT};
use super::{Node, NodeError};
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
        if outcome.peer_device_id == source.device_id() {
            self.unpair(&peer.pairing_id)?;
            return Err(NodeError::SelfPairing);
        }
        self.record_cursor(&peer.pairing_id, &outcome);
        self.record_authenticated_profile(&peer.pairing_id, outcome.peer_profile.as_ref());
        self.touch_peer(
            peer,
            outcome.peer_listen_addr.or(Some(addr)),
            Some(&outcome.peer_device_name),
        );
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
    use crate::peers::PeerStore;
    use crate::sync::testutil::TestSource;

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
            last_seen_ms: 0,
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
}
