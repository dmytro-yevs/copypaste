//! Cutting a device off, and sweeping the codes nobody redeemed.
//!
//! Both are refusals rather than deletions, and the difference from
//! [`PeerStore::remove`] is the point: `remove` un-pairs and the same pairing
//! id may be used again, while [`PeerStore::revoke`] records the id so a code
//! someone wrote down, a stale backup of the file, or a peer that keeps
//! re-announcing cannot bring the pairing back (`CopyPaste-gbo`). Pruning is
//! neither — it is hygiene for credentials that were minted and never used.

use super::store::PeerStore;
use super::tentative;
use super::{PeerStoreError, RevokedDevice};

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
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        let removed = guard.peers.remove(pairing_id);
        let deadline = guard.pending.remove(pairing_id);
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
                if let Some(deadline) = deadline {
                    guard.pending.insert(pairing_id.to_string(), deadline);
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
        let pending = std::mem::take(&mut guard.pending);
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
                guard.pending = pending;
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

    /// Erase pairings whose codes were never redeemed and have aged out.
    /// Returns how many went.
    ///
    /// Hygiene, not enforcement: an expired pairing is already refused by every
    /// read path. Run it on a timer so a code that was minted and forgotten does
    /// not sit in a file full of pre-shared keys.
    ///
    /// # Errors
    ///
    /// As [`PeerStore::remove`].
    pub fn prune_expired(&self, now_ms: i64) -> Result<usize, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        let stale: Vec<String> = guard
            .pending
            .iter()
            .filter(|(_, deadline)| **deadline <= now_ms)
            .map(|(id, _)| id.clone())
            .collect();
        if stale.is_empty() {
            return Ok(0);
        }

        let mut removed = Vec::with_capacity(stale.len());
        for pairing_id in &stale {
            let deadline = guard.pending.remove(pairing_id);
            let peer = guard.peers.remove(pairing_id);
            removed.push((pairing_id.clone(), peer, deadline));
        }

        match self.persist(&guard) {
            Ok(()) => {
                for pairing_id in &stale {
                    tentative::bump(&mut guard, pairing_id);
                }
                tracing::info!(count = stale.len(), "dropped unredeemed pairing codes");
                Ok(stale.len())
            }
            Err(err) => {
                for (pairing_id, peer, deadline) in removed {
                    if let Some(peer) = peer {
                        guard.peers.insert(pairing_id.clone(), peer);
                    }
                    if let Some(deadline) = deadline {
                        guard.pending.insert(pairing_id, deadline);
                    }
                }
                Err(err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::now_ms;
    use crate::peers::testutil::{age_out, peer, redeemed, store_path, unredeemed};
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
        let touched = redeemed(&lost, 1_753_999_999_999);
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

    #[test]
    fn pruning_erases_only_the_pairings_that_aged_out() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = store_path(&dir);
        let store = PeerStore::open(&path).expect("open");

        let stale = unredeemed("never claimed");
        let stale_id = stale.pairing_id.clone();
        let fresh = unredeemed("just minted");
        let fresh_id = fresh.pairing_id.clone();
        let live = peer("in use");
        let live_id = live.pairing_id.clone();
        store.upsert(stale).expect("upsert");
        store.upsert(fresh).expect("upsert");
        store.upsert(live).expect("upsert");

        age_out(&path, &stale_id);
        let store = PeerStore::open(&path).expect("reopen");
        assert_eq!(store.prune_expired(now_ms()).expect("prune"), 1);
        assert_eq!(store.prune_expired(now_ms()).expect("prune"), 0);

        let reopened = PeerStore::open(&path).expect("reopen");
        assert!(reopened.get(&stale_id).is_none());
        assert!(reopened.get(&fresh_id).is_some());
        assert!(reopened.get(&live_id).is_some());
        // The PSK is gone from the file, not merely hidden.
        let text = std::fs::read_to_string(&path).expect("read");
        assert!(!text.contains(&stale_id));
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
            last_seen_ms: 0,
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
            last_seen_ms: 0,
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
