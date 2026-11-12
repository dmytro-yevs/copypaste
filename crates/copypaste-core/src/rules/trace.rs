//! Explaining which rules matched an item and why.
//!
//! A trace never carries content: only rule ids, per-condition results and
//! byte ranges of regex matches inside the evaluated sample.

use serde::{Deserialize, Serialize};

use super::detect::DetectedKind;

/// Regex match ranges kept per condition for highlighting.
pub const MAX_TRACE_MATCHES: usize = 32;

/// The result of one condition, after negation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionResult {
    Matched,
    NotMatched,
    /// The attribute is unknown for this capture; both polarities do not
    /// match (content-rules R-5).
    Unknown,
    /// The condition could not be evaluated: unsupported type, failed
    /// compilation or an exhausted budget.
    Indeterminate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleResult {
    Matched,
    NotMatched,
    /// Received item created before the rule's `effective_from`.
    NotEffective,
    /// Unknown types in a rule that is not purely protective: disabled here.
    NeedsUpdate,
    /// The rule could not be evaluated and might match: the item is
    /// protected instead (content-rules R-20).
    FailedClosed,
    /// The evaluation budget ran out before this rule finished.
    BudgetExceeded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConditionTrace {
    pub index: usize,
    pub result: ConditionResult,
    /// Byte ranges of matches in the examined text, at most
    /// [`MAX_TRACE_MATCHES`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub matches: Vec<(usize, usize)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTraceEntry {
    pub rule_id: String,
    pub result: RuleResult,
    pub conditions: Vec<ConditionTrace>,
}

/// Rules in evaluation order. Rules after a stop are not listed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleTrace {
    pub rules: Vec<RuleTraceEntry>,
    /// Kinds detected for the item, when a condition needed detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detected_kinds: Option<Vec<DetectedKind>>,
    /// [`super::evaluate::EvalFlags`] bits.
    pub flags: u32,
}

impl RuleTrace {
    #[must_use]
    pub fn entry(&self, rule_id: &str) -> Option<&RuleTraceEntry> {
        self.rules.iter().find(|entry| entry.rule_id == rule_id)
    }
}
