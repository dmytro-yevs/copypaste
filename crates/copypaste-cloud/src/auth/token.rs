//! The GoTrue token response, and the rules for turning one into a session.
//!
//! Both places this can go wrong are here: an expiry that must saturate rather
//! than wrap, and a 200 that is a *user* rather than a session.

use serde::Deserialize;

use super::error::AuthError;
use super::session::Session;

/// The GoTrue token response, and — for a confirmation-gated sign-up — the bare
/// user object that comes back instead.
///
/// No `Debug`: it holds tokens.
#[derive(Deserialize)]
pub(super) struct TokenBody {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    /// Unix *seconds*, sent alongside `expires_in`. Used only as a fallback.
    pub expires_at: Option<i64>,
    pub user: Option<UserBody>,
    /// Present when the response is a user object rather than a session.
    pub id: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct UserBody {
    pub id: Option<String>,
}

impl TokenBody {
    pub(super) fn into_session(self, now_ms: i64) -> Result<Session, AuthError> {
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

#[cfg(test)]
mod tests {
    use super::super::http::now_ms;
    use super::*;

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
}
