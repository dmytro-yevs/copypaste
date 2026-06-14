//! Discovery-initiated SAS pairing state machine (LAN/SAS Phase 2).
//!
//! Drives the human-confirmation step of the discovery pairing flow. Unlike the
//! QR path — where the high-entropy `PairingToken` carried in the QR is the
//! authenticator and PAKE alone proves both sides know it — the discovery path
//! has NO pre-shared secret. The bootstrap handshake therefore runs with an
//! EPHEMERAL random password the initiator transmits in-clear inside the
//! (unauthenticated) bootstrap TLS channel, and authentication is provided
//! ENTIRELY by the human Short Authentication String (SAS) comparison.
//!
//! The SAS is derived from the post-PAKE, post-channel-binding `bound_key`
//! (`copypaste_p2p::pake::derive_sas`). A man-in-the-middle that substitutes its
//! own password per leg yields a DIFFERENT `bound_key` per leg → a different SAS
//! per leg → the two humans see mismatched codes and abort. Both sides MUST
//! confirm (frame 10a ACCEPT/REJECT in `run_with_confirm`) before any key is
//! trusted or persisted.
//!
//! ## States
//! ```text
//! Idle ──pair_with_discovered / inbound─▶ Initiating
//!   Initiating ──SAS derived (frame 9)──▶ AwaitingSas { sas, role, expires_at }
//!     AwaitingSas ──pair_confirm_sas(true) + peer accept──▶ Confirmed
//!     AwaitingSas ──pair_confirm_sas(false)──▶ Rejected
//!     AwaitingSas ──pair_abort──▶ Aborted
//!     AwaitingSas ──SAS_CONFIRM_TIMEOUT elapsed──▶ TimedOut
//! ```
//! `Confirmed | Rejected | Aborted | TimedOut` are terminal; the handler resets
//! the machine to `Idle` once the caller has observed the terminal state.
//!
//! ## Single active pairing
//! Only ONE pairing may be in flight at a time (v0.6 simplicity, plan risk #3).
//! A concurrent `pair_with_discovered` while the machine is non-`Idle` is
//! rejected with a rate-limited error.

use std::time::Instant;

use tokio::sync::oneshot;

/// Peer identity / address info available when the SAS confirmation step begins.
///
/// On the initiator path this comes from the mDNS [`PeerInfo`] snapshot taken
/// before dialling (device name + resolved IP addresses + mDNS `device_id` which
/// is the canonical cert fingerprint).  On the responder path the inbound
/// connection arrives without prior mDNS resolution, so all fields are `None` /
/// empty — the UI renders whatever is populated and silently omits the rest.
///
/// `peer_model`, `peer_os`, `peer_app_version` are NOT available here: the
/// metadata extension exchange happens AFTER the SAS human-confirmation step
/// (frames 10/11) and is surfaced in the `pair_with_discovered` / final response
/// instead.
///
/// [`PeerInfo`]: copypaste_p2p::discovery::PeerInfo
#[derive(Debug, Clone, Default)]
pub struct PeerSnapshot {
    /// Human-readable device name advertised over mDNS (initiator path only).
    pub device_name: Option<String>,
    /// Resolved IP addresses as display strings, IPv4-first (initiator path only).
    pub ip_addrs: Vec<String>,
    /// Cert fingerprint (the mDNS `device_id` = hex SHA-256 of the peer cert).
    /// Available on the initiator path because we know the mDNS `device_id`
    /// before dialling.  `None` on the responder path (inbound connection —
    /// the TLS fingerprint is available post-handshake but not yet surfaced here;
    /// follow-up: thread `tls_peer_fp` into the responder's confirm callback).
    pub fingerprint: Option<String>,
}

/// How long the daemon waits for the local user to confirm or reject the SAS
/// before auto-aborting the in-flight pairing.
///
/// Distinct from `copypaste_p2p::bootstrap::PAKE_EXCHANGE_TIMEOUT` (which bounds
/// the machine-to-machine
/// 9-frame exchange): a human reading and comparing a 6-digit code may take
/// noticeably longer than a stalled-peer network timeout, so the confirmation
/// window is generous. After this elapses with no decision the pairing is
/// aborted (keys drop/zeroize, nothing persisted).
pub const SAS_CONFIRM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Which side of the discovery handshake this daemon is playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingRole {
    /// This daemon dialed the peer (`pair_with_discovered`).
    Initiator,
    /// This daemon accepted an inbound discovery-pair connection (standing
    /// responder).
    Responder,
}

impl PairingRole {
    /// Lowercase wire string surfaced in the `pair_get_sas` IPC response.
    pub fn as_str(self) -> &'static str {
        match self {
            PairingRole::Initiator => "initiator",
            PairingRole::Responder => "responder",
        }
    }
}

/// The discovery-pairing state machine.
///
/// Non-terminal: `Idle`, `Initiating`, `AwaitingSas`.
/// Terminal: `Confirmed`, `Rejected`, `Aborted`, `TimedOut`.
#[derive(Debug, Clone)]
pub enum PairingState {
    /// No pairing in flight — the only state in which a new pairing may start.
    Idle,
    /// A pairing has started; the bootstrap handshake is running but the SAS is
    /// not yet known (pre-frame-9).
    Initiating {
        /// Role this daemon is playing.
        role: PairingRole,
        /// Peer metadata known before dialling (initiator: from mDNS snapshot;
        /// responder: empty — the inbound connection carries no prior context).
        peer: PeerSnapshot,
    },
    /// The handshake reached frame 9, the SAS is derived, and the daemon is
    /// waiting for the local user's accept/reject decision.
    AwaitingSas {
        /// The 6-digit decimal SAS to display to the user.
        sas: String,
        /// Role this daemon is playing.
        role: PairingRole,
        /// Wall-clock deadline after which the pairing auto-aborts.
        expires_at: Instant,
        /// Peer identity / address info available at SAS time (initiator: from
        /// mDNS; responder: empty).  Surfaced in `pair_get_sas` so the UI can
        /// show the peer's name, IPs, and fingerprint alongside the SAS code.
        peer: PeerSnapshot,
    },
    /// Both sides accepted the SAS — keys are trusted and have been persisted.
    Confirmed,
    /// The local user (or the peer) rejected the SAS — keys dropped, nothing
    /// persisted.
    Rejected,
    /// The pairing was explicitly aborted (`pair_abort`) — keys dropped.
    Aborted,
    /// The confirmation window elapsed with no decision — keys dropped.
    TimedOut,
}

impl PairingState {
    /// `true` when a new pairing may be started (the machine is `Idle`).
    pub fn is_idle(&self) -> bool {
        matches!(self, PairingState::Idle)
    }

    /// `true` when the machine is mid-pairing (not `Idle`, not terminal).
    pub fn is_active(&self) -> bool {
        matches!(
            self,
            PairingState::Initiating { .. } | PairingState::AwaitingSas { .. }
        )
    }

    /// `true` for the four terminal outcomes.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            PairingState::Confirmed
                | PairingState::Rejected
                | PairingState::Aborted
                | PairingState::TimedOut
        )
    }

    /// Lowercase wire string surfaced in the `pair_get_sas` IPC response so the
    /// UI can branch on a stable, serialisable token.
    pub fn as_str(&self) -> &'static str {
        match self {
            PairingState::Idle => "idle",
            PairingState::Initiating { .. } => "initiating",
            PairingState::AwaitingSas { .. } => "awaiting_sas",
            PairingState::Confirmed => "confirmed",
            PairingState::Rejected => "rejected",
            PairingState::Aborted => "aborted",
            PairingState::TimedOut => "timed_out",
        }
    }

    /// The SAS string when in [`PairingState::AwaitingSas`], else `None`.
    pub fn sas(&self) -> Option<&str> {
        match self {
            PairingState::AwaitingSas { sas, .. } => Some(sas.as_str()),
            _ => None,
        }
    }

    /// The role when in an active state, else `None`.
    pub fn role(&self) -> Option<PairingRole> {
        match self {
            PairingState::Initiating { role, .. } | PairingState::AwaitingSas { role, .. } => {
                Some(*role)
            }
            _ => None,
        }
    }

    /// Peer metadata snapshot when in an active (non-idle, non-terminal) state.
    ///
    /// Returns `None` for `Idle` and all terminal states.  On the initiator
    /// path the snapshot carries the mDNS device name, IP addresses, and
    /// fingerprint.  On the responder path all fields are `None`/empty (the
    /// inbound connection arrives without prior mDNS resolution).
    pub fn peer_snapshot(&self) -> Option<&PeerSnapshot> {
        match self {
            PairingState::Initiating { peer, .. } | PairingState::AwaitingSas { peer, .. } => {
                Some(peer)
            }
            _ => None,
        }
    }
}

/// Coordinator owning the live [`PairingState`] plus the channel used to deliver
/// the user's accept/reject decision into the in-flight handshake task.
///
/// `state` is the observable machine (`pair_get_sas` reads it). `pending_sas`
/// holds the SAS currently awaiting confirmation together with the
/// [`oneshot::Sender<bool>`] that the handshake's `confirm` callback is awaiting
/// — `pair_confirm_sas` fires it. A separate `abort` sender lets `pair_abort`
/// cancel the handshake task.
///
/// Both inner fields are guarded by plain `std::sync::Mutex` because every
/// critical section is a trivial take/replace with no `.await`.
#[derive(Default)]
pub struct PairingCoordinator {
    state: std::sync::Mutex<StateSlot>,
    pending: std::sync::Mutex<Option<Pending>>,
}

/// `std::sync::Mutex` cannot hold a non-`Default` enum behind `#[derive(Default)]`
/// directly, so wrap it.
struct StateSlot(PairingState);

impl Default for StateSlot {
    fn default() -> Self {
        StateSlot(PairingState::Idle)
    }
}

/// In-flight confirmation channel + abort handle for the single active pairing.
struct Pending {
    /// Fired by `pair_confirm_sas` with the user's accept(`true`)/reject(`false`)
    /// decision; awaited by the handshake's `confirm` callback.
    confirm_tx: oneshot::Sender<bool>,
}

impl PairingCoordinator {
    /// Construct an idle coordinator.
    pub fn new() -> Self {
        Self::default()
    }

    /// Snapshot the current state.
    pub fn snapshot(&self) -> PairingState {
        self.lock_state().0.clone()
    }

    /// Attempt to claim the machine for a new pairing as `role`.
    ///
    /// `peer` carries whatever peer identity information is known before dialling
    /// (initiator: mDNS device name / IPs / fingerprint; responder: default empty).
    ///
    /// Returns `true` and transitions to `Initiating` when no pairing is
    /// genuinely in progress. Specifically:
    /// - From `Idle`: claims immediately.
    /// - From a stale **terminal** state (`Confirmed`/`Rejected`/`Aborted`/
    ///   `TimedOut`): auto-resets (clears any stale pending channel) and claims,
    ///   so the handler never needs to call `reset()` explicitly before a retry.
    /// - From an **active** state (`Initiating`/`AwaitingSas`): returns `false`
    ///   so the caller can reject the concurrent request with a rate-limited error.
    pub fn try_begin(&self, role: PairingRole, peer: PeerSnapshot) -> bool {
        let mut slot = self.lock_state();
        if slot.0.is_active() {
            return false; // genuine in-progress pairing — refuse
        }
        // Idle OR stale terminal: clear any stale pending channel and claim.
        let _ = self.lock_pending().take();
        slot.0 = PairingState::Initiating { role, peer };
        true
    }

    /// Transition `Initiating → AwaitingSas`, storing the derived SAS and the
    /// confirmation channel. Called from the handshake's `confirm` callback.
    ///
    /// `peer` is the same snapshot passed to [`try_begin`]; it is forwarded into
    /// `AwaitingSas` so `pair_get_sas` can surface it while the user is reading
    /// the code.
    ///
    /// Returns the receiver the callback awaits for the user's decision.
    pub fn enter_awaiting_sas(
        &self,
        sas: String,
        role: PairingRole,
        peer: PeerSnapshot,
    ) -> oneshot::Receiver<bool> {
        let (confirm_tx, confirm_rx) = oneshot::channel();
        {
            let mut slot = self.lock_state();
            slot.0 = PairingState::AwaitingSas {
                sas,
                role,
                expires_at: Instant::now() + SAS_CONFIRM_TIMEOUT,
                peer,
            };
        }
        *self.lock_pending() = Some(Pending { confirm_tx });
        confirm_rx
    }

    /// Deliver the user's accept/reject decision into the waiting handshake.
    ///
    /// Returns `true` when a decision was delivered (a pairing was awaiting),
    /// `false` when no pairing is awaiting confirmation. Does NOT itself move to
    /// a terminal state — the handshake task records the outcome via
    /// [`finish`](Self::finish) once both sides have exchanged frame 10a.
    pub fn deliver_decision(&self, accept: bool) -> bool {
        let pending = self.lock_pending().take();
        match pending {
            Some(p) => {
                // Receiver dropped (handshake task already gone) is benign.
                let _ = p.confirm_tx.send(accept);
                true
            }
            None => false,
        }
    }

    /// Abort an in-flight pairing: drop the confirmation channel (which makes the
    /// handshake `confirm` await resolve to a rejection / error) and move to
    /// `Aborted`. No-op when already terminal/idle.
    pub fn abort(&self) {
        // Dropping the sender resolves the handshake's await with `Err(Recv)`,
        // which the callback maps to a rejection so keys drop/zeroize.
        let _ = self.lock_pending().take();
        let mut slot = self.lock_state();
        if slot.0.is_active() {
            slot.0 = PairingState::Aborted;
        }
    }

    /// Record the terminal outcome reported by the handshake task and clear any
    /// pending confirmation channel.
    pub fn finish(&self, outcome: PairingState) {
        debug_assert!(outcome.is_terminal());
        let _ = self.lock_pending().take();
        self.lock_state().0 = outcome;
    }

    /// Reset to `Idle` (called once the caller has observed a terminal state, so
    /// a fresh pairing may begin).
    pub fn reset(&self) {
        let _ = self.lock_pending().take();
        self.lock_state().0 = PairingState::Idle;
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, StateSlot> {
        // Poisoned mutex (a panic mid-mutation) is recovered: the slot holds
        // only a small enum and an optional channel — reusing it after a panic
        // is safe and keeps pairing functional.
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    fn lock_pending(&self) -> std::sync::MutexGuard<'_, Option<Pending>> {
        self.pending.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_coordinator_is_idle() {
        let c = PairingCoordinator::new();
        assert!(c.snapshot().is_idle());
        assert_eq!(c.snapshot().as_str(), "idle");
    }

    #[test]
    fn begin_transitions_idle_to_initiating() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator, PeerSnapshot::default()));
        let s = c.snapshot();
        assert_eq!(s.as_str(), "initiating");
        assert_eq!(s.role(), Some(PairingRole::Initiator));
        assert!(s.is_active());
    }

    #[test]
    fn concurrent_begin_is_rejected() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator, PeerSnapshot::default()));
        // A second begin while non-idle must be refused (single active pairing).
        assert!(!c.try_begin(PairingRole::Responder, PeerSnapshot::default()));
        // State unchanged.
        assert_eq!(c.snapshot().role(), Some(PairingRole::Initiator));
    }

    #[test]
    fn enter_awaiting_sas_exposes_sas_and_role() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Responder, PeerSnapshot::default()));
        let _rx = c.enter_awaiting_sas(
            "123456".to_string(),
            PairingRole::Responder,
            PeerSnapshot::default(),
        );
        let s = c.snapshot();
        assert_eq!(s.as_str(), "awaiting_sas");
        assert_eq!(s.sas(), Some("123456"));
        assert_eq!(s.role(), Some(PairingRole::Responder));
    }

    #[tokio::test]
    async fn deliver_decision_fires_oneshot() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator, PeerSnapshot::default()));
        let rx = c.enter_awaiting_sas(
            "000000".to_string(),
            PairingRole::Initiator,
            PeerSnapshot::default(),
        );
        assert!(c.deliver_decision(true));
        assert!(rx.await.unwrap());
    }

    #[tokio::test]
    async fn reject_delivers_false_and_aborts_drop_keys() {
        // A reject must propagate `false` to the handshake so it sends REJECT in
        // frame 10a and drops/zeroizes the session key (no persist, no rotate).
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator, PeerSnapshot::default()));
        let rx = c.enter_awaiting_sas(
            "424242".to_string(),
            PairingRole::Initiator,
            PeerSnapshot::default(),
        );
        assert!(c.deliver_decision(false));
        assert!(!rx.await.unwrap());
        // The handshake task would then call finish(Rejected).
        c.finish(PairingState::Rejected);
        assert_eq!(c.snapshot().as_str(), "rejected");
        assert!(c.snapshot().is_terminal());
    }

    #[tokio::test]
    async fn abort_drops_confirm_channel_so_handshake_sees_rejection() {
        // pair_abort must cancel the in-flight handshake: dropping the sender
        // resolves the await with an Err, which the callback treats as reject.
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Responder, PeerSnapshot::default()));
        let rx = c.enter_awaiting_sas(
            "999999".to_string(),
            PairingRole::Responder,
            PeerSnapshot::default(),
        );
        c.abort();
        assert!(rx.await.is_err(), "dropping the sender must error the recv");
        assert_eq!(c.snapshot().as_str(), "aborted");
    }

    #[test]
    fn deliver_decision_without_pending_is_false() {
        let c = PairingCoordinator::new();
        assert!(!c.deliver_decision(true));
    }

    #[test]
    fn reset_returns_to_idle_for_next_pairing() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator, PeerSnapshot::default()));
        c.finish(PairingState::Confirmed);
        assert_eq!(c.snapshot().as_str(), "confirmed");
        c.reset();
        assert!(c.snapshot().is_idle());
        // A fresh pairing may begin after reset.
        assert!(c.try_begin(PairingRole::Responder, PeerSnapshot::default()));
    }

    /// After finish(terminal), try_begin must succeed WITHOUT requiring reset().
    /// This is the LAN retry regression: stale terminal state must not block new pairings.
    #[test]
    fn try_begin_succeeds_after_terminal_without_reset() {
        let c = PairingCoordinator::new();

        // Confirmed terminal → next try_begin must succeed (auto-reset).
        assert!(c.try_begin(PairingRole::Initiator, PeerSnapshot::default()));
        c.finish(PairingState::Confirmed);
        assert_eq!(c.snapshot().as_str(), "confirmed");
        assert!(
            c.try_begin(PairingRole::Responder, PeerSnapshot::default()),
            "try_begin must succeed after Confirmed without explicit reset"
        );
        assert_eq!(c.snapshot().as_str(), "initiating");

        // Rejected terminal → next try_begin must also succeed.
        c.finish(PairingState::Rejected);
        assert!(
            c.try_begin(PairingRole::Initiator, PeerSnapshot::default()),
            "try_begin must succeed after Rejected without explicit reset"
        );

        // TimedOut terminal → same.
        c.finish(PairingState::TimedOut);
        assert!(
            c.try_begin(PairingRole::Initiator, PeerSnapshot::default()),
            "try_begin must succeed after TimedOut without explicit reset"
        );
    }

    /// After abort(), try_begin must succeed (abort() leaves state as Aborted, a terminal).
    #[test]
    fn try_begin_succeeds_after_abort() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator, PeerSnapshot::default()));
        let _rx = c.enter_awaiting_sas(
            "123456".to_string(),
            PairingRole::Initiator,
            PeerSnapshot::default(),
        );
        c.abort();
        assert_eq!(c.snapshot().as_str(), "aborted");
        assert!(
            c.try_begin(PairingRole::Responder, PeerSnapshot::default()),
            "try_begin must succeed after abort() without explicit reset"
        );
        assert_eq!(c.snapshot().as_str(), "initiating");
    }

    /// While genuinely active (Initiating or AwaitingSas), try_begin must still return false.
    #[test]
    fn try_begin_refused_while_active() {
        let c = PairingCoordinator::new();

        // Refused while Initiating.
        assert!(c.try_begin(PairingRole::Initiator, PeerSnapshot::default()));
        assert_eq!(c.snapshot().as_str(), "initiating");
        assert!(
            !c.try_begin(PairingRole::Responder, PeerSnapshot::default()),
            "try_begin must be refused while Initiating"
        );
        assert_eq!(
            c.snapshot().role(),
            Some(PairingRole::Initiator),
            "state must be unchanged"
        );

        // Refused while AwaitingSas.
        let _rx = c.enter_awaiting_sas(
            "999999".to_string(),
            PairingRole::Initiator,
            PeerSnapshot::default(),
        );
        assert_eq!(c.snapshot().as_str(), "awaiting_sas");
        assert!(
            !c.try_begin(PairingRole::Responder, PeerSnapshot::default()),
            "try_begin must be refused while AwaitingSas"
        );
        assert_eq!(
            c.snapshot().as_str(),
            "awaiting_sas",
            "state must be unchanged"
        );
    }

    // ── peer metadata surfacing ────────────────────────────────────────────────

    /// `AwaitingSas` must carry peer metadata (device name, addresses,
    /// fingerprint) so `pair_get_sas` can surface them in the IPC response for
    /// the macOS UI to display before the user confirms the SAS.
    #[test]
    fn awaiting_sas_carries_peer_metadata() {
        let c = PairingCoordinator::new();
        let meta = PeerSnapshot {
            device_name: Some("Alice's MacBook".to_string()),
            ip_addrs: vec!["192.168.1.42".to_string()],
            fingerprint: Some("ab:cd:ef".to_string()),
        };
        assert!(c.try_begin(PairingRole::Initiator, meta.clone()));
        let _rx = c.enter_awaiting_sas("123456".to_string(), PairingRole::Initiator, meta);
        let s = c.snapshot();
        assert_eq!(s.as_str(), "awaiting_sas");
        assert_eq!(s.sas(), Some("123456"));
        let snap = s
            .peer_snapshot()
            .expect("peer_snapshot must be Some in AwaitingSas");
        assert_eq!(snap.device_name.as_deref(), Some("Alice's MacBook"));
        assert_eq!(snap.ip_addrs, vec!["192.168.1.42".to_string()]);
        assert_eq!(snap.fingerprint.as_deref(), Some("ab:cd:ef"));
    }

    /// `pair_get_sas` metadata must be available even when no info is known
    /// (responder path — mDNS info unavailable). All fields default to None/empty.
    #[test]
    fn awaiting_sas_empty_peer_metadata_is_valid() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Responder, PeerSnapshot::default()));
        let meta = PeerSnapshot::default();
        let _rx = c.enter_awaiting_sas("654321".to_string(), PairingRole::Responder, meta);
        let s = c.snapshot();
        let snap = s
            .peer_snapshot()
            .expect("peer_snapshot must be Some in AwaitingSas");
        assert!(snap.device_name.is_none());
        assert!(snap.ip_addrs.is_empty());
        assert!(snap.fingerprint.is_none());
    }
}
