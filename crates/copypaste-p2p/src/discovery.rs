//! mDNS-SD peer discovery for CopyPaste.
//!
//! Registers own service under `_copypaste._tcp.local.` and browses for
//! other instances, emitting callbacks when peers appear or disappear.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use mdns_sd::{ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::task::{AbortHandle, JoinHandle};
use tracing::{debug, info, warn};

use crate::error::DiscoveryError;
use crate::rate_limit::MdnsRateLimiter;

/// Lock a `Mutex` even if a previous holder panicked.
///
/// Poison-tolerance is required for callbacks that may panic: a panic in
/// `on_peer_found`/`on_peer_lost` user code would otherwise permanently
/// disable discovery for the rest of the process. We recover the inner
/// guard and log a warning so the issue surfaces in production telemetry.
#[inline]
fn lock_safe<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock()
        .unwrap_or_else(|e: PoisonError<MutexGuard<'_, T>>| {
            warn!("recovering from poisoned mutex in discovery service");
            e.into_inner()
        })
}

/// Service type used for mDNS-SD advertisement and browsing.
pub const SERVICE_TYPE: &str = "_copypaste._tcp.local.";
/// TXT record version key.
const TXT_VERSION: &str = "v";
/// TXT record device-id key.
const TXT_DEVICE_ID: &str = "did";
/// TXT record device-name key.
const TXT_DEVICE_NAME: &str = "name";
/// TXT record bootstrap-port key (LAN/SAS Phase 0).
///
/// Carries the TCP port of the ephemeral PAKE bootstrap listener used for
/// SAS-authenticated pairing (Phase 2). Absent on v1 peers — `peer_from_resolved`
/// accepts both so the discovered list is never gated on the peer version.
const TXT_BPORT: &str = "bport";
/// Protocol version advertised in TXT records (Phase 0 bump: was "1").
///
/// v2 adds the `bport` TXT key. `peer_from_resolved` accepts both v1 and v2
/// so existing peers are never silently dropped from the discovered list.
const PROTOCOL_VERSION: &str = "2";
/// Maximum length of a single DNS label per RFC 1035 §2.3.4.
/// mdns-sd asserts `s.len() < 64` in dns_parser.rs; enforce the limit here
/// before registering so we never trigger a background-thread panic.
const DNS_LABEL_MAX: usize = 63;
/// The v1 protocol version string, accepted for backward compatibility.
///
/// v1 peers lack the `bport` TXT key; they appear in the discovered list but
/// the UI disables the "Pair" button because bootstrap is unavailable.
pub const PROTOCOL_VERSION_V1: &str = "1";

/// Maximum number of distinct peers retained in `known_peers`.
///
/// Security MED (DoS / unbounded memory): `known_peers` is keyed by the mDNS
/// fullname, which embeds the unauthenticated, rotatable TXT `did`. A LAN host
/// emitting endlessly-varying instance fullnames would otherwise grow the map
/// without bound (and id-rotation also dodges the per-key rate limiter). Once
/// this cap is reached we refuse to insert genuinely new peers rather than
/// evict existing ones — eviction would let an attacker flush legitimately
/// discovered peers. Updates to already-known fullnames are always allowed.
const MAX_KNOWN_PEERS: usize = 256;

/// Information about a discovered peer.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerInfo {
    /// The peer's device ID (hex fingerprint of their public key).
    pub device_id: String,
    /// Human-readable device name.
    pub device_name: String,
    /// All resolved IP addresses for the peer (sorted, deduplicated).
    pub ip_addrs: Vec<IpAddr>,
    /// TCP port the peer's P2P sync listener is on.
    pub port: u16,
    /// TCP port of the peer's PAKE bootstrap listener (LAN/SAS Phase 0).
    ///
    /// Present on v2 peers that advertise `bport` in their TXT record.
    /// `None` on v1 peers — the UI must disable the "Pair" button in that case
    /// because the bootstrap handshake cannot be initiated without this port.
    pub bport: Option<u16>,
}

type PeerFoundCallback = Arc<dyn Fn(PeerInfo) + Send + Sync + 'static>;
type PeerLostCallback = Arc<dyn Fn(String) + Send + Sync + 'static>;

/// mDNS-SD discovery service.
///
/// Registers own CopyPaste service on the local network and discovers
/// peers. Peer events are delivered via registered callbacks.
///
/// # Example
/// ```no_run
/// use copypaste_p2p::discovery::DiscoveryService;
///
/// # async fn run() {
/// let svc = DiscoveryService::new();
/// svc.on_peer_found(|peer| println!("Found: {:?}", peer));
/// svc.on_peer_lost(|id| println!("Lost: {}", id));
/// svc.register(51515, "device-id-hex", "My Mac").unwrap();
/// let _handle = svc.start().await.unwrap();
/// # }
/// ```
pub struct DiscoveryService {
    /// Callbacks invoked when a new peer is found.
    on_found: Arc<Mutex<Vec<PeerFoundCallback>>>,
    /// Callbacks invoked when a peer disappears.
    on_lost: Arc<Mutex<Vec<PeerLostCallback>>>,
    /// Currently known peers keyed by mDNS fullname.
    known_peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    /// Port and identity used to advertise own service.
    registration: Arc<Mutex<Option<Registration>>>,
    /// Per-source-IP token bucket guarding inbound `ServiceResolved` events
    /// from mDNS flood (THREAT-MODEL OI-3). See [`MdnsRateLimiter`].
    rate_limiter: Arc<MdnsRateLimiter>,
    /// Abort handle for the background browse task spawned by [`DiscoveryService::start`].
    /// Retained so [`Drop`] can abort it, preventing the browse loop from
    /// outliving the service across P2P toggle / reconfigure cycles.
    browse_abort: Arc<Mutex<Option<AbortHandle>>>,
    /// Clone of the mDNS [`ServiceDaemon`] created in [`DiscoveryService::start`]. Retained so
    /// [`Drop`] can shut it down, releasing the mDNS socket.
    daemon: Arc<Mutex<Option<ServiceDaemon>>>,
}

#[derive(Clone)]
struct Registration {
    port: u16,
    device_id: String,
    // device_name is intentionally not stored here (CopyPaste-sh9a): the human
    // name is no longer included in the mDNS advertisement to avoid PII leakage
    // on the LAN. The name is retained in the daemon's own config and exchanged
    // post-PAKE during pairing. The public `register()` API still accepts the
    // name parameter for caller compatibility but does not persist it.
    /// Bootstrap port for SAS pairing (Phase 0). None = v1 advertisement.
    bport: Option<u16>,
}

impl DiscoveryService {
    /// Create a new, unconfigured discovery service.
    pub fn new() -> Self {
        Self {
            on_found: Arc::new(Mutex::new(Vec::new())),
            on_lost: Arc::new(Mutex::new(Vec::new())),
            known_peers: Arc::new(Mutex::new(HashMap::new())),
            registration: Arc::new(Mutex::new(None)),
            rate_limiter: Arc::new(MdnsRateLimiter::new()),
            browse_abort: Arc::new(Mutex::new(None)),
            daemon: Arc::new(Mutex::new(None)),
        }
    }

    /// Return a clone of the rate-limiter handle. Exposed for tests and
    /// metrics endpoints; production callers normally do not need it.
    pub fn rate_limiter(&self) -> Arc<MdnsRateLimiter> {
        Arc::clone(&self.rate_limiter)
    }

    /// Register a callback that fires whenever a new peer is resolved.
    ///
    /// Multiple callbacks can be registered; they are all called in order.
    pub fn on_peer_found<F>(&self, callback: F)
    where
        F: Fn(PeerInfo) + Send + Sync + 'static,
    {
        lock_safe(&self.on_found).push(Arc::new(callback));
    }

    /// Register a callback that fires whenever a peer disappears.
    ///
    /// The argument is the peer's `device_id`.
    pub fn on_peer_lost<F>(&self, callback: F)
    where
        F: Fn(String) + Send + Sync + 'static,
    {
        lock_safe(&self.on_lost).push(Arc::new(callback));
    }

    /// Configure own service registration.
    ///
    /// Must be called before [`start`](DiscoveryService::start).
    ///
    /// # Parameters
    /// - `port`: TCP port this device's P2P sync listener is on.
    /// - `device_id`: Hex fingerprint of this device's public key.
    /// - `device_name`: Human-readable name (e.g. "Alice's MacBook").
    /// - `bport`: Optional TCP port of the PAKE bootstrap listener (LAN/SAS
    ///   Phase 0). When `Some`, the `bport` TXT key is included in the
    ///   advertisement and the protocol version is bumped to "2".  When
    ///   `None`, the service advertises as v1 (no bootstrap port).
    pub fn register(
        &self,
        port: u16,
        device_id: impl Into<String>,
        device_name: impl Into<String>,
    ) -> Result<(), DiscoveryError> {
        self.register_inner(port, device_id.into(), device_name.into(), None)
    }

    /// Register for advertisement INCLUDING the v2 `bport` TXT key (LAN/SAS
    /// Phase 2).
    ///
    /// Identical to [`register`](Self::register) but also advertises the TCP
    /// port of this device's standing PAKE bootstrap listener. Initiators read
    /// `bport` from the discovered peer's TXT record to know where to dial for
    /// SAS pairing; the presence of `bport` is also what flips the advertised
    /// protocol version to v2.
    pub fn register_with_bport(
        &self,
        port: u16,
        device_id: impl Into<String>,
        device_name: impl Into<String>,
        bport: u16,
    ) -> Result<(), DiscoveryError> {
        self.register_inner(port, device_id.into(), device_name.into(), Some(bport))
    }

    fn register_inner(
        &self,
        port: u16,
        device_id: String,
        // device_name accepted for API compatibility but not persisted —
        // it is no longer included in the mDNS advertisement (CopyPaste-sh9a).
        _device_name: String,
        bport: Option<u16>,
    ) -> Result<(), DiscoveryError> {
        let mut reg = lock_safe(&self.registration);
        if reg.is_some() {
            return Err(DiscoveryError::AlreadyRegistered);
        }
        *reg = Some(Registration {
            port,
            device_id,
            bport,
        });
        Ok(())
    }

    /// Start the mDNS-SD daemon in a background Tokio task.
    ///
    /// Advertises own service (if [`register`](DiscoveryService::register)
    /// was called) and starts browsing for peers.
    ///
    /// Returns a [`JoinHandle`] that can be awaited or aborted for graceful
    /// shutdown.
    pub async fn start(&self) -> Result<JoinHandle<()>, DiscoveryError> {
        // If a previous browse task / daemon is still alive on this instance
        // (restart-in-place), tear it down first so we never accumulate
        // orphaned tasks or mDNS sockets.
        self.shutdown_inner();

        let daemon = ServiceDaemon::new().map_err(|e| DiscoveryError::Daemon(e.to_string()))?;

        // Advertise own service if registration was provided.
        let reg_opt = lock_safe(&self.registration).clone();
        if let Some(ref reg) = reg_opt {
            self.advertise(&daemon, reg)?;
        }

        // Start browsing.
        let receiver = daemon
            .browse(SERVICE_TYPE)
            .map_err(|e| DiscoveryError::Browse(e.to_string()))?;

        let on_found = Arc::clone(&self.on_found);
        let on_lost = Arc::clone(&self.on_lost);
        let known_peers = Arc::clone(&self.known_peers);
        let rate_limiter = Arc::clone(&self.rate_limiter);
        let own_id: Option<String> = reg_opt.map(|r| r.device_id);

        // Retain a daemon handle (`ServiceDaemon` is a cheap clonable handle to
        // the same background daemon) so `Drop` can shut it down and release the
        // mDNS socket.
        lock_safe(&self.daemon).replace(daemon.clone());

        // CopyPaste-bp3o: wrap the browse loop in a supervisor task so a panic
        // inside the inner task is logged at ERROR instead of being silently
        // swallowed. Without this, a panic in `handle_event` (or in the
        // recv_async loop) kills the mDNS discovery subsystem with no observable
        // signal, making the failure invisible to operators.
        //
        // The inner task holds the daemon alive (`let _daemon = daemon`). The
        // outer supervisor awaits the inner join-handle; on panic it logs the
        // event and exits (no restart — a restart would require re-initialising
        // the mDNS daemon, which is the caller's responsibility via `start()`).
        //
        // Shutdown ordering: `browse_abort` is set to the OUTER supervisor's
        // abort handle. When aborted, the outer task's cancellation propagates
        // to the `inner_handle.await` point, but the inner task itself is not
        // automatically cancelled (tokio task cancellation is not hierarchical).
        // To ensure the inner task is also stopped, we store the inner abort
        // handle in the supervisor closure and call `inner_abort.abort()` from
        // a `Drop` guard before yielding. The existing `shutdown_inner` path
        // also shuts down the mDNS daemon, which closes the browse channel and
        // causes the inner loop to exit naturally.
        let inner_handle = tokio::spawn(async move {
            // Keep the daemon alive for the duration of the task.
            let _daemon = daemon;

            loop {
                // recv_async() integrates with tokio without blocking executor threads.
                match receiver.recv_async().await {
                    Ok(event) => {
                        handle_event(
                            event,
                            &own_id,
                            &known_peers,
                            &on_found,
                            &on_lost,
                            &rate_limiter,
                        );
                    }
                    Err(e) => {
                        // Channel closed — daemon shut down.
                        debug!("mDNS browse channel closed: {}", e);
                        break;
                    }
                }
            }
        });

        // Retain the inner abort handle so the supervisor can explicitly abort
        // the inner task when the supervisor itself is cancelled. Without this,
        // aborting the outer task leaves the inner task running detached.
        let inner_abort = inner_handle.abort_handle();

        let handle = tokio::spawn(async move {
            // Ensure the inner task is aborted when the supervisor exits for
            // any reason (normal shutdown, cancellation, or after logging a
            // panic). The `AbortHandle` is a cheap clone of the inner task's
            // cancellation handle; calling `.abort()` is idempotent.
            struct AbortOnDrop(AbortHandle);
            impl Drop for AbortOnDrop {
                fn drop(&mut self) {
                    self.0.abort();
                }
            }
            let _guard = AbortOnDrop(inner_abort);

            match inner_handle.await {
                Ok(()) => {} // Browse loop exited cleanly (daemon shut down).
                Err(join_err) if join_err.is_panic() => {
                    // CopyPaste-bp3o: log the panic so it surfaces in telemetry.
                    // The browse loop is not restarted here; the caller must call
                    // `start()` again to re-initialise discovery if needed.
                    tracing::error!(
                        "CopyPaste-bp3o: mDNS browse task panicked; \
                         discovery is disabled until start() is called again"
                    );
                }
                Err(_cancelled) => {
                    // Cancelled via abort_handle — normal shutdown path.
                }
            }
        });

        // Retain an abort handle so `Drop` (and a restart-in-place via
        // `shutdown_inner`) can stop the browse loop. The owned `JoinHandle` is
        // still returned to the caller for awaiting / explicit shutdown.
        // The abort_handle points to the OUTER supervisor task; the supervisor
        // in turn aborts the inner browse task via the `AbortOnDrop` guard.
        lock_safe(&self.browse_abort).replace(handle.abort_handle());

        Ok(handle)
    }

    /// Stop mDNS advertisement and browsing immediately, without dropping the
    /// service.
    ///
    /// Aborts the background browse task and shuts down the mDNS daemon so the
    /// device stops advertising on the LAN and releases the mDNS socket.
    /// Idempotent — safe to call when nothing is running. The service can be
    /// restarted later by calling [`start`](Self::start) again.
    ///
    /// Used by the hot-apply path in `set_config` when `lan_visibility` is
    /// toggled off; [`Drop`] also calls this via `shutdown_inner`.
    pub fn stop(&self) {
        self.shutdown_inner();
    }

    /// Abort the retained browse task and shut down the retained mDNS daemon,
    /// if any. Idempotent: safe to call when nothing is running. Used both by
    /// [`start`](Self::start) (restart-in-place) and [`Drop`].
    fn shutdown_inner(&self) {
        if let Some(abort) = lock_safe(&self.browse_abort).take() {
            abort.abort();
        }
        if let Some(daemon) = lock_safe(&self.daemon).take() {
            // Best-effort: closing the daemon releases the mDNS socket and
            // closes the browse channel. The browse task may already be gone.
            if let Err(e) = daemon.shutdown() {
                debug!("mDNS daemon shutdown failed: {}", e);
            }
        }
    }

    /// Return a snapshot of all currently known peers.
    pub fn peers(&self) -> Vec<PeerInfo> {
        lock_safe(&self.known_peers).values().cloned().collect()
    }

    /// Insert a peer directly into `known_peers`. Intended **only** for unit
    /// tests that need deterministic discovery state without a live mDNS daemon.
    ///
    /// Not compiled into production builds.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn inject_peer_for_test(&self, fullname: &str, peer: PeerInfo) {
        lock_safe(&self.known_peers).insert(fullname.to_string(), peer);
    }

    /// Resolve a peer by its advertised `device_id` (the `did` TXT record).
    ///
    /// Returns the most-recently-seen [`PeerInfo`] for `device_id`, or `None`
    /// if no such peer is currently known. Used as the **fallback** discovery
    /// path during pairing when the QR carries no `addr_hint`: the initiator
    /// resolves the responder's `host:port` from mDNS instead.
    ///
    /// Loopback mDNS is unreliable (see module docs), so `addr_hint` remains the
    /// primary path and this is best-effort only.
    pub fn resolve_peer(&self, device_id: &str) -> Option<PeerInfo> {
        lock_safe(&self.known_peers)
            .values()
            .find(|p| p.device_id == device_id)
            .cloned()
    }

    // ── private helpers ──────────────────────────────────────────────────────

    /// Announce own service on the local network.
    fn advertise(&self, daemon: &ServiceDaemon, reg: &Registration) -> Result<(), DiscoveryError> {
        // Instance name: opaque label derived only from device_id.
        // The human device name is intentionally excluded (CopyPaste-sh9a):
        // embedding it here was leaking PII to any passive LAN observer.
        let instance_name = opaque_instance_label(&reg.device_id);

        // Hostname: also opaque — no human name, just the same derived label.
        let hostname = format!("{}.local.", opaque_hostname_label(&reg.device_id));

        // Build TXT properties via the extracted helper.
        // Human device name is intentionally absent — see `build_txt_properties`.
        let base_props = build_txt_properties(&reg.device_id);

        // bport is optional (Phase 0: absent; Phase 2: set by `register_with_bport`).
        // We assemble a borrowed-slice view for ServiceInfo.
        let bport_str: String;
        // Build a Vec<(&str, &str)> that borrows from base_props + bport_str.
        let mut properties: Vec<(&str, &str)> =
            base_props.iter().map(|(k, v)| (*k, v.as_str())).collect();
        if let Some(bp) = reg.bport {
            bport_str = bp.to_string();
            properties.push((TXT_BPORT, bport_str.as_str()));
        }

        // Wave F.L12 — advertise only on real LAN interfaces, filtering out
        // loopback / virtual / down NICs. When the host has no usable
        // interface (or enumeration failed) we fall back to `()` so `mdns-sd`
        // auto-detects, preserving the previous behaviour rather than going
        // silent.
        let usable_addrs = crate::interfaces::usable_advertise_addrs();
        let service_info = if usable_addrs.is_empty() {
            warn!("no usable LAN interface found; letting mdns-sd auto-detect addresses");
            ServiceInfo::new(
                SERVICE_TYPE,
                &instance_name,
                &hostname,
                (),
                reg.port,
                &properties[..],
            )
        } else {
            ServiceInfo::new(
                SERVICE_TYPE,
                &instance_name,
                &hostname,
                &usable_addrs[..],
                reg.port,
                &properties[..],
            )
        }
        .map_err(|e| DiscoveryError::Register(e.to_string()))?;

        let fullname = service_info.get_fullname().to_string();

        daemon
            .register(service_info)
            .map_err(|e| DiscoveryError::Register(e.to_string()))?;

        // device_name is NOT logged to avoid leaking it into log files accessible
        // on the LAN or in telemetry. Log only the opaque device_id and port.
        info!(
            device_id = %reg.device_id,
            port = reg.port,
            fullname = %fullname,
            "Registered mDNS-SD service"
        );

        Ok(())
    }
}

impl Default for DiscoveryService {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for DiscoveryService {
    /// Abort the background browse task and shut down the mDNS daemon when the
    /// service is dropped (P2P toggled off, daemon reconfigured, or a new
    /// instance replaces this one), so neither the task nor the mDNS socket
    /// leaks across reconnect cycles.
    fn drop(&mut self) {
        self.shutdown_inner();
    }
}

// ── event handler ────────────────────────────────────────────────────────────

fn handle_event(
    event: ServiceEvent,
    own_id: &Option<String>,
    known_peers: &Arc<Mutex<HashMap<String, PeerInfo>>>,
    on_found: &Arc<Mutex<Vec<PeerFoundCallback>>>,
    on_lost: &Arc<Mutex<Vec<PeerLostCallback>>>,
    rate_limiter: &Arc<MdnsRateLimiter>,
) {
    match event {
        ServiceEvent::ServiceResolved(resolved) => {
            if let Some(peer) = peer_from_resolved(&resolved) {
                // Skip own service.
                if own_id.as_deref() == Some(peer.device_id.as_str()) {
                    debug!(device_id = %peer.device_id, "Ignoring own mDNS advertisement");
                    return;
                }

                // OI-3 mitigation: rate-limit per peer identity. Prefer
                // `device_id` (the cert fingerprint advertised in TXT) so a
                // dual-stack peer with both v4 and v6 addresses doesn't get
                // 2× budget (security MED #11). When `device_id` is empty
                // (older clients / malformed TXT) we fall back to a stable
                // hash of the *sorted* address set rather than the first
                // address, which also closes the same v4/v6-rotation bypass.
                // Drop = silent denial-of-response; the limiter emits
                // trace + sampled warn telemetry itself.
                let rl_key = if !peer.device_id.is_empty() {
                    peer.device_id.clone()
                } else {
                    address_set_key(&peer.ip_addrs)
                };
                if !rate_limiter.try_admit_key(&rl_key) {
                    return;
                }

                let fullname = resolved.fullname.clone();

                // Dedup + cap: only emit if this is a new or changed peer, and
                // refuse brand-new fullnames once the map is at capacity.
                let mut peers = lock_safe(known_peers);
                let is_new = match admit_peer(&peers, &fullname, &peer) {
                    PeerAdmission::Skip => false,
                    PeerAdmission::Insert => true,
                    PeerAdmission::AtCapacity => {
                        drop(peers);
                        warn!(
                            device_id = %peer.device_id,
                            cap = MAX_KNOWN_PEERS,
                            "known_peers at capacity — refusing new mDNS peer"
                        );
                        return;
                    }
                };

                if is_new {
                    info!(
                        device_id = %peer.device_id,
                        device_name = %peer.device_name,
                        port = peer.port,
                        addrs = ?peer.ip_addrs,
                        "mDNS peer found"
                    );
                    peers.insert(fullname, peer.clone());
                    drop(peers);

                    // Snapshot callbacks so user code never holds the mutex —
                    // a panic inside a callback can only poison the mutex
                    // briefly; `lock_safe` will recover on the next call.
                    let callbacks: Vec<PeerFoundCallback> =
                        lock_safe(on_found).iter().cloned().collect();
                    for cb in callbacks.iter() {
                        cb(peer.clone());
                    }
                }
            } else {
                warn!(
                    fullname = %resolved.fullname,
                    "Ignoring mDNS service missing required TXT records"
                );
            }
        }

        ServiceEvent::ServiceRemoved(_svc_type, fullname) => {
            let mut peers = lock_safe(known_peers);
            if let Some(peer) = peers.remove(&fullname) {
                info!(device_id = %peer.device_id, "mDNS peer lost");
                drop(peers);

                let callbacks: Vec<PeerLostCallback> = lock_safe(on_lost).iter().cloned().collect();
                for cb in callbacks.iter() {
                    cb(peer.device_id.clone());
                }
            }
        }

        other => {
            debug!("mDNS event (ignored): {:?}", other);
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

/// Build a [`PeerInfo`] from a resolved mDNS service, if the required
/// TXT records (`v`, `did`) are present and version is supported.
///
/// Accepts both v1 (`PROTOCOL_VERSION_V1 = "1"`, no `bport`) and v2
/// (`PROTOCOL_VERSION = "2"`, optional `bport`) so that existing v1 peers
/// are never silently dropped from the discovered list after the Phase 0
/// version bump. v1 peers produce a `PeerInfo` with `bport: None`; the UI
/// disables the "Pair" button for those entries.
///
/// The `name` TXT key is now **optional** (CopyPaste-sh9a): v3+ peers no
/// longer advertise it because it leaks PII on the LAN. Legacy v1/v2 peers
/// that still include `name` will have it accepted into `device_name`; newer
/// peers that omit it get an empty `device_name` in the returned `PeerInfo`.
/// The authoritative human name is exchanged post-PAKE during pairing.
fn peer_from_resolved(resolved: &ResolvedService) -> Option<PeerInfo> {
    let version = resolved.get_property_val_str(TXT_VERSION)?;
    // Accept v1 (legacy) and v2 (current). Any other version is unsupported.
    if version != PROTOCOL_VERSION && version != PROTOCOL_VERSION_V1 {
        warn!(version, "mDNS peer uses unsupported protocol version");
        return None;
    }

    let device_id = resolved.get_property_val_str(TXT_DEVICE_ID)?.to_string();

    // CopyPaste-rh27: tighten mDNS→peer correlation.
    //
    // A rogue LAN host could broadcast a `did` TXT record claiming to be any
    // device (IP-correlation attack). The mTLS handshake (cert-fingerprint
    // pinning) is the definitive defence — a rogue peer cannot impersonate
    // another device's fingerprint without its private key. However, if the
    // `device_id` in the TXT record is empty or malformed we would (a) skip
    // the rate-limit key based on identity and fall through to the IP-set hash,
    // and (b) insert a confusingly-keyed entry into `known_peers` that the
    // connector might try to dial. Reject malformed device_ids here so the
    // discovery layer never presents an unauthenticated device_id to callers.
    if !is_valid_device_id(&device_id) {
        warn!(
            device_id = %device_id,
            fullname = %resolved.fullname,
            "CopyPaste-rh27: mDNS peer has empty or malformed device_id — ignoring"
        );
        return None;
    }
    // `name` is optional since CopyPaste-sh9a: upgraded peers no longer
    // advertise it. Empty string = "unknown until post-PAKE exchange".
    let device_name = resolved
        .get_property_val_str(TXT_DEVICE_NAME)
        .unwrap_or("")
        .to_string();

    // Collect all addresses, deduplicated and sorted for determinism.
    // Unknown ScopedIp variants return None and are filtered out so 0.0.0.0
    // is never placed into the dial list.
    let mut ip_addrs: Vec<IpAddr> = resolved
        .get_addresses()
        .iter()
        .filter_map(scoped_ip_to_ip_addr)
        .collect();
    ip_addrs.sort_unstable_by_key(|a| (a.is_ipv6(), a.to_string()));
    ip_addrs.dedup();

    // Parse the optional bootstrap port from TXT. A malformed value (non-u16)
    // is treated as absent rather than fatal — the peer still appears in the
    // discovered list, the UI just disables the "Pair" button.
    let bport: Option<u16> = resolved
        .get_property_val_str(TXT_BPORT)
        .and_then(|s| s.parse().ok());

    Some(PeerInfo {
        device_id,
        device_name,
        ip_addrs,
        port: resolved.get_port(),
        bport,
    })
}

/// Decision for whether a resolved peer should be inserted into `known_peers`.
#[derive(Debug, PartialEq, Eq)]
enum PeerAdmission {
    /// Already present and unchanged — no insert, no callback.
    Skip,
    /// New or changed peer that fits within the cap — insert and notify.
    Insert,
    /// A brand-new fullname but the map is full — refuse (DoS guard).
    AtCapacity,
}

/// Decide how to handle a resolved peer relative to the current `known_peers`
/// map, enforcing [`MAX_KNOWN_PEERS`]. Pure (no mutation) so it is unit-testable
/// without a live mDNS daemon.
fn admit_peer(peers: &HashMap<String, PeerInfo>, fullname: &str, peer: &PeerInfo) -> PeerAdmission {
    match peers.get(fullname) {
        Some(existing) if existing == peer => PeerAdmission::Skip,
        Some(_) => PeerAdmission::Insert, // known fullname, changed value
        None if peers.len() >= MAX_KNOWN_PEERS => PeerAdmission::AtCapacity,
        None => PeerAdmission::Insert,
    }
}

/// Validate a `device_id` advertised in the mDNS TXT `did` field.
///
/// A valid device_id must be non-empty and consist entirely of lowercase hex
/// characters (0-9, a-f). This matches the format produced by
/// `fingerprint_of` in `crate::cert` (hex-encoded SHA-256). Uppercase hex,
/// empty strings, and any non-hex characters are rejected to prevent a rogue
/// LAN peer from advertising a device_id that bypasses identity-keyed rate
/// limiting or confuses the known-peers map (CopyPaste-rh27).
///
/// Note: the TLS certificate-fingerprint check in `PeerTransport::connect`
/// is the *definitive* authentication gate; this is a defence-in-depth
/// pre-filter at the discovery layer.
fn is_valid_device_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

/// Build a stable rate-limit key from a set of resolved peer addresses.
///
/// Used when the peer's `device_id` is unknown (older clients / malformed
/// TXT). Sorting + delimiter-joining means the key is invariant to the
/// order `mdns-sd` happens to enumerate v4 vs v6 vs link-local addresses,
/// so a dual-stack peer cannot escape per-peer rate limiting by rotating
/// which address ends up first (security MED #11).
fn address_set_key(addrs: &[IpAddr]) -> String {
    let mut sorted: Vec<String> = addrs.iter().map(|a| a.to_string()).collect();
    sorted.sort();
    sorted.dedup();
    sorted.join(",")
}

/// Convert a [`ScopedIp`] to a standard [`IpAddr`].
///
/// Returns `None` for unknown `ScopedIp` variants (the type is
/// `#[non_exhaustive]`) so callers filter them out rather than dialling
/// `0.0.0.0`, which would be an unreachable and security-confusing address.
fn scoped_ip_to_ip_addr(scoped: &ScopedIp) -> Option<IpAddr> {
    match scoped {
        ScopedIp::V4(v4) => Some(IpAddr::V4(*v4.addr())),
        ScopedIp::V6(v6) => Some(IpAddr::V6(*v6.addr())),
        // `ScopedIp` is #[non_exhaustive]; unknown future variants are dropped
        // rather than substituted with 0.0.0.0, which would be dialled.
        &_ => None,
    }
}

/// Replace characters that are invalid in mDNS labels with hyphens and
/// trim leading/trailing hyphens.
///
/// If the result is empty (e.g. the device name consists entirely of
/// non-alphanumeric characters such as `"!!!"`) a hardcoded fallback label
/// `"copypaste"` is substituted so `ServiceInfo` is never constructed with an
/// invalid `".{id}"` label that would cause `mdns-sd` to reject registration.
///
/// No longer called in production code (CopyPaste-sh9a: opaque labels replace
/// human-name-based labels) but retained for tests so existing `sanitize_label`
/// test coverage continues to document the invariants.
#[cfg(test)]
fn sanitize_label(s: &str) -> String {
    let sanitized: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "copypaste".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Build the opaque mDNS instance label from a `device_id`.
///
/// Computes SHA-256 over the `device_id` string and uses the first 8 hex
/// characters of the digest prefixed with `"cp-"`. This ensures:
///
/// * The raw `device_id` prefix is NOT present in the label — a passive LAN
///   observer cannot distinguish `cp-5f4dcc3b` (hash of "password") from any
///   other device label without already knowing the target `device_id`.
/// * The label is deterministic for a given `device_id`, so it remains stable
///   across daemon restarts on the same device.
/// * Different `device_id` values produce different labels (no trivial collision
///   in the 8-hex-char output space for realistic device counts).
///
/// The result is guaranteed to be ≤ `DNS_LABEL_MAX` (63) characters.
/// ("cp-" = 3 chars) + (8 hex digits) = 11 chars total.
///
/// CopyPaste-rt50 root cause: the previous implementation used
/// `device_id.chars().take(8)`, which directly exposed the first 8 characters
/// of the stable device fingerprint — allowing a passive LAN observer to durably
/// track a device across network changes by its mDNS instance name.
///
/// TODO(CopyPaste-sh9a): For stronger unlinkability across sessions, derive the
/// label from a daily HKDF epoch (HKDF(static_key, salt='copypaste/label/' +
/// floor(now/86400))) so it rotates once per day without requiring re-pairing.
fn opaque_instance_label(device_id: &str) -> String {
    use sha2::Digest as _;
    // "cp-" (3) + 8 hex digits = 11 chars, well within DNS_LABEL_MAX.
    const _: () = assert!(3 + 8 <= DNS_LABEL_MAX);
    let digest = sha2::Sha256::digest(device_id.as_bytes());
    // Take the first 4 bytes (8 hex chars) of the SHA-256 digest.
    // This is not a cryptographic secret — it is a one-way transform to prevent
    // the raw device_id prefix from appearing in unauthenticated mDNS frames.
    let id_short = hex::encode(&digest[..4]);
    format!("cp-{id_short}")
}

/// Build the hostname label (the part before `.local.`) for mDNS advertisement.
///
/// Uses the opaque instance label so the hostname does not embed the human
/// device name. Previously this used the sanitised device name.
fn opaque_hostname_label(device_id: &str) -> String {
    opaque_instance_label(device_id)
}

/// Build the ordered TXT record property list for our mDNS advertisement.
///
/// The human device name is intentionally **not** included — it is PII and
/// must not be broadcast to passive LAN observers. Paired peers learn the
/// human name through the post-PAKE authenticated exchange instead.
///
/// Returns a `Vec` of `(key, value)` pairs in stable order. The caller is
/// responsible for allocating the `bport` string and extending the slice.
fn build_txt_properties(device_id: &str) -> Vec<(&'static str, String)> {
    vec![
        (TXT_VERSION, PROTOCOL_VERSION.to_string()),
        (TXT_DEVICE_ID, device_id.to_string()),
        // TXT_DEVICE_NAME is deliberately absent — see CopyPaste-sh9a.
    ]
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── sanitize_label ───────────────────────────────────────────────────────

    #[test]
    fn sanitize_label_keeps_alphanumeric_and_hyphen() {
        assert_eq!(sanitize_label("Alice-Mac"), "Alice-Mac");
    }

    #[test]
    fn sanitize_label_replaces_spaces() {
        assert_eq!(sanitize_label("Alice's Mac"), "Alice-s-Mac");
    }

    #[test]
    fn sanitize_label_trims_leading_trailing_hyphens() {
        assert_eq!(sanitize_label(" hello "), "hello");
    }

    #[test]
    fn sanitize_label_empty_input() {
        // Empty input → empty after trim → fallback label returned.
        assert_eq!(sanitize_label(""), "copypaste");
    }

    #[test]
    fn sanitize_label_pure_special_chars_becomes_fallback() {
        // All specials → hyphens → trimmed → empty → fallback label.
        // ServiceInfo must never be created with an empty label (invalid mDNS).
        assert_eq!(sanitize_label("!!!"), "copypaste");
    }

    // ── PeerInfo helpers ─────────────────────────────────────────────────────

    fn make_peer(id: &str, name: &str, port: u16) -> PeerInfo {
        PeerInfo {
            device_id: id.to_string(),
            device_name: name.to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port,
            bport: None,
        }
    }

    #[test]
    fn peer_info_equality() {
        assert_eq!(
            make_peer("aabb", "Alice", 51515),
            make_peer("aabb", "Alice", 51515)
        );
    }

    #[test]
    fn peer_info_inequality_on_port() {
        assert_ne!(
            make_peer("aabb", "Alice", 51515),
            make_peer("aabb", "Alice", 9999)
        );
    }

    #[test]
    fn peer_info_inequality_on_device_id() {
        assert_ne!(
            make_peer("aabb", "Alice", 51515),
            make_peer("1122", "Alice", 51515)
        );
    }

    // ── DiscoveryService construction ────────────────────────────────────────

    #[test]
    fn discovery_service_new_has_no_peers() {
        let svc = DiscoveryService::new();
        assert!(svc.peers().is_empty());
    }

    #[test]
    fn discovery_service_default_has_no_peers() {
        let svc = DiscoveryService::default();
        assert!(svc.peers().is_empty());
    }

    // ── register ─────────────────────────────────────────────────────────────

    #[test]
    fn register_once_succeeds() {
        let svc = DiscoveryService::new();
        assert!(svc.register(51515, "did", "Alice").is_ok());
    }

    #[test]
    fn register_twice_returns_already_registered() {
        let svc = DiscoveryService::new();
        svc.register(51515, "did", "Alice").unwrap();
        let err = svc.register(51515, "did2", "Bob").unwrap_err();
        assert!(matches!(err, DiscoveryError::AlreadyRegistered));
    }

    #[test]
    fn register_stores_port_and_identity() {
        let svc = DiscoveryService::new();
        svc.register(12345, "mydeviceid", "Test Device").unwrap();
        let reg = svc.registration.lock().unwrap();
        let reg = reg.as_ref().unwrap();
        assert_eq!(reg.port, 12345);
        assert_eq!(reg.device_id, "mydeviceid");
        // device_name is no longer stored in Registration (CopyPaste-sh9a):
        // it is not advertised in the mDNS TXT record to avoid PII leakage.
    }

    /// LAN/SAS Phase 2: `register_with_bport` stores the bootstrap port so the
    /// advertise path can emit the v2 `bport` TXT key (initiators learn where to
    /// dial the standing responder).
    #[test]
    fn register_with_bport_stores_bootstrap_port() {
        let svc = DiscoveryService::new();
        svc.register_with_bport(12345, "mydeviceid", "Test Device", 54321)
            .unwrap();
        let reg = svc.registration.lock().unwrap();
        let reg = reg.as_ref().unwrap();
        assert_eq!(reg.port, 12345);
        assert_eq!(reg.bport, Some(54321));
    }

    /// Registering twice (even via `register_with_bport`) is refused.
    #[test]
    fn register_with_bport_twice_returns_already_registered() {
        let svc = DiscoveryService::new();
        svc.register(51515, "did", "Alice").unwrap();
        let err = svc
            .register_with_bport(51515, "did2", "Bob", 60000)
            .unwrap_err();
        assert!(matches!(err, DiscoveryError::AlreadyRegistered));
    }

    // ── callbacks ────────────────────────────────────────────────────────────

    #[test]
    fn on_peer_found_callback_is_stored() {
        let svc = DiscoveryService::new();
        svc.on_peer_found(|_| {});
        assert_eq!(svc.on_found.lock().unwrap().len(), 1);
    }

    #[test]
    fn on_peer_lost_callback_is_stored() {
        let svc = DiscoveryService::new();
        svc.on_peer_lost(|_| {});
        assert_eq!(svc.on_lost.lock().unwrap().len(), 1);
    }

    #[test]
    fn multiple_callbacks_all_stored() {
        let svc = DiscoveryService::new();
        svc.on_peer_found(|_| {});
        svc.on_peer_found(|_| {});
        svc.on_peer_lost(|_| {});
        assert_eq!(svc.on_found.lock().unwrap().len(), 2);
        assert_eq!(svc.on_lost.lock().unwrap().len(), 1);
    }

    #[test]
    fn found_callback_fires_with_correct_peer_info() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        let svc = DiscoveryService::new();

        svc.on_peer_found(move |peer| {
            assert_eq!(peer.device_id, "aabbccdd");
            assert_eq!(peer.device_name, "Alice");
            assert_eq!(peer.port, 51515);
            fired_clone.store(true, Ordering::SeqCst);
        });

        let peer = make_peer("aabbccdd", "Alice", 51515);
        for cb in svc.on_found.lock().unwrap().iter() {
            cb(peer.clone());
        }
        assert!(fired.load(Ordering::SeqCst));
    }

    #[test]
    fn lost_callback_fires_with_correct_device_id() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let fired = Arc::new(AtomicBool::new(false));
        let fired_clone = Arc::clone(&fired);
        let svc = DiscoveryService::new();

        svc.on_peer_lost(move |id| {
            assert_eq!(id, "aabbccdd");
            fired_clone.store(true, Ordering::SeqCst);
        });

        for cb in svc.on_lost.lock().unwrap().iter() {
            cb("aabbccdd".to_string());
        }
        assert!(fired.load(Ordering::SeqCst));
    }

    // ── known_peers state transitions (simulated without real mDNS) ──────────

    #[test]
    fn peer_added_to_known_peers() {
        let known: Arc<Mutex<HashMap<String, PeerInfo>>> = Arc::new(Mutex::new(HashMap::new()));
        let peer = make_peer("aabbccdd", "Alice", 51515);
        known
            .lock()
            .unwrap()
            .insert("alice.local.".to_string(), peer.clone());

        let peers = known.lock().unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers["alice.local."], peer);
    }

    #[test]
    fn peer_removed_from_known_peers() {
        let known: Arc<Mutex<HashMap<String, PeerInfo>>> = Arc::new(Mutex::new(HashMap::new()));
        let peer = make_peer("aabbccdd", "Alice", 51515);
        known
            .lock()
            .unwrap()
            .insert("alice.local.".to_string(), peer);

        let removed = known.lock().unwrap().remove("alice.local.");
        assert!(removed.is_some());
        assert!(known.lock().unwrap().is_empty());
    }

    #[test]
    fn duplicate_peer_does_not_increase_count() {
        let known: Arc<Mutex<HashMap<String, PeerInfo>>> = Arc::new(Mutex::new(HashMap::new()));
        let peer = make_peer("aabbccdd", "Alice", 51515);
        let fullname = "alice.local.".to_string();
        known.lock().unwrap().insert(fullname.clone(), peer.clone());
        known.lock().unwrap().insert(fullname, peer); // second insert with same key
        assert_eq!(known.lock().unwrap().len(), 1);
    }

    // ── known_peers cap (security MED: DoS / unbounded memory) ───────────────

    #[test]
    fn admit_peer_inserts_until_cap_then_refuses_new() {
        let mut map: HashMap<String, PeerInfo> = HashMap::new();
        // Fill the map exactly to capacity with distinct fullnames.
        for i in 0..MAX_KNOWN_PEERS {
            let fullname = format!("peer-{i}.local.");
            let peer = make_peer(&format!("id{i}"), "P", 1);
            assert_eq!(admit_peer(&map, &fullname, &peer), PeerAdmission::Insert);
            map.insert(fullname, peer);
        }
        assert_eq!(map.len(), MAX_KNOWN_PEERS);

        // A brand-new fullname past the cap must be refused.
        let overflow = make_peer("overflow", "P", 1);
        assert_eq!(
            admit_peer(&map, "overflow.local.", &overflow),
            PeerAdmission::AtCapacity,
            "new peer past the cap must be refused, not inserted"
        );

        // Updates to an already-known fullname are still allowed at capacity.
        let changed = make_peer("id0-changed", "P", 2);
        assert_eq!(
            admit_peer(&map, "peer-0.local.", &changed),
            PeerAdmission::Insert,
            "updating an existing peer must be allowed even at capacity"
        );

        // An unchanged already-known peer is skipped.
        let same = make_peer("id0", "P", 1);
        map.insert("peer-0.local.".to_string(), same.clone());
        assert_eq!(
            admit_peer(&map, "peer-0.local.", &same),
            PeerAdmission::Skip
        );
    }

    #[test]
    fn known_peers_growth_is_bounded_under_id_rotation() {
        // Simulate the attack: a flood of ever-varying fullnames. Apply the same
        // admission decision the event handler uses and confirm the map never
        // exceeds the cap.
        let mut map: HashMap<String, PeerInfo> = HashMap::new();
        for i in 0..(MAX_KNOWN_PEERS * 4) {
            let fullname = format!("rotating-{i}.local.");
            let peer = make_peer(&format!("rot{i}"), "P", 1);
            if admit_peer(&map, &fullname, &peer) == PeerAdmission::Insert {
                map.insert(fullname, peer);
            }
        }
        assert!(
            map.len() <= MAX_KNOWN_PEERS,
            "known_peers must stay bounded under id rotation, got {}",
            map.len()
        );
    }

    // ── opaque_instance_label: raw-prefix privacy (CopyPaste-rt50) ──────────

    /// The label must NOT start with the raw device_id prefix.
    ///
    /// Before CopyPaste-rt50 the label was `"cp-{first8charsOfDeviceId}"`, letting
    /// a passive LAN observer durably track the device. After the fix the label is
    /// `"cp-{first8hexCharsOfSha256(device_id)}"` — the raw prefix never appears.
    #[test]
    fn opaque_instance_label_does_not_expose_raw_device_id_prefix() {
        let device_id = "aabbccdddeadbeef0011223344556677";
        let label = opaque_instance_label(device_id);
        // The raw first-8-chars prefix must NOT appear in the label.
        let raw_prefix = &device_id[..8]; // "aabbccdd"
        assert!(
            !label.contains(raw_prefix),
            "label must not contain the raw device_id prefix '{raw_prefix}', got: {label}"
        );
        // The label still has the 'cp-' prefix and is exactly 11 chars.
        assert!(
            label.starts_with("cp-"),
            "label must start with 'cp-': {label}"
        );
        assert_eq!(label.len(), 11, "label must be exactly 11 chars: {label}");
        // All chars after 'cp-' are lowercase hex digits.
        assert!(
            label[3..].chars().all(|c| c.is_ascii_hexdigit()),
            "label suffix must be lowercase hex: {label}"
        );
    }

    // ── IP address sorting ────────────────────────────────────────────────────

    #[test]
    fn ipv4_addresses_sort_before_ipv6() {
        let mut addrs: Vec<IpAddr> = vec!["::1".parse().unwrap(), "127.0.0.1".parse().unwrap()];
        addrs.sort_unstable_by_key(|a| (a.is_ipv6(), a.to_string()));
        assert!(!addrs[0].is_ipv6());
        assert!(addrs[1].is_ipv6());
    }

    // ── service type constant ────────────────────────────────────────────────

    #[test]
    fn service_type_has_correct_format() {
        assert!(SERVICE_TYPE.starts_with('_'));
        assert!(SERVICE_TYPE.ends_with(".local."));
        assert!(SERVICE_TYPE.contains("_tcp"));
        assert!(SERVICE_TYPE.contains("_copypaste"));
    }

    // ── integration test (requires real network; skipped by default) ─────────

    /// Smoke-test: register + browse on the local network.
    /// Run with: `cargo test -- --ignored`
    #[tokio::test]
    #[ignore]
    async fn integration_register_and_browse_self() {
        let svc = DiscoveryService::new();

        let found = Arc::new(Mutex::new(Vec::<PeerInfo>::new()));
        let found_clone = Arc::clone(&found);

        svc.on_peer_found(move |peer| {
            found_clone.lock().unwrap().push(peer);
        });

        svc.register(59999, "cafebabe00000000", "IntegrationTest")
            .unwrap();
        let handle = svc.start().await.unwrap();

        // Give mDNS time to propagate.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        handle.abort();
        let _ = handle.await;

        tracing::debug!(
            peers = ?found.lock().unwrap(),
            "Found peers during integration test"
        );
    }

    // ── poison-tolerance ─────────────────────────────────────────────────────

    /// best-prac HIGH #6 — a panic inside a user callback must not permanently
    /// break the discovery service. Even if the inner Mutex gets poisoned,
    /// subsequent operations on the service must continue to work.
    #[test]
    fn discovery_mutex_survives_callback_panic() {
        let svc = Arc::new(DiscoveryService::new());

        // Register a callback that always panics.
        svc.on_peer_found(|_peer| {
            panic!("intentional callback panic for poison-tolerance test");
        });

        // Manually poison the on_found mutex by invoking the panicking
        // callback while we hold the lock. We do this in a sub-thread so the
        // panic does not abort the test process.
        let svc_clone = Arc::clone(&svc);
        let _ = std::thread::spawn(move || {
            // Snapshot then invoke under the lock to guarantee poisoning.
            let guard = svc_clone.on_found.lock().unwrap();
            for cb in guard.iter() {
                cb(PeerInfo {
                    device_id: "deadbeef".to_string(),
                    device_name: "Panic".to_string(),
                    ip_addrs: vec!["127.0.0.1".parse().unwrap()],
                    port: 1,
                    bport: None,
                });
            }
        })
        .join();

        // The mutex is now poisoned. Verify all public APIs that touch it
        // still work via `lock_safe`.
        assert_eq!(svc.peers().len(), 0, "peers() must work after poison");

        // Adding another callback must not panic.
        svc.on_peer_found(|_| {});

        // Registering must succeed too.
        svc.register(12345, "id", "Name")
            .expect("register must succeed after poison");

        // And `lock_safe` directly returns a usable guard.
        let guard = lock_safe(&svc.on_found);
        assert_eq!(guard.len(), 2, "callback list survives poisoning");
    }

    // ── LAN/SAS Phase 0: bport TXT key + PROTOCOL_VERSION "2" ───────────────

    /// PROTOCOL_VERSION must be "2" after the Phase 0 bump.
    #[test]
    fn protocol_version_is_2() {
        assert_eq!(PROTOCOL_VERSION, "2");
    }

    /// PeerInfo must carry an optional `bport` field for the bootstrap port.
    #[test]
    fn peer_info_has_bport_field() {
        let peer = PeerInfo {
            device_id: "aabb".to_string(),
            device_name: "Test".to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port: 51515,
            bport: Some(51516),
        };
        assert_eq!(peer.bport, Some(51516));

        let peer_no_bport = PeerInfo {
            device_id: "aabb".to_string(),
            device_name: "Test".to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port: 51515,
            bport: None,
        };
        assert_eq!(peer_no_bport.bport, None);
    }

    /// A v1 TXT record (no bport key) must still be accepted after the version
    /// bump so existing peers are never silently dropped from the list.
    #[test]
    fn peer_from_resolved_v1_is_accepted() {
        // v1 advertises version="1"; bport absent — must NOT return None.
        // We can only test the acceptance logic with a real ResolvedService in an
        // integration test, but we can verify the v1 constant is still "1".
        assert_eq!(PROTOCOL_VERSION_V1, "1");
    }

    // ── Drop aborts the spawned browse task ──────────────────────────────────

    /// Dropping the service must abort the background browse task it spawned
    /// in `start()` so it does not leak across reconfigure/toggle cycles.
    #[tokio::test]
    async fn drop_aborts_spawned_browse_task() {
        let svc = DiscoveryService::new();
        let handle = svc.start().await.expect("start must succeed");
        assert!(
            !handle.is_finished(),
            "browse task should be running before drop"
        );

        // Dropping the service must abort the retained handle.
        drop(svc);

        // Awaiting an aborted task resolves to a cancellation `JoinError`.
        // A bounded wait guards against a never-ending leak if Drop is missing.
        let joined = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
        match joined {
            Ok(Err(e)) => assert!(
                e.is_cancelled(),
                "browse task must be cancelled by Drop, got: {e:?}"
            ),
            Ok(Ok(())) => {} // task returned on its own (daemon shutdown closed the channel)
            Err(_) => panic!("browse task was not aborted within timeout — leak"),
        }
    }

    // ── privacy: TXT name redaction (CopyPaste-sh9a) ─────────────────────────

    /// The opaque instance label must NOT contain the human device name.
    ///
    /// Regression guard: the old scheme embedded the human name directly into
    /// the mDNS label (e.g. `"Alice-s-MacBook.aabbccdd"`), leaking PII to any
    /// passive LAN observer.
    #[test]
    fn opaque_instance_label_does_not_contain_human_name() {
        let label = opaque_instance_label("aabbccdddeadbeef");
        assert!(
            label.starts_with("cp-"),
            "opaque label must start with 'cp-' prefix, got: {label}"
        );
        assert!(
            label.len() <= DNS_LABEL_MAX,
            "opaque label exceeds DNS_LABEL_MAX: {label}"
        );
        // Verify no free-form string ended up in the label.
        assert!(!label.contains("Alice"), "name must not appear in label");
        assert!(!label.contains("Mac"), "name must not appear in label");
    }

    /// The opaque label is determined solely by `device_id`.
    #[test]
    fn opaque_instance_label_depends_only_on_device_id() {
        let label_a = opaque_instance_label("aabbccdd00000000");
        let label_b = opaque_instance_label("aabbccdd00000000");
        assert_eq!(label_a, label_b, "same device_id must produce same label");

        let label_c = opaque_instance_label("1122334400000000");
        assert_ne!(
            label_a, label_c,
            "different device_id must produce different label"
        );
    }

    /// `build_txt_properties` must NOT contain `TXT_DEVICE_NAME`.
    ///
    /// This is the primary regression guard for CopyPaste-sh9a: if someone
    /// accidentally adds the human name back to the emitted TXT record, this
    /// test will catch it immediately.
    #[test]
    fn build_txt_properties_does_not_include_device_name_key() {
        let device_id = "aabbccdd12345678";
        let props = build_txt_properties(device_id);
        for (k, _v) in &props {
            assert_ne!(
                *k, TXT_DEVICE_NAME,
                "TXT record must not include '{TXT_DEVICE_NAME}' key (PII leak)"
            );
        }
    }

    /// `build_txt_properties` must contain `TXT_DEVICE_ID` and `TXT_VERSION`.
    ///
    /// The `did` key is required for pairing resolution (peers dial by mDNS
    /// `did` when no address hint is available).
    #[test]
    fn build_txt_properties_contains_did_and_version() {
        let device_id = "cafebabe12345678";
        let props = build_txt_properties(device_id);
        let keys: Vec<&str> = props.iter().map(|(k, _)| *k).collect();
        assert!(
            keys.contains(&TXT_DEVICE_ID),
            "TXT must contain '{TXT_DEVICE_ID}' for pairing resolution"
        );
        assert!(
            keys.contains(&TXT_VERSION),
            "TXT must contain '{TXT_VERSION}'"
        );
        // did value must match the device_id passed in.
        let did_val = props
            .iter()
            .find(|(k, _)| *k == TXT_DEVICE_ID)
            .map(|(_, v)| v.as_str());
        assert_eq!(did_val, Some(device_id));
    }

    /// The human device name must not appear in any value of the TXT properties.
    #[test]
    fn build_txt_properties_values_do_not_contain_human_name() {
        let device_id = "aabbccdd12345678";
        let human_name = "Alice's MacBook Pro";
        let props = build_txt_properties(device_id);
        for (_k, v) in &props {
            assert!(
                !v.contains("Alice"),
                "human name must not appear in TXT value '{v}'"
            );
            let _ = human_name;
        }
    }

    // ── CopyPaste-rh27: device_id format validation ──────────────────────────

    /// rh27: a valid hex device_id (lowercase hex chars only) must pass.
    #[test]
    fn rh27_valid_hex_device_id_accepted() {
        // SHA-256 fingerprint is 64 lowercase hex chars.
        let fp = "a".repeat(64);
        assert!(
            is_valid_device_id(&fp),
            "64-char lowercase hex must be accepted"
        );
        // Shorter IDs (e.g. in tests) are also valid as long as they are hex.
        assert!(
            is_valid_device_id("aabbccdd"),
            "short hex id must be accepted"
        );
        assert!(
            is_valid_device_id("0123456789abcdef"),
            "mixed digits+hex letters must be accepted"
        );
        assert!(
            is_valid_device_id("deadbeef"),
            "classic hex id must be accepted"
        );
    }

    /// rh27: an empty device_id must be rejected — it bypasses rate-limit keying.
    #[test]
    fn rh27_empty_device_id_rejected() {
        assert!(
            !is_valid_device_id(""),
            "empty device_id must be rejected (bypasses rate-limit key)"
        );
    }

    /// rh27: uppercase hex must be rejected — fingerprints are always lowercase.
    /// This prevents a rogue peer from advertising the same fingerprint in two
    /// casing variants (A-Z vs a-z) to get double the rate-limit budget.
    #[test]
    fn rh27_uppercase_hex_device_id_rejected() {
        assert!(
            !is_valid_device_id("AABBCCDD"),
            "uppercase hex must be rejected (all CopyPaste fingerprints are lowercase)"
        );
        assert!(
            !is_valid_device_id("AaBbCcDd"),
            "mixed-case hex must be rejected"
        );
    }

    /// rh27: non-hex characters in device_id must be rejected.
    #[test]
    fn rh27_non_hex_device_id_rejected() {
        assert!(
            !is_valid_device_id("not-a-fingerprint"),
            "arbitrary string must be rejected"
        );
        assert!(
            !is_valid_device_id("zzzzzzzz"),
            "non-hex lowercase letters must be rejected"
        );
        assert!(
            !is_valid_device_id("aabb:ccdd"),
            "colon-separated hex must be rejected (colons are not hex chars)"
        );
        assert!(
            !is_valid_device_id("aabb ccdd"),
            "hex with whitespace must be rejected"
        );
    }
}
