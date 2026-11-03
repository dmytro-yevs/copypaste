//! Runtime effects for the shared outgoing peer scheduler.
use crate::Runtime;
use copypaste_p2p::{node::SyncCycle, Peer};
use copypaste_sync::peer_driver::PeerSyncHost;
use std::sync::Arc;

pub(crate) struct RuntimePeerHost(pub(crate) Arc<Runtime>);

impl PeerSyncHost for RuntimePeerHost {
    fn enabled(&self) -> bool {
        self.0.settings.get().sync_enabled
    }
    fn peers(&self) -> Vec<Peer> {
        self.0.p2p.peers().list()
    }
    fn can_dial(&self, peer: &Peer) -> bool {
        !self.0.p2p.node().dial_candidates(peer).is_empty()
    }
    fn reconcile_discovery(&self) {
        self.0.p2p.node().reconcile_discovery();
    }
    async fn sync_peer(&self, peer: Peer, cycle: SyncCycle) -> u64 {
        let result = self.0.sync_one(&peer, &cycle).await;
        if result.received > 0 {
            self.0.note_remote_change();
        }
        u64::from(result.sent) + u64::from(result.received)
    }
}
