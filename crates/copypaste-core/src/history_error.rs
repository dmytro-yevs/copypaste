//! Pathless errors shared by History operations and their IPC adapters.
use copypaste_ipc::{ErrorCode, Response};

#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
    #[error("item not found")]
    NotFound,
    #[error("the history database could not be accessed")]
    Store(#[source] crate::StoreError),
    #[error("the stored item could not be decrypted")]
    Decrypt(#[source] crate::CryptoError),
    #[error("This content is too large for this operation. Your history is unchanged.")]
    ContentTooLarge,
    #[error("this item has no supported representation for that clipboard action")]
    Unsupported,
    #[error("the system clipboard could not be written")]
    Clipboard,
    #[error("a destination is required")]
    BadPath,
    #[error("a file is already there; choose a name that does not exist yet")]
    Exists,
    #[error("the folder to write the backup into does not exist")]
    MissingDirectory,
    #[error("the folder to write the backup into does not exist")]
    BackupMissingDirectory,
    #[error("the file could not be saved")]
    Save,
    #[error("the backup could not be written")]
    Backup,
    #[error("restoring replaces this device's history; confirm it")]
    NeedsConfirmation,
    #[error("there is no backup file there")]
    BackupNotFound,
    #[error("that file is not a CopyPaste backup for this device, or it is damaged; nothing was changed")]
    InvalidBackup,
    #[error("the restore could not be completed; this device's history is unchanged")]
    Restore,
    #[error("{0}")]
    File(#[from] crate::file_transfer::FileTransferError),
}
impl HistoryError {
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::NotFound | Self::MissingDirectory | Self::BackupNotFound => ErrorCode::NotFound,
            Self::ContentTooLarge => ErrorCode::ContentTooLarge,
            Self::Unsupported => ErrorCode::UnsupportedContent,
            Self::BackupMissingDirectory
            | Self::BadPath
            | Self::Exists
            | Self::NeedsConfirmation
            | Self::InvalidBackup
            | Self::File(_) => ErrorCode::InvalidRequest,
            _ => ErrorCode::Internal,
        }
    }
    pub fn response(&self, id: u64) -> Response {
        tracing::warn!(error = ?self, "History operation refused");
        Response::err(id, self.code(), self.to_string())
    }
}
impl From<crate::ClipboardWriteError> for HistoryError {
    fn from(error: crate::ClipboardWriteError) -> Self {
        match error {
            crate::ClipboardWriteError::UnsupportedContent => Self::Unsupported,
            crate::ClipboardWriteError::Failed => Self::Clipboard,
        }
    }
}
