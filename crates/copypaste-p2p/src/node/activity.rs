//! Session observations shared by outgoing and incoming peer transports.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use copypaste_ipc::{PeerSyncStatus, SyncPhase, SyncStatus};
use tokio::sync::watch;

use crate::peers::Peer;
use crate::sync::SyncStats;

#[derive(Default)]
struct State {
    revision: u64,
    peers: HashMap<String, Observation>,
}

#[derive(Default)]
struct Observation {
    active: usize,
    status: PeerSyncStatus,
}

pub(super) struct SyncActivity {
    state: Mutex<State>,
    changes: watch::Sender<()>,
}

impl SyncActivity {
    pub(super) fn new() -> Arc<Self> {
        let (changes, _) = watch::channel(());
        Arc::new(Self {
            state: Mutex::new(State::default()),
            changes,
        })
    }

    pub(super) fn subscribe(&self) -> watch::Receiver<()> {
        self.changes.subscribe()
    }

    pub(super) fn begin(self: &Arc<Self>, pairing_id: &str) -> SessionObservation {
        self.update(pairing_id, |observation| {
            observation.active += 1;
            observation.status.phase = SyncPhase::Syncing;
            observation.status.started_at_ms = Some(crate::now_ms());
            observation.status.error = None;
        });
        SessionObservation {
            activity: Arc::clone(self),
            pairing_id: pairing_id.into(),
            finished: false,
        }
    }

    fn update(&self, pairing_id: &str, apply: impl FnOnce(&mut Observation)) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        apply(state.peers.entry(pairing_id.into()).or_default());
        state.revision = state.revision.saturating_add(1);
        drop(state);
        self.changes.send_replace(());
    }

    pub(super) fn snapshot(&self, peers: &[Peer], enabled: bool) -> SyncStatus {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.peers.retain(|id, observation| {
            observation.active > 0 || peers.iter().any(|peer| &peer.pairing_id == id)
        });
        let peers: Vec<_> = peers
            .iter()
            .map(|peer| {
                let mut status = state
                    .peers
                    .get(&peer.pairing_id)
                    .map(|observation| observation.status.clone())
                    .unwrap_or_else(|| PeerSyncStatus {
                        phase: SyncPhase::Waiting,
                        ..Default::default()
                    });
                status.pairing_id = peer.pairing_id.clone();
                status.name = peer.name.clone();
                if !enabled {
                    status.phase = SyncPhase::Disabled;
                }
                status
            })
            .collect();
        let phase = if !enabled {
            SyncPhase::Disabled
        } else if peers.iter().any(|peer| peer.phase == SyncPhase::Syncing) {
            SyncPhase::Syncing
        } else if peers.iter().any(|peer| peer.phase == SyncPhase::Failed) {
            SyncPhase::Failed
        } else if !peers.is_empty() && peers.iter().all(|peer| peer.phase == SyncPhase::Synced) {
            SyncPhase::Synced
        } else {
            SyncPhase::Waiting
        };
        SyncStatus {
            revision: state.revision,
            phase,
            peers,
        }
    }
}

pub(super) struct SessionObservation {
    activity: Arc<SyncActivity>,
    pairing_id: String,
    finished: bool,
}

impl SessionObservation {
    pub(super) fn finish(mut self, stats: Option<&SyncStats>, error: Option<String>) {
        self.finish_inner(stats, error);
    }

    fn finish_inner(&mut self, stats: Option<&SyncStats>, error: Option<String>) {
        self.activity.update(&self.pairing_id, |observation| {
            observation.active = observation.active.saturating_sub(1);
            if let Some(stats) = stats {
                observation.status.last_success_ms = Some(crate::now_ms());
                observation.status.sent = stats.sent as u64;
                observation.status.received = stats.received as u64;
                observation.status.skipped_too_large = stats.skipped_too_large as u64;
            }
            observation.status.error = error;
            observation.status.phase = if observation.active > 0 {
                SyncPhase::Syncing
            } else if observation.status.error.is_some() {
                SyncPhase::Failed
            } else {
                SyncPhase::Synced
            };
        });
        self.finished = true;
    }
}

impl Drop for SessionObservation {
    fn drop(&mut self) {
        if !self.finished {
            self.finish_inner(None, Some("Synchronization was interrupted.".into()));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_sessions_leave_syncing_and_can_recover() {
        let activity = SyncActivity::new();
        let peer = Peer {
            pairing_id: "peer".into(),
            device_id: None,
            name: "phone".into(),
            psk: [7; 32],
            last_addr: None,
            last_seen_ms: 0,
            profile: None,
            profile_observed_at_ms: 0,
        };
        let peers = [peer];
        let session = activity.begin("peer");
        assert_eq!(activity.snapshot(&peers, true).phase, SyncPhase::Syncing);
        drop(session);
        let failed = activity.snapshot(&peers, true);
        assert_eq!(failed.phase, SyncPhase::Failed);
        assert!(failed.peers[0].error.is_some());
        activity.begin("peer").finish(
            Some(&SyncStats {
                sent: 2,
                ..Default::default()
            }),
            None,
        );
        let completed = activity.snapshot(&peers, true);
        assert_eq!(completed.phase, SyncPhase::Synced);
        assert_eq!(completed.peers[0].sent, 2);
        assert!(completed.peers[0].last_success_ms.is_some());
        assert_eq!(activity.snapshot(&peers, false).phase, SyncPhase::Disabled);
    }
}
