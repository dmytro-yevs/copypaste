//! One outgoing peer scheduler for daemon and in-process platform hosts.
use crate::cadence::{AdaptiveCadence, MAX_POLL_INTERVAL_WITHOUT_PUSH, MIN_POLL_INTERVAL};
use copypaste_core::sync::{RoundGate, RoundGuard};
use copypaste_p2p::{node::SyncCycle, Peer};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::sync::{watch, Notify};

pub const NO_PEERS_INTERVAL: Duration = Duration::from_secs(60);

pub trait PeerSyncHost: Send + Sync + 'static {
    fn enabled(&self) -> bool;
    fn peers(&self) -> Vec<Peer>;
    fn can_dial(&self, peer: &Peer) -> bool;
    fn reconcile_discovery(&self);
    /// Publishes host-specific effects and returns the number of moved items.
    fn sync_peer(&self, peer: Peer, cycle: SyncCycle) -> impl Future<Output = u64> + Send;
}

pub struct PeerSyncDriver {
    pub rounds: RoundGate,
    pub cycle: SyncCycle,
    pub cadence: AdaptiveCadence,
    wake: Notify,
}
impl PeerSyncDriver {
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            rounds: RoundGate::new(),
            cycle: SyncCycle::new(),
            cadence: AdaptiveCadence::new(MIN_POLL_INTERVAL, MAX_POLL_INTERVAL_WITHOUT_PUSH),
            wake: Notify::new(),
        })
    }
    pub fn wake(&self) {
        self.cadence.reset();
        self.wake.notify_one();
    }
    pub async fn wake_signal(&self) {
        self.wake.notified().await;
    }
    pub fn set_enabled(&self, enabled: bool) {
        self.cycle.set_enabled(enabled);
        if enabled {
            self.wake();
        }
    }
    pub async fn run<H: PeerSyncHost>(
        self: Arc<Self>,
        host: Arc<H>,
        mut shutdown: watch::Receiver<bool>,
    ) {
        loop {
            if *shutdown.borrow() {
                return;
            }
            let wait = if host.peers().is_empty() {
                NO_PEERS_INTERVAL
            } else {
                self.cadence.interval()
            };
            let woke = tokio::select! {
                biased;
                _ = shutdown.changed() => return,
                _ = self.wake.notified() => true,
                _ = tokio::time::sleep(wait) => false,
            };
            if !host.enabled() {
                continue;
            }
            let round = if woke {
                tokio::select! {
                    biased;
                    _ = shutdown.changed() => return,
                    round = self.rounds.enter() => round,
                }
            } else {
                let Some(round) = self.rounds.try_enter() else {
                    continue;
                };
                round
            };
            host.reconcile_discovery();
            self.run_round(Arc::clone(&host), shutdown.clone(), round)
                .await;
        }
    }
    /// Own the round permit until every cancelled session has released its resources.
    pub async fn run_round<H: PeerSyncHost>(
        &self,
        host: Arc<H>,
        mut shutdown: watch::Receiver<bool>,
        _round: RoundGuard,
    ) {
        if !host.enabled() || *shutdown.borrow() {
            return;
        }
        let peers = host
            .peers()
            .into_iter()
            .filter(|peer| host.can_dial(peer))
            .collect::<Vec<_>>();
        if peers.is_empty() {
            return;
        }
        let cancel = self.cycle.cancel_token();
        let mut sessions = tokio::task::JoinSet::new();
        for peer in peers {
            let host = Arc::clone(&host);
            let cycle = self.cycle.clone();
            sessions.spawn(async move { host.sync_peer(peer, cycle).await });
        }
        let mut moved = 0u64;
        while !sessions.is_empty() {
            let result = tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                _ = cancel.cancelled() => break,
                _ = self.wake.notified() => { self.wake(); break; },
                result = sessions.join_next() => result,
            };
            if let Some(Ok(count)) = result {
                moved = moved.saturating_add(count);
            }
        }
        // Aborting unfinished sessions releases their session permits on drop.
        sessions.abort_all();
        while sessions.join_next().await.is_some() {}
        self.cadence.note_activity(moved > 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

    struct EmptyHost {
        enabled: AtomicBool,
        reconciled: AtomicUsize,
    }
    impl PeerSyncHost for EmptyHost {
        fn enabled(&self) -> bool {
            self.enabled.load(Ordering::SeqCst)
        }
        fn peers(&self) -> Vec<Peer> {
            Vec::new()
        }
        fn can_dial(&self, _: &Peer) -> bool {
            false
        }
        fn reconcile_discovery(&self) {
            self.reconciled.fetch_add(1, Ordering::SeqCst);
        }
        async fn sync_peer(&self, _: Peer, _: SyncCycle) -> u64 {
            unreachable!()
        }
    }
    #[tokio::test(start_paused = true)]
    async fn no_peer_cadence_and_capture_wake_are_shared_by_both_hosts() {
        let driver = PeerSyncDriver::new();
        let host = Arc::new(EmptyHost {
            enabled: AtomicBool::new(true),
            reconciled: AtomicUsize::new(0),
        });
        let (stop, receiver) = watch::channel(false);
        let task = tokio::spawn(Arc::clone(&driver).run(Arc::clone(&host), receiver));
        tokio::task::yield_now().await;
        tokio::time::advance(Duration::from_secs(59)).await;
        tokio::task::yield_now().await;
        assert_eq!(host.reconciled.load(Ordering::SeqCst), 0);
        tokio::time::advance(Duration::from_secs(1)).await;
        tokio::task::yield_now().await;
        assert_eq!(host.reconciled.load(Ordering::SeqCst), 1);
        driver.wake();
        tokio::task::yield_now().await;
        assert_eq!(host.reconciled.load(Ordering::SeqCst), 2);
        assert_eq!(driver.cadence.interval(), MIN_POLL_INTERVAL);
        stop.send(true).unwrap();
        task.await.unwrap();
        assert!(!driver.rounds.is_running());
    }
}
