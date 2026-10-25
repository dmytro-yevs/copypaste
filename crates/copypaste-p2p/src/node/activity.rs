//! Live activity and durable conditions have independent lifecycles.
use super::NodeError;
use crate::{peers::Peer, sync::SyncStats};
use copypaste_ipc::{
    sync_issue::{SyncEvidence, SyncIssueCode},
    PeerSyncStatus, SyncPhase, SyncStatus,
};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use tokio::sync::watch;

#[derive(Default)]
struct State {
    revision: u64,
    next_session: u64,
    peers: HashMap<String, Observation>,
    journal: super::journal::Journal,
}
#[derive(Default)]
struct Observation {
    active: usize,
    transfers: HashSet<u64>,
    authenticated_at_ms: Option<i64>,
    generation: u64,
    completed_session: u64,
    status: PeerSyncStatus,
}
pub(super) struct SyncActivity {
    state: Mutex<State>,
    changes: watch::Sender<()>,
}

impl SyncActivity {
    #[cfg(test)]
    pub(super) fn new() -> Arc<Self> {
        Self::open(None)
    }
    pub(super) fn open(path: Option<std::path::PathBuf>) -> Arc<Self> {
        let (changes, _) = watch::channel(());
        Arc::new(Self {
            state: Mutex::new(State {
                journal: super::journal::Journal::open(path),
                ..Default::default()
            }),
            changes,
        })
    }
    fn mutate(&self, apply: impl FnOnce(&mut State) -> bool) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if !apply(&mut state) {
            return;
        }
        state.journal.flush();
        state.revision = state.revision.saturating_add(1);
        drop(state);
        self.changes.send_replace(());
    }
    pub(super) fn connection(&self, id: &str, online: bool) {
        self.mutate(|state| {
            state
                .journal
                .record(copypaste_ipc::sync_status::SyncLogEntry {
                    pairing_id: id.into(),
                    timestamp_ms: crate::now_ms(),
                    kind: if online {
                        copypaste_ipc::sync_status::SyncLogKind::Online
                    } else {
                        copypaste_ipc::sync_status::SyncLogKind::Offline
                    },
                    issue_code: None,
                    error: None,
                    sent: 0,
                    received: 0,
                })
        });
    }
    pub(super) fn authenticated(&self, id: &str) {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state
            .peers
            .entry(id.into())
            .or_default()
            .authenticated_at_ms = Some(crate::now_ms());
    }
    pub(super) fn authenticated_since(&self, id: &str, since: i64) -> bool {
        self.state
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .peers
            .get(id)
            .and_then(|peer| peer.authenticated_at_ms)
            .is_some_and(|time| time >= since)
    }
    pub(super) fn evidence(&self, id: &str, evidence: SyncEvidence) {
        self.mutate(|state| state.journal.evidence(id, evidence));
    }
    pub(super) fn failure(&self, id: &str, error: NodeError) {
        self.mutate(|state| {
            error
                .sync_issue()
                .is_some_and(|code| state.journal.condition(id, code, 0))
        });
    }
    pub(super) fn forget(&self, id: &str) {
        self.mutate(|state| {
            let changed = state.journal.forget(id);
            state.peers.remove(id).is_some() || changed
        });
    }
    pub(super) fn subscribe(&self) -> watch::Receiver<()> {
        self.changes.subscribe()
    }
    pub(super) fn begin(self: &Arc<Self>, id: &str) -> SessionObservation {
        let mut generation = 0;
        let mut observation = 0;
        let mut sequence = 0;
        self.mutate(|state| {
            state.next_session = state.next_session.saturating_add(1);
            sequence = state.next_session;
            observation = state.journal.observation();
            let peer = state.peers.entry(id.into()).or_default();
            peer.active += 1;
            if peer.status.phase == SyncPhase::Unavailable {
                peer.status.phase = SyncPhase::Waiting;
            }
            peer.status.started_at_ms = Some(crate::now_ms());
            generation = peer.generation;
            true
        });
        SessionObservation {
            activity: Arc::clone(self),
            pairing_id: id.into(),
            generation,
            observation,
            sequence,
            finished: false,
        }
    }
    pub(super) fn local_changed(&self, except: Option<&str>) {
        self.mutate(|state| {
            let mut visible_change = false;
            for (id, peer) in &mut state.peers {
                if except == Some(id.as_str()) {
                    continue;
                }
                peer.generation = peer.generation.saturating_add(1);
                if peer.status.phase == SyncPhase::Synced {
                    peer.status.phase = SyncPhase::Waiting;
                    visible_change = true;
                }
            }
            visible_change
        });
    }
    pub(super) fn transfer_scope(self: &Arc<Self>, id: &str, token: u64) -> TransferScope {
        TransferScope {
            activity: Arc::clone(self),
            id: id.into(),
            token,
        }
    }
    pub(super) fn transferring(&self, id: &str, token: u64) {
        self.mutate(|state| {
            state
                .peers
                .get_mut(id)
                .is_some_and(|peer| peer.transfers.insert(token))
        });
    }
    pub(super) fn snapshot(&self, peers: &[Peer], enabled: bool) -> SyncStatus {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        state.peers.retain(|id, peer| {
            peer.active > 0 || peers.iter().any(|candidate| &candidate.pairing_id == id)
        });
        let peers: Vec<_> = peers
            .iter()
            .map(|peer| {
                let mut status = state
                    .peers
                    .get(&peer.pairing_id)
                    .map(|peer| peer.status.clone())
                    .unwrap_or_else(|| PeerSyncStatus {
                        phase: SyncPhase::Waiting,
                        ..Default::default()
                    });
                status.pairing_id = peer.pairing_id.clone();
                status.name = peer.name.clone();
                status.checking = state
                    .peers
                    .get(&peer.pairing_id)
                    .is_some_and(|peer| peer.active > 0 && peer.transfers.is_empty());
                if state
                    .peers
                    .get(&peer.pairing_id)
                    .is_some_and(|peer| !peer.transfers.is_empty())
                {
                    status.phase = SyncPhase::Syncing;
                }
                status.issues = state.journal.issues(&peer.pairing_id);
                status
                    .issues
                    .sort_by_key(|issue| (!issue.code.blocking(), issue.code));
                status.error = status
                    .issues
                    .first()
                    .map(|issue| issue.code.message().into());
                if !enabled {
                    status.phase = SyncPhase::Disabled;
                } else if status.issues.iter().any(|issue| issue.code.blocking()) {
                    status.phase = SyncPhase::Failed;
                }
                status
            })
            .collect();
        let phase = if !enabled {
            SyncPhase::Disabled
        } else if peers.iter().any(|peer| peer.phase == SyncPhase::Failed) {
            SyncPhase::Failed
        } else if peers.iter().any(|peer| peer.phase == SyncPhase::Syncing) {
            SyncPhase::Syncing
        } else if !peers.is_empty() && peers.iter().all(|peer| peer.phase == SyncPhase::Synced) {
            SyncPhase::Synced
        } else {
            SyncPhase::Waiting
        };
        SyncStatus {
            diagnostics_available: state.journal.available(),
            revision: state.revision,
            phase,
            log: state.journal.snapshot(&peers),
            peers,
        }
    }
}

pub(super) struct TransferScope {
    activity: Arc<SyncActivity>,
    id: String,
    token: u64,
}
impl Drop for TransferScope {
    fn drop(&mut self) {
        self.activity.mutate(|state| {
            state
                .peers
                .get_mut(&self.id)
                .is_some_and(|peer| peer.transfers.remove(&self.token))
        });
    }
}

pub(super) struct SessionObservation {
    activity: Arc<SyncActivity>,
    pairing_id: String,
    generation: u64,
    observation: u64,
    sequence: u64,
    finished: bool,
}
impl SessionObservation {
    pub(super) fn finish(mut self, stats: Option<&SyncStats>, error: Option<NodeError>) {
        self.finish_inner(stats, error);
    }
    fn finish_inner(&mut self, stats: Option<&SyncStats>, error: Option<NodeError>) {
        self.activity.mutate(|state| {
            let peer = state.peers.entry(self.pairing_id.clone()).or_default();
            peer.active = peer.active.saturating_sub(1);
            let current_result = self.sequence >= peer.completed_session;
            if current_result {
                peer.completed_session = self.sequence;
            }
            if let Some(stats) = stats.filter(|_| current_result) {
                if stats.skipped_unavailable == 0
                    && stats.skipped_too_large == 0
                    && stats.skipped_invalid == 0
                    && stats.skipped_future == 0
                    && stats.deferred == 0
                    && peer.generation == self.generation
                {
                    peer.status.last_success_ms = Some(crate::now_ms());
                }
                peer.status.sent = stats.sent as u64;
                peer.status.received = stats.received as u64;
                peer.status.skipped_too_large = stats.skipped_too_large as u64;
                peer.status.phase = if peer.generation == self.generation {
                    SyncPhase::Synced
                } else {
                    SyncPhase::Waiting
                };
            } else if current_result && stats.is_none() {
                peer.status.phase = SyncPhase::Waiting;
            }
            if let Some(code) = error.and_then(NodeError::sync_issue) {
                state.journal.condition(&self.pairing_id, code, 0);
            }
            if let Some(stats) = stats.filter(|_| current_result) {
                state.journal.evidence_at(
                    &self.pairing_id,
                    SyncEvidence::HistoryComplete,
                    self.observation,
                );
                for (code, count) in [
                    (SyncIssueCode::ItemsTooLarge, stats.skipped_too_large),
                    (SyncIssueCode::ItemsRejected, stats.skipped_invalid),
                    (
                        SyncIssueCode::ItemsUnavailable,
                        stats
                            .skipped_unavailable
                            .saturating_sub(stats.skipped_invalid),
                    ),
                    (SyncIssueCode::ClockSkew, stats.skipped_future),
                    (SyncIssueCode::MorePending, stats.deferred),
                ] {
                    state.journal.pending(&self.pairing_id, code, count as u64);
                }
                state
                    .journal
                    .record(copypaste_ipc::sync_status::SyncLogEntry {
                        pairing_id: self.pairing_id.clone(),
                        timestamp_ms: crate::now_ms(),
                        kind: copypaste_ipc::sync_status::SyncLogKind::Transferred,
                        sent: stats.sent as u64,
                        received: stats.received as u64,
                        error: None,
                        issue_code: None,
                    });
            }
            true
        });
        self.finished = true;
    }
}
impl Drop for SessionObservation {
    fn drop(&mut self) {
        if !self.finished {
            self.finish_inner(None, Some(NodeError::Cancelled));
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
        assert_eq!(activity.snapshot(&peers, true).phase, SyncPhase::Waiting);
        let transfer = activity.transfer_scope("peer", 1);
        activity.transferring("peer", 1);
        assert_eq!(activity.snapshot(&peers, true).phase, SyncPhase::Syncing);
        drop(transfer);
        drop(session);
        assert_eq!(activity.snapshot(&peers, true).phase, SyncPhase::Waiting);
        activity
            .begin("peer")
            .finish(None, Some(super::super::NodeError::PeerVersion));
        let failed = activity.snapshot(&peers, true);
        assert_eq!(failed.phase, SyncPhase::Failed);
        assert!(failed.peers[0].error.is_some());
        assert_eq!(failed.log.len(), 1);
        let retry = activity.begin("peer");
        assert_eq!(activity.snapshot(&peers, true).phase, SyncPhase::Failed);
        assert_eq!(activity.snapshot(&peers, true).log.len(), 1);
        retry.finish(
            Some(&SyncStats {
                sent: 2,
                ..Default::default()
            }),
            None,
        );
        let completed = activity.snapshot(&peers, true);
        assert_eq!(completed.phase, SyncPhase::Synced);
        assert_eq!(completed.log.len(), 3);
        let idle = activity.begin("peer");
        assert_eq!(activity.snapshot(&peers, true).phase, SyncPhase::Synced);
        idle.finish(Some(&SyncStats::default()), None);
        assert_eq!(activity.snapshot(&peers, true).log.len(), 3);
        let running = activity.begin("peer");
        activity.local_changed(None);
        running.finish(Some(&SyncStats::default()), None);
        assert_eq!(activity.snapshot(&peers, true).phase, SyncPhase::Waiting);
        activity
            .begin("peer")
            .finish(Some(&SyncStats::default()), None);
        assert_eq!(activity.snapshot(&peers, true).phase, SyncPhase::Synced);
        assert_eq!(completed.peers[0].sent, 2);
        assert!(completed.peers[0].last_success_ms.is_some());
        assert_eq!(activity.snapshot(&peers, false).phase, SyncPhase::Disabled);
    }

    #[test]
    fn completed_sessions_with_pending_items_keep_success_and_report_the_refusals() {
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
        activity.begin("peer").finish(
            Some(&SyncStats {
                sent: 6,
                received: 2,
                skipped_unavailable: 5,
                ..Default::default()
            }),
            None,
        );
        let completed = activity.snapshot(&peers, true);
        assert_eq!(completed.phase, SyncPhase::Synced);
        let status = &completed.peers[0];
        assert_eq!((status.sent, status.received), (6, 2));
        assert!(status.last_success_ms.is_none());
        assert_eq!(status.issues[0].code, SyncIssueCode::ItemsUnavailable);
        assert_eq!(status.issues[0].count, 5);
        activity
            .begin("peer")
            .finish(Some(&SyncStats::default()), None);
        assert!(activity.snapshot(&peers, true).peers[0].error.is_none());
    }
}
