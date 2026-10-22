//! Personal device membership, independent of the transport carrying it.
//!
//! SAS-confirmed peers and an authenticated cloud account with the sync key
//! are admission authorities. Every active device may admit and remove members.
//! Snapshots are individually signed; private device keys never leave this
//! device. Removal wins over a stale admission of the same generation.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use x25519_dalek::{PublicKey, StaticSecret};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use crate::{
    peers::{PeerStoreError, MAX_PAIRINGS, MAX_REVOCATIONS},
    DeviceProfile, Peer,
};

pub const MAX_GROUP_BYTES: usize = 768 * 1024;
pub(crate) const GROUP_PAIRING_PREFIX: &str = "group-";
/// Membership generations certified by one bilateral SAS credential.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PairAdmissions {
    pub local: u64,
    pub remote: u64,
}
impl PairAdmissions {
    pub(crate) fn agree(self, remote: Self) -> Result<Self, PeerStoreError> {
        // This device participated in every admission of its own identity.
        // A peer can know one later removal while we were offline, but cannot
        // legitimately certify an arbitrary jump in our generation.
        let next_local = self.local.checked_add(1).ok_or_else(invalid)?;
        if remote.remote > next_local {
            return Err(invalid());
        }
        let agreed = Self {
            local: self.local.max(remote.remote),
            remote: self.remote.max(remote.local),
        };
        if agreed.local == u64::MAX || agreed.remote == u64::MAX {
            return Err(invalid());
        }
        Ok(agreed)
    }
}
#[derive(Debug, thiserror::Error)]
pub enum GroupImportError {
    #[error("the device group snapshot was refused")]
    Rejected,
    #[error("the device group could not be saved")]
    Persistence,
}
const DOMAIN: &[u8] = b"copypaste/group/v1/";

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub(crate) struct Identity {
    signing: [u8; 32],
    exchange: [u8; 32],
}

impl Identity {
    fn new() -> Self {
        let mut result = Self {
            signing: [0; 32],
            exchange: [0; 32],
        };
        rand::rngs::OsRng.fill_bytes(&mut result.signing);
        rand::rngs::OsRng.fill_bytes(&mut result.exchange);
        result
    }
    fn signer(&self) -> SigningKey {
        SigningKey::from_bytes(&self.signing)
    }
    fn dh(&self) -> StaticSecret {
        StaticSecret::from(self.exchange)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub device_id: String,
    pub name: String,
    pub signing_key: [u8; 32],
    pub exchange_key: [u8; 32],
    /// Advanced only by a new explicitly confirmed admission after removal.
    pub admission: u64,
    pub revision: u64,
    pub profile: DeviceProfile,
    pub listen_addr: Option<std::net::SocketAddr>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Roster {
    pub issuer: String,
    pub members: BTreeMap<String, Member>,
    /// Highest removed admission generation, retained across offline replay.
    pub removed: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupSnapshot {
    pub roster: Roster,
    pub signature: String,
}

impl GroupSnapshot {
    pub fn validate(&self) -> Result<(), PeerStoreError> {
        let roster = &self.roster;
        if roster.members.is_empty()
            || roster.members.len() > MAX_PAIRINGS + 1
            || roster.removed.len() > MAX_REVOCATIONS
            || self.signature.len() != 128
        {
            return Err(invalid());
        }
        for (id, member) in &roster.members {
            validate_id(id)?;
            if id != &member.device_id
                || member.name.len() > crate::protocol::MAX_DEVICE_NAME_BYTES
                || member.exchange_key == [0; 32]
                || member.admission == u64::MAX
            {
                return Err(invalid());
            }
            crate::protocol::validate_device_profile(&member.profile).map_err(|_| invalid())?;
            if member
                .listen_addr
                .is_some_and(|addr| !crate::netif::is_dialable(&addr))
            {
                return Err(invalid());
            }
        }
        for (id, generation) in &roster.removed {
            validate_id(id)?;
            if *generation == u64::MAX {
                return Err(invalid());
            }
        }
        let issuer = roster.members.get(&roster.issuer).ok_or_else(invalid)?;
        let signature = hex::decode(&self.signature).map_err(|_| invalid())?;
        VerifyingKey::from_bytes(&issuer.signing_key)
            .map_err(|_| invalid())?
            .verify_strict(
                &signed_bytes(roster)?,
                &Signature::from_slice(&signature).map_err(|_| invalid())?,
            )
            .map_err(|_| invalid())
    }
    pub fn encode(&self) -> Result<String, PeerStoreError> {
        let json = serde_json::to_string(self).map_err(|_| invalid())?;
        if json.len() > MAX_GROUP_BYTES {
            return Err(invalid());
        }
        Ok(json)
    }
    pub fn decode(json: &str) -> Result<Self, PeerStoreError> {
        if json.len() > MAX_GROUP_BYTES {
            return Err(invalid());
        }
        let result: Self = serde_json::from_str(json).map_err(|_| invalid())?;
        result.validate()?;
        Ok(result)
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct GroupState {
    identity: Identity,
    roster: Roster,
}

impl GroupState {
    pub(crate) fn identity_tag(&self, device_id: Option<&str>) -> Option<String> {
        let id = device_id.unwrap_or(&self.roster.issuer);
        self.is_active(id)
            .then(|| self.roster.members.get(id))
            .flatten()
            .map(|member| hex::encode(Sha256::digest(member.exchange_key)))
    }
    pub(crate) fn metadata_key(&self, id: &str) -> Result<Zeroizing<[u8; 32]>, PeerStoreError> {
        if !self.active() || !self.is_active(id) || id == self.roster.issuer {
            return Err(PeerStoreError::Revoked);
        }
        let me = self
            .roster
            .members
            .get(&self.roster.issuer)
            .ok_or_else(invalid)?;
        let peer = self.roster.members.get(id).ok_or_else(invalid)?;
        let shared = self
            .identity
            .dh()
            .diffie_hellman(&PublicKey::from(peer.exchange_key));
        if !shared.was_contributory() {
            return Err(invalid());
        }
        let mut members = [me, peer];
        members.sort_by(|a, b| a.device_id.cmp(&b.device_id));
        let context = serde_json::to_vec(&(
            "copypaste/group/membership-channel/v1",
            members.map(|member| (&member.device_id, member.exchange_key)),
        ))
        .map_err(|_| invalid())?;
        let mut key = Zeroizing::new([0; 32]);
        Hkdf::<Sha256>::new(Some(DOMAIN), shared.as_bytes())
            .expand(&context, key.as_mut())
            .map_err(|_| invalid())?;
        Ok(key)
    }
    pub(crate) fn desired_admission(&self, id: &str) -> Result<u64, PeerStoreError> {
        let current = self
            .roster
            .members
            .get(id)
            .map_or(0, |member| member.admission);
        match self.roster.removed.get(id) {
            Some(removed) if current <= *removed => removed.checked_add(1).ok_or_else(invalid),
            _ => Ok(current),
        }
    }
    pub(crate) fn local_admission(&self) -> Result<u64, PeerStoreError> {
        self.desired_admission(&self.roster.issuer)
    }
    pub(crate) fn check_local_admission(&self, admission: u64) -> Result<(), PeerStoreError> {
        let current = self
            .roster
            .members
            .get(&self.roster.issuer)
            .ok_or_else(invalid)?
            .admission;
        if admission < self.local_admission()?
            || admission == u64::MAX
            || admission > current.checked_add(1).ok_or_else(invalid)?
        {
            return Err(PeerStoreError::Revoked);
        }
        Ok(())
    }
    pub(crate) fn certify_local(&mut self, admission: u64) -> Result<(), PeerStoreError> {
        self.check_local_admission(admission)?;
        self.roster
            .members
            .get_mut(&self.roster.issuer)
            .ok_or_else(invalid)?
            .admission = admission;
        Ok(())
    }
    pub(crate) fn pair_admissions(&self, id: &str) -> Result<PairAdmissions, PeerStoreError> {
        Ok(PairAdmissions {
            local: self
                .roster
                .members
                .get(&self.roster.issuer)
                .ok_or_else(invalid)?
                .admission,
            remote: self.roster.members.get(id).ok_or_else(invalid)?.admission,
        })
    }
    pub(crate) fn local_matches(
        &self,
        id: &str,
        name: &str,
        profile: &DeviceProfile,
        addr: Option<std::net::SocketAddr>,
    ) -> bool {
        id == self.roster.issuer
            && self.roster.members.get(id).is_some_and(|member| {
                member.name == name && &member.profile == profile && member.listen_addr == addr
            })
    }
    pub(crate) fn member_changed(&self, previous: Option<&Self>, id: &str) -> bool {
        self.roster.members.get(id) != previous.and_then(|group| group.roster.members.get(id))
    }
    pub(crate) fn new(
        id: &str,
        name: &str,
        profile: DeviceProfile,
        addr: Option<std::net::SocketAddr>,
    ) -> Result<Self, PeerStoreError> {
        validate_id(id)?;
        let identity = Identity::new();
        let member = Member {
            device_id: id.into(),
            name: name.into(),
            signing_key: identity.signer().verifying_key().to_bytes(),
            exchange_key: PublicKey::from(&identity.dh()).to_bytes(),
            admission: 0,
            revision: 0,
            profile,
            listen_addr: addr,
        };
        let state = Self {
            identity,
            roster: Roster {
                issuer: id.into(),
                members: BTreeMap::from([(id.into(), member)]),
                removed: BTreeMap::new(),
            },
        };
        state.snapshot()?.validate()?;
        Ok(state)
    }
    pub(crate) fn validate(&self) -> Result<(), PeerStoreError> {
        let me = self
            .roster
            .members
            .get(&self.roster.issuer)
            .ok_or_else(invalid)?;
        if me.signing_key != self.identity.signer().verifying_key().to_bytes()
            || me.exchange_key != PublicKey::from(&self.identity.dh()).to_bytes()
        {
            return Err(invalid());
        }
        self.snapshot()?.validate()
    }
    pub(crate) fn active(&self) -> bool {
        self.is_active(&self.roster.issuer)
    }
    fn is_active(&self, id: &str) -> bool {
        self.roster.members.get(id).is_some_and(|m| {
            self.roster
                .removed
                .get(id)
                .is_none_or(|removed| m.admission > *removed)
        })
    }
    pub(crate) fn snapshot(&self) -> Result<GroupSnapshot, PeerStoreError> {
        // Removed members' public records are unnecessary; keep only our own
        // so an excluded device can still authenticate its local state on open.
        let mut roster = self.roster.clone();
        roster
            .members
            .retain(|id, _| id == &roster.issuer || self.is_active(id));
        let signature = hex::encode(
            self.identity
                .signer()
                .sign(&signed_bytes(&roster)?)
                .to_bytes(),
        );
        Ok(GroupSnapshot { roster, signature })
    }
    pub(crate) fn update_local(
        &mut self,
        id: &str,
        name: &str,
        profile: DeviceProfile,
        addr: Option<std::net::SocketAddr>,
    ) -> Result<bool, PeerStoreError> {
        if id != self.roster.issuer {
            return Err(invalid());
        }
        let member = self.roster.members.get_mut(id).ok_or_else(invalid)?;
        if member.name == name && member.profile == profile && member.listen_addr == addr {
            return Ok(false);
        }
        member.name = name.into();
        member.profile = profile;
        member.listen_addr = addr;
        member.revision = member.revision.checked_add(1).ok_or_else(invalid)?;
        Ok(true)
    }
    pub(crate) fn merge(
        &mut self,
        incoming: &GroupSnapshot,
        explicit_admission: bool,
    ) -> Result<bool, PeerStoreError> {
        incoming.validate()?;
        let remote = &incoming.roster;
        let issuer = remote.members.get(&remote.issuer).ok_or_else(invalid)?;
        if remote.issuer == self.roster.issuer {
            return Ok(false);
        }
        if let Some(known) = self.roster.members.get(&remote.issuer) {
            if known.signing_key != issuer.signing_key || known.exchange_key != issuer.exchange_key
            {
                return Err(invalid());
            }
        }
        // A removed device cannot use retained cloud credentials to restore
        // itself or sign new group changes. Only fresh SAS admits it again.
        if !explicit_admission
            && (self.removed_device(&remote.issuer)
                || remote
                    .removed
                    .get(&remote.issuer)
                    .is_some_and(|removed| issuer.admission <= *removed)
                || self
                    .roster
                    .removed
                    .get(&remote.issuer)
                    .is_some_and(|removed| issuer.admission <= *removed))
        {
            return Err(PeerStoreError::Revoked);
        }
        // Another member cannot advance our identity without our SAS consent,
        // or claim to have removed an admission we have never participated in.
        let local_admission = self
            .roster
            .members
            .get(&self.roster.issuer)
            .ok_or_else(invalid)?
            .admission;
        if remote
            .members
            .get(&self.roster.issuer)
            .is_some_and(|member| member.admission > local_admission)
            || remote
                .removed
                .get(&self.roster.issuer)
                .is_some_and(|removed| *removed > local_admission)
        {
            return Err(invalid());
        }
        let before = self.roster.clone();
        for (id, generation) in &remote.removed {
            if !self.roster.removed.contains_key(id) && self.roster.removed.len() >= MAX_REVOCATIONS
            {
                return Err(PeerStoreError::TooManyRevocations);
            }
            let slot = self.roster.removed.entry(id.clone()).or_insert(*generation);
            *slot = (*slot).max(*generation);
        }
        for (id, member) in &remote.members {
            if id == &self.roster.issuer {
                let local = self.roster.members.get_mut(id).ok_or_else(invalid)?;
                if local.signing_key != member.signing_key
                    || local.exchange_key != member.exchange_key
                {
                    return Err(invalid());
                }
                local.admission = local.admission.max(member.admission);
                continue;
            }
            if self
                .roster
                .removed
                .get(id)
                .is_some_and(|removed| member.admission <= *removed)
            {
                continue;
            }
            match self.roster.members.get(id) {
                Some(old)
                    if old.signing_key != member.signing_key
                        || old.exchange_key != member.exchange_key =>
                {
                    // A current member may certify a freshly admitted key
                    // after another device was reset. Self-signed key swaps
                    // and equal-generation replacements remain forbidden.
                    if member.admission <= old.admission || id == &remote.issuer {
                        return Err(invalid());
                    }
                    self.roster.members.insert(id.clone(), member.clone());
                }
                Some(old)
                    if (old.admission, old.revision) >= (member.admission, member.revision) => {}
                _ => {
                    self.roster.members.insert(id.clone(), member.clone());
                }
            }
        }
        if explicit_admission {
            // Both devices have just confirmed this admission, including a
            // deliberate return of a previously removed personal device.
            for id in [&self.roster.issuer.clone(), &remote.issuer] {
                let mut member = if id == &remote.issuer {
                    issuer.clone()
                } else {
                    self.roster.members.get(id).ok_or_else(invalid)?.clone()
                };
                if let Some(removed) = self.roster.removed.get(id) {
                    member.admission = member
                        .admission
                        .max(removed.checked_add(1).ok_or_else(invalid)?);
                }
                self.roster.members.insert(id.clone(), member);
            }
        }
        let removed = &self.roster.removed;
        let own = &self.roster.issuer;
        self.roster.members.retain(|id, member| {
            id == own || removed.get(id).is_none_or(|r| member.admission > *r)
        });
        if !self.active() {
            // Excluded devices retain only their own key identity and removal
            // evidence. Admission metadata is obtained again after fresh SAS.
            let own = self.roster.issuer.clone();
            self.roster.members.retain(|id, _| id == &own);
        }
        if self.roster.members.len() > MAX_PAIRINGS + 1 {
            return Err(PeerStoreError::TooManyPairings);
        }
        let changed = before != self.roster;
        if changed {
            self.validate()?;
        }
        Ok(changed)
    }
    pub(crate) fn remove(&mut self, id: &str) -> Result<bool, PeerStoreError> {
        validate_id(id)?;
        // A legacy pairing may be removed before its first upgraded session
        // has supplied a roster. Its stable id must still get a tombstone.
        let generation = self
            .roster
            .members
            .get(id)
            .map_or(0, |member| member.admission);
        if self
            .roster
            .removed
            .get(id)
            .is_some_and(|old| *old >= generation)
        {
            return Ok(false);
        }
        if self.roster.removed.len() >= MAX_REVOCATIONS && !self.roster.removed.contains_key(id) {
            return Err(PeerStoreError::TooManyRevocations);
        }
        self.roster.removed.insert(id.into(), generation);
        if id != self.roster.issuer {
            self.roster.members.remove(id);
        }
        Ok(true)
    }
    pub(crate) fn peers(&self) -> Result<Vec<Peer>, PeerStoreError> {
        if !self.active() {
            return Ok(Vec::new());
        }
        let me = self
            .roster
            .members
            .get(&self.roster.issuer)
            .ok_or_else(invalid)?;
        let dh = self.identity.dh();
        let mut peers = Vec::new();
        for member in
            self.roster.members.values().filter(|member| {
                member.device_id != me.device_id && self.is_active(&member.device_id)
            })
        {
            let shared = dh.diffie_hellman(&PublicKey::from(member.exchange_key));
            if !shared.was_contributory() {
                return Err(invalid());
            }
            let mut endpoints = [me, member];
            endpoints.sort_by(|a, b| a.device_id.cmp(&b.device_id));
            let context = serde_json::to_vec(&(
                "copypaste/group/pair/v1",
                endpoints.map(|m| (&m.device_id, m.exchange_key, m.admission)),
            ))
            .map_err(|_| invalid())?;
            let mut psk = Zeroizing::new([0; 32]);
            Hkdf::<Sha256>::new(Some(DOMAIN), shared.as_bytes())
                .expand(&context, psk.as_mut())
                .map_err(|_| invalid())?;
            let pairing_id = format!(
                "{GROUP_PAIRING_PREFIX}{}",
                data_encoding::BASE32_NOPAD.encode(&Sha256::digest(&context))
            );
            // Preserve the existing mDNS identifier contract; a full hex
            // digest plus the namespace prefix would exceed its 64 bytes.
            if !crate::discovery::is_valid_pairing_id(&pairing_id) {
                return Err(invalid());
            }
            peers.push(Peer {
                pairing_id,
                device_id: Some(member.device_id.clone()),
                name: member.name.clone(),
                psk: *psk,
                last_addr: member.listen_addr,
                last_seen_ms: 0,
                // A relayed descriptor is not a completed direct profile
                // observation. The existing probe/session owner records it.
                profile: None,
                profile_observed_at_ms: 0,
            });
        }
        Ok(peers)
    }
    pub(crate) fn removed_device(&self, id: &str) -> bool {
        self.roster.removed.contains_key(id) && !self.is_active(id)
    }
}

fn signed_bytes(roster: &Roster) -> Result<Vec<u8>, PeerStoreError> {
    let mut bytes = DOMAIN.to_vec();
    bytes.extend(serde_json::to_vec(roster).map_err(|_| invalid())?);
    if bytes.len() > MAX_GROUP_BYTES {
        return Err(invalid());
    }
    Ok(bytes)
}
fn validate_id(id: &str) -> Result<(), PeerStoreError> {
    if id.is_empty()
        || id.len() > crate::protocol::MAX_ID_BYTES
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(invalid());
    }
    Ok(())
}
fn invalid() -> PeerStoreError {
    PeerStoreError::Invalid("invalid device group")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str) -> GroupState {
        GroupState::new(id, id, DeviceProfile::default(), None).unwrap()
    }

    #[test]
    fn self_excluded_issuer_cannot_mutate_an_offline_member() {
        let mut b = group("b");
        let mut c = group("c");
        c.merge(&b.snapshot().unwrap(), false).unwrap();
        let before = c.snapshot().unwrap();
        b.remove("b").unwrap();
        b.roster.removed.insert("c".into(), 0);
        let excluded = b.snapshot().unwrap();
        excluded.validate().unwrap();
        assert!(matches!(
            c.merge(&excluded, false),
            Err(PeerStoreError::Revoked)
        ));
        assert_eq!(c.snapshot().unwrap(), before);
        assert!(c.active());
        // Excluded local state must still authenticate and reopen normally.
        b.validate().unwrap();
    }

    #[test]
    fn relayed_generation_claims_cannot_advance_or_poison_our_identity() {
        let mut a = group("a");
        let mut b = group("b");
        b.merge(&a.snapshot().unwrap(), false).unwrap();
        let before = a.snapshot().unwrap();
        b.roster.members.get_mut("a").unwrap().admission = 100;
        assert!(a.merge(&b.snapshot().unwrap(), false).is_err());
        assert_eq!(a.snapshot().unwrap(), before);
        b.roster.members.get_mut("a").unwrap().admission = 0;
        b.roster.removed.insert("a".into(), 100);
        assert!(a.merge(&b.snapshot().unwrap(), false).is_err());
        assert_eq!(a.snapshot().unwrap(), before);
        assert!(a.certify_local(100).is_err());
        assert_eq!(a.snapshot().unwrap(), before);
    }
}
