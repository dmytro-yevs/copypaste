use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tracing::debug;

use super::dial::resolve;
use super::pairing::{
    LocalDecision, PairingInvite, PairingPeer, PairingPhase, PairingRole, PairingStatus,
    PAIRING_CONFIRM_TIMEOUT,
};
use super::{placeholder_name, Node, NodeError};
use crate::peers::{Peer, MAX_PAIRINGS};
use crate::protocol::{MAX_DEVICE_NAME_BYTES, MAX_ID_BYTES, MAX_LISTEN_ADDR_BYTES};
use crate::sync::SyncSource;
use crate::transport::{PairingToken, PskCandidate, Session, TOKEN_LEN};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum PairingMessage {
    Hello {
        protocol_version: u32,
        device_id: String,
        device_name: String,
        listen_addr: Option<String>,
    },
    Decision {
        accept: bool,
    },
    Stored {
        ok: bool,
    },
}

const PAIRING_PROTOCOL_VERSION: u32 = 1;

impl Node {
    pub fn pair_create_invite(&self) -> Result<PairingInvite, NodeError> {
        if self.peers.usable_count() >= MAX_PAIRINGS {
            return Err(NodeError::TooManyPairings);
        }
        let invite = self.pairing.create_invite(self.listen_addr())?;
        self.republish();
        Ok(invite)
    }

    pub async fn pair_join<S: SyncSource>(
        self: &Arc<Self>,
        code: &str,
        addr: &str,
        source: &S,
    ) -> Result<PairingStatus, NodeError> {
        let token = PairingToken::parse(code).map_err(|_| NodeError::BadCode)?;
        let pairing_id = token.pairing_id();
        if self.peers.get(&pairing_id).is_none() && self.peers.usable_count() >= MAX_PAIRINGS {
            return Err(NodeError::TooManyPairings);
        }
        let addr = resolve(addr).await.ok_or(NodeError::BadAddress)?;
        let control = self.pairing.begin_join(pairing_id.clone())?;
        let session = Session::connect(addr, &token.psk()).await.map_err(|_| {
            self.pairing.finish(
                &pairing_id,
                PairingPhase::Failed,
                Some(NodeError::Handshake),
            );
            NodeError::Handshake
        })?;
        let identity = local_identity(self, source);
        let (session, peer, shown) = match establish(
            session,
            identity,
            pairing_id.clone(),
            token.psk(),
            PairingRole::Initiator,
        )
        .await
        {
            Ok(established) => established,
            Err(error) => {
                self.pairing
                    .finish(&pairing_id, PairingPhase::Failed, Some(error));
                return Err(error);
            }
        };
        self.pairing
            .awaiting(&pairing_id, session.pairing_sas(), shown);
        let node = Arc::clone(self);
        tokio::spawn(async move {
            node.complete_pairing(session, peer, control).await;
        });
        Ok(self.pairing.progress())
    }

    pub fn pair_progress(&self) -> PairingStatus {
        let was_active = self.pairing.is_active();
        let status = self.pairing.progress();
        if was_active && !self.pairing.is_active() {
            self.republish();
        }
        status
    }

    pub fn pair_confirm(&self, accept: bool) -> Result<PairingStatus, NodeError> {
        self.pairing.confirm(accept)
    }

    pub fn pair_cancel(&self) -> PairingStatus {
        let status = self.pairing.cancel();
        self.republish();
        status
    }

    pub(super) fn pairing_candidate(&self) -> Option<PskCandidate> {
        let was_active = self.pairing.is_active();
        let candidate = self.pairing.candidate();
        if was_active && candidate.is_none() && !self.pairing.is_active() {
            self.republish();
        }
        candidate
    }

    pub(super) async fn accept_pairing<S: SyncSource>(
        self: &Arc<Self>,
        session: Session,
        pairing_id: &str,
        source: &S,
    ) -> Result<(), NodeError> {
        let (psk, control) = self.pairing.begin_responder(pairing_id)?;
        let identity = local_identity(self, source);
        let established = establish(
            session,
            identity,
            pairing_id.to_string(),
            *psk,
            PairingRole::Responder,
        )
        .await;
        let (session, peer, shown) = match established {
            Ok(established) => established,
            Err(error) => {
                self.pairing
                    .finish(pairing_id, PairingPhase::Failed, Some(error));
                self.republish();
                return Err(error);
            }
        };
        self.pairing
            .awaiting(pairing_id, session.pairing_sas(), shown);
        self.complete_pairing(session, peer, control).await;
        Ok(())
    }

    async fn complete_pairing(
        &self,
        mut session: Session,
        peer: Peer,
        control: watch::Receiver<LocalDecision>,
    ) {
        let pairing_id = peer.pairing_id.clone();
        let decision = tokio::time::timeout(
            PAIRING_CONFIRM_TIMEOUT,
            exchange_decisions(&mut session, control),
        )
        .await;
        let (phase, error) = match decision {
            Err(_) => (PairingPhase::TimedOut, None),
            Ok(Err(DecisionEnd::Rejected)) => (PairingPhase::Rejected, None),
            Ok(Err(DecisionEnd::Cancelled)) => (PairingPhase::Cancelled, None),
            Ok(Err(DecisionEnd::Failed)) => (PairingPhase::Failed, Some(NodeError::Session)),
            Ok(Ok(())) if !self.pairing.begin_commit(&pairing_id) => {
                (PairingPhase::Cancelled, None)
            }
            Ok(Ok(())) => match persist_bilaterally(self, &mut session, &peer).await {
                Commit::Both => {
                    self.republish();
                    (PairingPhase::Confirmed, None)
                }
                Commit::PeerSilent => (PairingPhase::TimedOut, Commit::PeerSilent.error()),
                other => (PairingPhase::Failed, other.error()),
            },
        };
        self.pairing.finish(&pairing_id, phase, error);
        self.republish();
        let _ = session.close().await;
    }
}

struct LocalIdentity {
    device_id: String,
    device_name: String,
    listen_addr: Option<String>,
}

fn local_identity<S: SyncSource>(node: &Node, source: &S) -> LocalIdentity {
    LocalIdentity {
        device_id: source.device_id(),
        device_name: source.device_name(),
        listen_addr: node.listen_addr(),
    }
}

async fn establish(
    mut session: Session,
    local: LocalIdentity,
    pairing_id: String,
    psk: [u8; TOKEN_LEN],
    role: PairingRole,
) -> Result<(Session, Peer, PairingPeer), NodeError> {
    session
        .send(&PairingMessage::Hello {
            protocol_version: PAIRING_PROTOCOL_VERSION,
            device_id: local.device_id.clone(),
            device_name: local.device_name,
            listen_addr: local.listen_addr,
        })
        .await
        .map_err(|_| NodeError::Session)?;
    let hello = tokio::time::timeout(Duration::from_secs(10), session.recv::<PairingMessage>())
        .await
        .map_err(|_| NodeError::Timeout)?
        .map_err(|_| NodeError::Session)?
        .ok_or(NodeError::Session)?;
    let PairingMessage::Hello {
        protocol_version,
        device_id,
        device_name,
        listen_addr,
    } = hello
    else {
        return Err(NodeError::Session);
    };
    if protocol_version != PAIRING_PROTOCOL_VERSION
        || device_id.is_empty()
        || device_id.len() > MAX_ID_BYTES
        || device_id == local.device_id
        || device_name.len() > MAX_DEVICE_NAME_BYTES
        || listen_addr
            .as_ref()
            .is_some_and(|addr| addr.len() > MAX_LISTEN_ADDR_BYTES)
    {
        return Err(NodeError::Handshake);
    }
    // The peer's own claim about where to dial it, kept only when it is a
    // usable target — see [`crate::netif::is_dialable`]. The initiator falls
    // back to the socket it actually reached, which is an observation rather
    // than a claim.
    let advertised = listen_addr
        .and_then(|addr| addr.parse::<std::net::SocketAddr>().ok())
        .filter(crate::netif::is_dialable);
    let addr =
        advertised.or_else(|| (role == PairingRole::Initiator).then_some(session.peer_addr()));
    let name = placeholder_name(&device_name);
    let peer = Peer {
        pairing_id: pairing_id.clone(),
        name: name.clone(),
        psk,
        last_addr: addr,
        last_seen_ms: crate::now_ms(),
    };
    let shown = PairingPeer {
        pairing_id,
        device_id,
        name,
        addr,
    };
    Ok((session, peer, shown))
}

enum DecisionEnd {
    Rejected,
    Cancelled,
    Failed,
}

async fn exchange_decisions(
    session: &mut Session,
    mut local: watch::Receiver<LocalDecision>,
) -> Result<(), DecisionEnd> {
    let mut local_accepted = false;
    let mut remote_accepted = false;
    while !local_accepted || !remote_accepted {
        tokio::select! {
            changed = local.changed() => {
                if changed.is_err() {
                    return Err(DecisionEnd::Failed);
                }
                let decision = *local.borrow_and_update();
                match decision {
                    LocalDecision::Pending => {}
                    LocalDecision::Accept => {
                        if !local_accepted {
                            session.send(&PairingMessage::Decision { accept: true })
                                .await.map_err(|_| DecisionEnd::Failed)?;
                            local_accepted = true;
                        }
                    }
                    LocalDecision::Reject => {
                        let _ = session.send(&PairingMessage::Decision { accept: false }).await;
                        return Err(DecisionEnd::Rejected);
                    }
                    LocalDecision::Cancel => {
                        let _ = session.send(&PairingMessage::Decision { accept: false }).await;
                        return Err(DecisionEnd::Cancelled);
                    }
                }
            }
            message = session.recv::<PairingMessage>() => {
                match message {
                    Ok(Some(PairingMessage::Decision { accept: true })) => {
                        remote_accepted = true;
                    }
                    Ok(Some(PairingMessage::Decision { accept: false })) => {
                        return Err(DecisionEnd::Rejected);
                    }
                    _ => return Err(DecisionEnd::Failed),
                }
            }
        }
    }
    Ok(())
}

/// The commit exchange is the one place a silent peer costs more than a
/// session.
///
/// By then the ceremony holds the commit lock, the ceremony slot and (on the
/// responder) an inbound session permit, and [`PAIRING_CONFIRM_TIMEOUT`] covers
/// only the decisions *before* it. With no deadline here a peer that accepted
/// and then stopped writing kept all three for good: no cancel, no new invite,
/// no pairing again without a restart.
///
/// Short on purpose: the far side has already decided.
const PAIRING_COMMIT_TIMEOUT: Duration = Duration::from_secs(15);

/// Store the peer on both devices, or on neither.
///
/// `false` rolls this side back — an unacknowledged pairing is worse than no
/// pairing, because the other device will not answer a key it never kept.
async fn persist_bilaterally(node: &Node, session: &mut Session, peer: &Peer) -> Commit {
    let mut slot = node.peers.snapshot(&peer.pairing_id);
    let stored = node.peers.upsert_from(&mut slot, peer.clone()).is_ok();
    let acknowledged = tokio::time::timeout(PAIRING_COMMIT_TIMEOUT, async {
        session
            .send(&PairingMessage::Stored { ok: stored })
            .await
            .map_err(|_| ())?;
        match session.recv::<PairingMessage>().await {
            Ok(Some(PairingMessage::Stored { ok })) => Ok(ok),
            _ => Err(()),
        }
    })
    .await;

    let outcome = match (stored, &acknowledged) {
        (true, Ok(Ok(true))) => Commit::Both,
        (_, Err(_)) => Commit::PeerSilent,
        (false, _) => Commit::LocalStoreFailed,
        (true, _) => Commit::PeerRefused,
    };
    if outcome == Commit::PeerSilent {
        debug!(
            pairing_id = %peer.pairing_id,
            "a peer accepted the pairing and then stopped answering; rolling it back"
        );
    }
    // Rolled back on every outcome but agreement, through the conditional
    // restore rather than a bare `remove`: an established pairing this ceremony
    // replaced has to come back (B5), and a concurrent session's write must not
    // be discarded to do it. A rollback that could not be written is a failure
    // of its own — the key is out of the candidate list, but the file still
    // names it, and saying "the peer timed out" would hide that (B4).
    if stored && outcome != Commit::Both {
        if let Err(e) = node.peers.restore(&slot) {
            tracing::warn!(
                pairing_id = %peer.pairing_id,
                error = ?e,
                "a pairing rollback could not be written"
            );
            return Commit::RollbackFailed;
        }
    }
    outcome
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Commit {
    Both,
    /// The peer accepted and then stopped writing, or the channel broke.
    PeerSilent,
    PeerRefused,
    LocalStoreFailed,
    /// The pairing was rolled back in memory but the file still names it.
    RollbackFailed,
}

impl Commit {
    /// The failure a user is shown. `Timeout` rather than `PeerStore` for a
    /// silent peer: nothing is wrong with this device's storage, and the two
    /// need different sentences on the pairing screen. A rollback that did not
    /// reach the disk *is* this device's storage failing, and reporting the
    /// peer's outcome instead would leave the user with no reason to look.
    fn error(self) -> Option<NodeError> {
        match self {
            Self::Both => None,
            Self::PeerSilent => Some(NodeError::Timeout),
            Self::PeerRefused | Self::LocalStoreFailed | Self::RollbackFailed => {
                Some(NodeError::PeerStore)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use tokio::net::TcpListener;

    use super::*;
    use crate::discovery::Discovery;
    use crate::node::listen;
    use crate::peers::PeerStore;
    use crate::sync::testutil::TestSource;
    use crate::SyncOutcome;

    fn node(dir: &tempfile::TempDir, name: &str) -> Arc<Node> {
        Arc::new(Node::new(
            PeerStore::open(&dir.path().join(format!("{name}-peers.json"))).unwrap(),
            None::<Discovery>,
            0,
            true,
        ))
    }

    async fn start_listener(
        node: Arc<Node>,
        source: Arc<TestSource>,
    ) -> (SocketAddr, watch::Sender<bool>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown, receiver) = watch::channel(false);
        tokio::spawn(listen(
            node,
            listener,
            source,
            |_: &str, _: &SyncOutcome| {},
            receiver,
        ));
        (addr, shutdown)
    }

    /// Sleeps rather than yielding: under `start_paused` the clock only moves
    /// when the runtime has nothing to run, so a `yield_now` spin would keep it
    /// busy and the deadline under test would never arrive.
    async fn wait_for(node: &Node, phase: PairingPhase) -> PairingStatus {
        tokio::time::timeout(PAIRING_CONFIRM_TIMEOUT * 4, async {
            loop {
                let status = node.pair_progress();
                if status.phase == phase {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("pairing did not reach {phase:?}"))
    }

    /// The pairing protocol driven by hand, so the far side can be left
    /// waiting at exactly the message this test is about.
    ///
    /// Returns after the hello exchange, which is what puts the responder into
    /// `AwaitingConfirmation`. Nothing here reads the far side's messages: they
    /// are a handful of bytes and sit in the socket buffer, and a read would
    /// make this a peer that is still answering.
    async fn handshake_by_hand(addr: SocketAddr, code: &str) -> Session {
        let token = PairingToken::parse(code).unwrap();
        let mut session = Session::connect(addr, &token.psk()).await.unwrap();
        session
            .send(&PairingMessage::Hello {
                protocol_version: PAIRING_PROTOCOL_VERSION,
                device_id: "silent-device".into(),
                device_name: "silent".into(),
                listen_addr: None,
            })
            .await
            .unwrap();
        session.recv::<PairingMessage>().await.unwrap();
        session
    }

    /// Both sides accept, and then this one stops writing before the commit
    /// exchange — the shape that left the responder committing for good.
    async fn accept_then_go_silent(responder: &Node, session: &mut Session) {
        wait_for(responder, PairingPhase::AwaitingConfirmation).await;
        responder.pair_confirm(true).unwrap();
        session
            .send(&PairingMessage::Decision { accept: true })
            .await
            .unwrap();
    }

    /// The whole of the defect. A peer that accepts and then stops writing had
    /// no deadline against it: the responder sat in `persist_bilaterally`
    /// forever, holding the commit lock (so `pair_cancel` refused), the
    /// ceremony slot (so no new invitation could be minted) and the inbound
    /// session permit. The device could not pair again without a restart.
    #[tokio::test(start_paused = true)]
    async fn a_peer_that_stops_answering_after_accepting_releases_the_ceremony() {
        let dir = tempfile::tempdir().unwrap();
        let responder = node(&dir, "responder");
        let source = Arc::new(TestSource::new("responder-id", Vec::new()));
        let (addr, shutdown) = start_listener(Arc::clone(&responder), Arc::clone(&source)).await;

        let invite = responder.pair_create_invite().unwrap();
        let mut peer = handshake_by_hand(addr, &invite.code).await;
        accept_then_go_silent(&responder, &mut peer).await;

        // Bounded, and reported as a peer timeout rather than as this device's
        // storage failing.
        let done = wait_for(&responder, PairingPhase::TimedOut).await;
        assert_eq!(done.error, Some(NodeError::Timeout));

        // Nothing half-written: no peer row, and so no pre-shared key left in
        // the listener's candidates for a pairing the other side never kept.
        assert!(responder.peers().list().is_empty());
        assert!(responder.peers().psks().is_empty());

        // And the device can pair again, without a restart.
        assert_eq!(responder.pair_cancel().phase, PairingPhase::TimedOut);
        let fresh = responder.pair_create_invite().expect("PairingBusy leaked");
        assert_ne!(fresh.pairing_id, invite.pairing_id);

        drop(peer);
        let _ = shutdown.send(true);
    }

    /// The inbound session permit is the second thing the stall held. Four
    /// concurrent sessions is the whole budget, so one stuck ceremony that
    /// never returns is a quarter of the device's ability to serve peers, for
    /// good.
    #[tokio::test(start_paused = true)]
    async fn a_stalled_ceremony_gives_its_inbound_session_permit_back() {
        let dir = tempfile::tempdir().unwrap();
        let responder = node(&dir, "responder");
        let source = Arc::new(TestSource::new("responder-id", Vec::new()));
        let (addr, shutdown) = start_listener(Arc::clone(&responder), Arc::clone(&source)).await;

        let invite = responder.pair_create_invite().unwrap();
        let mut peer = handshake_by_hand(addr, &invite.code).await;
        accept_then_go_silent(&responder, &mut peer).await;
        wait_for(&responder, PairingPhase::TimedOut).await;

        let permits = Arc::clone(&responder.sessions);
        assert_eq!(
            permits.available_permits(),
            crate::node::MAX_CONCURRENT_PEER_SESSIONS,
            "a stalled ceremony kept its inbound session permit"
        );

        drop(peer);
        let _ = shutdown.send(true);
    }

    /// The tentative record is on disk. Until it is, blocking the file would
    /// only fail the write the rollback is meant to undo.
    async fn wait_for_tentative_write(node: &Node) {
        tokio::time::timeout(PAIRING_CONFIRM_TIMEOUT, async {
            while node.peers().psks().is_empty() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the ceremony never wrote its peer");
    }

    /// Renaming over a directory fails on every platform, and the parent still
    /// exists, so the write path cannot recreate its way out of it.
    fn block(path: &std::path::Path) {
        let _ = std::fs::remove_file(path);
        std::fs::create_dir(path).expect("block the path");
    }

    /// B4. The rollback could not be written, so the file still names the
    /// pre-shared key this ceremony repudiated. The key is out of the
    /// listener's candidates immediately — that is what fail-closed means here
    /// — but reporting the peer's timeout would leave the user with nothing to
    /// act on while a live credential sat on disk.
    #[tokio::test(start_paused = true)]
    async fn a_rollback_that_cannot_be_written_fails_the_pairing_rather_than_timing_out() {
        let dir = tempfile::tempdir().unwrap();
        let responder = node(&dir, "responder");
        let source = Arc::new(TestSource::new("responder-id", Vec::new()));
        let (addr, shutdown) = start_listener(Arc::clone(&responder), Arc::clone(&source)).await;

        let invite = responder.pair_create_invite().unwrap();
        let mut peer = handshake_by_hand(addr, &invite.code).await;
        accept_then_go_silent(&responder, &mut peer).await;
        wait_for_tentative_write(&responder).await;
        block(&dir.path().join("responder-peers.json"));

        let done = wait_for(&responder, PairingPhase::Failed).await;
        assert_eq!(
            done.error,
            Some(NodeError::PeerStore),
            "a rollback that did not reach the disk must not read as a peer timeout"
        );
        assert!(
            responder.peers().psks().is_empty(),
            "the repudiated key is still an authentication candidate"
        );

        drop(peer);
        let _ = shutdown.send(true);
    }

    /// B5. Re-pairing a device that is already stored writes over its record
    /// before the other side has agreed. Losing that exchange used to delete the
    /// pairing outright — the working device, its key and all — which is the
    /// data-loss outcome rule 4 rules out. The pre-ceremony record must come
    /// back, and the tentative key must not.
    #[tokio::test(start_paused = true)]
    async fn a_failed_re_pair_restores_the_device_that_was_already_paired() {
        let dir = tempfile::tempdir().unwrap();
        let responder = node(&dir, "responder");
        let source = Arc::new(TestSource::new("responder-id", Vec::new()));
        let (addr, shutdown) = start_listener(Arc::clone(&responder), Arc::clone(&source)).await;

        let invite = responder.pair_create_invite().unwrap();
        let established = Peer {
            pairing_id: invite.pairing_id.clone(),
            name: "the device the user already has".to_string(),
            psk: PairingToken::generate().psk(),
            last_addr: None,
            last_seen_ms: crate::now_ms(),
        };
        let established_psk = established.psk;
        responder.peers().upsert(established).unwrap();

        let mut peer = handshake_by_hand(addr, &invite.code).await;
        accept_then_go_silent(&responder, &mut peer).await;
        wait_for(&responder, PairingPhase::TimedOut).await;

        let back = responder
            .peers()
            .get(&invite.pairing_id)
            .expect("the established pairing was deleted by a failed re-pair");
        assert_eq!(back.name, "the device the user already has");
        assert!(back.psk_matches(&established_psk));
        let candidates = responder.peers().psks();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].psk, established_psk);

        drop(peer);
        let _ = shutdown.send(true);
    }

    /// A peer may claim any `listen_addr` it likes over an authenticated
    /// channel, and the claim is written into the peer file as the address
    /// every later round dials. `0.0.0.0:0` is storable and not dialable.
    #[test]
    fn a_peer_advertised_address_that_cannot_be_dialled_is_not_stored() {
        for hostile in [
            "0.0.0.0:47654",
            "127.0.0.1:0",
            "255.255.255.255:47654",
            "224.0.0.1:47654",
            "169.254.1.1:47654",
            "[::]:47654",
            "[ff02::1]:47654",
        ] {
            let addr: SocketAddr = hostile.parse().unwrap();
            assert!(
                !crate::netif::is_dialable(&addr),
                "{hostile} would have replaced an address that worked"
            );
        }
        assert!(crate::netif::is_dialable(
            &"192.168.1.9:47654".parse().unwrap()
        ));
        // Loopback stays dialable: two daemons on one host is a supported
        // topology and `127.0.0.1` is the address they exchange.
        assert!(crate::netif::is_dialable(
            &"127.0.0.1:47654".parse().unwrap()
        ));
    }
}
