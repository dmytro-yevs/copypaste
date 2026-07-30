//! GoTrue accounts and sessions.
//!
//! # What this module is
//!
//! Four calls against Supabase's auth service — sign up, sign in, refresh,
//! sign out — and one [`Session`] type that holds the tokens the rest of the
//! crate presents to PostgREST and Realtime.
//!
//! ```text
//! POST /auth/v1/signup                        -> Session (or EmailConfirmationRequired)
//! POST /auth/v1/token?grant_type=password     -> Session
//! POST /auth/v1/token?grant_type=refresh_token-> Session (rotated refresh token)
//! POST /auth/v1/logout                        -> ()
//! ```
//!
//! Every request carries `apikey: <anon key>`. The bearer differs by call: the
//! anon key for the three unauthenticated ones, the user's access token for
//! logout.
//!
//! # The one thing that is easy to get wrong: `invalid_grant`
//!
//! GoTrue answers **both** "that password is wrong" and "that refresh token is
//! dead" with `400`/`422` and the OAuth code `invalid_grant`. The body is not a
//! discriminator, and guessing from its text is how this goes wrong in both
//! directions:
//!
//! * treat a bad password as a dead session and the daemon throws away a
//!   perfectly good session and signs the user out because they typo'd;
//! * treat a dead session as a bad password and a long-lived daemon retries a
//!   refresh that can never succeed, forever.
//!
//! The fix — carried from port manifest 05 §4.6.1, which is *binding* — is that
//! **the grant kind we asked for is authoritative**. `GrantKind` is threaded
//! into the request helper and decides the variant; nothing in this file
//! inspects the error body to classify a failure. The body is only ever
//! *logged*, truncated, so a 502 HTML gateway page stays diagnosable
//! (manifest AT-38).
//!
//! # Secret hygiene
//!
//! * [`Session`]'s `Debug` prints `<redacted>` for both tokens (AT-42). It is a
//!   hand-written impl precisely so a later `#[derive(Debug)]` cannot leak them.
//! * Both tokens are zeroized on drop.
//! * `Session` deliberately does **not** implement `Serialize`. Persisting a
//!   session is the session store's job and should be a deliberate act, not
//!   something that falls out of `serde_json::to_string` in a log line.
//! * Emails are masked in logs by `redact_email`.
//! * No error variant carries free text, so no path and no token can reach a
//!   user-facing message (`CLAUDE.md` rule 4). `reqwest`'s own errors have their
//!   URL stripped before we keep them.
//!
//! # Retry
//!
//! One policy, from the `backoff` crate, shared with [`crate::rest`] via
//! [`transient_backoff`]. Network faults and 5xx retry; everything else is
//! permanent and surfaces to the caller immediately. A `429` is **not** retried
//! here — it is returned as [`AuthError::RateLimited`] with the server's
//! `Retry-After`, because the caller (the refresh loop) is the thing that knows
//! how long it may sleep. v1 had six separate hand-rolled backoffs; this crate
//! has none of its own.

use std::fmt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backoff::backoff::Backoff;
use backoff::future::retry;
use backoff::{Error as Retryable, ExponentialBackoff, ExponentialBackoffBuilder};
use reqwest::header::HeaderMap;
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use serde_json::json;
use zeroize::{Zeroize, ZeroizeOnDrop};

use crate::CloudConfig;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Per-request timeout on every auth call.
///
/// Applied per request rather than on the `Client`, so there is no fallible
/// client builder and therefore no tempting "fall back to a client with no
/// timeout" branch — the failure mode behind manifest bug `CopyPaste-16vr`.
/// Without a timeout a stalled GoTrue endpoint blocks the refresh loop forever
/// (`CopyPaste-8ebg.49`).
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(30);

/// How long before expiry a session should be refreshed.
///
/// Exported for the refresh loop that lives outside this module; the manifest's
/// `REFRESH_MARGIN_SECS = 60`.
pub const REFRESH_MARGIN_MS: i64 = 60_000;

/// Longest error body we will put in a log line.
const MAX_ERROR_SNIPPET: usize = 200;

/// Retry envelope for transient failures: ~1 s, 2 s, 4 s, 8 s and stop.
///
/// `backoff` bounds a retry by elapsed time rather than attempt count; with the
/// intervals below that is four to five attempts, which is the manifest's "hard
/// cap of 4 attempts" (§4.6.3) expressed in the vocabulary the crate has.
const RETRY_INITIAL: Duration = Duration::from_secs(1);
const RETRY_MAX_INTERVAL: Duration = Duration::from_secs(30);
const RETRY_MAX_ELAPSED: Duration = Duration::from_secs(16);

// ---------------------------------------------------------------------------
// Session
// ---------------------------------------------------------------------------

/// A signed-in user's tokens.
///
/// `Clone` is derived because the daemon hands copies to the poll, push and
/// Realtime paths. `Debug` is hand-written and redacts both tokens. Dropping a
/// `Session` — including every clone — zeroizes them.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct Session {
    /// The user JWT. Presented as `Authorization: Bearer` to PostgREST and as
    /// `config.access_token` on the Realtime join.
    pub access_token: String,
    /// Rotated by GoTrue on every refresh — the value returned by
    /// [`SupabaseAuth::refresh`] must replace the one that was sent, or the
    /// next refresh presents a token the server has already retired.
    pub refresh_token: String,
    /// `auth.uid()` — the RLS pivot, and the Realtime filter value.
    pub user_id: String,
    /// Absolute expiry, ms since epoch, computed with a saturating add so a
    /// hostile `expires_in` cannot wrap into "already expired" (AT-43).
    pub expires_at_ms: i64,
}

impl Session {
    /// Whether the access token is past its expiry at `now_ms`.
    pub fn is_expired(&self, now_ms: i64) -> bool {
        now_ms >= self.expires_at_ms
    }

    /// Whether the access token expires within [`REFRESH_MARGIN_MS`] — the
    /// condition a proactive refresh loop waits on.
    pub fn needs_refresh(&self, now_ms: i64) -> bool {
        self.expires_at_ms.saturating_sub(now_ms) <= REFRESH_MARGIN_MS
    }
}

impl fmt::Debug for Session {
    /// Redacts both tokens. `expires_at_ms` and `user_id` stay visible because
    /// they are what a bug report actually needs (AT-42).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("user_id", &self.user_id)
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("expires_at_ms", &self.expires_at_ms)
            .finish()
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Why an auth call failed.
///
/// No variant carries free text: every one is a fixed shape the caller can
/// branch on, which is also what structurally guarantees that no token, email
/// or filesystem path reaches a user-facing string.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// The password was wrong. Distinct from [`AuthError::SessionExpired`] even
    /// though GoTrue reports both as `invalid_grant`; the two need opposite
    /// recoveries (prompt the user vs. silently re-authenticate).
    #[error("email or password is incorrect")]
    InvalidCredentials,

    /// The refresh token is dead — expired, revoked, or already rotated away.
    /// The user must sign in again; retrying the refresh cannot help.
    #[error("the session has expired; sign in again")]
    SessionExpired,

    /// GoTrue is throttling. `retry_after_secs` is the `Retry-After` header in
    /// its delta-seconds form; the HTTP-date form is deliberately unsupported
    /// (manifest §4.6.3 — Supabase emits integer seconds and date parsing buys
    /// nothing).
    #[error("too many attempts; try again later")]
    RateLimited { retry_after_secs: Option<u64> },

    /// The request never got an answer. The URL is stripped from the underlying
    /// error before it is kept.
    #[error("could not reach the account service")]
    Network(#[source] reqwest::Error),

    /// The service faulted (5xx). Transient — this is what gets retried.
    #[error("the account service returned status {status}")]
    Server { status: u16 },

    /// A 4xx that is not a grant failure, a 401 or a 429: a weak password, an
    /// already-registered email, a malformed request.
    ///
    /// *Addition to the requested enum.* Folding these into
    /// [`AuthError::InvalidCredentials`] would tell a user that their password
    /// was wrong when the real problem is that the account already exists, and
    /// folding them into [`AuthError::Server`] would blame the backend for a
    /// client-side mistake. The human-readable detail is logged, not carried.
    #[error("the account service rejected the request (status {status})")]
    Rejected { status: u16 },

    /// Sign-up succeeded but returned no session, because the project requires
    /// email confirmation first.
    ///
    /// *Addition to the requested enum.* This is a correct, common deployment
    /// configuration; reporting it as [`AuthError::Malformed`] would tell the
    /// user their backend is broken when in fact they have mail waiting.
    #[error("check your email to confirm the account before signing in")]
    EmailConfirmationRequired,

    /// A 2xx whose body was not a session we can use.
    #[error("the account service returned an unexpected response")]
    Malformed,
}

impl AuthError {
    fn from_reqwest(err: reqwest::Error) -> Self {
        // `without_url` drops the project URL from the message. Nothing secret
        // lives there, but errors are user-facing and the project ref is noise.
        AuthError::Network(err.without_url())
    }

    /// Whether retrying the identical request could plausibly succeed.
    fn is_transient(&self) -> bool {
        matches!(self, AuthError::Network(_) | AuthError::Server { .. })
    }
}

/// Which grant we asked for. **This, not the response body, decides how a
/// `400`/`422` is reported** (manifest §4.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GrantKind {
    /// `POST /auth/v1/signup`.
    SignUp,
    /// `grant_type=password` — a rejection means the credentials are wrong.
    Password,
    /// `grant_type=refresh_token` — a rejection means the session is dead.
    Refresh,
    /// A call authenticated by an existing access token (logout).
    Bearer,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

/// Talks to one Supabase project's GoTrue endpoints.
#[derive(Clone)]
pub struct SupabaseAuth {
    config: CloudConfig,
    http: Client,
    retry: ExponentialBackoff,
}

impl fmt::Debug for SupabaseAuth {
    /// The anon key is not a secret, but printing it in every log line is
    /// still noise, and `CloudConfig`'s own `Debug` would.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SupabaseAuth")
            .field("url", &self.config.url)
            .finish_non_exhaustive()
    }
}

impl SupabaseAuth {
    /// Build a client for `config`.
    pub fn new(config: CloudConfig) -> Self {
        Self {
            config,
            http: Client::new(),
            retry: transient_backoff(),
        }
    }

    /// Replace the retry policy. Mostly for tests, which cannot afford to sleep
    /// seconds, and for a caller that wants a different envelope.
    #[must_use]
    pub fn with_retry_policy(mut self, policy: ExponentialBackoff) -> Self {
        self.retry = policy;
        self
    }

    /// Create an account.
    ///
    /// Returns [`AuthError::EmailConfirmationRequired`] when the project has
    /// email confirmation enabled: GoTrue answers `200` with a user object and
    /// no tokens, which is success, not a malformed response.
    pub async fn sign_up(&self, email: &str, password: &str) -> Result<Session, AuthError> {
        tracing::debug!(email = %redact_email(email), "sign-up requested");
        let body = json!({ "email": email, "password": password });
        let text = self
            .post(
                &self.endpoint("/auth/v1/signup"),
                &body,
                &self.config.anon_key,
                GrantKind::SignUp,
            )
            .await?;
        self.session_from(&text)
    }

    /// Exchange an email and password for a session.
    ///
    /// A `400`/`422` here is [`AuthError::InvalidCredentials`] — decided by the
    /// grant, never by the body.
    pub async fn sign_in(&self, email: &str, password: &str) -> Result<Session, AuthError> {
        tracing::debug!(email = %redact_email(email), "sign-in requested");
        let body = json!({ "email": email, "password": password });
        let text = self
            .post(
                &self.endpoint("/auth/v1/token?grant_type=password"),
                &body,
                &self.config.anon_key,
                GrantKind::Password,
            )
            .await?;
        self.session_from(&text)
    }

    /// Exchange a refresh token for a fresh session.
    ///
    /// A `400`/`422` here is [`AuthError::SessionExpired`] — the identical body
    /// that [`SupabaseAuth::sign_in`] reports as bad credentials. The caller
    /// must persist the returned `refresh_token`: GoTrue rotates it, and the
    /// one that was passed in is retired the moment this succeeds.
    pub async fn refresh(&self, refresh_token: &str) -> Result<Session, AuthError> {
        tracing::debug!("refreshing session");
        let body = json!({ "refresh_token": refresh_token });
        let text = self
            .post(
                &self.endpoint("/auth/v1/token?grant_type=refresh_token"),
                &body,
                &self.config.anon_key,
                GrantKind::Refresh,
            )
            .await?;
        self.session_from(&text)
    }

    /// Revoke `access_token` server-side.
    ///
    /// A `401` is reported as success: the goal of signing out is that the
    /// token stops working, and a token the server already refuses satisfies
    /// that. Anything else would make "sign out" fail on exactly the sessions
    /// that most need clearing.
    pub async fn sign_out(&self, access_token: &str) -> Result<(), AuthError> {
        let body = json!({});
        match self
            .post(
                &self.endpoint("/auth/v1/logout"),
                &body,
                access_token,
                GrantKind::Bearer,
            )
            .await
        {
            Ok(_) => Ok(()),
            Err(AuthError::Rejected { status: 401 }) | Err(AuthError::SessionExpired) => {
                tracing::debug!("sign-out: token was already invalid");
                Ok(())
            }
            Err(err) => Err(err),
        }
    }

    fn endpoint(&self, path_and_query: &str) -> String {
        format!(
            "{}{}",
            self.config.url.trim_end_matches('/'),
            path_and_query
        )
    }

    /// One request, retried under the shared policy, classified by `grant`.
    async fn post(
        &self,
        url: &str,
        body: &serde_json::Value,
        bearer: &str,
        grant: GrantKind,
    ) -> Result<String, AuthError> {
        let mut policy = self.retry.clone();
        policy.reset();

        retry(policy, || async move {
            let sent = self
                .http
                .post(url)
                .timeout(AUTH_TIMEOUT)
                .header("apikey", self.config.anon_key.as_str())
                .bearer_auth(bearer)
                .json(body)
                .send()
                .await;

            let err = match sent {
                Ok(response) => match classify(response, grant).await {
                    Ok(text) => return Ok(text),
                    Err(err) => err,
                },
                Err(err) => AuthError::from_reqwest(err),
            };

            if err.is_transient() {
                Err(Retryable::transient(err))
            } else {
                Err(Retryable::permanent(err))
            }
        })
        .await
    }

    fn session_from(&self, body: &str) -> Result<Session, AuthError> {
        let parsed: TokenBody = serde_json::from_str(body).map_err(|_| {
            tracing::warn!("auth response was not JSON");
            AuthError::Malformed
        })?;
        parsed.into_session(now_ms())
    }
}

/// Turn a response into its body text, or into the error the status means.
///
/// `grant` is the only input to the `400`/`422`/`401` decision.
async fn classify(response: Response, grant: GrantKind) -> Result<String, AuthError> {
    let status = response.status();
    if status.is_success() {
        return response
            .text()
            .await
            .map_err(|_| AuthError::Malformed)
            .map(|text| {
                if text.is_empty() {
                    "{}".to_string()
                } else {
                    text
                }
            });
    }

    let retry_after = retry_after_secs(response.headers());
    let code = status.as_u16();
    // Read the body before deciding, purely so the failure is diagnosable. It
    // never influences the classification.
    let body = response.text().await.unwrap_or_default();
    let detail = error_detail(&body);

    if status == StatusCode::TOO_MANY_REQUESTS {
        tracing::warn!(status = code, retry_after_secs = ?retry_after, detail = %detail, "auth throttled");
        return Err(AuthError::RateLimited {
            retry_after_secs: retry_after,
        });
    }

    if status.is_server_error() {
        tracing::warn!(status = code, detail = %detail, "auth service faulted");
        return Err(AuthError::Server { status: code });
    }

    // The disambiguation. GoTrue says `invalid_grant` for a wrong password and
    // for a dead refresh token; the grant we asked for is what tells them
    // apart (manifest §4.6.1). Never parse `detail` to decide this.
    let grant_failure = matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY | StatusCode::UNAUTHORIZED
    );
    if grant_failure {
        match grant {
            GrantKind::Password => {
                tracing::debug!(status = code, detail = %detail, "password grant rejected");
                return Err(AuthError::InvalidCredentials);
            }
            GrantKind::Refresh => {
                tracing::debug!(status = code, detail = %detail, "refresh grant rejected");
                return Err(AuthError::SessionExpired);
            }
            GrantKind::SignUp | GrantKind::Bearer => {}
        }
    }

    tracing::warn!(status = code, detail = %detail, "auth request rejected");
    Err(AuthError::Rejected { status: code })
}

// ---------------------------------------------------------------------------
// Response shapes
// ---------------------------------------------------------------------------

/// The GoTrue token response, and — for a confirmation-gated sign-up — the bare
/// user object that comes back instead.
///
/// No `Debug`: it holds tokens.
#[derive(Deserialize)]
struct TokenBody {
    access_token: Option<String>,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    /// Unix *seconds*, sent alongside `expires_in`. Used only as a fallback.
    expires_at: Option<i64>,
    user: Option<UserBody>,
    /// Present when the response is a user object rather than a session.
    id: Option<String>,
}

#[derive(Deserialize)]
struct UserBody {
    id: Option<String>,
}

impl TokenBody {
    fn into_session(self, now_ms: i64) -> Result<Session, AuthError> {
        let access_token = self.access_token.filter(|t| !t.is_empty());
        let Some(access_token) = access_token else {
            // A user object with no tokens is a confirmation-gated sign-up,
            // which is a success the caller must be able to explain.
            return Err(if self.id.is_some() {
                AuthError::EmailConfirmationRequired
            } else {
                tracing::warn!("auth response carried no access token");
                AuthError::Malformed
            });
        };

        let refresh_token = self
            .refresh_token
            .filter(|t| !t.is_empty())
            .ok_or(AuthError::Malformed)?;

        let user_id = self
            .user
            .and_then(|u| u.id)
            .or(self.id)
            .filter(|id| !id.is_empty())
            .ok_or(AuthError::Malformed)?;

        // Saturating throughout: a hostile or buggy `expires_in` near i64::MAX
        // must not wrap into a timestamp in the past, which would make the
        // token look already-expired and stall the refresh loop forever
        // (manifest AT-43).
        let expires_at_ms = match self.expires_in {
            Some(secs) => now_ms.saturating_add(secs.max(0).saturating_mul(1000)),
            // No `expires_in`: fall back to the absolute `expires_at`, in
            // seconds. If neither is present we fail closed rather than invent
            // a lifetime — a session we cannot time is a session the refresh
            // loop cannot manage.
            None => self
                .expires_at
                .ok_or(AuthError::Malformed)?
                .max(0)
                .saturating_mul(1000),
        };

        Ok(Session {
            access_token,
            refresh_token,
            user_id,
            expires_at_ms,
        })
    }
}

// ---------------------------------------------------------------------------
// Shared helpers (also used by `crate::rest`)
// ---------------------------------------------------------------------------

/// The one retry policy in this crate.
///
/// `rest` uses it too. It lives here rather than in a module of its own because
/// the module list is fixed by `lib.rs`; what matters is that there is exactly
/// one, built by `backoff`, rather than the six hand-rolled loops v1 accrued
/// (`CLAUDE.md` rule 1).
pub fn transient_backoff() -> ExponentialBackoff {
    ExponentialBackoffBuilder::new()
        .with_initial_interval(RETRY_INITIAL)
        .with_max_interval(RETRY_MAX_INTERVAL)
        .with_max_elapsed_time(Some(RETRY_MAX_ELAPSED))
        .build()
}

/// `Retry-After`, delta-seconds form only.
///
/// The HTTP-date form is deliberately unsupported: Supabase emits integer
/// seconds, and a date parser here would be a second time-handling code path
/// for no behaviour (manifest §4.6.3).
pub(crate) fn retry_after_secs(headers: &HeaderMap) -> Option<u64> {
    headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim()
        .parse::<u64>()
        .ok()
}

/// Mask an email for logging: `alice@example.com` -> `a***@example.com`.
///
/// Anything without a usable `@` collapses to `<redacted>` rather than being
/// passed through, so a malformed input cannot become a log-line disclosure.
pub(crate) fn redact_email(email: &str) -> String {
    let trimmed = email.trim();
    let Some((local, domain)) = trimmed.split_once('@') else {
        return "<redacted>".to_string();
    };
    if local.is_empty() || domain.is_empty() || domain.contains('@') {
        return "<redacted>".to_string();
    }
    let first = local.chars().next().expect("local part is non-empty");
    format!("{first}***@{domain}")
}

/// A short, diagnosable description of an error body — for logs only.
///
/// Tries the structured GoTrue envelope first; falls back to a truncated raw
/// snippet so a 502 HTML gateway page reads as something rather than as
/// "unknown error" (manifest AT-38).
fn error_detail(body: &str) -> String {
    #[derive(Deserialize)]
    struct GoTrueError {
        error: Option<String>,
        error_description: Option<String>,
        error_code: Option<String>,
        msg: Option<String>,
        message: Option<String>,
    }

    if let Ok(envelope) = serde_json::from_str::<GoTrueError>(body) {
        let message = envelope
            .error_description
            .or(envelope.message)
            .or(envelope.msg)
            .or(envelope.error)
            .or(envelope.error_code);
        if let Some(message) = message {
            if !message.trim().is_empty() {
                return truncate(message.trim());
            }
        }
    }

    let raw = body.trim();
    if raw.is_empty() {
        "<empty body>".to_string()
    } else {
        truncate(raw)
    }
}

/// Truncate to [`MAX_ERROR_SNIPPET`] bytes, on a char boundary.
fn truncate(text: &str) -> String {
    if text.len() <= MAX_ERROR_SNIPPET {
        return text.to_string();
    }
    let mut end = MAX_ERROR_SNIPPET;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &text[..end])
}

/// Wall clock in milliseconds. Saturating, so a clock before the epoch gives 0
/// rather than a negative expiry.
pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Test support
// ---------------------------------------------------------------------------

/// A scripted HTTP/1.1 server, so the tests exercise real `reqwest` requests
/// without touching the network.
///
/// Lives here because `rest`'s tests use it too and the module list is fixed by
/// `lib.rs`; one stub, not two.
#[cfg(test)]
pub(crate) mod stub {
    use std::sync::{Arc, Mutex};

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// One request as the stub saw it on the wire.
    #[derive(Debug, Clone)]
    pub(crate) struct Captured {
        pub method: String,
        /// Path plus query, exactly as sent.
        pub target: String,
        pub headers: Vec<(String, String)>,
        pub body: String,
    }

    impl Captured {
        pub fn header(&self, name: &str) -> Option<&str> {
            self.headers
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(name))
                .map(|(_, v)| v.as_str())
        }

        pub fn json(&self) -> serde_json::Value {
            serde_json::from_str(&self.body).expect("request body should be JSON")
        }
    }

    /// One scripted response.
    #[derive(Debug, Clone)]
    pub(crate) struct Reply {
        pub status: u16,
        pub headers: Vec<(String, String)>,
        pub body: String,
    }

    impl Reply {
        pub fn json(status: u16, body: &str) -> Self {
            Self {
                status,
                headers: vec![("content-type".into(), "application/json".into())],
                body: body.to_string(),
            }
        }

        pub fn text(status: u16, body: &str) -> Self {
            Self {
                status,
                headers: vec![("content-type".into(), "text/html".into())],
                body: body.to_string(),
            }
        }

        pub fn empty(status: u16) -> Self {
            Self {
                status,
                headers: Vec::new(),
                body: String::new(),
            }
        }

        pub fn with_header(mut self, name: &str, value: &str) -> Self {
            self.headers.push((name.to_string(), value.to_string()));
            self
        }
    }

    pub(crate) struct Stub {
        /// Base URL to hand to `CloudConfig::url`.
        pub base_url: String,
        captured: Arc<Mutex<Vec<Captured>>>,
    }

    impl Stub {
        /// Serve `replies` in order; once exhausted the last one repeats, so a
        /// test that only cares about the first response need not script the
        /// retries.
        pub async fn start(replies: Vec<Reply>) -> Self {
            assert!(!replies.is_empty(), "a stub needs at least one reply");
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind loopback");
            let addr = listener.local_addr().expect("local addr");
            let captured = Arc::new(Mutex::new(Vec::new()));
            let sink = Arc::clone(&captured);

            tokio::spawn(async move {
                let mut served = 0usize;
                while let Ok((mut socket, _)) = listener.accept().await {
                    let request = match read_request(&mut socket).await {
                        Some(request) => request,
                        None => continue,
                    };
                    sink.lock().expect("stub mutex").push(request);

                    let reply = replies
                        .get(served)
                        .cloned()
                        .unwrap_or_else(|| replies.last().expect("non-empty").clone());
                    served += 1;

                    let mut head = format!(
                        "HTTP/1.1 {} X\r\ncontent-length: {}\r\nconnection: close\r\n",
                        reply.status,
                        reply.body.len()
                    );
                    for (name, value) in &reply.headers {
                        head.push_str(&format!("{name}: {value}\r\n"));
                    }
                    head.push_str("\r\n");
                    let _ = socket.write_all(head.as_bytes()).await;
                    let _ = socket.write_all(reply.body.as_bytes()).await;
                    let _ = socket.flush().await;
                    let _ = socket.shutdown().await;
                }
            });

            Self {
                base_url: format!("http://{addr}"),
                captured,
            }
        }

        pub fn requests(&self) -> Vec<Captured> {
            self.captured.lock().expect("stub mutex").clone()
        }

        pub fn request_count(&self) -> usize {
            self.captured.lock().expect("stub mutex").len()
        }

        /// The single request the test expected to be made.
        pub fn only_request(&self) -> Captured {
            let requests = self.requests();
            assert_eq!(requests.len(), 1, "expected exactly one request");
            requests.into_iter().next().expect("one request")
        }
    }

    async fn read_request(socket: &mut tokio::net::TcpStream) -> Option<Captured> {
        let mut buffer = Vec::new();
        let mut chunk = [0u8; 4096];

        let header_end = loop {
            let read = socket.read(&mut chunk).await.ok()?;
            if read == 0 {
                return None;
            }
            buffer.extend_from_slice(&chunk[..read]);
            if let Some(at) = find_double_crlf(&buffer) {
                break at;
            }
        };

        let head = String::from_utf8_lossy(&buffer[..header_end]).to_string();
        let mut lines = head.split("\r\n");
        let request_line = lines.next()?;
        let mut parts = request_line.split_whitespace();
        let method = parts.next()?.to_string();
        let target = parts.next()?.to_string();

        let mut headers = Vec::new();
        for line in lines {
            if let Some((name, value)) = line.split_once(':') {
                headers.push((name.trim().to_lowercase(), value.trim().to_string()));
            }
        }

        let content_length: usize = headers
            .iter()
            .find(|(k, _)| k == "content-length")
            .and_then(|(_, v)| v.parse().ok())
            .unwrap_or(0);

        let mut body = buffer[header_end + 4..].to_vec();
        while body.len() < content_length {
            let read = socket.read(&mut chunk).await.ok()?;
            if read == 0 {
                break;
            }
            body.extend_from_slice(&chunk[..read]);
        }

        Some(Captured {
            method,
            target,
            headers,
            body: String::from_utf8_lossy(&body).to_string(),
        })
    }

    fn find_double_crlf(buffer: &[u8]) -> Option<usize> {
        buffer.windows(4).position(|w| w == b"\r\n\r\n")
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::stub::{Reply, Stub};
    use super::*;

    const ANON: &str = "anon-key-abc";

    /// A retry policy that finishes in milliseconds, so a test that exercises
    /// the transient path does not sleep for seconds.
    fn fast_retry() -> ExponentialBackoff {
        ExponentialBackoffBuilder::new()
            .with_initial_interval(Duration::from_millis(1))
            .with_max_interval(Duration::from_millis(2))
            .with_max_elapsed_time(Some(Duration::from_millis(10)))
            .build()
    }

    fn client(stub: &Stub) -> SupabaseAuth {
        SupabaseAuth::new(CloudConfig {
            url: stub.base_url.clone(),
            anon_key: ANON.to_string(),
        })
        .with_retry_policy(fast_retry())
    }

    fn session_body(access: &str, refresh: &str, expires_in: i64) -> String {
        format!(
            r#"{{"access_token":"{access}","refresh_token":"{refresh}",
                 "token_type":"bearer","expires_in":{expires_in},
                 "user":{{"id":"user-uuid-1","email":"a@example.com"}}}}"#
        )
    }

    /// The body GoTrue returns for a wrong password *and* for a dead refresh
    /// token. Byte-identical on purpose: it is the whole point.
    const INVALID_GRANT: &str =
        r#"{"error":"invalid_grant","error_description":"Invalid login credentials"}"#;

    // -- the disambiguation, both directions ------------------------------

    #[tokio::test]
    async fn invalid_grant_on_the_password_grant_is_bad_credentials() {
        let stub = Stub::start(vec![Reply::json(400, INVALID_GRANT)]).await;
        let err = client(&stub)
            .sign_in("alice@example.com", "wrong")
            .await
            .expect_err("should fail");
        assert!(
            matches!(err, AuthError::InvalidCredentials),
            "password grant must not report a dead session, got {err:?}"
        );
    }

    #[tokio::test]
    async fn the_same_body_on_the_refresh_grant_is_an_expired_session() {
        let stub = Stub::start(vec![Reply::json(400, INVALID_GRANT)]).await;
        let err = client(&stub)
            .refresh("dead-refresh-token")
            .await
            .expect_err("should fail");
        assert!(
            matches!(err, AuthError::SessionExpired),
            "refresh grant must not report bad credentials, got {err:?}"
        );
    }

    #[tokio::test]
    async fn the_disambiguation_holds_for_422_and_401_too() {
        for status in [422, 401] {
            let stub = Stub::start(vec![Reply::json(status, INVALID_GRANT)]).await;
            let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
            assert!(matches!(err, AuthError::InvalidCredentials), "{status}");

            let stub = Stub::start(vec![Reply::json(status, INVALID_GRANT)]).await;
            let err = client(&stub).refresh("t").await.unwrap_err();
            assert!(matches!(err, AuthError::SessionExpired), "{status}");
        }
    }

    #[tokio::test]
    async fn a_grant_failure_is_not_retried() {
        let stub = Stub::start(vec![Reply::json(400, INVALID_GRANT)]).await;
        let _ = client(&stub).sign_in("a@b.co", "x").await;
        assert_eq!(stub.request_count(), 1, "a wrong password is permanent");
    }

    // -- endpoints, headers, bodies ---------------------------------------

    #[tokio::test]
    async fn sign_in_posts_the_password_grant_with_both_auth_headers() {
        let stub = Stub::start(vec![Reply::json(200, &session_body("at", "rt", 3600))]).await;
        client(&stub)
            .sign_in("alice@example.com", "hunter2")
            .await
            .expect("sign-in");

        let request = stub.only_request();
        assert_eq!(request.method, "POST");
        assert_eq!(request.target, "/auth/v1/token?grant_type=password");
        assert_eq!(request.header("apikey"), Some(ANON));
        assert_eq!(
            request.header("authorization"),
            Some(format!("Bearer {ANON}").as_str())
        );
        assert_eq!(request.json()["email"], "alice@example.com");
        assert_eq!(request.json()["password"], "hunter2");
    }

    #[tokio::test]
    async fn refresh_posts_only_the_refresh_token() {
        let stub = Stub::start(vec![Reply::json(200, &session_body("at2", "rt2", 3600))]).await;
        client(&stub).refresh("rt1").await.expect("refresh");

        let request = stub.only_request();
        assert_eq!(request.target, "/auth/v1/token?grant_type=refresh_token");
        let body = request.json();
        assert_eq!(body["refresh_token"], "rt1");
        assert!(body.get("password").is_none(), "no password on a refresh");
    }

    #[tokio::test]
    async fn sign_up_posts_to_the_signup_endpoint() {
        let stub = Stub::start(vec![Reply::json(200, &session_body("at", "rt", 60))]).await;
        client(&stub)
            .sign_up("new@example.com", "pw")
            .await
            .expect("sign-up");
        assert_eq!(stub.only_request().target, "/auth/v1/signup");
    }

    #[tokio::test]
    async fn sign_out_bearers_the_access_token_not_the_anon_key() {
        let stub = Stub::start(vec![Reply::empty(204)]).await;
        client(&stub)
            .sign_out("user-access-token")
            .await
            .expect("sign-out");

        let request = stub.only_request();
        assert_eq!(request.target, "/auth/v1/logout");
        assert_eq!(request.header("apikey"), Some(ANON));
        assert_eq!(
            request.header("authorization"),
            Some("Bearer user-access-token")
        );
    }

    #[tokio::test]
    async fn sign_out_of_an_already_dead_token_succeeds() {
        let stub = Stub::start(vec![Reply::json(401, INVALID_GRANT)]).await;
        client(&stub)
            .sign_out("stale")
            .await
            .expect("signing out a dead token is not a failure");
    }

    #[tokio::test]
    async fn a_trailing_slash_on_the_project_url_does_not_double_up() {
        let stub = Stub::start(vec![Reply::json(200, &session_body("at", "rt", 1))]).await;
        let auth = SupabaseAuth::new(CloudConfig {
            url: format!("{}/", stub.base_url),
            anon_key: ANON.to_string(),
        })
        .with_retry_policy(fast_retry());
        auth.sign_in("a@b.co", "x").await.expect("sign-in");
        assert_eq!(
            stub.only_request().target,
            "/auth/v1/token?grant_type=password"
        );
    }

    // -- session construction ---------------------------------------------

    #[tokio::test]
    async fn a_session_carries_the_tokens_and_a_computed_expiry() {
        let stub = Stub::start(vec![Reply::json(200, &session_body("acc", "ref", 3600))]).await;
        let before = now_ms();
        let session = client(&stub).sign_in("a@b.co", "x").await.expect("sign-in");

        assert_eq!(session.access_token, "acc");
        assert_eq!(session.refresh_token, "ref");
        assert_eq!(session.user_id, "user-uuid-1");
        assert!(session.expires_at_ms >= before + 3_600_000);
        assert!(session.expires_at_ms <= now_ms() + 3_600_000);
        assert!(!session.is_expired(now_ms()));
        assert!(!session.needs_refresh(now_ms()));
    }

    #[test]
    fn a_hostile_expires_in_saturates_instead_of_wrapping() {
        let body = TokenBody {
            access_token: Some("a".into()),
            refresh_token: Some("r".into()),
            expires_in: Some(i64::MAX),
            expires_at: None,
            user: None,
            id: Some("u".into()),
        };
        let session = body.into_session(now_ms()).expect("session");
        assert_eq!(session.expires_at_ms, i64::MAX);
        assert!(
            !session.is_expired(now_ms()),
            "a wrapped expiry would read as already-expired and stall auth"
        );
    }

    #[test]
    fn a_session_with_no_expiry_at_all_is_malformed_rather_than_invented() {
        let body = TokenBody {
            access_token: Some("a".into()),
            refresh_token: Some("r".into()),
            expires_in: None,
            expires_at: None,
            user: None,
            id: Some("u".into()),
        };
        assert!(matches!(
            body.into_session(0).unwrap_err(),
            AuthError::Malformed
        ));
    }

    #[test]
    fn absolute_expires_at_is_accepted_when_expires_in_is_absent() {
        let body = TokenBody {
            access_token: Some("a".into()),
            refresh_token: Some("r".into()),
            expires_in: None,
            expires_at: Some(1_700_000_000),
            user: None,
            id: Some("u".into()),
        };
        let session = body.into_session(0).expect("session");
        assert_eq!(session.expires_at_ms, 1_700_000_000_000);
    }

    #[tokio::test]
    async fn a_signup_awaiting_confirmation_is_not_reported_as_malformed() {
        let stub = Stub::start(vec![Reply::json(
            200,
            r#"{"id":"user-uuid-9","email":"new@example.com","confirmation_sent_at":"2026-07-30T00:00:00Z"}"#,
        )])
        .await;
        let err = client(&stub)
            .sign_up("new@example.com", "pw")
            .await
            .unwrap_err();
        assert!(
            matches!(err, AuthError::EmailConfirmationRequired),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn a_2xx_that_is_not_a_session_is_malformed() {
        let stub = Stub::start(vec![Reply::json(200, r#"{"nonsense":true}"#)]).await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        assert!(matches!(err, AuthError::Malformed), "{err:?}");
    }

    #[tokio::test]
    async fn a_non_json_2xx_is_malformed_not_a_panic() {
        let stub = Stub::start(vec![Reply::text(200, "<html>hello</html>")]).await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        assert!(matches!(err, AuthError::Malformed), "{err:?}");
    }

    // -- 429 ---------------------------------------------------------------

    #[tokio::test]
    async fn a_429_surfaces_retry_after_in_seconds() {
        let stub =
            Stub::start(vec![Reply::json(429, r#"{"message":"too many requests"}"#)
                .with_header("retry-after", "42")])
            .await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        match err {
            AuthError::RateLimited { retry_after_secs } => {
                assert_eq!(retry_after_secs, Some(42));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
        assert_eq!(stub.request_count(), 1, "a 429 must not be retried here");
    }

    #[tokio::test]
    async fn a_429_without_the_header_still_reports_rate_limiting() {
        let stub = Stub::start(vec![Reply::json(429, "{}")]).await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        assert!(
            matches!(
                err,
                AuthError::RateLimited {
                    retry_after_secs: None
                }
            ),
            "{err:?}"
        );
    }

    #[test]
    fn retry_after_parses_delta_seconds_and_ignores_the_http_date_form() {
        let mut headers = HeaderMap::new();
        headers.insert("retry-after", "30".parse().unwrap());
        assert_eq!(retry_after_secs(&headers), Some(30));

        headers.insert("retry-after", " 7 ".parse().unwrap());
        assert_eq!(retry_after_secs(&headers), Some(7));

        // Deliberately unsupported: Supabase emits integer seconds.
        headers.insert(
            "retry-after",
            "Wed, 21 Oct 2026 07:28:00 GMT".parse().unwrap(),
        );
        assert_eq!(retry_after_secs(&headers), None);

        headers.insert("retry-after", "-5".parse().unwrap());
        assert_eq!(retry_after_secs(&headers), None);

        assert_eq!(retry_after_secs(&HeaderMap::new()), None);
    }

    // -- 5xx and retry ------------------------------------------------------

    #[tokio::test]
    async fn a_5xx_is_transient_and_is_retried() {
        let stub = Stub::start(vec![
            Reply::text(502, "<html>bad gateway</html>"),
            Reply::json(200, &session_body("at", "rt", 60)),
        ])
        .await;
        let session = client(&stub).sign_in("a@b.co", "x").await.expect("sign-in");
        assert_eq!(session.access_token, "at");
        assert_eq!(stub.request_count(), 2);
    }

    #[tokio::test]
    async fn a_permanent_5xx_gives_up_and_reports_the_status() {
        let stub = Stub::start(vec![Reply::text(503, "down")]).await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        assert!(matches!(err, AuthError::Server { status: 503 }), "{err:?}");
        assert!(stub.request_count() > 1, "a 5xx should have been retried");
    }

    #[tokio::test]
    async fn a_signup_rejection_is_neither_bad_credentials_nor_a_server_fault() {
        let stub = Stub::start(vec![Reply::json(
            422,
            r#"{"code":422,"msg":"User already registered"}"#,
        )])
        .await;
        let err = client(&stub)
            .sign_up("taken@example.com", "pw")
            .await
            .unwrap_err();
        assert!(
            matches!(err, AuthError::Rejected { status: 422 }),
            "{err:?}"
        );
    }

    // -- redaction ----------------------------------------------------------

    #[test]
    fn debug_for_a_session_shows_no_token() {
        let session = Session {
            access_token: "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.secret".into(),
            refresh_token: "v1-refresh-secret".into(),
            user_id: "user-uuid-1".into(),
            expires_at_ms: 1_700_000_000_000,
        };
        let rendered = format!("{session:?}");
        assert!(
            !rendered.contains("eyJhbGciOi"),
            "access token leaked: {rendered}"
        );
        assert!(
            !rendered.contains("v1-refresh-secret"),
            "refresh token leaked: {rendered}"
        );
        assert!(rendered.contains("<redacted>"));
        assert!(rendered.contains("user-uuid-1"));
        assert!(
            rendered.contains("1700000000000"),
            "expiry should stay visible"
        );
    }

    #[test]
    fn debug_for_the_client_shows_no_key_material() {
        let auth = SupabaseAuth::new(CloudConfig {
            url: "https://project.supabase.co".into(),
            anon_key: "anon-secret-looking-key".into(),
        });
        let rendered = format!("{auth:?}");
        assert!(!rendered.contains("anon-secret-looking-key"), "{rendered}");
    }

    #[test]
    fn emails_are_masked_for_logs() {
        assert_eq!(redact_email("alice@example.com"), "a***@example.com");
        assert_eq!(
            redact_email("  bob@sub.example.org "),
            "b***@sub.example.org"
        );
        assert_eq!(redact_email("not-an-email"), "<redacted>");
        assert_eq!(redact_email("@example.com"), "<redacted>");
        assert_eq!(redact_email("alice@"), "<redacted>");
        assert_eq!(redact_email("a@b@c"), "<redacted>");
        assert_eq!(redact_email(""), "<redacted>");
    }

    #[test]
    fn a_session_zeroizes_on_drop() {
        fn assert_zeroize_on_drop<T: ZeroizeOnDrop>() {}
        assert_zeroize_on_drop::<Session>();
    }

    // -- error bodies -------------------------------------------------------

    #[test]
    fn a_structured_gotrue_error_reads_as_its_description() {
        assert_eq!(error_detail(INVALID_GRANT), "Invalid login credentials");
        assert_eq!(
            error_detail(r#"{"msg":"User already registered"}"#),
            "User already registered"
        );
        assert_eq!(
            error_detail(r#"{"error":"invalid_grant"}"#),
            "invalid_grant"
        );
    }

    #[test]
    fn a_gateway_html_page_is_truncated_rather_than_lost() {
        let page = format!("<html>{}</html>", "x".repeat(4000));
        let detail = error_detail(&page);
        assert!(
            detail.len() <= MAX_ERROR_SNIPPET + 4,
            "not truncated: {}",
            detail.len()
        );
        assert!(
            detail.starts_with("<html>"),
            "the snippet should be diagnosable"
        );
        assert!(detail.ends_with('…'));
    }

    #[test]
    fn an_empty_error_body_says_so() {
        assert_eq!(error_detail("   "), "<empty body>");
        // A JSON envelope with nothing usable in it falls back to the raw
        // body: "{}" is uninformative, but it is what the server actually
        // said, and inventing a friendlier string would hide that.
        assert_eq!(error_detail("{}"), "{}");
    }

    #[test]
    fn truncation_lands_on_a_char_boundary() {
        let text = "é".repeat(MAX_ERROR_SNIPPET);
        let detail = truncate(&text);
        assert!(detail.ends_with('…'));
        assert!(detail.is_char_boundary(detail.len() - '…'.len_utf8()));
    }

    // -- no paths in errors -------------------------------------------------

    #[tokio::test]
    async fn a_network_failure_message_carries_no_url_or_path() {
        // Port 1 on loopback refuses immediately.
        let auth = SupabaseAuth::new(CloudConfig {
            url: "http://127.0.0.1:1".into(),
            anon_key: ANON.into(),
        })
        .with_retry_policy(fast_retry());
        let err = auth.sign_in("a@b.co", "x").await.unwrap_err();
        assert!(matches!(err, AuthError::Network(_)), "{err:?}");
        let rendered = format!("{err}");
        assert!(!rendered.contains('/'), "no path-shaped text: {rendered}");
    }
}
