use thiserror::Error;

#[cfg(target_os = "macos")]
use security_framework::base::Error as SfError;

#[cfg(target_os = "macos")]
pub mod acl;
mod device_key;
pub mod file_store;
mod fingerprint;
mod secure_write;
#[cfg(target_os = "macos")]
pub mod signing;
mod supabase_password;

#[cfg(target_os = "macos")]
pub use device_key::delete_stored;
pub use device_key::load_or_create;
pub use fingerprint::own_fingerprint;
#[cfg(all(target_os = "macos", feature = "cloud-sync"))]
pub(crate) use secure_write::set_generic_password_locked_down;
pub use supabase_password::{
    delete_supabase_password_from_keychain, read_supabase_password_from_keychain,
    store_supabase_password_to_keychain,
};

pub(crate) const SERVICE: &str = "com.copypaste.daemon";
pub(crate) const ACCOUNT: &str = "device-secret-key";
/// Keychain account key for the cloud sync passphrase-derived key bytes.
/// Stored under the same service as the device key but a distinct account
/// so they are never confused. This is the single per-account-salt sync key
/// shared by the cloud (Supabase) and relay paths.
pub(crate) const CLOUD_SYNC_ACCOUNT: &str = "cloud-sync-key";
/// Keychain account key for the Supabase GoTrue account password.
/// Stored under `SERVICE` so all CopyPaste secrets live in one service.
/// Migration: if absent from Keychain, callers fall back to config.json.
pub(crate) const SUPABASE_PASSWORD_ACCOUNT: &str = "supabase-password";

/// Dev/test escape hatch: when `COPYPASTE_EPHEMERAL_KEY` is set in the
/// environment, every keychain entry point in this module short-circuits
/// BEFORE any Security-framework call so the macOS login-keychain password
/// prompt is never triggered.
///
/// Why centralize here: ad-hoc-signed dev builds change signature on every
/// rebuild, invalidating the persisted item's ACL and forcing an interactive
/// keychain prompt. `cargo test --workspace` and `make dev-daemon` set this
/// env so they run non-interactively. Production (env unset) is unaffected —
/// every caller falls through to the real Security-framework path unchanged.
/// CopyPaste-qvtg.5: in production the env var is read **once** and cached for
/// the process lifetime, so an attacker who can mutate the running daemon's
/// environment mid-session (e.g. via a debugger) cannot flip it into
/// ephemeral-key bypass *after* the real Keychain-backed key is already in use.
/// The legitimate opt-in is always set before launch, so caching the first
/// observed value loses no real functionality.
#[cfg(not(test))]
pub(crate) fn keychain_bypassed() -> bool {
    static BYPASS: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *BYPASS.get_or_init(|| std::env::var_os("COPYPASTE_EPHEMERAL_KEY").is_some())
}

/// Test build: read the env live each call. The suite toggles
/// `COPYPASTE_EPHEMERAL_KEY` under `TEST_ENV_LOCK` to exercise both the bypass
/// and the real path within one process, which a `OnceLock` cache (pinning the
/// first-observed value for the whole run) would break.
#[cfg(test)]
pub(crate) fn keychain_bypassed() -> bool {
    std::env::var_os("COPYPASTE_EPHEMERAL_KEY").is_some()
}

#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("Key is wrong length: expected 32 bytes, got {0}")]
    InvalidLength(usize),
    #[cfg(target_os = "macos")]
    #[error("Keychain error: {0}")]
    Keychain(#[from] SfError),
    #[cfg(not(target_os = "macos"))]
    #[error("Keychain not supported on this platform")]
    Unsupported,
    #[cfg(not(target_os = "macos"))]
    #[error("Keychain file I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Core key error: {0}")]
    Key(#[from] copypaste_core::KeyError),
    // ── v0.3 ACL surface (macOS only) ──────────────────────────────────────
    #[cfg(target_os = "macos")]
    #[error("Keychain ACL trust list is empty — refusing to create unrestricted entry")]
    AclEmpty,
    #[cfg(target_os = "macos")]
    #[error("Cannot encode binary path for Keychain ACL (interior NUL byte)")]
    AclPathEncoding,
    #[cfg(target_os = "macos")]
    #[error("Security.framework call {op} failed with OSStatus {code}")]
    OsStatus { op: &'static str, code: i32 },
    #[cfg(target_os = "macos")]
    #[error("Filesystem I/O: {0}")]
    Io(#[from] std::io::Error),
    /// The Keychain item exists (or its existence cannot be determined) but
    /// could not be read because the Keychain is locked, access was denied, an
    /// interactive prompt is disallowed, or the read timed out. Distinct from a
    /// genuine `errSecItemNotFound` so the caller can DEGRADE (leave the
    /// encrypted DB untouched) instead of silently minting an ephemeral key that
    /// would mismatch the existing DB key. Carries the originating OSStatus for
    /// diagnostics.
    #[cfg(target_os = "macos")]
    #[error("Keychain locked or access denied (OSStatus {0}); cannot read device key")]
    Locked(i32),
}
