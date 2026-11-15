//! The device space as clients see it. Members themselves are `Peers`.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpaceData {
    /// False after another device removed this one, or after it left.
    pub member: bool,
    /// The cloud key epoch; `None` until a sync provider registered a key.
    pub key_epoch: Option<u64>,
    /// Whether a device can still join with the sync passphrase.
    pub passphrase_recovery: bool,
    /// Pairings of members that have not yet confirmed the current cloud key.
    pub waiting_pairing_ids: Vec<String>,
}
