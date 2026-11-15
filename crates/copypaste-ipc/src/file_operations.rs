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
    /// The saved file or, for several files, their folder. A receive that
    /// stopped early keeps its destination so it can be resumed.
    pub local_path: Option<String>,
    #[serde(default = "single_file", skip_serializing_if = "is_single_file")]
    pub file_count: u32,
    /// An interrupted receive whose verified bytes are kept for resuming.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub resumable: bool,
}

fn single_file() -> u32 {
    1
}
fn is_single_file(count: &u32) -> bool {
    *count == 1
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FileOperationUpdate {
    pub id: Option<String>,
    pub phase: Option<FileOperationPhase>,
    pub written: u64,
    pub total: u64,
}
