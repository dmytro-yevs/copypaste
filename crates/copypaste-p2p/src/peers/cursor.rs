use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use copypaste_fs::Visibility;

use crate::sync::SyncCursor;

pub const DEFAULT_CURSOR_FILE_NAME: &str = "sync-cursors.json";

#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
struct Record {
    #[serde(default)]
    since_ms: i64,
    #[serde(default)]
    relay_floor_ms: Option<i64>,
}

impl From<Record> for SyncCursor {
    fn from(record: Record) -> Self {
        Self {
            since_ms: record.since_ms,
            relay_floor_ms: record.relay_floor_ms,
        }
    }
}

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct CursorFile {
    #[serde(default)]
    peers: BTreeMap<String, Record>,
}

pub struct CursorStore {
    path: PathBuf,
    state: RwLock<BTreeMap<String, Record>>,
}

impl std::fmt::Debug for CursorStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CursorStore").finish_non_exhaustive()
    }
}

impl CursorStore {
    #[must_use]
    pub fn open(path: &Path) -> Self {
        let peers = std::fs::read(path)
            .ok()
            .and_then(|bytes| serde_json::from_slice::<CursorFile>(&bytes).ok())
            .map(|file| file.peers)
            .unwrap_or_default();
        Self {
            path: path.to_path_buf(),
            state: RwLock::new(peers),
        }
    }

    #[must_use]
    pub fn get(&self, pairing_id: &str) -> SyncCursor {
        self.state
            .read()
            .ok()
            .and_then(|state| state.get(pairing_id).copied())
            .unwrap_or_default()
            .into()
    }

    pub fn record_session(&self, pairing_id: &str, since_ms: i64) {
        self.mutate(|state| {
            state.insert(
                pairing_id.to_string(),
                Record {
                    since_ms: since_ms.max(0),
                    relay_floor_ms: None,
                },
            );
            true
        });
    }

    pub fn note_applied(&self, from_pairing_id: &str, floor_ms: i64) {
        self.note_written(Some(from_pairing_id), floor_ms);
    }

    pub fn note_local(&self, floor_ms: i64) {
        self.note_written(None, floor_ms);
    }

    fn note_written(&self, from_pairing_id: Option<&str>, floor_ms: i64) {
        self.mutate(|state| {
            let mut changed = false;
            for (pairing_id, record) in state.iter_mut() {
                if from_pairing_id.is_some_and(|from| pairing_id == from)
                    || record.since_ms <= floor_ms
                {
                    continue;
                }
                let lowered = record.relay_floor_ms.map_or(floor_ms, |f| f.min(floor_ms));
                if record.relay_floor_ms != Some(lowered) {
                    record.relay_floor_ms = Some(lowered);
                    changed = true;
                }
            }
            changed
        });
    }

    pub fn forget(&self, pairing_id: &str) {
        self.mutate(|state| state.remove(pairing_id).is_some());
    }

    pub fn retain(&self, keep: &[String]) {
        self.mutate(|state| {
            let before = state.len();
            state.retain(|id, _| keep.iter().any(|k| k == id));
            state.len() != before
        });
    }

    fn mutate(&self, f: impl FnOnce(&mut BTreeMap<String, Record>) -> bool) {
        let Ok(mut state) = self.state.write() else {
            tracing::error!("sync-cursor store lock is poisoned");
            return;
        };
        if !f(&mut state) {
            return;
        }
        if let Err(error) = write_atomically(&self.path, &state) {
            tracing::debug!(%error, "could not persist the peer sync cursors");
        }
    }
}

/// `fsync`ed before the rename publishes it, unlike the copy this replaced: a
/// rename that outlives its own contents leaves a file that will not parse, and
/// [`CursorStore::open`] answers that by resetting every cursor to zero — a full
/// re-exchange with every peer.
fn write_atomically(path: &Path, peers: &BTreeMap<String, Record>) -> Result<(), std::io::Error> {
    let json = serde_json::to_vec(&CursorFile {
        peers: peers.clone(),
    })
    .map_err(std::io::Error::other)?;
    copypaste_fs::write_atomically(path, &json, Visibility::Inherited)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(dir: &tempfile::TempDir) -> CursorStore {
        CursorStore::open(&dir.path().join(DEFAULT_CURSOR_FILE_NAME))
    }

    #[test]
    fn an_unknown_pairing_starts_from_the_beginning_of_history() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(store(&dir).get("nobody"), SyncCursor::default());
        assert_eq!(store(&dir).get("nobody").since_ms, 0);
    }

    #[test]
    fn a_recorded_session_survives_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        store(&dir).record_session("laptop", 1_700);
        assert_eq!(store(&dir).get("laptop").since_ms, 1_700);
    }

    #[test]
    fn a_damaged_or_missing_file_reads_as_a_full_exchange() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DEFAULT_CURSOR_FILE_NAME);
        std::fs::write(&path, b"not json").unwrap();
        assert_eq!(CursorStore::open(&path).get("laptop").since_ms, 0);
    }

    #[test]
    fn applying_an_older_version_pulls_every_other_peer_back_to_it() {
        let dir = tempfile::tempdir().unwrap();
        let cursors = store(&dir);
        cursors.record_session("laptop", 5_000);
        cursors.record_session("phone", 5_000);

        cursors.note_applied("phone", 900);

        assert_eq!(
            cursors.get("laptop").relay_floor_ms,
            Some(900),
            "the relayed version must be re-advertised to the other peer"
        );
        assert_eq!(
            cursors.get("phone").relay_floor_ms,
            None,
            "the peer it came from already has it"
        );
        assert_eq!(cursors.get("laptop").advertise_from(5_000), 900);
    }

    #[test]
    fn writing_an_older_local_version_pulls_every_peer_back_to_it() {
        let dir = tempfile::tempdir().unwrap();
        let cursors = store(&dir);
        cursors.record_session("laptop", 5_000);
        cursors.record_session("phone", 6_000);

        cursors.note_local(900);

        assert_eq!(cursors.get("laptop").advertise_from(5_000), 900);
        assert_eq!(cursors.get("phone").advertise_from(6_000), 900);
    }

    #[test]
    fn a_completed_session_clears_the_relay_floor() {
        let dir = tempfile::tempdir().unwrap();
        let cursors = store(&dir);
        cursors.record_session("laptop", 5_000);
        cursors.note_applied("phone", 900);
        assert_eq!(cursors.get("laptop").relay_floor_ms, Some(900));

        cursors.record_session("laptop", 5_100);
        assert_eq!(cursors.get("laptop").relay_floor_ms, None);
        assert_eq!(cursors.get("laptop").advertise_from(5_100), 5_100);
    }

    #[test]
    fn the_lowest_of_several_relayed_versions_wins() {
        let dir = tempfile::tempdir().unwrap();
        let cursors = store(&dir);
        cursors.record_session("laptop", 5_000);
        cursors.note_applied("phone", 900);
        cursors.note_applied("phone", 400);
        cursors.note_applied("phone", 4_000);
        assert_eq!(cursors.get("laptop").relay_floor_ms, Some(400));
    }

    #[test]
    fn forgetting_a_pairing_drops_its_cursor() {
        let dir = tempfile::tempdir().unwrap();
        let cursors = store(&dir);
        cursors.record_session("laptop", 5_000);
        cursors.forget("laptop");
        assert_eq!(cursors.get("laptop").since_ms, 0);
        assert_eq!(store(&dir).get("laptop").since_ms, 0);
    }

    #[test]
    fn retain_drops_cursors_for_pairings_that_are_gone() {
        let dir = tempfile::tempdir().unwrap();
        let cursors = store(&dir);
        cursors.record_session("laptop", 5_000);
        cursors.record_session("phone", 6_000);
        cursors.retain(&["phone".to_string()]);
        assert_eq!(cursors.get("laptop").since_ms, 0);
        assert_eq!(cursors.get("phone").since_ms, 6_000);
    }

    #[test]
    fn the_cursor_file_holds_no_key_material() {
        let dir = tempfile::tempdir().unwrap();
        let cursors = store(&dir);
        cursors.record_session("laptop", 5_000);
        let text = std::fs::read_to_string(dir.path().join(DEFAULT_CURSOR_FILE_NAME)).unwrap();
        assert!(text.contains("laptop"));
        assert!(!text.contains("psk"));
    }
}
