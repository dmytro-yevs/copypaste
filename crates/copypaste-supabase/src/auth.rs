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
        debug!("signing in as {email}");
        let url = format!("{}/auth/v1/token?grant_type=password", self.base_url);
        let body = PasswordGrantRequest { email, password };

        let raw: GoTrueTokenResponse = self.post_json(&url, &body).await?;
        let session = self.build_session(raw);
        self.store.save(&session);
        info!("signed in as {email} (expires_at={})", session.expires_at);
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
        let body: GoTrueErrorBody = resp.json().await.unwrap_or(GoTrueErrorBody {
            error: Some("sign-out failed".into()),
            error_description: None,
            msg: None,
            message: None,
        });
        Err(AuthError::GoTrue {
            status: code,
            message: body.message(),
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
        let body: GoTrueErrorBody = resp.json().await.unwrap_or(GoTrueErrorBody {
            error: Some("get_user failed".into()),
            error_description: None,
            msg: None,
            message: None,
        });
        Err(AuthError::GoTrue {
            status: code,
            message: body.message(),
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
                        let refresh_at = session
                            .expires_at
                            .saturating_sub(REFRESH_MARGIN_SECS);
                        if now >= refresh_at {
                            // Time to refresh.
                            match self.refresh_session(&session.refresh_token).await {
                                Ok(new) => {
                                    info!(
                                        "auto-refresh: new expiry = {}",
                                        new.expires_at
                                    );
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

        // Try to decode a GoTrue error envelope.
        let code = status.as_u16();
        let body: GoTrueErrorBody = resp.json().await.unwrap_or(GoTrueErrorBody {
            error: Some("unknown".into()),
            error_description: None,
            msg: None,
            message: None,
        });
        let message = body.message();

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
