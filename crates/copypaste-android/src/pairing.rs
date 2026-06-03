//! Android discovery + SAS pairing (LAN/SAS Phase 4 — Android parity).
//!
//! This is the Android analog of the macOS daemon's discovery-pairing path
//! (`copypaste-daemon/src/pairing_sm.rs` + the `start_p2p` standing responder).
//! Unlike the QR path — where the high-entropy `PairingToken` carried in the QR
//! is the authenticator and PAKE alone proves both sides know it — the discovery
//! path has NO pre-shared secret. The bootstrap handshake runs with an EPHEMERAL
//! random password the initiator transmits in-clear inside the (unauthenticated)
//! bootstrap TLS channel, and authentication is provided ENTIRELY by the human
//! Short Authentication String (SAS) comparison.
//!
//! The SAS is derived from the post-PAKE, post-channel-binding `bound_key`
//! (`copypaste_p2p::pake::derive_sas`). A man-in-the-middle that substitutes its
//! own password per leg yields a DIFFERENT `bound_key` per leg → a different SAS
//! per leg → the two humans see mismatched codes and abort. Both sides MUST
//! confirm (frame 10a ACCEPT/REJECT in `run_with_confirm` /
//! `run_initiator_with_confirm`) before any key is trusted or persisted.
//!
//! # FFI shape — POLLED state machine (NOT a callback interface)
//!
//! UniFFI cannot pass an async Rust callback across the boundary, so the
//! handshake's `confirm` closure is wired to a [`PairingCoordinator`] instead:
//! the closure transitions the coordinator into `AwaitingSas` and parks on a
//! `tokio::sync::oneshot`. Kotlin POLLS [`pair_get_sas`](crate::pair_get_sas)
//! for the SAS, shows it, and calls
//! [`pair_confirm_sas`](crate::pair_confirm_sas) to fire the oneshot. There is a
//! single process-global [`AndroidPairing`] (the coordinator + the standing
//! responder + the in-flight initiator task) — exactly ONE pairing may be in
//! flight at a time (v0.6 simplicity).
//!
//! The standing responder (bound on `bport` when [`start`](start) is called)
//! makes the Android device pairable FROM macOS: it accepts an inbound bootstrap
//! connection, runs `run_with_confirm` wired to the SAME coordinator with the
//! `Responder` role, and routes the SAS through the same poll/confirm flow.
//!
//! # Security (load-bearing — mirrors macOS)
//!
//! * SAS derives from the post-channel-binding `bound_key`; both sides exchange
//!   frame 10a ACCEPT before the key is trusted.
//! * Reject / abort / timeout drops the confirmation channel → the handshake's
//!   `confirm` await resolves to a rejection → keys drop/zeroize, NOTHING is
//!   persisted (the coordinator never reaches `Confirmed`).
//! * Purely additive: the QR transcript (`run` / `run_initiator`) and
//!   fingerprint pinning are untouched.
//! * `session_key` crosses the FFI per the documented contract
//!   ([`PairStatus`]); Kotlin zeroes it after KEK-wrapping.
//! * Key / SAS bytes are NEVER logged.

use std::sync::{Arc, Mutex, OnceLock};

use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use copypaste_p2p::discovery::{DiscoveryService, PeerInfo};

use crate::{CopypasteError, SyncProvisioning};

/// Fixed, well-known PAKE password for the LAN/SAS *discovery* pairing path.
///
/// The discovery path has NO pre-shared secret, so PAKE alone cannot
/// authenticate — the human SAS comparison does. opaque-ke is an ASYMMETRIC
/// PAKE: the initiator's `ClientLogin` only succeeds against a `PasswordFile`
/// registered for the IDENTICAL password (a mismatch fails at frame 7, before
/// any SAS is derived). Both ends therefore agree on this constant up front and
/// rely ENTIRELY on the post-channel-binding SAS compare for authentication
/// (Bluetooth numeric-comparison / Magic-Wormhole-verifier pattern): a MitM
/// substituting its own per-leg session yields a different `bound_key` → a
/// different SAS → the two humans see a mismatch and abort.
///
/// This is NON-SECRET by design — publishing it changes nothing, because the
/// SAS, not the password, gates trust and persistence.
///
/// NOTE (interop caveat): this is the value BOTH the Android initiator and the
/// Android standing responder use, so Android↔Android discovery pairing
/// converges. macOS↔Android discovery pairing additionally requires the macOS
/// daemon's discovery path to agree on this same constant; reconciling that is a
/// desktop-side (`copypaste-daemon`) concern outside this crate's scope.
pub const DISCOVERY_PAIRING_PASSWORD: &str = "copypaste/p2p/lan-sas-discovery/v1";

/// How long the standing responder / a polling Kotlin client waits for the local
/// user to confirm or reject the SAS before auto-aborting the in-flight pairing.
///
/// Mirrors the macOS daemon's `pairing_sm::SAS_CONFIRM_TIMEOUT`. Distinct from
/// `copypaste_p2p::bootstrap::PAKE_EXCHANGE_TIMEOUT` (which bounds the
/// machine-to-machine 9-frame exchange): a human reading and comparing a 6-digit
/// code may take noticeably longer than a stalled-peer network timeout, so the
/// confirmation window is generous. After this elapses with no decision the
/// pairing is aborted (keys drop/zeroize, nothing persisted).
pub const SAS_CONFIRM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Which side of the discovery handshake this device is playing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingRole {
    /// This device dialed the peer (`pair_with_discovered`).
    Initiator,
    /// This device accepted an inbound discovery-pair connection (standing
    /// responder — makes Android pairable FROM macOS).
    Responder,
}

impl PairingRole {
    /// Lowercase wire string surfaced in the `pair_get_sas` FFI response.
    pub fn as_str(self) -> &'static str {
        match self {
            PairingRole::Initiator => "initiator",
            PairingRole::Responder => "responder",
        }
    }
}

/// The FFI-safe outputs of a CONFIRMED bootstrap pairing, captured into the
/// terminal `Confirmed` state so Kotlin can persist them after polling.
///
/// # SECURITY NOTE — `session_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after deriving the content sync key from it (KEK-wrap into the
/// AndroidKeystore); never log it.
#[derive(Debug, Clone)]
pub struct ConfirmedPairing {
    /// The peer's pinned cert fingerprint (hex SHA-256 of its cert DER).
    pub peer_fingerprint: String,
    /// The peer's P2P sync-listener address (`host:port`), sent in-band.
    pub peer_sync_addr: String,
    /// The 32-byte PAKE+channel-bound session key (identical on both sides).
    pub session_key: Vec<u8>,
    /// Sync-account provisioning the PEER advertised over the authenticated
    /// tunnel. `None` when the peer advertised nothing or is a legacy build.
    pub peer_provisioning: Option<SyncProvisioning>,
}

/// The discovery-pairing state machine. Ported from the macOS daemon's
/// `pairing_sm::PairingState`, with the terminal `Confirmed` extended to carry
/// the FFI-safe outputs ([`ConfirmedPairing`]) so Kotlin persists them.
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
        /// Role this device is playing.
        role: PairingRole,
    },
    /// The handshake reached frame 9, the SAS is derived, and the daemon is
    /// waiting for the local user's accept/reject decision.
    AwaitingSas {
        /// The 6-digit decimal SAS to display to the user.
        sas: String,
        /// Role this device is playing.
        role: PairingRole,
    },
    /// Both sides accepted the SAS — the FFI-safe outputs are captured here for
    /// Kotlin to persist.
    Confirmed(ConfirmedPairing),
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
            PairingState::Confirmed(_)
                | PairingState::Rejected
                | PairingState::Aborted
                | PairingState::TimedOut
        )
    }

    /// Lowercase wire string surfaced in the `pair_get_sas` FFI response so
    /// Kotlin can branch on a stable, serialisable token.
    pub fn as_str(&self) -> &'static str {
        match self {
            PairingState::Idle => "idle",
            PairingState::Initiating { .. } => "initiating",
            PairingState::AwaitingSas { .. } => "awaiting_sas",
            PairingState::Confirmed(_) => "confirmed",
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
            PairingState::Initiating { role } | PairingState::AwaitingSas { role, .. } => {
                Some(*role)
            }
            _ => None,
        }
    }

    /// The captured outputs when in [`PairingState::Confirmed`], else `None`.
    pub fn confirmed(&self) -> Option<&ConfirmedPairing> {
        match self {
            PairingState::Confirmed(c) => Some(c),
            _ => None,
        }
    }
}

/// Coordinator owning the live [`PairingState`] plus the channel used to deliver
/// the user's accept/reject decision into the in-flight handshake task.
///
/// `state` is the observable machine ([`crate::pair_get_sas`] reads it).
/// `pending` holds the [`oneshot::Sender<bool>`] that the handshake's `confirm`
/// callback is awaiting — [`crate::pair_confirm_sas`] fires it. Both inner
/// fields are guarded by plain `std::sync::Mutex` because every critical section
/// is a trivial take/replace with no `.await` (a verbatim port of the daemon's
/// `PairingCoordinator`).
#[derive(Default)]
pub struct PairingCoordinator {
    state: Mutex<StateSlot>,
    pending: Mutex<Option<Pending>>,
}

/// `std::sync::Mutex` cannot hold a non-`Default` enum behind `#[derive(Default)]`
/// directly, so wrap it.
struct StateSlot(PairingState);

impl Default for StateSlot {
    fn default() -> Self {
        StateSlot(PairingState::Idle)
    }
}

/// In-flight confirmation channel for the single active pairing.
struct Pending {
    /// Fired by [`PairingCoordinator::deliver_decision`] with the user's
    /// accept(`true`)/reject(`false`) decision; awaited by the handshake's
    /// `confirm` callback.
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
    /// Returns `true` and transitions `Idle → Initiating` when no pairing is in
    /// flight; returns `false` (leaving state unchanged) when one already is, so
    /// the caller can reject the concurrent request.
    pub fn try_begin(&self, role: PairingRole) -> bool {
        let mut slot = self.lock_state();
        if !slot.0.is_idle() {
            return false;
        }
        slot.0 = PairingState::Initiating { role };
        true
    }

    /// Transition `Initiating → AwaitingSas`, storing the derived SAS and the
    /// confirmation channel. Called from the handshake's `confirm` callback.
    ///
    /// Returns the receiver the callback awaits for the user's decision.
    pub fn enter_awaiting_sas(&self, sas: String, role: PairingRole) -> oneshot::Receiver<bool> {
        let (confirm_tx, confirm_rx) = oneshot::channel();
        {
            let mut slot = self.lock_state();
            slot.0 = PairingState::AwaitingSas { sas, role };
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

/// Process-global discovery + pairing state for the Android FFI.
///
/// Holds the single [`PairingCoordinator`], the live [`DiscoveryService`] (mDNS
/// browse + advertise), the discovery browse [`JoinHandle`], the standing
/// responder task handle, and the in-flight initiator task handle. There is at
/// most ONE of each because exactly one pairing may be in flight at a time.
pub struct AndroidPairing {
    /// The single shared pairing coordinator.
    pub coordinator: Arc<PairingCoordinator>,
    /// Live discovery service (advertise + browse). `None` until [`start`].
    discovery: Mutex<Option<Arc<DiscoveryService>>>,
    /// Background browse task spawned by `DiscoveryService::start`. Aborted on
    /// [`stop`].
    discovery_task: Mutex<Option<JoinHandle<()>>>,
    /// The standing bootstrap responder task (re-binds `bport` and accepts one
    /// inbound discovery-pair connection per iteration). Aborted on [`stop`].
    responder_task: Mutex<Option<JoinHandle<()>>>,
    /// The in-flight initiator task spawned by `pair_with_discovered`. Aborted on
    /// `pair_abort` / `pair_reset` / a new pairing.
    initiator_task: Mutex<Option<JoinHandle<()>>>,
}

impl AndroidPairing {
    fn new() -> Self {
        Self {
            coordinator: Arc::new(PairingCoordinator::new()),
            discovery: Mutex::new(None),
            discovery_task: Mutex::new(None),
            responder_task: Mutex::new(None),
            initiator_task: Mutex::new(None),
        }
    }

    fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        m.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The live discovery service, if [`start`] has run.
    pub fn discovery(&self) -> Option<Arc<DiscoveryService>> {
        Self::lock(&self.discovery).clone()
    }

    /// Install the discovery service + its browse/responder tasks (replacing any
    /// previous ones, which are aborted first — idempotent restart-in-place).
    pub fn install(
        &self,
        discovery: Arc<DiscoveryService>,
        discovery_task: JoinHandle<()>,
        responder_task: JoinHandle<()>,
    ) {
        if let Some(old) = Self::lock(&self.discovery_task).replace(discovery_task) {
            old.abort();
        }
        if let Some(old) = Self::lock(&self.responder_task).replace(responder_task) {
            old.abort();
        }
        *Self::lock(&self.discovery) = Some(discovery);
    }

    /// `true` when discovery has been started and not stopped.
    pub fn is_running(&self) -> bool {
        Self::lock(&self.discovery).is_some()
    }

    /// Store the in-flight initiator task (aborting any prior one first).
    pub fn set_initiator_task(&self, task: JoinHandle<()>) {
        if let Some(old) = Self::lock(&self.initiator_task).replace(task) {
            old.abort();
        }
    }

    /// Abort the in-flight initiator task, if any.
    pub fn abort_initiator(&self) {
        if let Some(task) = Self::lock(&self.initiator_task).take() {
            task.abort();
        }
    }

    /// Tear down discovery: abort the browse + responder + initiator tasks and
    /// drop the discovery service (its `Drop` shuts the mDNS daemon / socket).
    pub fn stop(&self) {
        if let Some(task) = Self::lock(&self.discovery_task).take() {
            task.abort();
        }
        if let Some(task) = Self::lock(&self.responder_task).take() {
            task.abort();
        }
        self.abort_initiator();
        // Dropping the service aborts its internal browse task + mDNS daemon.
        *Self::lock(&self.discovery) = None;
        // Any in-flight confirmation is moot once discovery is torn down.
        self.coordinator.abort();
    }
}

/// The single process-global pairing/discovery instance.
static PAIRING: OnceLock<AndroidPairing> = OnceLock::new();

/// Access the process-global [`AndroidPairing`] (lazily initialised).
pub fn global() -> &'static AndroidPairing {
    PAIRING.get_or_init(AndroidPairing::new)
}

/// A peer discovered on the local network (FFI mirror of
/// [`copypaste_p2p::discovery::PeerInfo`] plus a `paired` flag the caller
/// computes by cross-referencing its persisted fingerprints).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// The peer's device id (hex cert fingerprint advertised as `did`).
    pub device_id: String,
    /// Human-readable device name.
    pub device_name: String,
    /// All resolved IP addresses for the peer (string form).
    pub ip_addrs: Vec<String>,
    /// TCP port of the peer's P2P sync listener.
    pub port: u16,
    /// TCP port of the peer's PAKE bootstrap listener (v2 peers only). `None`
    /// for v1 peers — the UI must disable "Pair" because the bootstrap handshake
    /// cannot be initiated without it.
    pub bport: Option<u16>,
    /// `true` when this peer's fingerprint is already in the caller's paired set.
    pub paired: bool,
}

impl DiscoveredPeer {
    /// Build from a [`PeerInfo`] snapshot, computing `paired` against a
    /// canonical-fingerprint set of already-paired device ids.
    pub fn from_peer_info(peer: PeerInfo, paired: bool) -> Self {
        DiscoveredPeer {
            device_id: peer.device_id,
            device_name: peer.device_name,
            ip_addrs: peer.ip_addrs.iter().map(|ip| ip.to_string()).collect(),
            port: peer.port,
            bport: peer.bport,
            paired,
        }
    }
}

/// Polled pairing status surfaced to Kotlin by [`crate::pair_get_sas`].
///
/// `state` is the lowercase machine token (`idle` / `initiating` /
/// `awaiting_sas` / `confirmed` / `rejected` / `aborted` / `timed_out`). `sas`
/// and `role` are populated while active; the `peer_*` outputs are populated
/// ONLY when `state == "confirmed"`.
///
/// # SECURITY NOTE — `session_key` crosses the FFI boundary unzeroized.
/// UniFFI copies it into a Kotlin `ByteArray`. The Kotlin layer MUST zero that
/// array after deriving the content sync key (KEK-wrap into AndroidKeystore);
/// never log it.
#[derive(Debug, Clone, Default)]
pub struct PairStatus {
    pub state: String,
    pub sas: Option<String>,
    pub role: Option<String>,
    pub peer_fingerprint: Option<String>,
    pub peer_sync_addr: Option<String>,
    pub session_key: Option<Vec<u8>>,
    pub peer_provisioning: Option<SyncProvisioning>,
}

impl PairStatus {
    /// Build from a [`PairingState`] snapshot. `peer_*` fields are only set in
    /// the `Confirmed` terminal state.
    pub fn from_state(state: &PairingState) -> Self {
        let mut status = PairStatus {
            state: state.as_str().to_string(),
            sas: state.sas().map(|s| s.to_string()),
            role: state.role().map(|r| r.as_str().to_string()),
            ..PairStatus::default()
        };
        if let Some(c) = state.confirmed() {
            status.peer_fingerprint = Some(c.peer_fingerprint.clone());
            status.peer_sync_addr = Some(c.peer_sync_addr.clone());
            status.session_key = Some(c.session_key.clone());
            status.peer_provisioning = c.peer_provisioning.clone();
        }
        status
    }
}

/// Pick the IPv4-first resolvable `host:port` to dial for a discovered peer.
///
/// mDNS often resolves both IPv4 and IPv6 (incl. link-local) addresses; the
/// bootstrap dialer wants a single routable address. Prefer IPv4 (most reliable
/// on consumer LANs / Android), fall back to the first address. Returns `None`
/// when the peer advertised no addresses.
pub fn ipv4_first_addr(peer: &PeerInfo) -> Option<std::net::SocketAddr> {
    let ip = peer
        .ip_addrs
        .iter()
        .find(|ip| ip.is_ipv4())
        .or_else(|| peer.ip_addrs.first())?;
    Some(std::net::SocketAddr::new(
        *ip,
        peer.bport.unwrap_or(peer.port),
    ))
}

/// Map a `pair_with_discovered` failure outcome (handshake error, timeout, or
/// rejection) onto a terminal [`PairingState`] for the coordinator. A handshake
/// `Err` from a confirm-rejected SAS is reported as `Rejected`; everything else
/// (network/PAKE/MitM failure) is `Aborted`. Used by the spawned initiator task.
pub fn outcome_for_initiator_error(rejected: bool) -> PairingState {
    if rejected {
        PairingState::Rejected
    } else {
        PairingState::Aborted
    }
}

/// Build a `CopypasteError::P2pError` with a fixed reason (helper so the FFI
/// surface never constructs the variant inline at multiple call sites).
pub fn p2p_err(reason: impl Into<String>) -> CopypasteError {
    CopypasteError::P2pError {
        reason: reason.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn confirmed_sample() -> ConfirmedPairing {
        ConfirmedPairing {
            peer_fingerprint: "abc123".to_string(),
            peer_sync_addr: "10.0.0.2:51515".to_string(),
            session_key: vec![0x42u8; 32],
            peer_provisioning: None,
        }
    }

    #[test]
    fn fresh_coordinator_is_idle() {
        let c = PairingCoordinator::new();
        assert!(c.snapshot().is_idle());
        assert_eq!(c.snapshot().as_str(), "idle");
    }

    #[test]
    fn begin_transitions_idle_to_initiating() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator));
        let s = c.snapshot();
        assert_eq!(s.as_str(), "initiating");
        assert_eq!(s.role(), Some(PairingRole::Initiator));
        assert!(s.is_active());
    }

    #[test]
    fn concurrent_begin_is_rejected_single_active() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator));
        // A second begin while non-idle must be refused (single active pairing).
        assert!(!c.try_begin(PairingRole::Responder));
        assert_eq!(c.snapshot().role(), Some(PairingRole::Initiator));
    }

    #[test]
    fn enter_awaiting_sas_exposes_sas_and_role() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Responder));
        let _rx = c.enter_awaiting_sas("123456".to_string(), PairingRole::Responder);
        let s = c.snapshot();
        assert_eq!(s.as_str(), "awaiting_sas");
        assert_eq!(s.sas(), Some("123456"));
        assert_eq!(s.role(), Some(PairingRole::Responder));
    }

    #[tokio::test]
    async fn deliver_decision_accept_fires_oneshot_true() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator));
        let rx = c.enter_awaiting_sas("000000".to_string(), PairingRole::Initiator);
        assert!(c.deliver_decision(true));
        assert!(rx.await.unwrap());
    }

    #[tokio::test]
    async fn reject_delivers_false_then_finish_rejected() {
        // A reject must propagate `false` to the handshake so it sends REJECT in
        // frame 10a and drops/zeroizes the session key (no persist, no rotate).
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator));
        let rx = c.enter_awaiting_sas("424242".to_string(), PairingRole::Initiator);
        assert!(c.deliver_decision(false));
        assert!(!rx.await.unwrap());
        c.finish(PairingState::Rejected);
        assert_eq!(c.snapshot().as_str(), "rejected");
        assert!(c.snapshot().is_terminal());
        // A rejected pairing exposes NO key material.
        assert!(c.snapshot().confirmed().is_none());
    }

    #[tokio::test]
    async fn abort_drops_confirm_channel_so_handshake_sees_rejection() {
        // pair_abort must cancel the in-flight handshake: dropping the sender
        // resolves the await with an Err, which the callback treats as reject.
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Responder));
        let rx = c.enter_awaiting_sas("999999".to_string(), PairingRole::Responder);
        c.abort();
        assert!(rx.await.is_err(), "dropping the sender must error the recv");
        assert_eq!(c.snapshot().as_str(), "aborted");
        assert!(c.snapshot().confirmed().is_none());
    }

    #[tokio::test]
    async fn timeout_path_drops_keys() {
        // Simulate the SAS_CONFIRM_TIMEOUT branch: the handshake's confirm
        // closure times out waiting on the oneshot, reports TimedOut, and the
        // key never reaches a Confirmed state.
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator));
        let rx = c.enter_awaiting_sas("555555".to_string(), PairingRole::Initiator);
        // No deliver_decision; emulate the timeout firing.
        let timed_out = tokio::time::timeout(std::time::Duration::from_millis(20), rx).await;
        assert!(timed_out.is_err(), "no decision delivered → recv times out");
        c.finish(PairingState::TimedOut);
        assert_eq!(c.snapshot().as_str(), "timed_out");
        assert!(c.snapshot().confirmed().is_none());
    }

    #[test]
    fn deliver_decision_without_pending_is_false() {
        let c = PairingCoordinator::new();
        assert!(!c.deliver_decision(true));
    }

    #[test]
    fn confirmed_carries_ffi_outputs_for_persistence() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator));
        c.finish(PairingState::Confirmed(confirmed_sample()));
        let s = c.snapshot();
        assert_eq!(s.as_str(), "confirmed");
        let out = s.confirmed().expect("confirmed carries outputs");
        assert_eq!(out.peer_fingerprint, "abc123");
        assert_eq!(out.peer_sync_addr, "10.0.0.2:51515");
        assert_eq!(out.session_key.len(), 32);

        // PairStatus surfaces the peer_* only on confirmed.
        let status = PairStatus::from_state(&s);
        assert_eq!(status.state, "confirmed");
        assert_eq!(status.peer_fingerprint.as_deref(), Some("abc123"));
        assert!(status.session_key.is_some());
    }

    #[test]
    fn pair_status_hides_peer_fields_while_awaiting() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Responder));
        let _rx = c.enter_awaiting_sas("121212".to_string(), PairingRole::Responder);
        let status = PairStatus::from_state(&c.snapshot());
        assert_eq!(status.state, "awaiting_sas");
        assert_eq!(status.sas.as_deref(), Some("121212"));
        assert_eq!(status.role.as_deref(), Some("responder"));
        assert!(
            status.peer_fingerprint.is_none(),
            "peer_* only on confirmed"
        );
        assert!(status.session_key.is_none(), "no key before confirmed");
    }

    #[test]
    fn reset_returns_to_idle_for_next_pairing() {
        let c = PairingCoordinator::new();
        assert!(c.try_begin(PairingRole::Initiator));
        c.finish(PairingState::Confirmed(confirmed_sample()));
        assert_eq!(c.snapshot().as_str(), "confirmed");
        c.reset();
        assert!(c.snapshot().is_idle());
        // A fresh pairing may begin after reset.
        assert!(c.try_begin(PairingRole::Responder));
    }

    #[test]
    fn role_wire_strings() {
        assert_eq!(PairingRole::Initiator.as_str(), "initiator");
        assert_eq!(PairingRole::Responder.as_str(), "responder");
    }

    #[test]
    fn ipv4_first_prefers_v4() {
        use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
        let peer = PeerInfo {
            device_id: "d".into(),
            device_name: "n".into(),
            ip_addrs: vec![
                IpAddr::V6(Ipv6Addr::LOCALHOST),
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 5)),
            ],
            port: 51515,
            bport: Some(60000),
        };
        let addr = ipv4_first_addr(&peer).expect("addr");
        assert!(addr.ip().is_ipv4(), "IPv4 must be preferred");
        assert_eq!(addr.port(), 60000, "bport dialed when present");
    }

    #[test]
    fn ipv4_first_falls_back_to_port_without_bport() {
        use std::net::{IpAddr, Ipv4Addr};
        let peer = PeerInfo {
            device_id: "d".into(),
            device_name: "n".into(),
            ip_addrs: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 9))],
            port: 51515,
            bport: None,
        };
        let addr = ipv4_first_addr(&peer).expect("addr");
        assert_eq!(addr.port(), 51515, "falls back to sync port without bport");
    }

    #[test]
    fn ipv4_first_none_for_no_addrs() {
        let peer = PeerInfo {
            device_id: "d".into(),
            device_name: "n".into(),
            ip_addrs: vec![],
            port: 51515,
            bport: Some(60000),
        };
        assert!(ipv4_first_addr(&peer).is_none());
    }

    #[test]
    fn discovered_peer_from_peer_info_maps_fields() {
        use std::net::{IpAddr, Ipv4Addr};
        let peer = PeerInfo {
            device_id: "fp123".into(),
            device_name: "Alice's Mac".into(),
            ip_addrs: vec![IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))],
            port: 51515,
            bport: Some(60000),
        };
        let dp = DiscoveredPeer::from_peer_info(peer, true);
        assert_eq!(dp.device_id, "fp123");
        assert_eq!(dp.ip_addrs, vec!["10.0.0.2".to_string()]);
        assert_eq!(dp.port, 51515);
        assert_eq!(dp.bport, Some(60000));
        assert!(dp.paired);
    }

    #[test]
    fn outcome_mapping() {
        assert!(matches!(
            outcome_for_initiator_error(true),
            PairingState::Rejected
        ));
        assert!(matches!(
            outcome_for_initiator_error(false),
            PairingState::Aborted
        ));
    }

    /// The SAS this crate surfaces is `copypaste_p2p::pake::derive_sas` — the
    /// SAME function the macOS daemon uses. Re-verify its load-bearing
    /// properties here (mirroring the p2p `derive_sas_*` tests) so the Android
    /// FFI's authentication contract is pinned: deterministic, 6 decimal digits,
    /// and domain-separated (a different `bound_key` → a (near-certainly)
    /// different SAS, which is what a MitM-per-leg attack trips on).
    #[test]
    fn sas_is_deterministic_six_digits_and_domain_separated() {
        use copypaste_p2p::pake::derive_sas;

        let key_a = [0x11u8; 32];
        let key_b = [0x22u8; 32];

        let sas_a1 = derive_sas(&key_a);
        let sas_a2 = derive_sas(&key_a);
        let sas_b = derive_sas(&key_b);

        // Deterministic on the same bound_key (both honest endpoints agree).
        assert_eq!(sas_a1, sas_a2, "SAS must be deterministic per bound_key");
        // Exactly 6 decimal digits.
        assert_eq!(sas_a1.len(), 6, "SAS must be 6 chars");
        assert!(
            sas_a1.chars().all(|c| c.is_ascii_digit()),
            "SAS must be all decimal digits"
        );
        // Domain separation: a different bound_key (the MitM-per-leg case)
        // yields a different SAS so the humans see a mismatch and abort.
        assert_ne!(
            sas_a1, sas_b,
            "different bound_keys must derive different SAS"
        );
    }

    /// The fixed discovery PAKE password is non-empty and stable (both ends must
    /// agree on it for opaque-ke to converge — see its docs).
    #[test]
    fn discovery_password_is_stable_nonempty() {
        assert!(!DISCOVERY_PAIRING_PASSWORD.is_empty());
        assert_eq!(
            DISCOVERY_PAIRING_PASSWORD,
            "copypaste/p2p/lan-sas-discovery/v1"
        );
    }
}
