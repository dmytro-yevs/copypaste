//! Why an auth call failed — and the one rule that decides it.
//!
//! [`classify`] and [`GrantKind`] live here rather than beside the request
//! helper because the disambiguation *is* the difference between two of these
//! variants. Reading the `400` handling and the variants it produces should not
//! require opening two files.

use reqwest::{Response, StatusCode};

use super::http::{error_detail, retry_after_secs};

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
    pub(super) fn from_reqwest(err: reqwest::Error) -> Self {
        // `without_url` drops the project URL from the message. Nothing secret
        // lives there, but errors are user-facing and the project ref is noise.
        AuthError::Network(err.without_url())
    }

    /// Whether retrying the identical request could plausibly succeed.
    pub(super) fn is_transient(&self) -> bool {
        matches!(self, AuthError::Network(_) | AuthError::Server { .. })
    }
}

/// Which grant we asked for. **This, not the response body, decides how a
/// `400`/`422` is reported** (manifest §4.6.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum GrantKind {
    /// `POST /auth/v1/signup`.
    SignUp,
    /// `grant_type=password` — a rejection means the credentials are wrong.
    Password,
    /// `grant_type=refresh_token` — a rejection means the session is dead.
    Refresh,
    /// A call authenticated by an existing access token (logout).
    Bearer,
}

/// Turn a response into its body text, or into the error the status means.
///
/// `grant` is the only input to the `400`/`422`/`401` decision.
pub(super) async fn classify(response: Response, grant: GrantKind) -> Result<String, AuthError> {
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
// Tests
// ---------------------------------------------------------------------------
//
// Asserted through a real `reqwest` round trip against wiremock: the rule under
// test is what a status does to the *caller*, which includes whether the request
// was retried at all.

#[cfg(test)]
mod tests {
    use super::super::stub::{Reply, Stub};
    use super::super::testkit::{client, fast_retry, session_body, ANON, INVALID_GRANT};
    use super::*;
    use crate::auth::SupabaseAuth;
    use crate::CloudConfig;

    // -- the disambiguation, both directions ------------------------------

    #[tokio::test]
    async fn invalid_grant_on_the_password_grant_is_bad_credentials() {
        let stub = Stub::start(vec![Reply::json(400, INVALID_GRANT)], 1).await;
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
        let stub = Stub::start(vec![Reply::json(400, INVALID_GRANT)], 1).await;
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
            let stub = Stub::start(vec![Reply::json(status, INVALID_GRANT)], 1).await;
            let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
            assert!(matches!(err, AuthError::InvalidCredentials), "{status}");

            let stub = Stub::start(vec![Reply::json(status, INVALID_GRANT)], 1).await;
            let err = client(&stub).refresh("t").await.unwrap_err();
            assert!(matches!(err, AuthError::SessionExpired), "{status}");
        }
    }

    #[tokio::test]
    async fn a_grant_failure_is_not_retried() {
        let stub = Stub::start(vec![Reply::json(400, INVALID_GRANT)], 1).await;
        let _ = client(&stub).sign_in("a@b.co", "x").await;
        assert_eq!(
            stub.request_count().await,
            1,
            "a wrong password is permanent"
        );
    }

    // -- 429 ---------------------------------------------------------------

    #[tokio::test]
    async fn a_429_surfaces_retry_after_in_seconds() {
        let stub = Stub::start(
            vec![Reply::json(429, r#"{"message":"too many requests"}"#)
                .with_header("retry-after", "42")],
            1,
        )
        .await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        match err {
            AuthError::RateLimited { retry_after_secs } => {
                assert_eq!(retry_after_secs, Some(42));
            }
            other => panic!("expected RateLimited, got {other:?}"),
        }
        assert_eq!(
            stub.request_count().await,
            1,
            "a 429 must not be retried here"
        );
    }

    #[tokio::test]
    async fn a_429_without_the_header_still_reports_rate_limiting() {
        let stub = Stub::start(vec![Reply::json(429, "{}")], 1).await;
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

    // -- 5xx and retry ------------------------------------------------------

    #[tokio::test]
    async fn a_5xx_is_transient_and_is_retried() {
        let stub = Stub::start(
            vec![
                Reply::text(502, "<html>bad gateway</html>"),
                Reply::json(200, &session_body("at", "rt", 60)),
            ],
            2,
        )
        .await;
        let session = client(&stub).sign_in("a@b.co", "x").await.expect("sign-in");
        assert_eq!(session.access_token, "at");
        assert_eq!(stub.request_count().await, 2);
    }

    #[tokio::test]
    async fn a_permanent_5xx_gives_up_and_reports_the_status() {
        let stub = Stub::start(vec![Reply::text(503, "down")], 1..).await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        assert!(matches!(err, AuthError::Server { status: 503 }), "{err:?}");
        assert!(
            stub.request_count().await > 1,
            "a 5xx should have been retried"
        );
    }

    #[tokio::test]
    async fn a_signup_rejection_is_neither_bad_credentials_nor_a_server_fault() {
        let stub = Stub::start(
            vec![Reply::json(
                422,
                r#"{"code":422,"msg":"User already registered"}"#,
            )],
            1,
        )
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
