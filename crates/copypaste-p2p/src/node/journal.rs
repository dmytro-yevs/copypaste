//! Synchronization conditions and the bounded history of the ones that needed
//! the user. Those survive restarts and history eviction. Conditions that
//! clear on their own stay in memory: they are reported while they last and
//! never reach the history or the disk.
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

type Conditions = BTreeMap<String, Vec<Condition>>;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct JournalFile {
    entries: VecDeque<SyncLogEntry>,
    conditions: Conditions,
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
    /// Conditions that clear on their own; never saved.
    passing: Conditions,
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
        let mut file = path
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
        let stored = (file.entries.len(), count(&file.conditions));
        file.conditions.retain(|_, issues| {
            issues.retain(|condition| condition.issue.code.needs_attention());
            !issues.is_empty()
        });
        file.entries = incidents(std::mem::take(&mut file.entries), &file.conditions);
        let migrated = stored != (file.entries.len(), count(&file.conditions));
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
            passing: Conditions::new(),
            dirty: migrated,
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

    fn active<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a Condition> {
        [&self.file.conditions, &self.passing]
            .into_iter()
            .filter_map(move |conditions| conditions.get(id))
            .flatten()
    }

    pub(super) fn issues(&self, id: &str) -> Vec<SyncIssue> {
        self.active(id)
            .map(|condition| condition.issue.clone())
            .collect()
    }

    pub(super) fn condition(&mut self, id: &str, code: SyncIssueCode, count: u64) -> bool {
        let durable = code.needs_attention();
        let conditions = if durable {
            &mut self.file.conditions
        } else {
            &mut self.passing
        };
        if !valid_id(id)
            || (!conditions.contains_key(id) && conditions.len() >= crate::peers::MAX_PAIRINGS)
        {
            return false;
        }
        self.observation = self.observation.saturating_add(1);
        let observed = self.observation;
        let issues = conditions.entry(id.into()).or_default();
        if let Some(condition) = issues
            .iter_mut()
            .find(|condition| condition.issue.code == code)
        {
            condition.observed = observed;
            if condition.issue.count == count {
                return false;
            }
            condition.issue.count = count;
            self.dirty |= durable;
            return true;
        }
        if issues.len() >= SyncIssueCode::COUNT {
            return false;
        }
        let since_ms = crate::now_ms();
        issues.push(Condition {
            issue: SyncIssue {
                code,
                count,
                since_ms,
            },
            observed,
        });
        if durable {
            tracing::warn!("peer sync issue: {}", code.as_str());
            self.dirty = true;
            self.push(incident(id, code, since_ms, None));
        } else {
            tracing::debug!("peer sync condition: {}", code.as_str());
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
            .active(id)
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
        let durable = code.needs_attention();
        let conditions = if durable {
            &mut self.file.conditions
        } else {
            &mut self.passing
        };
        let Some(issues) = conditions.get_mut(id) else {
            return false;
        };
        let Some(index) = issues
            .iter()
            .position(|condition| condition.issue.code == code)
        else {
            return false;
        };
        let since_ms = issues.remove(index).issue.since_ms;
        if issues.is_empty() {
            conditions.remove(id);
        }
        if durable {
            tracing::info!("peer sync issue resolved: {}", code.as_str());
            self.dirty = true;
            // A clock set back must not end a problem before it began.
            let ended = Some(crate::now_ms().max(since_ms));
            match open_incident(&mut self.file.entries, id, code) {
                Some(entry) => entry.resolved_ms = ended,
                // Its opening entry was evicted; the condition kept the start.
                None => self.push(incident(id, code, since_ms, ended)),
            }
        }
        true
    }

    pub(super) fn forget(&mut self, id: &str) -> bool {
        let durable = self.file.conditions.remove(id).is_some();
        self.dirty |= durable;
        self.passing.remove(id).is_some() || durable
    }

    fn push(&mut self, entry: SyncLogEntry) {
        if self.file.entries.len() == MAX_ENTRIES {
            self.file.entries.pop_front();
        }
        self.file.entries.push_back(entry);
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

fn count(conditions: &Conditions) -> usize {
    conditions.values().map(Vec::len).sum()
}

fn incident(
    id: &str,
    code: SyncIssueCode,
    since_ms: i64,
    resolved_ms: Option<i64>,
) -> SyncLogEntry {
    SyncLogEntry {
        pairing_id: id.into(),
        timestamp_ms: since_ms,
        kind: SyncLogKind::ProblemDetected,
        issue_code: Some(code),
        error: None,
        sent: 0,
        received: 0,
        resolved_ms,
    }
}

fn open_incident<'a>(
    entries: &'a mut VecDeque<SyncLogEntry>,
    id: &str,
    code: SyncIssueCode,
) -> Option<&'a mut SyncLogEntry> {
    entries.iter_mut().rev().find(|entry| {
        entry.pairing_id == id && entry.issue_code == Some(code) && entry.resolved_ms.is_none()
    })
}

/// Rebuilds stored events as one entry per problem that needed the user.
/// Earlier journals kept one event for the start and another for the end of
/// every condition, including the ones that clear on their own.
fn incidents(entries: VecDeque<SyncLogEntry>, active: &Conditions) -> VecDeque<SyncLogEntry> {
    let mut kept = VecDeque::<SyncLogEntry>::new();
    for entry in entries {
        let Some(code) = entry.issue_code.filter(|code| code.needs_attention()) else {
            continue;
        };
        match entry.kind {
            SyncLogKind::ProblemDetected => kept.push_back(entry),
            SyncLogKind::ProblemResolved => {
                if let Some(open) = open_incident(&mut kept, &entry.pairing_id, code) {
                    open.resolved_ms = Some(entry.timestamp_ms.max(open.timestamp_ms));
                }
            }
            _ => {}
        }
    }
    // An open entry stands for an active condition; without one its end is
    // unknown and it would read as a problem that never cleared.
    kept.retain(|entry| {
        entry.resolved_ms.is_some()
            || active.get(&entry.pairing_id).is_some_and(|issues| {
                issues
                    .iter()
                    .any(|condition| Some(condition.issue.code) == entry.issue_code)
            })
    });
    kept
}

fn valid_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && !id.chars().any(char::is_control)
}
fn valid_event(entry: &SyncLogEntry) -> bool {
    valid_id(&entry.pairing_id)
        && entry.timestamp_ms >= 0
        && entry.resolved_ms.is_none_or(|ended| ended >= 0)
        && entry
            .error
            .as_ref()
            .is_none_or(|error| error.len() <= 512 && !error.chars().any(char::is_control))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn journal() -> (tempfile::TempDir, PathBuf, Journal) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sync-log.json");
        let journal = Journal::open(Some(path.clone()));
        (dir, path, journal)
    }

    #[test]
    fn conditions_that_clear_on_their_own_stay_out_of_the_file_and_the_history() {
        let (_dir, path, mut journal) = journal();
        assert!(journal.condition("peer", SyncIssueCode::ConnectionLost, 0));
        assert!(journal.pending("peer", SyncIssueCode::MorePending, 3));
        assert_eq!(journal.issues("peer").len(), 2);
        journal.flush();
        assert!(!path.exists());
        assert!(journal.evidence("peer", SyncEvidence::CompatibleProtocol));
        assert!(journal.pending("peer", SyncIssueCode::MorePending, 0));
        assert!(journal.issues("peer").is_empty());
        assert!(journal.file.entries.is_empty());
        journal.flush();
        assert!(!path.exists());
    }

    #[test]
    fn a_problem_that_needs_the_user_is_one_entry_across_restart_and_eviction() {
        let (_dir, path, mut journal) = journal();
        journal.condition("peer", SyncIssueCode::VersionMismatch, 0);
        journal.condition("peer", SyncIssueCode::VersionMismatch, 0);
        assert_eq!(journal.file.entries.len(), 1);
        journal.flush();
        let mut journal = Journal::open(Some(path));
        assert_eq!(
            journal.issues("peer")[0].code,
            SyncIssueCode::VersionMismatch
        );
        assert_eq!(journal.file.entries[0].resolved_ms, None);

        // Evidence from an older round cannot clear a newer observation.
        let old_round = journal.observation();
        journal.condition("peer", SyncIssueCode::StorageFull, 0);
        assert!(journal.evidence_at("peer", SyncEvidence::HistoryComplete, old_round));
        assert_eq!(journal.issues("peer").len(), 1);
        assert_eq!(journal.issues("peer")[0].code, SyncIssueCode::StorageFull);
        assert_eq!(journal.file.entries.len(), 2);
        let closed = &journal.file.entries[0];
        assert_eq!(closed.issue_code, Some(SyncIssueCode::VersionMismatch));
        assert!(closed
            .resolved_ms
            .is_some_and(|ended| ended >= closed.timestamp_ms));

        // The open entry is evicted; the active condition still closes it.
        for _ in 0..MAX_ENTRIES {
            journal.condition("other", SyncIssueCode::HistoryLocked, 0);
            journal.evidence("other", SyncEvidence::HistoryComplete);
        }
        assert!(journal
            .file
            .entries
            .iter()
            .all(|entry| entry.pairing_id == "other"));
        let since_ms = journal.issues("peer")[0].since_ms;
        journal.evidence("peer", SyncEvidence::HistoryComplete);
        assert!(journal.issues("peer").is_empty());
        assert_eq!(journal.file.entries.len(), MAX_ENTRIES);
        let restored = journal.file.entries.back().unwrap();
        assert_eq!(restored.pairing_id, "peer");
        assert_eq!(restored.timestamp_ms, since_ms);
        assert!(restored.resolved_ms.is_some());
    }

    #[test]
    fn earlier_journals_keep_only_the_problems_that_needed_the_user() {
        let (_dir, path, _) = journal();
        let event = |timestamp_ms, kind, issue_code| SyncLogEntry {
            pairing_id: "peer".into(),
            timestamp_ms,
            kind,
            issue_code,
            error: None,
            sent: 0,
            received: 0,
            resolved_ms: None,
        };
        let condition = |code, since_ms| Condition {
            issue: SyncIssue {
                code,
                since_ms,
                count: 0,
            },
            observed: 1,
        };
        let earlier = JournalFile {
            entries: [
                event(1, SyncLogKind::Failed, None),
                event(
                    2,
                    SyncLogKind::ProblemDetected,
                    Some(SyncIssueCode::ConnectionLost),
                ),
                event(
                    3,
                    SyncLogKind::ProblemResolved,
                    Some(SyncIssueCode::ConnectionLost),
                ),
                event(
                    4,
                    SyncLogKind::ProblemDetected,
                    Some(SyncIssueCode::StorageFull),
                ),
                event(
                    9,
                    SyncLogKind::ProblemResolved,
                    Some(SyncIssueCode::StorageFull),
                ),
                // No active condition: its end is unknown.
                event(
                    10,
                    SyncLogKind::ProblemDetected,
                    Some(SyncIssueCode::HistoryLocked),
                ),
                event(
                    11,
                    SyncLogKind::ProblemDetected,
                    Some(SyncIssueCode::VersionMismatch),
                ),
            ]
            .into(),
            conditions: [(
                "peer".to_owned(),
                vec![
                    condition(SyncIssueCode::ConnectionLost, 2),
                    condition(SyncIssueCode::VersionMismatch, 11),
                ],
            )]
            .into(),
        };
        std::fs::write(&path, serde_json::to_vec(&earlier).unwrap()).unwrap();

        let mut journal = Journal::open(Some(path.clone()));
        assert!(journal.available());
        let kept: Vec<_> = journal
            .file
            .entries
            .iter()
            .map(|entry| (entry.timestamp_ms, entry.resolved_ms))
            .collect();
        assert_eq!(kept, [(4, Some(9)), (11, None)]);
        assert_eq!(journal.issues("peer").len(), 1);
        assert_eq!(
            journal.issues("peer")[0].code,
            SyncIssueCode::VersionMismatch
        );
        journal.flush();
        assert!(!Journal::open(Some(path)).dirty);
    }
}
