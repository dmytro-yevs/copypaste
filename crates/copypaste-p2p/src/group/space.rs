//! Space content keys for the cloud.
//!
//! P2P never uses this key: pairwise credentials come from member identities,
//! so removal alone cuts a device off there. The cloud shares one symmetric
//! key, so it rotates after a removal. Epoch 0 is the passphrase-derived key a
//! sync provider registers; later epochs are random. Each epoch is sealed to
//! every active member with static X25519 between the sender and recipient.
use std::collections::BTreeSet;

use chacha20poly1305::{
    aead::{Aead, KeyInit, Payload},
    XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use x25519_dalek::PublicKey;
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

use super::{invalid, validate_id, GroupState, Member};
use crate::peers::{PeerStoreError, MAX_PAIRINGS};

pub const MAX_ENVELOPE_BYTES: usize = 16 * 1024;
const ENVELOPE_LABEL: &str = "copypaste/space/envelope/v1";
const NONCE_LEN: usize = 24;

#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpaceKey {
    #[zeroize(skip)]
    epoch: u64,
    /// Device that generated this epoch; with `epoch` it orders rotations.
    #[zeroize(skip)]
    creator: String,
    key: [u8; 32],
    #[zeroize(skip)]
    recipients: BTreeSet<String>,
    /// Whether a passphrase envelope may be published for this epoch.
    #[zeroize(skip)]
    recovery: bool,
    /// Members proven to hold this epoch (they wrote data under it).
    #[zeroize(skip)]
    confirmed: BTreeSet<String>,
    /// Removed devices still to be told, under the key they hold.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[zeroize(skip)]
    notice: Option<RemovalNotice>,
}

/// The key removed devices still hold, kept until they have been told.
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
#[serde(deny_unknown_fields)]
struct RemovalNotice {
    #[zeroize(skip)]
    epoch: u64,
    key: [u8; 32],
    #[zeroize(skip)]
    recipients: BTreeSet<String>,
}

/// What a sync provider needs to tell removed devices about their removal.
pub struct SpaceRemovalNotice {
    pub previous_epoch: u64,
    pub previous_key: Zeroizing<[u8; 32]>,
    pub recipients: Vec<String>,
}

/// One epoch key sealed from `sender` to `recipient`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceEnvelope {
    pub epoch: u64,
    pub sender: String,
    pub recipient: String,
    pub nonce: String,
    pub ciphertext: String,
}

impl SpaceEnvelope {
    pub fn validate(&self) -> Result<(), PeerStoreError> {
        validate_id(&self.sender)?;
        validate_id(&self.recipient)?;
        if self.epoch == u64::MAX
            || self.nonce.len() != NONCE_LEN * 2
            || self.ciphertext.is_empty()
            || self.ciphertext.len() > MAX_ENVELOPE_BYTES
        {
            return Err(invalid());
        }
        Ok(())
    }
}

/// The current epoch as a sync provider needs it.
pub struct SpaceKeyMaterial {
    pub epoch: u64,
    pub key: Zeroizing<[u8; 32]>,
}

/// Plaintext of a member or recovery envelope.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Sealed {
    epoch: u64,
    creator: String,
    key: String,
    recipients: BTreeSet<String>,
    recovery: bool,
}
impl Drop for Sealed {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

/// Space state shown to the user.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceInfo {
    pub epoch: u64,
    pub recovery: bool,
    /// Active members not yet proven to hold the current epoch.
    pub waiting: Vec<String>,
}

impl SpaceKey {
    fn validate(&self) -> Result<(), PeerStoreError> {
        validate_id(&self.creator)?;
        if self.epoch == u64::MAX
            || self.recipients.len() > MAX_PAIRINGS + 1
            || self.confirmed.len() > MAX_PAIRINGS + 1
        {
            return Err(invalid());
        }
        for id in self.recipients.iter().chain(&self.confirmed) {
            validate_id(id)?;
        }
        Ok(())
    }
    fn order(&self) -> (u64, &str) {
        (self.epoch, self.creator.as_str())
    }
    fn sealed(&self) -> Sealed {
        Sealed {
            epoch: self.epoch,
            creator: self.creator.clone(),
            key: hex::encode(self.key),
            recipients: self.recipients.clone(),
            recovery: self.recovery,
        }
    }
    fn from_sealed(sealed: &Sealed, me: &str, sender: &str) -> Result<Self, PeerStoreError> {
        let mut key = [0; 32];
        hex::decode_to_slice(&sealed.key, &mut key).map_err(|_| invalid())?;
        let mut recipients = sealed.recipients.clone();
        recipients.insert(me.into());
        let result = Self {
            epoch: sealed.epoch,
            creator: sealed.creator.clone(),
            key,
            recipients,
            recovery: sealed.recovery,
            confirmed: BTreeSet::from([me.to_string(), sender.to_string()]),
            notice: None,
        };
        key.zeroize();
        result.validate()?;
        Ok(result)
    }
}

fn envelope_cipher(shared: &[u8; 32], info: &[u8]) -> Result<XChaCha20Poly1305, PeerStoreError> {
    let mut key = Zeroizing::new([0; 32]);
    Hkdf::<Sha256>::new(Some(super::DOMAIN), shared)
        .expand(info, key.as_mut())
        .map_err(|_| invalid())?;
    XChaCha20Poly1305::new_from_slice(key.as_ref()).map_err(|_| invalid())
}

fn envelope_info(
    epoch: u64,
    sender: &Member,
    recipient: &Member,
) -> Result<Vec<u8>, PeerStoreError> {
    serde_json::to_vec(&(
        ENVELOPE_LABEL,
        epoch,
        (&sender.device_id, sender.exchange_key),
        (&recipient.device_id, recipient.exchange_key),
    ))
    .map_err(|_| invalid())
}

impl GroupState {
    fn issuer_member(&self) -> Result<&Member, PeerStoreError> {
        self.roster
            .members
            .get(&self.roster.issuer)
            .ok_or_else(invalid)
    }
    fn active_member(&self, id: &str) -> Option<&Member> {
        self.is_active(id)
            .then(|| self.roster.members.get(id))
            .flatten()
    }
    fn shared_with(&self, peer: &Member) -> Result<Zeroizing<[u8; 32]>, PeerStoreError> {
        let shared = self
            .identity
            .dh()
            .diffie_hellman(&PublicKey::from(peer.exchange_key));
        if !shared.was_contributory() {
            return Err(invalid());
        }
        Ok(Zeroizing::new(*shared.as_bytes()))
    }
    pub(crate) fn validate_space(&self) -> Result<(), PeerStoreError> {
        self.space.as_ref().map_or(Ok(()), SpaceKey::validate)
    }
    /// Forgets the key of a device that left once its departure is out.
    pub(crate) fn forget_departed_space(&mut self) -> bool {
        !self.active() && self.space.take().is_some()
    }
    pub(crate) fn has_space(&self) -> bool {
        self.space.is_some()
    }
    pub(crate) fn space_key(&self) -> Option<SpaceKeyMaterial> {
        let space = self.space.as_ref()?;
        Some(SpaceKeyMaterial {
            epoch: space.epoch,
            key: Zeroizing::new(space.key),
        })
    }
    pub(crate) fn space_info(&self) -> Option<SpaceInfo> {
        let space = self.space.as_ref()?;
        let waiting = if space.epoch == 0 {
            Vec::new()
        } else {
            self.roster
                .members
                .keys()
                .filter(|id| self.is_active(id) && !space.confirmed.contains(*id))
                .cloned()
                .collect()
        };
        Some(SpaceInfo {
            epoch: space.epoch,
            recovery: space.recovery,
            waiting,
        })
    }
    fn active_ids(&self) -> BTreeSet<String> {
        self.roster
            .members
            .keys()
            .filter(|id| self.is_active(id))
            .cloned()
            .collect()
    }
    /// A sync provider registers its passphrase key as epoch 0 once.
    pub(crate) fn register_space_key(&mut self, key: &[u8; 32]) -> Result<bool, PeerStoreError> {
        if !self.active() {
            return Err(PeerStoreError::Revoked);
        }
        if self.space.is_some() {
            return Ok(false);
        }
        let me = self.roster.issuer.clone();
        self.space = Some(SpaceKey {
            epoch: 0,
            creator: me.clone(),
            key: *key,
            recipients: self.active_ids(),
            recovery: true,
            confirmed: BTreeSet::from([me]),
            notice: None,
        });
        Ok(true)
    }
    /// Rotates when a device that held the current key has been removed.
    pub(crate) fn rotate_space_if_needed(&mut self) -> Result<bool, PeerStoreError> {
        if !self.active() {
            return Ok(false);
        }
        let Some(current) = &self.space else {
            return Ok(false);
        };
        if !current.recipients.iter().any(|id| self.removed_device(id)) {
            return Ok(false);
        }
        let epoch = current.epoch.checked_add(1).ok_or_else(invalid)?;
        let notice = RemovalNotice {
            epoch: current.epoch,
            key: current.key,
            recipients: current
                .recipients
                .iter()
                .filter(|id| self.removed_device(id))
                .cloned()
                .collect(),
        };
        self.new_epoch(epoch, false);
        if let Some(space) = &mut self.space {
            space.notice = Some(notice);
        }
        Ok(true)
    }
    /// A new passphrase protects recovery from now on. Epoch 0 is the old
    /// passphrase key itself, so it is replaced by a random epoch first.
    pub(crate) fn allow_space_recovery(&mut self) -> Result<bool, PeerStoreError> {
        if !self.active() {
            return Err(PeerStoreError::Revoked);
        }
        let space = self.space.as_mut().ok_or(PeerStoreError::Revoked)?;
        if space.epoch > 0 {
            return Ok(!std::mem::replace(&mut space.recovery, true));
        }
        self.new_epoch(1, true);
        Ok(true)
    }
    fn new_epoch(&mut self, epoch: u64, recovery: bool) {
        let me = self.roster.issuer.clone();
        let mut key = [0; 32];
        rand::rngs::OsRng.fill_bytes(&mut key);
        self.space = Some(SpaceKey {
            epoch,
            creator: me.clone(),
            key,
            recipients: self.active_ids(),
            recovery,
            confirmed: BTreeSet::from([me]),
            notice: None,
        });
        key.zeroize();
    }
    pub(crate) fn space_removal_notice(&self) -> Option<SpaceRemovalNotice> {
        let notice = self.space.as_ref()?.notice.as_ref()?;
        Some(SpaceRemovalNotice {
            previous_epoch: notice.epoch,
            previous_key: Zeroizing::new(notice.key),
            recipients: notice.recipients.iter().cloned().collect(),
        })
    }
    pub(crate) fn clear_space_removal_notice(&mut self) -> bool {
        self.space
            .as_mut()
            .is_some_and(|space| space.notice.take().is_some())
    }
    pub(crate) fn confirm_space_member(&mut self, id: &str, epoch: u64) -> bool {
        let active = self.is_active(id);
        match &mut self.space {
            Some(space) if space.epoch == epoch && active => space.confirmed.insert(id.into()),
            _ => false,
        }
    }
    /// Seals the current epoch for every other active member. Newly admitted
    /// members become recipients of the current epoch.
    pub(crate) fn space_envelopes(&mut self) -> Result<(Vec<SpaceEnvelope>, bool), PeerStoreError> {
        if !self.active() || self.space.is_none() {
            return Ok((Vec::new(), false));
        }
        let active = self.active_ids();
        let space = self.space.as_mut().ok_or_else(invalid)?;
        let before = space.recipients.len();
        space.recipients.extend(active.iter().cloned());
        let changed = space.recipients.len() != before;
        let space = self.space.as_ref().ok_or_else(invalid)?;
        let me = self.issuer_member()?;
        let plaintext = Zeroizing::new(serde_json::to_vec(&space.sealed()).map_err(|_| invalid())?);
        let mut result = Vec::new();
        for id in active.iter().filter(|id| **id != me.device_id) {
            let recipient = self.roster.members.get(id).ok_or_else(invalid)?;
            let info = envelope_info(space.epoch, me, recipient)?;
            let cipher = envelope_cipher(&*self.shared_with(recipient)?, &info)?;
            let mut nonce = [0; NONCE_LEN];
            rand::rngs::OsRng.fill_bytes(&mut nonce);
            let ciphertext = cipher
                .encrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: &plaintext,
                        aad: &info,
                    },
                )
                .map_err(|_| invalid())?;
            result.push(SpaceEnvelope {
                epoch: space.epoch,
                sender: me.device_id.clone(),
                recipient: id.clone(),
                nonce: hex::encode(nonce),
                ciphertext: data_encoding::BASE64.encode(&ciphertext),
            });
        }
        Ok((result, changed))
    }
    /// Adopts a newer epoch sealed to this device by an active member.
    pub(crate) fn accept_space_envelope(
        &mut self,
        envelope: &SpaceEnvelope,
    ) -> Result<bool, PeerStoreError> {
        envelope.validate()?;
        if !self.active() || envelope.recipient != self.roster.issuer {
            return Err(invalid());
        }
        let sender = self
            .active_member(&envelope.sender)
            .filter(|member| member.device_id != self.roster.issuer)
            .ok_or(PeerStoreError::Revoked)?;
        let me = self.issuer_member()?;
        let info = envelope_info(envelope.epoch, sender, me)?;
        let cipher = envelope_cipher(&*self.shared_with(sender)?, &info)?;
        let mut nonce = [0; NONCE_LEN];
        hex::decode_to_slice(&envelope.nonce, &mut nonce).map_err(|_| invalid())?;
        let ciphertext = data_encoding::BASE64
            .decode(envelope.ciphertext.as_bytes())
            .map_err(|_| invalid())?;
        let plaintext = Zeroizing::new(
            cipher
                .decrypt(
                    XNonce::from_slice(&nonce),
                    Payload {
                        msg: &ciphertext,
                        aad: &info,
                    },
                )
                .map_err(|_| invalid())?,
        );
        let sealed: Sealed = serde_json::from_slice(&plaintext).map_err(|_| invalid())?;
        if sealed.epoch != envelope.epoch {
            return Err(invalid());
        }
        let incoming = SpaceKey::from_sealed(&sealed, &me.device_id, &sender.device_id)?;
        self.adopt(incoming)
    }
    /// Plaintext of the passphrase envelope, when recovery is allowed.
    pub(crate) fn space_recovery_plaintext(&self) -> Option<(u64, Zeroizing<Vec<u8>>)> {
        let space = self
            .space
            .as_ref()
            .filter(|space| space.recovery && space.epoch > 0)?;
        let plaintext = serde_json::to_vec(&space.sealed()).ok()?;
        Some((space.epoch, Zeroizing::new(plaintext)))
    }
    /// Only a device without a rotated key accepts the passphrase envelope.
    pub(crate) fn accept_space_recovery(
        &mut self,
        epoch: u64,
        plaintext: &[u8],
    ) -> Result<bool, PeerStoreError> {
        if !self.active() {
            return Err(PeerStoreError::Revoked);
        }
        if self.space.as_ref().is_some_and(|space| space.epoch > 0) {
            return Ok(false);
        }
        let sealed: Sealed = serde_json::from_slice(plaintext).map_err(|_| invalid())?;
        if sealed.epoch != epoch || epoch == 0 || !sealed.recovery {
            return Err(invalid());
        }
        let me = self.roster.issuer.clone();
        self.space = Some(SpaceKey::from_sealed(&sealed, &me, &me)?);
        Ok(true)
    }
    fn adopt(&mut self, incoming: SpaceKey) -> Result<bool, PeerStoreError> {
        match &mut self.space {
            Some(current) if incoming.order() < current.order() => Ok(false),
            Some(current) if incoming.order() == current.order() => {
                if incoming.key != current.key {
                    return Err(invalid());
                }
                let before = (current.recipients.len(), current.confirmed.len());
                current
                    .recipients
                    .extend(incoming.recipients.iter().cloned());
                current.confirmed.extend(incoming.confirmed.iter().cloned());
                Ok(before != (current.recipients.len(), current.confirmed.len()))
            }
            _ => {
                self.space = Some(incoming);
                Ok(true)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DeviceProfile;

    fn device(id: &str) -> GroupState {
        GroupState::new(id, id, DeviceProfile::default(), None).unwrap()
    }
    /// Three members that all know each other, `a` holding epoch 0.
    fn space() -> (GroupState, GroupState, GroupState) {
        let mut all = [device("a"), device("b"), device("c")];
        for _ in 0..2 {
            for i in 0..3 {
                for j in 0..3 {
                    if i != j {
                        let snapshot = all[j].snapshot().unwrap();
                        all[i].merge(&snapshot, false).unwrap();
                    }
                }
            }
        }
        let [mut a, b, c] = all;
        a.register_space_key(&[7; 32]).unwrap();
        (a, b, c)
    }
    fn deliver(from: &mut GroupState, to: &mut GroupState) -> bool {
        let (envelopes, _) = from.space_envelopes().unwrap();
        let mine = envelopes
            .iter()
            .find(|e| e.recipient == to.roster.issuer)
            .unwrap();
        to.accept_space_envelope(mine).unwrap()
    }

    #[test]
    fn removal_rotates_and_only_remaining_members_get_the_new_key() {
        let (mut a, mut b, mut c) = space();
        assert!(deliver(&mut a, &mut b));
        assert!(deliver(&mut a, &mut c));
        assert!(!a.rotate_space_if_needed().unwrap());

        a.remove("c").unwrap();
        assert!(a.rotate_space_if_needed().unwrap());
        let epoch = a.space_key().unwrap();
        assert_eq!(epoch.epoch, 1);
        let (envelopes, _) = a.space_envelopes().unwrap();
        assert!(envelopes.iter().all(|e| e.recipient == "b"));
        assert!(deliver(&mut a, &mut b));
        assert_eq!(*b.space_key().unwrap().key, *epoch.key);

        // A forged recipient field cannot make b's envelope open for c.
        let mut stolen = envelopes[0].clone();
        stolen.recipient = "c".into();
        assert!(c.accept_space_envelope(&stolen).is_err());
        c.merge(&a.snapshot().unwrap(), false).unwrap();
        assert!(c.space_key().is_none());
    }

    #[test]
    fn concurrent_rotations_converge_on_the_highest_creator() {
        let (mut a, mut b, mut c) = space();
        deliver(&mut a, &mut b);
        deliver(&mut a, &mut c);
        a.remove("c").unwrap();
        b.remove("c").unwrap();
        a.rotate_space_if_needed().unwrap();
        b.rotate_space_if_needed().unwrap();
        // b's epoch wins on both devices; a's older choice is ignored by b.
        assert!(deliver(&mut b, &mut a));
        let chosen = b.space_key().unwrap();
        deliver(&mut a, &mut b);
        assert_eq!(*b.space_key().unwrap().key, *chosen.key);
        assert!(b.space_info().unwrap().waiting.is_empty());
        assert_eq!(*a.space_key().unwrap().key, *b.space_key().unwrap().key);
        assert_eq!(a.space_info().unwrap().epoch, 1);
    }

    #[test]
    fn recovery_is_closed_after_removal_until_allowed() {
        let (mut a, mut b, _) = space();
        deliver(&mut a, &mut b);
        a.remove("c").unwrap();
        a.rotate_space_if_needed().unwrap();
        assert!(a.space_recovery_plaintext().is_none());
        a.allow_space_recovery().unwrap();
        let (epoch, plaintext) = a.space_recovery_plaintext().unwrap();
        let mut fresh = device("d");
        assert!(fresh.accept_space_recovery(epoch, &plaintext).unwrap());
        assert_eq!(*fresh.space_key().unwrap().key, *a.space_key().unwrap().key);
    }

    #[test]
    fn a_member_that_left_is_removed_everywhere_and_triggers_rotation() {
        let (mut a, mut b, mut c) = space();
        deliver(&mut a, &mut b);
        deliver(&mut a, &mut c);
        assert!(c.leave().unwrap());
        assert!(c.has_space());
        b.merge(&c.snapshot().unwrap(), false).unwrap();
        assert!(b.removed_device("c"));
        assert!(b.rotate_space_if_needed().unwrap());
        assert_eq!(b.space_info().unwrap().waiting, vec!["a".to_string()]);
    }

    #[test]
    fn two_populated_spaces_do_not_merge_on_admission() {
        let (a, _, _) = space();
        let (mut x, mut y) = (device("x"), device("y"));
        x.merge(&y.snapshot().unwrap(), false).unwrap();
        y.merge(&x.snapshot().unwrap(), false).unwrap();
        assert!(matches!(
            x.merge(&a.snapshot().unwrap(), true),
            Err(PeerStoreError::OtherSpace)
        ));
        let mut lone = device("z");
        assert!(lone.merge(&a.snapshot().unwrap(), true).unwrap());
    }
}
