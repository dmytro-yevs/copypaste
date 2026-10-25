//! Durable active conditions and bounded synchronization events. Active
//! conditions are independent of event eviction and survive application restarts.
use copypaste_ipc::{
    sync_issue::{SyncEvidence, SyncIssue, SyncIssueCode},
    sync_status::{SyncLogEntry, SyncLogKind},
};
use std::{
    collections::{BTreeMap, VecDeque},
    io::Read,
    path::PathBuf,
};

pub(super) const MAX_ENTRIES: usize = 100;
const MAX_FILE_BYTES: u64 = 128 * 1024;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct JournalFile {
    entries: VecDeque<SyncLogEntry>,
    conditions: BTreeMap<String, Vec<Condition>>,
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Condition {
    #[serde(flatten)]
    issue: SyncIssue,
    #[serde(default)]
    observed: u64,
}

pub(super) struct Journal {
    path: Option<PathBuf>,
    file: JournalFile,
    dirty: bool,
    available: bool,
    observation: u64,
}

impl Default for Journal {
    fn default() -> Self {
        Self::open(None)
    }
}

impl Journal {
    pub(super) fn open(path: Option<PathBuf>) -> Self {
        let mut available = path.is_none();
        let file = path
            .as_ref()
            .and_then(|path| {
                let mut bytes = Vec::new();
                let file = match std::fs::File::open(path) {
                    Ok(file) => file,
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                        available = true;
                        return None;
                    }
                    Err(_) => return None,
                };
                file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes).ok()?;
                if bytes.len() as u64 > MAX_FILE_BYTES {
                    return None;
                }
                // Preserve the preceding journal format; historical strings are
                // never used to infer a current condition during migration.
                let file = serde_json::from_slice::<JournalFile>(&bytes)
                    .ok()
                    .or_else(|| {
                        serde_json::from_slice::<VecDeque<SyncLogEntry>>(&bytes)
                            .ok()
                            .map(|entries| JournalFile {
                                entries,
                                ..Default::default()
                            })
                    })?;
                let valid = file.entries.len() <= MAX_ENTRIES
                    && file.entries.iter().all(valid_event)
                    && file.conditions.len() <= crate::peers::MAX_PAIRINGS
                    && file.conditions.iter().all(|(id, issues)| {
                        valid_id(id)
                            && issues.len() <= SyncIssueCode::COUNT
                            && issues.iter().all(|condition| condition.issue.since_ms >= 0)
                    });
                if valid {
                    available = true;
                    Some(file)
                } else {
                    None
                }
            })
            .unwrap_or_default();
        let observation = file
            .conditions
            .values()
            .flatten()
            .map(|condition| condition.observed)
            .max()
            .unwrap_or(0);
        Self {
            observation,
            path,
            file,
            dirty: false,
            available,
        }
    }

    pub(super) fn available(&self) -> bool {
        self.available
    }

    pub(super) fn snapshot(&self, peers: &[copypaste_ipc::PeerSyncStatus]) -> Vec<SyncLogEntry> {
        self.file
            .entries
            .iter()
            .rev()
            .filter(|entry| peers.iter().any(|peer| peer.pairing_id == entry.pairing_id))
            .cloned()
            .collect()
    }

    pub(super) fn issues(&self, id: &str) -> Vec<SyncIssue> {
        self.file
            .conditions
            .get(id)
            .map(|conditions| {
                conditions
                    .iter()
                    .map(|condition| condition.issue.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub(super) fn condition(&mut self, id: &str, code: SyncIssueCode, count: u64) -> bool {
        if !valid_id(id)
            || (!self.file.conditions.contains_key(id)
                && self.file.conditions.len() >= crate::peers::MAX_PAIRINGS)
        {
            return false;
        }
        self.observation = self.observation.saturating_add(1);
        let issues = self.file.conditions.entry(id.into()).or_default();
        if let Some(condition) = issues
            .iter_mut()
            .find(|condition| condition.issue.code == code)
        {
            condition.observed = self.observation;
            if condition.issue.count != count {
                condition.issue.count = count;
                self.dirty = true;
                return true;
            }
            return false;
        }
        if issues.len() >= SyncIssueCode::COUNT {
            return false;
        }
        issues.push(Condition {
            issue: SyncIssue {
                code,
                count,
                since_ms: crate::now_ms(),
            },
            observed: self.observation,
        });
        self.dirty = true;
        if code != SyncIssueCode::MorePending {
            self.record(problem_event(id, code, SyncLogKind::ProblemDetected));
        }
        true
    }

    pub(super) fn observation(&self) -> u64 {
        self.observation
    }
    pub(super) fn evidence(&mut self, id: &str, evidence: SyncEvidence) -> bool {
        self.evidence_at(id, evidence, self.observation)
    }
    pub(super) fn evidence_at(&mut self, id: &str, evidence: SyncEvidence, observed: u64) -> bool {
        let codes: Vec<_> = self
            .file
            .conditions
            .get(id)
            .into_iter()
            .flatten()
            .filter(|condition| {
                condition.observed <= observed && condition.issue.code.resolved_by(evidence)
            })
            .map(|condition| condition.issue.code)
            .collect();
        let changed = !codes.is_empty();
        for code in codes {
            self.resolve(id, code);
        }
        changed
    }
    pub(super) fn pending(&mut self, id: &str, code: SyncIssueCode, count: u64) -> bool {
        debug_assert!(code.pending());
        if count > 0 {
            self.condition(id, code, count)
        } else {
            self.resolve(id, code)
        }
    }

    fn resolve(&mut self, id: &str, code: SyncIssueCode) -> bool {
        let Some(issues) = self.file.conditions.get_mut(id) else {
            return false;
        };
        let before = issues.len();
        issues.retain(|condition| condition.issue.code != code);
        if issues.len() == before {
            return false;
        }
        if issues.is_empty() {
            self.file.conditions.remove(id);
        }
        self.dirty = true;
        if code != SyncIssueCode::MorePending {
            self.record(problem_event(id, code, SyncLogKind::ProblemResolved));
        }
        true
    }

    pub(super) fn forget(&mut self, id: &str) -> bool {
        if self.file.conditions.remove(id).is_some() {
            self.dirty = true;
            return true;
        }
        false
    }

    pub(super) fn record(&mut self, mut entry: SyncLogEntry) -> bool {
        entry.error = entry.error.map(|error| {
            let mut safe = copypaste_ipc::redact::scrub_paths(&error);
            while safe.len() > 512 {
                safe.pop();
            }
            safe
        });
        if !valid_event(&entry) {
            return false;
        }
        if entry.kind == SyncLogKind::Transferred && entry.sent == 0 && entry.received == 0 {
            return false;
        }
        if self
            .file
            .entries
            .iter()
            .rev()
            .find(|old| old.pairing_id == entry.pairing_id)
            .is_some_and(|old| {
                old.kind == entry.kind
                    && old.issue_code == entry.issue_code
                    && old.error == entry.error
                    && entry.kind != SyncLogKind::Transferred
            })
        {
            return false;
        }
        if self.file.entries.len() == MAX_ENTRIES {
            self.file.entries.pop_front();
        }
        self.file.entries.push_back(entry);
        self.dirty = true;
        true
    }

    pub(super) fn flush(&mut self) {
        if !self.dirty {
            return;
        }
        let Some(path) = &self.path else {
            self.dirty = false;
            return;
        };
        let saved = serde_json::to_vec(&self.file)
            .ok()
            .filter(|bytes| bytes.len() as u64 <= MAX_FILE_BYTES)
            .and_then(|bytes| {
                copypaste_fs::write_atomically(path, &bytes, copypaste_fs::Visibility::OwnerOnly)
                    .ok()
            });
        self.available = saved.is_some();
        if self.available {
            self.dirty = false;
        } else {
            tracing::warn!("Synchronization conditions and history could not be saved.");
        }
    }
}

fn problem_event(id: &str, code: SyncIssueCode, kind: SyncLogKind) -> SyncLogEntry {
    SyncLogEntry {
        pairing_id: id.into(),
        timestamp_ms: crate::now_ms(),
        kind,
        issue_code: Some(code),
        error: Some(
            if kind == SyncLogKind::ProblemResolved {
                code.resolved_message()
            } else {
                code.message()
            }
            .into(),
        ),
        sent: 0,
        received: 0,
    }
}
fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}
fn valid_event(entry: &SyncLogEntry) -> bool {
    valid_id(&entry.pairing_id)
        && entry.timestamp_ms >= 0
        && entry
            .error
            .as_ref()
            .is_none_or(|error| error.len() <= 512 && !error.chars().any(char::is_control))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn active_blockers_survive_offline_restart_and_event_eviction_until_relevant_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync-log.json");
        let mut journal = Journal::open(Some(path.clone()));
        journal.condition("peer", SyncIssueCode::VersionMismatch, 0);
        let old_round = journal.observation();
        journal.condition("peer", SyncIssueCode::StorageFull, 0);
        journal.evidence_at("peer", SyncEvidence::HistoryComplete, old_round);
        assert_eq!(journal.issues("peer").len(), 1);
        assert_eq!(journal.issues("peer")[0].code, SyncIssueCode::StorageFull);
        journal.condition("peer", SyncIssueCode::VersionMismatch, 0);
        journal.condition("peer", SyncIssueCode::VersionMismatch, 0);
        assert_eq!(journal.file.entries.len(), 4);
        for i in 0..110 {
            journal.record(SyncLogEntry {
                pairing_id: "peer".into(),
                timestamp_ms: i,
                kind: SyncLogKind::Transferred,
                sent: 1,
                received: 0,
                error: None,
                issue_code: None,
            });
        }
        journal.flush();
        let mut journal = Journal::open(Some(path));
        assert_eq!(journal.file.entries.len(), MAX_ENTRIES);
        assert_eq!(journal.issues("peer").len(), 2);
        journal.evidence("peer", SyncEvidence::CompatibleProtocol);
        assert_eq!(journal.issues("peer")[0].code, SyncIssueCode::StorageFull);
        journal.evidence("peer", SyncEvidence::HistoryComplete);
        assert!(journal.issues("peer").is_empty());
        let count = journal.file.entries.len();
        journal.record(SyncLogEntry {
            pairing_id: "peer".into(),
            timestamp_ms: 200,
            kind: SyncLogKind::Transferred,
            sent: 0,
            received: 0,
            error: None,
            issue_code: None,
        });
        assert_eq!(journal.file.entries.len(), count);
    }
}
