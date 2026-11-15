//! Applying a rule preview to existing clips, with progress and cancel.
//!
//! Apply runs on this device only and writes ordinary edits that sync:
//! "Don't save" deletes the clip, a secret or an expiry is set as the user
//! would set it, an integration hide writes the clip's note, and tags stay local
//! rule tags. It stops before the next clip once cancelled. Progress goes
//! out as coalesced `Progress` events and is kept for `ApplyProgress`.

use std::sync::{Mutex, MutexGuard};

use copypaste_core::now_ms;
use copypaste_core::StoreError;
use copypaste_ipc::clips::ClipOp;
use copypaste_ipc::rules::{RuleApplyReport, RuleOp};
use copypaste_ipc::{ErrorCode, OperationProgress, Response, ResponseData};

use super::preview::{self, Planned, BATCH};
use super::{cancel, register, runtime_key};
use crate::Runtime;

const MSG_PLAN_GONE: &str = "This preview has ended. Preview the rule again.";

/// The latest apply's progress per runtime, for callers that cannot
/// subscribe to events.
static PROGRESS: Mutex<Vec<(usize, OperationProgress)>> = Mutex::new(Vec::new());

fn progress() -> MutexGuard<'static, Vec<(usize, OperationProgress)>> {
    PROGRESS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

pub(super) fn handle(runtime: &Runtime, id: u64, op: RuleOp) -> Response {
    match op {
        RuleOp::Apply {
            preview_token,
            request_id,
        } => apply(runtime, id, &preview_token, request_id),
        RuleOp::ApplyProgress { request_id } => {
            let key = runtime_key(runtime);
            let found = progress()
                .iter()
                .find(|(runtime, progress)| *runtime == key && progress.request_id == request_id)
                .map(|(_, progress)| progress.clone());
            Response::ok(id, ResponseData::OperationProgress(found))
        }
        RuleOp::Cancel { request_id } => {
            cancel(runtime, &request_id);
            preview::release_request(runtime, &request_id);
            Response::ok(id, ResponseData::Empty {})
        }
        _ => crate::features::unsupported(id),
    }
}

fn report_progress(runtime: &Runtime, request_id: &str, done: usize, total: usize) {
    let update = OperationProgress {
        request_id: request_id.to_owned(),
        done: done as u64,
        total: total as u64,
    };
    let key = runtime_key(runtime);
    {
        let mut held = progress();
        held.retain(|(runtime, _)| *runtime != key);
        held.push((key, update.clone()));
    }
    runtime.note_progress(update);
}

fn apply(runtime: &Runtime, id: u64, token: &str, request_id: String) -> Response {
    let registration = register(runtime, &request_id);
    let Some(entries) = preview::take(runtime, token) else {
        return Response::err(id, ErrorCode::NotFound, MSG_PLAN_GONE);
    };
    let started = now_ms();
    let total = entries.len();
    let mut report = RuleApplyReport::default();
    let mut done = 0;
    report_progress(runtime, &request_id, done, total);
    'batches: for batch in entries.chunks(BATCH) {
        let mut hide = Vec::new();
        for planned in batch {
            if registration.cancelled() {
                report.cancelled = true;
                hide_from_integrations(runtime, id, hide, &mut report);
                break 'batches;
            }
            match apply_one(runtime, planned) {
                Ok(Applied::Deleted) => report.deleted += 1,
                Ok(Applied::Changed) => report.changed += 1,
                Ok(Applied::Gone) => {}
                Err(error) => {
                    tracing::warn!(?error, "a rule could not be applied to a clip");
                    report.failed += 1;
                }
            }
            if planned.hide_from_integrations && !planned.delete {
                hide.push(planned.id.clone());
            }
            done += 1;
        }
        hide_from_integrations(runtime, id, hide, &mut report);
        report_progress(runtime, &request_id, done, total);
    }
    if report.changed + report.deleted > 0 {
        runtime.note_local_change_since(started);
    }
    tracing::debug!(
        changed = report.changed,
        deleted = report.deleted,
        failed = report.failed,
        cancelled = report.cancelled,
        "rules applied to existing clips"
    );
    Response::ok(id, ResponseData::RuleApplyReport(report))
}

enum Applied {
    Deleted,
    Changed,
    /// Deleted since the preview, or now pinned and only to be deleted.
    Gone,
}

fn apply_one(runtime: &Runtime, planned: &Planned) -> Result<Applied, StoreError> {
    let Some(subject) = runtime.store.rule_subject(&planned.id)? else {
        return Ok(Applied::Gone);
    };
    if planned.delete {
        if subject.pinned {
            return Ok(Applied::Gone);
        }
        runtime.store.delete(&planned.id)?;
        return Ok(Applied::Deleted);
    }
    if planned.secret && !subject.secret {
        runtime
            .store
            .set_secret(&runtime.keyring, &planned.id, true)?;
    }
    if let Some(expiry) = planned.expiry.filter(|_| !subject.pinned) {
        runtime.set_clip_expiry(&planned.id, expiry.duration_ms, expiry.clear_clipboard)?;
    }
    if !planned.tag_ids.is_empty() {
        runtime.store.add_rule_tags(&planned.id, &planned.tag_ids)?;
    }
    runtime
        .store
        .note_applied_rules(&planned.id, &planned.applied_rules)?;
    Ok(Applied::Changed)
}

/// Hides a batch of clips from integrations through their synced notes.
fn hide_from_integrations(
    runtime: &Runtime,
    id: u64,
    ids: Vec<String>,
    report: &mut RuleApplyReport,
) {
    if ids.is_empty() {
        return;
    }
    let count = u32::try_from(ids.len()).unwrap_or(u32::MAX);
    let response = runtime.clips(id, ClipOp::SetHiddenFromIntegrations { ids, hidden: true });
    if !response.ok {
        tracing::warn!("clips could not be hidden from integrations by a rule");
        report.failed = report.failed.saturating_add(count);
    }
}
