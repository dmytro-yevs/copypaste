//! LAN peer discovery over mDNS-SD.
//!
//! # Discovery is a convenience, never a dependency
//!
//! Every path in the daemon works with an explicit `host:port`. This module
//! only saves the user typing one. Consequently *nothing here is allowed to
//! fail loudly*: [`Discovery::start`] returns a working handle even when the
//! host forbids multicast (containers, guest Wi-Fi, a locked-down corporate
//! network), in which case [`Discovery::peers`] simply stays empty and the
//! reason is logged at debug. That degraded mode is the normal case in our own
//! test environment, so it is exercised by the tests below.
//!
//! # Presence is not trust
//!
//! A [`DiscoveredPeer`] means "something on this LAN answered an mDNS query and
//! claimed this pairing id". Anyone on the network can claim anything. Trust
//! comes only from the Noise `NNpsk0` handshake in [`crate::transport`], which
//! requires the pairing token; discovery just supplies a candidate address to
//! try. A hostile advertiser can therefore waste one handshake attempt and
//! nothing more.
//!
//! # What goes in the TXT record
//!
//! Only non-secret material:
//!
//! | key | value |
//! |---|---|
//! | `v` | discovery record version, currently `1` |
//! | `n` | device display name, UTF-8 |
//! | `p0`…`pN` | one advertised `pairing_id` per key |
//!
//! **The pairing token / PSK is never advertised, in any form — not hashed, not
//! truncated, not as a "fingerprint".** The `pairing_id` is a public handle for
//! a pairing; the token is the key to it. Anything derived from the token that
//! appears on the wire is a gift to a passive listener, so the encoder here
//! only ever sees the id. `advertisement_has_no_secret` in the tests pins the
//! allowed key set so a future key cannot be added without the test failing.
//!
//! # Why mdns-sd rather than our own responder
//!
//! Per `CLAUDE.md` rule 1: mDNS is a real protocol with probing, conflict
//! resolution, cache coherency and known-answer suppression. `mdns-sd` is
//! maintained, pure Rust, and works on both macOS and Android. The only code
//! here is the CopyPaste-specific part — what we put in TXT, and how long we
//! believe it.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use mdns_sd::{
    DaemonEvent, ResolvedService, ServiceDaemon, ServiceEvent, ServiceInfo, TxtProperties,
};
use tracing::debug;

use crate::SERVICE_TYPE;

/// Version stamped into the TXT record, so a future format change can be
/// ignored by old builds instead of misread by them.
const TXT_VERSION: &str = "1";

const TXT_KEY_VERSION: &str = "v";
const TXT_KEY_NAME: &str = "n";
/// Pairing id keys are this prefix followed by a decimal index: `p0`, `p1`, …
const TXT_KEY_PAIRING_PREFIX: &str = "p";

/// How long a peer stays in the table after it was last seen.
///
/// mdns-sd publishes host records (A/SRV) with a 120 s TTL and refreshes ahead
/// of expiry, so three minutes rides out a single lost refresh while still
/// dropping a device that walked out of the building. Removal is normally
/// prompt anyway — `ServiceRemoved` deletes the entry immediately — this is the
/// backstop for the case where the goodbye packet never arrives.
pub const PEER_TTL: Duration = Duration::from_secs(180);

/// Hard ceiling on tracked peers.
///
/// mDNS is unauthenticated: anyone on the LAN can announce as many instances as
/// they like. The table is bounded so that costs them nothing and us nothing.
/// When full, the least recently seen entry is evicted — a live peer refreshes
/// itself well inside [`PEER_TTL`], so flooding cannot push out a real device
/// that is still talking.
pub const MAX_PEERS: usize = 256;

/// Ceiling on pairing ids accepted from a single advertisement, so one host
/// cannot fill the table on its own.
pub const MAX_PAIRING_IDS_PER_PEER: usize = 16;

/// Ceiling on pairing ids we advertise. Beyond this the record stops fitting
/// comfortably in one mDNS packet. Extra ids are dropped with a debug log
/// rather than raising an error: discovery is a convenience, and refusing to
/// advertise must never be able to break pairing.
pub const MAX_ADVERTISED_PAIRING_IDS: usize = 16;

/// Longest pairing id accepted or advertised, in bytes.
const MAX_PAIRING_ID_LEN: usize = 64;
/// Longest device name accepted or advertised, in bytes. Keeps `n=<name>`
/// inside the 255-byte limit RFC 6763 §6.1 puts on one TXT string.
const MAX_NAME_LEN: usize = 128;
/// Longest DNS label, per RFC 1035 §2.3.4.
const MAX_LABEL_LEN: usize = 63;

/// How long teardown waits for the mDNS daemon to acknowledge. Bounded so
/// `shutdown` can never wedge the caller on a stuck daemon thread.
const TEARDOWN_TIMEOUT: Duration = Duration::from_millis(500);

/// A peer seen on the network. Presence here means "reachable", never
/// "trusted" — trust comes only from having been paired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// From the TXT record; may be unknown to us, and is unverified until the
    /// Noise handshake succeeds.
    pub pairing_id: String,
    /// Display name the peer advertised. Untrusted, unvalidated beyond length
    /// and control-character stripping — never use it as an identity.
    pub name: String,
    pub addr: SocketAddr,
    pub last_seen_ms: i64,
}

/// Discovery never surfaces a filesystem path or any secret in these messages;
/// the daemon socket path in particular would disclose the local username.
#[derive(Debug, thiserror::Error)]
pub enum DiscoveryError {
    /// The device name is empty, or is nothing but characters we must strip.
    #[error("device name is empty or unusable")]
    InvalidDeviceName,

    /// A pairing id was not an identifier we are willing to put on the wire.
    #[error("pairing id is not a valid identifier")]
    InvalidPairingId,

    /// The mDNS library refused a command. Its messages carry socket and
    /// interface details only — no paths, no key material.
    #[error("mdns is unavailable: {0}")]
    Mdns(String),
}

impl From<mdns_sd::Error> for DiscoveryError {
    fn from(e: mdns_sd::Error) -> Self {
        Self::Mdns(e.to_string())
    }
}

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
    /// through. Tracked so we can re-register on `republish`, unregister on
    /// teardown, and filter our own advertisement out of the browse results.
    ///
    /// mDNS resolves name conflicts by renaming, so this is not necessarily
    /// what we asked for; the monitor thread keeps it current.
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

// ---------------------------------------------------------------------------
// Background threads
// ---------------------------------------------------------------------------

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
///
/// `IntoIterator` rather than the concrete receiver so the loop is unit
/// testable from a plain `Vec` of events, with no network and no daemon.
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

// ---------------------------------------------------------------------------
// Advertisement: encode
// ---------------------------------------------------------------------------

/// Build the advertisement. Pure — touches no socket, so the encoding is
/// testable on a host with no network at all.
///
/// `instance` is the mDNS instance label (which the daemon may rename on
/// conflict); `device_name` is what we put in TXT for humans to read.
fn build_service_info(
    instance: &str,
    device_name: &str,
    pairing_ids: &[String],
    port: u16,
) -> Result<ServiceInfo, DiscoveryError> {
    let instance = sanitise_instance(instance).ok_or(DiscoveryError::InvalidDeviceName)?;
    let display = sanitise_display_name(device_name).ok_or(DiscoveryError::InvalidDeviceName)?;
    let hostname = format!(
        "{}.local.",
        sanitise_host_label(device_name).unwrap_or_else(|| "copypaste".to_string())
    );

    // Deduplicate, keeping the caller's order, then cap.
    let mut advertised: Vec<&String> = Vec::new();
    for id in pairing_ids {
        if !is_valid_pairing_id(id) {
            return Err(DiscoveryError::InvalidPairingId);
        }
        if !advertised.contains(&id) {
            advertised.push(id);
        }
    }
    if advertised.len() > MAX_ADVERTISED_PAIRING_IDS {
        debug!(
            advertised = MAX_ADVERTISED_PAIRING_IDS,
            held = advertised.len(),
            "too many pairings to advertise; the rest still work with an explicit address"
        );
        advertised.truncate(MAX_ADVERTISED_PAIRING_IDS);
    }

    // Only these three kinds of key ever exist. No token, no PSK, no digest of
    // either — see the module docs.
    let mut txt: HashMap<String, String> = HashMap::new();
    txt.insert(TXT_KEY_VERSION.to_string(), TXT_VERSION.to_string());
    txt.insert(TXT_KEY_NAME.to_string(), display);
    for (i, id) in advertised.iter().enumerate() {
        txt.insert(format!("{TXT_KEY_PAIRING_PREFIX}{i}"), (*id).clone());
    }

    // Addresses are left to the daemon: `enable_addr_auto` keeps the A/AAAA
    // records in step with the host as interfaces come and go, which a laptop
    // moving between Wi-Fi and a dock does constantly.
    let info = ServiceInfo::new(SERVICE_TYPE, &instance, &hostname, (), port, txt)?;
    Ok(info.enable_addr_auto())
}

// ---------------------------------------------------------------------------
// Advertisement: decode
// ---------------------------------------------------------------------------

/// What we were able to read out of somebody's TXT record.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Advertisement {
    name: String,
    pairing_ids: Vec<String>,
}

/// Parse a TXT record. Returns `None` for a record that is not ours or not this
/// version — a foreign service on `_copypaste._tcp` is simply ignored.
///
/// Everything here is attacker-controlled, so each field is length-bounded and
/// character-checked before it can reach a log line or the peer table.
fn parse_advertisement(txt: &TxtProperties, fallback_name: &str) -> Option<Advertisement> {
    if txt.get_property_val_str(TXT_KEY_VERSION)? != TXT_VERSION {
        return None;
    }

    let name = txt
        .get_property_val_str(TXT_KEY_NAME)
        .and_then(sanitise_display_name)
        .or_else(|| sanitise_display_name(fallback_name))
        .unwrap_or_else(|| "unknown".to_string());

    // Collect `p<n>` in index order so the result is deterministic regardless
    // of how the peer ordered its strings.
    let mut indexed: Vec<(usize, String)> = Vec::new();
    for prop in txt.iter() {
        let Some(index) = prop.key().strip_prefix(TXT_KEY_PAIRING_PREFIX) else {
            continue;
        };
        let Ok(index) = index.parse::<usize>() else {
            continue;
        };
        let value = prop.val_str();
        if !is_valid_pairing_id(value) {
            continue;
        }
        indexed.push((index, value.to_string()));
    }
    indexed.sort_unstable();

    let mut pairing_ids: Vec<String> = Vec::new();
    for (_, id) in indexed {
        if pairing_ids.len() >= MAX_PAIRING_IDS_PER_PEER {
            break;
        }
        if !pairing_ids.contains(&id) {
            pairing_ids.push(id);
        }
    }

    Some(Advertisement { name, pairing_ids })
}

/// One [`DiscoveredPeer`] per pairing id the service claims, so `find` is a
/// direct lookup.
fn peers_from_resolved(resolved: &ResolvedService, now_ms: i64) -> Vec<DiscoveredPeer> {
    let Some(advertisement) = parse_advertisement(&resolved.txt_properties, &resolved.fullname)
    else {
        return Vec::new();
    };
    if resolved.port == 0 {
        return Vec::new();
    }

    let addrs: Vec<IpAddr> = resolved.addresses.iter().map(|a| a.to_ip_addr()).collect();
    let Some(ip) = best_addr(&addrs) else {
        return Vec::new();
    };
    let addr = SocketAddr::new(ip, resolved.port);

    advertisement
        .pairing_ids
        .into_iter()
        .map(|pairing_id| DiscoveredPeer {
            pairing_id,
            name: advertisement.name.clone(),
            addr,
            last_seen_ms: now_ms,
        })
        .collect()
}

/// Prefer a routable IPv4 address: it is the one most likely to connect, and
/// picking deterministically keeps the table stable across re-resolutions.
fn best_addr(addrs: &[IpAddr]) -> Option<IpAddr> {
    let rank = |ip: &IpAddr| match ip {
        IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => 0,
        IpAddr::V6(v6) if !v6.is_loopback() && !v6.is_unspecified() => 1,
        IpAddr::V4(_) => 2,
        IpAddr::V6(_) => 3,
    };
    addrs
        .iter()
        .min_by_key(|ip| (rank(ip), ip.to_string()))
        .copied()
}

// ---------------------------------------------------------------------------
// Peer table: expiry and capping. Pure, so both are tested without a network.
// ---------------------------------------------------------------------------

/// Keyed by `(service fullname, pairing id)`: one device may hold several
/// pairings, and two devices may legitimately claim the same pairing id
/// (a reinstall, a device that moved networks) without evicting each other.
type PeerKey = (String, String);

#[derive(Debug)]
struct PeerTable {
    entries: HashMap<PeerKey, DiscoveredPeer>,
    ttl_ms: i64,
    max_peers: usize,
}

impl Default for PeerTable {
    fn default() -> Self {
        Self::new(PEER_TTL.as_millis() as i64, MAX_PEERS)
    }
}

impl PeerTable {
    fn new(ttl_ms: i64, max_peers: usize) -> Self {
        Self {
            entries: HashMap::new(),
            ttl_ms,
            max_peers: max_peers.max(1),
        }
    }

    fn observe(&mut self, fullname: &str, peers: Vec<DiscoveredPeer>, now_ms: i64) {
        self.prune(now_ms);
        for peer in peers {
            self.entries
                .insert((fullname.to_string(), peer.pairing_id.clone()), peer);
        }
        self.enforce_cap();
    }

    fn remove_service(&mut self, fullname: &str) {
        self.entries.retain(|(service, _), _| service != fullname);
    }

    fn prune(&mut self, now_ms: i64) {
        let ttl = self.ttl_ms;
        // `now - last_seen` rather than a stored deadline, so a backwards step
        // of the wall clock expires entries early instead of stranding them.
        self.entries
            .retain(|_, peer| now_ms.saturating_sub(peer.last_seen_ms) < ttl);
    }

    /// Evict least-recently-seen until we are inside the cap. A live peer
    /// refreshes well inside the TTL, so a flood evicts its own entries first.
    fn enforce_cap(&mut self) {
        while self.entries.len() > self.max_peers {
            let victim = self
                .entries
                .iter()
                .min_by(|a, b| {
                    a.1.last_seen_ms
                        .cmp(&b.1.last_seen_ms)
                        .then_with(|| a.0.cmp(b.0))
                })
                .map(|(key, _)| key.clone());
            match victim {
                Some(key) => drop(self.entries.remove(&key)),
                None => break,
            }
        }
    }

    fn snapshot(&mut self, now_ms: i64) -> Vec<DiscoveredPeer> {
        self.prune(now_ms);
        let mut peers: Vec<DiscoveredPeer> = self.entries.values().cloned().collect();
        peers.sort_by(|a, b| {
            a.pairing_id
                .cmp(&b.pairing_id)
                .then_with(|| a.addr.cmp(&b.addr))
        });
        peers
    }

    fn find(&mut self, pairing_id: &str, now_ms: i64) -> Option<DiscoveredPeer> {
        self.prune(now_ms);
        self.entries
            .values()
            .filter(|peer| peer.pairing_id == pairing_id)
            .max_by(|a, b| {
                a.last_seen_ms
                    .cmp(&b.last_seen_ms)
                    .then_with(|| a.addr.cmp(&b.addr))
            })
            .cloned()
    }

    fn clear(&mut self) {
        self.entries.clear();
    }
}

// ---------------------------------------------------------------------------
// Small helpers
// ---------------------------------------------------------------------------

/// Milliseconds since the Unix epoch. Local because `copypaste-p2p` does not
/// depend on `copypaste-core`, where the shared helper lives.
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// A poisoned lock here means a background thread panicked while holding it.
/// The peer table is a cache of hearsay, so recovering it is strictly better
/// than propagating a panic into the daemon over a discovery convenience.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn is_valid_pairing_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_PAIRING_ID_LEN
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// Truncate on a char boundary so the result is still valid UTF-8.
fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// Strip control characters — they would otherwise reach logs and the UI from
/// an unauthenticated source.
fn strip_controls(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

fn sanitise_display_name(name: &str) -> Option<String> {
    let cleaned = strip_controls(name);
    let cleaned = truncate_bytes(cleaned.trim(), MAX_NAME_LEN).trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

/// mDNS instance label. `.` is dropped rather than escaped so that splitting a
/// fullname back into instance + service type stays unambiguous.
fn sanitise_instance(name: &str) -> Option<String> {
    let cleaned: String = strip_controls(name)
        .chars()
        .map(|c| if c == '.' || c == '\\' { '-' } else { c })
        .collect();
    let cleaned = truncate_bytes(cleaned.trim(), MAX_LABEL_LEN).trim();
    (!cleaned.is_empty()).then(|| cleaned.to_string())
}

/// A single DNS label for the host record: lowercase ASCII alphanumerics and
/// hyphens only.
fn sanitise_host_label(name: &str) -> Option<String> {
    let mapped: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = truncate_bytes(&mapped, MAX_LABEL_LEN)
        .trim_matches('-')
        .to_string();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Recover the instance label from a fullname such as
/// `Laptop (2)._copypaste._tcp.local.`.
fn instance_of(fullname: &str) -> Option<String> {
    let instance = fullname
        .strip_suffix(SERVICE_TYPE)?
        .strip_suffix('.')?
        .to_string();
    (!instance.is_empty()).then_some(instance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    const SECRET: &str = "d3adb33fd3adb33fd3adb33fd3adb33fd3adb33fd3adb33fd3adb33fd3adb33f";

    fn peer(pairing_id: &str, last_seen_ms: i64) -> DiscoveredPeer {
        DiscoveredPeer {
            pairing_id: pairing_id.to_string(),
            name: "peer".to_string(),
            addr: SocketAddr::new(
                IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)),
                crate::DEFAULT_PORT,
            ),
            last_seen_ms,
        }
    }

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

    /// Everything a TXT record puts on the wire, flattened, for the audit test.
    fn txt_bytes(info: &ServiceInfo) -> Vec<u8> {
        let mut out = Vec::new();
        for prop in info.get_properties().iter() {
            out.extend_from_slice(prop.key().as_bytes());
            out.push(b'=');
            out.extend_from_slice(prop.val().unwrap_or_default());
            out.push(0);
        }
        out
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

    // -- TXT encode / decode --------------------------------------------------

    #[test]
    fn txt_round_trip() {
        let ids = vec![
            "pair-one".to_string(),
            "pair_two".to_string(),
            "P3".to_string(),
        ];
        let info = build_service_info("Dmitriy's Laptop", "Dmitriy's Laptop", &ids, 47_654)
            .expect("valid advertisement");

        let parsed =
            parse_advertisement(info.get_properties(), "fallback").expect("record is ours");

        assert_eq!(parsed.name, "Dmitriy's Laptop");
        assert_eq!(parsed.pairing_ids, ids);
        assert_eq!(info.get_port(), 47_654);
        assert!(info.get_fullname().ends_with(SERVICE_TYPE));
    }

    #[test]
    fn txt_round_trip_survives_a_renamed_instance() {
        let ids = vec!["pair-one".to_string()];
        // What `republish` does after a conflict rename: instance differs from
        // the display name, but the display name still round-trips.
        let info = build_service_info("Laptop (2)", "Laptop", &ids, 47_654).unwrap();
        let parsed = parse_advertisement(info.get_properties(), "fallback").unwrap();
        assert_eq!(parsed.name, "Laptop");
        assert_eq!(
            instance_of(info.get_fullname()).as_deref(),
            Some("Laptop (2)")
        );
    }

    #[test]
    fn foreign_and_versionless_records_are_ignored() {
        let mut txt = HashMap::new();
        txt.insert("p0".to_string(), "pair-one".to_string());
        let info = ServiceInfo::new(SERVICE_TYPE, "other", "other.local.", (), 1, txt).unwrap();
        assert!(parse_advertisement(info.get_properties(), "other").is_none());

        let mut txt = HashMap::new();
        txt.insert("v".to_string(), "99".to_string());
        let info = ServiceInfo::new(SERVICE_TYPE, "future", "future.local.", (), 1, txt).unwrap();
        assert!(parse_advertisement(info.get_properties(), "future").is_none());
    }

    #[test]
    fn hostile_txt_fields_are_bounded_and_scrubbed() {
        let mut txt = HashMap::new();
        txt.insert("v".to_string(), TXT_VERSION.to_string());
        txt.insert("n".to_string(), "evil\u{7}\u{1b}[31mname".to_string());
        txt.insert("p0".to_string(), "ok-id".to_string());
        txt.insert("p1".to_string(), "../../etc/passwd".to_string());
        txt.insert("p2".to_string(), "x".repeat(MAX_PAIRING_ID_LEN + 1));
        txt.insert("p3".to_string(), String::new());
        txt.insert("pnotanumber".to_string(), "sneaky".to_string());
        let info = ServiceInfo::new(SERVICE_TYPE, "evil", "evil.local.", (), 1, txt).unwrap();

        let parsed = parse_advertisement(info.get_properties(), "fallback").unwrap();
        assert_eq!(parsed.name, "evil[31mname");
        assert_eq!(parsed.pairing_ids, vec!["ok-id".to_string()]);
    }

    #[test]
    fn a_single_advertisement_cannot_flood_the_table() {
        let mut txt = HashMap::new();
        txt.insert("v".to_string(), TXT_VERSION.to_string());
        txt.insert("n".to_string(), "greedy".to_string());
        for i in 0..(MAX_PAIRING_IDS_PER_PEER * 4) {
            txt.insert(format!("p{i}"), format!("id-{i}"));
        }
        let info = ServiceInfo::new(SERVICE_TYPE, "greedy", "greedy.local.", (), 1, txt).unwrap();
        let parsed = parse_advertisement(info.get_properties(), "greedy").unwrap();
        assert_eq!(parsed.pairing_ids.len(), MAX_PAIRING_IDS_PER_PEER);
    }

    #[test]
    fn advertised_pairing_ids_are_capped_and_deduplicated() {
        let mut ids: Vec<String> = (0..MAX_ADVERTISED_PAIRING_IDS * 2)
            .map(|i| format!("id-{i}"))
            .collect();
        ids.push("id-0".to_string());
        let info = build_service_info("Laptop", "Laptop", &ids, 1).unwrap();
        let parsed = parse_advertisement(info.get_properties(), "Laptop").unwrap();
        assert_eq!(parsed.pairing_ids.len(), MAX_ADVERTISED_PAIRING_IDS);
        assert_eq!(parsed.pairing_ids[0], "id-0");
    }

    // -- the security property ------------------------------------------------

    /// The advertisement is public to anyone within radio range. Nothing
    /// derived from the pairing token may appear in it, and the key set is
    /// pinned so a new field cannot be added without a decision.
    #[test]
    fn advertisement_has_no_secret_material() {
        let ids = vec!["pair-one".to_string(), "pair-two".to_string()];
        let info = build_service_info("Laptop", "Laptop", &ids, 47_654).unwrap();

        let wire = txt_bytes(&info);
        let rendered = String::from_utf8(wire.clone()).unwrap();

        for needle in [SECRET, &SECRET[..16], &SECRET.to_uppercase()] {
            assert!(
                !rendered.contains(needle),
                "secret leaked into the TXT record"
            );
            assert!(!info.get_fullname().contains(needle));
            assert!(!info.get_hostname().contains(needle));
        }
        for banned in ["psk", "token", "secret", "key"] {
            assert!(
                !rendered.to_lowercase().contains(banned),
                "advertisement mentions {banned}"
            );
        }

        // Pin the key set: only v, n and p<n> are ever published.
        let mut keys: Vec<String> = info
            .get_properties()
            .iter()
            .map(|p| p.key().to_string())
            .collect();
        keys.sort();
        assert_eq!(keys, vec!["n", "p0", "p1", "v"]);
    }

    // -- expiry ---------------------------------------------------------------

    #[test]
    fn stale_entries_expire() {
        let mut table = PeerTable::new(1_000, MAX_PEERS);
        table.observe(
            "a._copypaste._tcp.local.",
            vec![peer("fresh", 10_000)],
            10_000,
        );
        assert_eq!(table.snapshot(10_500).len(), 1);
        assert!(table.find("fresh", 10_500).is_some());

        // Exactly at the TTL is already stale.
        assert!(table.snapshot(11_000).is_empty());
        assert!(table.find("fresh", 11_000).is_none());
    }

    #[test]
    fn a_clock_that_steps_backwards_expires_rather_than_strands() {
        let mut table = PeerTable::new(1_000, MAX_PEERS);
        table.observe("a._copypaste._tcp.local.", vec![peer("x", 10_000)], 10_000);
        // saturating_sub floors at 0, which is inside the TTL, so the entry
        // survives one backwards step instead of being pinned forever.
        assert_eq!(table.snapshot(0).len(), 1);
        assert!(table.snapshot(-5_000).len() <= 1);
    }

    #[test]
    fn a_departing_peer_is_removed_immediately() {
        let mut table = PeerTable::default();
        let now = now_ms();
        table.observe("gone._copypaste._tcp.local.", vec![peer("x", now)], now);
        table.observe("here._copypaste._tcp.local.", vec![peer("y", now)], now);
        table.remove_service("gone._copypaste._tcp.local.");
        let remaining = table.snapshot(now);
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].pairing_id, "y");
    }

    #[test]
    fn one_device_may_hold_several_pairings() {
        let mut table = PeerTable::default();
        let now = now_ms();
        table.observe(
            "laptop._copypaste._tcp.local.",
            vec![peer("a", now), peer("b", now)],
            now,
        );
        assert_eq!(table.snapshot(now).len(), 2);
        assert_eq!(table.find("a", now).unwrap().pairing_id, "a");
    }

    #[test]
    fn find_prefers_the_most_recently_seen_claimant() {
        let mut table = PeerTable::default();
        let now = now_ms();
        table.observe(
            "old._copypaste._tcp.local.",
            vec![peer("dup", now - 5_000)],
            now,
        );
        table.observe("new._copypaste._tcp.local.", vec![peer("dup", now)], now);
        assert_eq!(table.find("dup", now).unwrap().last_seen_ms, now);
    }

    // -- cap ------------------------------------------------------------------

    #[test]
    fn the_peer_table_is_bounded() {
        let mut table = PeerTable::new(600_000, 8);
        for i in 0..500 {
            table.observe(
                &format!("flood-{i}._copypaste._tcp.local."),
                vec![peer(&format!("id-{i}"), 1_000 + i as i64)],
                1_000 + i as i64,
            );
            assert!(table.entries.len() <= 8);
        }
        let peers = table.snapshot(1_500);
        assert_eq!(peers.len(), 8);
    }

    #[test]
    fn flooding_evicts_the_flood_before_a_live_peer() {
        let mut table = PeerTable::new(600_000, 4);
        let mut clock = 1_000i64;
        table.observe(
            "real._copypaste._tcp.local.",
            vec![peer("real", clock)],
            clock,
        );

        for i in 0..100 {
            clock += 10;
            // The real peer keeps refreshing, as a live device does.
            table.observe(
                "real._copypaste._tcp.local.",
                vec![peer("real", clock)],
                clock,
            );
            table.observe(
                &format!("flood-{i}._copypaste._tcp.local."),
                vec![peer(&format!("junk-{i}"), clock)],
                clock,
            );
        }

        assert!(table.entries.len() <= 4);
        assert!(
            table.find("real", clock).is_some(),
            "a refreshing peer must survive a flood"
        );
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

    // -- helpers --------------------------------------------------------------

    #[test]
    fn address_choice_prefers_a_routable_ipv4() {
        let loopback = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let routable = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 4));
        let v6: IpAddr = "fe80::1".parse().unwrap();
        assert_eq!(best_addr(&[loopback, routable, v6]), Some(routable));
        assert_eq!(best_addr(&[loopback, v6]), Some(v6));
        assert_eq!(best_addr(&[]), None);
    }

    #[test]
    fn errors_disclose_no_paths() {
        for e in [
            DiscoveryError::InvalidDeviceName,
            DiscoveryError::InvalidPairingId,
            DiscoveryError::Mdns("socket bind refused".to_string()),
        ] {
            let text = e.to_string();
            assert!(!text.contains('/'), "error discloses a path: {text}");
            assert!(!text.contains('\\'), "error discloses a path: {text}");
        }
    }

    #[test]
    fn names_are_sanitised_for_dns() {
        assert_eq!(
            sanitise_host_label("Dmitriy's Laptop"),
            Some("dmitriy-s-laptop".to_string())
        );
        assert_eq!(sanitise_host_label("***"), None);
        assert_eq!(sanitise_instance("a.b"), Some("a-b".to_string()));
        assert_eq!(sanitise_instance("  "), None);
        assert_eq!(
            sanitise_display_name("  spaced  "),
            Some("spaced".to_string())
        );
        assert!(sanitise_instance(&"x".repeat(200)).unwrap().len() <= MAX_LABEL_LEN);
        assert!(sanitise_display_name(&"é".repeat(200)).unwrap().len() <= MAX_NAME_LEN);
    }
}
