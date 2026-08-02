//! Sensitive-content (secret) detection.
//!
//! Binding specification: `docs/rewrite/port-manifest/07-secret-detection.md`.
//! `§n` references below point into it; every `CopyPaste-*` / `P2 *` /
//! `Audit MED #*` note records a real production miss or a real deletion of
//! user data (§8.1.5 — carry the *reasons* forward even when the regex is
//! replaced).
//!
//! # Detection and whole-item classification stay separate (manifest I2)
//!
//! [`Detector::scan_all`] returns every validated match, including inert
//! low-confidence PII, with byte offsets into the NFKC-normalised string.
//! [`Detector::scan`] preserves the deterministic highest-confidence label.
//!
//! [`Detector::may_auto_wipe`] and its compatibility predicate
//! [`Detector::is_sensitive`] share the only content-based whole-item gate.
//! Everything below the 0.70 floor remains detectable and redactable but
//! inert. Password-manager provenance is an independent capture-time floor.

mod engine;
mod finding;
mod normalise;
mod purge;
mod redact;
mod rules;
mod rules_generated;
mod spec;
mod validators;
mod wipe;

pub use engine::{Detector, DetectorError};
pub use finding::{Finding, Severity, SpannedFinding};
pub use purge::{purge_indexed_secrets, purge_indexed_secrets_in_transaction, PurgeReport};
pub use redact::redact_spans;
pub use wipe::{sweep_sensitive, DEFAULT_SENSITIVE_TTL, SENSITIVE_TTL_DISABLED};

/// Credential managers are an independent sensitivity floor: their copied
/// values must never enter full-text search even when their contents do not
/// match a detector rule.
pub fn is_password_manager_app(bundle_id: &str) -> bool {
    let bundle_id = bundle_id.to_ascii_lowercase();
    matches!(
        bundle_id.as_str(),
        "com.1password.1password"
            | "com.agilebits.onepassword7"
            | "com.bitwarden.desktop"
            | "org.keepassxc.keepassxc"
            | "com.dashlane.dashlane"
            | "com.lastpass.lastpass"
            | "com.apple.passwords"
    ) || bundle_id.contains("1password")
        || bundle_id.contains("bitwarden")
        || bundle_id.contains("keepass")
        || bundle_id.contains("dashlane")
        || bundle_id.contains("lastpass")
        || bundle_id.contains("protonpass")
        || bundle_id.contains("proton.pass")
        || bundle_id.contains("strongbox")
        || bundle_id.contains("secretive")
        || bundle_id.contains("keepassium")
}
