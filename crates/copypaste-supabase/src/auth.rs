use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::{Client, StatusCode};
use tracing::{debug, info, warn};

use crate::error::{AuthError, AuthResult};
use crate::models::{
    GoTrueErrorBody, GoTrueTokenResponse, PasswordGrantRequest, RefreshGrantRequest, Session, User,
};
use crate::store::{InMemoryStore, SessionStore};

// How many seconds before expiry we proactively refresh the token.
const REFRESH_MARGIN_SECS: u64 = 60;

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
    pub fn with_store(
        base_url: impl Into<String>,
        anon_key: impl Into<String>,
        store: Arc<dyn SessionStore>,
    ) -> Self {
        Self {
            http: Client::new(),
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

        let raw: GoTrueTokenResponse = self.post_json(&url, &body).await?;
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

        let raw: GoTrueTokenResponse = self.post_json(&url, &body).await?;
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
    pub fn spawn_auto_refresh(self: Arc<Self>) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
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
                                    // Next check in expires_in - margin.
                                    new.expires_in.saturating_sub(REFRESH_MARGIN_SECS)
                                }
                                Err(e) => {
                                    warn!("auto-refresh failed: {e}");
                                    // Back-off 30 s and try again.
                                    30
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
    async fn post_json<B, T>(&self, url: &str, body: &B) -> AuthResult<T>
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
            // Check for refresh-token errors specifically.
            if message.to_lowercase().contains("refresh_token") {
                return Err(AuthError::InvalidRefreshToken(message));
            }
            return Err(AuthError::InvalidCredentials(message));
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
            expires_at: now + raw.expires_in,
            token_type: raw.token_type,
            user: raw.user,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
