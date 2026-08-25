//! Cutting a device off permanently.
//!
//! The difference from [`PeerStore::remove`] is the point: `remove` un-pairs
//! and allows the id to be paired again, while [`PeerStore::revoke`] records the
//! id so a retained credential or a peer that keeps re-announcing cannot bring
//! the pairing back (`CopyPaste-gbo`).

use super::peer::validate_pairing_id;
use super::store::PeerStore;
use super::tentative;
use super::{PeerStoreError, RevokedDevice, MAX_REVOCATIONS};

impl PeerStore {
    /// Cut a device off for good: drop its key **and** record the pairing id so
    /// it can never be added again (`CopyPaste-gbo`; see this module's header).
    ///
    /// Returns whether a peer was there to cut off. Recording the revocation is
    /// unconditional, so revoking an id this device has not yet seen still
    /// refuses it later.
    ///
    /// # Errors
    ///
    /// As [`PeerStore::remove`].
    pub fn revoke(&self, pairing_id: &str, now_ms: i64) -> Result<bool, PeerStoreError> {
        // Checked here as well as on load: `revoke` is the one way an id that
        // was never a peer's reaches the list, so an unvalidated caller could
        // write a file this store then refuses to open.
        validate_pairing_id(pairing_id)?;
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        if !guard.revoked.contains_key(pairing_id) && guard.revoked.len() >= MAX_REVOCATIONS {
            return Err(PeerStoreError::TooManyRevocations);
        }
        let removed = guard.peers.remove(pairing_id);
        let previous_revocation = guard.revoked.insert(pairing_id.to_string(), now_ms);

        match self.persist(&guard) {
            Ok(()) => {
                tentative::bump(&mut guard, pairing_id);
                tracing::info!(%pairing_id, "revoked a paired device");
                Ok(removed.is_some())
            }
            Err(err) => {
                if let Some(peer) = removed {
                    guard.peers.insert(pairing_id.to_string(), peer);
                }
                match previous_revocation {
                    Some(at) => guard.revoked.insert(pairing_id.to_string(), at),
                    None => guard.revoked.remove(pairing_id),
                };
                Err(err)
            }
        }
    }

    /// [`PeerStore::revoke`] for every pairing at once, in one file write.
    /// Returns how many peers were cut off. An empty store is a success.
    ///
    /// # Errors
    ///
    /// As [`PeerStore::remove`]. On failure nothing is revoked.
    pub fn revoke_all(&self, now_ms: i64) -> Result<usize, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        if guard.peers.is_empty() {
            return Ok(0);
        }
        let peers = std::mem::take(&mut guard.peers);
        for pairing_id in peers.keys() {
            guard.revoked.insert(pairing_id.clone(), now_ms);
        }

        match self.persist(&guard) {
            Ok(()) => {
                for pairing_id in peers.keys() {
                    tentative::bump(&mut guard, pairing_id);
                }
                tracing::info!(revoked = peers.len(), "revoked every paired device");
                Ok(peers.len())
            }
            Err(err) => {
                for pairing_id in peers.keys() {
                    guard.revoked.remove(pairing_id);
                }
                guard.peers = peers;
                Err(err)
            }
        }
    }

    /// The revocation audit trail, oldest pairing id first.
    #[must_use]
    pub fn revoked(&self) -> Vec<RevokedDevice> {
        let Ok(guard) = self.state.read() else {
            return Vec::new();
        };
        guard
            .revoked
            .iter()
            .map(|(pairing_id, revoked_at_ms)| RevokedDevice {
                pairing_id: pairing_id.clone(),
                revoked_at_ms: *revoked_at_ms,
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::peers::testutil::{peer, store_path, with_last_seen};
    use crate::peers::Peer;

    /// The lazy path is for `last_seen_ms` and nothing else. A revocation that
    /// only reached the disk on the next mutation would let a device the user
    /// cut off come back after a crash (`CopyPaste-gbo`).
    #[test]
    fn revoking_after_a_deferred_touch_is_durable_immediately() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");

        let lost = peer("stolen phone");
        let id = lost.pairing_id.clone();
        let touched = with_last_seen(&lost, 1_753_999_999_999);
        store.upsert(lost).expect("upsert");
        store.upsert(touched).expect("session");

        assert!(store.revoke(&id, 1_753_900_000_000).expect("revoke"));

        let reopened = PeerStore::open(&path).expect("reopen");
        assert!(
            reopened.get(&id).is_none(),
            "the pairing survived a restart"
        );
        assert!(reopened.psks().is_empty());
        assert_eq!(reopened.revoked().len(), 1);
    }

    /// Finding 7 / `CopyPaste-gbo`. `remove` un-pairs; `revoke` cuts a device
    /// off, and the difference is whether the pairing can come back.
    #[test]
    fn a_revoked_pairing_cannot_be_added_again() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");

        let lost = peer("stolen phone");
        let id = lost.pairing_id.clone();
        let psk = lost.psk;
        store.upsert(lost).expect("upsert");

        assert!(store.revoke(&id, 1_753_900_000_000).expect("revoke"));
        assert!(store.get(&id).is_none());
        assert!(store.psks().is_empty(), "the key must leave the candidates");
        assert_eq!(
            store.revoked(),
            vec![RevokedDevice {
                pairing_id: id.clone(),
                revoked_at_ms: 1_753_900_000_000,
            }]
        );

        // The device still holds a working code; saving it again must fail.
        let returning = Peer {
            pairing_id: id.clone(),
            name: "stolen phone".to_string(),
            psk,
            last_addr: None,
            last_seen_ms: 1,
        };
        assert!(matches!(
            store.upsert(returning),
            Err(PeerStoreError::Revoked)
        ));
        assert!(store.psks().is_empty());

        // And it survives a restart, which is when a stale file would undo it.
        let reopened = PeerStore::open(&path).expect("reopen");
        assert!(reopened.get(&id).is_none());
        assert_eq!(reopened.revoked().len(), 1);
        assert!(!reopened.revoke(&id, 1).expect("idempotent"));

        // Whereas `remove` leaves the pairing id usable.
        let ordinary = peer("laptop");
        let ordinary_id = ordinary.pairing_id.clone();
        let again = peer("laptop");
        let again = Peer {
            pairing_id: ordinary_id.clone(),
            name: again.name.clone(),
            psk: again.psk,
            last_addr: None,
            last_seen_ms: 1,
        };
        reopened.upsert(ordinary).expect("upsert");
        reopened.remove(&ordinary_id).expect("remove");
        reopened
            .upsert(again)
            .expect("re-pairing after unpair is fine");
    }

    #[test]
    fn revoking_everything_cuts_off_every_device_in_one_write() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");
        assert_eq!(store.revoke_all(1).expect("empty is a success"), 0);

        let ids: Vec<String> = ["a", "b", "c"]
            .iter()
            .map(|name| {
                let p = peer(name);
                let id = p.pairing_id.clone();
                store.upsert(p).expect("upsert");
                id
            })
            .collect();

        assert_eq!(store.revoke_all(1_753_900_000_000).expect("revoke all"), 3);
        assert!(store.psks().is_empty());
        assert_eq!(store.revoked().len(), 3);

        let reopened = PeerStore::open(&path).expect("reopen");
        for id in &ids {
            assert!(reopened.get(id).is_none());
        }
        let text = std::fs::read_to_string(&path).expect("read");
        for id in &ids {
            assert!(text.contains(id), "the audit trail must survive");
        }
    }

    /// `revoke` is the only way an id that was never a peer's reaches the list,
    /// so it is where the shape is checked. Recording one the store would then
    /// refuse to open is the failure this prevents.
    #[test]
    fn revoking_an_id_no_peer_could_carry_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PeerStore::open(&store_path(&dir)).expect("open");

        for bad in ["", &"a".repeat(129)] {
            assert!(
                matches!(store.revoke(bad, 1), Err(PeerStoreError::Invalid(_))),
                "an invalid pairing id was revoked"
            );
        }
        assert!(
            store.revoked().is_empty(),
            "a refused revocation was recorded anyway"
        );
    }

    /// Revoking an id this device has not seen still refuses it later — which is
    /// what "revoke the device I lost before it syncs here" needs.
    #[test]
    fn revoking_an_unknown_pairing_still_bars_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PeerStore::open(&store_path(&dir)).expect("open");
        let future = peer("not here yet");
        let id = future.pairing_id.clone();

        assert!(
            !store.revoke(&id, 1).expect("revoke"),
            "nothing was removed"
        );
        assert!(matches!(store.upsert(future), Err(PeerStoreError::Revoked)));
    }
}
