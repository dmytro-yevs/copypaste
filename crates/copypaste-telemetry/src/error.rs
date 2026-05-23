//! Anonymized error event types for the opt-in error reporter.
//!
//! Privacy ethos: a [`ReportableError`] MUST NOT contain any user-controlled
//! payload (clipboard contents, file paths, device identifiers, IP addresses,
//! account emails, etc.). Only coarse-grained categorical fields are allowed:
//!
//! - `crate_name`: which CopyPaste crate raised the error
//! - `crate_version`: semver of that crate
//! - `error_class`: a short, developer-chosen taxonomy string
//!   (e.g. `"keychain.read_failed"`, `"ipc.parse_error"`)
//! - `os`: high-level platform tag (`"macos"`, `"windows"`, `"android"`,
//!   `"unknown"`). No version, no build, no hostname.
//!   Linux and iOS are frozen per platform policy and intentionally not
//!   represented as distinct tags; those targets fall back to `unknown`.
//!
//! There is intentionally no free-form `message` field. Adding one is a
//! deliberate decision that requires updating the privacy policy in
//! `docs/privacy/telemetry-policy.md`.

use serde::{Deserialize, Serialize};

/// A coarse-grained, anonymized error event suitable for reporting to an
/// external service when the user has explicitly opted in.
///
/// Construct via [`ReportableError::new`] to ensure all fields are populated.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ReportableError {
    /// Originating crate, e.g. `"copypaste-daemon"`.
    pub crate_name: String,
    /// Semver of that crate, e.g. `"0.2.0-beta.0"`.
    pub crate_version: String,
    /// Developer-defined taxonomy string. Keep stable and machine-parseable.
    pub error_class: String,
    /// Coarse OS tag. See module docs for the allowed value set.
    pub os: OsTag,
}

impl ReportableError {
    /// Construct a [`ReportableError`] with all required fields.
    ///
    /// Strings are owned to keep the struct `'static`-safe across async
    /// boundaries and FFI.
    pub fn new(
        crate_name: impl Into<String>,
        crate_version: impl Into<String>,
        error_class: impl Into<String>,
        os: OsTag,
    ) -> Self {
        Self {
            crate_name: crate_name.into(),
            crate_version: crate_version.into(),
            error_class: error_class.into(),
            os,
        }
    }
}

/// Coarse platform tag. Intentionally low-cardinality.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OsTag {
    /// Apple macOS.
    MacOs,
    /// Microsoft Windows.
    Windows,
    /// Google Android.
    Android,
    /// Could not be determined at compile time.
    Unknown,
}

impl OsTag {
    /// Best-effort tag based on compile-time `cfg` flags. Never panics.
    pub const fn current() -> Self {
        if cfg!(target_os = "macos") {
            Self::MacOs
        } else if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "android") {
            Self::Android
        } else {
            Self::Unknown
        }
    }
}
