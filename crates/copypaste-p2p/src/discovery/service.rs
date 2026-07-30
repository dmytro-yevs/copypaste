//! The live half: the mDNS daemon, the advertisement's lifecycle, and the two
//! background threads that keep the peer table current.
//!
//! Nothing here may fail loudly (see the module docs): [`Discovery::start`]
//! returns a working handle even when the host forbids multicast — containers,
//! guest Wi-Fi, a locked-down corporate network — and [`Discovery::peers`] then
//! stays empty with the reason logged at debug. That degraded mode is the
//! normal case in our own test environment, so the tests below exercise it.
//!
//! The two loops take an `IntoIterator` rather than the concrete mdns-sd
//! receiver, so they can be driven from a plain `Vec` of events with no network
//! and no daemon.

use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use mdns_sd::{DaemonEvent, ServiceDaemon, ServiceEvent};
use tracing::debug;

use super::names::{instance_of, sanitise_instance};
use super::record::{build_service_info, peers_from_resolved};
use super::table::{now_ms, DiscoveredPeer, PeerTable};
use super::DiscoveryError;
use crate::SERVICE_TYPE;

/// How long teardown waits for the mDNS daemon to acknowledge. Bounded so
/// `shutdown` can never wedge the caller on a stuck daemon thread.
const TEARDOWN_TIMEOUT: Duration = Duration::from_millis(500);

/// Advertises this device and tracks the peers it hears from.
///
/// Cheap to keep around and safe to drop: dropping unregisters the
/// advertisement and stops the background threads, same as [`Discovery::shutdown`].
pub struct Discovery {
    /// `None` once torn down, and also when the mDNS daemon could not be
    /// created at all — the degraded mode described in the module docs.
    daemon: Option<ServiceDaemon>,
    shared: Arc<Shared>,
    device_name: String,
    port: u16,
}

/// State the background threads and the public API share.
#[derive(Debug, Default)]
struct Shared {
    table: Mutex<PeerTable>,
    /// Instance name and fullname we currently advertise, if registration went
    /// through: needed to re-register on `republish`, unregister on teardown,
    /// and filter our own advertisement out of browse results. mDNS resolves
    /// name conflicts by renaming, so it is not necessarily what we asked for;
    /// the monitor thread keeps it current.
    registration: Mutex<Option<Registration>>,
}

#[derive(Debug, Clone)]
struct Registration {
    instance: String,
    fullname: String,
}

impl Discovery {
    /// Start advertising this device and browsing for others.
    ///
    /// Returns `Err` only for input we refuse to put on the wire. Every network
    /// failure — no multicast, no interfaces, no permission — yields `Ok` with
    /// a handle whose [`peers`](Self::peers) stays empty, logged at debug.
    pub fn start(
        device_name: &str,
        pairing_ids: &[String],
        port: u16,
    ) -> Result<Self, DiscoveryError> {
        // Validate and encode before touching the network, so bad input is a
        // clean error rather than a half-started daemon.
        let info = build_service_info(device_name, device_name, pairing_ids, port)?;
        let registration = Registration {
            instance: sanitise_instance(device_name).ok_or(DiscoveryError::InvalidDeviceName)?,
            fullname: info.get_fullname().to_string(),
        };

        let shared = Arc::new(Shared::default());
        let mut this = Self {
            daemon: None,
            shared: Arc::clone(&shared),
            device_name: device_name.to_string(),
            port,
        };

        // `ServiceDaemon::new` only binds a loopback signalling socket; the
        // multicast sockets are opened later on the daemon thread, so a host
        // that forbids multicast fails through `DaemonEvent::Error` rather than
        // here. Either way we degrade.
        let daemon = match ServiceDaemon::new() {
            Ok(daemon) => daemon,
            Err(e) => {
                debug!(reason = %e, "mdns daemon unavailable; discovery is disabled");
                return Ok(this);
            }
        };

        match daemon.register(info) {
            Ok(()) => *lock(&shared.registration) = Some(registration),
            Err(e) => debug!(reason = %e, "mdns registration refused; not advertising"),
        }

        match daemon.browse(SERVICE_TYPE) {
            Ok(events) => {
                let shared = Arc::clone(&shared);
                spawn("copypaste-mdns-browse", move || {
                    browse_loop(&shared, events)
                });
            }
            Err(e) => debug!(reason = %e, "mdns browse refused; peer list stays empty"),
        }

        match daemon.monitor() {
            Ok(events) => {
                let shared = Arc::clone(&shared);
                spawn("copypaste-mdns-monitor", move || {
                    monitor_loop(&shared, events);
                });
            }
            Err(e) => debug!(reason = %e, "mdns monitor refused"),
        }

        this.daemon = Some(daemon);
        Ok(this)
    }

    /// Currently-known peers, stale entries dropped.
    ///
    /// Never blocks on the network — it reads the table the browse thread fills.
    pub fn peers(&self) -> Vec<DiscoveredPeer> {
        lock(&self.shared.table).snapshot(now_ms())
    }

    /// Look up one by pairing id. If several addresses claim it, the most
    /// recently seen wins.
    pub fn find(&self, pairing_id: &str) -> Option<DiscoveredPeer> {
        lock(&self.shared.table).find(pairing_id, now_ms())
    }

    /// Update the advertised pairing ids after a new pairing.
    ///
    /// A no-op returning `Ok` when discovery is degraded — there is nothing to
    /// republish, and a new pairing must not fail because mDNS is unavailable.
    pub fn republish(&self, pairing_ids: &[String]) -> Result<(), DiscoveryError> {
        // Re-register under whatever instance name the daemon settled on, so a
        // conflict-driven rename does not leave two advertisements behind.
        let instance = lock(&self.shared.registration)
            .as_ref()
            .map(|r| r.instance.clone())
            .unwrap_or_else(|| self.device_name.clone());

        let mut info = build_service_info(&instance, &self.device_name, pairing_ids, self.port)?;
        // The name is already ours; probing again would only conflict with our
        // own records and rename us to "<name> (2)".
        info.set_requires_probe(false);

        let Some(daemon) = self.daemon.as_ref() else {
            debug!("discovery is disabled; nothing to republish");
            return Ok(());
        };

        let fullname = info.get_fullname().to_string();
        daemon.register(info)?;
        *lock(&self.shared.registration) = Some(Registration { instance, fullname });
        Ok(())
    }

    pub fn shutdown(mut self) {
        self.teardown();
    }

    fn teardown(&mut self) {
        let Some(daemon) = self.daemon.take() else {
            return;
        };

        if let Some(registration) = lock(&self.shared.registration).take() {
            match daemon.unregister(&registration.fullname) {
                // Bounded: a wedged daemon thread must not hold up the caller.
                Ok(status) => drop(status.recv_timeout(TEARDOWN_TIMEOUT)),
                Err(e) => debug!(reason = %e, "mdns unregister refused"),
            }
        }

        match daemon.shutdown() {
            Ok(status) => drop(status.recv_timeout(TEARDOWN_TIMEOUT)),
            Err(e) => debug!(reason = %e, "mdns shutdown refused"),
        }

        // Dropping the daemon closes the browse and monitor channels, so both
        // background threads fall out of their `recv` loops on their own.
        lock(&self.shared.table).clear();
    }
}

impl Drop for Discovery {
    fn drop(&mut self) {
        self.teardown();
    }
}

impl std::fmt::Debug for Discovery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Discovery")
            .field("enabled", &self.daemon.is_some())
            .field("port", &self.port)
            .finish_non_exhaustive()
    }
}

fn spawn(name: &str, body: impl FnOnce() + Send + 'static) {
    if let Err(e) = std::thread::Builder::new()
        .name(name.to_string())
        .spawn(body)
    {
        // `io::Error` from thread spawning is a resource limit, not a path.
        debug!(reason = %e, "could not spawn discovery thread; discovery is disabled");
    }
}

/// Drains browse events until the daemon closes the channel.
fn browse_loop(shared: &Shared, events: impl IntoIterator<Item = ServiceEvent>) {
    for event in events {
        match event {
            ServiceEvent::ServiceResolved(resolved) => {
                if is_self(shared, &resolved.fullname) {
                    continue;
                }
                let now = now_ms();
                let peers = peers_from_resolved(&resolved, now);
                if peers.is_empty() {
                    continue;
                }
                lock(&shared.table).observe(&resolved.fullname, peers, now);
            }
            ServiceEvent::ServiceRemoved(_, fullname) => {
                lock(&shared.table).remove_service(&fullname);
            }
            _ => {}
        }
    }
    debug!("mdns browse channel closed; discovery is idle");
}

/// Logs daemon trouble at debug and keeps our advertised name current after a
/// conflict-driven rename.
fn monitor_loop(shared: &Shared, events: impl IntoIterator<Item = DaemonEvent>) {
    for event in events {
        match event {
            // This is how "multicast is not permitted here" actually arrives.
            DaemonEvent::Error(e) => debug!(reason = %e, "mdns daemon error; peers may be missed"),
            DaemonEvent::NameChange(change) => {
                let mut registration = lock(&shared.registration);
                if let Some(current) = registration.as_mut() {
                    if current.fullname == change.original {
                        if let Some(instance) = instance_of(&change.new_name) {
                            current.instance = instance;
                            current.fullname = change.new_name;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn is_self(shared: &Shared, fullname: &str) -> bool {
    lock(&shared.registration)
        .as_ref()
        .is_some_and(|r| r.fullname.eq_ignore_ascii_case(fullname))
}

/// A poisoned lock here means a background thread panicked while holding it.
/// The peer table is a cache of hearsay, so recovering it is strictly better
/// than propagating a panic into the daemon over a discovery convenience.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdns_sd::{ResolvedService, ServiceInfo};
    use std::net::{IpAddr, Ipv4Addr};

    /// A `ResolvedService` as the browse thread would receive one. Built from a
    /// `ServiceInfo` because `ResolvedService` is `#[non_exhaustive]` — and it
    /// keeps the test honest: the TXT record is the one our encoder produced.
    fn resolved_from(instance: &str, ids: &[String], ip: &str, port: u16) -> ResolvedService {
        let advertised = build_service_info(instance, instance, ids, port).unwrap();
        let props: Vec<mdns_sd::TxtProperty> =
            advertised.get_properties().iter().cloned().collect();
        ServiceInfo::new(SERVICE_TYPE, instance, "laptop.local.", ip, port, props)
            .unwrap()
            .as_resolved_service()
    }

    // -- start / degraded mode ------------------------------------------------

    /// The environment this runs in has no multicast. `start` must still hand
    /// back a working handle, quickly, and every accessor must answer without
    /// touching the network.
    #[test]
    fn start_degrades_without_multicast() {
        let ids = vec!["pair-one".to_string()];
        let discovery = Discovery::start("Test Device", &ids, crate::DEFAULT_PORT)
            .expect("start must not fail when the network disallows multicast");

        // No peer can have been resolved yet, with or without multicast.
        assert!(discovery.peers().is_empty());
        assert!(discovery.find("pair-one").is_none());
        assert!(discovery.find("nobody").is_none());

        // A new pairing must never fail because mDNS is unavailable.
        discovery
            .republish(&["pair-one".to_string(), "pair-two".to_string()])
            .expect("republish must tolerate a degraded daemon");

        discovery.shutdown();
    }

    #[test]
    fn start_rejects_input_we_will_not_put_on_the_wire() {
        assert!(matches!(
            Discovery::start("   ", &[], crate::DEFAULT_PORT),
            Err(DiscoveryError::InvalidDeviceName)
        ));
        assert!(matches!(
            Discovery::start(
                "Laptop",
                &["not a valid id".to_string()],
                crate::DEFAULT_PORT
            ),
            Err(DiscoveryError::InvalidPairingId)
        ));
    }

    // -- browse loop, driven from a Vec: no network, no daemon ----------------

    #[test]
    fn browse_loop_records_and_drops_services() {
        let shared = Shared::default();
        let ids = vec!["pair-one".to_string()];
        let resolved = resolved_from("Laptop", &ids, "192.168.1.9", 47_654);
        let fullname = resolved.fullname.clone();

        browse_loop(
            &shared,
            vec![ServiceEvent::ServiceResolved(Box::new(resolved))],
        );

        let found = lock(&shared.table).snapshot(now_ms());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].pairing_id, "pair-one");
        assert_eq!(found[0].name, "Laptop");
        assert_eq!(found[0].addr.port(), 47_654);
        assert_eq!(
            found[0].addr.ip(),
            IpAddr::V4(Ipv4Addr::new(192, 168, 1, 9))
        );

        browse_loop(
            &shared,
            vec![ServiceEvent::ServiceRemoved(
                SERVICE_TYPE.to_string(),
                fullname,
            )],
        );
        assert!(lock(&shared.table).snapshot(now_ms()).is_empty());
    }

    #[test]
    fn our_own_advertisement_is_not_a_peer() {
        let shared = Shared::default();
        let ids = vec!["pair-one".to_string()];
        let resolved = resolved_from("Laptop", &ids, "192.168.1.9", 47_654);
        *lock(&shared.registration) = Some(Registration {
            instance: "Laptop".to_string(),
            fullname: resolved.fullname.clone(),
        });

        browse_loop(
            &shared,
            vec![ServiceEvent::ServiceResolved(Box::new(resolved))],
        );
        assert!(lock(&shared.table).snapshot(now_ms()).is_empty());
    }
}
