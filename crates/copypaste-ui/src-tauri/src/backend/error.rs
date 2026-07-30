//! The one error type both backends return, and the two rules it enforces.
//!
//! # Rule 1 — no filesystem path reaches a user
//!
//! The daemon socket lives under the user's home directory, so its path spells
//! out the local username (CLAUDE.md rule 4, manifest 06). Everything that
//! arrives from outside this crate is passed through
//! [`copypaste_ipc::redact::scrub_paths`] — the module every client shares,
//! never a second copy of it — on the way in, at
//! [`BackendError::from_daemon`] and [`BackendError::internal`].
//!
//! The variants that this crate writes itself hold `&'static str`, so there is
//! no way to interpolate a path into one even by accident. That is the same
//! trick `copypaste_core::CryptoError` uses, and it is why those two variants
//! are not simply `String`.
//!
//! # Rule 2 — the message is the whole message
//!
//! `Display` is the exact sentence the user sees; the frontend renders it
//! verbatim and adds nothing. So each string has to be a complete, actionable
//! sentence rather than a fragment to be prefixed with "Error: ".

use copypaste_ipc::redact::scrub_paths;
use copypaste_ipc::ErrorCode;

/// A failure fit to show a user.
#[derive(Debug, Clone, thiserror::Error)]
pub enum BackendError {
    /// Nothing is listening on the socket, or it went away mid-request.
    ///
    /// Every connect failure collapses to this one message. The underlying kind
    /// (missing, refused, permission denied) is not actionably different to a
    /// user, and distinguishing it would tempt someone to include the path in
    /// order to explain the difference.
    ///
    /// The message names no command: the app starts the service itself
    /// (ADR-0004), so telling a user to open a terminal would be advice for a
    /// button that is already on screen.
    #[error("The background service isn't running.")]
    Unreachable,

    /// The daemon answered, and said no.
    #[error("{0}")]
    Daemon(String),

    /// The backend is still coming up.
    #[error("CopyPaste is still starting up. Try again in a moment.")]
    NotReady,

    /// App and daemon disagree about the protocol.
    #[error("The app and the daemon are different versions. Upgrade both and restart the daemon.")]
    ProtocolMismatch,

    /// No such item, or no such peer.
    #[error("{0}")]
    NotFound(&'static str),

    /// The caller asked for something the request itself forbids.
    #[error("{0}")]
    Invalid(&'static str),

    /// Something failed inside this process.
    #[error("{0}")]
    Internal(String),

    /// This build cannot do this yet, and the reason is structural rather than
    /// transient.
    ///
    /// Used by the in-process backend for the operations whose implementation
    /// still lives inside the `copypaste-daemon` binary. Retrying will not
    /// help, so it must not read like a transient failure — see
    /// `backend::embedded` for the full list and what has to move.
    #[error("{0}")]
    Unsupported(&'static str),
}

impl BackendError {
    /// Build an error from text that came from outside this crate.
    ///
    /// The scrub is here rather than at each call site so that forgetting it is
    /// not possible: there is no other public constructor that takes a
    /// `String`.
    pub fn from_daemon(message: &str) -> Self {
        Self::Daemon(scrub_paths(message))
    }

    /// Build an internal failure from text this crate did not author.
    ///
    /// Scrubbed for the same reason: a `StoreError` or an `io::Error` rendered
    /// into a string is exactly where a path appears.
    pub fn internal(message: &str) -> Self {
        Self::Internal(scrub_paths(message))
    }

    /// Map a daemon `error_code` plus its text onto a variant.
    ///
    /// Branching on the code and never on the string is manifest 04 I9: the
    /// text is for humans and may be reworded, the code is the contract.
    pub fn from_code(code: Option<ErrorCode>, message: Option<&str>) -> Self {
        match code {
            Some(ErrorCode::NotReady) => Self::NotReady,
            Some(ErrorCode::ProtocolMismatch) => Self::ProtocolMismatch,
            Some(ErrorCode::NotFound) => Self::NotFound("That item is no longer there."),
            _ => Self::from_daemon(
                message.unwrap_or("The daemon reported a failure but gave no reason."),
            ),
        }
    }

    /// The reply had the right shape for a success but the wrong payload.
    pub fn wrong_shape(expected: &'static str) -> Self {
        // Not a `format!` into `Daemon`: the expected-shape word is one of a
        // fixed set written in this crate, so it stays a `&'static str` and the
        // sentence cannot pick up caller data.
        Self::Internal(format!(
            "The daemon replied with something other than {expected}; \
             the app and the daemon may be different versions."
        ))
    }
}

/// Tauri renders a command failure by serialising it, so the wire form has to
/// be the sentence itself rather than a struct the frontend would have to know
/// how to read.
impl serde::Serialize for BackendError {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The property the whole module exists for.
    #[test]
    fn a_path_from_the_daemon_is_scrubbed_on_the_way_in() {
        let err = BackendError::from_daemon(
            "could not open /Users/dmitriy/Library/Application Support/CopyPaste/x.db",
        );
        let shown = err.to_string();
        assert!(!shown.contains("dmitriy"), "{shown}");
        assert!(shown.contains("<path>"), "{shown}");
    }

    #[test]
    fn an_internal_failure_is_scrubbed_too() {
        let err = BackendError::internal("no such file or directory: /home/bob/.local/x");
        assert!(!err.to_string().contains("bob"), "{err}");
    }

    /// The unreachable message must name the condition, name no path, and —
    /// since ADR-0004 — name no terminal command either: the app owns the
    /// service's lifetime, so the recovery is a button, not a shell.
    #[test]
    fn the_unreachable_message_names_no_path_and_no_command() {
        let shown = BackendError::Unreachable.to_string();
        assert!(!shown.contains('/'), "{shown}");
        assert!(!shown.contains("copypaste-daemon"), "{shown}");
        assert!(shown.contains("background service"), "{shown}");
    }

    /// Every variant's rendered text, including the ones this crate authors,
    /// has to survive the same check — a hard-coded sentence is just as capable
    /// of naming a path as an interpolated one.
    #[test]
    fn no_variant_renders_a_path() {
        let cases = [
            BackendError::Unreachable,
            BackendError::NotReady,
            BackendError::ProtocolMismatch,
            BackendError::NotFound("That item is no longer there."),
            BackendError::Invalid("There is nothing to add."),
            BackendError::Unsupported("Pairing is not available in this build."),
            BackendError::wrong_shape("a list of items"),
            BackendError::from_daemon("plain trouble"),
            BackendError::internal("plain trouble"),
        ];
        for case in cases {
            let shown = case.to_string();
            assert!(!shown.contains("/Users/"), "{shown}");
            assert!(!shown.contains("/home/"), "{shown}");
            assert!(!shown.starts_with('/'), "{shown}");
        }
    }

    #[test]
    fn codes_map_to_variants_and_the_string_is_ignored() {
        assert!(matches!(
            BackendError::from_code(Some(ErrorCode::NotReady), Some("anything at all")),
            BackendError::NotReady
        ));
        assert!(matches!(
            BackendError::from_code(Some(ErrorCode::ProtocolMismatch), None),
            BackendError::ProtocolMismatch
        ));
        assert!(matches!(
            BackendError::from_code(Some(ErrorCode::NotFound), Some("gone")),
            BackendError::NotFound(_)
        ));
        // An untagged failure still becomes an error rather than a success.
        assert!(matches!(
            BackendError::from_code(None, Some("unknown method")),
            BackendError::Daemon(_)
        ));
    }

    #[test]
    fn serialises_as_the_sentence_itself() {
        let json = serde_json::to_string(&BackendError::NotReady).unwrap();
        assert_eq!(
            json,
            "\"CopyPaste is still starting up. Try again in a moment.\""
        );
    }
}
