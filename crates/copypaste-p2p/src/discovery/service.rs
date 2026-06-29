//! [`DiscoveryService`]: the public-facing mDNS-SD service struct and all its impl.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tokio::task::{AbortHandle, JoinHandle};
use tracing::{debug, info, warn};

use super::browse::handle_event;
use super::lock_safe;
use super::registry::{
    build_txt_properties, opaque_hostname_label, opaque_instance_label, reannounce_once,
};
use super::types::{
    PeerFoundCallback, PeerInfo, PeerLostCallback, Registration, MDNS_REANNOUNCE_INTERVAL,
    SERVICE_TYPE, TXT_BPORT,
};
use crate::error::DiscoveryError;
use crate::rate_limit::MdnsRateLimiter;

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
    pub(super) on_found: Arc<Mutex<Vec<PeerFoundCallback>>>,
    /// Callbacks invoked when a peer disappears.
    pub(super) on_lost: Arc<Mutex<Vec<PeerLostCallback>>>,
    /// Currently known peers keyed by mDNS fullname.
    pub(super) known_peers: Arc<Mutex<HashMap<String, PeerInfo>>>,
    /// Port and identity used to advertise own service.
    pub(super) registration: Arc<Mutex<Option<Registration>>>,
    /// Per-source-IP token bucket guarding inbound `ServiceResolved` events
    /// from mDNS flood (THREAT-MODEL OI-3). See [`MdnsRateLimiter`].
    rate_limiter: Arc<MdnsRateLimiter>,
    /// Abort handle for the background browse task spawned by [`DiscoveryService::start`].
    /// Retained so [`Drop`] can abort it, preventing the browse loop from
    /// outliving the service across P2P toggle / reconfigure cycles.
    browse_abort: Arc<Mutex<Option<AbortHandle>>>,
    /// Abort handle for the periodic mDNS re-announcement task (crh3.103).
    /// Aborted alongside `browse_abort` in `shutdown_inner` / `Drop`.
    reannounce_abort: Arc<Mutex<Option<AbortHandle>>>,
    /// Clone of the mDNS [`ServiceDaemon`] created in [`DiscoveryService::start`]. Retained so
    /// [`Drop`] can shut it down, releasing the mDNS socket.
    daemon: Arc<Mutex<Option<ServiceDaemon>>>,
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
            reannounce_abort: Arc::new(Mutex::new(None)),
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
        //
        // CopyPaste-crh3.95: `shutdown_inner` aborts the previous browse/reannounce
        // tasks via `AbortHandle::abort()`, which is fire-and-forget — it does NOT
        // await the old task. So for a brief window (until the next scheduler tick
        // delivers the abort) the OLD browse task and the NEW one started below may
        // both be live. This overlap is SAFE and intentional: (1) inbound peer
        // events are de-duplicated through the shared `known_peers` HashMap (a
        // second observation of the same peer is a no-op), and (2) the shared
        // `rate_limiter` throttles repeated callbacks for the same peer — so a
        // transient double-browse cannot double-fire `on_found`/`on_lost` or leak a
        // duplicate peer. Awaiting the old task here is not possible without
        // retaining its `JoinHandle` (we keep only an `AbortHandle`), and is
        // unnecessary given those two guards.
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
        // Capture whether we registered before `own_id` is moved into the browse task.
        let had_registration = own_id.is_some();

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

        // ── Periodic re-announce task (CopyPaste-crh3.103) ──────────────────
        //
        // mDNS advertisements carry the IP addresses resolved at registration
        // time. After a Wi-Fi roam, VPN connect, or DHCP renew the host IP
        // changes but the mDNS record stays stale — peers dial the old address
        // and fail. Re-announcing every MDNS_REANNOUNCE_INTERVAL self-heals
        // the record without requiring platform-specific netlink / SCDynamicStore
        // change detection.
        //
        // The task holds clones of the daemon and registration Arcs (not self)
        // so it can be spawned as a 'static future.
        if had_registration {
            // Only start re-announce task if we actually registered a service.
            let reannounce_daemon_arc = Arc::clone(&self.daemon);
            let reannounce_reg_arc = Arc::clone(&self.registration);
            let reannounce_task = tokio::spawn(async move {
                let mut timer = tokio::time::interval(MDNS_REANNOUNCE_INTERVAL);
                // MissedTickBehavior::Skip: if a tick is missed (e.g. the task
                // was descheduled for longer than the interval), skip the missed
                // ticks rather than bursting — we never want a burst of
                // re-announcements on a congested LAN.
                timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                // Skip the immediate first tick; we just registered in start().
                timer.tick().await;
                loop {
                    timer.tick().await;
                    let daemon_opt = lock_safe(&reannounce_daemon_arc).clone();
                    let reg_opt = lock_safe(&reannounce_reg_arc).clone();
                    if let (Some(ref daemon), Some(ref reg)) = (daemon_opt, reg_opt) {
                        match reannounce_once(daemon, reg) {
                            Ok(()) => debug!("mDNS service re-announced (periodic IP refresh)"),
                            Err(e) => warn!("periodic mDNS re-announcement failed: {e}"),
                        }
                    }
                }
            });
            lock_safe(&self.reannounce_abort).replace(reannounce_task.abort_handle());
        }

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
        // Abort the periodic re-announce task too so it doesn't fire against a
        // daemon that's already been shut down.
        if let Some(abort) = lock_safe(&self.reannounce_abort).take() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_peer(id: &str, name: &str, port: u16) -> PeerInfo {
        PeerInfo {
            device_id: id.to_string(),
            device_name: name.to_string(),
            ip_addrs: vec!["127.0.0.1".parse().unwrap()],
            port,
            bport: None,
        }
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
