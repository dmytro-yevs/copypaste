//! The in-memory map, the lock around it, and the write-through to disk.

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use zeroize::Zeroizing;

use super::file::{parse, warn_if_permissive, write_atomically};
use super::{Peer, PeerStoreError};
use crate::transport::TOKEN_LEN;

/// The paired-device list: backed by a file, cached in memory, shareable behind
/// an `Arc`. Every mutation rewrites the whole file atomically — there are a
/// handful of peers, not a database — and the read methods never touch the
/// disk, so a listener checking PSKs on every inbound connection costs a read
/// lock and nothing else.
pub struct PeerStore {
    path: PathBuf,
    peers: RwLock<HashMap<String, Peer>>,
}

impl PeerStore {
    /// Open the store at `path`, creating an empty one in memory if the file
    /// does not exist. Nothing is written until the first mutation.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Legacy`] if another version wrote it — distinguishable
    /// on purpose, so the daemon can explain rather than report corruption;
    /// `Corrupt` if damaged; `Io` if it could not be read.
    pub fn open(path: &Path) -> Result<Self, PeerStoreError> {
        let peers = match std::fs::read(path) {
            Ok(bytes) => {
                warn_if_permissive(path);
                let text = Zeroizing::new(bytes);
                parse(&text)?
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => HashMap::new(),
            Err(err) => return Err(PeerStoreError::Io(err)),
        };
        Ok(Self {
            path: path.to_path_buf(),
            peers: RwLock::new(peers),
        })
    }

    /// Every known peer, ordered by pairing id so callers and tests see a
    /// stable sequence.
    #[must_use]
    pub fn list(&self) -> Vec<Peer> {
        let Ok(guard) = self.peers.read() else {
            // A poisoned lock cannot be reported through this signature. An
            // empty list is the fail-closed answer: it pairs nobody and trusts
            // nobody. The mutating methods do report it.
            tracing::error!("paired-devices store lock is poisoned");
            return Vec::new();
        };
        let mut peers: Vec<Peer> = guard.values().cloned().collect();
        peers.sort_by(|a, b| a.pairing_id.cmp(&b.pairing_id));
        peers
    }

    /// One peer by pairing id.
    #[must_use]
    pub fn get(&self, pairing_id: &str) -> Option<Peer> {
        let guard = self.peers.read().ok()?;
        guard.get(pairing_id).cloned()
    }

    /// Insert a peer, or replace the one with the same pairing id. The file is
    /// rewritten atomically before this returns, and a failed write rolls the
    /// in-memory map back to what is on disk, so a caller is never told a
    /// pairing was saved when it was not.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Invalid`] if the record does not validate — nothing is
    /// written; `Io` if the file could not be replaced; `Poisoned` if another
    /// thread panicked holding the lock.
    pub fn upsert(&self, peer: Peer) -> Result<(), PeerStoreError> {
        peer.validate()?;
        let mut guard = self.peers.write().map_err(|_| PeerStoreError::Poisoned)?;
        let pairing_id = peer.pairing_id.clone();
        let previous = guard.insert(pairing_id.clone(), peer);
        match write_atomically(&self.path, &guard) {
            Ok(()) => {
                tracing::debug!(%pairing_id, "paired device saved");
                Ok(())
            }
            Err(err) => {
                match previous {
                    Some(old) => guard.insert(pairing_id, old),
                    None => guard.remove(&pairing_id),
                };
                Err(err)
            }
        }
    }

    /// Forget a peer. Returns whether one was there to forget.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Io`] if the file could not be replaced — the peer stays
    /// in memory, matching what is still on disk; `Poisoned` if another thread
    /// panicked holding the lock.
    pub fn remove(&self, pairing_id: &str) -> Result<bool, PeerStoreError> {
        let mut guard = self.peers.write().map_err(|_| PeerStoreError::Poisoned)?;
        let Some(removed) = guard.remove(pairing_id) else {
            return Ok(false);
        };
        match write_atomically(&self.path, &guard) {
            Ok(()) => {
                tracing::debug!(%pairing_id, "paired device removed");
                Ok(true)
            }
            Err(err) => {
                guard.insert(pairing_id.to_string(), removed);
                Err(err)
            }
        }
    }

    /// Every known pairing id and PSK, for a responder to try against an
    /// inbound handshake ([`crate::Session::accept_any`]).
    ///
    /// Ordered by pairing id, so the trial order does not depend on hash-map
    /// iteration and is therefore reproducible.
    ///
    /// These are copies of live key material and are **not** zeroized for you.
    /// Bind the result to a `zeroize::Zeroizing` if it outlives the handshake.
    #[must_use]
    pub fn psks(&self) -> Vec<(String, [u8; TOKEN_LEN])> {
        self.list()
            .iter()
            .map(|peer| (peer.pairing_id.clone(), peer.psk))
            .collect()
    }

    /// How many peers are known. Cheaper than [`PeerStore::list`] for a status
    /// line.
    #[must_use]
    pub fn len(&self) -> usize {
        self.peers.read().map(|g| g.len()).unwrap_or(0)
    }

    /// Whether this device is paired with anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Peer count only. The path is not printed — `CLAUDE.md` rule 4 keeps paths
/// away from users, and a `Debug` line ends up in the same log as everything
/// else.
impl fmt::Debug for PeerStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PeerStore")
            .field("peers", &self.len())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::testutil::{peer, store_path};
    use crate::peers::DEFAULT_FILE_NAME;
    use crate::transport::PairingToken;

    #[test]
    fn opening_a_missing_file_yields_an_empty_store_and_writes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        assert!(store.is_empty());
        assert!(store.list().is_empty());
        assert!(store.psks().is_empty());
        assert!(!path.exists(), "opening must not create the file");
    }

    #[test]
    fn round_trips_through_the_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);

        let laptop = peer("Laptop");
        let phone = peer("Phone");
        let (laptop_id, laptop_psk) = (laptop.pairing_id.clone(), laptop.psk);
        let phone_id = phone.pairing_id.clone();

        let store = PeerStore::open(&path).expect("open");
        store.upsert(laptop).expect("upsert laptop");
        store.upsert(phone).expect("upsert phone");
        assert_eq!(store.len(), 2);

        // Reopened from disk, everything survives byte for byte.
        let reopened = PeerStore::open(&path).expect("reopen");
        assert_eq!(reopened.len(), 2);
        let got = reopened.get(&laptop_id).expect("laptop present");
        assert_eq!(got.name, "Laptop");
        assert!(got.psk_matches(&laptop_psk));
        assert_eq!(
            got.last_addr,
            Some("192.168.1.7:47654".parse().expect("addr"))
        );
        assert_eq!(got.last_seen_ms, 1_753_900_000_000);
        assert!(reopened.get("nobody").is_none());

        // `list` and `psks` are ordered and agree with each other.
        let listed: Vec<String> = reopened
            .list()
            .iter()
            .map(|p| p.pairing_id.clone())
            .collect();
        let mut expected = vec![laptop_id.clone(), phone_id.clone()];
        expected.sort();
        assert_eq!(listed, expected);
        let psks = reopened.psks();
        assert_eq!(psks.len(), 2);
        assert_eq!(
            psks.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
            expected
        );
        let stored = psks
            .iter()
            .find(|(id, _)| *id == laptop_id)
            .expect("laptop psk");
        assert_eq!(stored.1, laptop_psk);
    }

    #[test]
    fn upsert_replaces_by_pairing_id_and_overwrites_atomically() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");

        let original = peer("Old name");
        let id = original.pairing_id.clone();
        let psk = original.psk;
        store.upsert(original).expect("first write");
        let first_len = std::fs::metadata(&path).expect("meta").len();

        let renamed = Peer {
            pairing_id: id.clone(),
            name: "New name".to_string(),
            psk,
            last_addr: None,
            last_seen_ms: 1_753_900_999_999,
        };
        store.upsert(renamed).expect("second write");

        // One record, not two, and the file was replaced rather than appended.
        assert_eq!(store.len(), 1);
        let reopened = PeerStore::open(&path).expect("reopen");
        assert_eq!(reopened.len(), 1);
        let got = reopened.get(&id).expect("present");
        assert_eq!(got.name, "New name");
        assert_eq!(got.last_addr, None);
        assert_eq!(got.last_seen_ms, 1_753_900_999_999);
        let second_len = std::fs::metadata(&path).expect("meta").len();
        assert!(second_len < first_len + 16, "file grew as if appended");

        // No temporary file was left behind.
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .expect("read dir")
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != DEFAULT_FILE_NAME)
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn remove_reports_whether_anything_went() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        let p = peer("Tablet");
        let id = p.pairing_id.clone();
        store.upsert(p).expect("upsert");

        assert!(store.remove(&id).expect("remove"));
        assert!(!store.remove(&id).expect("second remove"), "already gone");
        assert!(store.is_empty());
        assert!(PeerStore::open(&path).expect("reopen").is_empty());
    }

    #[test]
    fn an_all_zero_psk_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PeerStore::open(&store_path(&dir)).expect("open");
        let bad = Peer {
            pairing_id: "id".to_string(),
            name: "Uninitialised".to_string(),
            psk: [0u8; TOKEN_LEN],
            last_addr: None,
            last_seen_ms: 0,
        };
        assert!(matches!(store.upsert(bad), Err(PeerStoreError::Invalid(_))));
        assert!(store.is_empty());

        // Note: `Peer` has a `Drop`, so `..peer("x")` (functional record update)
        // does not compile — every field is written out. That friction is the
        // point: a PSK should never be moved out of a record by accident.
        let empty_id = Peer {
            pairing_id: String::new(),
            name: "Nameless".to_string(),
            psk: PairingToken::generate().psk(),
            last_addr: None,
            last_seen_ms: 0,
        };
        assert!(matches!(
            store.upsert(empty_id),
            Err(PeerStoreError::Invalid(_))
        ));
    }

    #[test]
    fn concurrent_upserts_all_land() {
        use std::sync::Arc;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = Arc::new(PeerStore::open(&path).expect("open"));

        let mut handles = Vec::new();
        for i in 0..8 {
            let store = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                let mut ids = Vec::new();
                for j in 0..4 {
                    let p = peer(&format!("device-{i}-{j}"));
                    ids.push(p.pairing_id.clone());
                    store.upsert(p).expect("concurrent upsert");
                }
                ids
            }));
        }
        let expected: Vec<String> = handles
            .into_iter()
            .flat_map(|h| h.join().expect("thread"))
            .collect();

        assert_eq!(store.len(), expected.len());
        // Every write is serialised through the lock, so the final file has all
        // of them — no thread's rewrite dropped another's record.
        let reopened = PeerStore::open(&path).expect("reopen");
        assert_eq!(reopened.len(), expected.len());
        for id in expected {
            assert!(reopened.get(&id).is_some(), "lost record {id}");
        }
    }
}
