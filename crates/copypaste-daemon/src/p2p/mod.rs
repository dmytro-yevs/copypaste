//! Peer-to-peer sync, wired into the daemon.
//!
//! The node itself is [`copypaste_p2p::Node`] — the peer list, the
//! advertisement, the inbound listener and the four pairing/sync operations. It
//! lives there rather than here because Android has no daemon to put it in, and
//! a second node is how the two would come apart.
//!
//! What this module supplies is what the node asks of *this* device:
//!
//! * [`P2p`] — the node plus the daemon's cadence and wake signal, hanging off
//!   `AppState` exactly like the store and the keyring do,
//! * [`handlers`] — the five IPC operations a client drives all of this with.
//!
//! The sync view of the history is *not* here: it is
//! [`copypaste_core::StoreSource`], because the cloud transport reads and
//! writes the same rows through the same comparator, and one of the two
//! transports owning it is how the second one ends up with a copy.

pub mod handlers;
pub mod poll;

use std::sync::Arc;

use copypaste_p2p::discovery::{DiscoveredPeer, Discovery};
use copypaste_p2p::peers::PeerStore;
use copypaste_p2p::sync::SyncOutcome;
use copypaste_p2p::Node;
use tokio::net::TcpListener;
use tokio::sync::{watch, Notify};
use tracing::warn;

use copypaste_core::sync::{RoundGate, RoundGuard};

use crate::cadence::Idle;
use crate::sync::peer_source;
use crate::AppState;

pub use copypaste_p2p::node::bind;

/// The node, plus what only a long-lived daemon has: a cadence and a wake.
pub struct P2p {
    node: Arc<Node>,
    /// Woken by a local capture and by `copypaste sync`, so neither waits out
    /// the idle interval. Mirrors `Cloud::wake`.
    wake: Notify,
    /// The idle cadence [`poll::run`] waits on.
    idle: Idle,
    /// One outbound pass over the peers at a time. Two passes dial the same
    /// peers at once, and the second one is refused by the far side's session
    /// limit rather than doing anything useful.
    rounds: RoundGate,
}

impl std::fmt::Debug for P2p {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.node.fmt(f)
    }
}

impl P2p {
    pub fn new(
        peers: PeerStore,
        discovery: Option<Discovery>,
        port: u16,
        lan_visible: bool,
    ) -> Self {
        Self {
            node: Arc::new(Node::new(peers, discovery, port, lan_visible)),
            wake: Notify::new(),
            idle: Idle::default(),
            rounds: RoundGate::new(),
        }
    }

    pub fn node(&self) -> &Arc<Node> {
        &self.node
    }

    pub fn peers(&self) -> &PeerStore {
        self.node.peers()
    }

    pub fn discovery(&self) -> Option<&Discovery> {
        self.node.discovery()
    }

    /// Everything currently visible on the LAN. Empty when discovery is off or
    /// degraded, which is never an error: an explicit address always works.
    pub fn seen(&self) -> Vec<DiscoveredPeer> {
        self.node.seen()
    }

    /// Whether a given pairing is visible right now.
    pub fn find(&self, pairing_id: &str) -> Option<DiscoveredPeer> {
        self.node.find(pairing_id)
    }

    pub fn port(&self) -> u16 {
        self.node.port()
    }

    pub fn republish(&self) {
        self.node.republish();
    }

    pub fn listen_addr(&self) -> Option<String> {
        self.node.listen_addr()
    }

    pub fn idle(&self) -> &Idle {
        &self.idle
    }

    /// The permit for one outbound pass, or `None` when one is running.
    pub(crate) fn try_begin_round(&self) -> Option<RoundGuard> {
        self.rounds.try_enter()
    }

    /// The permit for one outbound pass, waiting for any running pass first.
    pub(crate) async fn begin_round(&self) -> RoundGuard {
        self.rounds.enter().await
    }

    #[cfg(test)]
    pub(crate) fn round_in_flight(&self) -> bool {
        self.rounds.is_running()
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
}

/// Accept peer connections until shutdown, serving them from this daemon's
/// history.
pub async fn listen(listener: TcpListener, state: Arc<AppState>, shutdown: watch::Receiver<bool>) {
    let node = Arc::clone(state.p2p.node());
    let source = Arc::new(peer_source(&state));
    let on_session = move |_pairing_id: &str, outcome: &SyncOutcome| {
        remember_device(&state, outcome);
        if outcome.stats.received > 0 {
            state.note_remote_change();
        }
    };
    copypaste_p2p::node::listen(node, listener, source, on_session, shutdown).await;
}

/// Remember what the device on the other end of a session calls itself.
///
/// The origin table has always held the device *id*, because the merge
/// tie-break orders on it. An id is a UUID, so a row that arrived from the
/// phone could be attributed but not named, and `Item::origin_device_name`
/// would be `None` forever. This is the other half.
///
/// Recorded from both sides of a session, because either may be the first to
/// speak to a given device — the responder here, the initiator in
/// [`handlers`].
///
/// Best-effort: a failure costs a label, not an item, and it must not fail a
/// session that has already exchanged its rows.
pub(crate) fn remember_device(state: &AppState, outcome: &SyncOutcome) {
    if let Err(e) = state
        .meta
        .record_device_name(&outcome.peer_device_id, &outcome.peer_device_name)
    {
        warn!(error = ?e, "could not record a peer device name");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{add, contents, test_state};
    use copypaste_p2p::peers::Peer;
    use std::net::SocketAddr;

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
