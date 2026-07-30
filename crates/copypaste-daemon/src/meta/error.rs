//! What the sync metadata layer can fail with.
//!
//! No variant renders a path: `rusqlite`'s messages come from `sqlite3_errmsg`,
//! which does not embed the filename, and every cause is a `#[source]` rather
//! than interpolated text so it cannot reach a user (CLAUDE.md rule 4).

use copypaste_core::StoreError;

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("the history database could not be read or written")]
    Sqlite(#[source] rusqlite::Error),

    #[error("the history database could not be read or written")]
    Store(#[source] StoreError),

    /// Fail closed: a key that does not open the file is an error, never a
    /// fallback to an unkeyed read (CLAUDE.md rule 4).
    #[error("the history database could not be opened with this device's key")]
    InvalidKey,
}

impl From<rusqlite::Error> for MetaError {
    fn from(e: rusqlite::Error) -> Self {
        MetaError::Sqlite(e)
    }
}

impl From<StoreError> for MetaError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::InvalidKey | StoreError::LegacyDatabase => MetaError::InvalidKey,
            e => MetaError::Store(e),
        }
    }
}

pub(crate) fn is_not_a_database(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::NotADatabase
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_contain_no_paths() {
        for message in [
            MetaError::InvalidKey.to_string(),
            MetaError::Sqlite(rusqlite::Error::QueryReturnedNoRows).to_string(),
            MetaError::Store(StoreError::NotFound).to_string(),
        ] {
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }
}
