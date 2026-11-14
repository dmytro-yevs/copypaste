//! Evaluating compiled rules against one item.
//!
//! Single pass over the original capture (R-11, R-12): conditions never see
//! transformed content, protection is sticky, the shortest expiry wins, tags
//! are a union and transforms chain in rule order. At capture every action is
//! returned; on receipt only protection actions and tags are, and only rules
//! whose `effective_from` is not after the item's creation run. The core never
//! executes a transform: it returns the id and parameters to the runtime.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::compile::{origin_bit, CompiledRule, CompiledRuleSet, Mode, Test};
use super::detect::{detect, prefix, DetectedKind, Detection};
use super::model::{
    Action, CaptureOrigin, Comparison, ContentTypeKind, MatchMode, PrivacyMarker, SizeUnit,
};
use super::trace::{
    ConditionResult, ConditionTrace, RuleResult, RuleTrace, RuleTraceEntry, MAX_TRACE_MATCHES,
};

/// Text examined by content conditions (R-19).
pub const TEXT_PREFIX_BYTES: usize = 1024 * 1024;
/// Wall-clock budget for one evaluation (R-19).
pub const EVALUATION_BUDGET: Duration = Duration::from_millis(50);

pub(crate) const fn bit(index: u8) -> u8 {
    1 << index
}

/// Content types present in a capture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentTypes(u8);

impl ContentTypes {
    /// Map a wire content type. Color and link are added from detection.
    #[must_use]
    pub fn from_content_type(content_type: &str) -> Self {
        use copypaste_ipc::content_type::{classify, ContentClass, TEXT};

        let kind = match classify(content_type) {
            ContentClass::Text
                if content_type == TEXT
                    || content_type.eq_ignore_ascii_case("text/plain")
                    || content_type.to_ascii_lowercase().starts_with("text/plain;") =>
            {
                Some(ContentTypeKind::PlainText)
            }
            ContentClass::Text => Some(ContentTypeKind::RichText),
            ContentClass::Image => Some(ContentTypeKind::Image),
            ContentClass::File => Some(ContentTypeKind::Files),
            ContentClass::Other => None,
        };
        kind.into_iter().collect()
    }

    #[must_use]
    pub const fn with(self, kind: ContentTypeKind) -> Self {
        Self(self.0 | bit(kind as u8))
    }

    #[must_use]
    pub const fn contains(self, kind: ContentTypeKind) -> bool {
        self.0 & bit(kind as u8) != 0
    }

    const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    const fn needs_detection(self) -> bool {
        self.contains(ContentTypeKind::Color) || self.contains(ContentTypeKind::Link)
    }
}

impl FromIterator<ContentTypeKind> for ContentTypes {
    fn from_iter<I: IntoIterator<Item = ContentTypeKind>>(iter: I) -> Self {
        iter.into_iter().fold(Self::default(), Self::with)
    }
}

/// OS privacy markers, each known present, known absent or unknown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PrivacyMarkers {
    known: u8,
    present: u8,
}

impl PrivacyMarkers {
    /// No marker is known: every marker condition evaluates to unknown.
    pub const UNKNOWN: Self = Self {
        known: 0,
        present: 0,
    };

    #[must_use]
    pub const fn with(self, marker: PrivacyMarker, present: bool) -> Self {
        let flag = bit(marker as u8);
        Self {
            known: self.known | flag,
            present: if present {
                self.present | flag
            } else {
                self.present & !flag
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationContext {
    /// A new capture on this device, before persistence.
    Capture,
    /// A version received through sync, in the apply transaction.
    Receipt,
}

/// The attributes one evaluation reads. `None` means unknown (R-5).
#[derive(Debug, Clone, Copy)]
pub struct EvaluationInput<'a> {
    /// Plain-text representation; `None` for content without text.
    pub text: Option<&'a str>,
    pub content_types: ContentTypes,
    pub byte_len: u64,
    pub source_app: Option<&'a str>,
    pub markers: PrivacyMarkers,
    pub origin: CaptureOrigin,
    /// Wall-clock ms of the item's creation, compared with `effective_from`.
    pub created_at: i64,
}

impl<'a> EvaluationInput<'a> {
    /// Plain text copied on this device with no other known attributes.
    #[must_use]
    pub fn text(text: &'a str, created_at: i64) -> Self {
        Self {
            text: Some(text),
            content_types: ContentTypes::default().with(ContentTypeKind::PlainText),
            byte_len: text.len() as u64,
            source_app: None,
            markers: PrivacyMarkers::UNKNOWN,
            origin: CaptureOrigin::LocalCopy,
            created_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationLimits {
    pub budget: Duration,
    pub text_prefix_bytes: usize,
}

impl Default for EvaluationLimits {
    fn default() -> Self {
        Self {
            budget: EVALUATION_BUDGET,
            text_prefix_bytes: TEXT_PREFIX_BYTES,
        }
    }
}

/// Stable bits stored with an item (`clip_local_state.eval_flags`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EvalFlags(u32);

impl EvalFlags {
    /// A rule could not be evaluated; the item was protected instead.
    pub const RULES_INCOMPLETE: Self = Self(1);
    pub const BUDGET_EXCEEDED: Self = Self(1 << 1);
    /// Content conditions saw only the text prefix.
    pub const PARTIAL_TEXT: Self = Self(1 << 2);
    /// Detection saw only its prefix.
    pub const PARTIAL_DETECTION: Self = Self(1 << 3);
    /// A rule written by a newer version was skipped here.
    pub const NEEDS_UPDATE: Self = Self(1 << 4);

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: Self) {
        self.0 |= other.0;
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DontSave {
    pub rule_id: String,
    pub clear_clipboard_after_secs: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expiry {
    pub duration_ms: i64,
    /// Clear the system clipboard at expiry if it still holds the item.
    pub clear_clipboard: bool,
}

/// One transform for the runtime to run, in chain order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransformStep {
    pub rule_id: String,
    pub transform_id: String,
    pub params: Map<String, Value>,
    pub replace_clipboard: bool,
}

/// The combined actions of every matched rule.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RuleOutcome {
    /// Terminal when set: nothing else applies.
    pub dont_save: Option<DontSave>,
    pub secret: bool,
    pub hide_from_integrations: bool,
    pub expiry: Option<Expiry>,
    pub tag_ids: Vec<String>,
    pub transforms: Vec<TransformStep>,
    /// Ids of rules whose actions were applied, in order.
    pub applied_rules: Vec<String>,
    pub flags: EvalFlags,
}

/// The protection actions alone: what a receiving device may apply.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ProtectionOutcome {
    pub secret: bool,
    pub hide_from_integrations: bool,
    pub expiry: Option<Expiry>,
    pub flags: EvalFlags,
}

impl RuleOutcome {
    #[must_use]
    pub fn protection_subset(&self) -> ProtectionOutcome {
        ProtectionOutcome {
            secret: self.secret,
            hide_from_integrations: self.hide_from_integrations,
            expiry: self.expiry,
            flags: self.flags,
        }
    }

    /// True when no rule changed anything about the item.
    #[must_use]
    pub fn is_noop(&self) -> bool {
        self.dont_save.is_none()
            && !self.secret
            && !self.hide_from_integrations
            && self.expiry.is_none()
            && self.tag_ids.is_empty()
            && self.transforms.is_empty()
    }

    fn fail_closed(&mut self, flags: EvalFlags) {
        self.secret = true;
        self.hide_from_integrations = true;
        self.flags.insert(EvalFlags::RULES_INCOMPLETE);
        self.flags.insert(flags);
    }

    /// Returns true when the action ends evaluation.
    fn apply(&mut self, rule_id: &str, action: &Action, context: EvaluationContext) -> bool {
        let capture = context == EvaluationContext::Capture;
        match action {
            Action::DontSave {
                clear_clipboard_after_secs,
            } if capture => {
                self.dont_save = Some(DontSave {
                    rule_id: rule_id.to_owned(),
                    clear_clipboard_after_secs: *clear_clipboard_after_secs,
                });
                return true;
            }
            Action::MarkSecret {} => self.secret = true,
            Action::HideFromIntegrations {} => self.hide_from_integrations = true,
            Action::DeleteAfter {
                duration_ms,
                clear_clipboard,
            } => {
                let clear = *clear_clipboard || self.expiry.is_some_and(|e| e.clear_clipboard);
                let duration_ms = self
                    .expiry
                    .map_or(*duration_ms, |e| e.duration_ms.min(*duration_ms));
                self.expiry = Some(Expiry {
                    duration_ms,
                    clear_clipboard: clear,
                });
            }
            Action::AddTags { tag_ids } => {
                for tag in tag_ids {
                    if !self.tag_ids.contains(tag) {
                        self.tag_ids.push(tag.clone());
                    }
                }
            }
            Action::Transform {
                transform_id,
                params,
                replace_clipboard,
            } if capture => self.transforms.push(TransformStep {
                rule_id: rule_id.to_owned(),
                transform_id: transform_id.clone(),
                params: params.clone(),
                replace_clipboard: *replace_clipboard,
            }),
            Action::DontSave { .. } | Action::Transform { .. } => {}
        }
        false
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Evaluation {
    pub outcome: RuleOutcome,
    pub trace: Option<RuleTrace>,
}

/// Lazily derived attributes, computed at most once per evaluation.
struct Subject<'a> {
    input: &'a EvaluationInput<'a>,
    prefix_bytes: usize,
    text: Option<Option<&'a str>>,
    detection: Option<Option<Detection>>,
    characters: Option<u64>,
    lines: Option<u64>,
    flags: EvalFlags,
}

impl<'a> Subject<'a> {
    fn text(&mut self) -> Option<&'a str> {
        if self.text.is_none() {
            let limited = self.input.text.map(|text| {
                let (head, cut) = prefix(text, self.prefix_bytes);
                if cut {
                    self.flags.insert(EvalFlags::PARTIAL_TEXT);
                }
                head
            });
            self.text = Some(limited);
        }
        self.text.flatten()
    }

    fn detection(&mut self) -> Option<Detection> {
        if self.detection.is_none() {
            let detection = self.input.text.map(detect);
            if detection.is_some_and(|d| d.partial) {
                self.flags.insert(EvalFlags::PARTIAL_DETECTION);
            }
            self.detection = Some(detection);
        }
        self.detection.flatten()
    }

    fn content_types(&mut self, wanted: ContentTypes) -> ContentTypes {
        let mut types = self.input.content_types;
        if wanted.needs_detection() {
            if let Some(detection) = self.detection() {
                if detection.kinds.contains(DetectedKind::Color) {
                    types = types.with(ContentTypeKind::Color);
                }
                if detection.kinds.contains(DetectedKind::Url) {
                    types = types.with(ContentTypeKind::Link);
                }
            }
        }
        types
    }

    fn size(&mut self, unit: SizeUnit) -> Option<u64> {
        let text = self.input.text;
        match unit {
            SizeUnit::Bytes => Some(self.input.byte_len),
            SizeUnit::Characters => {
                let text = text?;
                Some(
                    *self
                        .characters
                        .get_or_insert_with(|| text.chars().count() as u64),
                )
            }
            SizeUnit::Lines => {
                let text = text?;
                Some(
                    *self
                        .lines
                        .get_or_insert_with(|| text.lines().count() as u64),
                )
            }
        }
    }
}

fn from_bool(matched: bool) -> ConditionResult {
    if matched {
        ConditionResult::Matched
    } else {
        ConditionResult::NotMatched
    }
}

fn evaluate_test(
    test: &Test,
    subject: &mut Subject<'_>,
    matches: Option<&mut Vec<(usize, usize)>>,
) -> ConditionResult {
    match test {
        Test::Text(regex) => {
            let Some(text) = subject.text() else {
                return ConditionResult::Unknown;
            };
            if let Some(matches) = matches {
                matches.extend(
                    regex
                        .find_iter(text)
                        .take(MAX_TRACE_MATCHES)
                        .map(|found| (found.start(), found.end())),
                );
                from_bool(!matches.is_empty())
            } else {
                from_bool(regex.is_match(text))
            }
        }
        Test::ContentType(wanted) => from_bool(subject.content_types(*wanted).intersects(*wanted)),
        Test::Detected(kinds) => from_bool(
            subject
                .detection()
                .is_some_and(|detection| detection.kinds.intersects(*kinds)),
        ),
        Test::SourceApp(apps) => subject
            .input
            .source_app
            .map_or(ConditionResult::Unknown, |app| {
                from_bool(
                    apps.iter()
                        .any(|candidate| candidate.eq_ignore_ascii_case(app)),
                )
            }),
        Test::Size { unit, op, value } => {
            subject
                .size(*unit)
                .map_or(ConditionResult::Unknown, |size| {
                    from_bool(match op {
                        Comparison::GreaterThan => size > *value,
                        Comparison::LessThan => size < *value,
                    })
                })
        }
        Test::Marker(wanted) => {
            let markers = subject.input.markers;
            if markers.present & markers.known & wanted != 0 {
                ConditionResult::Matched
            } else if markers.known & wanted == *wanted {
                ConditionResult::NotMatched
            } else {
                ConditionResult::Unknown
            }
        }
        Test::Origin(wanted) => from_bool(origin_bit(subject.input.origin) & wanted != 0),
        Test::Indeterminate => ConditionResult::Indeterminate,
    }
}

const fn negate(result: ConditionResult) -> ConditionResult {
    match result {
        ConditionResult::Matched => ConditionResult::NotMatched,
        ConditionResult::NotMatched => ConditionResult::Matched,
        other => other,
    }
}

enum Verdict {
    Matched,
    NotMatched,
    /// An indeterminate condition decides the result.
    Maybe,
    OutOfBudget,
}

fn evaluate_rule(
    rule: &CompiledRule,
    subject: &mut Subject<'_>,
    deadline: Instant,
    trace: Option<&mut Vec<ConditionTrace>>,
) -> Verdict {
    let tracing = trace.is_some();
    let mut traces = Vec::new();
    let (mut any_matched, mut any_failed, mut any_indeterminate) = (false, false, false);
    for (index, condition) in rule.conditions.iter().enumerate() {
        if Instant::now() > deadline {
            return Verdict::OutOfBudget;
        }
        let mut matches = Vec::new();
        let raw = evaluate_test(&condition.test, subject, tracing.then_some(&mut matches));
        let result = if condition.negate { negate(raw) } else { raw };
        if tracing {
            if result != ConditionResult::Matched {
                matches.clear();
            }
            traces.push(ConditionTrace {
                index,
                result,
                matches,
            });
        }
        match result {
            ConditionResult::Matched => any_matched = true,
            ConditionResult::NotMatched | ConditionResult::Unknown => any_failed = true,
            ConditionResult::Indeterminate => any_indeterminate = true,
        }
        let decided = match rule.match_mode {
            MatchMode::All => any_failed,
            MatchMode::Any => any_matched,
        };
        if decided && !tracing {
            break;
        }
    }
    if let Some(trace) = trace {
        *trace = traces;
    }
    let matched = match rule.match_mode {
        MatchMode::All => !any_failed,
        MatchMode::Any => any_matched || rule.conditions.is_empty(),
    };
    let decided_by_indeterminate = match rule.match_mode {
        MatchMode::All => !any_failed && any_indeterminate,
        MatchMode::Any => !any_matched && any_indeterminate,
    };
    if decided_by_indeterminate {
        Verdict::Maybe
    } else if matched {
        Verdict::Matched
    } else {
        Verdict::NotMatched
    }
}

impl CompiledRuleSet {
    #[must_use]
    pub fn evaluate(&self, input: &EvaluationInput<'_>, context: EvaluationContext) -> RuleOutcome {
        self.evaluate_with(input, context, EvaluationLimits::default(), false)
            .outcome
    }

    /// Evaluate and explain, for "Why…" and the rule tester.
    #[must_use]
    pub fn evaluate_traced(
        &self,
        input: &EvaluationInput<'_>,
        context: EvaluationContext,
    ) -> Evaluation {
        self.evaluate_with(input, context, EvaluationLimits::default(), true)
    }

    #[must_use]
    pub fn evaluate_with(
        &self,
        input: &EvaluationInput<'_>,
        context: EvaluationContext,
        limits: EvaluationLimits,
        trace: bool,
    ) -> Evaluation {
        let deadline = Instant::now() + limits.budget;
        let mut subject = Subject {
            input,
            prefix_bytes: limits.text_prefix_bytes,
            text: None,
            detection: None,
            characters: None,
            lines: None,
            flags: EvalFlags::default(),
        };
        let mut outcome = RuleOutcome::default();
        let mut entries = Vec::new();
        let mut record = |rule: &CompiledRule, result, conditions| {
            if trace {
                entries.push(RuleTraceEntry {
                    rule_id: rule.id.clone(),
                    result,
                    conditions,
                });
            }
        };
        for rule in &self.rules {
            if rule.mode == Mode::Inactive {
                outcome.flags.insert(EvalFlags::NEEDS_UPDATE);
                record(rule, RuleResult::NeedsUpdate, Vec::new());
                continue;
            }
            if context == EvaluationContext::Receipt && input.created_at < rule.effective_from {
                record(rule, RuleResult::NotEffective, Vec::new());
                continue;
            }
            let mut conditions = Vec::new();
            let verdict = evaluate_rule(
                rule,
                &mut subject,
                deadline,
                trace.then_some(&mut conditions),
            );
            match (verdict, rule.mode) {
                (Verdict::OutOfBudget, _) => {
                    outcome.fail_closed(EvalFlags::BUDGET_EXCEEDED);
                    record(rule, RuleResult::BudgetExceeded, conditions);
                    break;
                }
                (Verdict::NotMatched, _) => record(rule, RuleResult::NotMatched, conditions),
                (Verdict::Maybe, _) | (Verdict::Matched, Mode::FailClosed | Mode::Inactive) => {
                    outcome.fail_closed(EvalFlags::default());
                    record(rule, RuleResult::FailedClosed, conditions);
                }
                (Verdict::Matched, Mode::Normal) => {
                    outcome.applied_rules.push(rule.id.clone());
                    record(rule, RuleResult::Matched, conditions);
                    let terminal = rule
                        .actions
                        .iter()
                        .any(|action| outcome.apply(&rule.id, action, context));
                    if terminal || rule.stop_processing {
                        break;
                    }
                }
            }
        }
        outcome.flags.insert(subject.flags);
        let trace = trace.then(|| RuleTrace {
            rules: entries,
            detected_kinds: subject
                .detection
                .flatten()
                .map(|detection| detection.kinds.iter().collect()),
            flags: outcome.flags.bits(),
        });
        Evaluation { outcome, trace }
    }
}
