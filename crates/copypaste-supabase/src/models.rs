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
        // saturating_add prevents u64 overflow when margin_secs is very large
        // (e.g. u64::MAX in tests or a misconfigured caller).
        now.saturating_add(margin_secs) >= self.expires_at
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_expired_with_margin ─────────────────────────────────────────────────

    /// u64::MAX + any margin_secs must not panic (saturating_add, not wrapping add).
    #[test]
    fn is_expired_with_margin_does_not_overflow_on_max_margin() {
        // A session that expires far in the future (max u64) should not panic
        // even when called with margin_secs = u64::MAX.
        let session = Session {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_in: 3600,
            expires_at: u64::MAX,
            token_type: "bearer".into(),
            user: User {
                id: "u".into(),
                email: None,
                role: None,
                created_at: None,
                updated_at: None,
            },
        };
        // Must not panic — saturating_add prevents overflow.
        let result = session.is_expired_with_margin(u64::MAX);
        // now (real time) << u64::MAX, so (now).saturating_add(u64::MAX) == u64::MAX == expires_at
        // so the result is "expired" (>= boundary). The important thing is it doesn't panic.
        let _ = result; // panic-free is the assertion
    }

    /// A session expiring exactly at now+margin should be considered expired.
    #[test]
    fn is_expired_with_margin_zero_margin_expired_in_past() {
        let session = Session {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_in: 0,
            // expires_at = 0 means it expired at the Unix epoch — definitely expired
            expires_at: 0,
            token_type: "bearer".into(),
            user: User {
                id: "u".into(),
                email: None,
                role: None,
                created_at: None,
                updated_at: None,
            },
        };
        assert!(
            session.is_expired_with_margin(0),
            "session expiring at epoch should always be expired"
        );
    }

    /// A session expiring far in the future should NOT be considered expired.
    #[test]
    fn is_expired_with_margin_future_session_not_expired() {
        let session = Session {
            access_token: "tok".into(),
            refresh_token: "ref".into(),
            expires_in: 3600,
            // Year 2100 in Unix seconds — well in the future
            expires_at: 4_102_444_800,
            token_type: "bearer".into(),
            user: User {
                id: "u".into(),
                email: None,
                role: None,
                created_at: None,
                updated_at: None,
            },
        };
        // With a 60-second margin, a session expiring in 2100 is not expired.
        assert!(
            !session.is_expired_with_margin(60),
            "session expiring in 2100 should not be expired"
        );
    }
}
