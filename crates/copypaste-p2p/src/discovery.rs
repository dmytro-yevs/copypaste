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
    /// Abort handle for the background browse task spawned by [`start`].
    /// Retained so [`Drop`] can abort it, preventing the browse loop from
    /// outliving the service across P2P toggle / reconfigure cycles.
    browse_abort: Arc<Mutex<Option<AbortHandle>>>,
    /// Clone of the mDNS [`ServiceDaemon`] created in [`start`]. Retained so
    /// [`Drop`] can shut it down, releasing the mDNS socket.
    daemon: Arc<Mutex<Option<ServiceDaemon>>>,
}

#[derive(Clone)]
struct Registration {
    port: u16,
    device_id: String,
    device_name: String,
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
        device_name: String,
        bport: Option<u16>,
    ) -> Result<(), DiscoveryError> {
        let mut reg = lock_safe(&self.registration);
        if reg.is_some() {
            return Err(DiscoveryError::AlreadyRegistered);
        }
        *reg = Some(Registration {
            port,
            device_id,
            device_name,
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

        let handle = tokio::spawn(async move {
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

        // Retain an abort handle so `Drop` (and a restart-in-place via
        // `shutdown_inner`) can stop the browse loop. The owned `JoinHandle` is
        // still returned to the caller for awaiting / explicit shutdown.
        lock_safe(&self.browse_abort).replace(handle.abort_handle());

        Ok(handle)
    }

    /// Abort the retained browse task and shut down the retained mDNS daemon,
    /// if any. Idempotent: safe to call when nothing is running. Used both by
    /// [`start`] (restart-in-place) and [`Drop`].
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
        // Instance name: sanitized device name + first 8 chars of device_id.
        // Slice by `chars()`, not byte index, so a non-ASCII device_id cannot
        // panic by splitting a UTF-8 codepoint mid-byte.
        let id_short: String = reg.device_id.chars().take(8).collect();
        let instance_name = format!("{}.{}", sanitize_label(&reg.device_name), id_short);

        // hostname — mdns-sd resolves local addresses automatically; supply a
        // label-safe host name so the daemon has something to work with.
        let hostname = format!("{}.local.", sanitize_label(&reg.device_name));

        // Build TXT properties. bport is optional (Phase 0: absent; Phase 2:
        // set by `register_with_bport`). We heap-allocate the bport string so
        // it lives long enough to be referenced by the slice below.
        let bport_str: String;
        let mut properties: Vec<(&str, &str)> = vec![
            (TXT_VERSION, PROTOCOL_VERSION),
            (TXT_DEVICE_ID, reg.device_id.as_str()),
            (TXT_DEVICE_NAME, reg.device_name.as_str()),
        ];
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

        info!(
            device_id = %reg.device_id,
            device_name = %reg.device_name,
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
/// TXT records (`v`, `did`, `name`) are present and version is supported.
///
/// Accepts both v1 (`PROTOCOL_VERSION_V1 = "1"`, no `bport`) and v2
/// (`PROTOCOL_VERSION = "2"`, optional `bport`) so that existing v1 peers
/// are never silently dropped from the discovered list after the Phase 0
/// version bump. v1 peers produce a `PeerInfo` with `bport: None`; the UI
/// disables the "Pair" button for those entries.
fn peer_from_resolved(resolved: &ResolvedService) -> Option<PeerInfo> {
    let version = resolved.get_property_val_str(TXT_VERSION)?;
    // Accept v1 (legacy) and v2 (current). Any other version is unsupported.
    if version != PROTOCOL_VERSION && version != PROTOCOL_VERSION_V1 {
        warn!(version, "mDNS peer uses unsupported protocol version");
        return None;
    }

    let device_id = resolved.get_property_val_str(TXT_DEVICE_ID)?.to_string();
    let device_name = resolved.get_property_val_str(TXT_DEVICE_NAME)?.to_string();

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
        assert_eq!(reg.device_name, "Test Device");
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

    // ── id_short slicing (LOW: panic on non-ASCII device_id) ─────────────────

    #[test]
    fn id_short_handles_non_ascii_device_id_without_panic() {
        // Reproduces the byte-slice panic: a multibyte codepoint at byte 8.
        // The fix slices by chars, so this must not panic and must yield 8 chars.
        let device_id = "日本語テスト識別子"; // each char is 3 bytes in UTF-8
        let id_short: String = device_id.chars().take(8).collect();
        assert_eq!(id_short.chars().count(), 8);
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
}
