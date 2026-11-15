//! The space key contract for sync providers, over the device group.
use std::sync::Arc;

use copypaste_p2p::{group, Node};
use copypaste_sync::space::{SpaceEnvelope, SpaceHost, SpaceKey, SpaceNotice, SpaceRecovery};

pub(crate) struct NodeSpace(pub(crate) Arc<Node>);

fn error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
fn to_wire(envelope: group::SpaceEnvelope) -> SpaceEnvelope {
    SpaceEnvelope {
        epoch: envelope.epoch,
        sender: envelope.sender,
        recipient: envelope.recipient,
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext,
    }
}
fn from_wire(envelope: &SpaceEnvelope) -> group::SpaceEnvelope {
    group::SpaceEnvelope {
        epoch: envelope.epoch,
        sender: envelope.sender.clone(),
        recipient: envelope.recipient.clone(),
        nonce: envelope.nonce.clone(),
        ciphertext: envelope.ciphertext.clone(),
    }
}

impl SpaceHost for NodeSpace {
    fn key(&self, fallback: Option<&[u8; 32]>) -> Result<Option<SpaceKey>, String> {
        let peers = self.0.peers();
        if let Some(key) = fallback {
            if peers.group_active_member() && peers.space_key().is_none() {
                peers.register_space_key(key).map_err(error)?;
            }
        }
        Ok(peers.space_key().map(|key| SpaceKey {
            epoch: key.epoch,
            key: key.key,
            active: peers.group_active_member(),
        }))
    }
    fn rotate(&self) -> Result<bool, String> {
        self.0.peers().rotate_space_if_needed().map_err(error)
    }
    fn envelopes(&self) -> Result<Vec<SpaceEnvelope>, String> {
        Ok(self
            .0
            .peers()
            .space_envelopes()
            .map_err(error)?
            .into_iter()
            .map(to_wire)
            .collect())
    }
    fn accept(&self, envelopes: &[SpaceEnvelope]) -> Result<bool, String> {
        let peers = self.0.peers();
        let mut changed = false;
        for envelope in envelopes {
            // One unreadable or stale envelope must not hide a valid one.
            changed |= peers
                .accept_space_envelope(&from_wire(envelope))
                .unwrap_or(false);
        }
        Ok(changed)
    }
    fn recovery(&self) -> Result<Option<SpaceRecovery>, String> {
        Ok(self
            .0
            .peers()
            .space_recovery_plaintext()
            .map(|(epoch, plaintext)| SpaceRecovery { epoch, plaintext }))
    }
    fn recover(&self, epoch: u64, plaintext: &[u8]) -> Result<bool, String> {
        Ok(self
            .0
            .peers()
            .accept_space_recovery(epoch, plaintext)
            .unwrap_or(false))
    }
    fn allow_recovery(&self) -> Result<bool, String> {
        self.0.peers().allow_space_recovery().map_err(error)
    }
    fn confirm(&self, device_id: &str, epoch: u64) -> Result<(), String> {
        self.0
            .peers()
            .confirm_space_member(device_id, epoch)
            .map(|_| ())
            .map_err(error)
    }
    fn notice(&self) -> Result<Option<SpaceNotice>, String> {
        Ok(self
            .0
            .peers()
            .space_removal_notice()
            .map(|notice| SpaceNotice {
                previous_epoch: notice.previous_epoch,
                previous_key: notice.previous_key,
                recipients: notice.recipients,
            }))
    }
    fn noticed(&self) -> Result<(), String> {
        self.0
            .peers()
            .clear_space_removal_notice()
            .map(|_| ())
            .map_err(error)
    }
    fn departed(&self) -> Result<(), String> {
        self.0
            .peers()
            .forget_departed_space()
            .map(|_| ())
            .map_err(error)
    }
}
