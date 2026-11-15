//! Preserve native storage causes at the synchronization boundary.
use copypaste_ipc::sync_issue::SyncIssueCode as Code;

pub(crate) fn io(error: &std::io::Error) -> Code {
    match error.kind() {
        std::io::ErrorKind::StorageFull => Code::StorageFull,
        std::io::ErrorKind::OutOfMemory => Code::ResourcesUnavailable,
        std::io::ErrorKind::PermissionDenied => Code::StoragePermissionDenied,
        std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted => Code::HistoryBusy,
        _ => Code::HistoryUnavailable,
    }
}
pub(crate) fn sqlite(error: &rusqlite::Error) -> Code {
    match error {
        rusqlite::Error::SqliteFailure(error, _) => match error.code {
            rusqlite::ErrorCode::DiskFull => Code::StorageFull,
            rusqlite::ErrorCode::OutOfMemory => Code::ResourcesUnavailable,
            rusqlite::ErrorCode::PermissionDenied
            | rusqlite::ErrorCode::ReadOnly
            | rusqlite::ErrorCode::AuthorizationForStatementDenied => Code::StoragePermissionDenied,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked => {
                Code::HistoryBusy
            }
            rusqlite::ErrorCode::DatabaseCorrupt | rusqlite::ErrorCode::NotADatabase => {
                Code::HistoryCorrupt
            }
            _ => Code::HistoryUnavailable,
        },
        _ => Code::HistoryUnavailable,
    }
}
pub(crate) fn store(error: &crate::StoreError) -> Code {
    match error {
        crate::StoreError::File(error) => io(error),
        crate::StoreError::Sqlite(error) => sqlite(error),
        crate::StoreError::Pool(_) => Code::HistoryUnavailable,
        crate::StoreError::InvalidKey => Code::HistoryLocked,
        crate::StoreError::IntegrityCheckFailed => Code::HistoryCorrupt,
        crate::StoreError::InvalidSchema => Code::HistoryIncompatible,
        crate::StoreError::InvalidPayload => Code::LocalPayloadInvalid,
        crate::StoreError::NotFound
        | crate::StoreError::InvalidCursor
        | crate::StoreError::InvalidHistoryQuery
        | crate::StoreError::InvalidDeviceName => Code::HistoryUnavailable,
    }
}
