//! Space key contracts shared by the application and sync providers.
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

/// Recipient name of the passphrase-wrapped envelope.
pub const RECOVERY_RECIPIENT: &str = "recovery";
/// Room for a removal notice: a whole signed roster, as in roster rows.
pub const MAX_ENVELOPE_CIPHERTEXT: usize = 1_048_600;

/// One epoch key sealed for one recipient. Member envelopes are sealed by the
/// application; the recovery envelope by the provider with its passphrase key.
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
    pub fn is_valid(&self) -> bool {
        fn id(value: &str) -> bool {
            !value.is_empty()
                && value.len() <= 128
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        }
        id(&self.sender)
            && id(&self.recipient)
            && self.epoch < 1 << 53
            && (32..=48).contains(&self.nonce.len())
            && self
                .nonce
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"+/=".contains(&b))
            && !self.ciphertext.is_empty()
            && self.ciphertext.len() <= MAX_ENVELOPE_CIPHERTEXT
    }
}

/// The key a provider writes and reads cloud data with.
pub struct SpaceKey {
    pub epoch: u64,
    pub key: Zeroizing<[u8; 32]>,
    /// False on a device that left and only publishes its departure.
    pub active: bool,
}

/// Plaintext of the recovery envelope for the current epoch.
pub struct SpaceRecovery {
    pub epoch: u64,
    pub plaintext: Zeroizing<Vec<u8>>,
}

/// Removed devices to tell, and the key they still hold.
pub struct SpaceNotice {
    pub previous_epoch: u64,
    pub previous_key: Zeroizing<[u8; 32]>,
    pub recipients: Vec<String>,
}

/// Wire reply of [`crate::host::SyncHostRequest::SpaceNotice`].
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceNoticeReply {
    pub previous_epoch: u64,
    pub previous_key: Zeroizing<String>,
    pub recipients: Vec<String>,
}

/// Wire reply of [`crate::host::SyncHostRequest::SpaceKey`].
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceKeyReply {
    pub epoch: u64,
    pub key: Zeroizing<String>,
    pub active: bool,
}

/// Wire reply of [`crate::host::SyncHostRequest::SpaceRecovery`].
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceRecoveryReply {
    pub epoch: u64,
    pub plaintext: Zeroizing<String>,
}

/// Application side of the space contract, implemented over the peer store.
pub trait SpaceHost: Send + Sync {
    /// Current key; registers `fallback` as epoch 0 when there is none.
    fn key(&self, fallback: Option<&[u8; 32]>) -> Result<Option<SpaceKey>, String>;
    fn rotate(&self) -> Result<bool, String>;
    fn envelopes(&self) -> Result<Vec<SpaceEnvelope>, String>;
    fn accept(&self, envelopes: &[SpaceEnvelope]) -> Result<bool, String>;
    fn recovery(&self) -> Result<Option<SpaceRecovery>, String>;
    fn recover(&self, epoch: u64, plaintext: &[u8]) -> Result<bool, String>;
    fn allow_recovery(&self) -> Result<bool, String>;
    fn confirm(&self, device_id: &str, epoch: u64) -> Result<(), String>;
    /// The departure of this device was published; forget the key.
    fn departed(&self) -> Result<(), String>;
    fn notice(&self) -> Result<Option<SpaceNotice>, String>;
    fn noticed(&self) -> Result<(), String>;
}
