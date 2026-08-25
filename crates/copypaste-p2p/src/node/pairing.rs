use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::watch;
use zeroize::Zeroizing;

use super::NodeError;
use crate::transport::{PairingToken, PskCandidate, TOKEN_LEN};

pub const PAIRING_INVITE_TTL: Duration = Duration::from_secs(120);
pub const PAIRING_CONFIRM_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingRole {
    Initiator,
    Responder,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingPhase {
    Idle,
    WaitingForPeer,
    Handshaking,
    AwaitingConfirmation,
    Confirmed,
    Rejected,
    Cancelled,
    TimedOut,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingPeer {
    pub pairing_id: String,
    pub device_id: String,
    pub name: String,
    pub addr: Option<SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairingStatus {
    pub pairing_id: Option<String>,
    pub role: Option<PairingRole>,
    pub phase: PairingPhase,
    pub sas: Option<String>,
    pub peer: Option<PairingPeer>,
    pub error: Option<NodeError>,
    pub expires_in_ms: Option<u64>,
}

impl PairingStatus {
    fn idle() -> Self {
        Self {
            pairing_id: None,
            role: None,
            phase: PairingPhase::Idle,
            sas: None,
            peer: None,
            error: None,
            expires_in_ms: None,
        }
    }
}

pub struct PairingInvite {
    pub code: String,
    pub pairing_id: String,
    pub listen_addr: Option<String>,
    pub expires_in_secs: u64,
}

impl std::fmt::Debug for PairingInvite {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PairingInvite")
            .field("pairing_id", &self.pairing_id)
            .field("code", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalDecision {
    Pending,
    Accept,
    Reject,
    Cancel,
    Expire,
}

struct Invitation {
    status: PairingStatus,
    token: PairingToken,
    expires_at: Instant,
    control: watch::Sender<LocalDecision>,
}

struct Active {
    status: PairingStatus,
    control: watch::Sender<LocalDecision>,
    expires_at: Option<Instant>,
    committing: bool,
}

enum PairingState {
    Idle,
    Invited(Invitation),
    Active(Active),
    Terminal(PairingStatus),
}

pub(super) struct PairingManager {
    state: Mutex<PairingState>,
    now: Arc<dyn Fn() -> Instant + Send + Sync>,
}

impl PairingManager {
    pub(super) fn new() -> Self {
        Self::with_clock(Instant::now)
    }

    fn with_clock(now: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        Self {
            state: Mutex::new(PairingState::Idle),
            now: Arc::new(now),
        }
    }

    fn now(&self) -> Instant {
        (self.now)()
    }

    pub(super) fn create_invite(
        &self,
        listen_addr: Option<String>,
    ) -> Result<PairingInvite, NodeError> {
        let mut state = self.lock();
        if matches!(*state, PairingState::Invited(_) | PairingState::Active(_)) {
            return Err(NodeError::PairingBusy);
        }
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        let code = token.to_code();
        let (control, _) = watch::channel(LocalDecision::Pending);
        *state = PairingState::Invited(Invitation {
            status: PairingStatus {
                pairing_id: Some(pairing_id.clone()),
                role: Some(PairingRole::Responder),
                phase: PairingPhase::WaitingForPeer,
                sas: None,
                peer: None,
                error: None,
                expires_in_ms: None,
            },
            token,
            expires_at: self.now() + PAIRING_INVITE_TTL,
            control,
        });
        Ok(PairingInvite {
            code,
            pairing_id,
            listen_addr,
            expires_in_secs: PAIRING_INVITE_TTL.as_secs(),
        })
    }

    pub(super) fn candidate(&self) -> Option<PskCandidate> {
        let mut state = self.lock();
        expire_due(&mut state, self.now());
        match &*state {
            PairingState::Invited(invite) => Some(PskCandidate {
                pairing_id: invite.status.pairing_id.clone().unwrap_or_default(),
                psk: invite.token.psk(),
            }),
            _ => None,
        }
    }

    pub(super) fn begin_responder(
        &self,
        pairing_id: &str,
    ) -> Result<(Zeroizing<[u8; TOKEN_LEN]>, watch::Receiver<LocalDecision>), NodeError> {
        let mut state = self.lock();
        let previous = std::mem::replace(&mut *state, PairingState::Idle);
        match previous {
            PairingState::Invited(invite)
                if invite.status.pairing_id.as_deref() == Some(pairing_id)
                    && self.now() < invite.expires_at =>
            {
                let psk = Zeroizing::new(invite.token.psk());
                let receiver = invite.control.subscribe();
                *state = PairingState::Active(Active {
                    status: PairingStatus {
                        phase: PairingPhase::Handshaking,
                        ..invite.status
                    },
                    control: invite.control,
                    expires_at: None,
                    committing: false,
                });
                Ok((psk, receiver))
            }
            other => {
                *state = other;
                Err(NodeError::NoPairing)
            }
        }
    }

    pub(super) fn begin_join(
        &self,
        pairing_id: String,
    ) -> Result<watch::Receiver<LocalDecision>, NodeError> {
        let mut state = self.lock();
        if matches!(*state, PairingState::Invited(_) | PairingState::Active(_)) {
            return Err(NodeError::PairingBusy);
        }
        let (control, receiver) = watch::channel(LocalDecision::Pending);
        *state = PairingState::Active(Active {
            status: PairingStatus {
                pairing_id: Some(pairing_id),
                role: Some(PairingRole::Initiator),
                phase: PairingPhase::Handshaking,
                sas: None,
                peer: None,
                error: None,
                expires_in_ms: None,
            },
            control,
            expires_at: None,
            committing: false,
        });
        Ok(receiver)
    }

    pub(super) fn awaiting(&self, pairing_id: &str, sas: String, peer: PairingPeer) {
        let mut state = self.lock();
        if let PairingState::Active(active) = &mut *state {
            if active.status.pairing_id.as_deref() == Some(pairing_id) {
                active.status.phase = PairingPhase::AwaitingConfirmation;
                active.status.sas = Some(sas);
                active.status.peer = Some(peer);
                active.expires_at = Some(self.now() + PAIRING_CONFIRM_TIMEOUT);
            }
        }
    }

    pub(super) fn finish(&self, pairing_id: &str, phase: PairingPhase, error: Option<NodeError>) {
        let mut state = self.lock();
        let current = match &*state {
            PairingState::Active(active)
                if active.status.pairing_id.as_deref() == Some(pairing_id) =>
            {
                Some(active.status.clone())
            }
            PairingState::Terminal(status) if status.pairing_id.as_deref() == Some(pairing_id) => {
                return
            }
            _ => None,
        };
        if let Some(mut status) = current {
            status.phase = phase;
            status.error = error;
            status.sas = None;
            status.expires_in_ms = None;
            *state = PairingState::Terminal(status);
        }
    }

    pub(super) fn begin_commit(&self, pairing_id: &str) -> bool {
        let mut state = self.lock();
        let PairingState::Active(active) = &mut *state else {
            return false;
        };
        if active.status.pairing_id.as_deref() != Some(pairing_id) {
            return false;
        }
        active.committing = true;
        active.expires_at = None;
        true
    }

    pub(super) fn confirm(&self, accept: bool) -> Result<PairingStatus, NodeError> {
        let mut state = self.lock();
        expire_due(&mut state, self.now());
        if let PairingState::Terminal(status) = &*state {
            if status.phase == PairingPhase::TimedOut {
                return Ok(status.clone());
            }
        }
        let PairingState::Active(active) = &*state else {
            return Err(NodeError::NoPairing);
        };
        if active.status.phase != PairingPhase::AwaitingConfirmation {
            return Err(NodeError::NoPairing);
        }
        active
            .control
            .send(if accept {
                LocalDecision::Accept
            } else {
                LocalDecision::Reject
            })
            .map_err(|_| NodeError::Session)?;
        Ok(snapshot(
            active.status.clone(),
            active.expires_at,
            self.now(),
        ))
    }

    pub(super) fn cancel(&self) -> PairingStatus {
        let mut state = self.lock();
        let status = match &*state {
            PairingState::Idle => return PairingStatus::idle(),
            PairingState::Invited(invite) => {
                let _ = invite.control.send(LocalDecision::Cancel);
                invite.status.clone()
            }
            PairingState::Active(active) => {
                if active.committing {
                    return active.status.clone();
                }
                let _ = active.control.send(LocalDecision::Cancel);
                active.status.clone()
            }
            PairingState::Terminal(status) => return status.clone(),
        };
        let status = PairingStatus {
            phase: PairingPhase::Cancelled,
            sas: None,
            expires_in_ms: None,
            ..status
        };
        *state = PairingState::Terminal(status.clone());
        status
    }

    pub(super) fn progress(&self) -> PairingStatus {
        let mut state = self.lock();
        let now = self.now();
        expire_due(&mut state, now);
        match &*state {
            PairingState::Idle => PairingStatus::idle(),
            PairingState::Invited(invite) => {
                snapshot(invite.status.clone(), Some(invite.expires_at), now)
            }
            PairingState::Active(active) => snapshot(active.status.clone(), active.expires_at, now),
            PairingState::Terminal(status) => status.clone(),
        }
    }

    pub(super) fn is_active(&self) -> bool {
        matches!(
            *self.lock(),
            PairingState::Invited(_) | PairingState::Active(_)
        )
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, PairingState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn terminal_from(
    state: &PairingState,
    phase: PairingPhase,
    error: Option<NodeError>,
) -> PairingStatus {
    let mut status = match state {
        PairingState::Invited(invite) => invite.status.clone(),
        PairingState::Active(active) => active.status.clone(),
        PairingState::Terminal(status) => status.clone(),
        PairingState::Idle => PairingStatus::idle(),
    };
    status.phase = phase;
    status.error = error;
    status.sas = None;
    status.expires_in_ms = None;
    status
}

fn expire_due(state: &mut PairingState, now: Instant) {
    let due = match state {
        PairingState::Invited(invite) => (now >= invite.expires_at).then_some(&invite.control),
        PairingState::Active(active) => active
            .expires_at
            .filter(|deadline| now >= *deadline)
            .map(|_| &active.control),
        PairingState::Idle | PairingState::Terminal(_) => None,
    };
    let Some(control) = due else {
        return;
    };
    let _ = control.send(LocalDecision::Expire);
    *state = PairingState::Terminal(terminal_from(state, PairingPhase::TimedOut, None));
}

fn snapshot(mut status: PairingStatus, expires_at: Option<Instant>, now: Instant) -> PairingStatus {
    status.expires_in_ms = expires_at.map(|deadline| remaining_ms(deadline, now));
    status
}

fn remaining_ms(deadline: Instant, now: Instant) -> u64 {
    let remaining = deadline.saturating_duration_since(now);
    u64::try_from(remaining.as_millis())
        .unwrap_or(u64::MAX)
        .saturating_add(u64::from(
            !remaining.is_zero() && remaining.subsec_nanos() % 1_000_000 != 0,
        ))
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::discovery::Discovery;
    use crate::node::{listen, Node};
    use crate::peers::PeerStore;
    use crate::sync::testutil::TestSource;
    use tokio::net::TcpListener;

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
            |_: &str, _: &crate::SyncOutcome| {},
            receiver,
        ));
        (addr, shutdown)
    }

    async fn wait_for(node: &Node, phase: PairingPhase) -> PairingStatus {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                let status = node.pair_progress();
                if status.phase == phase {
                    return status;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("pairing did not reach {phase:?}"))
    }

    fn controlled_manager() -> (PairingManager, Arc<Mutex<Instant>>) {
        let now = Arc::new(Mutex::new(Instant::now()));
        let clock = Arc::clone(&now);
        let manager = PairingManager::with_clock(move || {
            *clock
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        });
        (manager, now)
    }

    fn advance(clock: &Mutex<Instant>, duration: Duration) {
        let mut now = clock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *now += duration;
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn both_confirmations_are_required_before_either_peer_is_persisted() {
        let dir = tempfile::tempdir().unwrap();
        let responder = node(&dir, "responder");
        let initiator = node(&dir, "initiator");
        let responder_source = Arc::new(TestSource::new("responder-id", Vec::new()));
        let initiator_source = Arc::new(TestSource::new("initiator-id", Vec::new()));
        let (addr, shutdown) =
            start_listener(Arc::clone(&responder), Arc::clone(&responder_source)).await;

        let invite = responder.pair_create_invite().unwrap();
        assert!(!format!("{invite:?}").contains(&invite.code));
        let joined = initiator
            .pair_join(&invite.code, &addr.to_string(), initiator_source.as_ref())
            .await
            .unwrap();
        assert_eq!(joined.phase, PairingPhase::AwaitingConfirmation);
        let inbound = wait_for(&responder, PairingPhase::AwaitingConfirmation).await;
        assert_eq!(joined.sas, inbound.sas);
        assert!(joined
            .sas
            .as_deref()
            .is_some_and(|sas| sas.len() == 6 && sas.bytes().all(|byte| byte.is_ascii_digit())));
        assert!(responder.peers().list().is_empty());
        assert!(initiator.peers().list().is_empty());

        responder.pair_confirm(true).unwrap();
        tokio::task::yield_now().await;
        assert!(responder.peers().list().is_empty());
        assert!(initiator.peers().list().is_empty());

        initiator.pair_confirm(true).unwrap();
        let responder_done = wait_for(&responder, PairingPhase::Confirmed).await;
        let initiator_done = wait_for(&initiator, PairingPhase::Confirmed).await;
        assert!(responder_done.peer.is_some());
        assert!(initiator_done.peer.is_some());
        assert_eq!(responder.peers().list().len(), 1);
        assert_eq!(initiator.peers().list().len(), 1);
        let _ = shutdown.send(true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn rejection_and_cancel_leave_no_peer_and_allow_a_fresh_invite() {
        let dir = tempfile::tempdir().unwrap();
        let responder = node(&dir, "responder");
        let initiator = node(&dir, "initiator");
        let responder_source = Arc::new(TestSource::new("responder-id", Vec::new()));
        let initiator_source = Arc::new(TestSource::new("initiator-id", Vec::new()));
        let (addr, shutdown) =
            start_listener(Arc::clone(&responder), Arc::clone(&responder_source)).await;

        let invite = responder.pair_create_invite().unwrap();
        initiator
            .pair_join(&invite.code, &addr.to_string(), initiator_source.as_ref())
            .await
            .unwrap();
        wait_for(&responder, PairingPhase::AwaitingConfirmation).await;
        responder.pair_confirm(false).unwrap();
        wait_for(&responder, PairingPhase::Rejected).await;
        wait_for(&initiator, PairingPhase::Rejected).await;
        assert!(responder.peers().list().is_empty());
        assert!(initiator.peers().list().is_empty());

        let fresh = responder.pair_create_invite().unwrap();
        initiator
            .pair_join(&fresh.code, &addr.to_string(), initiator_source.as_ref())
            .await
            .unwrap();
        wait_for(&responder, PairingPhase::AwaitingConfirmation).await;
        initiator.pair_confirm(true).unwrap();
        tokio::task::yield_now().await;
        assert_eq!(initiator.pair_cancel().phase, PairingPhase::Cancelled);
        wait_for(&responder, PairingPhase::Rejected).await;
        assert!(responder.peers().list().is_empty());
        assert!(initiator.peers().list().is_empty());

        let final_invite = responder.pair_create_invite().unwrap();
        assert_ne!(final_invite.pairing_id, invite.pairing_id);
        assert_eq!(responder.pair_cancel().phase, PairingPhase::Cancelled);
        assert_eq!(responder.pair_cancel().phase, PairingPhase::Cancelled);
        assert!(responder.peers().list().is_empty());
        let _ = shutdown.send(true);
    }

    #[test]
    fn an_expired_invite_is_terminal_and_releases_the_ceremony_slot() {
        let (manager, clock) = controlled_manager();
        let first = manager.create_invite(None).unwrap();
        assert_eq!(manager.progress().expires_in_ms, Some(120_000));
        advance(&clock, PAIRING_INVITE_TTL - Duration::from_millis(1));
        assert_eq!(manager.progress().expires_in_ms, Some(1));
        advance(&clock, Duration::from_millis(1));

        assert_eq!(manager.progress().phase, PairingPhase::TimedOut);
        assert_eq!(manager.progress().expires_in_ms, None);
        assert!(manager.candidate().is_none());
        assert_ne!(
            manager.create_invite(None).unwrap().pairing_id,
            first.pairing_id
        );
    }

    #[test]
    fn confirmation_deadline_owns_timeout_and_cannot_be_relabelled_cancelled() {
        let (manager, clock) = controlled_manager();
        let pairing_id = "pairing-id";
        let decision = manager.begin_join(pairing_id.into()).unwrap();
        manager.awaiting(
            pairing_id,
            "123456".into(),
            PairingPeer {
                pairing_id: pairing_id.into(),
                device_id: "device-id".into(),
                name: "Phone".into(),
                addr: None,
            },
        );
        assert_eq!(manager.progress().expires_in_ms, Some(60_000));
        advance(&clock, PAIRING_CONFIRM_TIMEOUT);

        let expired = manager.confirm(true).expect("expiry is a terminal result");
        assert_eq!(expired.phase, PairingPhase::TimedOut);
        assert_eq!(expired.sas, None);
        assert_eq!(expired.expires_in_ms, None);
        assert!(*decision.borrow() == LocalDecision::Expire);
        assert_eq!(manager.cancel().phase, PairingPhase::TimedOut);
        manager.finish(pairing_id, PairingPhase::Cancelled, None);
        assert_eq!(manager.progress().phase, PairingPhase::TimedOut);
    }
}
