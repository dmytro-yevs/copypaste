use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::watch;

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
            Ok(Ok(())) if persist_bilaterally(self, &mut session, &peer).await => {
                self.republish();
                (PairingPhase::Confirmed, None)
            }
            Ok(Ok(())) => (PairingPhase::Failed, Some(NodeError::PeerStore)),
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
    let advertised = listen_addr.and_then(|addr| addr.parse().ok());
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

async fn persist_bilaterally(node: &Node, session: &mut Session, peer: &Peer) -> bool {
    let stored = node.peers.upsert(peer.clone()).is_ok();
    if session
        .send(&PairingMessage::Stored { ok: stored })
        .await
        .is_err()
    {
        if stored {
            let _ = node.peers.remove(&peer.pairing_id);
        }
        return false;
    }
    let remote = session.recv::<PairingMessage>().await;
    let both = stored && matches!(remote, Ok(Some(PairingMessage::Stored { ok: true })));
    if stored && !both {
        let _ = node.peers.remove(&peer.pairing_id);
    }
    both
}
