//! Peer-to-peer sync, wired into the daemon.
//!
//! `copypaste-p2p` provides the parts — a Noise channel, a last-write-wins
//! session, an mDNS browser — and deliberately knows nothing about a database.
//! This module supplies the four things it asks the daemon for:
//!
//! * [`source::StoreSource`] — the history, as a session sees it,
//! * [`channel::NoiseChannel`] — the session over the encrypted transport,
//!   with the read deadline the sync engine leaves to its caller,
//! * [`listen`] — the inbound half: accept, authenticate against every stored
//!   pairing, respond,
//! * [`handlers`] — the five IPC operations a client drives all of this with.
//!
//! Everything shared sits in [`P2p`], which hangs off `AppState` exactly like
//! the store and the keyring do: constructed once in `main`, never rebuilt, no
//! optional fields. The sync view of the history is *not* here: it is
//! [`crate::meta`], because the cloud transport reads and writes the same rows
//! through the same comparator, and one of the two transports owning it is how
//! the second one ends up with a copy.
//!
//! # Discovery is a convenience, never a dependency
//!
//! `Discovery::start` returns `Ok` on a host with no multicast — that is this
//! container, and it is also a corporate network with mDNS filtered. The peer
//! list stays empty, `online` reads false for everyone, and an explicit address
//! still pairs and still syncs. Nothing below treats a discovery failure as an
//! error.

pub mod channel;
pub mod handlers;
pub mod poll;
pub mod source;

use std::net::SocketAddr;
use std::sync::Arc;

use copypaste_p2p::discovery::{DiscoveredPeer, Discovery};
use copypaste_p2p::peers::{Peer, PeerStore};
use copypaste_p2p::sync::run_responder;
use copypaste_p2p::transport::Session;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Notify, Semaphore};
use tracing::{debug, info, warn};

use crate::cadence::Idle;
use crate::p2p::channel::{NoiseChannel, SESSION_TIMEOUT};
use crate::p2p::source::StoreSource;
use crate::AppState;

/// How many peers may be mid-session at once, inbound.
///
/// A session holds a database connection's worth of work and a decrypted batch
/// in memory, and a device has a handful of peers, not hundreds. Connections
/// past the limit are dropped without a handshake: refusing early tells an
/// unauthenticated dialler nothing it did not already know from the open port.
const MAX_CONCURRENT_PEER_SESSIONS: usize = 4;

/// Everything the peer half of the daemon shares.
///
/// Held inside `AppState`; every field is always present, for the same reason
/// the rest of `AppState` is (see the crate docs).
pub struct P2p {
    peers: PeerStore,
    /// `None` when the user turned LAN visibility off, and
    /// degraded-but-present when multicast is merely unavailable — see the
    /// module docs. The two are different states and only the first is a
    /// decision, so they are not collapsed into one.
    discovery: Option<Discovery>,
    /// The port [`listen`] binds, which is what a pairing tells a peer to dial.
    port: u16,
    sessions: Arc<Semaphore>,
    /// Woken by a local capture and by `copypaste sync`, so neither waits out
    /// the idle interval. Mirrors `Cloud::wake`.
    wake: Notify,
    /// The idle cadence [`poll::run`] waits on.
    idle: Idle,
}

impl std::fmt::Debug for P2p {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P2p")
            .field("port", &self.port)
            .field("peers", &self.peers.len())
            .finish_non_exhaustive()
    }
}

impl P2p {
    pub fn new(peers: PeerStore, discovery: Option<Discovery>, port: u16) -> Self {
        Self {
            peers,
            discovery,
            port,
            sessions: Arc::new(Semaphore::new(MAX_CONCURRENT_PEER_SESSIONS)),
            wake: Notify::new(),
            idle: Idle::default(),
        }
    }

    pub fn peers(&self) -> &PeerStore {
        &self.peers
    }

    pub fn discovery(&self) -> Option<&Discovery> {
        self.discovery.as_ref()
    }

    /// Everything currently visible on the LAN. Empty when discovery is off or
    /// degraded, which is never an error: an explicit address always works.
    pub fn seen(&self) -> Vec<DiscoveredPeer> {
        self.discovery
            .as_ref()
            .map(Discovery::peers)
            .unwrap_or_default()
    }

    /// Whether a given pairing is visible right now.
    pub fn find(&self, pairing_id: &str) -> Option<DiscoveredPeer> {
        self.discovery.as_ref()?.find(pairing_id)
    }

    pub fn idle(&self) -> &Idle {
        &self.idle
    }

    /// Ask the peer sync loop to run now.
    pub fn wake(&self) {
        self.idle.reset();
        self.wake.notify_one();
    }

    /// Resolves when someone calls [`P2p::wake`]. `Notify` stores one permit,
    /// so a wake during a round is not lost.
    pub async fn wake_signal(&self) {
        self.wake.notified().await;
    }

    pub fn port(&self) -> u16 {
        self.port
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
            .map(|peer| peer.pairing_id.clone())
            .collect();
        let Some(discovery) = self.discovery.as_ref() else {
            return;
        };
        if let Err(e) = discovery.republish(&ids) {
            debug!(error = %e, "could not republish the discovery record");
        }
    }

    /// Where a peer should dial this device, when that can be determined.
    ///
    /// Loopback is skipped: a peer on another device cannot use it. When the
    /// host has no routable address the answer is `None` rather than a guess —
    /// the user is told to supply the address themselves.
    pub fn listen_addr(&self) -> Option<String> {
        let addrs = if_addrs::get_if_addrs().ok()?;
        let best = addrs
            .iter()
            .filter(|iface| !iface.is_loopback())
            .map(|iface| iface.ip())
            // IPv4 first: it is what a user can read out over the phone.
            .min_by_key(|ip| u8::from(ip.is_ipv6()))?;
        Some(SocketAddr::new(best, self.port).to_string())
    }

    /// Record that a session with this peer succeeded.
    ///
    /// The name comes off the wire and is cosmetic — never an identity — so it
    /// is only taken when the peer offered one.
    fn touch_peer(&self, peer: &Peer, addr: Option<SocketAddr>, name: Option<&str>) {
        let updated = Peer {
            pairing_id: peer.pairing_id.clone(),
            name: match name {
                Some(name) if !name.trim().is_empty() => name.to_string(),
                _ => peer.name.clone(),
            },
            // `[u8; 32]` is `Copy`, so this reads the field rather than moving
            // out of a type that has a `Drop` (see `peers::Peer`).
            psk: peer.psk,
            last_addr: addr.or(peer.last_addr),
            last_seen_ms: copypaste_core::now_ms(),
        };
        if let Err(e) = self.peers.upsert(updated) {
            warn!(error = %e, "could not record a successful peer session");
        }
    }
}

/// Bind the peer port.
///
/// `0.0.0.0`: the channel refuses anyone without a stored pre-shared key, so an
/// open port discloses only that CopyPaste is running.
pub fn bind(port: u16) -> std::io::Result<std::net::TcpListener> {
    let listener = std::net::TcpListener::bind(("0.0.0.0", port))?;
    listener.set_nonblocking(true)?;
    Ok(listener)
}

/// Accept peer connections until shutdown.
pub async fn listen(
    listener: TcpListener,
    state: Arc<AppState>,
    mut shutdown: watch::Receiver<bool>,
) {
    info!(port = state.p2p.port(), "peer listener started");
    let mut sessions = tokio::task::JoinSet::new();

    loop {
        tokio::select! {
            _ = shutdown.changed() => break,
            accepted = listener.accept() => match accepted {
                Ok((stream, addr)) => {
                    let Ok(permit) = Arc::clone(&state.p2p.sessions).try_acquire_owned() else {
                        debug!(%addr, "too many peer sessions in flight; dropping the connection");
                        continue;
                    };
                    let state = Arc::clone(&state);
                    sessions.spawn(async move {
                        let _permit = permit;
                        serve_peer(state, stream, addr).await;
                    });
                }
                Err(e) => warn!(error = %e, "could not accept a peer connection"),
            },
        }
    }

    sessions.shutdown().await;
    info!("peer listener stopped");
}

/// One inbound peer: authenticate against every stored pairing, then respond.
///
/// An unknown pre-shared key fails the handshake and the connection is dropped
/// without a reply. That is deliberate and it is the whole authentication
/// story: `accept_any` reports nothing about which pairings this device holds,
/// and neither does this function's logging.
async fn serve_peer(state: Arc<AppState>, stream: TcpStream, addr: SocketAddr) {
    let candidates = state.p2p.peers().psks();
    if candidates.is_empty() {
        debug!(%addr, "a peer connected but this device has no pairings");
        return;
    }

    let (session, pairing_id) = match Session::accept_any(stream, &candidates).await {
        Ok(accepted) => accepted,
        Err(e) => {
            debug!(%addr, error = %e, "inbound peer handshake failed");
            return;
        }
    };

    let mut channel = NoiseChannel::new(session);
    let source = StoreSource::new(Arc::clone(&state));
    let outcome = tokio::time::timeout(SESSION_TIMEOUT, run_responder(&mut channel, &source)).await;
    channel.close().await;

    match outcome {
        Ok(Ok(outcome)) => {
            info!(
                %pairing_id,
                sent = outcome.stats.sent,
                received = outcome.stats.received,
                skipped = outcome.stats.skipped,
                "served a peer sync session"
            );
            if outcome.stats.received > 0 {
                state.note_remote_change();
            }
            if let Some(peer) = state.p2p.peers().get(&pairing_id) {
                // The dialler's source port is not where it listens, so only
                // the name is learned here.
                state
                    .p2p
                    .touch_peer(&peer, None, Some(&outcome.peer_device_name));
            }
        }
        Ok(Err(e)) => warn!(%pairing_id, error = %e, "peer sync session failed"),
        Err(_) => warn!(%pairing_id, "peer sync session timed out"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{add, contents, test_state};
    use copypaste_p2p::peers::Peer;

    /// Pair two states over loopback and hand back the address A listens on.
    ///
    /// Both halves get the same pre-shared key, which is the whole of the
    /// pairing: the initiator dials, the listener recognises the key, and
    /// neither side needs anything else.
    async fn pair(
        a: &Arc<AppState>,
        b: &Arc<AppState>,
        shutdown: watch::Receiver<bool>,
    ) -> (String, SocketAddr) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("addr");
        tokio::spawn(listen(listener, Arc::clone(a), shutdown));

        let token = copypaste_p2p::transport::PairingToken::generate();
        let pairing_id = token.pairing_id();
        a.p2p
            .peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                name: "b".into(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 0,
            })
            .expect("store the pairing on A");
        b.p2p
            .peers()
            .upsert(Peer {
                pairing_id: pairing_id.clone(),
                name: "a".into(),
                psk: token.psk(),
                last_addr: Some(addr),
                last_seen_ms: 0,
            })
            .expect("store the pairing on B");
        (pairing_id, addr)
    }

    /// The whole thing, in process: a listener, a dialler, two databases with
    /// two different device secrets, and the three properties that matter.
    ///
    /// Every assertion runs the instant `sync_now` returns, with no sleep. That
    /// is deliberate and it is a real guarantee: the initiator's last act is a
    /// `Done` the responder has yet to *apply*, so a session that returned as
    /// soon as the bytes left would make this test — and any script that syncs
    /// and then reads — a race. `NoiseChannel::wait_for_close` is what closes
    /// it. This test failed roughly one run in ten before that existed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn two_daemons_converge_without_leaking_a_secret() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (pairing_id, _addr) = pair(&a, &b, shutdown_rx).await;

        add(&a, "from a");
        add(&b, "from b");
        add(&a, "AKIAIOSFODNN7EXAMPLE");

        let response = crate::p2p::handlers::sync_now(&b, 1, Some(&pairing_id)).await;
        assert!(response.ok, "{:?}", response.error);

        let on_a = contents(&a);
        let on_b = contents(&b);
        for item in ["from a", "from b"] {
            assert!(
                on_a.iter().any(|c| c == item),
                "A is missing {item}: {on_a:?}"
            );
            assert!(
                on_b.iter().any(|c| c == item),
                "B is missing {item}: {on_b:?}"
            );
        }
        assert!(
            on_a.iter().any(|c| c == "AKIAIOSFODNN7EXAMPLE"),
            "the secret must stay on the device that captured it"
        );
        assert!(
            !on_b.iter().any(|c| c == "AKIAIOSFODNN7EXAMPLE"),
            "a sensitive item crossed the wire: {on_b:?}"
        );

        // The same item ids on both sides, which is what makes the second
        // session free rather than a second copy of everything.
        let ids_a: std::collections::HashSet<String> = a
            .store
            .list(100, 0)
            .unwrap()
            .into_iter()
            .filter(|row| !row.is_sensitive)
            .map(|row| row.id)
            .collect();
        let ids_b: std::collections::HashSet<String> = b
            .store
            .list(100, 0)
            .unwrap()
            .into_iter()
            .map(|row| row.id)
            .collect();
        assert_eq!(ids_a, ids_b);

        let _ = shutdown_tx.send(true);
    }

    /// A repeated session must move nothing: `plan` skips a tie, and `apply`
    /// re-checks the merge, so replay is a no-op at two layers.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_second_session_transfers_nothing() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (pairing_id, _addr) = pair(&a, &b, shutdown_rx).await;

        add(&a, "from a");
        add(&b, "from b");

        let first = crate::p2p::handlers::sync_now(&b, 1, Some(&pairing_id)).await;
        assert!(first.ok);
        let second = crate::p2p::handlers::sync_now(&b, 2, Some(&pairing_id)).await;

        let results = match second.data {
            Some(copypaste_ipc::ResponseData::Sync(results)) => results,
            other => panic!("{other:?}"),
        };
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].error, None);
        assert_eq!(
            (results[0].sent, results[0].received),
            (0, 0),
            "a replayed session moved data"
        );

        let _ = shutdown_tx.send(true);
    }

    /// The listener must refuse a dialler holding a key it does not know, and
    /// say nothing about which keys it does know.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn an_unknown_pairing_key_cannot_open_a_session() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (_pairing_id, addr) = pair(&a, &b, shutdown_rx).await;
        add(&a, "not yours");

        // A different pairing entirely: well-formed, and unknown to A.
        let stranger = copypaste_p2p::transport::PairingToken::generate();
        b.p2p
            .peers()
            .upsert(Peer {
                pairing_id: stranger.pairing_id(),
                name: "stranger".into(),
                psk: stranger.psk(),
                last_addr: Some(addr),
                last_seen_ms: 0,
            })
            .unwrap();

        let response = crate::p2p::handlers::sync_now(&b, 1, Some(&stranger.pairing_id())).await;
        let results = match response.data {
            Some(copypaste_ipc::ResponseData::Sync(results)) => results,
            other => panic!("{other:?}"),
        };
        assert!(results[0].error.is_some(), "the handshake must fail");
        assert!(
            contents(&b).is_empty(),
            "nothing may be transferred over a refused handshake"
        );

        let _ = shutdown_tx.send(true);
    }

    #[test]
    fn two_states_have_distinct_device_identities() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        assert_ne!(a.meta.device_id(), b.meta.device_id());
        assert_eq!(a.meta.device_name(), "alpha");
    }

    #[test]
    fn a_listen_address_is_never_loopback() {
        let (state, _dir) = test_state("alpha");
        if let Some(addr) = state.p2p.listen_addr() {
            let parsed: SocketAddr = addr.parse().expect("a socket address");
            assert!(!parsed.ip().is_loopback(), "{addr}");
        }
    }
}
