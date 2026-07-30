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
//! optional fields.
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
pub mod meta;
pub mod source;

use std::net::SocketAddr;
use std::sync::Arc;

use copypaste_p2p::discovery::Discovery;
use copypaste_p2p::peers::{Peer, PeerStore};
use copypaste_p2p::sync::run_responder;
use copypaste_p2p::transport::Session;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{watch, Semaphore};
use tracing::{debug, info, warn};

use crate::p2p::channel::{NoiseChannel, SESSION_TIMEOUT};
use crate::p2p::meta::Meta;
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
    /// The sync view of the item table, and this device's identity.
    pub meta: Meta,
    peers: PeerStore,
    /// Degraded-but-present when multicast is unavailable — see the module docs.
    discovery: Discovery,
    /// The port [`listen`] binds, which is what a pairing tells a peer to dial.
    port: u16,
    sessions: Arc<Semaphore>,
}

impl std::fmt::Debug for P2p {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P2p")
            .field("device_id", &self.meta.device_id())
            .field("port", &self.port)
            .field("peers", &self.peers.len())
            .finish_non_exhaustive()
    }
}

impl P2p {
    pub fn new(meta: Meta, peers: PeerStore, discovery: Discovery, port: u16) -> Self {
        Self {
            meta,
            peers,
            discovery,
            port,
            sessions: Arc::new(Semaphore::new(MAX_CONCURRENT_PEER_SESSIONS)),
        }
    }

    pub fn peers(&self) -> &PeerStore {
        &self.peers
    }

    pub fn discovery(&self) -> &Discovery {
        &self.discovery
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
        if let Err(e) = self.discovery.republish(&ids) {
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
    let outcome = tokio::time::timeout(
        SESSION_TIMEOUT,
        run_responder(&mut channel, &source),
    )
    .await;
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
pub mod tests {
    use super::*;
    use crate::clipboard::{Capture, ClipboardSource};
    use copypaste_core::{Detector, Keyring, Store};

    /// Writes nowhere and reads nothing: these tests are about sync, not the
    /// pasteboard.
    #[derive(Default)]
    pub struct FakeClipboard;

    impl ClipboardSource for FakeClipboard {
        fn poll(&mut self) -> Option<Capture> {
            None
        }
        fn set_contents(&mut self, _text: &str) -> anyhow::Result<()> {
            Ok(())
        }
        fn backend_name(&self) -> &'static str {
            "fake"
        }
    }

    /// A fully wired daemon state on a temporary data directory.
    ///
    /// The keyring is deterministic rather than loaded: a test must not touch
    /// the developer's real keystore, and two states built with different names
    /// get different secrets, which is what makes "re-encrypted under the local
    /// key" a meaningful assertion.
    pub fn test_state(name: &str) -> (Arc<AppState>, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let db_path = dir.path().join("copypaste-v2.db");

        let mut secret = [0u8; 32];
        for (slot, byte) in secret.iter_mut().zip(name.bytes().cycle()) {
            *slot = byte;
        }
        let keyring = Keyring::from_secret(&secret);
        let store = Store::open(&db_path, &keyring.db_key()).expect("store");
        let meta = Meta::open(&db_path, &keyring.db_key(), name).expect("meta");
        let peers = PeerStore::open(&dir.path().join("peers-v2.json")).expect("peer store");
        // Port 0 is never bound in these tests; discovery degrades either way.
        let discovery = Discovery::start(name, &[], 0).expect("discovery");

        let state = AppState::new(
            store,
            keyring,
            Detector::new().expect("detector"),
            Box::new(FakeClipboard),
            P2p::new(meta, peers, discovery, 0),
        );
        state.set_ready(true);
        (Arc::new(state), dir)
    }

    #[test]
    fn two_states_have_distinct_device_identities() {
        let (a, _da) = test_state("alpha");
        let (b, _db) = test_state("beta");
        assert_ne!(a.p2p.meta.device_id(), b.p2p.meta.device_id());
        assert_eq!(a.p2p.meta.device_name(), "alpha");
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
