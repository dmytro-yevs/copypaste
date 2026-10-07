//! Authenticated, content-free reachability probes for trusted peers.
//!
//! Discovery says an mDNS record was recently observed. A probe says a holder
//! of this pairing's PSK completed a Noise handshake and echoed a nonce over
//! the encrypted application channel. The latter is the only input to an
//! authenticated latency value.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rand::random;
use tokio::time::timeout;

use super::channel::NoiseChannel;
use super::{DialCandidateSource, Node, NodeError};
use crate::peers::Peer;
use crate::protocol::{SyncMessage, PROTOCOL_VERSION};
use crate::sync::SyncChannel;
use crate::transport::Session;
use crate::DeviceProfile;

/// A successful probe remains current only briefly. This avoids presenting an
/// old successful connection as a live one while keeping ordinary UI polling
/// from opening a connection for every request.
pub const PROBE_FRESH_MS: i64 = 30_000;

/// A failure is also rate-limited, but for less time so a device returning to
/// the LAN is noticed promptly.
pub const PROBE_FAILURE_FRESH_MS: i64 = 10_000;

/// A probe includes the Noise handshake and one encrypted request/response.
/// It must not monopolize a probe slot behind a sleeping device.
const PROBE_TIMEOUT_SECS: u64 = 5;
const PROBE_TIMEOUT: Duration = Duration::from_secs(PROBE_TIMEOUT_SECS);
// At most one discovery address and one persisted address are attempted.
const PROBE_REFRESH_LEAD_MS: i64 = PROBE_TIMEOUT_SECS as i64 * 2 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeState {
    Online,
    Offline,
    Unknown,
}

/// An observation made only after a successful Noise handshake for one
/// pairing. `latency_ms` is populated exclusively for `Online`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedReachability {
    pub state: ProbeState,
    pub latency_ms: Option<u64>,
    pub observed_at_ms: i64,
    pub fresh_until_ms: Option<i64>,
}

impl AuthenticatedReachability {
    #[must_use]
    pub fn online(latency_ms: Option<u64>, observed_at_ms: i64) -> Self {
        Self {
            state: ProbeState::Online,
            latency_ms,
            observed_at_ms,
            fresh_until_ms: Some(observed_at_ms.saturating_add(PROBE_FRESH_MS)),
        }
    }

    #[must_use]
    pub fn offline(observed_at_ms: i64) -> Self {
        Self {
            state: ProbeState::Offline,
            latency_ms: None,
            observed_at_ms,
            fresh_until_ms: Some(observed_at_ms.saturating_add(PROBE_FAILURE_FRESH_MS)),
        }
    }

    /// Returns `Unknown` rather than an expired positive or negative value.
    #[must_use]
    pub fn state_at(&self, now_ms: i64) -> ProbeState {
        if self.observed_at_ms > now_ms
            || !matches!(self.fresh_until_ms, Some(fresh_until_ms) if now_ms <= fresh_until_ms)
        {
            ProbeState::Unknown
        } else {
            self.state
        }
    }

    #[must_use]
    pub fn is_current_at(&self, now_ms: i64) -> bool {
        self.state_at(now_ms) != ProbeState::Unknown
    }

    fn should_refresh_at(&self, now_ms: i64) -> bool {
        let refresh_lead_ms = match self.state {
            ProbeState::Online => PROBE_REFRESH_LEAD_MS,
            ProbeState::Offline | ProbeState::Unknown => 0,
        };
        (self.state == ProbeState::Online && self.latency_ms.is_none())
            || !self.is_current_at(now_ms)
            || matches!(
                self.fresh_until_ms,
                Some(fresh_until_ms)
                    if fresh_until_ms.saturating_sub(now_ms) <= refresh_lead_ms
            )
    }
}

impl Node {
    /// Maintains RTT independently of UI reads for the lifetime of the listener.
    /// Existing freshness and in-flight guards coalesce every renewal batch.
    pub(super) async fn monitor_reachability<F>(
        self: Arc<Self>,
        on_complete: F,
        mut shutdown: tokio::sync::watch::Receiver<bool>,
    ) where
        F: Fn() + Send + Sync + Clone + 'static,
    {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            if *shutdown.borrow() {
                return;
            }
            tokio::select! {
                biased;
                _ = shutdown.changed() => return,
                _ = tick.tick() => {
                    let notify = on_complete.clone();
                    let completion_shutdown = shutdown.clone();
                    self.refresh_reachability(self.peers().list(), move || {
                        if !*completion_shutdown.borrow() {
                            notify();
                        }
                    });
                }
            }
        }
    }

    /// Starts at most one content-free probe for each stale or nearly stale
    /// trusted peer.
    ///
    /// This intentionally returns immediately: a `Peers` read must never wait
    /// behind several sleeping devices. `on_complete` runs once after the
    /// whole newly-started batch records its observations, so hosts can publish
    /// one peers event rather than polling or emitting per device. Current
    /// discovery is preferred over the last authenticated address; discovery
    /// remains only a hint because every candidate must pass Noise.
    ///
    /// Returns false when every peer remains outside the renewal window or has
    /// an in-flight probe, in which case `on_complete` is not called.
    pub fn refresh_reachability<F>(
        self: &Arc<Self>,
        peers: impl IntoIterator<Item = Peer>,
        on_complete: F,
    ) -> bool
    where
        F: FnOnce() + Send + 'static,
    {
        let now = crate::now_ms();
        let claimed: Vec<_> = peers
            .into_iter()
            .filter(|peer| {
                !self.dial_candidates(peer).is_empty() && self.claim_probe(&peer.pairing_id, now)
            })
            .collect();
        if claimed.is_empty() {
            return false;
        }
        let remaining = Arc::new(AtomicUsize::new(claimed.len()));
        let completion = Arc::new(Mutex::new(Some(on_complete)));
        for peer in claimed {
            let node = Arc::clone(self);
            let remaining = Arc::clone(&remaining);
            let completion = Arc::clone(&completion);
            tokio::spawn(async move {
                let _permit = node
                    .probes
                    .clone()
                    .acquire_owned()
                    .await
                    .expect("probe semaphore is never closed");
                let _ = node.probe_claimed(&peer).await;
                node.release_probe(&peer.pairing_id);
                if remaining.fetch_sub(1, Ordering::AcqRel) == 1 {
                    if let Some(callback) = completion
                        .lock()
                        .unwrap_or_else(|poisoned| poisoned.into_inner())
                        .take()
                    {
                        callback();
                    }
                }
            });
        }
        true
    }

    /// Probes one known trusted endpoint. This is public for explicit callers;
    /// the system monitor uses [`Self::refresh_reachability`] for coalescing.
    pub async fn probe_one(&self, peer: &Peer) -> Result<AuthenticatedReachability, NodeError> {
        self.probe(peer).await
    }

    fn claim_probe(&self, pairing_id: &str, now_ms: i64) -> bool {
        if self
            .authenticated_reachability(pairing_id)
            .is_some_and(|observation| !observation.should_refresh_at(now_ms))
        {
            return false;
        }
        self.probe_flights
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(pairing_id.to_string())
    }

    fn release_probe(&self, pairing_id: &str) {
        self.probe_flights
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(pairing_id);
    }

    async fn probe_claimed(&self, peer: &Peer) -> Result<AuthenticatedReachability, NodeError> {
        self.probe(peer).await
    }

    async fn probe(&self, peer: &Peer) -> Result<AuthenticatedReachability, NodeError> {
        let candidates = self.dial_candidates_with_sources(peer, true);
        if candidates.is_empty() {
            return Err(NodeError::NoAddress);
        }
        let mut last_error = NodeError::Handshake;
        for candidate in candidates {
            let result = timeout(PROBE_TIMEOUT, probe_session(candidate.addr, &peer.psk)).await;
            if let Ok(Ok((latency_ms, profile))) = result {
                let observation =
                    AuthenticatedReachability::online(Some(latency_ms), crate::now_ms());
                self.record_reachability(&peer.pairing_id, observation.clone());
                self.record_authenticated_profile(&peer.pairing_id, profile.as_ref());
                self.touch_peer(
                    peer,
                    None,
                    Some(super::ListenerEndpoint::dialled(candidate.addr)),
                    None,
                );
                return Ok(observation);
            }
            if candidate.source == DialCandidateSource::Discovery {
                self.record_discovery_candidate_failure(&peer.pairing_id, candidate.addr);
            }
            last_error = match result {
                Ok(Err(error)) => error,
                Err(_) => NodeError::Timeout,
                Ok(Ok(_)) => unreachable!("successful probe returned above"),
            };
        }
        self.record_reachability(
            &peer.pairing_id,
            AuthenticatedReachability::offline(crate::now_ms()),
        );
        Err(last_error)
    }
}

/// Responds to an already decoded authenticated probe. No sync source is
/// accepted here, which makes transferring clipboard data impossible.
pub(super) async fn respond(
    channel: &mut NoiseChannel,
    protocol_version: u32,
    nonce: u64,
) -> Result<(), NodeError> {
    if protocol_version != PROTOCOL_VERSION {
        return Err(NodeError::PeerVersion);
    }
    channel
        .send(SyncMessage::ProbeAck {
            protocol_version: PROTOCOL_VERSION,
            nonce,
            profile: Some(crate::DeviceProfile::current()),
        })
        .await
        .map_err(|_| NodeError::Session)
}

async fn probe_session(
    addr: std::net::SocketAddr,
    psk: &[u8; crate::transport::TOKEN_LEN],
) -> Result<(u64, Option<crate::DeviceProfile>), NodeError> {
    let session = Session::connect(addr, psk)
        .await
        .map_err(|_| NodeError::Handshake)?;
    let mut channel = NoiseChannel::new(session);
    let nonce = random::<u64>();
    let profile = DeviceProfile::current();
    let started = Instant::now();
    let result = async {
        channel
            .send(SyncMessage::Probe {
                protocol_version: PROTOCOL_VERSION,
                nonce,
                profile: Some(profile),
            })
            .await
            .map_err(|_| NodeError::Session)?;
        match channel.recv().await.map_err(|_| NodeError::Session)? {
            SyncMessage::ProbeAck {
                protocol_version,
                nonce: received,
                profile,
            } if protocol_version == PROTOCOL_VERSION && received == nonce => Ok((
                u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
                profile,
            )),
            SyncMessage::ProbeAck { .. } => Err(NodeError::Session),
            _ => Err(NodeError::Session),
        }
    }
    .await;
    channel.close().await;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::discovery::{DiscoveredPeer, Discovery};
    use crate::node::listen;
    use crate::peers::PeerStore;
    use crate::sync::testutil::TestSource;
    use crate::transport::PairingToken;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::net::TcpListener;
    use tokio::sync::{oneshot, watch};

    fn node(dir: &tempfile::TempDir) -> Arc<Node> {
        Arc::new(Node::new(
            PeerStore::open(&dir.path().join("peers.json"), &crate::peers::testutil::KEY).unwrap(),
            None,
            0,
            true,
        ))
    }

    fn peer(token: &PairingToken, addr: Option<std::net::SocketAddr>) -> Peer {
        Peer {
            pairing_id: token.pairing_id(),
            device_id: None,
            name: "trusted peer".into(),
            psk: token.psk(),
            last_addr: addr,
            last_seen_ms: 0,
            profile: None,
            profile_observed_at_ms: 0,
        }
    }

    #[test]
    fn incoming_presence_does_not_erase_or_extend_a_local_rtt() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let now = crate::now_ms();
        let measured = AuthenticatedReachability::online(Some(24), now);
        node.record_reachability("peer", measured.clone());
        node.record_reachability("peer", AuthenticatedReachability::online(None, now + 1));
        assert_eq!(node.authenticated_reachability("peer"), Some(measured));
        let unmeasured = AuthenticatedReachability::online(None, now + PROBE_FRESH_MS + 1);
        node.record_reachability("peer", unmeasured.clone());
        assert_eq!(
            node.authenticated_reachability("peer"),
            Some(unmeasured.clone())
        );
        assert!(unmeasured.should_refresh_at(unmeasured.observed_at_ms));
        node.record_reachability(
            "peer",
            AuthenticatedReachability::offline(now + PROBE_FRESH_MS + 2),
        );
        assert_eq!(
            node.authenticated_reachability("peer").unwrap().state,
            ProbeState::Offline
        );
    }

    #[tokio::test]
    async fn the_monitor_measures_and_renews_without_peer_reads_or_content_sync() {
        let server_dir = tempfile::tempdir().unwrap();
        let client_dir = tempfile::tempdir().unwrap();
        let token = PairingToken::generate();
        let server = node(&server_dir);
        server.peers().upsert(peer(&token, None)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let source = Arc::new(TestSource::new("server", Vec::new()));
        let listener_task = tokio::spawn(listen(
            server,
            listener,
            Arc::clone(&source),
            |_, _| {},
            || {},
            shutdown_rx.clone(),
        ));
        let client = node(&client_dir);
        let trusted = peer(&token, Some(address));
        client.peers().upsert(trusted.clone()).unwrap();
        let (complete_tx, mut complete_rx) = tokio::sync::mpsc::unbounded_channel();
        let monitor = tokio::spawn(Arc::clone(&client).monitor_reachability(
            move || {
                complete_tx.send(()).unwrap();
            },
            shutdown_rx,
        ));
        tokio::time::timeout(Duration::from_secs(1), complete_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let first = client
            .authenticated_reachability(&trusted.pairing_id)
            .unwrap();
        assert!(first.latency_ms.is_some());
        assert!(first.is_current_at(crate::now_ms()));
        assert!(!client.refresh_reachability([trusted.clone()], || panic!(
            "fresh RTT must not be probed twice"
        )));

        client.record_reachability(
            &trusted.pairing_id,
            AuthenticatedReachability::online(first.latency_ms, crate::now_ms() - PROBE_FRESH_MS),
        );
        tokio::time::timeout(Duration::from_secs(2), complete_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let renewed = client
            .authenticated_reachability(&trusted.pairing_id)
            .unwrap();
        assert!(renewed.is_current_at(crate::now_ms()));
        assert!(renewed.latency_ms.is_some());
        shutdown_tx.send(true).unwrap();
        monitor.await.unwrap();
        listener_task.await.unwrap();
    }

    #[test]
    fn observations_fail_closed_after_their_freshness_window() {
        let online = AuthenticatedReachability::online(Some(12), 100);
        assert_eq!(online.state_at(100), ProbeState::Online);
        assert!(!online.should_refresh_at(100));
        assert!(online.should_refresh_at(100 + PROBE_FRESH_MS - PROBE_REFRESH_LEAD_MS));
        assert_eq!(
            online.state_at(100 + PROBE_FRESH_MS + 1),
            ProbeState::Unknown
        );

        let offline = AuthenticatedReachability::offline(100);
        assert_eq!(offline.state_at(100), ProbeState::Offline);
        assert!(!offline.should_refresh_at(100));
        assert!(offline.should_refresh_at(100 + PROBE_FAILURE_FRESH_MS));
        assert_eq!(
            offline.state_at(100 + PROBE_FAILURE_FRESH_MS + 1),
            ProbeState::Unknown
        );
    }

    #[test]
    fn probe_debug_output_has_no_pairing_secret() {
        let observation = AuthenticatedReachability::online(Some(12), 100);
        let rendered = format!("{observation:?}");
        assert!(!rendered.contains("psk"), "{rendered}");
        assert!(!rendered.contains("token"), "{rendered}");
    }

    #[test]
    fn probe_wire_carries_no_pairing_secret() {
        let token = PairingToken::generate();
        let secret = hex::encode(token.psk());
        let probe = SyncMessage::Probe {
            protocol_version: PROTOCOL_VERSION,
            nonce: 7,
            profile: None,
        };
        let encoded = String::from_utf8(probe.encode().unwrap()).unwrap();
        assert!(!encoded.contains(&secret), "{encoded}");
        assert!(!format!("{probe:?}").contains(&secret));
    }

    #[tokio::test]
    async fn a_probe_measures_an_authenticated_round_trip_without_syncing_content() {
        let server_dir = tempfile::tempdir().unwrap();
        let client_dir = tempfile::tempdir().unwrap();
        let server = node(&server_dir);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let token = PairingToken::generate();
        let trusted = peer(&token, Some(addr));
        server.peers().upsert(peer(&token, None)).unwrap();
        let source = Arc::new(TestSource::new("server", Vec::new()));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let probe_notifications = Arc::new(AtomicUsize::new(0));
        let probe_notifications_callback = Arc::clone(&probe_notifications);
        let listener_task = tokio::spawn(listen(
            Arc::clone(&server),
            listener,
            Arc::clone(&source),
            |_, _| {},
            move || {
                probe_notifications_callback.fetch_add(1, Ordering::Relaxed);
            },
            shutdown_rx,
        ));

        let client = node(&client_dir);
        client.peers().upsert(trusted.clone()).unwrap();
        let observation = client.probe_one(&trusted).await.unwrap();
        assert_eq!(observation.state, ProbeState::Online);
        assert!(observation.latency_ms.is_some());
        assert!(source.snapshot().is_empty(), "a probe must not enter sync");
        tokio::time::timeout(Duration::from_secs(1), async {
            while probe_notifications.load(Ordering::Relaxed) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("an inbound probe invalidates the peer view");
        assert_eq!(probe_notifications.load(Ordering::Relaxed), 1);
        assert_eq!(
            server
                .authenticated_reachability(&trusted.pairing_id)
                .unwrap()
                .state,
            ProbeState::Online
        );
        assert!(
            client
                .peers()
                .get(&trusted.pairing_id)
                .and_then(|peer| peer.profile.clone())
                .is_some(),
            "a nonce-matched authenticated probe persists the responder profile"
        );
        assert!(
            server
                .peers()
                .get(&trusted.pairing_id)
                .and_then(|peer| peer.profile.clone())
                .is_some(),
            "an authenticated inbound probe persists the initiator profile"
        );

        shutdown_tx.send(true).unwrap();
        listener_task.await.unwrap();
    }

    #[tokio::test]
    async fn a_refused_known_endpoint_is_a_fresh_offline_observation() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let closed = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = closed.local_addr().unwrap();
        drop(closed);
        let token = PairingToken::generate();
        let trusted = peer(&token, Some(addr));

        assert_eq!(node.probe_one(&trusted).await, Err(NodeError::Handshake));
        let observation = node
            .authenticated_reachability(&trusted.pairing_id)
            .unwrap();
        assert_eq!(
            observation.state_at(observation.observed_at_ms),
            ProbeState::Offline
        );
        assert_eq!(observation.latency_ms, None);
    }

    #[tokio::test]
    async fn a_failed_unverified_discovery_candidate_cools_down_while_persisted_fallback_runs() {
        let server_dir = tempfile::tempdir().unwrap();
        let client_dir = tempfile::tempdir().unwrap();
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        let server = node(&server_dir);
        server.peers().upsert(peer(&token, None)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let authenticated_addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let server_task = tokio::spawn(listen(
            Arc::clone(&server),
            listener,
            Arc::new(TestSource::new("server", Vec::new())),
            |_, _| {},
            || {},
            shutdown_rx,
        ));

        let spoof = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let spoof_addr = spoof.local_addr().unwrap();
        let spoof_attempts = Arc::new(AtomicUsize::new(0));
        let spoof_attempts_task = Arc::clone(&spoof_attempts);
        let wrong_psk = PairingToken::generate().psk();
        let spoof_task = tokio::spawn(async move {
            while let Ok((stream, _)) = spoof.accept().await {
                spoof_attempts_task.fetch_add(1, Ordering::Relaxed);
                let psk = wrong_psk;
                tokio::spawn(async move {
                    let _ = Session::accept(stream, &psk).await;
                });
            }
        });

        let client = Arc::new(Node::new(
            PeerStore::open(
                &client_dir.path().join("peers.json"),
                &crate::peers::testutil::KEY,
            )
            .unwrap(),
            Some(Discovery::dormant("client", 0).unwrap()),
            0,
            true,
        ));
        let trusted = peer(&token, Some(authenticated_addr));
        client.peers().upsert(trusted.clone()).unwrap();
        client
            .discovery()
            .unwrap()
            .observe_for_test(DiscoveredPeer {
                discovery_id: "spoofed-peer".into(),
                pairing_ids: vec![pairing_id],
                name: "spoofed".into(),
                profile: None,
                addr: spoof_addr,
                last_seen_ms: crate::now_ms(),
            });

        client.probe_one(&trusted).await.unwrap();
        client.probe_one(&trusted).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while spoof_attempts.load(Ordering::Relaxed) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            spoof_attempts.load(Ordering::Relaxed),
            1,
            "the cooled discovery endpoint is not retried while the authenticated fallback remains usable"
        );

        client.clear_discovery_candidate_cooldowns();
        client.probe_one(&trusted).await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), async {
            while spoof_attempts.load(Ordering::Relaxed) < 2 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("rescan budget reset allows one new discovery attempt");

        shutdown_tx.send(true).unwrap();
        server_task.await.unwrap();
        spoof_task.abort();
    }

    #[test]
    fn unverified_discovery_attempts_are_globally_bounded_and_rescan_reenables_one() {
        let dir = tempfile::tempdir().unwrap();
        let node = Node::new(
            PeerStore::open(&dir.path().join("peers.json"), &crate::peers::testutil::KEY).unwrap(),
            Some(Discovery::dormant("client", 0).unwrap()),
            0,
            true,
        );
        let mut peers = Vec::new();
        for index in 0..=4 {
            let pairing_id = format!("peer-{index}");
            let peer = Peer {
                pairing_id: pairing_id.clone(),
                device_id: None,
                name: "spoofed".into(),
                psk: [(index as u8).saturating_add(1); crate::transport::TOKEN_LEN],
                last_addr: None,
                last_seen_ms: 0,
                profile: None,
                profile_observed_at_ms: 0,
            };
            node.discovery().unwrap().observe_for_test(DiscoveredPeer {
                discovery_id: format!("spoof-{index}"),
                pairing_ids: vec![pairing_id],
                name: "spoofed".into(),
                profile: None,
                addr: format!("127.0.0.1:{}", 40_000 + index).parse().unwrap(),
                last_seen_ms: crate::now_ms(),
            });
            peers.push(peer);
        }

        for peer in peers.iter().take(4) {
            assert_eq!(node.dial_candidates_with_sources(peer, true).len(), 1);
        }
        let capped = peers.last().unwrap();
        assert!(node.dial_candidates_with_sources(capped, true).is_empty());
        node.clear_discovery_candidate_cooldowns();
        assert_eq!(node.dial_candidates_with_sources(capped, true).len(), 1);
    }

    #[tokio::test]
    async fn a_probe_prefers_a_current_discovery_address_after_dhcp_changes() {
        let server_dir = tempfile::tempdir().unwrap();
        let client_dir = tempfile::tempdir().unwrap();
        let token = PairingToken::generate();
        let pairing_id = token.pairing_id();
        let stale_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let stale_addr = stale_listener.local_addr().unwrap();
        drop(stale_listener);

        let server = node(&server_dir);
        server.peers().upsert(peer(&token, None)).unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let current_addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let listener_task = tokio::spawn(listen(
            Arc::clone(&server),
            listener,
            Arc::new(TestSource::new("server", Vec::new())),
            |_, _| {},
            || {},
            shutdown_rx,
        ));

        let client = Arc::new(Node::new(
            PeerStore::open(
                &client_dir.path().join("peers.json"),
                &crate::peers::testutil::KEY,
            )
            .unwrap(),
            Some(Discovery::dormant("client", 0).unwrap()),
            0,
            true,
        ));
        let trusted = peer(&token, Some(stale_addr));
        client.peers().upsert(trusted.clone()).unwrap();
        client
            .discovery()
            .unwrap()
            .observe_for_test(DiscoveredPeer {
                discovery_id: "server-new-address".into(),
                pairing_ids: vec![pairing_id.clone()],
                name: "server".into(),
                profile: None,
                addr: current_addr,
                last_seen_ms: crate::now_ms(),
            });

        let observation = client.probe_one(&trusted).await.unwrap();
        assert_eq!(observation.state, ProbeState::Online);
        assert_eq!(
            client.peers().get(&pairing_id).unwrap().last_addr,
            Some(current_addr)
        );

        shutdown_tx.send(true).unwrap();
        listener_task.await.unwrap();
    }

    #[tokio::test]
    async fn an_unaddressed_peer_remains_unknown_without_a_probe_attempt() {
        let dir = tempfile::tempdir().unwrap();
        let node = node(&dir);
        let token = PairingToken::generate();
        let trusted = peer(&token, None);

        assert_eq!(node.probe_one(&trusted).await, Err(NodeError::NoAddress));
        assert!(node
            .authenticated_reachability(&trusted.pairing_id)
            .is_none());
    }

    #[tokio::test]
    async fn a_refresh_batch_coalesces_and_notifies_once_after_recording() {
        let server_dir = tempfile::tempdir().unwrap();
        let client_dir = tempfile::tempdir().unwrap();
        let server = node(&server_dir);
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let token = PairingToken::generate();
        let trusted = peer(&token, Some(addr));
        server.peers().upsert(peer(&token, None)).unwrap();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let listener_task = tokio::spawn(listen(
            Arc::clone(&server),
            listener,
            Arc::new(TestSource::new("server", Vec::new())),
            |_, _| {},
            || {},
            shutdown_rx,
        ));
        let client = node(&client_dir);
        let (complete_tx, complete_rx) = oneshot::channel();
        assert!(client.refresh_reachability([trusted.clone()], move || {
            let _ = complete_tx.send(());
        }));
        tokio::time::timeout(Duration::from_secs(1), complete_rx)
            .await
            .expect("the completed batch notifies its host")
            .expect("the completion sender survives");
        assert_eq!(
            client
                .authenticated_reachability(&trusted.pairing_id)
                .unwrap()
                .state,
            ProbeState::Online
        );

        let (unexpected_tx, mut unexpected_rx) = oneshot::channel::<()>();
        assert!(!client.refresh_reachability([trusted], move || {
            let _ = unexpected_tx.send(());
        }));
        assert!(unexpected_rx.try_recv().is_err());

        shutdown_tx.send(true).unwrap();
        listener_task.await.unwrap();
    }
}
