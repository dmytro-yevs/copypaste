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
        let woke = tokio::select! {
            biased;
            _ = shutdown.changed() => return,
            _ = runtime.peer_sync.wake.notified() => true,
            _ = tokio::time::sleep(Duration::from_secs(10)) => false,
        };
        if !runtime.settings.config().sync_enabled {
            continue;
        }
        // A capture wake must survive a manual round that started before the
        // copy. Timer ticks may skip, but new content queues the next pass.
        let _round = if woke {
            tokio::select! {
                biased;
                _ = shutdown.changed() => return,
                round = runtime.peer_sync.rounds.enter() => round,
            }
        } else {
            let Some(round) = runtime.peer_sync.rounds.try_enter() else {
                continue;
            };
            round
        };
        runtime.node.reconcile_discovery();
        let cancel = runtime.peer_sync.cycle.cancel_token();
        let mut sessions = tokio::task::JoinSet::new();
        for peer in runtime.node.peers().list() {
            let runtime = Arc::clone(&runtime);
            let cycle = runtime.peer_sync.cycle.clone();
            sessions.spawn(async move {
                runtime
                    .node
                    .sync_one_in_cycle(&peer, runtime.source.as_ref(), &cycle)
                    .await
            });
        }
        while !sessions.is_empty() {
            let result = tokio::select! {
                biased;
                _ = shutdown.changed() => return,
                _ = cancel.cancelled() => break,
                _ = runtime.peer_sync.wake.notified() => {
                    runtime.peer_sync.wake();
                    break;
                }
                result = sessions.join_next() => result,
            };
            if let Some(Ok(Ok(outcome))) = result {
                runtime.remember_device(&outcome);
                if outcome.stats.received > 0 {
                    runtime.emit(copypaste_ipc::EventKind::Items);
                }
            }
        }
    }
}
