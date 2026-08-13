//! What a rule *is*: the fields every entry in [`super::rules::RULES`] carries,
//! and the mapping from a rule to the [`Finding`] it produces.

use super::finding::{Finding, Severity, AUTOWIPE_CONFIDENCE_FLOOR};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Category {
    Credential,
    Financial,
    PersonalId,
    Infrastructure,
}

impl Category {
    fn as_str(self) -> &'static str {
        match self {
            Category::Credential => "credential",
            Category::Financial => "financial",
            Category::PersonalId => "personal_id",
            Category::Infrastructure => "infrastructure",
        }
    }
}

/// Post-match validators. Manifest §5.3/§5.4 plus the two v2 additions called
/// for in §7.1 and §7.7. The implementations live in [`super::validators`];
/// the dispatch is in [`super::engine`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Validator {
    /// The regex is the whole decision.
    None,
    /// The captured value must survive [`super::validators::value_is_strong`].
    ValueStrength,
    /// Issuer range, per-brand length and Luhn over the digit run (§5.4).
    CardNumber,
    /// Registered country structure and checksum for an IBAN (§8.1.4).
    Iban,
    /// SSN group-structure check (§4.2 — "the correct fix is the structural
    /// validator").
    SsnStructure,
    /// The match must actually be *formatted* like a phone number (§7.7 — the
    /// benign-corpus entry `the order number is 1234567890`).
    PhoneShape,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Generated selections may use AND even when the current pin does not.
pub(super) enum AllowlistCondition {
    Any,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AllowlistTarget {
    Secret,
    Match,
    Line,
}

pub(super) struct AllowlistSpec {
    pub(super) condition: AllowlistCondition,
    pub(super) target: AllowlistTarget,
    pub(super) regexes: &'static [&'static str],
    pub(super) stopwords: &'static [&'static str],
}

pub(super) struct RuleSpec {
    pub(super) upstream_ids: &'static [&'static str],
    pub(super) name: &'static str,
    pub(super) category: Category,
    pub(super) confidence: f32,
    pub(super) pattern: &'static str,
    pub(super) validator: Validator,
    pub(super) secret_group: usize,
    /// Classify, but never delete. Opt-in per rule and never derived from the
    /// confidence, because the two answer different questions: confidence is
    /// how sure the match is, this is what a correct match licenses. A rule
    /// that omits it stays wipeable above the floor, so a new rule cannot drop
    /// out of the gate by being forgotten — only by being argued for.
    pub(super) never_auto_delete: bool,
    pub(super) entropy: Option<f64>,
    pub(super) keywords: &'static [&'static str],
    pub(super) allowlists: &'static [AllowlistSpec],
}

impl RuleSpec {
    pub(super) fn finding(&self) -> Finding {
        Finding {
            rule: self.name,
            category: self.category.as_str(),
            confidence: self.confidence,
            severity: match (
                self.confidence >= AUTOWIPE_CONFIDENCE_FLOOR,
                self.never_auto_delete,
            ) {
                (true, false) => Severity::HighConfidence,
                (true, true) => Severity::Restricted,
                (false, _) => Severity::Flag,
            },
        }
    }
}
