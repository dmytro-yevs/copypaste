//! What a match means — the deletion decision, kept apart from the detection
//! decision (manifest I2). The floor below is the only thing that turns a
//! confidence number into permission to delete user data.

use std::ops::Range;

/// Manifest §4.1. Nothing may sit *exactly* on the floor: `ip_with_port` did,
/// and RFC1918 addresses in docker-compose snippets were silently auto-wiped
/// (`CopyPaste-8ys1`). Pinned by `no_rule_sits_exactly_on_the_floor`.
pub(super) const AUTOWIPE_CONFIDENCE_FLOOR: f32 = 0.70;

/// What a match permits, in increasing order of consequence. Ordered so a
/// caller states the least severity it acts on rather than listing variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Detected and maskable, but inert for whole-item classification and wipe.
    Flag,
    /// Classified sensitive — kept out of the search index, out of sync and out
    /// of previews — and never a reason to delete.
    ///
    /// The band exists because those are two decisions, and a rule can be
    /// certain of the first while proving nothing about the second. A validated
    /// card number is the case: nothing in the digits separates the user's own
    /// card from a Luhn-valid order id in an issuer range, and I1 ranks an
    /// irreversible deletion of the second above the cost of keeping the first
    /// unsearchable. `sweep_sensitive` already withheld deletion this way for
    /// payloads it could not read; this makes the state reachable from a rule.
    Restricted,
    /// Above the auto-wipe floor.
    HighConfidence,
}

/// Metadata for the highest-confidence rule that matched.
///
/// Every field is a copy of something the rule table already owns for the
/// program's lifetime, so it is borrowed rather than allocated: a page of
/// history scans thousands of items and the two `String`s this used to build
/// per match were the whole cost of describing a match.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Finding {
    /// Stable rule id, e.g. `aws_access_key`. The label belongs to the rule
    /// definition — v1 derived it by string-prefix dispatch on the name, so
    /// renaming a rule silently changed its label (manifest §7.8).
    pub rule: &'static str,
    /// One of `credential`, `financial`, `personal_id`, `infrastructure`.
    pub category: &'static str,
    pub confidence: f32,
    pub severity: Severity,
}

/// One validated match. `start` and `end` are UTF-8 byte offsets into the
/// NFKC-normalised string scanned by [`super::Detector::scan_all`], not
/// necessarily into the caller's original input (manifest I3).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpannedFinding {
    pub rule: &'static str,
    pub category: &'static str,
    pub confidence: f32,
    pub severity: Severity,
    pub start: usize,
    pub end: usize,
}

impl SpannedFinding {
    pub(super) fn new(finding: Finding, start: usize, end: usize) -> Self {
        Self {
            rule: finding.rule,
            category: finding.category,
            confidence: finding.confidence,
            severity: finding.severity,
            start,
            end,
        }
    }

    /// The same normalised UTF-8 byte offsets as `start` and `end`.
    #[must_use]
    pub fn byte_range(&self) -> Range<usize> {
        self.start..self.end
    }

    pub(super) fn without_span(self) -> Finding {
        Finding {
            rule: self.rule,
            category: self.category,
            confidence: self.confidence,
            severity: self.severity,
        }
    }
}
