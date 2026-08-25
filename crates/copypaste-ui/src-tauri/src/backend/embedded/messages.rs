//! Refusal text for the in-process Android backend.

/// Reported in [`copypaste_ipc::StatusData::clipboard_backend`] so a build that
/// is not polling anything cannot be mistaken for one that is.
pub(in crate::backend::embedded) const BACKEND_NAME: &str = "android-inprocess";

pub(super) const MSG_EMPTY: &str = "There was nothing to save.";
pub(super) const MSG_TOO_LARGE: &str = "That item is larger than the size limit you set.";
pub(super) const MSG_NOT_STORED: &str = "That item could not be saved.";
pub(super) const MSG_NO_ITEM: &str = "That item is no longer there.";
pub(super) const MSG_UNSUPPORTED_CONTENT: &str =
    "That item cannot be written to this clipboard in the requested format.";
/// Refused rather than restarted: a load-more that silently began again would
/// replay the whole history and the caller could not tell.
pub(super) const MSG_BAD_CURSOR: &str = "That page marker isn't one this app issued.";
pub(super) const MSG_NO_PEER: &str = "That device isn't paired.";
