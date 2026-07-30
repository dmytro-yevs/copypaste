//! Every string this server can put in `Response.error`, and the two mappers
//! that turn an internal failure into one of them.
//!
//! **Errors never carry a filesystem path.** The socket path discloses the
//! local username (CLAUDE.md rule 4), and a `StoreError` from SQLite routinely
//! embeds the database path. Every failure is therefore mapped to one of the
//! fixed sentences below; the underlying error goes to the local log and never
//! onto the wire.
//!
//! They are gathered in one file so `ALL_MESSAGES` below is provably the whole
//! set: the test that pins the no-path rule is only as good as its list, and a
//! message defined next to its handler is a message that can be added without
//! anyone noticing.

use std::fmt::Display;

use copypaste_ipc::{ErrorCode, Response};
use tracing::error;

pub(super) const MSG_NOT_READY: &str = "daemon is still starting up; retry shortly";
pub(super) const MSG_NOT_FOUND: &str = "item not found";
pub(super) const MSG_MALFORMED: &str = "malformed request";
pub(super) const MSG_TOO_LARGE: &str = "request exceeds the maximum frame size";
pub(super) const MSG_EMPTY_CONTENT: &str = "content must not be empty";
pub(super) const MSG_STORAGE: &str = "the history database could not be accessed";
pub(super) const MSG_DECRYPT: &str = "the stored item could not be decrypted";
pub(super) const MSG_ENCRYPT: &str = "the item could not be encrypted";
pub(super) const MSG_CLIPBOARD: &str = "the system clipboard could not be written";
pub(super) const MSG_INTERNAL: &str = "the daemon failed to process the request";
pub(super) const MSG_WATCHERS_FULL: &str = "too many clients are already watching for changes";
pub(super) const MSG_TOO_BIG: &str = "the item is larger than the configured size limit";
pub(super) const MSG_IMPORT_EMPTY: &str = "there is nothing to import";
pub(super) const MSG_IMPORT_TOO_MANY: &str =
    "too many items in one import; split the file into smaller batches";
pub(super) const MSG_BAD_PATH: &str = "a destination is required";
pub(super) const MSG_BACKUP_EXISTS: &str =
    "a file is already there; choose a name that does not exist yet";
pub(super) const MSG_BACKUP_NO_DIR: &str = "the folder to write the backup into does not exist";
pub(super) const MSG_BACKUP_FAILED: &str = "the backup could not be written";
pub(super) const MSG_NEEDS_CONFIRM: &str = "restoring replaces this device's history; confirm it";
pub(super) const MSG_RESTORE_NOT_FOUND: &str = "there is no backup file there";
pub(super) const MSG_RESTORE_NOT_A_BACKUP: &str =
    "that file is not a CopyPaste backup for this device, or it is damaged; nothing was changed";
pub(super) const MSG_RESTORE_FAILED: &str =
    "the restore could not be completed; this device's history is unchanged";

/// Map a storage failure onto the wire.
///
/// The error itself is logged and dropped: a `StoreError` from SQLite carries
/// the database path, and a path in a client-visible string discloses the local
/// username.
pub(super) fn storage_error(id: u64, operation: &'static str, error: impl Display) -> Response {
    error!(operation, error = %error, "storage operation failed");
    Response::err(id, ErrorCode::Internal, MSG_STORAGE)
}

pub(super) fn decrypt_error(id: u64, error: copypaste_core::CryptoError) -> Response {
    error!(error = ?error, "decryption failed");
    Response::err(id, ErrorCode::Internal, MSG_DECRYPT)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every string this server can put in `Response.error`. Gathering the
    /// constants in one file is what makes this list provably complete.
    const ALL_MESSAGES: &[&str] = &[
        MSG_NOT_READY,
        MSG_NOT_FOUND,
        MSG_MALFORMED,
        MSG_TOO_LARGE,
        MSG_EMPTY_CONTENT,
        MSG_STORAGE,
        MSG_DECRYPT,
        MSG_ENCRYPT,
        MSG_CLIPBOARD,
        MSG_INTERNAL,
        MSG_WATCHERS_FULL,
        MSG_TOO_BIG,
        MSG_IMPORT_EMPTY,
        MSG_IMPORT_TOO_MANY,
        MSG_BAD_PATH,
        MSG_BACKUP_EXISTS,
        MSG_BACKUP_NO_DIR,
        MSG_BACKUP_FAILED,
        MSG_NEEDS_CONFIRM,
        MSG_RESTORE_NOT_FOUND,
        MSG_RESTORE_NOT_A_BACKUP,
        MSG_RESTORE_FAILED,
    ];

    #[test]
    fn error_messages_never_disclose_a_path() {
        // CLAUDE.md rule 4. The socket path contains the local username, and
        // the database path is right next to it; the cheapest way to keep both
        // off the wire is to keep every separator out of every message.
        for message in ALL_MESSAGES {
            assert!(
                !message.contains('/') && !message.contains('\\'),
                "client-visible message looks like it contains a path: {message}"
            );
        }
    }
}
