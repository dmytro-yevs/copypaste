//! Local file-operation history. File contents never enter this contract.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileOperationKind {
    Import,
    Receive,
    Send,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileOperationPhase {
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileOperation {
    pub id: String,
    pub item_id: String,
    pub name: String,
    pub kind: FileOperationKind,
    pub phase: FileOperationPhase,
    pub source_device: String,
    pub destination_device: String,
    pub written: u64,
    pub total: u64,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub error: Option<String>,
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileOperationUpdate {
    pub id: Option<String>,
    pub phase: Option<FileOperationPhase>,
    pub written: u64,
    pub total: u64,
}
