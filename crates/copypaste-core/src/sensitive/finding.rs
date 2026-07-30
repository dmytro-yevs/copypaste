//! What a match means — the deletion decision, kept apart from the detection
//! decision (manifest I2). The floor below is the only thing that turns a
//! confidence number into permission to delete user data.

/// Manifest §4.1. Nothing may sit *exactly* on the floor: `ip_with_port` did,
/// and RFC1918 addresses in docker-compose snippets were silently auto-wiped
/// (`CopyPaste-8ys1`). Pinned by `no_rule_sits_exactly_on_the_floor`.
pub(super) const AUTOWIPE_CONFIDENCE_FLOOR: f32 = 0.70;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// Flag and keep out of the index, but never auto-delete.
    Flag,
    /// Above the auto-wipe floor.
    HighConfidence,
}

/// The highest-confidence rule that matched.
#[derive(Debug, Clone, PartialEq)]
pub struct Finding {
    /// Stable rule id, e.g. `aws_access_key`. The label belongs to the rule
    /// definition — v1 derived it by string-prefix dispatch on the name, so
    /// renaming a rule silently changed its label (manifest §7.8).
    pub rule: String,
    /// One of `credential`, `financial`, `personal_id`, `infrastructure`.
    pub category: String,
    pub confidence: f32,
    pub severity: Severity,
}
