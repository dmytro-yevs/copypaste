//! The peer node: everything that has to be running for pairing and sync to
//! work, with no daemon around it.
//!
//! The rest of this crate provides the parts — a Noise channel, a last-write-
//! wins session, an mDNS browser — and deliberately knows nothing about a
//! database. This module owns the parts: the paired-device list, the
//! advertisement, the inbound listener and its concurrency limit, and the
//! pairing, unpairing and sync operations a user drives.
//!
//! It is generic over [`SyncSource`](crate::sync::SyncSource), which is the
//! whole of what it needs from a history. The desktop daemon supplies one over
//! its store; the Android app supplies one over the same core store in its own
//! process. Neither is a `Node` of its own — that is what put pairing out of
//! reach of everything except the daemon binary.
//!
//! # Discovery is a convenience, never a dependency
//!
//! `Discovery::start` returns `Ok` on a host with no multicast — a container,
//! or a corporate network with mDNS filtered. The peer list stays empty,
//! `online` reads false for everyone, and an explicit address still pairs and
//! still syncs. Nothing here treats a discovery failure as an error.

mod activity;
mod channel;
mod dial;
mod error;
mod listen;
mod pairing;
mod pairing_ceremony;
mod probe;

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

#[cfg(test)]
use tokio::sync::oneshot;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::discovery::{DiscoveredPeer, Discovery};
use crate::peers::{CursorStore, Peer, PeerStore, DEFAULT_CURSOR_FILE_NAME};
use crate::sync::SyncOutcome;
use crate::{AuthenticatedDeviceProfile, DeviceProfile};

pub use channel::{NoiseChannel, READ_TIMEOUT, SESSION_TIMEOUT};
pub use error::NodeError;
pub use listen::{bind, listen};
pub use pairing::{
    PairingInvite, PairingPeer, PairingPhase, PairingRole, PairingStatus, PAIRING_CONFIRM_TIMEOUT,
    PAIRING_INVITE_TTL,
};
pub use probe::{AuthenticatedReachability, ProbeState, PROBE_FAILURE_FRESH_MS, PROBE_FRESH_MS};

#[derive(Clone)]
pub struct SyncCycle {
    cancel: Arc<Mutex<CancellationToken>>,
    #[cfg(test)]
    pause: Arc<Mutex<Option<CommitPause>>>,
}

#[cfg(test)]
#[derive(Clone)]
pub(crate) struct CommitPause {
    entered_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    entered_rx: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
    release_tx: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    release_rx: Arc<Mutex<Option<oneshot::Receiver<()>>>>,
}

#[cfg(test)]
impl CommitPause {
    pub(crate) async fn wait_entered(&self) {
        let entered_rx = self
            .entered_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .expect("commit pause was already observed");
        entered_rx.await.expect("commit pause was dropped");
    }

    pub(crate) fn release(&self) {
        self.release_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take()
            .expect("commit pause was already released")
            .send(())
            .expect("commit pause was dropped");
    }
}

impl SyncCycle {
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancel: Arc::new(Mutex::new(CancellationToken::new())),
            #[cfg(test)]
            pause: Arc::new(Mutex::new(None)),
        }
    }

    pub fn set_enabled(&self, enabled: bool) {
        let mut cancel = self
            .cancel
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if enabled {
            if cancel.is_cancelled() {
                *cancel = CancellationToken::new();
            }
        } else {
            cancel.cancel();
        }
    }

    #[must_use]
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub(crate) fn commit<T>(
        &self,
        cancel: &CancellationToken,
        action: impl FnOnce() -> T,
    ) -> Option<T> {
        let _cycle = self
            .cancel
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        (!cancel.is_cancelled() && !_cycle.is_cancelled()).then(action)
    }

    #[cfg(test)]
    pub(crate) fn pause_before_commit(&self) -> CommitPause {
        let pause = CommitPause {
            entered_tx: Arc::new(Mutex::new(None)),
            entered_rx: Arc::new(Mutex::new(None)),
            release_tx: Arc::new(Mutex::new(None)),
            release_rx: Arc::new(Mutex::new(None)),
        };
        let (entered_tx, entered_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        *pause
            .entered_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(entered_tx);
        *pause
            .entered_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(entered_rx);
        *pause
            .release_tx
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(release_tx);
        *pause
            .release_rx
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(release_rx);
        *self.pause.lock().unwrap_or_else(|error| error.into_inner()) = Some(pause.clone());
        pause
    }

    #[cfg(test)]
    pub(crate) async fn wait_before_commit(&self) {
        let pause = self
            .pause
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .take();
        if let Some(pause) = pause {
            pause
                .entered_tx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
                .expect("commit pause was already entered")
                .send(())
                .expect("commit pause observer was dropped");
            let release_rx = pause
                .release_rx
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
                .expect("commit pause was already released");
            release_rx.await.expect("commit pause releaser was dropped");
        }
    }
}

impl Default for SyncCycle {
    fn default() -> Self {
        Self::new()
    }
}

/// How many peers may be mid-session at once, inbound.
///
/// A session holds a database connection's worth of work and a decrypted batch
/// in memory, and a device has a handful of peers, not hundreds. Connections
/// past the limit are dropped without a handshake: refusing early tells an
/// unauthenticated dialler nothing it did not already know from the open port.
pub(super) const MAX_CONCURRENT_PEER_SESSIONS: usize = 4;

const DISCOVERY_INTEREST_MS: i64 = 60_000;
const AUTHENTICATED_PROFILE_TTL_MS: i64 = 5 * 60_000;
const DISCOVERY_CANDIDATE_COOLDOWN: Duration = Duration::from_secs(60);
const MAX_DISCOVERY_ATTEMPTS_PER_PAIRING: usize = 1;
const MAX_UNVERIFIED_DISCOVERY_ATTEMPTS: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DialCandidateSource {
    Discovery,
    Persisted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DialCandidate {
    pub addr: SocketAddr,
    pub source: DialCandidateSource,
}

/// An authenticated listener endpoint, never an inbound connection's source port.
#[derive(Clone, Copy)]
struct ListenerEndpoint(SocketAddr);

impl ListenerEndpoint {
    fn dialled(addr: SocketAddr) -> Self {
        Self(addr)
    }

    fn advertised(addr: SocketAddr) -> Option<Self> {
        crate::netif::is_dialable(&addr).then_some(Self(addr))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
struct DiscoveryCandidateKey {
    pairing_id: String,
    addr: SocketAddr,
}

#[derive(Default)]
struct DiscoveryCandidateCooldowns {
    endpoint_failures: HashMap<DiscoveryCandidateKey, Instant>,
    pairing_attempts: HashMap<String, VecDeque<Instant>>,
    global_attempts: VecDeque<Instant>,
}

/// Everything the peer half of a device shares.
///
/// Constructed once and never rebuilt; no field is optional except
/// `discovery`, where `None` is a decision (the user turned LAN visibility
/// off) rather than a degraded state.
pub struct Node {
    sync_activity: Arc<activity::SyncActivity>,
    peers: PeerStore,
    cursors: CursorStore,
    discovery: Option<Discovery>,
    /// The port [`listen`] binds, which is what a pairing tells a peer to dial.
    port: u16,
    /// The address of the listener that is actually running.  A configured
    /// port is not an endpoint: tests, Android and a failed bind can all have
    /// a different answer.
    listen_addr: RwLock<Option<SocketAddr>>,
    bound_addr: RwLock<Option<SocketAddr>>,
    sessions: Arc<Semaphore>,
    browse_until_ms: AtomicI64,
    lan_visible: AtomicBool,
    pairing: pairing::PairingManager,
    profiles: RwLock<HashMap<String, AuthenticatedDeviceProfile>>,
    reachability: RwLock<HashMap<String, AuthenticatedReachability>>,
    probe_flights: Mutex<HashSet<String>>,
    probes: Arc<Semaphore>,
    discovery_candidate_cooldowns: Mutex<DiscoveryCandidateCooldowns>,
}

impl std::fmt::Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Node")
            .field("port", &self.port)
            .field("peers", &self.peers.len())
            .finish_non_exhaustive()
    }
}

impl Node {
    #[must_use]
    pub fn new(
        peers: PeerStore,
        discovery: Option<Discovery>,
        port: u16,
        lan_visible: bool,
    ) -> Self {
        let cursors = CursorStore::open(
            &peers
                .path()
                .parent()
                .unwrap_or_else(|| std::path::Path::new("."))
                .join(DEFAULT_CURSOR_FILE_NAME),
        );
        cursors.retain(
            &peers
                .list()
                .iter()
                .map(|peer| peer.pairing_id.clone())
                .collect::<Vec<_>>(),
        );
        let node = Self {
            sync_activity: activity::SyncActivity::new(),
            peers,
            cursors,
            discovery,
            port,
            listen_addr: RwLock::new(None),
            bound_addr: RwLock::new(None),
            sessions: Arc::new(Semaphore::new(MAX_CONCURRENT_PEER_SESSIONS)),
            browse_until_ms: AtomicI64::new(0),
            lan_visible: AtomicBool::new(lan_visible),
            pairing: pairing::PairingManager::new(),
            profiles: RwLock::new(HashMap::new()),
            reachability: RwLock::new(HashMap::new()),
            probe_flights: Mutex::new(HashSet::new()),
            probes: Arc::new(Semaphore::new(MAX_CONCURRENT_PEER_SESSIONS)),
            discovery_candidate_cooldowns: Mutex::new(DiscoveryCandidateCooldowns::default()),
        };
        if let Some(discovery) = &node.discovery {
            discovery.set_listener_port(0);
        }
        node.republish();
        node
    }

    #[must_use]
    pub fn peers(&self) -> &PeerStore {
        &self.peers
    }

    pub fn sync_status(&self, enabled: bool) -> copypaste_ipc::SyncStatus {
        self.sync_activity.snapshot(&self.peers.list(), enabled)
    }

    pub fn subscribe_sync_changes(&self) -> tokio::sync::watch::Receiver<()> {
        self.sync_activity.subscribe()
    }

    #[must_use]
    pub fn cursors(&self) -> &CursorStore {
        &self.cursors
    }

    /// Makes a locally written version visible below every peer's cursor.
    pub fn note_local_version(&self, floor_ms: i64) {
        self.cursors.note_local(floor_ms);
    }

    /// Order is load-bearing: `record_session` clears this peer's relay floor,
    /// so lowering the *other* peers' floors has to happen first. Swapped, the
    /// session would wipe the floor this exchange just established.
    pub(crate) fn record_cursor(&self, pairing_id: &str, outcome: &SyncOutcome) {
        if let Some(floor) = outcome.applied_floor {
            self.cursors.note_applied(pairing_id, floor);
        }
        self.cursors
            .record_session(pairing_id, outcome.cursor.since_ms);
    }

    #[must_use]
    pub fn discovery(&self) -> Option<&Discovery> {
        self.discovery.as_ref()
    }

    #[must_use]
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Bind and publish the actual listener before any client can request it.
    pub fn bind_listener(&self) -> std::io::Result<std::net::TcpListener> {
        let listener = bind(self.port)?;
        self.set_listen_addr(listener.local_addr()?);
        Ok(listener)
    }

    /// Everything currently visible on the LAN. Empty when discovery is off or
    /// degraded, which is never an error: an explicit address always works.
    #[must_use]
    pub fn seen(&self) -> Vec<DiscoveredPeer> {
        self.note_discovery_interest();
        self.discovery
            .as_ref()
            .map(Discovery::peers)
            .unwrap_or_default()
    }

    pub fn note_discovery_interest(&self) {
        let until = crate::now_ms().saturating_add(DISCOVERY_INTEREST_MS);
        self.browse_until_ms.fetch_max(until, Ordering::Relaxed);
        self.reconcile_discovery();
    }

    pub fn set_lan_visibility(&self, visible: bool) {
        if self.lan_visible.swap(visible, Ordering::Relaxed) != visible {
            self.reconcile_discovery();
        }
    }

    pub fn reconcile_discovery(&self) {
        let Some(discovery) = self.discovery.as_ref() else {
            return;
        };
        if self.wants_discovery() != discovery.is_running() {
            self.republish();
        }
    }

    fn wants_discovery(&self) -> bool {
        self.lan_visible.load(Ordering::Relaxed)
            && (!self.peers.is_empty()
                || self.pairing.is_active()
                || crate::now_ms() < self.browse_until_ms.load(Ordering::Relaxed))
    }

    /// Whether a given pairing is visible right now.
    #[must_use]
    pub fn find(&self, pairing_id: &str) -> Option<DiscoveredPeer> {
        self.discovery.as_ref()?.find(pairing_id)
    }

    /// Candidate endpoints for one trusted pairing, in freshness order.
    ///
    /// mDNS is only an address hint: every candidate still has to complete the
    /// pairing's Noise handshake before it can affect peer state. Retaining the
    /// previous authenticated address as a fallback handles transient discovery
    /// loss, while preferring the current record handles DHCP changes.
    #[must_use]
    pub fn dial_candidates(&self, peer: &Peer) -> Vec<SocketAddr> {
        self.dial_candidates_with_sources(peer, false)
            .into_iter()
            .map(|candidate| candidate.addr)
            .collect()
    }

    pub(crate) fn dial_candidates_with_sources(
        &self,
        peer: &Peer,
        reserve_discovery: bool,
    ) -> Vec<DialCandidate> {
        let mut candidates = Vec::with_capacity(2);
        if let Some(found) = self.find(&peer.pairing_id) {
            if peer.last_addr == Some(found.addr) {
                candidates.push(DialCandidate {
                    addr: found.addr,
                    source: DialCandidateSource::Persisted,
                });
            } else if self.discovery_candidate_allowed(&peer.pairing_id, found.addr)
                && (!reserve_discovery
                    || self.reserve_unverified_discovery_candidate(&peer.pairing_id, found.addr))
            {
                candidates.push(DialCandidate {
                    addr: found.addr,
                    source: DialCandidateSource::Discovery,
                });
            }
        }
        if let Some(last_addr) = peer.last_addr {
            if !candidates
                .iter()
                .any(|candidate| candidate.addr == last_addr)
            {
                candidates.push(DialCandidate {
                    addr: last_addr,
                    source: DialCandidateSource::Persisted,
                });
            }
        }
        candidates
    }

    /// Lets an explicit user rescan retry a current mDNS address once. It does
    /// not touch trusted persisted endpoints, pairing credentials, or any other
    /// liveness observation.
    pub fn clear_discovery_candidate_cooldowns(&self) {
        let mut cooldowns = self
            .discovery_candidate_cooldowns
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        cooldowns.endpoint_failures.clear();
        cooldowns.pairing_attempts.clear();
        cooldowns.global_attempts.clear();
    }

    pub(crate) fn record_discovery_candidate_failure(&self, pairing_id: &str, addr: SocketAddr) {
        let now = Instant::now();
        let mut cooldowns = self
            .discovery_candidate_cooldowns
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prune_discovery_candidate_cooldowns(&mut cooldowns, now);
        cooldowns.endpoint_failures.insert(
            DiscoveryCandidateKey {
                pairing_id: pairing_id.to_string(),
                addr,
            },
            now,
        );
    }

    fn discovery_candidate_allowed(&self, pairing_id: &str, addr: SocketAddr) -> bool {
        let now = Instant::now();
        let mut cooldowns = self
            .discovery_candidate_cooldowns
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prune_discovery_candidate_cooldowns(&mut cooldowns, now);
        let key = DiscoveryCandidateKey {
            pairing_id: pairing_id.to_string(),
            addr,
        };
        !cooldowns.endpoint_failures.contains_key(&key)
            && cooldowns
                .pairing_attempts
                .get(pairing_id)
                .is_none_or(|attempts| attempts.len() < MAX_DISCOVERY_ATTEMPTS_PER_PAIRING)
            && cooldowns.global_attempts.len() < MAX_UNVERIFIED_DISCOVERY_ATTEMPTS
    }

    fn reserve_unverified_discovery_candidate(&self, pairing_id: &str, addr: SocketAddr) -> bool {
        let now = Instant::now();
        let mut cooldowns = self
            .discovery_candidate_cooldowns
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        Self::prune_discovery_candidate_cooldowns(&mut cooldowns, now);
        let key = DiscoveryCandidateKey {
            pairing_id: pairing_id.to_string(),
            addr,
        };
        if cooldowns.endpoint_failures.contains_key(&key)
            || cooldowns
                .pairing_attempts
                .get(pairing_id)
                .is_some_and(|attempts| attempts.len() >= MAX_DISCOVERY_ATTEMPTS_PER_PAIRING)
            || cooldowns.global_attempts.len() >= MAX_UNVERIFIED_DISCOVERY_ATTEMPTS
        {
            return false;
        }
        cooldowns
            .pairing_attempts
            .entry(pairing_id.to_string())
            .or_default()
            .push_back(now);
        cooldowns.global_attempts.push_back(now);
        true
    }

    fn prune_discovery_candidate_cooldowns(
        cooldowns: &mut DiscoveryCandidateCooldowns,
        now: Instant,
    ) {
        cooldowns.endpoint_failures.retain(|_, failed_at| {
            now.saturating_duration_since(*failed_at) < DISCOVERY_CANDIDATE_COOLDOWN
        });
        cooldowns.pairing_attempts.retain(|_, attempts| {
            while attempts.front().is_some_and(|attempted_at| {
                now.saturating_duration_since(*attempted_at) >= DISCOVERY_CANDIDATE_COOLDOWN
            }) {
                attempts.pop_front();
            }
            !attempts.is_empty()
        });
        while cooldowns
            .global_attempts
            .front()
            .is_some_and(|attempted_at| {
                now.saturating_duration_since(*attempted_at) >= DISCOVERY_CANDIDATE_COOLDOWN
            })
        {
            cooldowns.global_attempts.pop_front();
        }
    }

    #[must_use]
    pub fn authenticated_profile(&self, pairing_id: &str) -> Option<AuthenticatedDeviceProfile> {
        let now = crate::now_ms();
        let mut profiles = self
            .profiles
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        profiles.retain(|_, observed| now < observed.fresh_until_ms);
        if let Some(observed) = profiles.get(pairing_id).cloned() {
            return Some(observed);
        }
        self.peers.get(pairing_id).and_then(|peer| {
            peer.profile
                .clone()
                .map(|profile| AuthenticatedDeviceProfile {
                    profile,
                    observed_at_ms: peer.profile_observed_at_ms,
                    fresh_until_ms: peer
                        .profile_observed_at_ms
                        .saturating_add(AUTHENTICATED_PROFILE_TTL_MS),
                })
        })
    }

    pub(crate) fn record_authenticated_profile(
        &self,
        pairing_id: &str,
        profile: Option<&DeviceProfile>,
    ) {
        let Some(profile) = profile else {
            return;
        };
        let observed_at_ms = crate::now_ms();
        self.profiles
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(
                pairing_id.to_string(),
                AuthenticatedDeviceProfile {
                    profile: profile.clone(),
                    observed_at_ms,
                    fresh_until_ms: observed_at_ms.saturating_add(AUTHENTICATED_PROFILE_TTL_MS),
                },
            );
        let Some(peer) = self.peers.get(pairing_id) else {
            return;
        };
        let updated = Peer {
            pairing_id: peer.pairing_id.clone(),
            device_id: peer.device_id.clone(),
            name: peer.name.clone(),
            psk: peer.psk,
            last_addr: peer.last_addr,
            last_seen_ms: peer.last_seen_ms,
            profile: Some(profile.clone()),
            profile_observed_at_ms: observed_at_ms,
        };
        match self.peers.touch(updated) {
            Ok(true) => {}
            Ok(false) => debug!(pairing_id, "profile observation outlived its pairing"),
            Err(error) => warn!(%error, "could not persist an authenticated device profile"),
        }
    }

    /// The most recent authenticated probe observation for a pairing.
    ///
    /// Expired observations remain available for diagnostics; callers must use
    /// [`AuthenticatedReachability::state_at`] to fail closed to `Unknown`.
    #[must_use]
    pub fn authenticated_reachability(
        &self,
        pairing_id: &str,
    ) -> Option<AuthenticatedReachability> {
        self.reachability
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(pairing_id)
            .cloned()
    }

    pub(crate) fn record_reachability(
        &self,
        pairing_id: &str,
        observation: AuthenticatedReachability,
    ) {
        let mut reachability = self
            .reachability
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // An inbound probe proves presence but cannot measure our round trip.
        // Keep a current local measurement without extending its lifetime.
        if observation.state == ProbeState::Online
            && observation.latency_ms.is_none()
            && reachability.get(pairing_id).is_some_and(|previous| {
                previous.latency_ms.is_some() && previous.is_current_at(observation.observed_at_ms)
            })
        {
            return;
        }
        reachability.insert(pairing_id.to_string(), observation);
    }

    /// Re-advertise after the set of pairings changed.
    ///
    /// Best-effort by construction: a new pairing must not fail because mDNS is
    /// unavailable, and the address the user was given still works.
    pub fn republish(&self) {
        let ids: Vec<String> = self
            .peers
            .list()
            .into_iter()
            // Cloned rather than moved out: `Peer` has a `Drop` that wipes its
            // key, so its fields cannot be destructured.
            .map(|peer| peer.pairing_id.clone())
            .collect();
        let Some(discovery) = self.discovery.as_ref() else {
            return;
        };
        if !self.wants_discovery() {
            discovery.stop();
            return;
        }
        if let Err(e) = discovery.publish(&ids) {
            debug!(error = %e, "could not republish the discovery record");
        }
    }

    pub fn set_device_name(&self, device_name: &str) {
        let Some(discovery) = self.discovery.as_ref() else {
            return;
        };
        if let Err(e) = discovery.set_device_name(device_name) {
            warn!(error = %e, "could not update the discovery device name");
            return;
        }
        self.republish();
    }

    /// Where a peer should dial this device, when that can be determined.
    #[must_use]
    pub fn listen_addr(&self) -> Option<String> {
        self.listen_addr
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(ToString::to_string)
    }

    fn session_listen_addr(&self, local_addr: Option<SocketAddr>) -> Option<String> {
        let bound = (*self
            .bound_addr
            .read()
            .unwrap_or_else(|error| error.into_inner()))?;
        let advertised = if bound.ip().is_unspecified() {
            local_addr
                .filter(|addr| addr.is_ipv4() == bound.is_ipv4())
                .map(|addr| SocketAddr::new(addr.ip(), bound.port()))
        } else {
            Some(bound)
        }
        .filter(crate::netif::is_dialable);
        if let Some(advertised) = advertised {
            *self
                .listen_addr
                .write()
                .unwrap_or_else(|error| error.into_inner()) = Some(advertised);
            Some(advertised.to_string())
        } else {
            self.listen_addr()
        }
    }

    pub(crate) fn set_listen_addr(&self, addr: SocketAddr) {
        *self
            .bound_addr
            .write()
            .unwrap_or_else(|error| error.into_inner()) = Some(addr);
        let advertised = if addr.ip().is_unspecified() {
            crate::netif::routable_ip()
                .filter(|ip| ip.is_ipv4() == addr.is_ipv4())
                .map(|ip| SocketAddr::new(ip, addr.port()))
        } else {
            Some(addr)
        };
        *self
            .listen_addr
            .write()
            .unwrap_or_else(|error| error.into_inner()) = advertised;
        if let Some(discovery) = &self.discovery {
            discovery.set_listener_port(addr.port());
        }
        self.republish();
    }

    /// Forget a peer. `Ok(false)` when there was no such pairing.
    ///
    /// Local and unilateral: the other device keeps its half until it also
    /// unpairs, which is why this cannot fail on an unreachable peer. What it
    /// does do is remove the pre-shared key from the listener's candidate list,
    /// so that pairing can no longer authenticate here.
    pub fn unpair(&self, pairing_id: &str) -> Result<bool, NodeError> {
        let removed = self.peers.remove(pairing_id).map_err(|e| {
            warn!(error = %e, "could not remove a pairing");
            NodeError::PeerStore
        })?;
        if removed {
            self.cursors.forget(pairing_id);
            self.profiles
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(pairing_id);
            self.reachability
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(pairing_id);
            self.probe_flights
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .remove(pairing_id);
            let mut cooldowns = self
                .discovery_candidate_cooldowns
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            cooldowns
                .endpoint_failures
                .retain(|key, _| key.pairing_id != pairing_id);
            cooldowns.pairing_attempts.remove(pairing_id);
            self.republish();
        }
        Ok(removed)
    }

    /// Permanently cut off a pairing and remove all cached non-secret state.
    pub fn revoke(&self, pairing_id: &str, now_ms: i64) -> Result<bool, NodeError> {
        let removed = self.peers.revoke(pairing_id, now_ms).map_err(|error| {
            warn!(%error, "could not revoke a pairing");
            NodeError::PeerStore
        })?;
        self.cursors.forget(pairing_id);
        self.profiles
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(pairing_id);
        self.reachability
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(pairing_id);
        self.probe_flights
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(pairing_id);
        self.republish();
        Ok(removed)
    }

    /// Record that a session with this peer succeeded.
    ///
    /// The name comes off the wire and is cosmetic — never an identity — so it
    /// is only taken when the peer offered one.
    fn touch_peer(
        &self,
        peer: &Peer,
        device_id: Option<&str>,
        addr: Option<ListenerEndpoint>,
        name: Option<&str>,
    ) {
        // A sync or probe may have refreshed authenticated metadata after its
        // caller captured `peer`. Re-read the stored record so that a stale
        // snapshot cannot erase a newer profile observation.
        let current = self
            .peers
            .get(&peer.pairing_id)
            .unwrap_or_else(|| peer.clone());
        let updated = Peer {
            pairing_id: current.pairing_id.clone(),
            device_id: device_id
                .map(str::to_string)
                .or_else(|| current.device_id.clone()),
            name: match name {
                Some(name) if !name.trim().is_empty() => name.to_string(),
                _ => current.name.clone(),
            },
            // `[u8; 32]` is `Copy`, so this reads the field rather than moving
            // out of a type that has a `Drop` (see `peers::Peer`).
            psk: current.psk,
            last_addr: addr.map(|addr| addr.0).or(current.last_addr),
            last_seen_ms: crate::now_ms(),
            profile: current.profile.clone(),
            profile_observed_at_ms: current.profile_observed_at_ms,
        };
        match self.peers.touch(updated) {
            Ok(true) => {}
            Ok(false) => debug!(
                pairing_id = %peer.pairing_id,
                "a session outlived its pairing; not recording it"
            ),
            Err(e) => warn!(error = %e, "could not record a successful peer session"),
        }
    }
}

/// A peer always has *some* name: the list is unreadable otherwise, and the
/// real one arrives with the first hello.
#[must_use]
pub fn placeholder_name(name: &str) -> String {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        "unnamed device".to_string()
    } else {
        trimmed.chars().take(64).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node() -> (Node, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let peers =
            PeerStore::open(&dir.path().join("peers.json"), &crate::peers::testutil::KEY).unwrap();
        (Node::new(peers, None, 0, true), dir)
    }

    fn node_with_discovery() -> (Node, tempfile::TempDir, std::net::TcpListener) {
        let dir = tempfile::tempdir().unwrap();
        let peers =
            PeerStore::open(&dir.path().join("peers.json"), &crate::peers::testutil::KEY).unwrap();
        let discovery = Discovery::dormant("test device", 0).expect("a valid device name");
        {
            let node = Node::new(peers, Some(discovery), 0, true);
            let listener = node.bind_listener().unwrap();
            (node, dir, listener)
        }
    }

    #[test]
    fn session_advertisement_uses_the_real_bound_port_and_route() {
        let (node, _dir) = node();
        let listener = node.bind_listener().unwrap();
        let port = listener.local_addr().unwrap().port();
        *node.listen_addr.write().unwrap() = None;
        let local: SocketAddr = "192.168.1.2:60000".parse().unwrap();
        assert_eq!(
            node.session_listen_addr(Some(local)),
            Some(format!("192.168.1.2:{port}"))
        );
        assert_eq!(node.listen_addr(), Some(format!("192.168.1.2:{port}")));
        assert_ne!(port, 0);
    }

    #[test]
    fn an_explicit_bind_is_not_replaced_by_another_network_interface() {
        let (node, _dir) = node();
        node.set_listen_addr("127.0.0.1:47654".parse().unwrap());
        assert_eq!(
            node.session_listen_addr(Some("192.168.1.2:60000".parse().unwrap())),
            Some("127.0.0.1:47654".into())
        );
    }

    fn browsing(node: &Node) -> bool {
        node.discovery().expect("a discovery handle").is_running()
    }

    #[test]
    fn a_device_with_nothing_to_discover_does_not_run_mdns() {
        let (node, _dir, _listener) = node_with_discovery();
        assert!(!browsing(&node));
    }

    #[test]
    fn an_active_invite_keeps_this_device_discoverable() {
        let (node, _dir, _listener) = node_with_discovery();
        assert!(node.peers().list().is_empty());
        node.pair_create_invite().expect("mint a pairing");
        assert!(browsing(&node));
    }

    #[test]
    fn cancelling_the_last_invite_stops_mdns() {
        let (node, _dir, _listener) = node_with_discovery();
        node.pair_create_invite().expect("mint a pairing");
        assert!(browsing(&node));

        assert_eq!(node.pair_cancel().phase, PairingPhase::Cancelled);
        assert!(!browsing(&node));
    }

    #[test]
    fn looking_at_the_lan_starts_mdns_on_a_device_with_no_peers() {
        let (node, _dir, _listener) = node_with_discovery();
        assert!(node.seen().is_empty());
        assert!(browsing(&node));
    }

    #[test]
    fn mdns_stops_again_once_nobody_is_looking() {
        let (node, _dir, _listener) = node_with_discovery();
        assert!(node.seen().is_empty());
        assert!(browsing(&node));

        node.browse_until_ms.store(0, Ordering::Relaxed);
        node.reconcile_discovery();
        assert!(!browsing(&node));
    }

    #[test]
    fn a_new_pairing_is_memory_only_before_both_devices_confirm() {
        let (node, _dir) = node();
        let pairing = node.pair_create_invite().unwrap();

        assert!(!pairing.code.is_empty());
        assert_ne!(pairing.code, pairing.pairing_id);

        let token = crate::transport::PairingToken::parse(&pairing.code).expect("a valid code");
        assert_eq!(token.pairing_id(), pairing.pairing_id);
        assert!(node.peers().get(&pairing.pairing_id).is_none());
        assert!(node.peers().psks().is_empty());
        assert_eq!(node.pair_progress().phase, PairingPhase::WaitingForPeer);
    }

    /// The code is a secret; nothing may render it but `to_code`.
    #[test]
    fn the_pairing_code_is_redacted_in_debug_output() {
        let (node, _dir) = node();
        let pairing = node.pair_create_invite().unwrap();
        let rendered = format!("{pairing:?}");
        assert!(!rendered.contains(&pairing.code), "{rendered}");
        assert!(rendered.contains(&pairing.pairing_id));
    }

    #[test]
    fn unpairing_removes_the_key_from_the_listeners_candidates() {
        let (node, _dir) = node();
        let token = crate::transport::PairingToken::generate();
        let pairing_id = token.pairing_id();
        node.peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "laptop".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();
        assert_eq!(node.peers().psks().len(), 1);

        assert!(node.unpair(&pairing_id).unwrap());
        assert!(node.peers().psks().is_empty());
        assert!(!node.unpair(&pairing_id).unwrap());
    }

    #[test]
    fn a_stale_session_touch_preserves_a_new_authenticated_profile() {
        let (node, _dir) = node();
        let token = crate::transport::PairingToken::generate();
        let pairing_id = token.pairing_id();
        node.peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .expect("peer");
        let stale = node.peers().get(&pairing_id).expect("peer");
        let profile = DeviceProfile {
            model: Some("Pixel 9".to_string()),
            ..DeviceProfile::default()
        };

        node.record_authenticated_profile(&pairing_id, Some(&profile));
        node.touch_peer(&stale, None, None, Some("renamed phone"));

        let stored = node.peers().get(&pairing_id).expect("peer after touch");
        assert_eq!(stored.name, "renamed phone");
        assert_eq!(stored.profile.as_ref(), Some(&profile));
        assert!(stored.profile_observed_at_ms > 0);
    }

    #[test]
    fn revoking_a_peer_removes_its_durable_and_cached_metadata() {
        let (node, _dir) = node();
        let token = crate::transport::PairingToken::generate();
        let pairing_id = token.pairing_id();
        let profile = DeviceProfile {
            model: Some("Pixel 9".to_string()),
            ..DeviceProfile::default()
        };
        node.peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: Some(profile.clone()),
                profile_observed_at_ms: 1,
            })
            .expect("peer");
        node.record_authenticated_profile(&pairing_id, Some(&profile));
        node.record_reachability(
            &pairing_id,
            AuthenticatedReachability::online(Some(5), crate::now_ms()),
        );

        assert!(node.revoke(&pairing_id, crate::now_ms()).expect("revoke"));
        assert!(node.peers().get(&pairing_id).is_none());
        assert!(node.authenticated_profile(&pairing_id).is_none());
        assert!(node.authenticated_reachability(&pairing_id).is_none());
    }

    #[test]
    fn durable_profile_is_available_after_a_node_restart_but_is_not_reachability() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("peers.json");
        let token = crate::transport::PairingToken::generate();
        let pairing_id = token.pairing_id();
        PeerStore::open(&path, &crate::peers::testutil::KEY)
            .unwrap()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "phone".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 1,
                profile: Some(DeviceProfile {
                    model: Some("Pixel 9".to_string()),
                    ..DeviceProfile::default()
                }),
                profile_observed_at_ms: 1,
            })
            .unwrap();
        let restarted = Node::new(
            PeerStore::open(&path, &crate::peers::testutil::KEY).unwrap(),
            None,
            0,
            true,
        );

        let profile = restarted
            .authenticated_profile(&pairing_id)
            .expect("durable profile");
        assert_eq!(profile.profile.model.as_deref(), Some("Pixel 9"));
        assert_eq!(profile.observed_at_ms, 1);
        assert!(profile.fresh_until_ms < crate::now_ms());
        assert!(restarted.authenticated_reachability(&pairing_id).is_none());
    }

    #[test]
    fn only_one_pairing_ceremony_runs_at_a_time() {
        let (node, _dir) = node();
        let first = node.pair_create_invite().unwrap();
        assert_eq!(
            node.pair_create_invite().unwrap_err(),
            NodeError::PairingBusy
        );
        node.pair_cancel();
        let second = node.pair_create_invite().unwrap();
        assert_ne!(first.pairing_id, second.pairing_id);
        assert_eq!(node.peers().len(), 0);
    }

    /// Security review F-13. At the cap the user gets a refusal naming the
    /// remedy, not an internal error and not a silently evicted device.
    #[test]
    fn minting_past_the_pairing_cap_is_refused_with_a_reason() {
        let (node, _dir) = node();
        for i in 0..crate::peers::MAX_PAIRINGS {
            let token = crate::transport::PairingToken::generate();
            node.peers()
                .upsert(Peer {
                    pairing_id: token.pairing_id(),
                    device_id: None,
                    name: format!("device-{i}"),
                    psk: token.psk(),
                    last_addr: None,
                    last_seen_ms: 1,
                    profile: None,
                    profile_observed_at_ms: 0,
                })
                .expect("up to the cap");
        }

        let err = node
            .pair_create_invite()
            .expect_err("past the cap must be refused");
        assert_eq!(err, NodeError::TooManyPairings);
        assert!(
            err.is_client_error(),
            "a refusal with a remedy must not read as an internal fault"
        );
        assert!(err.to_string().contains("unpair"), "{err}");

        // Nothing was taken to make room, and unpairing is what unblocks it.
        assert_eq!(node.peers().len(), crate::peers::MAX_PAIRINGS);
        let existing = node.peers().psks()[0].pairing_id.clone();
        assert!(node.unpair(&existing).unwrap());
        node.pair_create_invite()
            .expect("a freed slot must be usable");
    }

    #[test]
    fn a_missing_name_still_reads_as_something() {
        assert_eq!(placeholder_name("   "), "unnamed device");
        assert_eq!(placeholder_name(" laptop "), "laptop");
    }

    #[test]
    fn a_listen_address_is_never_loopback() {
        let (node, _dir) = node();
        if let Some(addr) = node.listen_addr() {
            let parsed: SocketAddr = addr.parse().expect("a socket address");
            assert!(!parsed.ip().is_loopback(), "{addr}");
        }
    }

    /// Nothing is visible without discovery, and that is a normal answer.
    #[test]
    fn a_node_without_discovery_sees_nobody() {
        let (node, _dir) = node();
        assert!(node.seen().is_empty());
        assert!(node.find("whoever").is_none());
        // Republishing must not panic or fail when there is nothing to publish.
        node.republish();
    }
}
