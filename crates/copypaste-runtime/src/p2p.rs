//! The shared peer node plus the outgoing scheduler's cadence and wake.
use std::sync::Arc;

use copypaste_core::sync::RoundGuard;
use copypaste_p2p::node::SyncCycle;
use copypaste_p2p::peers::PeerStore;
use copypaste_p2p::Node;
use copypaste_sync::cadence::AdaptiveCadence;
use copypaste_sync::peer_driver::PeerSyncDriver;

pub struct P2p {
    node: Arc<Node>,
    pub(crate) driver: Arc<PeerSyncDriver>,
}

impl std::fmt::Debug for P2p {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.node.fmt(f)
    }
}

impl P2p {
    pub(crate) fn new(node: Node) -> Self {
        Self {
            node: Arc::new(node),
            driver: PeerSyncDriver::new(),
        }
    }

    pub fn node(&self) -> &Arc<Node> {
        &self.node
    }

    pub fn peers(&self) -> &PeerStore {
        self.node.peers()
    }

    pub fn listen_addr(&self) -> Option<String> {
        self.node.listen_addr()
    }

    pub fn idle(&self) -> &AdaptiveCadence {
        &self.driver.cadence
    }

    /// The permit for one outbound pass, or `None` when one is running.
    pub fn try_begin_round(&self) -> Option<RoundGuard> {
        self.driver.rounds.try_enter()
    }

    /// The permit for one outbound pass, waiting for any running pass first.
    pub async fn begin_round(&self) -> RoundGuard {
        self.driver.rounds.enter().await
    }

    pub fn round_in_flight(&self) -> bool {
        self.driver.rounds.is_running()
    }

    /// Ask the peer sync loop to run now.
    pub fn wake(&self) {
        self.driver.wake();
    }

    /// Resolves when someone calls [`P2p::wake`]. One permit is stored, so a
    /// wake during a round is not lost.
    pub async fn wake_signal(&self) {
        self.driver.wake_signal().await;
    }

    pub(crate) fn sync_enabled_changed(&self, enabled: bool) {
        self.driver.set_enabled(enabled);
    }

    pub fn sync_cycle(&self) -> SyncCycle {
        self.driver.cycle.clone()
    }
}
