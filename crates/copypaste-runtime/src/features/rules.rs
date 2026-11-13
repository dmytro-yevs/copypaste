//! Content rules: management, test, retroactive preview and apply.
//!
//! Rules are synced records; this service lists and edits them, tests a
//! draft or a saved rule against a sample that is never stored, and
//! explains how rules treat a stored clip. Preview and apply live in their
//! own files. Logs carry rule ids, counts and error codes, never content,
//! names or patterns.

mod apply;
mod builtins;
mod preview;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_core::now_ms;
use copypaste_core::rules::compile::{self, CompiledRuleSet, RuleErrorCode, RuleHealth, RuleInput};
use copypaste_core::rules::evaluate::{
    ContentTypes, EvaluationContext, EvaluationInput, PrivacyMarkers,
};
use copypaste_core::rules::model::{Action, CaptureOrigin, RuleDefinition, SCHEMA_VERSION};
use copypaste_core::rules::trace::{self as core_trace, ConditionResult, RuleResult};
use copypaste_core::storage::organization::{ContentRuleRecord, RuleOrigin as CoreOrigin};
use copypaste_core::storage::rules::{
    RuleDraft as CoreDraft, RuleSubject, RuleWriteError, StoredRule, SubjectText,
};
use copypaste_core::StoreError;
use copypaste_ipc::rules::{
    PrivacyDefaults, RuleConflict, RuleDeviceScope, RuleDraft, RuleError, RuleList, RuleOp,
    RuleOrigin, RuleRow, RuleSample, RuleStats, RuleTrace, TraceCondition, TraceResult, TraceStep,
};
use copypaste_ipc::{ErrorCode, LibraryKinds, Response, ResponseData};

use crate::messages::{storage_error, MSG_NOT_FOUND};
use crate::Runtime;

/// Rules changed by another device this recently are highlighted.
const NEW_FROM_DEVICE_MS: i64 = 7 * 24 * 60 * 60 * 1000;
/// The id a draft without one is evaluated under.
const DRAFT_ID: &str = "draft";
/// Sorts a draft after every placed rule of its tier.
const DRAFT_ORDER_KEY: &str = "~";

const MSG_RULE_NOT_FOUND: &str = "That rule no longer exists.";
const MSG_INVALID_DEFINITION: &str = "The rule is not valid. Fix the marked fields.";
const MSG_INVALID_NAME: &str = "A rule needs a name of at most 128 characters.";
const MSG_INVALID_SCOPE: &str = "Choose the devices this rule runs on again.";
const MSG_LIMIT: &str = "You have reached the limit of 200 rules.";
const MSG_NOT_BUILTIN: &str = "Only a privacy default can be reset.";
const MSG_NO_RULE: &str = "Choose a rule or write one first.";

impl Runtime {
    pub(crate) fn rules(&self, id: u64, op: RuleOp) -> Response {
        match op {
            RuleOp::List => self.rule_list(id),
            RuleOp::Save { draft } => self.save_rule(id, draft),
            RuleOp::Delete { id: rule_id } => self.set_rule_deleted(id, &rule_id, true),
            RuleOp::Restore { id: rule_id } => self.set_rule_deleted(id, &rule_id, false),
            RuleOp::Reorder {
                id: rule_id,
                after_id,
            } => self.rule_write(id, |runtime| {
                runtime.store.reorder_content_rule(
                    &runtime.keyring.item_key(),
                    &rule_id,
                    after_id.as_deref(),
                )
            }),
            RuleOp::SetEnabled {
                id: rule_id,
                enabled,
            } => self.rule_write_row(id, &rule_id, |runtime| {
                runtime.store.set_content_rule_enabled(
                    &runtime.keyring.item_key(),
                    &rule_id,
                    enabled,
                )
            }),
            RuleOp::RestoreConflict { conflict_id } => {
                let started = now_ms();
                match self.store.restore_content_rule_conflict(
                    &self.keyring.item_key(),
                    &conflict_id,
                    started,
                ) {
                    Ok(rule_id) => {
                        self.library_written(started, LibraryKinds::RULES);
                        self.rule_response(id, &rule_id)
                    }
                    Err(error) => write_refusal(id, "restore_rule_conflict", &error),
                }
            }
            RuleOp::Validate { definition_json } => Response::ok(
                id,
                ResponseData::RuleErrors(
                    compile::validate(&definition_json)
                        .iter()
                        .map(wire_error)
                        .collect(),
                ),
            ),
            RuleOp::Test {
                definition_json,
                rule_id,
                sample,
                full_set,
            } => self.test_rule(
                id,
                definition_json.as_deref(),
                rule_id.as_deref(),
                &sample,
                full_set,
            ),
            RuleOp::SkipLog => Response::ok(id, ResponseData::SkipLog(self.skipped_captures())),
            RuleOp::ResetBuiltin { .. } => builtins::handle(self, id, op),
            RuleOp::Preview { .. } => preview::handle(self, id, op),
            RuleOp::Apply { .. } | RuleOp::ApplyProgress { .. } | RuleOp::Cancel { .. } => {
                apply::handle(self, id, op)
            }
        }
    }

    fn rule_list(&self, id: u64) -> Response {
        let listed = self
            .store
            .content_rule_list(now_ms())
            .and_then(|rules| self.rule_rows(rules))
            .and_then(|rules| Ok((rules, self.store.content_rule_conflicts()?)));
        let (rules, conflicts) = match listed {
            Ok(listed) => listed,
            Err(error) => return storage_error(id, "rule_list", &error),
        };
        let devices: Vec<String> = conflicts.iter().map(|c| c.from_device.clone()).collect();
        let names = self.device_labels(&devices);
        let config = self.settings.get();
        let privacy_defaults = PrivacyDefaults {
            skip_secret: config.skip_secret,
            skip_transient: config.skip_transient,
            excluded_app_count: u32::try_from(config.excluded_app_bundle_ids.len())
                .unwrap_or(u32::MAX),
        };
        drop(config);
        Response::ok(
            id,
            ResponseData::RuleList(RuleList {
                privacy_defaults,
                rules,
                conflicts: conflicts
                    .into_iter()
                    .map(|conflict| RuleConflict {
                        from_device: names.get(&conflict.from_device).cloned(),
                        id: conflict.id,
                        rule_id: conflict.rule_id,
                        created_at: conflict.created_at,
                    })
                    .collect(),
            }),
        )
    }

    fn save_rule(&self, id: u64, draft: RuleDraft) -> Response {
        let started = now_ms();
        let draft = CoreDraft {
            id: draft.id,
            name: draft.name,
            enabled: draft.enabled,
            definition_json: draft.definition_json,
            device_scope: match draft.device_scope {
                RuleDeviceScope::AllDevices => Vec::new(),
                RuleDeviceScope::Devices { ids } => ids,
            },
        };
        match self
            .store
            .save_content_rule(&self.keyring.item_key(), &draft, started)
        {
            Ok(rule_id) => {
                tracing::debug!(rule = %rule_id, "content rule saved");
                self.library_written(started, LibraryKinds::RULES);
                self.rule_response(id, &rule_id)
            }
            Err(error) => write_refusal(id, "save_rule", &error),
        }
    }

    fn set_rule_deleted(&self, id: u64, rule_id: &str, deleted: bool) -> Response {
        let started = now_ms();
        match self.store.set_content_rule_deleted(
            &self.keyring.item_key(),
            rule_id,
            deleted,
            started,
        ) {
            Ok(changed) => {
                if changed {
                    self.library_written(started, LibraryKinds::RULES);
                }
                Response::ok(id, ResponseData::Empty {})
            }
            Err(error) => write_refusal(id, "delete_rule", &error),
        }
    }

    /// Runs one rule write and answers with nothing.
    fn rule_write(
        &self,
        id: u64,
        write: impl FnOnce(&Self) -> Result<(), RuleWriteError>,
    ) -> Response {
        let started = now_ms();
        match write(self) {
            Ok(()) => {
                self.library_written(started, LibraryKinds::RULES);
                Response::ok(id, ResponseData::Empty {})
            }
            Err(error) => write_refusal(id, "rule_write", &error),
        }
    }

    /// Runs one rule write and answers with the rule as listed.
    fn rule_write_row(
        &self,
        id: u64,
        rule_id: &str,
        write: impl FnOnce(&Self) -> Result<(), RuleWriteError>,
    ) -> Response {
        let response = self.rule_write(id, write);
        if response.ok {
            self.rule_response(id, rule_id)
        } else {
            response
        }
    }

    fn rule_response(&self, id: u64, rule_id: &str) -> Response {
        let row = self
            .store
            .content_rule(rule_id)
            .and_then(|rule| self.rule_rows(rule.into_iter().collect()));
        match row {
            Ok(mut rows) if !rows.is_empty() => {
                Response::ok(id, ResponseData::Rule(rows.remove(0)))
            }
            Ok(_) => Response::err(id, ErrorCode::NotFound, MSG_RULE_NOT_FOUND),
            Err(error) => storage_error(id, "rule", &error),
        }
    }

    /// Rules as the list shows them, with their health on this device, local
    /// statistics and the device that recently changed them.
    fn rule_rows(&self, rules: Vec<StoredRule>) -> Result<Vec<RuleRow>, StoreError> {
        let compiled = self.compiled_rules()?;
        let stats = self.store.content_rule_stats()?;
        let now = now_ms();
        let here = self.meta.device_id();
        let recent: Vec<String> = rules
            .iter()
            .filter(|rule| {
                rule.record.origin == CoreOrigin::User && rule.stamp > now - NEW_FROM_DEVICE_MS
            })
            .filter_map(|rule| rule.author.clone())
            .filter(|author| author != here)
            .collect();
        let names = self.device_labels(&recent);
        Ok(rules
            .into_iter()
            .map(|rule| {
                let (unsupported, badge) = rule_health(&compiled, &rule.record);
                let (match_count, last_matched_at) = stats
                    .get(&rule.record.id)
                    .map_or((0, None), |(count, at)| (*count, Some(*at)));
                let new_from_device = rule
                    .author
                    .as_ref()
                    .filter(|_| {
                        rule.record.origin == CoreOrigin::User
                            && rule.stamp > now - NEW_FROM_DEVICE_MS
                    })
                    .and_then(|author| names.get(author).cloned());
                let record = rule.record;
                RuleRow {
                    origin: match record.origin {
                        CoreOrigin::Builtin => RuleOrigin::Builtin,
                        CoreOrigin::User => RuleOrigin::User,
                    },
                    builtin_version: record.builtin_version,
                    name: record.name,
                    enabled: rule.enabled,
                    order_key: rule.order_key,
                    definition_json: record.definition_json,
                    device_scope: if record.device_scope.is_empty() {
                        RuleDeviceScope::AllDevices
                    } else {
                        RuleDeviceScope::Devices {
                            ids: record.device_scope,
                        }
                    },
                    error_badge: badge.clone(),
                    new_from_device,
                    unsupported,
                    deleted_at: record.deleted_at,
                    stats: RuleStats {
                        match_count,
                        last_matched_at,
                        last_error: badge,
                    },
                    id: record.id,
                }
            })
            .collect())
    }

    /// Names of devices by id; this device answers with its own name and an
    /// unknown device is left out.
    fn device_labels(&self, ids: &[String]) -> HashMap<String, String> {
        let here = self.meta.device_id();
        let remote: Vec<String> = ids
            .iter()
            .filter(|id| *id != here)
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut names = if remote.is_empty() {
            HashMap::new()
        } else {
            self.store.device_names(&remote).unwrap_or_else(|error| {
                tracing::warn!(?error, "could not resolve rule device names");
                HashMap::new()
            })
        };
        if ids.iter().any(|id| id == here) {
            names.insert(here.to_owned(), self.meta.device_name());
        }
        names
    }

    fn test_rule(
        &self,
        id: u64,
        definition_json: Option<&str>,
        rule_id: Option<&str>,
        sample: &RuleSample,
        full_set: bool,
    ) -> Response {
        let set = match self.rule_set(definition_json, rule_id, full_set) {
            Ok(set) => set,
            Err(refusal) => return refusal.response(id),
        };
        let evaluation = match sample {
            RuleSample::Text { text } => set.compiled.evaluate_traced(
                &EvaluationInput::text(text, now_ms()),
                EvaluationContext::Capture,
            ),
            RuleSample::Clip { id: clip_id } => {
                let (subject, text) = match self.rule_subject_with_text(clip_id) {
                    Ok(Some(found)) => found,
                    Ok(None) => return Response::err(id, ErrorCode::NotFound, MSG_NOT_FOUND),
                    Err(error) => return storage_error(id, "rule_test", &error),
                };
                let input = self.subject_input(&subject, text.as_ref().map(|t| t.as_str()));
                set.compiled
                    .evaluate_traced(&input, EvaluationContext::Capture)
            }
        };
        let mut trace = set.wire_trace(&evaluation.trace.unwrap_or_default());
        let outcome = evaluation.outcome;
        trace.skipped = outcome.dont_save.is_some();
        trace.secret = outcome.secret;
        trace.hidden_from_ai = outcome.hide_from_ai;
        trace.expires_after_ms = outcome
            .expiry
            .and_then(|expiry| u64::try_from(expiry.duration_ms).ok());
        trace.tag_ids = outcome.tag_ids;
        trace.transform_ids = outcome
            .transforms
            .into_iter()
            .map(|step| step.transform_id)
            .collect();
        Response::ok(id, ResponseData::RuleTrace(trace))
    }

    /// How the rules this device runs treat a stored clip, with the
    /// protection it actually carries. `None` when the clip is gone.
    pub(crate) fn clip_rule_trace(&self, clip_id: &str) -> Result<Option<RuleTrace>, StoreError> {
        let Some((subject, text)) = self.rule_subject_with_text(clip_id)? else {
            return Ok(None);
        };
        let set = RuleSet::new(self.compiled_rules()?, &self.store.content_rule_inputs()?);
        let input = self.subject_input(&subject, text.as_ref().map(|t| t.as_str()));
        let context = if input.origin == CaptureOrigin::LocalCopy {
            EvaluationContext::Capture
        } else {
            EvaluationContext::Receipt
        };
        let evaluation = set.compiled.evaluate_traced(&input, context);
        drop(text);
        let annotations = self
            .store
            .item_annotations(&[clip_id.to_owned()])?
            .remove(clip_id)
            .unwrap_or_default();
        let state = self.store.clip_local_state(clip_id)?;
        let mut trace = set.wire_trace(&evaluation.trace.unwrap_or_default());
        trace.secret = subject.secret;
        trace.hidden_from_ai = annotations.hidden_from_ai;
        trace.expires_after_ms = state
            .and_then(|state| state.expiry_duration_ms)
            .and_then(|ms| u64::try_from(ms).ok());
        trace.tag_ids = annotations.rule_tag_ids;
        trace.transform_ids = evaluation
            .outcome
            .transforms
            .into_iter()
            .map(|step| step.transform_id)
            .collect();
        Ok(Some(trace))
    }

    /// A stored clip without its content, and the text rules read from it.
    fn rule_subject_with_text(
        &self,
        clip_id: &str,
    ) -> Result<Option<(RuleSubject, Option<SubjectText>)>, StoreError> {
        let Some(subject) = self.store.rule_subject(clip_id)? else {
            return Ok(None);
        };
        let text = self
            .store
            .rule_subject_text(&self.keyring.item_key(), &subject)?;
        Ok(Some((subject, text)))
    }

    /// The attributes of a stored clip as rule conditions read them.
    fn subject_input<'a>(
        &self,
        subject: &'a RuleSubject,
        text: Option<&'a str>,
    ) -> EvaluationInput<'a> {
        let local = subject.origin_device_id.is_empty()
            || subject.origin_device_id == self.meta.device_id();
        EvaluationInput {
            text: text.or(subject.file_text.as_deref()),
            content_types: ContentTypes::from_content_type(&subject.content_type),
            byte_len: subject.byte_len,
            source_app: subject.app_bundle_id.as_deref(),
            markers: PrivacyMarkers::UNKNOWN,
            origin: if local {
                CaptureOrigin::LocalCopy
            } else {
                CaptureOrigin::Received
            },
            created_at: subject.created_at,
        }
    }

    /// The rules a test or preview evaluates: a draft or a saved rule alone,
    /// or in place within every live rule with `full_set`. The rule under
    /// test runs whether or not it is enabled or scoped to this device.
    fn rule_set(
        &self,
        definition_json: Option<&str>,
        rule_id: Option<&str>,
        full_set: bool,
    ) -> Result<RuleSet, Refusal> {
        let inputs = self.store.content_rule_inputs().map_err(Refusal::Store)?;
        let stored =
            rule_id.and_then(|rule_id| inputs.iter().find(|input| input.rule.id == rule_id));
        let target = match (definition_json, stored) {
            (Some(definition), _) => {
                if !compile::validate(definition).is_empty() {
                    return Err(Refusal::Invalid);
                }
                Some(RuleInput {
                    rule: ContentRuleRecord {
                        definition_json: definition.to_owned(),
                        ..stored.map_or_else(|| draft_record(rule_id), |input| input.rule.clone())
                    },
                    enabled: true,
                    order_key: stored.map_or_else(
                        || DRAFT_ORDER_KEY.to_owned(),
                        |input| input.order_key.clone(),
                    ),
                })
            }
            (None, Some(stored)) => Some(stored.clone()),
            (None, None) if rule_id.is_some() => return Err(Refusal::NotFound),
            (None, None) if !full_set => return Err(Refusal::NoRule),
            (None, None) => None,
        }
        .map(|mut target| {
            target.enabled = true;
            target.rule.device_scope.clear();
            target.rule.effective_from = 0;
            target
        });
        let mut selected: Vec<RuleInput> = if full_set {
            inputs
                .iter()
                .filter(|input| target.as_ref().is_none_or(|t| t.rule.id != input.rule.id))
                .cloned()
                .collect()
        } else {
            Vec::new()
        };
        selected.extend(target);
        let compiled = CompiledRuleSet::compile(selected.iter().cloned(), self.meta.device_id());
        Ok(RuleSet::new(Arc::new(compiled), &selected))
    }
}

fn draft_record(rule_id: Option<&str>) -> ContentRuleRecord {
    ContentRuleRecord {
        id: rule_id.unwrap_or(DRAFT_ID).to_owned(),
        origin: CoreOrigin::User,
        builtin_version: None,
        customized_fields: Vec::new(),
        name: String::new(),
        definition_json: String::new(),
        device_scope: Vec::new(),
        schema_version: SCHEMA_VERSION,
        effective_from: 0,
        deleted_at: None,
        base_stamp: None,
    }
}

/// Why a rule set could not be built.
enum Refusal {
    Invalid,
    NotFound,
    NoRule,
    Store(StoreError),
}

impl Refusal {
    fn response(self, id: u64) -> Response {
        match self {
            Self::Invalid => Response::err(id, ErrorCode::InvalidRequest, MSG_INVALID_DEFINITION),
            Self::NotFound => Response::err(id, ErrorCode::NotFound, MSG_RULE_NOT_FOUND),
            Self::NoRule => Response::err(id, ErrorCode::InvalidRequest, MSG_NO_RULE),
            Self::Store(error) => storage_error(id, "rule_set", &error),
        }
    }
}

/// A compiled set with what its trace names: each rule's name, stop flag
/// and action codes.
struct RuleSet {
    compiled: Arc<CompiledRuleSet>,
    rules: HashMap<String, (String, Option<RuleDefinition>)>,
}

impl RuleSet {
    fn new(compiled: Arc<CompiledRuleSet>, inputs: &[RuleInput]) -> Self {
        let rules = inputs
            .iter()
            .map(|input| {
                (
                    input.rule.id.clone(),
                    (
                        input.rule.name.clone(),
                        RuleDefinition::parse(&input.rule.definition_json).ok(),
                    ),
                )
            })
            .collect();
        Self { compiled, rules }
    }

    /// Whether rule `rule_id` has an action that `wanted` selects.
    fn rule_acts(&self, rule_id: &str, wanted: fn(&Action) -> bool) -> bool {
        self.rules
            .get(rule_id)
            .and_then(|(_, definition)| definition.as_ref())
            .is_some_and(|definition| definition.known_actions().any(wanted))
    }

    fn wire_trace(&self, trace: &core_trace::RuleTrace) -> RuleTrace {
        let steps = trace
            .rules
            .iter()
            .map(|entry| {
                let (name, definition) = self
                    .rules
                    .get(&entry.rule_id)
                    .map_or((String::new(), None), |(name, definition)| {
                        (name.clone(), definition.as_ref())
                    });
                let matched = entry.result == RuleResult::Matched;
                let actions: Vec<String> = definition
                    .filter(|_| matched)
                    .map(|definition| {
                        definition
                            .known_actions()
                            .map(|action| action.type_name().to_owned())
                            .collect()
                    })
                    .unwrap_or_default();
                let stopped = matched
                    && definition.is_some_and(|definition| {
                        definition.stop_processing
                            || definition
                                .known_actions()
                                .any(|action| matches!(action, Action::DontSave { .. }))
                    });
                TraceStep {
                    rule_id: entry.rule_id.clone(),
                    rule_name: name,
                    result: rule_result(entry.result),
                    conditions: entry
                        .conditions
                        .iter()
                        .map(|condition| {
                            let (result, unknown_reason) = condition_result(condition.result);
                            TraceCondition {
                                index: u32::try_from(condition.index).unwrap_or(u32::MAX),
                                result,
                                unknown_reason: unknown_reason.map(str::to_owned),
                            }
                        })
                        .collect(),
                    actions,
                    stopped,
                }
            })
            .collect();
        RuleTrace {
            steps,
            ..RuleTrace::default()
        }
    }
}

const fn rule_result(result: RuleResult) -> TraceResult {
    match result {
        RuleResult::Matched => TraceResult::Matched,
        RuleResult::NotMatched => TraceResult::NotMatched,
        RuleResult::NotEffective | RuleResult::NeedsUpdate => TraceResult::Skipped,
        RuleResult::FailedClosed | RuleResult::BudgetExceeded => TraceResult::Error,
    }
}

const fn condition_result(result: ConditionResult) -> (TraceResult, Option<&'static str>) {
    match result {
        ConditionResult::Matched => (TraceResult::Matched, None),
        ConditionResult::NotMatched => (TraceResult::NotMatched, None),
        ConditionResult::Unknown => (TraceResult::Unknown, Some("attribute_unknown")),
        ConditionResult::Indeterminate => (TraceResult::Error, Some("not_evaluable")),
    }
}

fn error_code(code: RuleErrorCode) -> String {
    serde_json::to_value(code)
        .ok()
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default()
}

fn wire_error(error: &compile::RuleError) -> RuleError {
    RuleError {
        path: error.path.clone(),
        offset: error.offset.and_then(|offset| u32::try_from(offset).ok()),
        message_code: error_code(error.code),
    }
}

const fn needs_update(code: RuleErrorCode) -> bool {
    matches!(
        code,
        RuleErrorCode::UnsupportedSchema
            | RuleErrorCode::UnknownConditionType
            | RuleErrorCode::UnknownActionType
    )
}

/// Whether a rule needs a newer CopyPaste, and its error badge. Rules this
/// device runs report their compiled health; the others are validated.
fn rule_health(compiled: &CompiledRuleSet, rule: &ContentRuleRecord) -> (bool, Option<String>) {
    if rule.deleted_at.is_some() {
        return (false, None);
    }
    if let Some(status) = compiled.status(&rule.id) {
        return match status.health {
            RuleHealth::Ready => (false, None),
            RuleHealth::NeedsUpdate => (true, None),
            RuleHealth::Broken => (false, status.errors.first().map(|e| error_code(e.code))),
        };
    }
    if rule.schema_version > SCHEMA_VERSION {
        return (true, None);
    }
    let errors = compile::validate(&rule.definition_json);
    if errors.iter().any(|error| needs_update(error.code)) {
        return (true, None);
    }
    (false, errors.first().map(|error| error_code(error.code)))
}

fn write_refusal(id: u64, operation: &'static str, error: &RuleWriteError) -> Response {
    let (code, message) = match error {
        RuleWriteError::NotFound => (ErrorCode::NotFound, MSG_RULE_NOT_FOUND),
        RuleWriteError::Invalid(_) => (ErrorCode::InvalidRequest, MSG_INVALID_DEFINITION),
        RuleWriteError::InvalidName => (ErrorCode::InvalidRequest, MSG_INVALID_NAME),
        RuleWriteError::InvalidScope => (ErrorCode::InvalidRequest, MSG_INVALID_SCOPE),
        RuleWriteError::LimitReached => (ErrorCode::InvalidRequest, MSG_LIMIT),
        RuleWriteError::NotBuiltin => (ErrorCode::InvalidRequest, MSG_NOT_BUILTIN),
        RuleWriteError::Store(error) => return storage_error(id, operation, error),
    };
    Response::err(id, code, message)
}

/// Previews and applies running in this process, keyed by runtime and
/// request id. A cancel that arrives before its request starts is kept, so
/// the request stops as soon as it begins.
static IN_FLIGHT: Mutex<Vec<InFlight>> = Mutex::new(Vec::new());
/// Early cancels kept per process.
const MAX_EARLY_CANCELS: usize = 16;

struct InFlight {
    runtime: usize,
    request_id: String,
    cancel: Arc<AtomicBool>,
    running: bool,
}

fn in_flight() -> MutexGuard<'static, Vec<InFlight>> {
    IN_FLIGHT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn runtime_key(runtime: &Runtime) -> usize {
    std::ptr::from_ref(runtime).addr()
}

/// A running request's cancel flag; dropping it unregisters the request.
struct Registration {
    cancel: Arc<AtomicBool>,
}

impl Registration {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        in_flight().retain(|flight| !Arc::ptr_eq(&flight.cancel, &self.cancel));
    }
}

fn register(runtime: &Runtime, request_id: &str) -> Registration {
    let key = runtime_key(runtime);
    let mut flights = in_flight();
    let early = flights
        .iter()
        .position(|f| !f.running && f.runtime == key && f.request_id == request_id);
    let cancel = Arc::new(AtomicBool::new(early.is_some()));
    if let Some(index) = early {
        flights.remove(index);
    }
    flights.push(InFlight {
        runtime: key,
        request_id: request_id.to_owned(),
        cancel: Arc::clone(&cancel),
        running: true,
    });
    Registration { cancel }
}

/// Tells the request `request_id` to stop, now or when it starts.
fn cancel(runtime: &Runtime, request_id: &str) {
    let key = runtime_key(runtime);
    let mut flights = in_flight();
    let mut found = false;
    for flight in flights
        .iter()
        .filter(|f| f.runtime == key && f.request_id == request_id)
    {
        flight.cancel.store(true, Ordering::Relaxed);
        found = true;
    }
    if !found {
        if flights.iter().filter(|f| !f.running).count() >= MAX_EARLY_CANCELS {
            if let Some(oldest) = flights.iter().position(|f| !f.running) {
                flights.remove(oldest);
            }
        }
        flights.push(InFlight {
            runtime: key,
            request_id: request_id.to_owned(),
            cancel: Arc::new(AtomicBool::new(true)),
            running: false,
        });
    }
}
