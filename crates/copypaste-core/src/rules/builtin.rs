//! Built-in rules with well-known ids.
//!
//! Stable ids let the same default on two devices merge instead of
//! duplicating. Shipped defaults only mark and hide: none skips, deletes or
//! expires content. The credential expiry rule ships disabled for the user to
//! enable.

use super::detect::DetectedKind;
use super::model::{Action, ActionSlot, Condition, ConditionSlot, MatchMode, RuleDefinition};
use crate::storage::organization::{ContentRuleRecord, RuleOrigin, RulePlacementRecord};

pub const CREDENTIALS_ID: &str = "builtin.credentials";
pub const PAYMENT_CARDS_ID: &str = "builtin.payment_cards";
pub const CREDENTIALS_EXPIRY_ID: &str = "builtin.credentials_expiry";

const FIFTEEN_MINUTES_MS: i64 = 15 * 60 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuiltinRule {
    pub id: &'static str,
    /// Raised when the shipped definition changes.
    pub version: u32,
    pub name: &'static str,
    pub enabled_by_default: bool,
    pub order_key: &'static str,
    kind: DetectedKind,
    protect: bool,
    expire: bool,
}

const BUILTINS: [BuiltinRule; 3] = [
    BuiltinRule {
        id: CREDENTIALS_ID,
        version: 1,
        name: "Detected credentials",
        enabled_by_default: true,
        order_key: "a",
        kind: DetectedKind::Credential,
        protect: true,
        expire: false,
    },
    BuiltinRule {
        id: PAYMENT_CARDS_ID,
        version: 1,
        name: "Payment card numbers",
        enabled_by_default: true,
        order_key: "b",
        kind: DetectedKind::PaymentCard,
        protect: true,
        expire: false,
    },
    BuiltinRule {
        id: CREDENTIALS_EXPIRY_ID,
        version: 1,
        name: "Delete detected credentials",
        enabled_by_default: false,
        order_key: "c",
        kind: DetectedKind::Credential,
        protect: false,
        expire: true,
    },
];

#[must_use]
pub fn builtins() -> &'static [BuiltinRule] {
    &BUILTINS
}

#[must_use]
pub fn builtin(id: &str) -> Option<&'static BuiltinRule> {
    BUILTINS.iter().find(|rule| rule.id == id)
}

impl BuiltinRule {
    #[must_use]
    pub fn definition(&self) -> RuleDefinition {
        let mut actions = Vec::new();
        if self.protect {
            actions.push(Action::MarkSecret {});
            actions.push(Action::HideFromAi {});
        }
        if self.expire {
            actions.push(Action::DeleteAfter {
                duration_ms: FIFTEEN_MINUTES_MS,
                clear_clipboard: true,
            });
        }
        RuleDefinition {
            match_mode: MatchMode::All,
            conditions: vec![ConditionSlot::Known(Condition::DetectedKind {
                any_of: vec![self.kind],
                negate: false,
            })],
            actions: actions.into_iter().map(ActionSlot::Known).collect(),
            stop_processing: false,
        }
    }

    /// The synced record seeded on a device; `effective_from` is the seeding
    /// time, so received history created earlier is never re-evaluated.
    #[must_use]
    pub fn record(&self, effective_from: i64) -> ContentRuleRecord {
        ContentRuleRecord {
            id: self.id.to_owned(),
            origin: RuleOrigin::Builtin,
            builtin_version: Some(self.version),
            customized_fields: Vec::new(),
            name: self.name.to_owned(),
            definition_json: self.definition().to_json(),
            device_scope: Vec::new(),
            schema_version: super::model::SCHEMA_VERSION,
            effective_from,
            deleted_at: None,
            base_stamp: None,
        }
    }

    #[must_use]
    pub fn placement(&self) -> RulePlacementRecord {
        RulePlacementRecord {
            id: self.id.to_owned(),
            enabled: self.enabled_by_default,
            order_key: self.order_key.to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_defaults_never_expire_or_skip_unless_disabled() {
        for rule in builtins() {
            let definition = rule.definition();
            assert!(super::super::compile::validate_definition(&definition).is_empty());
            let destructive = definition.known_actions().any(|action| {
                matches!(action, Action::DontSave { .. } | Action::DeleteAfter { .. })
            });
            assert!(!(destructive && rule.enabled_by_default), "{}", rule.id);
        }
    }
}
