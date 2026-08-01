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
    /// Capture group 1 must survive [`super::validators::value_is_strong`].
    ValueStrength,
    /// Luhn checksum over the digit run (§5.4).
    Luhn,
    /// Registered country structure and checksum for an IBAN (§8.1.4).
    Iban,
    /// SSN group-structure check (§4.2 — "the correct fix is the structural
    /// validator").
    SsnStructure,
    /// The match must actually be *formatted* like a phone number (§7.7 — the
    /// benign-corpus entry `the order number is 1234567890`).
    PhoneShape,
}

pub(super) struct RuleSpec {
    pub(super) name: &'static str,
    pub(super) category: Category,
    pub(super) confidence: f32,
    pub(super) pattern: &'static str,
    pub(super) validator: Validator,
}

impl RuleSpec {
    pub(super) fn finding(&self) -> Finding {
        Finding {
            rule: self.name.to_string(),
            category: self.category.as_str().to_string(),
            confidence: self.confidence,
            severity: if self.confidence >= AUTOWIPE_CONFIDENCE_FLOOR {
                Severity::HighConfidence
            } else {
                Severity::Flag
            },
        }
    }
}
