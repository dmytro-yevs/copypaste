//! Sensitive-content (secret) detection.
//!
//! Ported from `docs/rewrite/port-manifest/07-secret-detection.md`, which is the
//! binding specification for this module. Section references (`§3.2`, `§5.3`,
//! `§7.1`, …) in the comments below point into that document; every
//! `CopyPaste-*` / `P2 *` / `Audit MED #*` note records a real production miss
//! or a real deletion of user data (§8.1.5 — carry the *reasons* forward even
//! when the regex is replaced).
//!
//! # The two decisions, kept separate (manifest I2)
//!
//! v1 collapsed "is this a secret?" and "may we delete it?" into one predicate
//! three separate times (bugs `AB-6a`, `PG-23`, `PG-3`). Here they are carried
//! by two different things:
//!
//! * [`Detector::is_sensitive`] — *any* validated rule matched. Gates the
//!   search index (manifest I4 / ADR-015: `is_sensitive = 1` ⇒ never written to
//!   FTS, never returned by search). Deliberately inclusive: keeping a phone
//!   number out of a plaintext index costs nothing.
//! * [`Severity`] on the [`Finding`] — only `HighConfidence` (confidence ≥ the
//!   0.70 auto-wipe floor) may drive automatic deletion. Everything else is
//!   `Flag`: detected, labelled, maskable, **inert** for deletion.
//!
//! CLAUDE.md rule 4 and manifest I1 both say the same thing: a false positive
//! silently destroys unrecoverable user data 30 seconds after it was copied.
//! When a shape is not distinctive enough to *prove* a secret, it stays below
//! the floor.
//!
//! # Ranking
//!
//! [`Detector::scan`] returns the **highest-confidence** match (manifest §7.2:
//! v1 returned the lowest *declaration index*, so text containing both an email
//! and a Terraform token was labelled "email"; declaration order must not be
//! semantic). Ties break on longer match, then on table order — all three are
//! deterministic, none is meaningful.
//!
//! # Layout
//!
//! The pipeline runs left to right and the modules are in that order:
//!
//! ```text
//! text ─▶ normalise ─▶ prefilter+regex ─▶ validators ─▶ ranking ─▶ Finding
//!         normalise     engine (rules)    validators     engine     finding
//! ```
//!
//! * [`finding`] — what a match *means*: [`Severity`], [`Finding`] and the
//!   auto-wipe floor that separates them.
//! * [`spec`] — what a rule *is*: category, confidence, pattern, validator.
//! * [`rules`] — which rules there are. Data only, and deliberately long; see
//!   its header.
//! * [`validators`] — the false-positive gates a matched rule must survive.
//! * [`normalise`] — NFKC folding, run once per scan before anything matches.
//! * [`engine`] — compiles the table and owns the two public verdicts.
//!
//! # Deliberately not in this module
//!
//! * Spans / redaction (manifest §1.1, I9) — no consumer yet; adding an unused
//!   span API would recreate §7.4's three-dead-entry-points problem.
//! * The password-manager bundle-ID list (§5.8) — it is a *capture-time* signal
//!   about the source app, not about content, and §5.8 says v2 should carry it
//!   as configuration rather than code.
//! * The telemetry scrubber's variety gate (§5.5) — §7.5 is explicit that v2
//!   must not maintain a second regex engine. The variety heuristic that does
//!   belong to content detection lives in [`validators::value_is_strong`].

mod engine;
mod finding;
mod normalise;
mod rules;
mod spec;
mod validators;

pub use engine::{Detector, DetectorError};
pub use finding::{Finding, Severity};
