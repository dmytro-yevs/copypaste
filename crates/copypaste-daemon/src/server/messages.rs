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
//! set *this module authors*: the test that pins the no-path rule is only as
//! good as its list, and a message defined next to its handler is a message
//! that can be added without anyone noticing.
//!
//! One set is not here: `crate::p2p::handlers` puts `copypaste_p2p::NodeError`
//! straight onto the wire, so those sentences are pinned pathless by that
//! crate's own test rather than by this one.

use std::fmt::Display;

use copypaste_core::{CryptoError, StoreError};
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
pub(super) const MSG_REORDER_TOO_MANY: &str =
    "too many items in one reorder; a pinned list is never this long";
pub(super) const MSG_IMAGE_PREVIEW: &str = "an image preview is unavailable for this item";
/// Refused rather than restarted from the top: a load-more that silently began
/// again would repeat the whole history, and a client cannot tell that from a
/// list that really does.
pub(super) const MSG_BAD_CURSOR: &str = "that page marker is not one this service issued";
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

pub(crate) const MSG_KEY_LOCKED: &str =
    "the key store could not be read, so this history could not be unlocked; \
     it is worth trying again once the key store is available";
/// Says the two things `MSG_KEY_LOCKED` does not: no retry helps, and the
/// history is gone rather than waiting.
pub(crate) const MSG_KEY_UNUSABLE: &str =
    "this device's key is present and cannot be used, so the history encrypted with it \
     cannot be read by anything; trying again will not change that";

/// A failure that carries a code of its own, distinct from the operation that
/// met it.
///
/// `None` is the common case — the caller's own sentence applies. `Some` is
/// reserved for conditions a client has to *render differently*: two of the
/// three below are unrecoverable, and a client that offers **Try again**
/// against them is the defect this exists to make impossible to reintroduce
/// (`ErrorCode::retryable`).
pub(crate) trait Refusal {
    fn refusal(&self) -> Option<(ErrorCode, &'static str)>;
}

impl Refusal for StoreError {
    fn refusal(&self) -> Option<(ErrorCode, &'static str)> {
        let _ = self;
        None
    }
}

impl Refusal for CryptoError {
    fn refusal(&self) -> Option<(ErrorCode, &'static str)> {
        match self {
            CryptoError::KeystoreUnavailable(_) => Some((ErrorCode::KeyLocked, MSG_KEY_LOCKED)),
            CryptoError::KeystoreEntryUnusable(_) => {
                Some((ErrorCode::KeyUnusable, MSG_KEY_UNUSABLE))
            }
            // `AuthFailed` deliberately stays indistinguishable from the rest
            // (manifest 02, I-15): distinguishing it turns decryption into an
            // oracle.
            _ => None,
        }
    }
}

/// Map a storage failure onto the wire.
///
/// The error itself is logged and dropped: a `StoreError` from SQLite carries
/// the database path, and a path in a client-visible string discloses the local
/// username. What crosses is one of the fixed sentences above.
pub(super) fn storage_error(
    id: u64,
    operation: &'static str,
    error: &(impl Refusal + Display),
) -> Response {
    error!(operation, error = %error, "storage operation failed");
    let (code, message) = error
        .refusal()
        .unwrap_or((ErrorCode::Internal, MSG_STORAGE));
    Response::err(id, code, message)
}

pub(super) fn decrypt_error(id: u64, error: &CryptoError) -> Response {
    error!(error = ?error, "decryption failed");
    let (code, message) = error
        .refusal()
        .unwrap_or((ErrorCode::Internal, MSG_DECRYPT));
    Response::err(id, code, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every string this module puts in `Response.error`. Gathering the
    /// constants in one file is what makes this list provably complete — see
    /// the module header for the one set that is pinned elsewhere.
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
        MSG_REORDER_TOO_MANY,
        MSG_IMAGE_PREVIEW,
        MSG_BAD_CURSOR,
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
        MSG_KEY_LOCKED,
        MSG_KEY_UNUSABLE,
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

    /// No message may name a file either. These two are *about* a file, which
    /// makes them the likeliest place for one to be written in by hand — and a
    /// filename with no separators would walk straight past the check above.
    #[test]
    fn no_message_names_a_file() {
        for message in ALL_MESSAGES {
            for fragment in [".db", ".sqlite", ".sock", "clipboard.db", "~"] {
                assert!(
                    !message.contains(fragment),
                    "client-visible message names a file: {message}"
                );
            }
        }
    }

    /// The whole reason `KeystoreEntryUnusable` is a separate variant: the two
    /// key-store failures must not collapse into one message, because one is
    /// worth waiting out and one is not.
    #[test]
    fn the_two_key_store_failures_do_not_collapse() {
        let locked = CryptoError::KeystoreUnavailable("locked")
            .refusal()
            .expect("its own code");
        let unusable = CryptoError::KeystoreEntryUnusable("wrong length")
            .refusal()
            .expect("its own code");
        assert_ne!(locked, unusable);
        assert!(locked.0.retryable());
        assert!(!unusable.0.retryable());
    }

    /// Fail-closed decryption keeps one sentence for every reason it can fail
    /// (manifest 02, I-15) — the classifier must not become an oracle.
    #[test]
    fn a_failed_decryption_stays_indistinguishable() {
        assert!(CryptoError::AuthFailed.refusal().is_none());
        assert!(CryptoError::InvalidNonce.refusal().is_none());
        let response = decrypt_error(1, &CryptoError::AuthFailed);
        assert_eq!(response.error.as_deref(), Some(MSG_DECRYPT));
        assert_eq!(response.error_code, Some(ErrorCode::Internal));
    }

    #[test]
    fn an_ordinary_storage_failure_keeps_the_generic_sentence() {
        let response = storage_error(1, "list", &StoreError::NotFound);
        assert_eq!(response.error.as_deref(), Some(MSG_STORAGE));
        assert_eq!(response.error_code, Some(ErrorCode::Internal));
    }
}
