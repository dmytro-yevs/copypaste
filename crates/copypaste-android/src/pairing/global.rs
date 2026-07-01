//! Process-global discovery + pairing singleton, plus the constants shared by
//! both the initiator and standing-responder handshake paths. See the
//! `pairing` module doc comment for the full protocol/security rationale.

use std::sync::{Arc, Mutex, OnceLock};

use tokio::task::JoinHandle;

use copypaste_p2p::discovery::DiscoveryService;

use super::coordinator::PairingCoordinator;

/// Fixed, well-known PAKE password for the LAN/SAS *discovery* pairing path.
///
/// Re-exported from [`copypaste_p2p::DISCOVERY_PAIRING_PASSWORD`] so every
/// platform (Android initiator + standing responder AND the macOS daemon's
/// initiator + responder) uses ONE byte-identical value — opaque-ke is
/// asymmetric, so `ClientLogin::finish` only succeeds against a `PasswordFile`
/// registered for the IDENTICAL password. Authentication comes ENTIRELY from
/// the post-channel-binding human SAS compare, not from password secrecy, so
/// this is NON-SECRET by design. See the doc comment on the source constant for
/// the full rationale. The value is unchanged from what Android already shipped,
/// so Android↔Android wire bytes / checksums are unaffected.
pub use copypaste_p2p::DISCOVERY_PAIRING_PASSWORD;

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

/// Process-global discovery + pairing state for the Android FFI.
///
/// Holds the single [`PairingCoordinator`], the live [`DiscoveryService`] (mDNS
/// browse + advertise), the discovery browse [`JoinHandle`], the standing
/// responder task handle, and the in-flight initiator task handle. There is at
/// most ONE of each because exactly one pairing may be in flight at a time.
pub struct AndroidPairing {
    /// The single shared pairing coordinator.
    pub coordinator: Arc<PairingCoordinator>,
    /// Live discovery service (advertise + browse). `None` until `start`.
    discovery: Mutex<Option<Arc<DiscoveryService>>>,
    /// Background browse task spawned by `DiscoveryService::start`. Aborted on
    /// `stop`.
    discovery_task: Mutex<Option<JoinHandle<()>>>,
    /// The standing bootstrap responder task (re-binds `bport` and accepts one
    /// inbound discovery-pair connection per iteration). Aborted on `stop`.
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

    /// The live discovery service, if `start` has run.
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
