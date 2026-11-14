//! Retroactive rule previews and their plan tokens.
//!
//! A preview evaluates a rule, or the whole rule set, on the newest clips in
//! batches of 50, reading one clip's text at a time, and can be cancelled
//! between clips. It keeps at most one plan per runtime: clip ids and the
//! effects to apply, never content. A new preview, an apply, a cancel of the
//! request that made it, or a 10-minute deadline releases the plan.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use copypaste_core::now_ms;
use copypaste_core::rules::evaluate::{EvaluationContext, Expiry, RuleOutcome};
use copypaste_core::rules::model::Action;
use copypaste_core::storage::rules::{RuleSubject, SubjectCursor};
use copypaste_ipc::rules::{
    RuleOp, RulePreview, RulePreviewCounts, RulePreviewItem, MAX_RULE_PREVIEW_ITEMS,
};
use copypaste_ipc::{ErrorCode, Response, ResponseData};

use super::{register, runtime_key, RuleSet};
use crate::deadlines::DeadlineKey;
use crate::messages::storage_error;
use crate::Runtime;

/// Clips read per batch.
pub(super) const BATCH: usize = 50;
/// How long an unused plan is kept.
const PLAN_TTL_MS: i64 = 10 * 60 * 1000;
const DEADLINE_OWNER: &str = "rule_preview";
/// Bytes of text shown per previewed clip.
const PREVIEW_BYTES: usize = 160;

const MSG_CANCELLED: &str = "The preview was cancelled.";

/// What applying a plan does to one clip.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Planned {
    pub(super) id: String,
    /// "Don't save" applied to an existing clip deletes it.
    pub(super) delete: bool,
    pub(super) secret: bool,
    pub(super) hide_from_integrations: bool,
    pub(super) expiry: Option<Expiry>,
    pub(super) tag_ids: Vec<String>,
    pub(super) applied_rules: Vec<String>,
}

impl Planned {
    /// Pinned clips are never deleted or given an expiry by a rule. Only
    /// matched rules' own actions count: the protection a rule that could
    /// not be evaluated adds at capture is not applied to history.
    fn new(subject: &RuleSubject, outcome: RuleOutcome, set: &RuleSet) -> Self {
        let acted = |wanted: fn(&Action) -> bool| {
            outcome
                .applied_rules
                .iter()
                .any(|rule_id| set.rule_acts(rule_id, wanted))
        };
        let secret = outcome.secret && acted(|a| matches!(a, Action::MarkSecret {}));
        let hide_from_integrations = outcome.hide_from_integrations
            && acted(|a| matches!(a, Action::HideFromIntegrations {}));
        Self {
            id: subject.id.clone(),
            delete: outcome.dont_save.is_some() && !subject.pinned,
            secret: secret && !subject.secret,
            hide_from_integrations,
            expiry: outcome.expiry.filter(|_| !subject.pinned),
            tag_ids: outcome.tag_ids,
            applied_rules: outcome.applied_rules,
        }
    }

    fn is_noop(&self) -> bool {
        !self.delete
            && !self.secret
            && !self.hide_from_integrations
            && self.expiry.is_none()
            && self.tag_ids.is_empty()
    }
}

static PLANS: Mutex<Vec<Outstanding>> = Mutex::new(Vec::new());
static NEXT_TOKEN: AtomicU64 = AtomicU64::new(1);

struct Outstanding {
    runtime: usize,
    token: String,
    request_id: String,
    entries: Vec<Planned>,
}

fn plans() -> MutexGuard<'static, Vec<Outstanding>> {
    PLANS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn deadline(token: &str) -> DeadlineKey {
    DeadlineKey::new(DEADLINE_OWNER, token)
}

/// Drops this runtime's plans that `drop_it` selects.
fn release_where(runtime: &Runtime, drop_it: impl Fn(&Outstanding) -> bool) {
    let key = runtime_key(runtime);
    let mut released = Vec::new();
    plans().retain(|plan| {
        let gone = plan.runtime == key && drop_it(plan);
        if gone {
            released.push(plan.token.clone());
        }
        !gone
    });
    for token in released {
        runtime.deadlines.cancel(&deadline(&token));
    }
}

/// Drops the plan the request `request_id` made, if it is still held.
pub(super) fn release_request(runtime: &Runtime, request_id: &str) {
    release_where(runtime, |plan| plan.request_id == request_id);
}

/// Takes the plan `token` for applying; `None` once it was released.
pub(super) fn take(runtime: &Runtime, token: &str) -> Option<Vec<Planned>> {
    let key = runtime_key(runtime);
    let mut held = plans();
    let index = held
        .iter()
        .position(|plan| plan.runtime == key && plan.token == token)?;
    let plan = held.remove(index);
    drop(held);
    runtime.deadlines.cancel(&deadline(token));
    Some(plan.entries)
}

/// Holds `entries` as this runtime's one plan until its deadline.
fn keep(runtime: &Runtime, request_id: String, entries: Vec<Planned>) -> String {
    release_where(runtime, |_| true);
    let token = format!(
        "rules-{:x}-{:x}",
        now_ms(),
        NEXT_TOKEN.fetch_add(1, Ordering::Relaxed)
    );
    plans().push(Outstanding {
        runtime: runtime_key(runtime),
        token: token.clone(),
        request_id,
        entries,
    });
    let expiring = token.clone();
    runtime.deadlines.schedule(
        deadline(&token),
        runtime.deadlines.now().saturating_add(PLAN_TTL_MS),
        Box::new(move |runtime| {
            release_where(runtime, |plan| plan.token == expiring);
        }),
    );
    token
}

pub(super) fn handle(runtime: &Runtime, id: u64, op: RuleOp) -> Response {
    let RuleOp::Preview {
        request_id,
        definition_json,
        rule_id,
        full_set,
        limit,
    } = op
    else {
        return crate::features::unsupported(id);
    };
    let registration = register(runtime, &request_id);
    release_where(runtime, |_| true);
    let set = match runtime.rule_set(definition_json.as_deref(), rule_id.as_deref(), full_set) {
        Ok(set) => set,
        Err(refusal) => return refusal.response(id),
    };
    let limit = match limit {
        0 => MAX_RULE_PREVIEW_ITEMS,
        limit => limit.min(MAX_RULE_PREVIEW_ITEMS),
    } as usize;
    let key = runtime.keyring.item_key();
    let mut cursor: Option<SubjectCursor> = None;
    let mut examined = 0;
    let mut counts = RulePreviewCounts::default();
    let mut entries = Vec::new();
    let mut items = Vec::new();
    while examined < limit {
        let wanted = u32::try_from(BATCH.min(limit - examined)).unwrap_or(1);
        let batch = match runtime.store.rule_subjects(cursor.as_ref(), wanted) {
            Ok(batch) => batch,
            Err(error) => return storage_error(id, "rule_preview", &error),
        };
        if batch.is_empty() {
            break;
        }
        for subject in batch {
            if registration.cancelled() {
                tracing::debug!(examined, "rule preview cancelled");
                return Response::err(id, ErrorCode::NotFound, MSG_CANCELLED);
            }
            examined += 1;
            cursor = Some(SubjectCursor {
                created_at: subject.created_at,
                id: subject.id.clone(),
            });
            let text = runtime
                .store
                .rule_subject_text(&key, &subject)
                .unwrap_or_else(|error| {
                    tracing::warn!(?error, "a clip could not be read for a rule preview");
                    None
                });
            let text = text.as_ref().map(|text| text.as_str());
            let outcome = set.compiled.evaluate(
                &runtime.subject_input(&subject, text),
                EvaluationContext::Capture,
            );
            if outcome.applied_rules.is_empty() {
                continue;
            }
            let planned = Planned::new(&subject, outcome, &set);
            if planned.is_noop() {
                continue;
            }
            count(&mut counts, &planned);
            items.push(RulePreviewItem {
                id: subject.id.clone(),
                preview: if subject.secret {
                    String::new()
                } else {
                    preview_text(text.or(subject.file_text.as_deref()).unwrap_or_default())
                },
                masked: subject.secret,
                created_at: subject.created_at,
                source_app_name: subject.app_name.clone(),
            });
            entries.push(planned);
        }
    }
    let truncated = examined == limit
        && cursor.as_ref().is_some_and(|cursor| {
            runtime
                .store
                .rule_subjects(Some(cursor), 1)
                .is_ok_and(|more| !more.is_empty())
        });
    tracing::debug!(examined, matched = counts.matched, "rule preview planned");
    let token = keep(runtime, request_id, entries);
    Response::ok(
        id,
        ResponseData::RulePreview(RulePreview {
            token,
            counts,
            items,
            truncated,
        }),
    )
}

fn count(counts: &mut RulePreviewCounts, planned: &Planned) {
    counts.matched += 1;
    if planned.delete {
        counts.deleted += 1;
        return;
    }
    counts.secret += u32::from(planned.secret);
    counts.hidden_from_integrations += u32::from(planned.hide_from_integrations);
    counts.expiring += u32::from(planned.expiry.is_some());
    counts.tagged += u32::from(!planned.tag_ids.is_empty());
}

/// The start of `text` on one line, cut at a character boundary.
fn preview_text(text: &str) -> String {
    let text = text.trim_start();
    let mut cut = text.len().min(PREVIEW_BYTES);
    while !text.is_char_boundary(cut) {
        cut -= 1;
    }
    text[..cut].split_whitespace().collect::<Vec<_>>().join(" ")
}
