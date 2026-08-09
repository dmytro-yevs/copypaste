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
mod origin;
mod presentation;
mod purge;
mod redact;
mod rules;
mod rules_generated;
mod spec;
mod validators;
mod wipe;

pub use engine::{Detector, DetectorError};
pub use finding::{Finding, Severity, SpannedFinding};
pub use origin::is_password_manager_app;
pub use presentation::MAX_SURFACED_SENSITIVE_SPANS;
pub use purge::{purge_indexed_secrets, purge_indexed_secrets_in_transaction, PurgeReport};
pub use redact::redact_spans;
pub use wipe::{sweep_sensitive, DEFAULT_SENSITIVE_TTL, SENSITIVE_TTL_DISABLED};
