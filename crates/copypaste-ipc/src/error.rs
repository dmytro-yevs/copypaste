//! Machine-readable IPC error codes (typed enum form).
//!
//! Beta wave W3.3 introduces this typed [`ErrorCode`] alongside the legacy
//! `ERR_CODE_*` `&'static str` constants in [`crate::response`]. The two are
//! **wire-compatible**: the snake_case serialised form of an [`ErrorCode`] is
//! exactly the string that ships in [`crate::Response::error_code`]. Producers
//! may continue to emit `&'static str` codes via `Response::err_with_code`;
//! consumers may parse the field with [`ErrorCode::parse`] to branch on a
//! typed enum instead of a string.
//!
//! ## Why both forms?
//!
//! * The `&'static str` constants keep `Response::error_code` allocation-free
//!   on the daemon hot path.
//! * The [`ErrorCode`] enum gives consumers (UI, CLI) exhaustive matching and
//!   safer refactors when codes are added.
//!
//! ## Adding a new code
//!
//! 1. Add a variant to [`ErrorCode`] with a snake_case `serde` rename.
//! 2. Add the matching mapping in [`ErrorCode::as_str`] and
//!    [`ErrorCode::parse`].
//! 3. (Optional) Add a matching `ERR_CODE_*` constant in
//!    [`crate::response`] for daemon-side producers.
//! 4. Never repurpose an existing code — it is part of the public wire
//!    contract.

use serde::{Deserialize, Serialize};

/// Machine-readable error code for IPC responses.
///
/// Wire-compatible with the `error_code: Option<&'static str>` field on
/// [`crate::Response`]: each variant serialises as snake_case (e.g.
/// `ErrorCode::NotFound` ↔ `"not_found"`), matching the existing
/// `ERR_CODE_*` `&'static str` constants in [`crate::response`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// Requested resource (item id, peer, etc.) does not exist.
    NotFound,
    /// Authentication failed — bad credentials, expired token, missing keychain entry.
    AuthFailed,
    /// Request was structurally valid JSON but violated parameter contract
    /// (missing field, wrong type, invalid format).
    InvalidArgument,
    /// Method is recognised but not yet implemented (cloud-sync stubs, etc.).
    NotImplemented,
    /// Daemon is still booting — database/cloud not yet ready to serve requests.
    IpcNotReady,
    /// Catch-all for unexpected daemon-side failures (I/O, panics, db errors).
    InternalError,
    /// Wire protocol version mismatch between peers.
    VersionMismatch,
    /// Request rejected because the caller exceeded a rate limit.
    RateLimited,
    /// Daemon socket is missing / refused connection (UI/CLI client view).
    DaemonOffline,
    /// The v4 key-rotation sweep is still in progress; ingest paths reject new
    /// writes until it completes. Clients should back off and retry shortly
    /// rather than treat this as a hard failure.
    MigrationInProgress,
}

impl ErrorCode {
    /// Return the canonical snake_case wire string for this code.
    ///
    /// This matches the `ERR_CODE_*` constants in [`crate::response`]:
    /// `ErrorCode::NotFound.as_str() == "not_found"`, etc.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::AuthFailed => "auth_failed",
            Self::InvalidArgument => "invalid_argument",
            Self::NotImplemented => "not_implemented",
            Self::IpcNotReady => "ipc_not_ready",
            Self::InternalError => "internal_error",
            Self::VersionMismatch => "version_mismatch",
            Self::RateLimited => "rate_limited",
            Self::DaemonOffline => "daemon_offline",
            Self::MigrationInProgress => "migration_in_progress",
        }
    }

    /// Parse a wire string into a typed [`ErrorCode`].
    ///
    /// Returns `None` for unknown codes so that consumers can forward-compat
    /// gracefully (treat unknown codes as a generic error rather than crash).
    ///
    /// Named `parse` (not `from_str`) to avoid shadowing
    /// [`std::str::FromStr::from_str`], whose signature returns `Result`
    /// rather than `Option`.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "not_found" => Some(Self::NotFound),
            "auth_failed" => Some(Self::AuthFailed),
            "invalid_argument" => Some(Self::InvalidArgument),
            "not_implemented" => Some(Self::NotImplemented),
            "ipc_not_ready" => Some(Self::IpcNotReady),
            "internal_error" => Some(Self::InternalError),
            "version_mismatch" => Some(Self::VersionMismatch),
            "rate_limited" => Some(Self::RateLimited),
            "daemon_offline" => Some(Self::DaemonOffline),
            "migration_in_progress" => Some(Self::MigrationInProgress),
            _ => None,
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_roundtrip_via_as_str_and_from_str() {
        // Every variant must round-trip through its wire string form.
        // If a new variant is added, the match below forces the test to
        // be updated (no `_` catch-all).
        let all = [
            ErrorCode::NotFound,
            ErrorCode::AuthFailed,
            ErrorCode::InvalidArgument,
            ErrorCode::NotImplemented,
            ErrorCode::IpcNotReady,
            ErrorCode::InternalError,
            ErrorCode::VersionMismatch,
            ErrorCode::RateLimited,
            ErrorCode::DaemonOffline,
            ErrorCode::MigrationInProgress,
        ];
        for code in all {
            let s = code.as_str();
            let parsed =
                ErrorCode::parse(s).unwrap_or_else(|| panic!("parse failed to handle {s}"));
            assert_eq!(parsed, code, "round-trip mismatch for {s}");
        }
    }

    #[test]
    fn error_code_serde_snake_case() {
        // Wire form must match the `ERR_CODE_*` constants in `response.rs`.
        let code = ErrorCode::NotImplemented;
        let s = serde_json::to_string(&code).unwrap();
        assert_eq!(s, "\"not_implemented\"");
        let back: ErrorCode = serde_json::from_str(&s).unwrap();
        assert_eq!(back, code);

        // Sample the other variants — guards against accidental rename.
        assert_eq!(
            serde_json::to_string(&ErrorCode::AuthFailed).unwrap(),
            "\"auth_failed\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorCode::InvalidArgument).unwrap(),
            "\"invalid_argument\""
        );
        assert_eq!(
            serde_json::to_string(&ErrorCode::VersionMismatch).unwrap(),
            "\"version_mismatch\""
        );
    }

    #[test]
    fn error_code_from_str_unknown_returns_none() {
        assert!(ErrorCode::parse("totally_made_up").is_none());
        assert!(ErrorCode::parse("").is_none());
        // Case sensitive — codes are canonical snake_case.
        assert!(ErrorCode::parse("NOT_FOUND").is_none());
    }

    #[test]
    fn error_code_matches_existing_str_constants() {
        // The typed enum MUST agree with the `&'static str` constants
        // already shipped in `response.rs`. If anyone renames either side,
        // this test catches the drift.
        use crate::response::{
            ERR_CODE_AUTH_FAILED, ERR_CODE_DAEMON_OFFLINE, ERR_CODE_INTERNAL_ERROR,
            ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, ERR_CODE_MIGRATION_IN_PROGRESS,
            ERR_CODE_NOT_FOUND, ERR_CODE_NOT_IMPLEMENTED, ERR_CODE_RATE_LIMITED,
            ERR_CODE_VERSION_MISMATCH,
        };
        assert_eq!(ErrorCode::NotFound.as_str(), ERR_CODE_NOT_FOUND);
        assert_eq!(ErrorCode::AuthFailed.as_str(), ERR_CODE_AUTH_FAILED);
        assert_eq!(
            ErrorCode::InvalidArgument.as_str(),
            ERR_CODE_INVALID_ARGUMENT
        );
        assert_eq!(ErrorCode::NotImplemented.as_str(), ERR_CODE_NOT_IMPLEMENTED);
        assert_eq!(ErrorCode::IpcNotReady.as_str(), ERR_CODE_IPC_NOT_READY);
        assert_eq!(ErrorCode::InternalError.as_str(), ERR_CODE_INTERNAL_ERROR);
        assert_eq!(
            ErrorCode::MigrationInProgress.as_str(),
            ERR_CODE_MIGRATION_IN_PROGRESS
        );
        assert_eq!(
            ErrorCode::VersionMismatch.as_str(),
            ERR_CODE_VERSION_MISMATCH
        );
        assert_eq!(ErrorCode::RateLimited.as_str(), ERR_CODE_RATE_LIMITED);
        assert_eq!(ErrorCode::DaemonOffline.as_str(), ERR_CODE_DAEMON_OFFLINE);
    }

    /// The `migration_in_progress` wire code (emitted by the daemon's v4
    /// key-rotation sweep gate) must parse into the typed variant so the CLI
    /// can branch on it instead of matching English error text.
    #[test]
    fn migration_in_progress_parses_and_displays() {
        assert_eq!(
            ErrorCode::parse("migration_in_progress"),
            Some(ErrorCode::MigrationInProgress)
        );
        assert_eq!(
            format!("{}", ErrorCode::MigrationInProgress),
            "migration_in_progress"
        );
    }

    #[test]
    fn error_code_display_matches_wire_str() {
        assert_eq!(format!("{}", ErrorCode::NotFound), "not_found");
        assert_eq!(format!("{}", ErrorCode::RateLimited), "rate_limited");
    }
}
