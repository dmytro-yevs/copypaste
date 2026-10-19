//! In-process host effects for the shared outgoing peer scheduler.
use crate::Runtime;
use copypaste_p2p::{node::SyncCycle, Peer};
pub(crate) use copypaste_sync::peer_driver::PeerSyncDriver;
use copypaste_sync::peer_driver::PeerSyncHost;
use std::sync::Arc;

struct RuntimePeerHost(Arc<Runtime>);
impl PeerSyncHost for RuntimePeerHost {
    fn enabled(&self) -> bool {
        self.0.settings.config().sync_enabled
    }
    fn peers(&self) -> Vec<Peer> {
        self.0.node.peers().list()
    }
    fn can_dial(&self, peer: &Peer) -> bool {
        !self.0.node.dial_candidates(peer).is_empty()
    }
    fn reconcile_discovery(&self) {
        self.0.node.reconcile_discovery();
    }
    async fn sync_peer(&self, peer: Peer, cycle: SyncCycle) -> u64 {
        match self
            .0
            .node
            .sync_one_in_cycle(&peer, self.0.source.as_ref(), &cycle)
            .await
        {
            Ok(outcome) => {
                self.0.remember_device(&outcome);
                if outcome.stats.received > 0 {
                    self.0.emit(copypaste_ipc::EventKind::Items);
                }
                outcome.stats.sent.saturating_add(outcome.stats.received) as u64
            }
            Err(_) => 0,
        }
    }
}
pub(crate) async fn run(runtime: Arc<Runtime>) {
    let driver = Arc::clone(&runtime.peer_sync);
    let shutdown = runtime.shutdown.subscribe();
    driver
        .run(Arc::new(RuntimePeerHost(runtime)), shutdown)
        .await;
}
