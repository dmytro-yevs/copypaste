//! The outbound half: consuming a pairing code, and syncing with a peer.
//!
//! Both end in [`run_session`], so a pairing proves itself by running exactly
//! the session every later round runs.

use std::net::SocketAddr;

use tokio::net::lookup_host;
use tracing::{debug, info, warn};

use super::channel::{NoiseChannel, SESSION_TIMEOUT};
use super::{placeholder_name, store_error, Node, NodeError};
use crate::peers::{Peer, MAX_PAIRINGS};
use crate::sync::{run_initiator, SyncError, SyncOutcome, SyncSource};
use crate::transport::{PairingToken, Session};

/// A pairing that has been proved by a complete session.
#[derive(Debug)]
pub struct AcceptedPairing {
    pub peer: Peer,
    pub outcome: SyncOutcome,
}

impl Node {
    /// Consume a code from the other device and prove the pairing works.
    ///
    /// The peer is persisted only after a complete session. A code that does
    /// not parse, an address that does not answer, a handshake the other end
    /// refuses, or a session that fails part-way all leave the paired-device
    /// list untouched: a stored pairing means a pairing that has worked at
    /// least once.
    pub async fn pair_accept<S: SyncSource>(
        &self,
        code: &str,
        addr: &str,
        source: &S,
    ) -> Result<AcceptedPairing, NodeError> {
        // No detail, and nothing logged: the input is a secret and saying *how*
        // it was wrong is a hint about what a valid one looks like.
        let token = PairingToken::parse(code).map_err(|_| NodeError::BadCode)?;
        let pairing_id = token.pairing_id();

        // A code this node minted is deliberately present only as an inbound
        // credential. Entering it back into the same device cannot create a
        // peer; discard the pending credential so it cannot leave a phantom
        // self-device behind.
        if self.peers().pending_until(&pairing_id).is_some() {
            self.peers().remove(&pairing_id).map_err(store_error)?;
            self.republish();
            return Err(NodeError::SelfPairing);
        }

        // Checked before dialling as well as at the write. The write is the
        // authority, but finding out after a full sync round that the pairing
        // cannot be stored would mean this device had already sent its history
        // to a peer it then refuses to know.
        if self.peers().get(&pairing_id).is_none() && self.peers().len() >= MAX_PAIRINGS {
            return Err(NodeError::TooManyPairings);
        }

        let addr = resolve(addr).await.ok_or(NodeError::BadAddress)?;

        let session = match Session::connect(addr, &token.psk()).await {
            Ok(session) => session,
            Err(e) => {
                debug!(%pairing_id, error = %e, "pairing handshake failed");
                return Err(NodeError::Handshake);
            }
        };

        let outcome = self.run_session(session, source).await.inspect_err(|_| {
            warn!(%pairing_id, "pairing session failed; not storing the peer");
        })?;

        if outcome.peer_device_id == source.device_id() {
            return Err(NodeError::SelfPairing);
        }

        let peer = Peer {
            pairing_id: pairing_id.clone(),
            name: placeholder_name(&outcome.peer_device_name),
            psk: token.psk(),
            last_addr: outcome.peer_listen_addr.or(Some(addr)),
            last_seen_ms: crate::now_ms(),
        };
        self.peers().upsert(peer.clone()).map_err(store_error)?;
        self.republish();

        info!(
            %pairing_id,
            sent = outcome.stats.sent,
            received = outcome.stats.received,
            "paired with a device"
        );
        Ok(AcceptedPairing { peer, outcome })
    }

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

        let outcome = self.run_session(session, source).await?;
        if outcome.peer_device_id == source.device_id() {
            self.unpair(&peer.pairing_id)?;
            return Err(NodeError::SelfPairing);
        }
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
    ) -> Result<SyncOutcome, NodeError> {
        let mut channel = NoiseChannel::new(session);
        let listen_addr = self.listen_addr();
        // The wait for the peer's close is inside the same budget on purpose:
        // see `NoiseChannel::wait_for_close` for why a session is not over when
        // `run_initiator` returns.
        let outcome = tokio::time::timeout(SESSION_TIMEOUT, async {
            let outcome = run_initiator(&mut channel, source, listen_addr.as_deref()).await?;
            channel.wait_for_close().await;
            Ok::<_, SyncError>(outcome)
        })
        .await;
        channel.close().await;

        match outcome {
            Ok(Ok(outcome)) => Ok(outcome),
            Ok(Err(e)) => {
                warn!(error = %e, "sync session failed");
                Err(NodeError::Session)
            }
            Err(_) => Err(NodeError::Timeout),
        }
    }
}

/// Resolve `host:port`, preferring IPv4.
///
/// A hostname is allowed — `copypaste.local:47654` is exactly what discovery
/// would have given the user — so this is a real lookup rather than a
/// `SocketAddr` parse.
async fn resolve(addr: &str) -> Option<SocketAddr> {
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
        Node::new(peers, None, 0)
    }

    #[tokio::test]
    async fn a_malformed_code_is_rejected_without_touching_the_peer_list() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let source = TestSource::new("me", Vec::new());

        let err = node
            .pair_accept("not-a-code", "127.0.0.1:1", &source)
            .await
            .expect_err("a malformed code must be refused");
        assert_eq!(err, NodeError::BadCode);
        assert_eq!(node.peers().len(), 0);
    }

    #[tokio::test]
    async fn entering_our_own_pending_code_removes_it_without_dialling() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let source = TestSource::new("me", Vec::new());
        let pairing = node.pair_create("other device").expect("mint pairing");

        let err = node
            .pair_accept(&pairing.code, "127.0.0.1:1", &source)
            .await
            .expect_err("a device must not pair with itself");

        assert_eq!(err, NodeError::SelfPairing);
        assert!(node.peers().get(&pairing.pairing_id).is_none());
        assert!(node.peers().psks().is_empty());
    }

    /// A well-formed code for a device that is not listening must not leave a
    /// half-made pairing behind.
    #[tokio::test]
    async fn a_failed_handshake_stores_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let source = TestSource::new("me", Vec::new());
        let code = PairingToken::generate().to_code();

        // Port 1 on loopback: nothing is listening, so the connect fails fast.
        let err = node
            .pair_accept(&code, "127.0.0.1:1", &source)
            .await
            .expect_err("an unanswered dial must be refused");
        assert_eq!(err, NodeError::Handshake);
        assert_eq!(node.peers().len(), 0, "a pairing was persisted anyway");
    }

    #[tokio::test]
    async fn an_unresolvable_address_is_a_client_error() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let source = TestSource::new("me", Vec::new());
        let code = PairingToken::generate().to_code();

        let err = node
            .pair_accept(&code, "no-port-here", &source)
            .await
            .expect_err("an address with no port must be refused");
        assert_eq!(err, NodeError::BadAddress);
        assert!(err.is_client_error());
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

    /// The cap is checked before the dial, so a pairing this device is going to
    /// refuse never gets a sync round first.
    #[tokio::test]
    async fn accepting_past_the_pairing_cap_is_refused_before_anything_is_sent() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let source = TestSource::new("me", Vec::new());
        for i in 0..MAX_PAIRINGS {
            node.pair_create(&format!("device-{i}"))
                .expect("up to the cap");
        }

        // Nothing is listening on port 1, so a dial would fail with
        // `Handshake`. Getting `TooManyPairings` is what proves the cap was
        // checked before the socket was opened.
        let code = PairingToken::generate().to_code();
        let err = node
            .pair_accept(&code, "127.0.0.1:1", &source)
            .await
            .expect_err("past the cap must be refused");
        assert_eq!(err, NodeError::TooManyPairings);
        assert_eq!(node.peers().len(), MAX_PAIRINGS);
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
