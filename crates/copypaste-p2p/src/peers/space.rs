//! Space key operations on the persisted group, each one atomic write.
use super::{group::project, tentative, PeerStore, PeerStoreError};
use crate::group::{GroupState, SpaceEnvelope, SpaceInfo, SpaceKeyMaterial, SpaceRemovalNotice};

impl PeerStore {
    fn update_space<T>(
        &self,
        update: impl FnOnce(&mut GroupState) -> Result<(T, bool), PeerStoreError>,
    ) -> Result<T, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        let mut next = guard.clone();
        let group = next
            .group
            .as_mut()
            .ok_or(PeerStoreError::Invalid("group is not initialized"))?;
        let (value, changed) = update(group)?;
        if changed {
            group.validate()?;
            self.persist(&next)?;
            *guard = next;
        }
        Ok(value)
    }
    fn read_group<T>(&self, read: impl FnOnce(&GroupState) -> Option<T>) -> Option<T> {
        self.state.read().ok()?.group.as_ref().and_then(read)
    }
    pub fn space_key(&self) -> Option<SpaceKeyMaterial> {
        self.read_group(GroupState::space_key)
    }
    pub fn space_info(&self) -> Option<SpaceInfo> {
        self.read_group(GroupState::space_info)
    }
    /// A device that left keeps its key until its departure is published.
    pub fn group_exportable(&self) -> bool {
        self.state.read().is_ok_and(|state| {
            state
                .group
                .as_ref()
                .is_none_or(|group| group.active() || group.has_space())
        })
    }
    pub fn register_space_key(&self, key: &[u8; 32]) -> Result<bool, PeerStoreError> {
        let registered = self.update_space(|group| {
            group
                .register_space_key(key)
                .map(|changed| (changed, changed))
        })?;
        if registered {
            tracing::info!("space key registered");
        }
        Ok(registered)
    }
    pub fn rotate_space_if_needed(&self) -> Result<bool, PeerStoreError> {
        let rotated = self
            .update_space(|group| {
                group
                    .rotate_space_if_needed()
                    .map(|changed| (changed, changed))
            })
            .inspect_err(|_| tracing::warn!("space key rotation failed"))?;
        if rotated {
            tracing::info!("space key rotated");
        }
        Ok(rotated)
    }
    pub fn allow_space_recovery(&self) -> Result<bool, PeerStoreError> {
        self.update_space(|group| {
            group
                .allow_space_recovery()
                .map(|changed| (changed, changed))
        })
    }
    pub fn confirm_space_member(
        &self,
        device_id: &str,
        epoch: u64,
    ) -> Result<bool, PeerStoreError> {
        self.update_space(|group| {
            let changed = group.confirm_space_member(device_id, epoch);
            Ok((changed, changed))
        })
    }
    pub fn space_envelopes(&self) -> Result<Vec<SpaceEnvelope>, PeerStoreError> {
        self.update_space(GroupState::space_envelopes)
    }
    pub fn accept_space_envelope(&self, envelope: &SpaceEnvelope) -> Result<bool, PeerStoreError> {
        let accepted = self.update_space(|group| {
            group
                .accept_space_envelope(envelope)
                .map(|changed| (changed, changed))
        })?;
        if accepted {
            tracing::info!("space key received");
        }
        Ok(accepted)
    }
    pub fn space_recovery_plaintext(&self) -> Option<(u64, zeroize::Zeroizing<Vec<u8>>)> {
        self.read_group(GroupState::space_recovery_plaintext)
    }
    pub fn accept_space_recovery(
        &self,
        epoch: u64,
        plaintext: &[u8],
    ) -> Result<bool, PeerStoreError> {
        let recovered = self.update_space(|group| {
            group
                .accept_space_recovery(epoch, plaintext)
                .map(|changed| (changed, changed))
        })?;
        if recovered {
            tracing::info!("space key recovered");
        }
        Ok(recovered)
    }
    pub fn space_removal_notice(&self) -> Option<SpaceRemovalNotice> {
        self.read_group(GroupState::space_removal_notice)
    }
    pub fn clear_space_removal_notice(&self) -> Result<bool, PeerStoreError> {
        self.update_space(|group| {
            let changed = group.clear_space_removal_notice();
            Ok((changed, changed))
        })
    }
    pub fn forget_departed_space(&self) -> Result<bool, PeerStoreError> {
        let forgotten = self.update_space(|group| {
            let changed = group.forget_departed_space();
            Ok((changed, changed))
        })?;
        if forgotten {
            tracing::info!("this device left its space; its key was discarded");
        }
        Ok(forgotten)
    }
    pub fn group_active_member(&self) -> bool {
        self.read_group(|group| Some(group.active()))
            .unwrap_or(false)
    }
    /// Removes this device from its space on every member that learns it.
    pub fn leave_space(&self) -> Result<bool, PeerStoreError> {
        let mut guard = self.state.write().map_err(|_| PeerStoreError::Poisoned)?;
        let mut next = guard.clone();
        let previous = guard.group.clone();
        let group = next
            .group
            .as_mut()
            .ok_or(PeerStoreError::Invalid("group is not initialized"))?;
        if !group.leave()? {
            return Ok(false);
        }
        group.validate()?;
        project(&mut next, previous.as_ref())?;
        self.persist(&next)?;
        let ids: Vec<_> = guard.peers.keys().cloned().collect();
        *guard = next;
        for id in ids {
            tentative::bump(&mut guard, &id);
        }
        self.group_changed();
        Ok(true)
    }
}
