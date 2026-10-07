//! Display-safe observations of real peer synchronization sessions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncPhase {
    #[default]
    Unavailable,
    Disabled,
    Waiting,
    Syncing,
    Synced,
    Failed,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PeerSyncStatus {
    pub pairing_id: String,
    pub name: String,
    pub phase: SyncPhase,
    pub started_at_ms: Option<i64>,
    pub last_success_ms: Option<i64>,
    pub sent: u64,
    pub received: u64,
    pub skipped_too_large: u64,
    pub error: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub revision: u64,
    pub phase: SyncPhase,
    pub peers: Vec<PeerSyncStatus>,
}
