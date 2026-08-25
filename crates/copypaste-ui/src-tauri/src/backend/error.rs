//! The one error type both backends return, and its WebView boundary.
//!
//! # Rule 1 — no filesystem path reaches a user
//!
//! The daemon socket lives under the user's home directory, so its path spells
//! out the local username (AGENTS.md rule 4, manifest 06). Everything that
//! arrives from outside this crate is passed through
//! [`copypaste_ipc::redact::scrub_paths`] — the module every client shares,
//! never a second copy of it — on the way in, at
//! [`BackendError::from_daemon`] and [`BackendError::internal`]. Internal text
//! remains useful in logs but is never part of the serialised [`UiError`].
//!
//! The variants that this crate writes itself hold `&'static str`, so there is
//! no way to interpolate a path into one even by accident. That is the same
//! trick `copypaste_core::CryptoError` uses, and it is why those two variants
//! are not simply `String`.
//!
//! # Rule 2 — the UI owns the message
//!
//! Tauri receives only `{ code, retryable }`. The frontend maps `code` to
//! localised actionable copy, so neither a daemon sentence nor a path can be
//! rendered by accident. A valid future snake_case code is preserved and
//! falls back to the frontend's unknown-code copy.

use copypaste_ipc::redact::scrub_paths;
use copypaste_ipc::ErrorCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum UiBoundaryErrorCode {
    Offline,
    Timeout,
    Unavailable,
    UpdateBusy,
    UpdateUnconfigured,
    UpdateUnsupported,
    UpdateSignatureInvalid,
    UpdateNetworkFailed,
    UpdateCheckFailed,
    UpdateInstallFailed,
    UpdatePermissionRequired,
    Unknown,
}

impl UiBoundaryErrorCode {
    pub const ALL: &'static [Self] = &[
        Self::Offline,
        Self::Timeout,
        Self::Unavailable,
        Self::UpdateBusy,
        Self::UpdateUnconfigured,
        Self::UpdateUnsupported,
        Self::UpdateSignatureInvalid,
        Self::UpdateNetworkFailed,
        Self::UpdateCheckFailed,
        Self::UpdateInstallFailed,
        Self::UpdatePermissionRequired,
        Self::Unknown,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Timeout => "timeout",
            Self::Unavailable => "unavailable",
            Self::UpdateBusy => "update_busy",
            Self::UpdateUnconfigured => "update_unconfigured",
            Self::UpdateUnsupported => "update_unsupported",
            Self::UpdateSignatureInvalid => "update_signature_invalid",
            Self::UpdateNetworkFailed => "update_network_failed",
            Self::UpdateCheckFailed => "update_check_failed",
            Self::UpdateInstallFailed => "update_install_failed",
            Self::UpdatePermissionRequired => "update_permission_required",
            Self::Unknown => "unknown",
        }
    }

    pub const fn retryable(self) -> bool {
        matches!(
            self,
            Self::Offline
                | Self::Timeout
                | Self::UpdateBusy
                | Self::UpdateNetworkFailed
                | Self::UpdateCheckFailed
                | Self::UpdatePermissionRequired
                | Self::Unknown
        )
    }
}

/// The complete and deliberately small Tauri error contract.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(export_to = "ipc.ts"))]
pub struct UiError {
    pub code: String,
    pub retryable: bool,
}

impl UiError {
    fn new(code: impl Into<String>, retryable: bool) -> Self {
        Self {
            code: code.into(),
            retryable,
        }
    }

    pub(crate) fn from_boundary(code: UiBoundaryErrorCode) -> Self {
        Self::new(code.as_str(), code.retryable())
    }

    pub(crate) fn from_error_code(code: Option<ErrorCode>) -> Self {
        match code {
            Some(code) => Self::new(code.as_str(), code.retryable()),
            None => Self::from_boundary(UiBoundaryErrorCode::Unknown),
        }
    }

    fn from_unknown_code(code: &str) -> Self {
        let safe = !code.is_empty()
            && code.len() <= 64
            && code
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_');
        if safe {
            Self::new(code, true)
        } else {
            Self::from_boundary(UiBoundaryErrorCode::Unknown)
        }
    }
}

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

    /// The daemon accepted the connection and then did not answer inside the
    /// client budget.
    ///
    /// Deliberately not [`Self::Unreachable`]. A process that took the
    /// connection is alive, and reporting it as "not running" would put a Start
    /// button in front of a daemon that is already holding the endpoint — the
    /// one action that cannot help. It also has to stay distinct so
    /// `Supervisor::state` reads it as `Unhealthy` rather than `Stopped`.
    #[error("The background service isn't responding.")]
    Timeout,

    /// The daemon answered, and said no.
    #[error("{message}")]
    Daemon { message: String, ui: UiError },

    /// The backend is still coming up.
    #[error("CopyPaste is still starting up. Try again in a moment.")]
    NotReady,

    /// App and daemon disagree about the protocol.
    #[error("The app and the daemon are different versions. Upgrade both and restart the daemon.")]
    ProtocolMismatch,

    /// No such item. A missing *peer* is not this: it arrives as
    /// `ErrorCode::PeerNotFound` and keeps the daemon's own sentence, because
    /// the fixed one here is about the clipboard.
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

    /// The item exists, but the requested clipboard representation does not.
    #[error("{0}")]
    UnsupportedContent(&'static str),
}

impl BackendError {
    /// Build an error from text that came from outside this crate.
    ///
    /// The scrub is here rather than at each call site so that forgetting it is
    /// not possible: there is no other public constructor that takes a
    /// `String`.
    pub fn from_daemon(message: &str) -> Self {
        Self::Daemon {
            message: scrub_paths(message),
            ui: UiError::from_error_code(None),
        }
    }

    fn from_unknown_code(code: &str, message: &str) -> Self {
        Self::Daemon {
            message: scrub_paths(message),
            ui: UiError::from_unknown_code(code),
        }
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
    ///
    /// **Substituting a sentence for the daemon's is the exception, not the
    /// rule, and each one is a decision.** Three codes name a condition this
    /// crate can word better than the daemon can; every other code passes the
    /// daemon's own sentence through, because that is where the remedy is
    /// written — "unpair one first", "get a fresh code", "sign in again". The
    /// arm is total so that a new code cannot inherit a substitution nobody
    /// chose: `NotFound` used to swallow "no such paired device" and answer
    /// "That item is no longer there.", which told a user unpairing a vanished
    /// device that their clipboard item had gone (post-merge review, finding 4).
    pub fn from_code(
        code: Option<ErrorCode>,
        raw_code: Option<&str>,
        message: Option<&str>,
    ) -> Self {
        let passthrough = || {
            let message = message.unwrap_or("The daemon reported a failure but gave no reason.");
            match (code, raw_code) {
                (Some(code), _) => Self::Daemon {
                    message: scrub_paths(message),
                    ui: UiError::from_error_code(Some(code)),
                },
                (None, Some(raw)) => Self::from_unknown_code(raw, message),
                (None, None) => Self::from_daemon(message),
            }
        };
        let Some(code) = code else {
            return passthrough();
        };
        match code {
            ErrorCode::NotReady => Self::NotReady,
            ErrorCode::ProtocolMismatch => Self::ProtocolMismatch,
            // Now only ever an item: a missing device is `PeerNotFound`.
            ErrorCode::NotFound => Self::NotFound("That item is no longer there."),
            ErrorCode::UnsupportedContent => {
                Self::UnsupportedContent("That item cannot be copied in the requested format.")
            }
            ErrorCode::PairingCode
            | ErrorCode::PairingAddress
            | ErrorCode::RateLimited
            | ErrorCode::PeerUnreachable
            | ErrorCode::PairingLimit
            | ErrorCode::PeerFailed
            | ErrorCode::PeerVersion
            | ErrorCode::PeerNotFound
            | ErrorCode::InvalidRequest
            | ErrorCode::AuthFailed
            | ErrorCode::KeyLocked
            | ErrorCode::KeyUnusable
            | ErrorCode::Internal => passthrough(),
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

    #[must_use]
    pub fn ui_error(&self) -> UiError {
        match self {
            Self::Unreachable => UiError::from_boundary(UiBoundaryErrorCode::Offline),
            Self::Timeout => UiError::from_boundary(UiBoundaryErrorCode::Timeout),
            Self::Daemon { ui, .. } => ui.clone(),
            Self::NotReady => UiError::from_error_code(Some(ErrorCode::NotReady)),
            Self::ProtocolMismatch => UiError::from_error_code(Some(ErrorCode::ProtocolMismatch)),
            Self::NotFound(_) => UiError::from_error_code(Some(ErrorCode::NotFound)),
            Self::Invalid(_) => UiError::from_error_code(Some(ErrorCode::InvalidRequest)),
            Self::Internal(_) => UiError::from_error_code(Some(ErrorCode::Internal)),
            Self::Unsupported(_) => UiError::from_boundary(UiBoundaryErrorCode::Unavailable),
            Self::UnsupportedContent(_) => {
                UiError::from_error_code(Some(ErrorCode::UnsupportedContent))
            }
        }
    }
}

/// No internal message is serialised across the Tauri boundary.
impl serde::Serialize for BackendError {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        self.ui_error().serialize(serializer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_codes_serialize_with_their_rust_owned_retry_policy() {
        for code in UiBoundaryErrorCode::ALL {
            assert_eq!(serde_json::to_value(code).unwrap(), code.as_str());
            let error = UiError::from_boundary(*code);
            assert_eq!(error.code, code.as_str());
            assert_eq!(error.retryable, code.retryable());
        }
    }

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
            BackendError::Timeout,
            BackendError::NotReady,
            BackendError::ProtocolMismatch,
            BackendError::NotFound("That item is no longer there."),
            BackendError::Invalid("There is nothing to add."),
            BackendError::Unsupported("Pairing is not available in this build."),
            BackendError::UnsupportedContent("That item cannot be copied in the requested format."),
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
            BackendError::from_code(Some(ErrorCode::NotReady), None, Some("anything at all")),
            BackendError::NotReady
        ));
        assert!(matches!(
            BackendError::from_code(Some(ErrorCode::ProtocolMismatch), None, None),
            BackendError::ProtocolMismatch
        ));
        assert!(matches!(
            BackendError::from_code(Some(ErrorCode::NotFound), None, Some("gone")),
            BackendError::NotFound(_)
        ));
        assert!(matches!(
            BackendError::from_code(Some(ErrorCode::UnsupportedContent), None, Some("ignored")),
            BackendError::UnsupportedContent(_)
        ));
        // An untagged failure still becomes an error rather than a success.
        assert!(matches!(
            BackendError::from_code(None, None, Some("unknown method")),
            BackendError::Daemon { .. }
        ));
    }

    /// Every code that carries a remedy has to arrive with it intact. This is
    /// exactly what `NotFound` failed: it answered "no such paired device" with
    /// "That item is no longer there.", so unpairing a device that had gone
    /// reported a missing clipboard item (post-merge review, finding 4).
    #[test]
    fn a_pairing_failure_keeps_the_sentence_the_daemon_wrote() {
        for (code, sentence) in [
            (ErrorCode::PairingCode, "that pairing code is not valid"),
            (
                ErrorCode::PairingAddress,
                "that address could not be resolved; expected host:port",
            ),
            (
                ErrorCode::PeerUnreachable,
                "the other device stopped responding",
            ),
            (
                ErrorCode::PairingLimit,
                "this device is already paired with as many devices as it can hold; \
                 unpair one first",
            ),
            (
                ErrorCode::PeerFailed,
                "the sync session with the other device failed",
            ),
            (ErrorCode::PeerNotFound, "no such paired device"),
        ] {
            let shown = BackendError::from_code(Some(code), None, Some(sentence)).to_string();
            assert_eq!(shown, sentence, "{code:?}");
            assert!(!shown.contains("clipboard"), "{code:?}: {shown}");
        }
    }

    /// The other half of the same rule: `NotFound` now only ever means an item,
    /// so its fixed sentence is finally the true one.
    #[test]
    fn a_missing_item_and_a_missing_device_do_not_render_alike() {
        let item = BackendError::from_code(Some(ErrorCode::NotFound), None, Some("item not found"));
        let device = BackendError::from_code(
            Some(ErrorCode::PeerNotFound),
            None,
            Some("no such paired device"),
        );
        assert_ne!(item.to_string(), device.to_string());
        assert!(item.to_string().contains("item"), "{item}");
        assert!(device.to_string().contains("device"), "{device}");
    }

    /// A daemon that took the connection and went quiet is not a daemon that
    /// is missing, and the two must not render or serialise alike: one asks the
    /// user to start the service, the other cannot be fixed that way.
    #[test]
    fn a_silent_daemon_does_not_read_as_a_missing_one() {
        assert_ne!(
            BackendError::Timeout.to_string(),
            BackendError::Unreachable.to_string()
        );
        assert_eq!(
            serde_json::to_string(&BackendError::Timeout).unwrap(),
            r#"{"code":"timeout","retryable":true}"#
        );
        let shown = BackendError::Timeout.to_string();
        assert!(!shown.contains('/'), "{shown}");
        assert!(!shown.contains("daemon"), "{shown}");
    }

    #[test]
    fn serialises_as_the_minimal_structured_error() {
        let json = serde_json::to_string(&BackendError::NotReady).unwrap();
        assert_eq!(json, r#"{"code":"not_ready","retryable":true}"#);
    }

    #[test]
    fn serialised_internal_error_discloses_no_message_path_or_username() {
        let json = serde_json::to_string(&BackendError::internal(
            "open /Users/alice/Library/Application Support/CopyPaste/history.db failed",
        ))
        .unwrap();
        assert_eq!(json, r#"{"code":"internal","retryable":true}"#);
        assert!(!json.contains("alice"), "{json}");
        assert!(!json.contains("history.db"), "{json}");
    }

    #[test]
    fn safe_future_code_survives_without_its_internal_message() {
        let error = BackendError::from_code(
            None,
            Some("future_state_2"),
            Some("failed under /home/bob/private"),
        );
        let json = serde_json::to_string(&error).unwrap();
        assert_eq!(json, r#"{"code":"future_state_2","retryable":true}"#);
        assert!(!json.contains("bob"), "{json}");
    }

    #[test]
    fn path_like_unknown_code_is_not_forwarded() {
        let error = BackendError::from_code(
            None,
            Some("ENOENT:/Users/alice/.copypaste.sock"),
            Some("untrusted"),
        );
        assert_eq!(
            serde_json::to_string(&error).unwrap(),
            r#"{"code":"unknown","retryable":true}"#
        );
    }
}
