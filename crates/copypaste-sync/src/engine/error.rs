//! Sync protocol error type.

/// Error type for sync operations.
#[derive(Debug)]
pub enum SyncError {
    /// I/O error on the underlying stream.
    Io(std::io::Error),
    /// JSON (de)serialisation failure.
    Json(serde_json::Error),
    /// Peer sent a frame larger than `MAX_FRAME_SIZE`.
    FrameTooLarge(u32),
    /// Peer sent a message out of sequence.
    ProtocolViolation(String),
}

impl std::fmt::Display for SyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SyncError::Io(e) => write!(f, "IO error: {e}"),
            SyncError::Json(e) => write!(f, "JSON error: {e}"),
            SyncError::FrameTooLarge(n) => write!(f, "frame too large: {n} bytes"),
            SyncError::ProtocolViolation(s) => write!(f, "protocol violation: {s}"),
        }
    }
}

impl std::error::Error for SyncError {}

impl From<std::io::Error> for SyncError {
    fn from(e: std::io::Error) -> Self {
        SyncError::Io(e)
    }
}

impl From<serde_json::Error> for SyncError {
    fn from(e: serde_json::Error) -> Self {
        SyncError::Json(e)
    }
}
