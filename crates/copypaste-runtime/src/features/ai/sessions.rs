//! MCP sessions, each bound to the `watch_events` connection that attached
//! it, so a client that exits or crashes frees its session.
//!
//! A session is opened on an ordinary request and must be attached within
//! ten seconds; an attached one ends with its connection or after thirty idle
//! minutes, both through the deadline scheduler. At the cap the least
//! recently used session is evicted. The state is memory only: attaching and
//! closing run on the daemon's reactor and touch no database.

use std::collections::HashMap;

use copypaste_core::storage::ai_view::new_ai_id;
use copypaste_core::storage::organization::AiGrantRecord;
use copypaste_ipc::ai::{
    AiApprovalKind, AiLookback, AiOp, AiScope, AiSession, AiSessionState, AiStatus,
};
use copypaste_ipc::{ErrorCode, Response, ResponseData};

use super::{approvals, grant, limits, lookback_of, scopes_of};
use crate::deadlines::DeadlineKey;
use crate::messages::storage_error;
use crate::Runtime;

pub(super) const MAX_SESSIONS: usize = 8;
pub(super) const ATTACH_TTL_MS: i64 = 10_000;
pub(super) const IDLE_TTL_MS: i64 = 30 * 60 * 1000;
const ATTACH_TIMER: &str = "ai_session_attach";
const IDLE_TIMER: &str = "ai_session_idle";
const EVENT_TIMER: &str = "ai_session_event";
const MAX_CLIENT_CHARS: usize = 128;
const MAX_VERSION_CHARS: usize = 64;
const MAX_EXECUTABLE_CHARS: usize = 1024;
const UNKNOWN_CLIENT: &str = "Unknown client";

const MSG_ATTACH_ON_WATCH: &str = "Attach a session on the watch_events connection that owns it.";
const MSG_NO_SESSION: &str = "That AI tool session has ended.";
const MSG_ATTACHED: &str = "That AI tool session is attached to another connection.";

/// Everything AI access keeps in memory, behind one lock.
#[derive(Default)]
pub(super) struct State {
    pub(super) sessions: HashMap<String, Session>,
    pub(super) approvals: approvals::Pending,
    pub(super) buckets: limits::Buckets,
}

pub(super) struct Session {
    pub(super) client_name: String,
    pub(super) client_version: Option<String>,
    pub(super) executable: Option<String>,
    /// The grant the session acts under, live or not.
    pub(super) grant_id: Option<String>,
    /// Access was asked for once (or refused as repeated); never again.
    prompted: bool,
    pub(super) denied: bool,
    pub(super) pending: Option<String>,
    connection: Option<u64>,
    last_used: i64,
    pub(super) read_bytes: u64,
    pub(super) allowance: u64,
    pub(super) volume_pending: Option<String>,
    pub(super) volume_denied: bool,
}

/// An authorized session's grant, read fresh for one call.
pub(super) struct Access {
    pub(super) session_id: String,
    pub(super) grant: AiGrantRecord,
    pub(super) scopes: Vec<AiScope>,
    pub(super) lookback: AiLookback,
}

/// Why a call cannot be served, in the session's terms.
pub(super) struct Blocked {
    pub(super) state: AiSessionState,
    pub(super) pending_request_id: Option<String>,
}

fn clean(text: &str, max_chars: usize) -> Option<String> {
    let text: String = text
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(max_chars)
        .collect();
    (!text.is_empty()).then_some(text)
}

pub(super) fn handle(runtime: &Runtime, id: u64, op: AiOp) -> Response {
    match op {
        AiOp::Status { session_id } => Response::ok(
            id,
            ResponseData::AiStatus(status(runtime, session_id.as_deref())),
        ),
        AiOp::SessionOpen {
            credential,
            client_name,
            client_version,
            executable,
        } => open(
            runtime,
            id,
            credential.as_deref(),
            &client_name,
            client_version.as_deref(),
            executable.as_deref(),
        ),
        AiOp::SessionAttach { .. } => {
            Response::err(id, ErrorCode::InvalidRequest, MSG_ATTACH_ON_WATCH)
        }
        AiOp::SessionClose { session_id } => {
            end(runtime, &[session_id]);
            Response::ok(id, ResponseData::Empty {})
        }
        _ => crate::features::unsupported(id),
    }
}

fn open(
    runtime: &Runtime,
    id: u64,
    credential: Option<&str>,
    client_name: &str,
    client_version: Option<&str>,
    executable: Option<&str>,
) -> Response {
    let grant_id = match credential.filter(|credential| !credential.trim().is_empty()) {
        Some(credential) => match runtime.store.ai_grant_for_credential(credential) {
            Ok(found) => found,
            Err(error) => return storage_error(id, "ai_session_open", &error),
        },
        None => None,
    };
    let now = runtime.deadlines.now();
    let session_id = new_ai_id();
    let evicted = {
        let mut state = runtime.ai.lock();
        let evicted = least_recently_used(&state);
        state.sessions.insert(
            session_id.clone(),
            Session {
                client_name: clean(client_name, MAX_CLIENT_CHARS)
                    .unwrap_or_else(|| UNKNOWN_CLIENT.to_owned()),
                client_version: client_version.and_then(|text| clean(text, MAX_VERSION_CHARS)),
                executable: executable.and_then(|text| clean(text, MAX_EXECUTABLE_CHARS)),
                prompted: grant_id.is_some(),
                grant_id,
                denied: false,
                pending: None,
                connection: None,
                last_used: now,
                read_bytes: 0,
                allowance: limits::SESSION_READ_ALLOWANCE_BYTES,
                volume_pending: None,
                volume_denied: false,
            },
        );
        evicted
    };
    end(runtime, &evicted);
    let expiring = session_id.clone();
    runtime.deadlines.schedule(
        DeadlineKey::new(ATTACH_TIMER, session_id.clone()),
        now + ATTACH_TTL_MS,
        Box::new(move |runtime: &Runtime| {
            let unattached = runtime
                .ai
                .lock()
                .sessions
                .get(&expiring)
                .is_some_and(|session| session.connection.is_none());
            if unattached {
                end(runtime, &[expiring]);
            }
        }),
    );
    let pending_request_id = prompt(runtime, &session_id);
    let authorized_scopes = match evaluate(runtime, &session_id) {
        Ok(access) => Some(access.scopes),
        Err(_) => None,
    };
    Response::ok(
        id,
        ResponseData::AiSession(AiSession {
            session_id,
            authorized_scopes,
            pending_request_id,
        }),
    )
}

/// The sessions to evict so one more fits: the least recently used one,
/// preferring a session that is not waiting for an answer.
fn least_recently_used(state: &State) -> Vec<String> {
    if state.sessions.len() < MAX_SESSIONS {
        return Vec::new();
    }
    state
        .sessions
        .iter()
        .min_by_key(|(_, session)| (session.pending.is_some(), session.last_used))
        .map(|(id, _)| vec![id.clone()])
        .unwrap_or_default()
}

/// Asks the desktop for access once per session, while AI tools are on and
/// the session has no grant. Answers the pending request.
fn prompt(runtime: &Runtime, session_id: &str) -> Option<String> {
    if !runtime.settings.get().mcp_enabled {
        return None;
    }
    let now = runtime.deadlines.now();
    let mut state = runtime.ai.lock();
    let session = state.sessions.get_mut(session_id)?;
    if session.prompted {
        return session.pending.clone();
    }
    session.prompted = true;
    let client = session.client_name.clone();
    if state.approvals.blocked(&client, now) {
        if let Some(session) = state.sessions.get_mut(session_id) {
            session.denied = true;
        }
        return None;
    }
    let request = approvals::open(
        runtime,
        &mut state,
        session_id,
        AiApprovalKind::Access,
        None,
    )?;
    if let Some(session) = state.sessions.get_mut(session_id) {
        session.pending = Some(request.clone());
    }
    drop(state);
    runtime.note_ai_approval();
    Some(request)
}

/// The session's grant if it may call tools now, read fresh.
fn evaluate(runtime: &Runtime, session_id: &str) -> Result<Access, Blocked> {
    let blocked = |state, pending_request_id| Blocked {
        state,
        pending_request_id,
    };
    let (grant_id, pending) = {
        let state = runtime.ai.lock();
        let session = state
            .sessions
            .get(session_id)
            .ok_or_else(|| blocked(AiSessionState::Expired, None))?;
        (session.grant_id.clone(), session.pending.clone())
    };
    if !runtime.settings.get().mcp_enabled {
        return Err(blocked(AiSessionState::Disabled, None));
    }
    let Some(grant_id) = grant_id else {
        return Err(match pending {
            Some(request) => blocked(AiSessionState::PendingApproval, Some(request)),
            None => blocked(AiSessionState::Unauthorized, None),
        });
    };
    match grant(runtime, &grant_id) {
        Ok(Some(grant)) if !grant.revoked && grant.owner_device == runtime.meta.device_id() => {
            Ok(Access {
                session_id: session_id.to_owned(),
                scopes: scopes_of(&grant),
                lookback: lookback_of(&grant),
                grant,
            })
        }
        Ok(_) => Err(blocked(AiSessionState::Revoked, None)),
        Err(error) => {
            tracing::warn!(?error, "an AI tool grant could not be read");
            Err(blocked(AiSessionState::Unauthorized, None))
        }
    }
}

/// Authorizes one tool call and marks the session used.
pub(super) fn authorize(runtime: &Runtime, session_id: &str) -> Result<Access, Blocked> {
    prompt(runtime, session_id);
    let access = evaluate(runtime, session_id)?;
    if let Some(session) = runtime.ai.lock().sessions.get_mut(session_id) {
        session.last_used = runtime.deadlines.now();
    }
    Ok(access)
}

pub(super) fn status(runtime: &Runtime, session_id: Option<&str>) -> AiStatus {
    let mcp_enabled = runtime.settings.get().mcp_enabled;
    let contributing_devices = contributing_devices(runtime);
    let Some(session_id) = session_id else {
        return AiStatus {
            mcp_enabled,
            session: None,
            scopes: Vec::new(),
            lookback: None,
            contributing_devices,
            next_step: (!mcp_enabled).then(|| "enable_ai_tools".to_owned()),
        };
    };
    prompt(runtime, session_id);
    let (state, scopes, lookback) = match evaluate(runtime, session_id) {
        Ok(access) => (
            AiSessionState::Authorized,
            access.scopes,
            Some(access.lookback),
        ),
        Err(blocked) => (blocked.state, Vec::new(), None),
    };
    let denied = runtime
        .ai
        .lock()
        .sessions
        .get(session_id)
        .is_some_and(|session| session.denied);
    let next_step = match state {
        AiSessionState::Authorized => None,
        AiSessionState::Disabled => Some("enable_ai_tools"),
        AiSessionState::PendingApproval => Some("approve_in_app"),
        AiSessionState::Unauthorized if denied => Some("access_denied"),
        AiSessionState::Unauthorized => Some("open_app_to_approve"),
        AiSessionState::Revoked => Some("connect_again"),
        AiSessionState::Expired => Some("restart_client"),
        AiSessionState::AtCapacity => Some("close_other_sessions"),
    };
    AiStatus {
        mcp_enabled,
        session: Some(state),
        scopes,
        lookback,
        contributing_devices,
        next_step: next_step.map(str::to_owned),
    }
}

/// Devices whose history the policy lets AI tools see.
fn contributing_devices(runtime: &Runtime) -> u32 {
    let policy = match super::policy(runtime) {
        Ok(policy) => policy,
        Err(error) => {
            tracing::warn!(?error, "the AI tool policy could not be read");
            return 0;
        }
    };
    let local = runtime.meta.device_id();
    let facets = runtime.store.history_facets(
        local,
        &runtime.meta.device_name(),
        runtime.meta.device_class(),
    );
    facets.map_or(0, |facets| {
        let count = facets
            .origin_devices
            .iter()
            .filter(|device| {
                (policy.include_other_devices || device.id == local)
                    && !policy.excluded_device_ids.contains(&device.id)
            })
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    })
}

/// Blocks a content call once the session read its allowance, asking the
/// desktop for more the first time.
pub(super) fn volume_gate(runtime: &Runtime, access: &Access) -> Result<(), Blocked> {
    let mut state = runtime.ai.lock();
    let Some(session) = state.sessions.get(&access.session_id) else {
        return Err(Blocked {
            state: AiSessionState::Expired,
            pending_request_id: None,
        });
    };
    if session.volume_denied {
        return Err(Blocked {
            state: AiSessionState::Authorized,
            pending_request_id: None,
        });
    }
    if session.read_bytes < session.allowance {
        return Ok(());
    }
    if let Some(pending) = session.volume_pending.clone() {
        return Err(Blocked {
            state: AiSessionState::Authorized,
            pending_request_id: Some(pending),
        });
    }
    let request = approvals::open(
        runtime,
        &mut state,
        &access.session_id,
        AiApprovalKind::Volume,
        Some(access.grant.name.clone()),
    );
    if let Some(session) = state.sessions.get_mut(&access.session_id) {
        session.volume_pending.clone_from(&request);
    }
    drop(state);
    runtime.note_ai_approval();
    Err(Blocked {
        state: AiSessionState::Authorized,
        pending_request_id: request,
    })
}

/// Counts content a call returned against the session's allowance.
pub(super) fn account(runtime: &Runtime, session_id: &str, bytes: u64) {
    if let Some(session) = runtime.ai.lock().sessions.get_mut(session_id) {
        session.read_bytes = session.read_bytes.saturating_add(bytes);
    }
}

pub(super) fn attach(runtime: &Runtime, id: u64, connection: u64, session_id: &str) -> Response {
    let now = runtime.deadlines.now();
    {
        let mut state = runtime.ai.lock();
        let Some(session) = state.sessions.get_mut(session_id) else {
            return Response::err(id, ErrorCode::NotFound, MSG_NO_SESSION);
        };
        match session.connection {
            Some(owner) if owner != connection => {
                return Response::err(id, ErrorCode::InvalidRequest, MSG_ATTACHED)
            }
            _ => {}
        }
        session.connection = Some(connection);
        session.last_used = now;
    }
    runtime
        .deadlines
        .cancel(&DeadlineKey::new(ATTACH_TIMER, session_id.to_owned()));
    arm_idle(runtime, session_id, now + IDLE_TTL_MS);
    Response::ok(id, ResponseData::Empty {})
}

/// Ends the session after thirty quiet minutes; a call since the timer was
/// armed moves it to thirty minutes after that call instead.
fn arm_idle(runtime: &Runtime, session_id: &str, at: i64) {
    let idle = session_id.to_owned();
    runtime.deadlines.schedule(
        DeadlineKey::new(IDLE_TIMER, session_id.to_owned()),
        at,
        Box::new(move |runtime: &Runtime| {
            let now = runtime.deadlines.now();
            let last_used = runtime
                .ai
                .lock()
                .sessions
                .get(&idle)
                .map(|session| session.last_used);
            match last_used {
                Some(last) if last + IDLE_TTL_MS > now => {
                    arm_idle(runtime, &idle, last + IDLE_TTL_MS);
                }
                Some(_) => end(runtime, &[idle]),
                None => {}
            }
        }),
    );
}

pub(super) fn connection_closed(runtime: &Runtime, connection: u64) {
    let owned: Vec<String> = runtime
        .ai
        .lock()
        .sessions
        .iter()
        .filter(|(_, session)| session.connection == Some(connection))
        .map(|(id, _)| id.clone())
        .collect();
    if owned.is_empty() {
        return;
    }
    if remove(runtime, &owned) {
        // The approval event reads the store, which the reactor that reports
        // a closed connection must not; the deadline worker sends it.
        runtime.deadlines.schedule(
            DeadlineKey::new(EVENT_TIMER, connection.to_string()),
            runtime.deadlines.now(),
            Box::new(|runtime: &Runtime| runtime.note_ai_approval()),
        );
    }
}

/// Ends sessions and announces any approval that went with them.
fn end(runtime: &Runtime, session_ids: &[String]) {
    if remove(runtime, session_ids) {
        runtime.note_ai_approval();
    }
}

/// Drops sessions with their timers and pending requests. Answers whether a
/// pending request was dropped.
fn remove(runtime: &Runtime, session_ids: &[String]) -> bool {
    let mut dropped = Vec::new();
    {
        let mut state = runtime.ai.lock();
        for session_id in session_ids {
            state.sessions.remove(session_id);
            dropped.extend(state.approvals.drop_session(session_id));
        }
    }
    for session_id in session_ids {
        for timer in [ATTACH_TIMER, IDLE_TIMER] {
            runtime
                .deadlines
                .cancel(&DeadlineKey::new(timer, session_id.clone()));
        }
    }
    approvals::cancel(runtime, &dropped);
    !dropped.is_empty()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use copypaste_ipc::{ConfigPatch, Method};

    use super::*;

    fn enabled() -> (Arc<Runtime>, tempfile::TempDir) {
        let (runtime, dir) = crate::tests::fixture();
        let response = runtime.request_blocking(
            1,
            Method::SetConfig {
                patch: ConfigPatch {
                    mcp_enabled: Some(true),
                    ..Default::default()
                },
            },
        );
        assert!(response.ok, "{:?}", response.error);
        (runtime, dir)
    }

    fn open_session(runtime: &Runtime) -> String {
        match open(runtime, 1, None, "client", None, None).data {
            Some(ResponseData::AiSession(session)) => session.session_id,
            other => panic!("unexpected {other:?}"),
        }
    }

    fn state_of(runtime: &Runtime, session_id: &str) -> AiSessionState {
        status(runtime, Some(session_id)).session.unwrap()
    }

    #[test]
    fn sessions_end_unattached_idle_or_with_their_connection_and_lru_frees_a_slot() {
        let (runtime, _dir) = enabled();
        let unattached = open_session(&runtime);
        let idle = open_session(&runtime);
        let closed = open_session(&runtime);
        assert!(runtime.attach_ai_session(1, 7, &idle).ok);
        assert!(runtime.attach_ai_session(1, 9, &closed).ok);
        assert_eq!(state_of(&runtime, &closed), AiSessionState::PendingApproval);

        runtime.on_watch_closed(9);
        assert_eq!(state_of(&runtime, &closed), AiSessionState::Expired);
        assert!(runtime.ai.lock().approvals.drop_session(&closed).is_empty());

        runtime.deadlines.advance(ATTACH_TTL_MS + 1);
        runtime.sweep_deadlines();
        assert_eq!(state_of(&runtime, &unattached), AiSessionState::Expired);
        assert_eq!(state_of(&runtime, &idle), AiSessionState::PendingApproval);
        runtime.deadlines.advance(IDLE_TTL_MS);
        runtime.sweep_deadlines();
        assert_eq!(state_of(&runtime, &idle), AiSessionState::Expired);

        let first = open_session(&runtime);
        assert!(runtime.attach_ai_session(1, 99, &first).ok);
        runtime.deadlines.advance(1);
        let rest: Vec<String> = (1..MAX_SESSIONS).map(|_| open_session(&runtime)).collect();
        for (connection, session) in rest.iter().enumerate() {
            assert!(
                runtime
                    .attach_ai_session(1, 100 + connection as u64, session)
                    .ok
            );
        }
        assert_eq!(runtime.ai.lock().sessions.len(), MAX_SESSIONS);
        let ninth = open_session(&runtime);
        assert_eq!(runtime.ai.lock().sessions.len(), MAX_SESSIONS);
        assert_eq!(state_of(&runtime, &first), AiSessionState::Expired);
        assert_ne!(state_of(&runtime, &ninth), AiSessionState::Expired);
    }

    #[test]
    fn attaching_on_an_ordinary_request_is_refused() {
        let (runtime, _dir) = enabled();
        let session_id = open_session(&runtime);
        let response = handle(&runtime, 1, AiOp::SessionAttach { session_id });
        assert_eq!(response.error_code, Some(ErrorCode::InvalidRequest));
        assert_eq!(
            runtime.attach_ai_session(1, 1, "missing").error_code,
            Some(ErrorCode::NotFound)
        );
    }
}
