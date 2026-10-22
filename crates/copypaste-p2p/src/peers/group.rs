//! Atomic membership and transport-key projection into the existing peer file.
use super::{file::State, tentative, PeerStore, PeerStoreError, MAX_PAIRINGS};
use crate::{
    group::{GroupSnapshot, GroupState},
    DeviceProfile,
};
pub(super) struct MembershipKeys {
    revision: u64,
    keys: zeroize::Zeroizing<Vec<crate::PskCandidate>>,
}

impl PeerStore {
    pub(crate) fn stored_group(&self) -> Result<GroupSnapshot, PeerStoreError> {
        self.state
            .read()
            .map_err(|_| PeerStoreError::Poisoned)?
            .group
            .as_ref()
            .ok_or(PeerStoreError::Invalid("group is not initialized"))?
            .snapshot()
    }
    pub(crate) fn identity_tag(&self, device_id: Option<&str>) -> Option<String> {
        self.state
            .read()
            .ok()?
            .group
            .as_ref()?
            .identity_tag(device_id)
    }
    pub(crate) fn metadata_key(&self, device_id: &str) -> Option<zeroize::Zeroizing<[u8; 32]>> {
        self.state
            .read()
            .ok()?
            .group
            .as_ref()?
            .metadata_key(device_id)
            .ok()
    }
    pub(crate) fn metadata_psks(&self) -> zeroize::Zeroizing<Vec<crate::PskCandidate>> {
        let revision = self.group_revision();
        let cache = self
            .metadata_cache
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(cached) = cache.as_ref().filter(|cached| cached.revision == revision) {
            return zeroize::Zeroizing::new(
                cached
                    .keys
                    .iter()
                    .map(|key| crate::PskCandidate {
                        pairing_id: key.pairing_id.clone(),
                        psk: key.psk,
                    })
                    .collect(),
            );
        }
        drop(cache);
        let Ok(state) = self.state.read() else {
            return zeroize::Zeroizing::new(Vec::new());
        };
        let mut result = Vec::with_capacity(state.peers.len());
        if let Some(group) = &state.group {
            for peer in state.peers.values() {
                if let Some(key) = peer
                    .device_id
                    .as_deref()
                    .and_then(|id| group.metadata_key(id).ok())
                {
                    result.push(crate::PskCandidate {
                        pairing_id: peer.pairing_id.clone(),
                        psk: *key,
                    });
                }
            }
        }
        let copied = zeroize::Zeroizing::new(
            result
                .iter()
                .map(|key| crate::PskCandidate {
                    pairing_id: key.pairing_id.clone(),
                    psk: key.psk,
                })
                .collect(),
        );
        drop(state);
        let mut cache = self
            .metadata_cache
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.group_revision() == revision {
            *cache = Some(MembershipKeys {
                revision,
                keys: zeroize::Zeroizing::new(result),
            });
        }
        copied
    }
    pub fn group_snapshot(
        &self,
        id: &str,
        name: &str,
        profile: DeviceProfile,
        addr: Option<std::net::SocketAddr>,
    ) -> Result<GroupSnapshot, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        if let Some(group) = &guard.group {
            if group.local_matches(id, name, &profile, addr) {
                return group.snapshot();
            }
        }
        let mut next = guard.clone();
        let changed = if let Some(group) = &mut next.group {
            group.update_local(id, name, profile, addr)?
        } else {
            next.group = Some(GroupState::new(id, name, profile, addr)?);
            true
        };
        let snapshot = next
            .group
            .as_ref()
            .ok_or(PeerStoreError::Corrupt)?
            .snapshot()?;
        if changed {
            snapshot.validate()?;
            self.persist(&next)?;
            *guard = next;
            self.group_changed();
        }
        Ok(snapshot)
    }

    /// Caller must authenticate either the existing pairing/SAS ceremony or
    /// the account plus sync key before passing a snapshot to this method.
    pub fn merge_group(
        &self,
        snapshot: &GroupSnapshot,
        pairing_id: Option<&str>,
        explicit: bool,
    ) -> Result<bool, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        let mut next = guard.clone();
        let Some(group) = &mut next.group else {
            return Err(PeerStoreError::Invalid("group is not initialized"));
        };
        let mut bound_identity = false;
        if let Some(id) = pairing_id {
            let peer = next.peers.get_mut(id).ok_or(PeerStoreError::Revoked)?;
            if peer
                .device_id
                .as_ref()
                .is_some_and(|id| id != &snapshot.roster.issuer)
            {
                return Err(PeerStoreError::Invalid(
                    "group identity does not match pairing",
                ));
            }
            bound_identity = peer.device_id.is_none();
            peer.device_id = Some(snapshot.roster.issuer.clone());
        }
        let fresh_sas = pairing_id.is_some_and(|id| next.group_admissions.contains(id));
        let changed = group.merge(snapshot, explicit || fresh_sas)?;
        if !changed && !fresh_sas && !bound_identity {
            return Ok(false);
        }
        if let Some(id) = pairing_id {
            next.group_admissions.remove(id);
        }
        project(&mut next, guard.group.as_ref())?;
        // Binding a legacy pairing's device id is durable even if its roster
        // was already learned over another channel.
        let peers_changed = next.peers.len() != guard.peers.len()
            || next.peers.values().any(|peer| {
                !super::file::only_last_seen_moved(&guard, peer)
                    || guard
                        .peers
                        .get(&peer.pairing_id)
                        .is_none_or(|old| old.last_seen_ms != peer.last_seen_ms)
            });
        if changed
            || peers_changed
            || next.group_admissions != guard.group_admissions
            || next.pair_admissions != guard.pair_admissions
        {
            self.persist(&next)?;
            let ids: Vec<_> = guard
                .peers
                .keys()
                .chain(next.peers.keys())
                .cloned()
                .collect();
            for id in ids {
                tentative::bump(&mut next, &id);
            }
            *guard = next;
            self.group_changed();
            return Ok(true);
        }
        Ok(false)
    }

    pub fn group_active(&self) -> bool {
        self.state
            .read()
            .is_ok_and(|state| state.group.as_ref().is_none_or(GroupState::active))
    }
    pub(crate) fn is_current_credential(&self, pairing_id: &str) -> bool {
        let Ok(state) = self.state.read() else {
            return false;
        };
        state.peers.contains_key(pairing_id)
            && state.group.as_ref().is_none_or(|group| {
                group.active()
                    && group.local_admission().is_ok_and(|current| {
                        state
                            .pair_admissions
                            .get(pairing_id)
                            .copied()
                            .unwrap_or_default()
                            .local
                            >= current
                    })
            })
    }
    pub fn subscribe_group_changes(&self) -> tokio::sync::watch::Receiver<u64> {
        self.group_changes.subscribe()
    }
    pub fn group_revision(&self) -> u64 {
        *self.group_changes.borrow()
    }
    pub(super) fn group_changed(&self) {
        // Drop and wipe obsolete scoped credentials immediately. Cache misses
        // never hold this mutex while acquiring the state lock.
        *self
            .metadata_cache
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = None;
        self.group_changes
            .send_modify(|value| *value = value.wrapping_add(1));
    }
    pub(crate) fn confirm_group_admission(&self, pairing_id: &str) -> Result<(), PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        if !guard.peers.contains_key(pairing_id) {
            return Err(PeerStoreError::Revoked);
        }
        let mut next = guard.clone();
        if let Some(admission) = next.pair_admissions.get(pairing_id).copied() {
            if let Some(group) = &mut next.group {
                group.certify_local(admission.local)?;
            }
        }
        next.group_admissions.insert(pairing_id.into());
        project(&mut next, guard.group.as_ref())?;
        self.persist(&next)?;
        *guard = next;
        self.group_changed();
        Ok(())
    }
    pub(crate) fn desired_group_admission(
        &self,
        device_id: Option<&str>,
    ) -> Result<u64, PeerStoreError> {
        let guard = self.state.read().map_err(|_| PeerStoreError::Poisoned)?;
        let group = guard
            .group
            .as_ref()
            .ok_or(PeerStoreError::Invalid("group is not initialized"))?;
        match device_id {
            Some(id) => group.desired_admission(id),
            None => group.local_admission(),
        }
    }
}

fn project(state: &mut State, previous_group: Option<&GroupState>) -> Result<(), PeerStoreError> {
    let Some(group) = &state.group else {
        return Ok(());
    };
    if !group.active() {
        state.peers.clear();
        state.group_admissions.clear();
        state.pair_admissions.clear();
        return Ok(());
    }
    state.peers.retain(|_, peer| {
        peer.device_id.as_deref().map_or_else(
            || {
                state
                    .pair_admissions
                    .get(&peer.pairing_id)
                    .copied()
                    .unwrap_or_default()
                    .local
                    >= group.local_admission().unwrap_or(u64::MAX)
            },
            |id| {
                !group.removed_device(id)
                    || (state.group_admissions.contains(&peer.pairing_id)
                        && state
                            .pair_admissions
                            .get(&peer.pairing_id)
                            .is_some_and(|admission| {
                                group
                                    .desired_admission(id)
                                    .is_ok_and(|required| admission.remote >= required)
                            }))
            },
        )
    });
    state
        .group_admissions
        .retain(|id| state.peers.contains_key(id));
    for peer in group.peers()? {
        peer.validate()?;
        let admission =
            group.pair_admissions(peer.device_id.as_deref().ok_or(PeerStoreError::Corrupt)?)?;
        let existing = state
            .peers
            .values()
            .find(|old| old.device_id == peer.device_id)
            .map(|old| old.pairing_id.clone());
        if let Some(id) = existing {
            // Keep working SAS/legacy credentials. Derived credentials rotate
            // on readmission, on both ends, and never reuse revoked pair ids.
            let current_admission = state.pair_admissions.get(&id).copied().unwrap_or_default();
            if (!id.starts_with(crate::group::GROUP_PAIRING_PREFIX)
                && current_admission == admission)
                || id == peer.pairing_id
            {
                if peer
                    .device_id
                    .as_deref()
                    .is_some_and(|device| group.member_changed(previous_group, device))
                {
                    let stored = state.peers.get_mut(&id).ok_or(PeerStoreError::Corrupt)?;
                    stored.name = peer.name.clone();
                    stored.last_addr = peer.last_addr.or(stored.last_addr);
                }
                continue;
            }
            state.peers.remove(&id);
            state.group_admissions.remove(&id);
            state.pair_admissions.remove(&id);
        }
        if state.revoked.contains_key(&peer.pairing_id) {
            return Err(PeerStoreError::Revoked);
        }
        if state.peers.len() >= MAX_PAIRINGS {
            return Err(PeerStoreError::TooManyPairings);
        }
        state
            .pair_admissions
            .insert(peer.pairing_id.clone(), admission);
        state.peers.insert(peer.pairing_id.clone(), peer);
    }
    state
        .pair_admissions
        .retain(|id, _| state.peers.contains_key(id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_local_metadata_never_replaces_a_readable_group_file() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("peers.json");
        let store = PeerStore::open(&path, &[7; 32]).unwrap();
        let profile = DeviceProfile::current();
        store
            .group_snapshot("device", "original", profile.clone(), None)
            .unwrap();
        assert!(store
            .group_snapshot(
                "device",
                &"x".repeat(crate::protocol::MAX_DEVICE_NAME_BYTES + 1),
                profile.clone(),
                None
            )
            .is_err());
        let reopened = PeerStore::open(&path, &[7; 32]).unwrap();
        assert_eq!(
            reopened
                .group_snapshot("device", "original", profile, None)
                .unwrap()
                .roster
                .members["device"]
                .name,
            "original"
        );
    }

    #[test]
    fn imported_exclusion_releases_pending_admission_markers() {
        let directory = tempfile::tempdir().unwrap();
        let profile = DeviceProfile::current();
        let a = PeerStore::open(&directory.path().join("a.json"), &[7; 32]).unwrap();
        let b = PeerStore::open(&directory.path().join("b.json"), &[7; 32]).unwrap();
        let sa = a.group_snapshot("a", "a", profile.clone(), None).unwrap();
        let sb = b.group_snapshot("b", "b", profile.clone(), None).unwrap();
        a.merge_group(&sb, None, false).unwrap();
        let mut legacy = crate::peers::testutil::peer("a");
        legacy.device_id = Some("a".into());
        let id = legacy.pairing_id.clone();
        b.upsert(legacy).unwrap();
        b.merge_group(&sa, None, false).unwrap();
        b.confirm_group_admission(&id).unwrap();
        a.remove(&a.list()[0].pairing_id).unwrap();
        let removal = a.group_snapshot("a", "a", profile, None).unwrap();
        b.merge_group(&removal, None, false).unwrap();
        assert!(b.state.read().unwrap().group_admissions.is_empty());
        assert!(b.is_empty());
        assert!(!PeerStore::open(&directory.path().join("b.json"), &[7; 32])
            .unwrap()
            .group_active());
    }
}
