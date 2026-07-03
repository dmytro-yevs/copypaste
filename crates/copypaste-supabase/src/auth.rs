use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use copypaste_ipc::backoff::BackoffScheduler;
use reqwest::{Client, StatusCode};
use tracing::{debug, info, warn};

use crate::error::{AuthError, AuthResult};
use crate::models::{
    GoTrueErrorBody, GoTrueTokenResponse, PasswordGrantRequest, RefreshGrantRequest, Session, User,
};
use crate::store::{InMemoryStore, SessionStore};

// How many seconds before expiry we proactively refresh the token.
const REFRESH_MARGIN_SECS: u64 = 60;

// Floor on the auto-refresh sleep interval. Without it, a short-lived token
// (`expires_in <= REFRESH_MARGIN_SECS`) yields a sleep of 0, producing an
// unthrottled refresh loop that hammers GoTrue. Never sleep below this.
const MIN_REFRESH_INTERVAL_SECS: u64 = 5;

// CopyPaste-vgpy: base/cap for the `BackoffScheduler` driving the
// refresh-failure retry path (see `spawn_auto_refresh`). Base matches the
// old flat retry delay so first-failure behaviour is unchanged; the cap
// bounds how far repeated failures can stretch the interval so a
// long-lasting GoTrue outage doesn't leave the client refreshing every 30 s
// forever, but also doesn't wait unboundedly long once GoTrue recovers.
const REFRESH_BACKOFF_BASE_SECS: u64 = 30;
const REFRESH_BACKOFF_CAP_SECS: u64 = 300;
// Any successful refresh resets the schedule immediately (no minimum "hold"
// duration needed — a successful token refresh is itself the success
// signal), so the threshold value here is never consulted by our call
// pattern (we always call `on_success_held()` right after a success).
const REFRESH_BACKOFF_SUCCESS_HOLD_SECS: u64 = 60;

/// Default HTTP timeout for all GoTrue auth requests.
///
/// Matches `SYNC_HTTP_TIMEOUT` / `REST_HTTP_TIMEOUT` used elsewhere in this
/// workspace (30 s) for consistency.
///
/// # CopyPaste-8ebg.49
///
/// The previous `AuthClient` used `Client::new()` with no timeout, so a
/// stalled GoTrue endpoint could block `sign_in`/`refresh_session` (and thus
/// the auto-refresh loop) indefinitely. The sibling `RestClient` was already
/// fixed for this in CopyPaste-16vr; this constant carries the same guard
/// over to `AuthClient`.
const AUTH_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Which GoTrue token grant a request used. A `400`/`422` means different
/// things per grant — bad password vs. bad refresh token — and the OAuth
/// `invalid_grant` code is shared by both, so the grant is the reliable
/// disambiguator, not the error body.
#[derive(Clone, Copy)]
enum GrantKind {
    Password,
    Refresh,
}

/// Compute the next auto-refresh sleep (in seconds) after a successful refresh,
/// given the freshly-issued token's `expires_in`. Refreshes `REFRESH_MARGIN_SECS`
/// before expiry but never sleeps below [`MIN_REFRESH_INTERVAL_SECS`], so a
/// short-lived token cannot spin the loop.
fn next_refresh_sleep_secs(expires_in: u64) -> u64 {
    expires_in
        .saturating_sub(REFRESH_MARGIN_SECS)
        .max(MIN_REFRESH_INTERVAL_SECS)
}

/// Redact an account email for logging. The email is PII and must never appear
/// verbatim in logs. Keeps the first character of the local part plus the
/// domain (`alice@example.com` → `a***@example.com`); inputs without a usable
/// `@` collapse to `<redacted>`.
fn redact_email(email: &str) -> String {
    match email.split_once('@') {
        Some((local, domain)) if !local.is_empty() && !domain.is_empty() => {
            let first = local.chars().next().unwrap_or('*');
            if local.chars().count() <= 1 {
                format!("*@{domain}")
            } else {
                format!("{first}***@{domain}")
            }
        }
        _ => "<redacted>".to_string(),
    }
}

// ---------------------------------------------------------------------------
// AuthClient
// ---------------------------------------------------------------------------

/// Supabase GoTrue auth client.
///
/// # Example
/// ```no_run
/// use copypaste_supabase::auth::AuthClient;
///
/// #[tokio::main]
/// async fn main() {
///     let client = AuthClient::from_env().unwrap();
///     let session = client.sign_in("user@example.com", "password").await.unwrap();
///     println!("access token: {}", session.access_token);
/// }
/// ```
pub struct AuthClient {
    http: Client,
    base_url: String,
    anon_key: String,
    store: Arc<dyn SessionStore>,
}

impl AuthClient {
    /// Create a client using `SUPABASE_URL` and `SUPABASE_ANON_KEY` environment
    /// variables.  Returns [`AuthError::MissingEnv`] if either is absent.
    pub fn from_env() -> AuthResult<Self> {
        let url = std::env::var("SUPABASE_URL")
            .map_err(|_| AuthError::MissingEnv("SUPABASE_URL".into()))?;
        let key = std::env::var("SUPABASE_ANON_KEY")
            .map_err(|_| AuthError::MissingEnv("SUPABASE_ANON_KEY".into()))?;
        Ok(Self::new(url, key))
    }

    /// Create a client with explicit URL and anonymous key.
    pub fn new(base_url: impl Into<String>, anon_key: impl Into<String>) -> Self {
        Self::with_store(base_url, anon_key, Arc::new(InMemoryStore::new()))
    }

    /// Create a client with a custom [`SessionStore`].
    ///
    /// # CopyPaste-8ebg.49
    ///
    /// Uses a 30 s HTTP timeout (`AUTH_HTTP_TIMEOUT`) so a stalled GoTrue
    /// endpoint cannot block `sign_in`/`refresh_session` indefinitely.
    pub fn with_store(
        base_url: impl Into<String>,
        anon_key: impl Into<String>,
        store: Arc<dyn SessionStore>,
    ) -> Self {
        // TLS cert-store load cannot fail on macOS/Linux in normal operation.
        // Propagate via expect rather than silently falling back to a no-timeout
        // client (which would be worse than aborting on a stalled endpoint).
        let http = Client::builder()
            .timeout(AUTH_HTTP_TIMEOUT)
            .build()
            .expect("reqwest Client::builder should not fail on supported platforms");
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            anon_key: anon_key.into(),
            store,
        }
    }

    // -----------------------------------------------------------------------
    // Public API
    // -----------------------------------------------------------------------

    /// Sign in with email and password.  Returns an active [`Session`].
    ///
    /// The session is automatically saved to the configured store.
    pub async fn sign_in(&self, email: &str, password: &str) -> AuthResult<Session> {
        debug!("signing in as {}", redact_email(email));
        let url = format!("{}/auth/v1/token?grant_type=password", self.base_url);
        let body = PasswordGrantRequest { email, password };

        let raw: GoTrueTokenResponse = self.post_json(&url, &body, GrantKind::Password).await?;
        let session = self.build_session(raw);
        self.store.save(&session);
        info!(
            "signed in as {} (expires_at={})",
            redact_email(email),
            session.expires_at
        );
        Ok(session)
    }

    /// Refresh an existing session using the refresh token.
    ///
    /// The new session replaces the old one in the store.
    pub async fn refresh_session(&self, refresh_token: &str) -> AuthResult<Session> {
        debug!("refreshing session");
        let url = format!("{}/auth/v1/token?grant_type=refresh_token", self.base_url);
        let body = RefreshGrantRequest { refresh_token };

        let raw: GoTrueTokenResponse = self.post_json(&url, &body, GrantKind::Refresh).await?;
        let session = self.build_session(raw);
        self.store.save(&session);
        info!("session refreshed (expires_at={})", session.expires_at);
        Ok(session)
    }

    /// Sign out by invalidating the access token on the server.
    pub async fn sign_out(&self, access_token: &str) -> AuthResult<()> {
        debug!("signing out");
        let url = format!("{}/auth/v1/logout", self.base_url);

        let resp = self
            .http
            .post(&url)
            .header("apikey", &self.anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await?;

        let status = resp.status();
        if status == StatusCode::NO_CONTENT || status == StatusCode::OK {
            self.store.clear();
            info!("signed out");
            return Ok(());
        }

        let code = status.as_u16();
        let message = Self::decode_error_body(resp, "sign-out failed").await;
        Err(AuthError::GoTrue {
            status: code,
            message,
        })
    }

    /// Fetch the currently authenticated user's profile.
    pub async fn get_user(&self, access_token: &str) -> AuthResult<User> {
        debug!("fetching user info");
        let url = format!("{}/auth/v1/user", self.base_url);

        let resp = self
            .http
            .get(&url)
            .header("apikey", &self.anon_key)
            .header("Authorization", format!("Bearer {access_token}"))
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            let user: User = resp.json().await?;
            return Ok(user);
        }

        let code = status.as_u16();
        let message = Self::decode_error_body(resp, "get_user failed").await;
        Err(AuthError::GoTrue {
            status: code,
            message,
        })
    }

    /// Return the session from the store (if any).
    pub fn current_session(&self) -> Option<Session> {
        self.store.load()
    }

    // -----------------------------------------------------------------------
    // Auto-refresh
    // -----------------------------------------------------------------------

    /// Spawn a background Tokio task that refreshes the session automatically
    /// `REFRESH_MARGIN_SECS` seconds before it expires.
    ///
    /// The returned [`tokio::task::JoinHandle`] can be dropped — the task
    /// keeps running.  Abort it when the user signs out.
    ///
    /// # Backoff on refresh failure (CopyPaste-8ebg.59 / CopyPaste-vgpy.59)
    ///
    /// The proactive-refresh-before-expiry timing (`REFRESH_MARGIN_SECS`
    /// countdown, and the `next_refresh_sleep_secs` interval after a
    /// successful refresh) is unchanged. Only the retry-on-*failure* path
    /// now escalates: it is driven by the same
    /// [`copypaste_ipc::backoff::BackoffScheduler`] the sibling Realtime
    /// client already uses, instead of a flat 30 s sleep. The scheduler is
    /// carried across loop iterations, reset on every successful refresh,
    /// and advanced on every failure so repeated GoTrue outages back off
    /// exponentially (30 s, 60 s, 120 s, ... capped at
    /// `REFRESH_BACKOFF_CAP_SECS`) instead of hammering at a fixed interval.
    /// The "no session yet" branch (10 s) is untouched — it is not a
    /// retry-after-failure, just idle polling for a first sign-in.
    pub fn spawn_auto_refresh(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut refresh_backoff = BackoffScheduler::new(
                std::time::Duration::from_secs(REFRESH_BACKOFF_BASE_SECS),
                std::time::Duration::from_secs(REFRESH_BACKOFF_CAP_SECS),
                std::time::Duration::from_secs(REFRESH_BACKOFF_SUCCESS_HOLD_SECS),
            );

            loop {
                let sleep_secs = match self.store.load() {
                    None => {
                        // No session yet — check again in 10 s.
                        10
                    }
                    Some(ref session) => {
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let refresh_at = session.expires_at.saturating_sub(REFRESH_MARGIN_SECS);
                        if now >= refresh_at {
                            // Time to refresh.
                            match self.refresh_session(&session.refresh_token).await {
                                Ok(new) => {
                                    info!("auto-refresh: new expiry = {}", new.expires_at);
                                    // A successful refresh is itself the success
                                    // signal — reset the failure-backoff schedule
                                    // unconditionally so the next failure starts
                                    // from the base delay again.
                                    refresh_backoff.on_success_held();
                                    // Next check in expires_in - margin, floored so a
                                    // short-lived token can't spin the refresh loop.
                                    next_refresh_sleep_secs(new.expires_in)
                                }
                                Err(e) => {
                                    warn!("auto-refresh failed: {e}");
                                    // Back off exponentially instead of a flat
                                    // 30 s retry; advance the schedule for the
                                    // *next* failure per BackoffScheduler's
                                    // documented next_delay()-then-on_failure()
                                    // ordering.
                                    let delay = refresh_backoff.next_delay();
                                    refresh_backoff.on_failure();
                                    delay.as_secs().max(1)
                                }
                            }
                        } else {
                            refresh_at - now
                        }
                    }
                };

                tokio::time::sleep(tokio::time::Duration::from_secs(sleep_secs)).await;
            }
        })
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    /// POST `body` as JSON, parse the success response as `T`, or map GoTrue
    /// error bodies to [`AuthError`].
    ///
    /// `grant` disambiguates a `400`/`422`: the OAuth `invalid_grant` code is
    /// emitted for *both* a bad password (password grant) and a bad refresh
    /// token (refresh grant), so the grant kind — not the error body — decides
    /// which [`AuthError`] variant we return.
    async fn post_json<B, T>(&self, url: &str, body: &B, grant: GrantKind) -> AuthResult<T>
    where
        B: serde::Serialize,
        T: serde::de::DeserializeOwned,
    {
        let resp = self
            .http
            .post(url)
            .header("apikey", &self.anon_key)
            .json(body)
            .send()
            .await?;

        let status = resp.status();
        if status.is_success() {
            let data: T = resp.json().await?;
            return Ok(data);
        }

        // Try to decode a GoTrue error envelope; preserve raw body on JSON
        // parse failure so callers see the actual failure reason.
        let code = status.as_u16();
        let message = Self::decode_error_body(resp, "unknown").await;

        if code == 400 || code == 422 {
            return Err(match grant {
                // The grant kind is authoritative: a refresh grant that 400s
                // means the refresh token is bad/expired; a password grant that
                // 400s means bad credentials. The OAuth `invalid_grant` code is
                // shared by both, so we must NOT guess from the body here.
                GrantKind::Refresh => AuthError::InvalidRefreshToken(message),
                GrantKind::Password => AuthError::InvalidCredentials(message),
            });
        }

        Err(AuthError::GoTrue {
            status: code,
            message,
        })
    }

    /// Decode a GoTrue error response body into a human-readable message.
    ///
    /// Reads the raw response text first so that if JSON parsing fails the
    /// caller still sees a truncated snippet of the actual body (e.g. an HTML
    /// gateway error page) rather than a generic "unknown error".  The raw text
    /// is truncated to 200 bytes to keep log lines manageable.
    async fn decode_error_body(resp: reqwest::Response, fallback: &str) -> String {
        let raw = match resp.text().await {
            Ok(t) => t,
            Err(_) => return fallback.to_owned(),
        };
        // Try structured GoTrue JSON first.
        if let Ok(body) = serde_json::from_str::<GoTrueErrorBody>(&raw) {
            let msg = body.message();
            if msg != "unknown error" {
                return msg;
            }
        }
        // Fall back to a truncated raw snippet so callers can diagnose the
        // actual failure (e.g. "502 Bad Gateway" HTML, misconfigured proxy, etc.).
        let snippet: String = raw.chars().take(200).collect();
        if snippet.is_empty() {
            fallback.to_owned()
        } else {
            snippet
        }
    }

    /// Build a [`Session`] from a raw GoTrue token response, computing
    /// `expires_at` from the current time and `expires_in`.
    fn build_session(&self, raw: GoTrueTokenResponse) -> Session {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Session {
            access_token: raw.access_token,
            refresh_token: raw.refresh_token,
            expires_in: raw.expires_in,
            // Saturating add: a hostile/large `expires_in` must not overflow
            // (panic in debug, wrap in release). A wrap would make the token
            // look already-expired and stall auth forever.
            expires_at: now.saturating_add(raw.expires_in),
            token_type: raw.token_type,
            user: raw.user,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_refresh_sleep_is_floored() {
        // Long-lived token: sleep is expires_in - margin.
        assert_eq!(
            next_refresh_sleep_secs(3600),
            3600 - REFRESH_MARGIN_SECS,
            "long-lived token should refresh margin-seconds before expiry"
        );
        // Exactly at the margin → would be 0; must be floored.
        assert_eq!(
            next_refresh_sleep_secs(REFRESH_MARGIN_SECS),
            MIN_REFRESH_INTERVAL_SECS,
            "expires_in == margin must not yield a zero sleep"
        );
        // Short-lived / hostile tiny token → saturates to 0 then floored.
        assert_eq!(next_refresh_sleep_secs(10), MIN_REFRESH_INTERVAL_SECS);
        assert_eq!(next_refresh_sleep_secs(0), MIN_REFRESH_INTERVAL_SECS);
        // Never below the floor.
        for expires_in in 0..=REFRESH_MARGIN_SECS + MIN_REFRESH_INTERVAL_SECS {
            assert!(
                next_refresh_sleep_secs(expires_in) >= MIN_REFRESH_INTERVAL_SECS,
                "sleep dropped below floor for expires_in={expires_in}"
            );
        }
    }

    #[test]
    fn build_session_expires_at_saturates_on_overflow() {
        // A hostile expires_in near u64::MAX must not overflow expires_at;
        // it should saturate rather than wrap (which would look expired).
        let client = AuthClient::new("https://example.com", "anon");
        let raw = GoTrueTokenResponse {
            access_token: "a".into(),
            refresh_token: "r".into(),
            expires_in: u64::MAX,
            token_type: "bearer".into(),
            user: User {
                id: "u".into(),
                email: None,
                role: None,
                created_at: None,
                updated_at: None,
            },
        };
        let session = client.build_session(raw);
        assert_eq!(session.expires_at, u64::MAX, "expires_at must saturate");
        // Saturated expiry is far in the future, not already-expired.
        assert!(!session.is_expired_with_margin(REFRESH_MARGIN_SECS));
    }

    #[test]
    fn redact_email_masks_pii() {
        assert_eq!(redact_email("alice@example.com"), "a***@example.com");
        assert_eq!(redact_email("a@example.com"), "*@example.com");
        assert_eq!(redact_email("not-an-email"), "<redacted>");
        assert_eq!(redact_email("@x.com"), "<redacted>");
        assert_eq!(redact_email(""), "<redacted>");
        let r = redact_email("dmitriy.evseev.99@gmail.com");
        assert!(!r.contains("evseev"), "local part leaked: {r}");
    }

    #[test]
    fn session_debug_redacts_tokens() {
        let s = Session {
            access_token: "supersecret-access".to_owned(),
            refresh_token: "supersecret-refresh".to_owned(),
            expires_in: 3600,
            expires_at: 9999,
            token_type: "bearer".to_owned(),
            user: User {
                id: "u1".to_owned(),
                email: Some("a@example.com".to_owned()),
                role: None,
                created_at: None,
                updated_at: None,
            },
        };
        let dbg = format!("{s:?}");
        assert!(
            !dbg.contains("supersecret-access"),
            "access token leaked: {dbg}"
        );
        assert!(
            !dbg.contains("supersecret-refresh"),
            "refresh token leaked: {dbg}"
        );
        assert!(
            dbg.contains("<redacted>"),
            "expected redaction marker: {dbg}"
        );
        // Non-secret fields are still visible for debugging.
        assert!(dbg.contains("9999"), "expires_at should be visible: {dbg}");
    }

    // ── error-body raw-text preservation ──────────────────────────────────────

    /// When the GoTrue error body cannot be decoded as JSON, the raw response
    /// text (truncated) must appear in the error message so the caller can
    /// diagnose the real failure rather than seeing a generic "unknown error".
    ///
    /// This test uses mockito 0.31 (module-level API) to return a non-JSON body
    /// with a 400 status.
    #[tokio::test]
    #[serial_test::serial]
    async fn post_json_preserves_raw_body_on_json_decode_failure() {
        let _mock = mockito::mock("POST", "/auth/v1/token?grant_type=password")
            .with_status(400)
            .with_header("content-type", "text/plain")
            .with_body("This is not JSON at all: unexpected gateway response")
            .create();

        let client = AuthClient::new(mockito::server_url(), "anon-key");
        let result = client.sign_in("user@example.com", "bad-password").await;

        let err = result.expect_err("should fail with a 400");
        let err_str = err.to_string();
        assert!(
            err_str.contains("unexpected gateway response") || err_str.contains("This is not JSON"),
            "error message should include raw body snippet, got: {err_str}"
        );
    }

    /// When the GoTrue error body IS valid JSON, the structured message is used.
    #[tokio::test]
    #[serial_test::serial]
    async fn post_json_uses_json_message_when_valid() {
        let _mock = mockito::mock("POST", "/auth/v1/token?grant_type=password")
            .with_status(400)
            .with_header("content-type", "application/json")
            .with_body(
                r#"{"error":"invalid_grant","error_description":"Invalid login credentials"}"#,
            )
            .create();

        let client = AuthClient::new(mockito::server_url(), "anon-key");
        let result = client.sign_in("user@example.com", "bad-password").await;

        let err = result.expect_err("should fail with a 400");
        let err_str = err.to_string();
        assert!(
            err_str.contains("Invalid login credentials"),
            "structured error_description should appear in error, got: {err_str}"
        );
    }
}
