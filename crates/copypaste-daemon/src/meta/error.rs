//! What the sync metadata layer can fail with.

/// Everything this module can fail with.
///
/// No variant renders a path: `rusqlite`'s messages come from
/// `sqlite3_errmsg`, which does not embed the filename, and the cause is a
/// `#[source]` rather than interpolated text so it cannot reach a user
/// (`CLAUDE.md` rule 4).
#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("the history database could not be read or written")]
    Sqlite(#[source] rusqlite::Error),

    /// Fail closed: a key that does not open the file is an error, never a
    /// fallback to an unkeyed read (`CLAUDE.md` rule 4).
    #[error("the history database could not be opened with this device's key")]
    InvalidKey,

    #[error("the sync metadata is no longer usable in this process")]
    Poisoned,
}

impl From<rusqlite::Error> for MetaError {
    fn from(e: rusqlite::Error) -> Self {
        MetaError::Sqlite(e)
    }
}

pub(super) fn is_not_a_database(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::NotADatabase
    )
}

pub(super) fn is_constraint_violation(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(err, _)
            if err.code == rusqlite::ErrorCode::ConstraintViolation
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_messages_contain_no_paths() {
        for message in [
            MetaError::InvalidKey.to_string(),
            MetaError::Poisoned.to_string(),
            MetaError::Sqlite(rusqlite::Error::QueryReturnedNoRows).to_string(),
        ] {
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }
}
