//! Two devices with no LAN route between them, syncing through an in-process
//! relay: directly over a punched UDP path when the relay runs its observer,
//! spliced through the relay otherwise.

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;
use tokio::sync::watch;

use crate::peers::{Peer, PeerStore};
use crate::sync::testutil::{item, TestSource};
use crate::sync::SyncSource;
use crate::transport::PairingToken;
use crate::{Node, PairingPhase, RelayStatus};

struct Relay {
    url: String,
    addr: std::net::SocketAddr,
    stop: watch::Sender<bool>,
}

impl Relay {
    async fn start(direct_paths: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (stop, stopped) = watch::channel(false);
        let mut config = copypaste_relay::Config::default();
        if direct_paths {
            let socket = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
            config.observe = Some(format!(":{}", socket.local_addr().unwrap().port()));
            tokio::spawn(copypaste_relay::observe(socket, stopped_future(&stopped)));
        }
        tokio::spawn(copypaste_relay::serve(
            listener,
            config,
            stopped_future(&stopped),
        ));
        Self {
            url: format!("ws://{addr}"),
            addr,
            stop,
        }
    }
}

fn stopped_future(stopped: &watch::Receiver<bool>) -> impl std::future::Future<Output = ()> {
    let mut stopped = stopped.clone();
    async move {
        let _ = stopped.changed().await;
    }
}

impl Drop for Relay {
    fn drop(&mut self) {
        let _ = self.stop.send(true);
    }
}

struct Device {
    _dir: tempfile::TempDir,
    node: Arc<Node>,
    source: Arc<TestSource>,
    shutdown: watch::Sender<bool>,
    listener: Option<tokio::task::JoinHandle<()>>,
}

impl Device {
    fn new(id: &str, relay: &Relay) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let node = Arc::new(Node::new(
            PeerStore::open(&dir.path().join("peers.json"), &crate::peers::testutil::KEY).unwrap(),
            None,
            0,
            true,
        ));
        node.relay().configure(Some(&relay.url)).unwrap();
        Self {
            _dir: dir,
            node,
            source: Arc::new(TestSource::new(id, vec![])),
            shutdown: watch::channel(false).0,
            listener: None,
        }
    }

    /// Start the listener, which also keeps this device present on the relay.
    async fn start(&mut self) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        self.listener = Some(tokio::spawn(super::listen(
            Arc::clone(&self.node),
            listener,
            Arc::clone(&self.source),
            |_, _| {},
            |_| {},
            self.shutdown.subscribe(),
        )));
        let mut status = self.node.relay().subscribe_status();
        tokio::time::timeout(
            Duration::from_secs(5),
            status.wait_for(|status| *status == RelayStatus::Connected),
        )
        .await
        .expect("relay presence connects")
        .unwrap();
    }

    fn peer(&self) -> Peer {
        self.node.peers().list().into_iter().next().expect("paired")
    }

    async fn wait_for_relay_route(&self) {
        let peer = self.peer();
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.node.relay().route(&peer.psk).is_none() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("the peer appears on the relay");
    }

    async fn stop(mut self) {
        let _ = self.shutdown.send(true);
        if let Some(listener) = self.listener.take() {
            listener.await.unwrap();
        }
    }
}

#[tokio::test]
async fn paired_devices_without_a_lan_address_sync_both_ways_over_a_punched_path() {
    let relay = Relay::start(true).await;
    let mut a = Device::new("a", &relay);
    let mut b = Device::new("b", &relay);
    a.source.apply(item("from-a", 1, "alpha", "a")).unwrap();
    b.source.apply(item("from-b", 2, "beta", "b")).unwrap();
    let token = PairingToken::generate();
    for (local, remote) in [(&a, &b), (&b, &a)] {
        local
            .node
            .peers()
            .upsert(Peer {
                pairing_id: token.pairing_id(),
                device_id: Some(remote.source.device_id()),
                name: remote.source.device_name(),
                psk: token.psk(),
                last_addr: None,
                last_seen_ms: 0,
                profile: None,
                profile_observed_at_ms: 0,
            })
            .unwrap();
    }
    a.start().await;
    b.start().await;
    a.wait_for_relay_route().await;
    assert!(a.node.dial_candidates(&a.peer()).is_empty(), "no LAN route");
    let direct = a
        .node
        .relay()
        .route(&a.peer().psk)
        .unwrap()
        .open()
        .await
        .unwrap();
    assert_ne!(direct.peer_addr(), relay.addr, "not spliced by the relay");
    drop(direct);

    let outcome = a.node.sync_one(&a.peer(), a.source.as_ref()).await.unwrap();

    assert_eq!((outcome.stats.sent, outcome.stats.received), (1, 1));
    assert!(a.source.get("from-b").is_some());
    assert!(b.source.get("from-a").is_some());
    a.stop().await;
    b.stop().await;
}

#[tokio::test]
async fn a_short_code_pairs_through_the_relay_and_the_pairing_then_syncs() {
    let relay = Relay::start(false).await;
    let mut a = Device::new("a", &relay);
    let mut b = Device::new("b", &relay);
    b.source.apply(item("from-b", 2, "beta", "b")).unwrap();
    a.start().await;
    b.start().await;

    let invite = b.node.pair_create_invite().unwrap();
    // Let the inviter's presence register the invitation.
    tokio::time::sleep(Duration::from_millis(100)).await;
    a.node
        .pair_join(&invite.code, "", a.source.as_ref())
        .await
        .expect("join through the relay");
    tokio::time::timeout(Duration::from_secs(5), async {
        while b.node.pair_progress().phase != PairingPhase::AwaitingConfirmation {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    a.node.pair_confirm(true).unwrap();
    b.node.pair_confirm(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while a.node.pair_progress().phase != PairingPhase::Confirmed
            || b.node.pair_progress().phase != PairingPhase::Confirmed
        {
            assert_ne!(a.node.pair_progress().phase, PairingPhase::Failed);
            assert_ne!(b.node.pair_progress().phase, PairingPhase::Failed);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();

    a.wait_for_relay_route().await;
    a.node.sync_one(&a.peer(), a.source.as_ref()).await.unwrap();
    assert!(a.source.get("from-b").is_some());
    a.stop().await;
    b.stop().await;
}
