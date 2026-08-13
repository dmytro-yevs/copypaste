//! The two-phase pairing write: save a peer before the other device has
//! agreed, then undo it if the agreement never lands.
//!
//! [`PeerStore::upsert`] and [`PeerStore::remove`] cannot do the undo. It has
//! to be **conditional**, because an established session ending inside the
//! commit deadline writes the same pairing through `Node::touch_peer` — an
//! unconditional restore discards that write, and an unconditional `remove`
//! deletes a device the user still owns (B5). And it has to **fail closed**:
//! those two put the in-memory record back when the file cannot be written,
//! but [`PeerStore::psks`] authenticates inbound handshakes, so a repudiated
//! PSK must stop being offered even while the file still names it (B4).

use super::file::State;
use super::store::PeerStore;
use super::{Peer, PeerStoreError};

/// How many times this pairing's slot has been written. Per pairing, or an
/// unrelated device ending a session would read as contention on this one.
pub type Generation = u64;

/// A pairing slot as it stood before a ceremony wrote to it.
#[derive(Debug)]
pub struct PeerSnapshot {
    pairing_id: String,
    previous: Option<Peer>,
    /// Restoring a peer without its deadline makes an unredeemed code permanent.
    pending_until: Option<i64>,
    /// What this ceremony last wrote, or what it read.
    generation: Generation,
}

impl PeerSnapshot {
    #[must_use]
    pub fn pairing_id(&self) -> &str {
        &self.pairing_id
    }

    #[must_use]
    pub fn had_peer(&self) -> bool {
        self.previous.is_some()
    }
}

/// What a rollback managed to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rollback {
    /// Memory and file both say what they said before the ceremony.
    Undone,
    /// Another writer had replaced the slot. Its observations were kept; the
    /// credential is the one from before.
    UndoneOverContention,
}

pub(super) fn generation(state: &State, pairing_id: &str) -> Generation {
    state.generations.get(pairing_id).copied().unwrap_or(0)
}

pub(super) fn bump(state: &mut State, pairing_id: &str) {
    let next = generation(state, pairing_id).wrapping_add(1);
    state.generations.insert(pairing_id.to_string(), next);
}

impl PeerStore {
    /// Read a pairing slot with the generation it was read at. Unlike
    /// [`PeerStore::get`] it does not hide an aged-out code: a rollback has to
    /// put back exactly what was there, deadline included.
    #[must_use]
    pub fn snapshot(&self, pairing_id: &str) -> PeerSnapshot {
        let Ok(guard) = self.state.read() else {
            tracing::error!("paired-devices store lock is poisoned");
            return PeerSnapshot {
                pairing_id: pairing_id.to_string(),
                previous: None,
                pending_until: None,
                generation: 0,
            };
        };
        PeerSnapshot {
            pairing_id: pairing_id.to_string(),
            previous: guard.peers.get(pairing_id).cloned(),
            pending_until: guard.pending.get(pairing_id).copied(),
            generation: generation(&guard, pairing_id),
        }
    }

    /// [`PeerStore::upsert`], refused unless the slot is still what `snapshot`
    /// read. On success `snapshot` advances, so [`Self::restore`] compares
    /// against this write and not against the original read.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Contended`] when another write landed first;
    /// otherwise as [`PeerStore::upsert`].
    pub fn upsert_from(
        &self,
        snapshot: &mut PeerSnapshot,
        peer: Peer,
    ) -> Result<(), PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        if generation(&guard, &snapshot.pairing_id) != snapshot.generation {
            return Err(PeerStoreError::Contended);
        }
        self.upsert_locked(&mut guard, peer)?;
        snapshot.generation = generation(&guard, &snapshot.pairing_id);
        Ok(())
    }

    /// Put the slot back the way [`Self::snapshot`] found it. Never a pairing
    /// that was revoked meanwhile: that would hand the id back.
    ///
    /// # Errors
    ///
    /// [`PeerStoreError::Io`] when the file could not be written. Memory is
    /// still rolled back, so the caller reports that the cleanup did not reach
    /// the disk, not that it did not happen.
    pub fn restore(&self, snapshot: &PeerSnapshot) -> Result<Rollback, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        let id = snapshot.pairing_id.clone();
        let contended = generation(&guard, &id) != snapshot.generation;
        let revoked = guard.revoked.contains_key(&id);

        guard.pending.remove(&id);
        let restored = match (&snapshot.previous, revoked) {
            (_, true) | (None, _) => None,
            // The other writer's observations are newer than the snapshot's
            // and are kept; its credential is the repudiated one and is not.
            (Some(previous), false) if contended => guard.peers.get(&id).map(|current| Peer {
                pairing_id: id.clone(),
                name: current.name.clone(),
                psk: previous.psk,
                last_addr: current.last_addr,
                last_seen_ms: current.last_seen_ms,
            }),
            (Some(previous), false) => Some(previous.clone()),
        };
        match restored {
            Some(peer) => {
                guard.peers.insert(id.clone(), peer);
                if let Some(deadline) = snapshot.pending_until {
                    guard.pending.insert(id.clone(), deadline);
                }
            }
            None => {
                guard.peers.remove(&id);
            }
        }
        bump(&mut guard, &id);

        let outcome = if contended {
            Rollback::UndoneOverContention
        } else {
            Rollback::Undone
        };
        // Not reverted on a write failure: the repudiated key must stop
        // authenticating now.
        self.persist_unflushed(&guard).map(|()| outcome)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::testutil::{peer, redeemed, store_path};
    use crate::transport::PairingToken;

    fn replacement(pairing_id: &str) -> Peer {
        Peer {
            pairing_id: pairing_id.to_string(),
            name: "the new ceremony".to_string(),
            psk: PairingToken::generate().psk(),
            last_addr: None,
            last_seen_ms: crate::now_ms(),
        }
    }

    #[test]
    fn an_uncontended_rollback_puts_the_previous_record_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        let established = peer("Laptop");
        let id = established.pairing_id.clone();
        let original_psk = established.psk;
        store.upsert(established).expect("upsert");

        let mut snapshot = store.snapshot(&id);
        store
            .upsert_from(&mut snapshot, replacement(&id))
            .expect("tentative write");
        assert_eq!(
            store.restore(&snapshot).expect("rollback"),
            Rollback::Undone
        );

        let back = store
            .get(&id)
            .expect("the established pairing must survive");
        assert!(back.psk_matches(&original_psk));
        assert_eq!(back.name, "Laptop");
        assert!(PeerStore::open(&path)
            .expect("reopen")
            .get(&id)
            .expect("present")
            .psk_matches(&original_psk));
    }

    /// B5. `Node::touch_peer` writes name, address and last-seen at the end of
    /// every established session, and does not share the ceremony's lock. A
    /// rollback must not discard that write, and must not leave its PSK — which
    /// is the replacement, because a touch copies the stored key forward.
    #[test]
    fn a_concurrent_session_write_keeps_its_observations_and_loses_the_new_key() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PeerStore::open(&store_path(&dir)).expect("open");
        let established = peer("Laptop");
        let id = established.pairing_id.clone();
        let original_psk = established.psk;
        store.upsert(established).expect("upsert");

        let mut snapshot = store.snapshot(&id);
        let tentative = replacement(&id);
        let tentative_psk = tentative.psk;
        store
            .upsert_from(&mut snapshot, tentative)
            .expect("tentative write");

        // A session that started before the ceremony ends during it.
        let touched = store.get(&id).expect("present");
        store
            .upsert(Peer {
                pairing_id: id.clone(),
                name: "the name off the wire".to_string(),
                psk: touched.psk,
                last_addr: Some("192.168.1.9:47654".parse().expect("addr")),
                last_seen_ms: 1_754_000_000_000,
            })
            .expect("touch");

        assert_eq!(
            store.restore(&snapshot).expect("rollback"),
            Rollback::UndoneOverContention
        );
        let back = store.get(&id).expect("present");
        assert!(back.psk_matches(&original_psk), "the old key must be back");
        assert!(
            !back.psk_matches(&tentative_psk),
            "the new key is still live"
        );
        assert_eq!(back.name, "the name off the wire");
        assert_eq!(
            back.last_addr,
            Some("192.168.1.9:47654".parse().expect("addr"))
        );
    }

    /// The ceremony decided from a record that has since been replaced. Writing
    /// its tentative peer over that would lose the replacement.
    #[test]
    fn a_tentative_write_is_refused_when_the_slot_moved_first() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PeerStore::open(&store_path(&dir)).expect("open");
        let established = peer("Laptop");
        let id = established.pairing_id.clone();
        store.upsert(established).expect("upsert");

        let mut snapshot = store.snapshot(&id);
        store
            .upsert(redeemed(
                &store.get(&id).expect("present"),
                1_754_000_000_001,
            ))
            .expect("someone else");

        assert!(matches!(
            store.upsert_from(&mut snapshot, replacement(&id)),
            Err(PeerStoreError::Contended)
        ));
    }

    /// B4. The file cannot be written, so the rollback cannot be made durable —
    /// but the repudiated key must stop authenticating *now*, because
    /// `psks` reads memory and the listener reads `psks`.
    #[test]
    fn a_rollback_that_cannot_be_written_still_takes_the_key_out_of_memory() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        let mut snapshot = store.snapshot("fresh-pairing");
        let tentative = Peer {
            pairing_id: "fresh-pairing".to_string(),
            name: "the other device".to_string(),
            psk: PairingToken::generate().psk(),
            last_addr: None,
            last_seen_ms: crate::now_ms(),
        };
        let tentative_psk = tentative.psk;
        store
            .upsert_from(&mut snapshot, tentative)
            .expect("tentative write");
        assert!(!store.psks().is_empty(), "the tentative key must be live");

        // A directory now stands where the file was. Renaming the replacement
        // over it fails on every platform, and the parent still exists, so the
        // write path cannot recreate its way out of it.
        std::fs::remove_file(&path).expect("remove");
        std::fs::create_dir(&path).expect("block the path");

        assert!(
            matches!(store.restore(&snapshot), Err(PeerStoreError::Io(_))),
            "a rollback that did not reach the disk must say so"
        );
        assert!(
            store
                .psks()
                .iter()
                .all(|candidate| candidate.psk != tentative_psk),
            "the repudiated key still authenticates"
        );
        assert!(store.get("fresh-pairing").is_none());
    }

    /// Revocation is final. A ceremony that raced one must not put the pairing
    /// back on its way out.
    #[test]
    fn a_rollback_never_restores_a_pairing_that_was_revoked_meanwhile() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PeerStore::open(&store_path(&dir)).expect("open");
        let established = peer("stolen phone");
        let id = established.pairing_id.clone();
        store.upsert(established).expect("upsert");

        let mut snapshot = store.snapshot(&id);
        store
            .upsert_from(&mut snapshot, replacement(&id))
            .expect("tentative write");
        store.revoke(&id, crate::now_ms()).expect("revoke");

        store.restore(&snapshot).expect("rollback");
        assert!(store.get(&id).is_none());
        assert!(store.psks().is_empty());
    }
}
