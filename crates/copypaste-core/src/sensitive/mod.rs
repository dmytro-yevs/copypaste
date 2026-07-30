//! Sensitive-content (secret) detection.
//!
//! Binding specification: `docs/rewrite/port-manifest/07-secret-detection.md`.
//! `§n` references below point into it; every `CopyPaste-*` / `P2 *` /
//! `Audit MED #*` note records a real production miss or a real deletion of
//! user data (§8.1.5 — carry the *reasons* forward even when the regex is
//! replaced).
//!
//! # The two decisions, kept separate (manifest I2)
//!
//! v1 collapsed "is this a secret?" and "may we delete it?" into one predicate
//! three separate times (bugs `AB-6a`, `PG-23`, `PG-3`). Here:
//!
//! * [`Detector::is_sensitive`] — *any* validated rule matched. Gates the
//!   search index (manifest I4 / ADR-015: `is_sensitive = 1` ⇒ never written to
//!   FTS, never returned by search), at capture and again in
//!   [`purge_indexed_secrets`] for rows the ruleset had not yet learned to
//!   catch. Deliberately inclusive.
//! * [`Severity`] on the [`Finding`] — only `HighConfidence` (confidence ≥ the
//!   0.70 auto-wipe floor) may drive automatic deletion, through
//!   [`Detector::may_auto_wipe`] and its one caller [`sweep_sensitive`].
//!   Everything else is `Flag`: detected, labelled, maskable, **inert** for
//!   deletion. A false positive silently destroys unrecoverable user data
//!   (`CLAUDE.md` rule 4, manifest I1), so a shape that cannot *prove* a secret
//!   stays below the floor.
//!
//! [`Detector::scan`] returns the **highest-confidence** match, not the lowest
//! declaration index: v1 did the latter and labelled text containing both an
//! email and a Terraform token "email" (§7.2). Ties break on longer match, then
//! table order.
//!
//! Deliberately absent: a span/redaction API (§1.1, I9 — no consumer, and an
//! unused one recreates §7.4's three dead entry points); the password-manager
//! bundle-ID list (§5.8 — a capture-time signal about the source app, carried
//! as configuration); the telemetry scrubber's variety gate (§7.5 — v2 must not
//! maintain a second regex engine; the part that belongs to content detection
//! is [`validators::value_is_strong`]).

mod engine;
mod finding;
mod normalise;
mod purge;
mod rules;
mod spec;
mod validators;
mod wipe;

pub use engine::{Detector, DetectorError};
pub use finding::{Finding, Severity};
pub use purge::{purge_indexed_secrets, PurgeReport};
pub use wipe::{sweep_sensitive, DEFAULT_SENSITIVE_TTL, SENSITIVE_TTL_DISABLED};
