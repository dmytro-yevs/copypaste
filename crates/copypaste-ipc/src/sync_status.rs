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
    #[serde(default)]
    pub issues: Vec<crate::sync_issue::SyncIssue>,
    #[serde(default)]
    pub connection: crate::DevicePresence,
    #[serde(default)]
    pub checking: bool,
    #[serde(default = "default_address_known")]
    pub address_known: bool,
}

fn default_address_known() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncStatus {
    pub revision: u64,
    pub phase: SyncPhase,
    pub peers: Vec<PeerSyncStatus>,
    #[serde(default)]
    pub log: Vec<SyncLogEntry>,
    #[serde(default = "default_available")]
    pub diagnostics_available: bool,
}
fn default_available() -> bool {
    true
}
impl Default for SyncStatus {
    fn default() -> Self {
        Self {
            revision: 0,
            phase: SyncPhase::Unavailable,
            peers: Vec::new(),
            log: Vec::new(),
            diagnostics_available: true,
        }
    }
}

/// A completed attempt, retained independently of the current connection state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncLogEntry {
    pub pairing_id: String,
    pub timestamp_ms: i64,
    pub kind: SyncLogKind,
    pub error: Option<String>,
    pub sent: u64,
    pub received: u64,
    #[serde(default)]
    pub issue_code: Option<crate::sync_issue::SyncIssueCode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncLogKind {
    Offline,
    Online,
    Failed,
    Recovered,
    Transferred,
    ProblemDetected,
    ProblemResolved,
}
