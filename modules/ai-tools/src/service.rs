//! Connections, approvals and authorization. Each endpoint connection is
//! one MCP session whose state lives here until the host closes it.
//!
//! A connection from a client this device remembers is authorized at once.
//! Any other connection asks the user once, through a host prompt on a
//! worker thread; "Always allow" remembers the client. Every call re-reads
//! its grant, so a revocation from any device applies on the next call.
//! Three denials of one client within ten minutes stop further prompts for
//! it for ten minutes.
use crate::access_log::{AccessLog, Entry};
use crate::calls::{ToolCall, ToolResult};
use crate::grants::{self, Grant, Scope, ALL_SCOPES, READ_SCOPES};
use crate::host::{self, Host};
use crate::limits::{
    Buckets, DENIALS_TO_BLOCK, DENIAL_WINDOW_MS, MAX_CLIENT_CHARS, MAX_DENIED_CLIENTS, MAX_GRANTS,
    PROMPT_TIMEOUT_SECS, SESSION_READ_ALLOWANCE_BYTES,
};
use crate::policy::{Lookback, Policy};
use crate::strings::{fill, text, Text};
use crate::tools::{self, now_ms, Answer, Call, Failure};
use copypaste_module_sdk::{
    DeviceName, HistoryFacets, HostError, HostErrorCode, ModuleHostRequest, PromptAnswer,
    PromptChoice, PromptRequest, MAX_PENDING_PROMPTS,
};
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;
use tokio::sync::watch;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;
const PREFERENCES_KEY: &str = "preferences";
const SETTLE_TIMEOUT: Duration = Duration::from_secs(3);
const CHOICE_DENY: &str = "deny";
const CHOICE_ONCE: &str = "once";
const CHOICE_ALWAYS: &str = "always";
const CHOICE_MORE: &str = "more";
const MAX_PROMPT_TITLE_CHARS: usize = 160;

/// Tells the client its tool and prompt lists changed.
pub(crate) type Notify = Arc<dyn Fn() + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum State {
    Unauthorized,
    PendingApproval,
    Authorized,
    Revoked,
    Expired,
}

impl State {
    fn name(self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::PendingApproval => "pending_approval",
            Self::Authorized => "authorized",
            Self::Revoked => "revoked",
            Self::Expired => "expired",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Volume {
    Open,
    Asking,
    Declined,
}

struct Session {
    client: Option<String>,
    grant: Option<String>,
    state: State,
    /// Access was asked for once; never again in this session.
    prompted: bool,
    denied: bool,
    scopes: Vec<Scope>,
    read_bytes: u64,
    allowance: u64,
    volume: Volume,
    notify: Option<Notify>,
    settled: watch::Sender<bool>,
}

impl Session {
    fn new() -> Self {
        Self {
            client: None,
            grant: None,
            state: State::Unauthorized,
            prompted: false,
            denied: false,
            scopes: Vec::new(),
            read_bytes: 0,
            allowance: SESSION_READ_ALLOWANCE_BYTES,
            volume: Volume::Open,
            notify: None,
            settled: watch::Sender::new(false),
        }
    }

    /// Records the authorization; answers whether the client must be told.
    fn authorize(&mut self, state: State, grant: Option<String>, scopes: Vec<Scope>) -> bool {
        let changed = self.scopes != scopes;
        self.state = state;
        self.grant = grant;
        self.scopes = scopes;
        changed
    }
}

#[derive(Default)]
struct Inner {
    sessions: HashMap<String, Session>,
    buckets: Buckets,
    denials: HashMap<String, Vec<i64>>,
    prompts: usize,
}

impl Inner {
    fn throttled(&mut self, client: &str, now: i64) -> bool {
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
}

/// An authorized call's grant, read fresh.
struct Access {
    grant: Grant,
    scopes: Vec<Scope>,
}

/// Why a call cannot be served.
enum Blocked {
    State(State, bool),
    VolumePending,
    VolumeDeclined,
    Busy,
}

impl Blocked {
    fn result(&self) -> ToolResult {
        let (state, message) = match self {
            Self::State(State::PendingApproval, _) => (
                State::PendingApproval,
                "Waiting for approval in the CopyPaste app on this computer.",
            ),
            Self::State(State::Unauthorized, true) => (
                State::Unauthorized,
                "Access was denied in CopyPaste. Restart the AI tool to ask again.",
            ),
            Self::State(State::Unauthorized | State::Authorized, _) => (
                State::Unauthorized,
                "This AI tool is not authorized. Restart it and approve it in CopyPaste on \
                 this computer.",
            ),
            Self::State(State::Revoked, _) => (
                State::Revoked,
                "This connection was revoked in CopyPaste. Restart the AI tool to ask for \
                 access again.",
            ),
            Self::State(State::Expired, _) => (
                State::Expired,
                "This session has ended. Restart the AI tool to reconnect.",
            ),
            Self::VolumePending => (
                State::Authorized,
                "This session has read a lot of clipboard history. Confirm in the CopyPaste \
                 app on this computer, then retry.",
            ),
            Self::VolumeDeclined => (
                State::Authorized,
                "CopyPaste declined further reads in this session.",
            ),
            Self::Busy => (
                State::Authorized,
                "CopyPaste is waiting for other answers. Retry in a moment.",
            ),
        };
        ToolResult {
            is_error: true,
            text: message.to_owned(),
            image: None,
            structured: json!({
                "state": state.name(),
                "pending": matches!(self, Self::VolumePending).then_some("volume"),
            }),
            retry_after_secs: matches!(self, Self::Busy).then_some(5),
        }
    }
}

pub(crate) struct Service {
    host: Arc<dyn Host>,
    inner: Mutex<Inner>,
    log: Mutex<AccessLog>,
    device: Mutex<Option<DeviceName>>,
    preferences: Mutex<Option<BTreeMap<String, Value>>>,
    runtime: Mutex<Option<tokio::runtime::Handle>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
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

impl Service {
    pub(crate) fn new(host: Arc<dyn Host>) -> Self {
        Self {
            host,
            inner: Mutex::default(),
            log: Mutex::default(),
            device: Mutex::default(),
            preferences: Mutex::default(),
            runtime: Mutex::default(),
        }
    }

    pub(crate) fn host(&self) -> &dyn Host {
        &*self.host
    }

    pub(crate) fn host_arc(&self) -> Arc<dyn Host> {
        Arc::clone(&self.host)
    }

    /// The runtime whose blocking pool runs prompts; `None` once the last
    /// connection closed.
    pub(crate) fn set_runtime(&self, runtime: Option<tokio::runtime::Handle>) {
        *lock(&self.runtime) = runtime;
    }

    fn spawn(&self, job: impl FnOnce() + Send + 'static) -> bool {
        match lock(&self.runtime).as_ref() {
            Some(runtime) => {
                drop(runtime.spawn_blocking(job));
                true
            }
            None => false,
        }
    }

    /// Keeps the preference values of the latest command, the only place a
    /// host delivers them today, for endpoint calls in between.
    pub(crate) fn remember_preferences(&self, values: &BTreeMap<String, Value>) {
        let mut cached = lock(&self.preferences);
        if cached.as_ref() == Some(values) {
            return;
        }
        *cached = Some(values.clone());
        if let Ok(json) = serde_json::to_string(values) {
            let _ = host::write_state(&*self.host, PREFERENCES_KEY, json);
        }
    }

    /// The UI locale and the exposure policy for one call.
    pub(crate) fn context(&self) -> (String, Policy) {
        let context = host::context(&*self.host);
        let locale = context
            .as_ref()
            .map_or_else(|| "en".to_owned(), |context| context.locale.clone());
        let mut cached = lock(&self.preferences);
        if let Some(values) = context.and_then(|context| context.preferences) {
            *cached = Some(values);
        }
        if cached.is_none() {
            *cached = host::read_state(&*self.host, PREFERENCES_KEY)
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str(&json).ok());
        }
        let policy = Policy::from_preferences(cached.as_ref().unwrap_or(&BTreeMap::new()));
        (locale, policy)
    }

    pub(crate) fn locale(&self) -> String {
        host::context(&*self.host).map_or_else(|| "en".to_owned(), |context| context.locale)
    }

    pub(crate) fn local_device(&self) -> Result<DeviceName, HostError> {
        if let Some(device) = lock(&self.device).clone() {
            return Ok(device);
        }
        let devices: Vec<DeviceName> = host::request(&*self.host, ModuleHostRequest::DeviceNames)?;
        let device = devices
            .into_iter()
            .find(|device| device.local)
            .ok_or_else(|| HostError::new(HostErrorCode::Unavailable, "No local device."))?;
        *lock(&self.device) = Some(device.clone());
        Ok(device)
    }

    pub(crate) fn open(&self, connection: &str) {
        lock(&self.inner)
            .sessions
            .insert(connection.to_owned(), Session::new());
    }

    pub(crate) fn is_open(&self, connection: &str) -> bool {
        lock(&self.inner).sessions.contains_key(connection)
    }

    pub(crate) fn idle(&self) -> bool {
        lock(&self.inner).sessions.is_empty()
    }

    /// Ends a connection's session and frees what it held.
    pub(crate) fn close(&self, connection: &str) {
        let idle = {
            let mut inner = lock(&self.inner);
            inner.sessions.remove(connection);
            inner.sessions.is_empty()
        };
        if idle {
            lock(&self.log).release(&*self.host);
            *lock(&self.device) = None;
        }
    }

    pub(crate) fn identify(&self, connection: &str, name: &str, notify: Notify) {
        if let Some(session) = lock(&self.inner).sessions.get_mut(connection) {
            session.client =
                Some(clean(name, MAX_CLIENT_CHARS).unwrap_or_else(|| "Unknown client".to_owned()));
            session.notify = Some(notify);
        }
    }

    pub(crate) fn scopes(&self, connection: &str) -> Vec<Scope> {
        lock(&self.inner)
            .sessions
            .get(connection)
            .map(|session| session.scopes.clone())
            .unwrap_or_default()
    }

    /// Waits briefly until the first authorization check finished, so a
    /// remembered client sees its tools in the first list.
    pub(crate) async fn settled(&self, connection: &str) {
        let receiver = lock(&self.inner)
            .sessions
            .get(connection)
            .map(|session| session.settled.subscribe());
        if let Some(mut receiver) = receiver {
            let _ = tokio::time::timeout(SETTLE_TIMEOUT, receiver.wait_for(|done| *done)).await;
        }
    }

    fn with_session<T>(
        &self,
        connection: &str,
        edit: impl FnOnce(&mut Session) -> T,
    ) -> Option<(T, Option<Notify>)> {
        let mut inner = lock(&self.inner);
        let session = inner.sessions.get_mut(connection)?;
        let result = edit(session);
        Some((result, session.notify.clone()))
    }

    /// Authorizes a remembered client, then asks the user once for any other
    /// one. Runs on a worker thread: the prompt blocks it.
    pub(crate) fn settle(&self, connection: &str) {
        let bound = self.bind_remembered(connection);
        let open = self
            .with_session(connection, |session| {
                session.settled.send_replace(true);
            })
            .is_some();
        if open && !bound {
            self.ask_access(connection);
        }
    }

    /// A session whose first prompt could not be shown asks again.
    fn ask_again(self: &Arc<Self>, connection: &str) -> bool {
        let waiting = lock(&self.inner)
            .sessions
            .get(connection)
            .is_some_and(|session| {
                session.grant.is_none()
                    && !session.prompted
                    && session.client.is_some()
                    && *session.settled.borrow()
            });
        let service = Arc::clone(self);
        let owned = connection.to_owned();
        waiting && self.spawn(move || service.ask_access(&owned))
    }

    /// Binds the session to a grant this device remembers for its client.
    fn bind_remembered(&self, connection: &str) -> bool {
        let Some(client) = lock(&self.inner)
            .sessions
            .get(connection)
            .and_then(|session| session.client.clone())
        else {
            return false;
        };
        let Ok(device) = self.local_device() else {
            return false;
        };
        let (Ok(grants), Ok(approvals)) =
            (grants::list(&*self.host), grants::approvals(&*self.host))
        else {
            return false;
        };
        let Some(grant) = grants.into_iter().find(|grant| {
            grant.remember
                && !grant.revoked()
                && grant.owner_device == device.id
                && grant.client_name == client
                && approvals.contains_key(&grant.id)
        }) else {
            return false;
        };
        let scopes = grant.scopes();
        self.update(connection, |session| {
            session.authorize(State::Authorized, Some(grant.id), scopes)
        });
        true
    }

    /// Applies `edit` and tells the client when its lists changed.
    fn update(&self, connection: &str, edit: impl FnOnce(&mut Session) -> bool) {
        if let Some((true, Some(notify))) = self.with_session(connection, edit) {
            notify();
        }
    }

    fn ask_access(&self, connection: &str) {
        let now = now_ms();
        let client = {
            let mut inner = lock(&self.inner);
            let at_capacity = inner.prompts >= MAX_PENDING_PROMPTS;
            let Some(client) = inner
                .sessions
                .get(connection)
                .filter(|session| !session.prompted && session.grant.is_none())
                .and_then(|session| session.client.clone())
            else {
                return;
            };
            if inner.throttled(&client, now) {
                if let Some(session) = inner.sessions.get_mut(connection) {
                    session.prompted = true;
                    session.denied = true;
                }
                return;
            }
            if at_capacity {
                return;
            }
            inner.prompts += 1;
            if let Some(session) = inner.sessions.get_mut(connection) {
                session.prompted = true;
                session.state = State::PendingApproval;
            }
            client
        };
        let locale = self.locale();
        let answer = self.prompt(PromptRequest {
            title: fill(text(&locale, Text::ApprovalTitle), &[("client", &client)])
                .chars()
                .take(MAX_PROMPT_TITLE_CHARS)
                .collect(),
            body: text(&locale, Text::ApprovalNote).to_owned(),
            choices: vec![
                choice(CHOICE_DENY, text(&locale, Text::Deny), true),
                choice(CHOICE_ONCE, text(&locale, Text::AllowOnce), false),
                choice(CHOICE_ALWAYS, text(&locale, Text::AlwaysAllow), false),
            ],
            timeout_secs: PROMPT_TIMEOUT_SECS,
        });
        let remember = match answer.as_ref().map(|answer| answer.choice.as_deref()) {
            Ok(Some(CHOICE_ONCE)) => Some(false),
            Ok(Some(CHOICE_ALWAYS)) => Some(true),
            _ => None,
        };
        let granted = remember.map(|remember| self.approved_grant(&client, remember, now));
        match (answer, granted) {
            (_, Some(Ok(grant))) => {
                let scopes = grant.scopes();
                self.update(connection, |session| {
                    session.authorize(State::Authorized, Some(grant.id), scopes)
                });
            }
            (Ok(_), None) => {
                let mut inner = lock(&self.inner);
                inner.record_denial(&client, now);
                if let Some(session) = inner.sessions.get_mut(connection) {
                    session.state = State::Unauthorized;
                    session.denied = true;
                }
            }
            (Err(error), _) => self.update(connection, |session| {
                session.state = State::Unauthorized;
                // A full prompt queue is retried by the next call.
                session.prompted = error.code != HostErrorCode::Busy;
                false
            }),
            (Ok(_), Some(Err(_))) => self.update(connection, |session| {
                session.state = State::Unauthorized;
                false
            }),
        }
    }

    fn prompt(&self, request: PromptRequest) -> Result<PromptAnswer, HostError> {
        let answer = host::request(&*self.host, ModuleHostRequest::Prompt(request));
        let mut inner = lock(&self.inner);
        inner.prompts = inner.prompts.saturating_sub(1);
        answer
    }

    /// The grant an approval authorizes: the one an earlier approval of the
    /// same client on this device created, or a new read-only one.
    fn approved_grant(&self, client: &str, remember: bool, now: i64) -> Result<Grant, HostError> {
        let device = self.local_device()?;
        let mut existing = grants::list(&*self.host)?;
        let earlier = existing.iter().position(|grant| {
            !grant.revoked() && grant.owner_device == device.id && grant.client_name == client
        });
        let grant = match earlier {
            Some(index) => {
                let mut grant = existing.swap_remove(index);
                if remember && !grant.remember {
                    grant.remember = true;
                    grants::put(&*self.host, &grant)?;
                }
                grant
            }
            None => {
                if existing.len() >= MAX_GRANTS {
                    let oldest_revoked = existing.iter().find(|grant| grant.revoked());
                    match oldest_revoked {
                        Some(grant) => grants::remove(&*self.host, &grant.id, now)?,
                        None => {
                            return Err(HostError::new(
                                HostErrorCode::Busy,
                                "Too many AI tools are connected.",
                            ))
                        }
                    }
                }
                let mut grant = Grant {
                    id: grants::new_id(),
                    client_name: client.to_owned(),
                    owner_device: device.id,
                    owner_name: Some(device.name),
                    scopes: Vec::new(),
                    remember,
                    created_at: now,
                    last_used_day: None,
                    revoked_at: None,
                };
                grant.set_scopes(&READ_SCOPES);
                grants::put(&*self.host, &grant)?;
                grant
            }
        };
        grants::approve(&*self.host, &grant.id, now)?;
        Ok(grant)
    }

    /// The session's grant if it may call tools now.
    fn access(&self, connection: &str) -> Result<Access, Blocked> {
        let (grant_id, state, denied) = lock(&self.inner)
            .sessions
            .get(connection)
            .map(|session| (session.grant.clone(), session.state, session.denied))
            .ok_or(Blocked::State(State::Expired, false))?;
        let Some(grant_id) = grant_id else {
            if state != State::PendingApproval && self.bind_remembered(connection) {
                return self.access(connection);
            }
            return Err(Blocked::State(state, denied));
        };
        let valid = |grant: &Grant| -> Result<bool, HostError> {
            Ok(!grant.revoked()
                && grant.owner_device == self.local_device()?.id
                && grants::approvals(&*self.host)?.contains_key(&grant.id))
        };
        match grants::get(&*self.host, &grant_id) {
            Ok(Some(grant)) if valid(&grant).unwrap_or(false) => {
                let scopes = grant.scopes();
                let current = scopes.clone();
                self.update(connection, |session| {
                    session.authorize(State::Authorized, Some(grant_id), current)
                });
                Ok(Access { grant, scopes })
            }
            Ok(_) => {
                self.update(connection, |session| {
                    session.authorize(State::Revoked, Some(grant_id), Vec::new())
                });
                Err(Blocked::State(State::Revoked, false))
            }
            Err(_) => Err(Blocked::State(State::Unauthorized, false)),
        }
    }

    /// Serves one tool call. Runs on a worker thread.
    pub(crate) fn call(self: &Arc<Self>, connection: &str, call: ToolCall) -> ToolResult {
        let tool = call.name();
        if self.ask_again(connection) {
            return Blocked::State(State::PendingApproval, false).result();
        }
        let access = match self.access(connection) {
            Ok(access) => access,
            Err(blocked) => return blocked.result(),
        };
        let outcome = self.serve(connection, &access, call);
        let now = now_ms();
        let (item_ids, error) = match &outcome {
            Ok(Ok(answer)) => (answer.ids.clone(), None),
            Ok(Err(failure)) => (Vec::new(), Some(failure.code.to_owned())),
            Err(_) => (Vec::new(), Some("volume".to_owned())),
        };
        lock(&self.log).append(
            &*self.host,
            Entry {
                at: now,
                grant: access.grant.id.clone(),
                tool: tool.to_owned(),
                result_count: u32::try_from(item_ids.len()).unwrap_or(u32::MAX),
                item_ids,
                error,
            },
        );
        self.note_used(&access.grant, now);
        match outcome {
            Err(blocked) => blocked.result(),
            Ok(Err(failure)) => ToolResult {
                is_error: true,
                structured: json!({ "error": failure.code }),
                text: failure.message,
                image: None,
                retry_after_secs: failure.retry_after,
            },
            Ok(Ok(answer)) => {
                self.with_session(connection, |session| {
                    session.read_bytes = session.read_bytes.saturating_add(answer.content_bytes);
                });
                ToolResult {
                    is_error: false,
                    text: answer.text,
                    image: answer.image,
                    structured: answer.structured,
                    retry_after_secs: None,
                }
            }
        }
    }

    fn serve(
        self: &Arc<Self>,
        connection: &str,
        access: &Access,
        call: ToolCall,
    ) -> Result<Result<Answer, Failure>, Blocked> {
        if let Err(seconds) = lock(&self.inner).buckets.take(&access.grant.id, now_ms()) {
            return Ok(Err(Failure {
                retry_after: Some(seconds),
                ..Failure::new(
                    "rate_limited",
                    format!("Too many calls. Retry after {seconds} s."),
                )
            }));
        }
        let scope = required_scope(&call);
        if !access.scopes.contains(&scope) {
            return Ok(Err(Failure::new(
                "scope",
                format!(
                    "This connection was not granted the `{}` scope.",
                    scope.name()
                ),
            )));
        }
        if call.returns_content() {
            self.volume_gate(connection)?;
        }
        let (_, policy) = self.context();
        Ok(tools::serve(
            &Call {
                host: &*self.host,
                scope: policy.scope,
                label: access.grant.client_name.clone(),
            },
            call,
        ))
    }

    /// Pauses content calls once the session read its allowance, asking
    /// the user for more on a worker thread.
    fn volume_gate(self: &Arc<Self>, connection: &str) -> Result<(), Blocked> {
        let client = {
            let mut inner = lock(&self.inner);
            let at_capacity = inner.prompts >= MAX_PENDING_PROMPTS;
            let session = inner
                .sessions
                .get_mut(connection)
                .ok_or(Blocked::State(State::Expired, false))?;
            match session.volume {
                Volume::Declined => return Err(Blocked::VolumeDeclined),
                Volume::Asking => return Err(Blocked::VolumePending),
                Volume::Open if session.read_bytes < session.allowance => return Ok(()),
                Volume::Open if at_capacity => return Err(Blocked::Busy),
                Volume::Open => {}
            }
            session.volume = Volume::Asking;
            let client = session.client.clone().unwrap_or_default();
            inner.prompts += 1;
            client
        };
        let service = Arc::clone(self);
        let owned = connection.to_owned();
        if self.spawn(move || service.ask_volume(&owned, &client)) {
            return Err(Blocked::VolumePending);
        }
        let mut inner = lock(&self.inner);
        inner.prompts = inner.prompts.saturating_sub(1);
        if let Some(session) = inner.sessions.get_mut(connection) {
            session.volume = Volume::Open;
        }
        Err(Blocked::Busy)
    }

    fn ask_volume(&self, connection: &str, client: &str) {
        let locale = self.locale();
        let answer = self.prompt(PromptRequest {
            title: text(&locale, Text::VolumeTitle).to_owned(),
            body: fill(text(&locale, Text::VolumeMessage), &[("client", client)]),
            choices: vec![
                choice(CHOICE_DENY, text(&locale, Text::Deny), true),
                choice(CHOICE_MORE, text(&locale, Text::Allow), false),
            ],
            timeout_secs: PROMPT_TIMEOUT_SECS,
        });
        let allowed =
            matches!(&answer, Ok(PromptAnswer { choice: Some(choice) }) if choice == CHOICE_MORE);
        self.with_session(connection, |session| {
            if allowed {
                session.volume = Volume::Open;
                session.allowance = session
                    .read_bytes
                    .saturating_add(SESSION_READ_ALLOWANCE_BYTES);
            } else if answer.is_err() {
                session.volume = Volume::Open;
            } else {
                session.volume = Volume::Declined;
            }
        });
    }

    /// Last use syncs by the day, so the grant record changes at most daily.
    fn note_used(&self, grant: &Grant, now: i64) {
        let day = now / DAY_MS;
        if grant.last_used_day == Some(day) {
            return;
        }
        if let Ok(Some(mut fresh)) = grants::get(&*self.host, &grant.id) {
            if !fresh.revoked() {
                fresh.last_used_day = Some(day);
                let _ = grants::put(&*self.host, &fresh);
            }
        }
    }

    /// The `status` tool. Never returns clipboard content.
    pub(crate) fn status(&self, connection: &str) -> ToolResult {
        let (state, scopes, denied) = match self.access(connection) {
            Ok(access) => (State::Authorized, access.scopes, false),
            Err(Blocked::State(state, denied)) => (state, Vec::new(), denied),
            Err(_) => (State::Authorized, self.scopes(connection), false),
        };
        let (_, policy) = self.context();
        let devices = host::request::<HistoryFacets>(
            &*self.host,
            ModuleHostRequest::HistoryFacets {
                scope: policy.scope.clone(),
            },
        )
        .map_or(0, |facets| facets.devices.len());
        let (next_step, advice) = match state {
            State::Authorized => (None, None),
            State::PendingApproval => (
                Some("approve_in_app"),
                Some("Waiting for approval in the CopyPaste app on this computer."),
            ),
            State::Unauthorized if denied => (
                Some("access_denied"),
                Some("Access was denied in CopyPaste. Restart the AI tool to ask again."),
            ),
            State::Unauthorized => (
                Some("restart_to_approve"),
                Some("This AI tool is not authorized yet. Restart it and approve it in CopyPaste."),
            ),
            State::Revoked => (
                Some("connect_again"),
                Some("This connection was revoked. Restart the AI tool to ask for access again."),
            ),
            State::Expired => (
                Some("restart_client"),
                Some("This session has ended. Restart the AI tool to reconnect."),
            ),
        };
        let mut lines = vec!["CopyPaste is running.".to_owned()];
        if state == State::Authorized {
            let names: Vec<&str> = scopes.iter().map(|scope| scope_label(*scope)).collect();
            lines.push(format!(
                "This AI tool is connected. Scopes: {}.",
                names.join(", ")
            ));
            lines.push(format!(
                "History it can read: {} (pinned items always).",
                policy.lookback.label()
            ));
        }
        lines.push(format!("Devices contributing history: {devices}."));
        lines.extend(advice.map(str::to_owned));
        ToolResult {
            is_error: false,
            text: lines.join("\n"),
            image: None,
            structured: json!({
                "running": true,
                "status": {
                    "session": state.name(),
                    "scopes": scopes.iter().map(|scope| scope.name()).collect::<Vec<_>>(),
                    "lookback": (state == State::Authorized).then(|| lookback_name(policy.lookback)),
                    "contributing_devices": devices,
                    "next_step": next_step,
                },
            }),
            retry_after_secs: None,
        }
    }

    /// A grant changed through a command; sessions using it see the change.
    fn refresh_grant(&self, grant_id: &str) {
        let connections: Vec<String> = lock(&self.inner)
            .sessions
            .iter()
            .filter(|(_, session)| session.grant.as_deref() == Some(grant_id))
            .map(|(connection, _)| connection.clone())
            .collect();
        for connection in connections {
            let _ = self.access(&connection);
        }
    }

    pub(crate) fn set_access(&self, grant_id: &str, write: bool) -> Result<(), HostError> {
        let mut grant = grants::get(&*self.host, grant_id)?
            .filter(|grant| !grant.revoked())
            .ok_or_else(missing_grant)?;
        grant.set_scopes(if write { &ALL_SCOPES } else { &READ_SCOPES });
        grants::put(&*self.host, &grant)?;
        self.refresh_grant(grant_id);
        Ok(())
    }

    pub(crate) fn revoke(&self, grant_id: &str) -> Result<(), HostError> {
        grants::get(&*self.host, grant_id)?.ok_or_else(missing_grant)?;
        grants::revoke(&*self.host, grant_id, now_ms())?;
        lock(&self.inner).buckets.forget(grant_id);
        self.refresh_grant(grant_id);
        Ok(())
    }

    pub(crate) fn remove(&self, grant_id: &str) -> Result<(), HostError> {
        grants::get(&*self.host, grant_id)?.ok_or_else(missing_grant)?;
        grants::remove(&*self.host, grant_id, now_ms())?;
        lock(&self.log).forget(&*self.host, grant_id);
        self.refresh_grant(grant_id);
        Ok(())
    }

    pub(crate) fn activity(&self, grant_id: &str) -> Vec<Entry> {
        lock(&self.log).of(&*self.host, grant_id)
    }

    pub(crate) fn flush_log(&self) {
        let idle = lock(&self.inner).sessions.is_empty();
        let mut log = lock(&self.log);
        if idle {
            log.release(&*self.host);
        } else {
            log.flush(&*self.host);
        }
    }
}

fn missing_grant() -> HostError {
    HostError::new(
        HostErrorCode::NotFound,
        "That AI tool connection no longer exists.",
    )
}

fn choice(id: &str, title: &str, destructive: bool) -> PromptChoice {
    PromptChoice {
        id: id.to_owned(),
        title: title.chars().take(64).collect(),
        destructive,
    }
}

fn required_scope(call: &ToolCall) -> Scope {
    match call {
        ToolCall::ListRecent { .. } | ToolCall::SearchHistory { .. } | ToolCall::GetItem { .. } => {
            Scope::ReadHistory
        }
        ToolCall::SearchSnippets { .. }
        | ToolCall::GetSnippet { .. }
        | ToolCall::RenderSnippet { .. } => Scope::ReadSnippets,
        ToolCall::CopyToClipboard { .. } => Scope::WriteClipboard,
        ToolCall::SetPinned { .. } | ToolCall::SaveSnippet { .. } => Scope::Organize,
    }
}

fn scope_label(scope: Scope) -> &'static str {
    match scope {
        Scope::ReadHistory => "read history",
        Scope::ReadSnippets => "read snippets",
        Scope::WriteClipboard => "write clipboard",
        Scope::Organize => "organize",
    }
}

fn lookback_name(lookback: Lookback) -> &'static str {
    match lookback {
        Lookback::Day => "day",
        Lookback::Week => "week",
        Lookback::Month => "month",
        Lookback::All => "all",
    }
}
