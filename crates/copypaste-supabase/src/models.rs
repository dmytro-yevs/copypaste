use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// GoTrue response types
// ---------------------------------------------------------------------------

/// A GoTrue user object (subset of fields).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    pub id: String,
    pub email: Option<String>,
    pub role: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

/// An active auth session holding both tokens.
///
/// `Debug` is implemented manually to **redact the access and refresh tokens** —
/// these are bearer secrets and must never reach logs, error payloads, or panic
/// messages. Derived `Debug` would print them verbatim.
#[derive(Clone, Serialize, Deserialize)]
pub struct Session {
    pub access_token: String,
    pub refresh_token: String,
    /// Seconds until the access token expires (from the time it was issued).
    pub expires_in: u64,
    /// Absolute Unix timestamp (seconds) when the access token expires.
    /// Computed locally from `expires_in` at the moment the session is created.
    pub expires_at: u64,
    pub token_type: String,
    pub user: User,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("expires_in", &self.expires_in)
            .field("expires_at", &self.expires_at)
            .field("token_type", &self.token_type)
            .field("user", &self.user)
            .finish()
    }
}

impl Session {
    /// Returns `true` when the access token has expired (or will expire within
    /// the provided `margin_secs` seconds).
    pub fn is_expired_with_margin(&self, margin_secs: u64) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        now + margin_secs >= self.expires_at
    }
}

// ---------------------------------------------------------------------------
// GoTrue request / response shapes (crate-internal)
// ---------------------------------------------------------------------------

/// Body sent to `POST /auth/v1/token?grant_type=password`.
#[derive(Debug, Serialize)]
pub(crate) struct PasswordGrantRequest<'a> {
    pub email: &'a str,
    pub password: &'a str,
}

/// Body sent to `POST /auth/v1/token?grant_type=refresh_token`.
#[derive(Debug, Serialize)]
pub(crate) struct RefreshGrantRequest<'a> {
    pub refresh_token: &'a str,
}

/// Raw GoTrue token response (both grant types share this shape).
#[derive(Debug, Deserialize)]
pub(crate) struct GoTrueTokenResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
    pub user: User,
}

/// GoTrue error body.
#[derive(Debug, Deserialize)]
pub(crate) struct GoTrueErrorBody {
    pub error: Option<String>,
    pub error_description: Option<String>,
    pub msg: Option<String>,
    pub message: Option<String>,
}

impl GoTrueErrorBody {
    /// Best-effort human-readable message from any of the possible fields.
    pub fn message(&self) -> String {
        self.error_description
            .clone()
            .or_else(|| self.message.clone())
            .or_else(|| self.msg.clone())
            .or_else(|| self.error.clone())
            .unwrap_or_else(|| "unknown error".into())
    }
}
