//! The in-memory paired-device state, its lock, and write-through to disk.

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;

use zeroize::Zeroizing;

use super::file::{only_last_seen_moved, parse, warn_if_permissive, write_atomically, State};
use super::tentative;
use super::{Peer, PeerStoreError, MAX_PAIRINGS};
use crate::transport::PskCandidate;

/// The paired-device list: backed by a file, cached in memory, shareable behind
/// an `Arc`. Every mutation rewrites the whole file atomically — there are a
/// handful of peers, not a database — and the read methods never touch the
/// disk, so a listener checking PSKs on every inbound connection costs a read
/// lock and nothing else.
pub struct PeerStore {
    path: PathBuf,
    pub(super) state: RwLock<State>,
    unflushed: AtomicBool,
}

impl PeerStore {
    /// Open the store at `path`, creating an empty one in memory if the file
    /// does not exist. Nothing is written until the first mutation.
    ///
    /// # Errors
    ///
    /// `Corrupt` if damaged; `Io` if it could not be read.
    pub fn open(path: &Path) -> Result<Self, PeerStoreError> {
        let state = match std::fs::read(path) {
            Ok(bytes) => {
                warn_if_permissive(path);
                let text = Zeroizing::new(bytes);
                parse(&text)?
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => State::default(),
            Err(err) => return Err(PeerStoreError::Io(err)),
        };
        Ok(Self {
            path: path.to_path_buf(),
            state: RwLock::new(state),
            unflushed: AtomicBool::new(false),
        })
    }

    /// Every peer, ordered by pairing id so callers and tests see a stable sequence.
    #[must_use]
    pub fn list(&self) -> Vec<Peer> {
        let Ok(guard) = self.state.read() else {
            // A poisoned lock cannot be reported through this signature. An
            // empty list is the fail-closed answer: it pairs nobody and trusts
            // nobody. The mutating methods do report it.
            tracing::error!("paired-devices store lock is poisoned");
            return Vec::new();
        };
        let mut peers: Vec<Peer> = guard.peers.values().cloned().collect();
        peers.sort_by(|a, b| a.pairing_id.cmp(&b.pairing_id));
        peers
    }

    #[must_use]
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// One peer by pairing id.
    #[must_use]
    pub fn get(&self, pairing_id: &str) -> Option<Peer> {
        let guard = self.state.read().ok()?;
        guard.peers.get(pairing_id).cloned()
    }

    /// Insert a peer, or replace the one with the same pairing id. The file is
    /// rewritten atomically before this returns, and a failed write rolls the
    /// in-memory state back to what is on disk, so a caller is never told a
    /// pairing was saved when it was not.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Revoked`] if this pairing was cut off — a revoked
    /// pairing id never comes back, so a code someone kept cannot re-add it;
    /// `TooManyPairings` if this would be a *new* pairing past
    /// [`MAX_PAIRINGS`], which never applies to one already stored;
    /// `Invalid` if the record does not validate, in which case nothing is
    /// written; `Io` if the file could not be replaced; `Poisoned` if another
    /// thread panicked holding the lock.
    pub fn upsert(&self, peer: Peer) -> Result<(), PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        self.upsert_locked(&mut guard, peer)
    }

    /// [`Self::upsert`] with the write lock already held, so a caller that has
    /// to compare something against the current state can do it in the same
    /// critical section as the write (see [`super::tentative`]).
    pub(super) fn upsert_locked(
        &self,
        guard: &mut State,
        peer: Peer,
    ) -> Result<(), PeerStoreError> {
        peer.validate()?;
        let pairing_id = peer.pairing_id.clone();
        if guard.revoked.contains_key(&pairing_id) {
            return Err(PeerStoreError::Revoked);
        }
        // Only an id that is not here yet is capped: refusing an update would
        // strand every stored device the moment the list filled, because the
        // end of every successful session writes one.
        if !guard.peers.contains_key(&pairing_id) && guard.peers.len() >= MAX_PAIRINGS {
            return Err(PeerStoreError::TooManyPairings);
        }

        if only_last_seen_moved(guard, &peer) {
            guard.peers.insert(pairing_id.clone(), peer);
            tentative::bump(guard, &pairing_id);
            self.unflushed.store(true, Ordering::Release);
            return Ok(());
        }

        let previous = guard.peers.insert(pairing_id.clone(), peer);

        match self.persist(guard) {
            Ok(()) => {
                tentative::bump(guard, &pairing_id);
                tracing::debug!(%pairing_id, "paired device saved");
                Ok(())
            }
            Err(err) => {
                match previous {
                    Some(old) => guard.peers.insert(pairing_id.clone(), old),
                    None => guard.peers.remove(&pairing_id),
                };
                Err(err)
            }
        }
    }

    /// [`Self::upsert`] for observations about a pairing that is already
    /// stored — name, address and last-seen off a finished session. Returns
    /// whether the slot was still there to write to.
    ///
    /// A session outlives the click that ends it, and `upsert` bars only
    /// *revoked* ids: a pass that read its `Peer` before an unpair wrote that
    /// record back and re-enrolled the pairing, key and all. The presence test
    /// shares the write lock, so no caller can check first and lose the race.
    ///
    /// # Errors
    ///
    /// As [`Self::upsert`]; an absent pairing is `Ok(false)`.
    pub fn touch(&self, peer: Peer) -> Result<bool, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        if !guard.peers.contains_key(&peer.pairing_id) {
            return Ok(false);
        }
        self.upsert_locked(&mut guard, peer).map(|()| true)
    }

    /// Forget a peer. Returns whether one was there to forget.
    ///
    /// Local and reversible: the pairing id may be used again by a fresh
    /// pairing. Use [`PeerStore::revoke`] for a device that should never be
    /// able to reconnect.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Io`] if the file could not be replaced — the peer stays
    /// in memory, matching what is still on disk; `Poisoned` if another thread
    /// panicked holding the lock.
    pub fn remove(&self, pairing_id: &str) -> Result<bool, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        let Some(removed) = guard.peers.remove(pairing_id) else {
            return Ok(false);
        };
        match self.persist(&guard) {
            Ok(()) => {
                tentative::bump(&mut guard, pairing_id);
                tracing::debug!(%pairing_id, "paired device removed");
                Ok(true)
            }
            Err(err) => {
                guard.peers.insert(pairing_id.to_string(), removed);
                Err(err)
            }
        }
    }

    /// Every pairing id and PSK, for a responder to try against an
    /// inbound handshake ([`crate::Session::accept_any`]).
    ///
    /// Ordered by pairing id, so the trial order does not depend on hash-map
    /// iteration and is therefore reproducible. Revoked pairings have no peer
    /// record and therefore no candidate.
    ///
    /// # Security
    ///
    /// This runs on every inbound connection, before the dialler has proved
    /// anything, so an attacker who never completes a handshake decides how
    /// often the whole key set is copied. Two things keep those copies out of
    /// freed heap (security review F-8):
    ///
    /// * The return type is `Zeroizing`, so the wipe cannot be lost by a caller
    ///   binding it to a plain local, and each [`PskCandidate`] wipes itself on
    ///   drop as well.
    /// * The buffer is allocated once, up front, and never grown. A `Vec` that
    ///   grows memcpies the keys it already holds into a new allocation and
    ///   frees the old one intact, where no `Drop` will ever reach them.
    #[must_use]
    pub fn psks(&self) -> Zeroizing<Vec<PskCandidate>> {
        let Ok(guard) = self.state.read() else {
            // Fail closed, as `list` does: no candidates authenticates nobody.
            tracing::error!("paired-devices store lock is poisoned");
            return Zeroizing::new(Vec::new());
        };
        let mut candidates = Vec::with_capacity(guard.peers.len());
        for peer in guard.peers.values() {
            candidates.push(PskCandidate {
                pairing_id: peer.pairing_id.clone(),
                // `[u8; 32]` is `Copy`, so this reads the field rather than
                // moving out of a type that has a `Drop`.
                psk: peer.psk,
            });
        }
        candidates.sort_by(|a, b| a.pairing_id.cmp(&b.pairing_id));
        Zeroizing::new(candidates)
    }

    /// Number of paired devices.
    #[must_use]
    pub fn len(&self) -> usize {
        let Ok(guard) = self.state.read() else {
            tracing::error!("paired-devices store lock is poisoned");
            return 0;
        };
        guard.peers.len()
    }

    /// Whether this device is paired with anything.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn flush(&self) -> Result<(), PeerStoreError> {
        if !self.unflushed.load(Ordering::Acquire) {
            return Ok(());
        }
        let guard = self.state.read().map_err(|_| PeerStoreError::Poisoned)?;
        self.persist(&guard)
    }

    pub(super) fn persist(&self, state: &State) -> Result<(), PeerStoreError> {
        let written = write_atomically(&self.path, state);
        if written.is_ok() {
            self.unflushed.store(false, Ordering::Release);
        }
        written
    }

    /// [`Self::persist`] for a caller that keeps its in-memory change whatever
    /// the disk does. Marking the store unflushed is what lets the next
    /// successful write — or [`Self::flush`] — repair the file.
    pub(super) fn persist_unflushed(&self, state: &State) -> Result<(), PeerStoreError> {
        let written = self.persist(state);
        if written.is_err() {
            self.unflushed.store(true, Ordering::Release);
        }
        written
    }
}

/// Peer count only. The path is not printed — `AGENTS.md` rule 4 keeps paths
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
    use crate::peers::testutil::{peer, store_path, with_last_seen};
    use crate::peers::DEFAULT_FILE_NAME;
    use crate::transport::{PairingToken, TOKEN_LEN};

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
            psks.iter()
                .map(|c| c.pairing_id.clone())
                .collect::<Vec<_>>(),
            expected
        );
        let stored = psks
            .iter()
            .find(|c| c.pairing_id == laptop_id)
            .expect("laptop psk");
        assert_eq!(stored.psk, laptop_psk);
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

    /// The end of every successful session writes a fresh `last_seen_ms` and
    /// nothing else. That must not cost two `fsync`s and a directory `fsync`;
    /// the value is still live to every reader, and the next write carries it.
    #[test]
    fn a_repeat_session_does_not_rewrite_the_file_for_last_seen_alone() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");

        let established = peer("Laptop");
        let id = established.pairing_id.clone();
        let touched = with_last_seen(&established, 1_753_999_999_999);
        store.upsert(established).expect("first session");
        let on_disk = std::fs::read(&path).expect("read");

        store.upsert(touched).expect("second session");
        assert_eq!(
            std::fs::read(&path).expect("read"),
            on_disk,
            "a fresh last_seen_ms must not cost a write"
        );
        assert_eq!(
            store.get(&id).expect("present").last_seen_ms,
            1_753_999_999_999,
            "the reader must still see it"
        );

        store.flush().expect("flush");
        assert_eq!(
            PeerStore::open(&path)
                .expect("reopen")
                .get(&id)
                .expect("present")
                .last_seen_ms,
            1_753_999_999_999
        );
        // And a flush with nothing outstanding writes nothing at all.
        let flushed = std::fs::read(&path).expect("read");
        store.flush().expect("second flush");
        assert_eq!(std::fs::read(&path).expect("read"), flushed);
    }

    /// A name or an address arriving with a session is not telemetry: the peer
    /// list renders it and the next dial uses it, so it is written now.
    #[test]
    fn a_changed_name_or_address_still_writes_through() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");

        let first = peer("placeholder");
        let id = first.pairing_id.clone();
        let psk = first.psk;
        let addr = first.last_addr;
        store.upsert(first).expect("upsert");

        store
            .upsert(Peer {
                pairing_id: id.clone(),
                name: "the name off the wire".to_string(),
                psk,
                last_addr: addr,
                last_seen_ms: 1_753_999_999_999,
            })
            .expect("session");
        assert_eq!(
            PeerStore::open(&path)
                .expect("reopen")
                .get(&id)
                .expect("present")
                .name,
            "the name off the wire"
        );

        store
            .upsert(Peer {
                pairing_id: id.clone(),
                name: "the name off the wire".to_string(),
                psk,
                last_addr: Some("192.168.1.9:47654".parse().expect("addr")),
                last_seen_ms: 1_754_000_000_000,
            })
            .expect("moved");
        assert_eq!(
            PeerStore::open(&path)
                .expect("reopen")
                .get(&id)
                .expect("present")
                .last_addr,
            Some("192.168.1.9:47654".parse().expect("addr"))
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

    /// `touch` records, it never enrols: the record a finished session carries
    /// was read before the unpair, and writing it back would hand the pairing —
    /// and its key — straight back to a device the user had just cut off.
    #[test]
    fn touching_a_pairing_that_is_gone_writes_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        let laptop = peer("Laptop");
        let id = laptop.pairing_id.clone();
        store.upsert(laptop.clone()).expect("upsert");

        assert!(store
            .touch(with_last_seen(&laptop, 1_754_000_000_000))
            .expect("touch"));
        assert_eq!(
            store.get(&id).expect("present").last_seen_ms,
            1_754_000_000_000
        );

        assert!(store.remove(&id).expect("remove"));
        assert!(
            !store
                .touch(with_last_seen(&laptop, 1_754_000_000_001))
                .expect("touch"),
            "the session put the pairing back"
        );
        assert!(store.get(&id).is_none());
        assert!(store.psks().is_empty());
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
            last_seen_ms: 1,
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
            last_seen_ms: 1,
        };
        assert!(matches!(
            store.upsert(empty_id),
            Err(PeerStoreError::Invalid(_))
        ));
    }

    /// Security review F-8. The listener copies this set on every inbound
    /// connection, before anything is authenticated, so the copies have to wipe
    /// themselves. Pinned as a type annotation (port manifest 02, I-12): a
    /// `psks` that went back to a plain `Vec` fails to compile here.
    #[test]
    fn psks_are_handed_out_wiped_on_drop() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PeerStore::open(&store_path(&dir)).expect("open");
        store.upsert(peer("Laptop")).expect("upsert");

        let candidates: Zeroizing<Vec<PskCandidate>> = store.psks();
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates.capacity(),
            candidates.len(),
            "the candidate list must be sized once, not grown"
        );
    }

    /// Security review F-13. The bound is a refusal, and refusing must never be
    /// a way to lose a device the user still owns (`AGENTS.md` rule 4).
    #[test]
    fn the_pairing_list_is_capped_by_refusing_not_by_evicting() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");

        let ids: Vec<String> = (0..MAX_PAIRINGS)
            .map(|i| {
                let p = peer(&format!("device-{i}"));
                let id = p.pairing_id.clone();
                store.upsert(p).expect("up to the cap must fit");
                id
            })
            .collect();

        let refused = peer("one too many");
        let refused_id = refused.pairing_id.clone();
        assert!(matches!(
            store.upsert(refused),
            Err(PeerStoreError::TooManyPairings)
        ));

        // Nothing was made room for: every device the user has is still here,
        // still authenticates, and survives a restart.
        assert_eq!(store.len(), MAX_PAIRINGS);
        assert_eq!(store.psks().len(), MAX_PAIRINGS);
        assert!(store.get(&refused_id).is_none());
        let reopened = PeerStore::open(&path).expect("reopen");
        for id in &ids {
            assert!(reopened.get(id).is_some(), "lost {id}");
        }

        // An *existing* pairing is never refused, or the end-of-session write
        // would fail for every device once the list filled.
        let established = reopened.get(&ids[0]).expect("present");
        reopened
            .upsert(Peer {
                pairing_id: ids[0].clone(),
                name: "renamed at the cap".to_string(),
                psk: established.psk,
                last_addr: None,
                last_seen_ms: 1_753_900_000_001,
            })
            .expect("updating a stored pairing must still work at the cap");
        assert_eq!(
            reopened.get(&ids[0]).expect("present").name,
            "renamed at the cap"
        );

        // And the remedy the error names actually works.
        assert!(reopened.remove(&ids[1]).expect("unpair"));
        reopened
            .upsert(peer("the replacement"))
            .expect("a freed slot must be usable");
    }

    #[test]
    fn concurrent_upserts_all_land() {
        use std::sync::Arc;

        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = Arc::new(PeerStore::open(&path).expect("open"));

        let mut handles = Vec::new();
        // Eight threads writing four pairings each would be 32, past
        // `MAX_PAIRINGS`; the property under test is that no thread's rewrite
        // drops another's record, which needs concurrency, not volume.
        let threads = MAX_PAIRINGS / 4;
        for i in 0..threads {
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
