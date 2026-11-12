//! Compiling rule definitions into an evaluator.
//!
//! A rule set is compiled once per rules revision into an immutable
//! [`CompiledRuleSet`]. Only enabled, live rules scoped to this device are
//! compiled. A rule that cannot be compiled here is kept in a fail-closed
//! state (content-rules R-20) instead of being dropped.

use std::sync::{Arc, RwLock};

use regex::{Regex, RegexBuilder};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use super::detect::DetectedKinds;
use super::evaluate::{bit, ContentTypes};
use super::model::{
    Action, ActionSlot, CaptureOrigin, Comparison, Condition, ConditionSlot, MatchMode,
    RuleDefinition, SizeUnit, TextOp, MAX_ACTIONS, MAX_CONDITIONS, MAX_EXPIRY_MS, MAX_LIST_VALUES,
    MAX_PATTERN_CHARS, MAX_RULES, MAX_TAGS_PER_ACTION, MIN_EXPIRY_MS, SCHEMA_VERSION,
};
use crate::storage::organization::{ContentRuleRecord, RuleOrigin};
use crate::storage::StoreError;

/// Compiled program size per pattern; larger patterns are rejected.
pub const REGEX_SIZE_LIMIT: usize = 512 * 1024;
/// Lazy DFA cache per pattern.
pub const REGEX_DFA_SIZE_LIMIT: usize = 1024 * 1024;
const MAX_ID_CHARS: usize = 128;
const MAX_CLEAR_AFTER_SECS: u32 = 86_400;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleErrorCode {
    InvalidDefinition,
    UnsupportedSchema,
    UnknownConditionType,
    UnknownActionType,
    TooManyConditions,
    TooManyActions,
    TooManyRules,
    NoActions,
    /// "Don't save" combined with another action (R-7).
    ConflictingActions,
    DuplicateAction,
    EmptyValue,
    EmptyList,
    TooManyValues,
    InvalidValue,
    PatternTooLong,
    /// Look-ahead or look-behind; the suggestion is two conditions with "all".
    UnsupportedLookAround,
    UnsupportedBackreference,
    PatternTooLarge,
    InvalidPattern,
    InvalidDuration,
    InvalidTags,
    InvalidTransform,
}

impl RuleErrorCode {
    const fn needs_update(self) -> bool {
        matches!(
            self,
            Self::UnsupportedSchema | Self::UnknownConditionType | Self::UnknownActionType
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleError {
    /// Location in the definition, such as `conditions[2].value`.
    pub path: String,
    /// Character offset inside a pattern, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    pub code: RuleErrorCode,
}

impl RuleError {
    fn at(path: impl Into<String>, code: RuleErrorCode) -> Self {
        Self {
            path: path.into(),
            offset: None,
            code,
        }
    }
}

/// Validate a definition as the editor and every receiving device do.
#[must_use]
pub fn validate(definition_json: &str) -> Vec<RuleError> {
    match RuleDefinition::parse(definition_json) {
        Ok(definition) => validate_definition(&definition),
        Err(_) => vec![RuleError::at("", RuleErrorCode::InvalidDefinition)],
    }
}

#[must_use]
pub fn validate_definition(definition: &RuleDefinition) -> Vec<RuleError> {
    let mut errors = Vec::new();
    if definition.conditions.len() > MAX_CONDITIONS {
        errors.push(RuleError::at(
            "conditions",
            RuleErrorCode::TooManyConditions,
        ));
    }
    for (index, slot) in definition.conditions.iter().enumerate() {
        let path = format!("conditions[{index}]");
        match slot {
            ConditionSlot::Known(condition) => {
                if let Err(error) = compile_condition(condition, &path) {
                    errors.push(error);
                }
            }
            ConditionSlot::Unknown(_) => {
                errors.push(RuleError::at(path, RuleErrorCode::UnknownConditionType));
            }
        }
    }
    validate_actions(&definition.actions, &mut errors);
    errors
}

fn validate_actions(actions: &[ActionSlot], errors: &mut Vec<RuleError>) {
    if actions.is_empty() {
        errors.push(RuleError::at("actions", RuleErrorCode::NoActions));
    }
    if actions.len() > MAX_ACTIONS {
        errors.push(RuleError::at("actions", RuleErrorCode::TooManyActions));
    }
    let mut seen: Vec<&str> = Vec::new();
    for (index, slot) in actions.iter().enumerate() {
        let path = format!("actions[{index}]");
        let action = match slot {
            ActionSlot::Known(action) => action,
            ActionSlot::Unknown(_) => {
                errors.push(RuleError::at(path, RuleErrorCode::UnknownActionType));
                continue;
            }
        };
        if seen.contains(&action.type_name()) {
            errors.push(RuleError::at(&path, RuleErrorCode::DuplicateAction));
        }
        seen.push(action.type_name());
        if actions.len() > 1 && matches!(action, Action::DontSave { .. }) {
            errors.push(RuleError::at(&path, RuleErrorCode::ConflictingActions));
        }
        let invalid = match action {
            Action::DontSave {
                clear_clipboard_after_secs: Some(secs),
            } => (!(1..=MAX_CLEAR_AFTER_SECS).contains(secs))
                .then_some(RuleErrorCode::InvalidDuration),
            Action::DeleteAfter { duration_ms, .. } => (!(MIN_EXPIRY_MS..=MAX_EXPIRY_MS)
                .contains(duration_ms))
            .then_some(RuleErrorCode::InvalidDuration),
            Action::AddTags { tag_ids } => (tag_ids.is_empty()
                || tag_ids.len() > MAX_TAGS_PER_ACTION
                || !tag_ids.iter().all(|id| valid_id(id)))
            .then_some(RuleErrorCode::InvalidTags),
            Action::Transform { transform_id, .. } => {
                (!valid_id(transform_id)).then_some(RuleErrorCode::InvalidTransform)
            }
            Action::DontSave { .. } | Action::MarkSecret {} | Action::HideFromAi {} => None,
        };
        if let Some(code) = invalid {
            errors.push(RuleError::at(path, code));
        }
    }
}

fn valid_id(id: &str) -> bool {
    !id.trim().is_empty() && id.chars().count() <= MAX_ID_CHARS && !id.chars().any(char::is_control)
}

/// One condition, ready to evaluate.
#[derive(Debug)]
pub(crate) enum Test {
    Text(Regex),
    ContentType(ContentTypes),
    Detected(DetectedKinds),
    SourceApp(Box<[String]>),
    Size {
        unit: SizeUnit,
        op: Comparison,
        value: u64,
    },
    Marker(u8),
    Origin(u8),
    /// Unsupported type or failed compilation: never decides a match.
    Indeterminate,
}

#[derive(Debug)]
pub(crate) struct CompiledCondition {
    pub(crate) negate: bool,
    pub(crate) test: Test,
}

/// How the evaluator treats a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Normal,
    /// Protect the item instead of applying actions whenever it might match.
    FailClosed,
    /// Not evaluated on this device.
    Inactive,
}

#[derive(Debug)]
pub(crate) struct CompiledRule {
    pub(crate) id: String,
    pub(crate) effective_from: i64,
    pub(crate) match_mode: MatchMode,
    pub(crate) conditions: Vec<CompiledCondition>,
    pub(crate) actions: Vec<Action>,
    pub(crate) stop_processing: bool,
    pub(crate) mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleHealth {
    Ready,
    /// Written by a newer version ("Update CopyPaste to run this rule").
    NeedsUpdate,
    /// Cannot be compiled here; matching captures fail closed (error badge).
    Broken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleStatus {
    pub rule_id: String,
    pub health: RuleHealth,
    pub errors: Vec<RuleError>,
}

/// A rule as stored: the synced record plus its placement.
#[derive(Debug, Clone, PartialEq)]
pub struct RuleInput {
    pub rule: ContentRuleRecord,
    pub enabled: bool,
    pub order_key: String,
}

/// The immutable evaluator for one rules revision.
#[derive(Debug, Default)]
pub struct CompiledRuleSet {
    pub(crate) rules: Vec<CompiledRule>,
    statuses: Vec<RuleStatus>,
}

impl CompiledRuleSet {
    /// Compile the enabled, live rules that run on `device_id`: built-ins
    /// first (the privacy guard tier), then user rules, each by order key.
    #[must_use]
    pub fn compile(inputs: impl IntoIterator<Item = RuleInput>, device_id: &str) -> Self {
        let mut selected: Vec<RuleInput> = inputs
            .into_iter()
            .filter(|input| {
                input.enabled
                    && input.rule.deleted_at.is_none()
                    && (input.rule.device_scope.is_empty()
                        || input.rule.device_scope.iter().any(|id| id == device_id))
            })
            .collect();
        selected.sort_by(|a, b| {
            let tier = |input: &RuleInput| input.rule.origin != RuleOrigin::Builtin;
            (tier(a), &a.order_key, &a.rule.id).cmp(&(tier(b), &b.order_key, &b.rule.id))
        });
        let mut set = Self::default();
        for (index, input) in selected.into_iter().enumerate() {
            let (rule, status) = if index < MAX_RULES {
                compile_rule(input.rule)
            } else {
                over_capacity(input.rule)
            };
            set.rules.push(rule);
            set.statuses.push(status);
        }
        set
    }

    /// Health of every compiled rule, in evaluation order.
    #[must_use]
    pub fn statuses(&self) -> &[RuleStatus] {
        &self.statuses
    }

    #[must_use]
    pub fn status(&self, rule_id: &str) -> Option<&RuleStatus> {
        self.statuses
            .iter()
            .find(|status| status.rule_id == rule_id)
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }
}

fn over_capacity(rule: ContentRuleRecord) -> (CompiledRule, RuleStatus) {
    let status = RuleStatus {
        rule_id: rule.id.clone(),
        health: RuleHealth::Broken,
        errors: vec![RuleError::at("", RuleErrorCode::TooManyRules)],
    };
    (
        CompiledRule {
            id: rule.id,
            effective_from: rule.effective_from,
            match_mode: MatchMode::All,
            conditions: vec![CompiledCondition {
                negate: false,
                test: Test::Indeterminate,
            }],
            actions: Vec::new(),
            stop_processing: false,
            mode: Mode::FailClosed,
        },
        status,
    )
}

fn compile_rule(rule: ContentRuleRecord) -> (CompiledRule, RuleStatus) {
    let newer_schema = rule.schema_version > SCHEMA_VERSION;
    let parsed = RuleDefinition::parse(&rule.definition_json);
    let Ok(definition) = parsed else {
        let code = if newer_schema {
            RuleErrorCode::UnsupportedSchema
        } else {
            RuleErrorCode::InvalidDefinition
        };
        let (mut compiled, mut status) = over_capacity(rule);
        status.errors = vec![RuleError::at("", code)];
        if newer_schema {
            status.health = RuleHealth::NeedsUpdate;
            compiled.mode = Mode::Inactive;
        }
        return (compiled, status);
    };
    let mut errors = validate_definition(&definition);
    if newer_schema {
        errors.insert(0, RuleError::at("", RuleErrorCode::UnsupportedSchema));
    }
    let needs_update = errors.iter().any(|error| error.code.needs_update());
    let broken = errors.iter().any(|error| !error.code.needs_update());
    let protective_only = !definition
        .actions
        .iter()
        .any(|slot| matches!(slot, ActionSlot::Unknown(_)))
        && definition.known_actions().all(Action::is_protective);
    let (health, mode) = if broken {
        (RuleHealth::Broken, Mode::FailClosed)
    } else if needs_update && protective_only {
        (RuleHealth::NeedsUpdate, Mode::FailClosed)
    } else if needs_update {
        (RuleHealth::NeedsUpdate, Mode::Inactive)
    } else {
        (RuleHealth::Ready, Mode::Normal)
    };
    let conditions = if mode == Mode::Inactive {
        Vec::new()
    } else {
        definition
            .conditions
            .iter()
            .map(|slot| match slot {
                ConditionSlot::Known(condition) => CompiledCondition {
                    negate: condition.negated(),
                    test: compile_condition(condition, "").unwrap_or(Test::Indeterminate),
                },
                ConditionSlot::Unknown(_) => CompiledCondition {
                    negate: false,
                    test: Test::Indeterminate,
                },
            })
            .collect()
    };
    let actions = definition.known_actions().cloned().collect();
    let status = RuleStatus {
        rule_id: rule.id.clone(),
        health,
        errors,
    };
    (
        CompiledRule {
            id: rule.id,
            effective_from: rule.effective_from,
            match_mode: definition.match_mode,
            conditions,
            actions,
            stop_processing: definition.stop_processing,
            mode,
        },
        status,
    )
}

fn list<T>(values: &[T], path: &str) -> Result<(), RuleError> {
    if values.is_empty() {
        Err(RuleError::at(
            format!("{path}.any_of"),
            RuleErrorCode::EmptyList,
        ))
    } else if values.len() > MAX_LIST_VALUES {
        Err(RuleError::at(
            format!("{path}.any_of"),
            RuleErrorCode::TooManyValues,
        ))
    } else {
        Ok(())
    }
}

fn compile_condition(condition: &Condition, path: &str) -> Result<Test, RuleError> {
    Ok(match condition {
        Condition::ContentText {
            op,
            value,
            case_sensitive,
            ..
        } => Test::Text(compile_text(*op, value, *case_sensitive, path)?),
        Condition::ContentType { any_of, .. } => {
            list(any_of, path)?;
            Test::ContentType(any_of.iter().copied().collect())
        }
        Condition::DetectedKind { any_of, .. } => {
            list(any_of, path)?;
            Test::Detected(any_of.iter().copied().collect())
        }
        Condition::SourceApp { any_of, .. } => {
            list(any_of, path)?;
            if !any_of.iter().all(|app| valid_id(app)) {
                return Err(RuleError::at(
                    format!("{path}.any_of"),
                    RuleErrorCode::InvalidValue,
                ));
            }
            Test::SourceApp(any_of.clone().into_boxed_slice())
        }
        Condition::Size {
            unit, op, value, ..
        } => Test::Size {
            unit: *unit,
            op: *op,
            value: *value,
        },
        Condition::PrivacyMarker { any_of, .. } => {
            list(any_of, path)?;
            Test::Marker(
                any_of
                    .iter()
                    .fold(0, |bits, marker| bits | bit(*marker as u8)),
            )
        }
        Condition::CaptureOrigin { any_of, .. } => {
            list(any_of, path)?;
            Test::Origin(
                any_of
                    .iter()
                    .fold(0, |bits, origin| bits | origin_bit(*origin)),
            )
        }
    })
}

pub(crate) const fn origin_bit(origin: CaptureOrigin) -> u8 {
    bit(origin as u8)
}

/// Every text operator compiles to the same engine, so case folding and
/// Unicode semantics are identical across operators and platforms (R-21).
fn compile_text(
    op: TextOp,
    value: &str,
    case_sensitive: bool,
    path: &str,
) -> Result<Regex, RuleError> {
    let path = format!("{path}.value");
    if value.is_empty() {
        return Err(RuleError::at(path, RuleErrorCode::EmptyValue));
    }
    if value.chars().count() > MAX_PATTERN_CHARS {
        return Err(RuleError::at(path, RuleErrorCode::PatternTooLong));
    }
    let pattern = match op {
        TextOp::MatchesRegex => {
            if let Some((code, offset)) = unsupported_syntax(value) {
                return Err(RuleError {
                    path,
                    offset: Some(offset),
                    code,
                });
            }
            value.to_owned()
        }
        TextOp::Contains => regex::escape(value),
        TextOp::Equals => format!(r"\A(?:{})\z", regex::escape(value)),
        TextOp::StartsWith => format!(r"\A(?:{})", regex::escape(value)),
        TextOp::EndsWith => format!(r"(?:{})\z", regex::escape(value)),
    };
    RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .multi_line(true)
        .size_limit(REGEX_SIZE_LIMIT)
        .dfa_size_limit(REGEX_DFA_SIZE_LIMIT)
        .build()
        .map_err(|error| {
            let code = if matches!(error, regex::Error::CompiledTooBig(_)) {
                RuleErrorCode::PatternTooLarge
            } else {
                RuleErrorCode::InvalidPattern
            };
            RuleError::at(path, code)
        })
}

/// Find syntax the linear-time engine rejects, to explain it precisely.
fn unsupported_syntax(pattern: &str) -> Option<(RuleErrorCode, usize)> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut class_depth = 0usize;
    let mut index = 0;
    while index < chars.len() {
        match chars[index] {
            '\\' => {
                let next = chars.get(index + 1).copied();
                let backreference = matches!(next, Some('1'..='9'))
                    || (next == Some('k') && chars.get(index + 2) == Some(&'<'));
                if class_depth == 0 && backreference {
                    return Some((RuleErrorCode::UnsupportedBackreference, index));
                }
                index += 1;
            }
            '[' => class_depth += 1,
            ']' if class_depth > 0 => class_depth -= 1,
            '(' if class_depth == 0 => {
                let rest: String = chars[index + 1..chars.len().min(index + 4)]
                    .iter()
                    .collect();
                if ["?=", "?!", "?<=", "?<!"]
                    .iter()
                    .any(|prefix| rest.starts_with(prefix))
                {
                    return Some((RuleErrorCode::UnsupportedLookAround, index));
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

/// Read every live rule with its placement. A rule whose placement has not
/// arrived yet runs, because a missing placement must not drop protection.
pub fn load_inputs(conn: &Connection) -> Result<Vec<RuleInput>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT r.id,r.origin,r.builtin_version,r.customized_json,r.name,r.definition_json,\
         r.device_scope_json,r.schema_version,r.effective_from,p.enabled,p.order_key \
         FROM content_rules r LEFT JOIN content_rule_placement p ON p.id=r.id \
         WHERE r.deleted_at IS NULL",
    )?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<u32>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, String>(6)?,
            row.get::<_, u32>(7)?,
            row.get::<_, i64>(8)?,
            row.get::<_, Option<bool>>(9)?,
            row.get::<_, Option<String>>(10)?,
        ))
    })?;
    let invalid = |_| StoreError::InvalidHistoryQuery;
    let mut inputs = Vec::new();
    for row in rows {
        let (id, origin, version, customized, name, definition, scope, schema, from, enabled, key) =
            row?;
        let origin = if origin == "builtin" {
            RuleOrigin::Builtin
        } else {
            RuleOrigin::User
        };
        let default_enabled = match origin {
            RuleOrigin::Builtin => {
                super::builtin::builtin(&id).is_none_or(|b| b.enabled_by_default)
            }
            RuleOrigin::User => true,
        };
        inputs.push(RuleInput {
            enabled: enabled.unwrap_or(default_enabled),
            order_key: key.unwrap_or_default(),
            rule: ContentRuleRecord {
                id,
                origin,
                builtin_version: version,
                customized_fields: serde_json::from_str(&customized).map_err(invalid)?,
                name,
                definition_json: definition,
                device_scope: serde_json::from_str(&scope).map_err(invalid)?,
                schema_version: schema,
                effective_from: from,
                deleted_at: None,
            },
        });
    }
    Ok(inputs)
}

/// Holds the compiled set for the current rules revision and swaps it when
/// the revision counter moves.
#[derive(Debug, Default)]
pub struct RuleSetCache {
    slot: RwLock<Option<(i64, String, Arc<CompiledRuleSet>)>>,
}

impl RuleSetCache {
    pub fn current(
        &self,
        conn: &Connection,
        device_id: &str,
    ) -> Result<Arc<CompiledRuleSet>, StoreError> {
        let revision: i64 = conn.query_row(
            "SELECT rules FROM resource_revisions WHERE singleton = 0",
            [],
            |row| row.get(0),
        )?;
        if let Some((cached, device, set)) = &*self.slot.read().unwrap_or_else(|e| e.into_inner()) {
            if *cached == revision && device == device_id {
                return Ok(Arc::clone(set));
            }
        }
        let set = Arc::new(CompiledRuleSet::compile(load_inputs(conn)?, device_id));
        *self.slot.write().unwrap_or_else(|e| e.into_inner()) =
            Some((revision, device_id.to_owned(), Arc::clone(&set)));
        Ok(set)
    }

    /// Drop the compiled set; the next capture recompiles it.
    pub fn release(&self) {
        *self.slot.write().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognises_unsupported_syntax_outside_classes() {
        assert_eq!(
            unsupported_syntax(r"foo(?=bar)"),
            Some((RuleErrorCode::UnsupportedLookAround, 3))
        );
        assert_eq!(
            unsupported_syntax(r"(a)\1"),
            Some((RuleErrorCode::UnsupportedBackreference, 3))
        );
        assert_eq!(unsupported_syntax(r"[(?=]\(?=x"), None);
        assert!(compile_text(TextOp::MatchesRegex, r"\w+@\w+\.\w+", false, "c").is_ok());
        assert_eq!(
            compile_text(TextOp::MatchesRegex, r"(a{1,100}){1,100}", false, "c")
                .unwrap_err()
                .code,
            RuleErrorCode::PatternTooLarge
        );
    }

    #[test]
    fn cache_recompiles_only_when_the_revision_moves() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE resource_revisions(singleton INTEGER, rules INTEGER);
             INSERT INTO resource_revisions VALUES (0, 1);
             CREATE TABLE content_rules(id TEXT, origin TEXT, builtin_version INTEGER,
               customized_json TEXT, name TEXT, definition_json TEXT, device_scope_json TEXT,
               schema_version INTEGER, effective_from INTEGER, deleted_at INTEGER);
             CREATE TABLE content_rule_placement(id TEXT, enabled INTEGER, order_key TEXT);
             INSERT INTO content_rules VALUES ('r1','user',NULL,'[]','R',
               '{\"match\":\"all\",\"actions\":[{\"type\":\"mark_secret\"}]}','[]',1,0,NULL);
             INSERT INTO content_rules VALUES ('r2','user',NULL,'[]','R',
               '{\"match\":\"all\",\"actions\":[{\"type\":\"mark_secret\"}]}','[]',1,0,5);
             INSERT INTO content_rule_placement VALUES ('r1',1,'a');",
        )
        .unwrap();
        let cache = RuleSetCache::default();
        let first = cache.current(&conn, "device").unwrap();
        assert_eq!(first.statuses().len(), 1);
        assert!(Arc::ptr_eq(
            &first,
            &cache.current(&conn, "device").unwrap()
        ));
        conn.execute_batch(
            "UPDATE content_rule_placement SET enabled=0; UPDATE resource_revisions SET rules=2;",
        )
        .unwrap();
        assert!(cache.current(&conn, "device").unwrap().is_empty());
    }
}
