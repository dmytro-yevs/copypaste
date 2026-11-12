//! First-use and volume approvals, answered only in the CopyPaste app.
//!
//! A request times out to Deny after two minutes through the deadline
//! scheduler. Three denials of one reported client within ten minutes deny
//! that client's further requests for ten minutes, so a looping client
//! cannot flood the desktop with prompts. Approving access creates a grant
//! (or updates the one an earlier approval of the same client created), so
//! the session shows up under Connected tools and can be revoked there.

use std::collections::HashMap;

use copypaste_core::storage::organization::AiGrantRecord;
use copypaste_ipc::ai::{AiApprovalKind, AiApprovalRequest, AiLookback, AiOp, AiScope};
use copypaste_ipc::{ErrorCode, Response, ResponseData};

use super::sessions::State;
use super::{issue_grant, limits, lookback_name, scope_names, write_grant};
use crate::deadlines::DeadlineKey;
use crate::messages::storage_error;
use crate::Runtime;

pub(super) const APPROVAL_TTL_MS: i64 = 2 * 60 * 1000;
const DENIAL_WINDOW_MS: i64 = 10 * 60 * 1000;
const DENIALS_TO_BLOCK: usize = 3;
/// Pending requests kept; a new one past this denies the oldest.
const MAX_PENDING: usize = 16;
/// Clients whose recent denials are remembered.
const MAX_DENIED_CLIENTS: usize = 64;
const TIMER: &str = "ai_approval";

const MSG_NO_REQUEST: &str = "That request is no longer waiting for an answer.";
const MSG_WRONG_KIND: &str = "That answer does not fit this request.";

struct Request {
    wire: AiApprovalRequest,
    session_id: String,
}

#[derive(Default)]
pub(super) struct Pending {
    requests: Vec<Request>,
    /// Denial instants per reported client name.
    denials: HashMap<String, Vec<i64>>,
}

impl Pending {
    /// Whether `client` was denied often enough recently to be refused.
    pub(super) fn blocked(&mut self, client: &str, now: i64) -> bool {
        let Some(times) = self.denials.get_mut(client) else {
            return false;
        };
        times.retain(|at| now.saturating_sub(*at) < DENIAL_WINDOW_MS);
        if times.is_empty() {
            self.denials.remove(client);
            return false;
        }
        times.len() >= DENIALS_TO_BLOCK
    }

    fn record_denial(&mut self, client: &str, now: i64) {
        if !self.denials.contains_key(client) && self.denials.len() >= MAX_DENIED_CLIENTS {
            self.denials
                .retain(|_, times| times.iter().any(|at| now - at < DENIAL_WINDOW_MS));
            if self.denials.len() >= MAX_DENIED_CLIENTS {
                return;
            }
        }
        let times = self.denials.entry(client.to_owned()).or_default();
        times.push(now);
        if times.len() > DENIALS_TO_BLOCK {
            times.remove(0);
        }
    }

    /// Drops the requests of a session that ended; answers their ids.
    pub(super) fn drop_session(&mut self, session_id: &str) -> Vec<String> {
        let mut dropped = Vec::new();
        self.requests.retain(|request| {
            let keep = request.session_id != session_id;
            if !keep {
                dropped.push(request.wire.request_id.clone());
            }
            keep
        });
        dropped
    }

    fn take(&mut self, request_id: &str) -> Option<Request> {
        let at = self
            .requests
            .iter()
            .position(|request| request.wire.request_id == request_id)?;
        Some(self.requests.remove(at))
    }
}

/// Opens a request for `session_id` and arms its timeout. Answers its id,
/// or `None` when the session is gone.
pub(super) fn open(
    runtime: &Runtime,
    state: &mut State,
    session_id: &str,
    kind: AiApprovalKind,
    grant_name: Option<String>,
) -> Option<String> {
    let now = runtime.deadlines.now();
    let session = state.sessions.get(session_id)?;
    let request_id = copypaste_core::storage::ai_view::new_ai_id();
    let wire = AiApprovalRequest {
        request_id: request_id.clone(),
        kind,
        client_name: session.client_name.clone(),
        client_version: session.client_version.clone(),
        executable: session.executable.clone(),
        grant_name,
        requested_at: now,
        expires_at: now + APPROVAL_TTL_MS,
        read_bytes: (kind == AiApprovalKind::Volume).then_some(session.read_bytes),
    };
    if state.approvals.requests.len() >= MAX_PENDING {
        let oldest = state.approvals.requests.remove(0);
        runtime
            .deadlines
            .cancel(&DeadlineKey::new(TIMER, oldest.wire.request_id.clone()));
        refuse(state, &oldest, now);
    }
    state.approvals.requests.push(Request {
        wire,
        session_id: session_id.to_owned(),
    });
    let expiring = request_id.clone();
    runtime.deadlines.schedule(
        DeadlineKey::new(TIMER, request_id.clone()),
        now + APPROVAL_TTL_MS,
        Box::new(move |runtime: &Runtime| {
            if resolve(runtime, &expiring, Decision::Deny).is_ok() {
                tracing::debug!("an AI tool approval timed out");
            }
        }),
    );
    Some(request_id)
}

/// Cancels the timers of requests that were dropped with their session.
pub(super) fn cancel(runtime: &Runtime, request_ids: &[String]) {
    for request_id in request_ids {
        runtime
            .deadlines
            .cancel(&DeadlineKey::new(TIMER, request_id.clone()));
    }
}

/// Applies a denial to the request's session.
fn refuse(state: &mut State, request: &Request, now: i64) {
    if let Some(session) = state.sessions.get_mut(&request.session_id) {
        match request.wire.kind {
            AiApprovalKind::Access => {
                session.pending = None;
                session.denied = true;
            }
            AiApprovalKind::Volume => {
                session.volume_pending = None;
                session.volume_denied = true;
            }
        }
    }
    if request.wire.kind == AiApprovalKind::Access {
        state
            .approvals
            .record_denial(&request.wire.client_name, now);
    }
}

enum Decision {
    Deny,
    Allow,
    Grant(Vec<AiScope>, AiLookback),
}

#[derive(Debug)]
enum Unresolved {
    Missing,
    WrongKind,
    Storage(copypaste_core::StoreError),
}

pub(super) fn handle(runtime: &Runtime, id: u64, op: AiOp) -> Response {
    let outcome = match op {
        AiOp::PendingApprovals => {
            let state = runtime.ai.lock();
            let requests = state
                .approvals
                .requests
                .iter()
                .map(|request| request.wire.clone())
                .collect();
            return Response::ok(id, ResponseData::AiApprovals(requests));
        }
        AiOp::Approve {
            request_id,
            scopes,
            lookback,
        } => resolve(runtime, &request_id, Decision::Grant(scopes, lookback)),
        AiOp::Deny { request_id } => resolve(runtime, &request_id, Decision::Deny),
        AiOp::VolumeDecision { request_id, allow } => resolve(
            runtime,
            &request_id,
            if allow {
                Decision::Allow
            } else {
                Decision::Deny
            },
        ),
        _ => return crate::features::unsupported(id),
    };
    match outcome {
        Ok(()) => Response::ok(id, ResponseData::Empty {}),
        Err(Unresolved::Missing) => Response::err(id, ErrorCode::NotFound, MSG_NO_REQUEST),
        Err(Unresolved::WrongKind) => Response::err(id, ErrorCode::InvalidRequest, MSG_WRONG_KIND),
        Err(Unresolved::Storage(error)) => storage_error(id, "ai_approval", &error),
    }
}

fn resolve(runtime: &Runtime, request_id: &str, decision: Decision) -> Result<(), Unresolved> {
    let now = runtime.deadlines.now();
    let request = {
        let mut state = runtime.ai.lock();
        let kind = state
            .approvals
            .requests
            .iter()
            .find(|request| request.wire.request_id == request_id)
            .ok_or(Unresolved::Missing)?
            .wire
            .kind;
        match (&decision, kind) {
            (Decision::Grant(..), AiApprovalKind::Volume)
            | (Decision::Allow, AiApprovalKind::Access) => return Err(Unresolved::WrongKind),
            _ => {}
        }
        state
            .approvals
            .take(request_id)
            .ok_or(Unresolved::Missing)?
    };
    runtime
        .deadlines
        .cancel(&DeadlineKey::new(TIMER, request_id.to_owned()));
    let outcome = match decision {
        Decision::Grant(scopes, lookback) => {
            let granted = approved_grant(runtime, &request.wire, &scopes, lookback);
            let mut state = runtime.ai.lock();
            if let Some(session) = state.sessions.get_mut(&request.session_id) {
                session.pending = None;
                if let Ok(grant_id) = &granted {
                    session.grant_id = Some(grant_id.clone());
                }
            }
            granted.map(drop).map_err(Unresolved::Storage)
        }
        Decision::Allow => {
            let mut state = runtime.ai.lock();
            if let Some(session) = state.sessions.get_mut(&request.session_id) {
                session.volume_pending = None;
                session.allowance = session
                    .read_bytes
                    .saturating_add(limits::SESSION_READ_ALLOWANCE_BYTES);
            }
            Ok(())
        }
        Decision::Deny => {
            refuse(&mut runtime.ai.lock(), &request, now);
            Ok(())
        }
    };
    runtime.note_ai_approval();
    outcome
}

/// The grant an approval authorizes: the one an earlier approval of the
/// same reported client created, updated, or a new one.
fn approved_grant(
    runtime: &Runtime,
    request: &AiApprovalRequest,
    scopes: &[AiScope],
    lookback: AiLookback,
) -> Result<String, copypaste_core::StoreError> {
    let local = runtime.meta.device_id();
    let earlier = runtime
        .store
        .ai_grants()?
        .into_iter()
        .map(|(grant, _)| grant)
        .find(|grant: &AiGrantRecord| {
            grant.owner_device == local
                && !grant.revoked
                && grant.client_name == request.client_name
                && grant.client_executable == request.executable
        });
    if let Some(mut grant) = earlier {
        grant.scopes = scope_names(scopes);
        grant.lookback = lookback_name(lookback).to_owned();
        let id = grant.id.clone();
        write_grant(runtime, grant)?;
        return Ok(id);
    }
    let (grant, _credential) = issue_grant(
        runtime,
        request.client_name.clone(),
        request.client_name.clone(),
        request.executable.clone(),
        scopes,
        lookback,
    )?;
    Ok(grant.id)
}
