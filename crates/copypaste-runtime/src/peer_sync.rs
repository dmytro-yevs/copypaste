//! Automatic outgoing peer sessions for an in-process platform host.

use std::sync::Arc;
use std::time::Duration;

use copypaste_core::sync::RoundGate;
use copypaste_p2p::node::SyncCycle;
use tokio::sync::Notify;

use crate::Runtime;

pub(crate) struct PeerSyncDriver {
    pub(crate) rounds: RoundGate,
    pub(crate) cycle: SyncCycle,
    wake: Notify,
}

impl PeerSyncDriver {
    pub(crate) fn new() -> Arc<Self> {
        Arc::new(Self {
            rounds: RoundGate::new(),
            cycle: SyncCycle::new(),
            wake: Notify::new(),
        })
    }

    pub(crate) fn wake(&self) {
        self.wake.notify_one();
    }

    pub(crate) fn set_enabled(&self, enabled: bool) {
        self.cycle.set_enabled(enabled);
        if enabled {
            self.wake();
        }
    }
}

pub(crate) async fn run(runtime: Arc<Runtime>) {
    let mut shutdown = runtime.shutdown.subscribe();
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => return,
            _ = runtime.peer_sync.wake.notified() => {},
            _ = tokio::time::sleep(Duration::from_secs(10)) => {},
        }
        if !runtime.settings.config().sync_enabled {
            continue;
        }
        let Some(_round) = runtime.peer_sync.rounds.try_enter() else {
            continue;
        };
        runtime.node.reconcile_discovery();
        let cancel = runtime.peer_sync.cycle.cancel_token();
        for peer in runtime.node.peers().list() {
            let result = tokio::select! {
                biased;
                _ = shutdown.changed() => return,
                _ = cancel.cancelled() => break,
                result = runtime.node.sync_one_in_cycle(&peer, runtime.source.as_ref(), &runtime.peer_sync.cycle) => result,
            };
            if let Ok(outcome) = result {
                runtime.remember_device(&outcome);
                if outcome.stats.received > 0 {
                    runtime.emit(copypaste_ipc::EventKind::Items);
                }
            }
        }
    }
}
