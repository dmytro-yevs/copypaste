//! CLI error type, exit-code mapping, and the user-facing message rules.
//!
//! Two rules from AGENTS.md are enforced here rather than at each call site:
//!
//! * **Errors shown to users must never contain paths** (rule 4). The daemon
//!   socket lives under the user's home directory, so printing it discloses the
//!   local username. Nothing in this module ever formats [`socket_path`], and
//!   [`scrub_paths`] is a belt-and-braces pass over daemon-supplied text so a
//!   future daemon regression cannot leak one through the CLI.
//! * **Clients branch on `error_code`, never on the `error` string**
//!   (port manifest 04, I9). [`CliError::exit_code`] matches on
//!   [`ErrorCode`] only.
//!
//! [`socket_path`]: copypaste_ipc::socket_path

use copypaste_ipc::{ErrorCode, PROTOCOL_VERSION};
use std::fmt;

/// The command did what was asked.
pub const EXIT_OK: i32 = 0;
/// The daemon could not be reached at all.
pub const EXIT_UNREACHABLE: i32 = 1;
/// The daemon answered, but the thing asked for does not exist.
pub const EXIT_NOT_FOUND: i32 = 2;
/// Everything else.
pub const EXIT_OTHER: i32 = 3;
/// The daemon took the request and did not answer in time.
pub const EXIT_TIMEOUT: i32 = 4;

#[derive(Debug)]
pub enum CliError {
    /// The socket is absent, refused the connection, or the daemon hung up
    /// before answering. Exit [`EXIT_UNREACHABLE`].
    DaemonUnreachable,
    /// The connection was accepted and no reply arrived inside the budget.
    /// Exit [`EXIT_TIMEOUT`].
    ///
    /// Split from [`Self::DaemonUnreachable`] because the two need opposite
    /// things from the user. "Start it with `copypaste-daemon`" is the wrong
    /// advice for a daemon that is already running and busy, and a script that
    /// retried on exit 1 would spend its retries starting a second daemon that
    /// cannot bind. A running daemon must never be reported as unreachable.
    DaemonTimeout,
    /// The daemon answered `ok: false`. `code` is `None` for an untagged
    /// failure, which the protocol permits and clients must tolerate.
    Daemon {
        code: Option<ErrorCode>,
        /// A future daemon code this CLI does not understand, kept verbatim.
        raw_code: Option<String>,
        message: String,
    },
    /// A client-side problem: a malformed frame, an over-long line, a response
    /// whose shape does not match the request, or bad user input.
    Local(String),
}

impl CliError {
    pub fn local(message: impl Into<String>) -> Self {
        CliError::Local(message.into())
    }

    /// Map to a process exit status.
    ///
    /// Only `not_found` gets its own code; every other daemon-reported failure
    /// is "other error", because a script that wants to distinguish them should
    /// be reading `--json` and branching on `error_code`.
    pub fn exit_code(&self) -> i32 {
        match self {
            CliError::DaemonUnreachable => EXIT_UNREACHABLE,
            CliError::DaemonTimeout => EXIT_TIMEOUT,
            // A missing device is the same answer to a script as a missing
            // item — "the thing you named is not here" — so splitting the code
            // on the wire must not split the exit status.
            CliError::Daemon {
                code: Some(ErrorCode::NotFound | ErrorCode::PeerNotFound),
                ..
            } => EXIT_NOT_FOUND,
            CliError::Daemon { .. } => EXIT_OTHER,
            CliError::Local(_) => EXIT_OTHER,
        }
    }

    /// The single line printed to stderr. Never contains a filesystem path.
    pub fn user_message(&self) -> String {
        match self {
            CliError::DaemonUnreachable => {
                // Deliberately no path: naming the socket would disclose the
                // local username (AGENTS.md rule 4).
                "cannot reach the CopyPaste daemon. Start it with `copypaste-daemon`, \
                 then run this command again."
                    .to_string()
            }
            CliError::DaemonTimeout => {
                "the CopyPaste daemon accepted the request but did not answer in time. \
                 It is running; try again, or check whether it is busy with a large \
                 import, backup or sync."
                    .to_string()
            }
            CliError::Daemon {
                code: Some(ErrorCode::ProtocolMismatch),
                ..
            } => {
                format!(
                    "daemon and CLI versions differ: this CLI speaks IPC protocol v{PROTOCOL_VERSION}, \
                     which the running daemon does not accept. Upgrade both to the same release \
                     and restart the daemon."
                )
            }
            CliError::Daemon {
                code: Some(ErrorCode::NotReady),
                ..
            } => "the daemon is still starting up. Try again in a few seconds.".to_string(),
            CliError::Daemon {
                code: Some(ErrorCode::NotFound),
                message,
                ..
            } => scrub_paths(message),
            CliError::Daemon {
                code: None,
                raw_code: Some(raw_code),
                message,
            } => format!(
                "error [{}]: {}",
                scrub_paths(raw_code),
                scrub_paths(message)
            ),
            CliError::Daemon { message, .. } => scrub_paths(message),
            CliError::Local(message) => scrub_paths(message),
        }
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.user_message())
    }
}

impl std::error::Error for CliError {}

/// Replace anything that looks like a filesystem path with `<path>`.
///
/// The daemon promises not to put paths in `error` strings, but this CLI is the
/// thing the user actually reads, so it does not rely on that promise. Matching
/// is token-based and deliberately blunt: over-redacting a message is harmless,
/// leaking `/Users/<name>` is not.
/// Re-exported so existing call sites keep working. The implementation
/// lives in `copypaste_ipc::redact` because the Tauri bridge needs the same
/// guarantee and cannot import it from this crate (bin-only, no lib target).
pub use copypaste_ipc::redact::scrub_paths;

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Corpus {
        display_leak_cases: Vec<DisplayCase>,
        safe_display_cases: Vec<SafeCase>,
        #[serde(default)]
        redaction_cases: Vec<RedactionCase>,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct DisplayCase {
        id: String,
        surface: String,
        expected_surface: String,
        forbidden_fragments: Vec<String>,
    }

    #[derive(Deserialize)]
    struct SafeCase {
        id: String,
        surface: String,
    }

    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RedactionCase {
        id: String,
        surface: String,
        expected_surface: String,
        forbidden_fragments: Vec<String>,
    }

    fn corpus() -> Corpus {
        serde_json::from_str(include_str!(
            "../../../test-support/security/path-security-vectors.json"
        ))
        .expect("path security corpus is valid")
    }

    #[test]
    fn unreachable_daemon_exits_one() {
        assert_eq!(CliError::DaemonUnreachable.exit_code(), EXIT_UNREACHABLE);
    }

    #[test]
    fn a_missing_item_and_a_missing_device_both_exit_two() {
        for (code, message) in [
            (ErrorCode::NotFound, "no such item"),
            (ErrorCode::PeerNotFound, "no such paired device"),
        ] {
            let err = CliError::Daemon {
                code: Some(code),
                raw_code: None,
                message: message.into(),
            };
            assert_eq!(err.exit_code(), EXIT_NOT_FOUND, "{code:?}");
            assert_eq!(err.user_message(), message);
        }
    }

    /// Every pairing failure reaches the user with the node's own sentence,
    /// remedy included — the CLI substitutes nothing.
    #[test]
    fn pairing_failures_keep_their_remedy() {
        let err = CliError::Daemon {
            code: Some(ErrorCode::PairingLimit),
            raw_code: None,
            message: "this device is already paired with as many devices as it can hold; \
                      unpair one first"
                .into(),
        };
        assert!(err.user_message().contains("unpair one first"));
        assert_eq!(err.exit_code(), EXIT_OTHER);
    }

    #[test]
    fn every_other_daemon_code_exits_three() {
        for code in [
            ErrorCode::InvalidRequest,
            ErrorCode::ProtocolMismatch,
            ErrorCode::NotReady,
            ErrorCode::Internal,
        ] {
            let err = CliError::Daemon {
                code: Some(code),
                raw_code: None,
                message: "boom".into(),
            };
            assert_eq!(err.exit_code(), EXIT_OTHER, "{code:?}");
        }
    }

    #[test]
    fn untagged_daemon_failure_exits_three() {
        let err = CliError::Daemon {
            code: None,
            raw_code: None,
            message: "unknown method".into(),
        };
        assert_eq!(err.exit_code(), EXIT_OTHER);
    }

    #[test]
    fn local_failure_exits_three() {
        assert_eq!(CliError::local("bad frame").exit_code(), EXIT_OTHER);
    }

    #[test]
    fn unreachable_message_is_actionable_and_pathless() {
        let msg = CliError::DaemonUnreachable.user_message();
        assert!(msg.contains("copypaste-daemon"), "{msg}");
        assert!(!msg.contains('/'), "message must not contain a path: {msg}");
    }

    /// The distinction the variant exists for. A running daemon reported as
    /// unreachable sends a user to start a second one, which cannot bind.
    #[test]
    fn a_slow_daemon_is_not_reported_as_a_missing_one() {
        let slow = CliError::DaemonTimeout;
        assert_eq!(slow.exit_code(), EXIT_TIMEOUT);
        assert_ne!(slow.exit_code(), CliError::DaemonUnreachable.exit_code());

        let msg = slow.user_message();
        assert!(!msg.contains("cannot reach"), "{msg}");
        assert!(
            !msg.contains("Start it with"),
            "advice to start a daemon that is already running: {msg}"
        );
        assert!(msg.contains("running"), "{msg}");
        assert!(!msg.contains('/'), "{msg}");
    }

    #[test]
    fn protocol_mismatch_message_says_versions_differ() {
        let err = CliError::Daemon {
            code: Some(ErrorCode::ProtocolMismatch),
            raw_code: None,
            message: "unsupported protocol version 9".into(),
        };
        let msg = err.user_message();
        assert!(msg.contains("versions differ"), "{msg}");
        assert!(msg.contains("Upgrade"), "{msg}");
    }

    #[test]
    fn daemon_supplied_path_is_scrubbed_from_the_user_message() {
        let err = CliError::Daemon {
            code: Some(ErrorCode::Internal),
            raw_code: None,
            message: "could not open /Users/dmitriy/Library/Application Support/com.copypaste.CopyPaste/x.db"
                .into(),
        };
        let msg = err.user_message();
        assert!(!msg.contains("dmitriy"), "{msg}");
        assert!(msg.contains("<path>"), "{msg}");
    }

    #[test]
    fn unknown_daemon_code_is_preserved_and_exits_three() {
        let err = CliError::Daemon {
            code: None,
            raw_code: Some("future_daemon_state".into()),
            message: "wait for the next release".into(),
        };
        assert_eq!(err.exit_code(), EXIT_OTHER);
        assert_eq!(
            err.user_message(),
            "error [future_daemon_state]: wait for the next release"
        );
    }

    #[test]
    fn scrub_leaves_ordinary_text_alone() {
        let msg = "item not found: 3f2a91c4-0000-4000-8000-000000000000";
        assert_eq!(scrub_paths(msg), msg);
    }

    #[test]
    fn scrub_handles_several_path_shapes() {
        for raw in [
            "/tmp/daemon.sock",
            "~/Library/Application",
            "./local.db",
            "../up.db",
            "file:///etc/passwd",
        ] {
            let out = scrub_paths(&format!("failed at {raw} sorry"));
            assert_eq!(out, "failed at <path> sorry", "{raw}");
        }
    }

    #[test]
    fn scrub_preserves_whitespace_layout() {
        assert_eq!(scrub_paths("a  b\nc"), "a  b\nc");
    }

    #[test]
    fn cli_error_path_security_corpus() {
        let corpus = corpus();
        for case in corpus.display_leak_cases {
            let rendered = CliError::local(&case.surface).user_message();
            assert_eq!(
                rendered, case.expected_surface,
                "{} rendered unexpectedly",
                case.id
            );
            for fragment in case.forbidden_fragments {
                assert!(
                    !rendered.contains(&fragment),
                    "{} kept a forbidden fragment",
                    case.id
                );
            }
        }
        for case in corpus.safe_display_cases {
            assert_eq!(
                CliError::local(&case.surface).user_message(),
                case.surface,
                "{} was changed",
                case.id
            );
        }
        for case in corpus.redaction_cases {
            let rendered = CliError::local(&case.surface).user_message();
            assert_eq!(
                rendered, case.expected_surface,
                "{} rendered unexpectedly",
                case.id
            );
            for fragment in case.forbidden_fragments {
                assert!(
                    !rendered.contains(&fragment),
                    "{} kept a forbidden fragment",
                    case.id
                );
            }
        }
    }
}
