//! mDNS-SD peer discovery for CopyPaste.
//!
//! Registers own service under `_copypaste._tcp.local.` and browses for
//! other instances, emitting callbacks when peers appear or disappear.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use mdns_sd::{ResolvedService, ScopedIp, ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::task::JoinHandle;
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
/// Protocol version advertised in TXT records.
const PROTOCOL_VERSION: &str = "1";

/// Information about a discovered peer.
#[derive(Debug, Clone, PartialEq)]
pub struct PeerInfo {
    /// The peer's device ID (hex fingerprint of their public key).
    pub device_id: String,
    /// Human-readable device name.
    pub device_name: String,
    /// All resolved IP addresses for the peer (sorted, deduplicated).
    pub ip_addrs: Vec<IpAddr>,
    /// TCP port the peer is listening on.
    pub port: u16,
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
}

#[derive(Clone)]
struct Registration {
    port: u16,
    device_id: String,
    device_name: String,
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
    /// - `port`: TCP port this device's CopyPaste listener is on.
    /// - `device_id`: Hex fingerprint of this device's public key.
    /// - `device_name`: Human-readable name (e.g. "Alice's MacBook").
    pub fn register(
        &self,
        port: u16,
        device_id: impl Into<String>,
        device_name: impl Into<String>,
    ) -> Result<(), DiscoveryError> {
        let mut reg = lock_safe(&self.registration);
        if reg.is_some() {
            return Err(DiscoveryError::AlreadyRegistered);
        }
        *reg = Some(Registration {
            port,
            device_id: device_id.into(),
            device_name: device_name.into(),
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

        Ok(handle)
    }

    /// Return a snapshot of all currently known peers.
    pub fn peers(&self) -> Vec<PeerInfo> {
        lock_safe(&self.known_peers).values().cloned().collect()
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
        let id_short = &reg.device_id[..reg.device_id.len().min(8)];
        let instance_name = format!("{}.{}", sanitize_label(&reg.device_name), id_short);

        // hostname — mdns-sd resolves local addresses automatically; supply a
        // label-safe host name so the daemon has something to work with.
        let hostname = format!("{}.local.", sanitize_label(&reg.device_name));

        let properties = [
            (TXT_VERSION, PROTOCOL_VERSION),
            (TXT_DEVICE_ID, reg.device_id.as_str()),
            (TXT_DEVICE_NAME, reg.device_name.as_str()),
        ];

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

                // Dedup: only emit if this is a new or changed peer.
                let mut peers = lock_safe(known_peers);
                let is_new = peers
                    .get(&fullname)
                    .map(|existing| existing != &peer)
                    .unwrap_or(true);

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
/// TXT records (`v`, `did`, `name`) are present and version matches.
fn peer_from_resolved(resolved: &ResolvedService) -> Option<PeerInfo> {
    // Require protocol version == "1".
    let version = resolved.get_property_val_str(TXT_VERSION)?;
    if version != PROTOCOL_VERSION {
        warn!(version, "mDNS peer uses unsupported protocol version");
        return None;
    }

    let device_id = resolved.get_property_val_str(TXT_DEVICE_ID)?.to_string();
    let device_name = resolved.get_property_val_str(TXT_DEVICE_NAME)?.to_string();

    // Collect all addresses, deduplicated and sorted for determinism.
    let mut ip_addrs: Vec<IpAddr> = resolved
        .get_addresses()
        .iter()
        .map(scoped_ip_to_ip_addr)
        .collect();
    ip_addrs.sort_unstable_by_key(|a| (a.is_ipv6(), a.to_string()));
    ip_addrs.dedup();

    Some(PeerInfo {
        device_id,
        device_name,
        ip_addrs,
        port: resolved.get_port(),
    })
}

/// Convert a [`ScopedIp`] to a standard [`IpAddr`].
///
/// `ScopedIp` is `#[non_exhaustive]`; the wildcard arm handles any future
/// variants by falling back to the IPv4 unspecified address so callers
/// can safely filter it out if needed.
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

fn scoped_ip_to_ip_addr(scoped: &ScopedIp) -> IpAddr {
    match scoped {
        ScopedIp::V4(v4) => IpAddr::V4(*v4.addr()),
        ScopedIp::V6(v6) => IpAddr::V6(*v6.addr()),
        // Safety net for any future ScopedIp variants.
        &_ => IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
    }
}

/// Replace characters that are invalid in mDNS labels with hyphens and
/// trim leading/trailing hyphens.
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
    sanitized.trim_matches('-').to_string()
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
        assert_eq!(sanitize_label(""), "");
    }

    #[test]
    fn sanitize_label_pure_special_chars_becomes_empty() {
        // All specials → hyphens → trimmed → empty
        assert_eq!(sanitize_label("!!!"), "");
    }

    // ── PeerInfo helpers ─────────────────────────────────────────────────────

    fn make_peer(id: &str, name: &str, port: u16) -> PeerInfo {
        PeerInfo {
            device_id: id.to_string(),
            device_name: name.to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port,
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
}
