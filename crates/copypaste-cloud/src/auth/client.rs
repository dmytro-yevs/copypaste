//! The four endpoints, and the one request helper they all go through.
//!
//! Each call names the [`GrantKind`] it is asking for, and that name is what
//! [`classify`] uses to tell a wrong password from a dead refresh token. It is
//! an argument rather than an inference precisely so that adding a fifth call
//! forces the decision to be made.

use std::fmt;
use std::time::Duration;

use backon::{ExponentialBuilder, Retryable};
use reqwest::Client;
use serde_json::json;
use url::Url;

use super::error::{classify, AuthError, GrantKind};
use super::http::{now_ms, redact_email, transient_backoff};
use super::session::Session;
use super::token::TokenBody;
use crate::CloudConfig;

/// Per-request timeout on every auth call.
///
/// Applied per request rather than on the `Client`, so there is no fallible
/// client builder and therefore no tempting "fall back to a client with no
/// timeout" branch — the failure mode behind manifest bug `CopyPaste-16vr`.
/// Without a timeout a stalled GoTrue endpoint blocks the refresh loop forever
/// (`CopyPaste-8ebg.49`).
pub const AUTH_TIMEOUT: Duration = Duration::from_secs(30);

/// Talks to one Supabase project's GoTrue endpoints.
#[derive(Clone)]
pub struct SupabaseAuth {
    config: CloudConfig,
    http: Client,
    retry: ExponentialBuilder,
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
    pub fn with_retry_policy(mut self, policy: ExponentialBuilder) -> Self {
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
                &self.endpoint("/auth/v1/signup", &[]),
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
                &self.endpoint("/auth/v1/token", &[("grant_type", "password")]),
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
                &self.endpoint("/auth/v1/token", &[("grant_type", "refresh_token")]),
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
                &self.endpoint("/auth/v1/logout", &[]),
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

    fn endpoint(&self, path: &str, query: &[(&str, &str)]) -> String {
        let Ok(mut endpoint) = Url::parse(&self.config.url).and_then(|base| base.join(path)) else {
            // Preserve the existing failure mode for an invalid configuration:
            // hand the raw value to reqwest, which reports a redacted network
            // error rather than panicking while constructing the client.
            return self.config.url.clone();
        };
        if !query.is_empty() {
            endpoint
                .query_pairs_mut()
                .extend_pairs(query.iter().copied());
        }
        endpoint.into()
    }

    /// One request, retried under the shared policy, classified by `grant`.
    async fn post(
        &self,
        url: &str,
        body: &serde_json::Value,
        bearer: &str,
        grant: GrantKind,
    ) -> Result<String, AuthError> {
        let attempt = || async {
            let sent = self
                .http
                .post(url)
                .timeout(AUTH_TIMEOUT)
                .header("apikey", self.config.anon_key.as_str())
                .bearer_auth(bearer)
                .json(body)
                .send()
                .await;

            match sent {
                Ok(response) => classify(response, grant).await,
                Err(err) => Err(AuthError::from_reqwest(err)),
            }
        };

        attempt
            .retry(self.retry)
            .when(AuthError::is_transient)
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::stub::{header as request_header, json as request_json, Reply, Stub};
    use super::super::testkit::{client, fast_retry, session_body, ANON, INVALID_GRANT};
    use super::*;
    use wiremock::matchers::{
        body_json, header as header_match, method, path, query_param, query_param_is_missing,
    };
    use wiremock::Mock;

    // -- endpoints, headers, bodies ---------------------------------------

    #[tokio::test]
    async fn sign_in_posts_the_password_grant_with_both_auth_headers() {
        let mock = Mock::given(method("POST"))
            .and(path("/auth/v1/token"))
            .and(query_param("grant_type", "password"))
            .and(header_match("apikey", ANON))
            .and(header_match("authorization", format!("Bearer {ANON}")))
            .and(body_json(json!({
                "email": "alice@example.com",
                "password": "hunter2"
            })));
        let stub = Stub::start_matching(
            mock,
            vec![Reply::json(200, &session_body("at", "rt", 3600))],
            1,
        )
        .await;
        client(&stub)
            .sign_in("alice@example.com", "hunter2")
            .await
            .expect("sign-in");

        let request = stub.only_request().await;
        assert_eq!(request.method.as_str(), "POST");
        assert_eq!(request.url.path(), "/auth/v1/token");
        assert_eq!(request.url.query(), Some("grant_type=password"));
        assert_eq!(request_header(&request, "apikey"), Some(ANON));
        assert_eq!(
            request_header(&request, "authorization"),
            Some(format!("Bearer {ANON}").as_str())
        );
        assert_eq!(request_json(&request)["email"], "alice@example.com");
        assert_eq!(request_json(&request)["password"], "hunter2");
    }

    #[tokio::test]
    async fn refresh_posts_only_the_refresh_token() {
        let mock = Mock::given(method("POST"))
            .and(path("/auth/v1/token"))
            .and(query_param("grant_type", "refresh_token"))
            .and(body_json(json!({ "refresh_token": "rt1" })));
        let stub = Stub::start_matching(
            mock,
            vec![Reply::json(200, &session_body("at2", "rt2", 3600))],
            1,
        )
        .await;
        client(&stub).refresh("rt1").await.expect("refresh");

        let request = stub.only_request().await;
        assert_eq!(request.url.path(), "/auth/v1/token");
        assert_eq!(request.url.query(), Some("grant_type=refresh_token"));
        let body = request_json(&request);
        assert_eq!(body["refresh_token"], "rt1");
        assert!(body.get("password").is_none(), "no password on a refresh");
    }

    #[tokio::test]
    async fn sign_up_posts_to_the_signup_endpoint() {
        let mock = Mock::given(method("POST"))
            .and(path("/auth/v1/signup"))
            .and(query_param_is_missing("grant_type"));
        let stub = Stub::start_matching(
            mock,
            vec![Reply::json(200, &session_body("at", "rt", 60))],
            1,
        )
        .await;
        client(&stub)
            .sign_up("new@example.com", "pw")
            .await
            .expect("sign-up");
        let request = stub.only_request().await;
        assert_eq!(request.url.path(), "/auth/v1/signup");
        assert!(request.url.query().is_none());
    }

    #[tokio::test]
    async fn sign_out_bearers_the_access_token_not_the_anon_key() {
        let mock = Mock::given(method("POST"))
            .and(path("/auth/v1/logout"))
            .and(header_match("apikey", ANON))
            .and(header_match("authorization", "Bearer user-access-token"));
        let stub = Stub::start_matching(mock, vec![Reply::empty(204)], 1).await;
        client(&stub)
            .sign_out("user-access-token")
            .await
            .expect("sign-out");

        let request = stub.only_request().await;
        assert_eq!(request.url.path(), "/auth/v1/logout");
        assert_eq!(request_header(&request, "apikey"), Some(ANON));
        assert_eq!(
            request_header(&request, "authorization"),
            Some("Bearer user-access-token")
        );
    }

    #[tokio::test]
    async fn sign_out_of_an_already_dead_token_succeeds() {
        let stub = Stub::start(vec![Reply::json(401, INVALID_GRANT)], 1).await;
        client(&stub)
            .sign_out("stale")
            .await
            .expect("signing out a dead token is not a failure");
    }

    #[tokio::test]
    async fn a_trailing_slash_on_the_project_url_does_not_double_up() {
        let stub = Stub::start(vec![Reply::json(200, &session_body("at", "rt", 1))], 1).await;
        let auth = SupabaseAuth::new(CloudConfig {
            url: format!("{}/", stub.base_url),
            anon_key: ANON.to_string(),
        })
        .with_retry_policy(fast_retry());
        auth.sign_in("a@b.co", "x").await.expect("sign-in");
        let request = stub.only_request().await;
        assert_eq!(request.url.path(), "/auth/v1/token");
        assert_eq!(request.url.query(), Some("grant_type=password"));
    }

    #[test]
    fn endpoint_join_preserves_ipv6_and_escapes_query_values() {
        let auth = SupabaseAuth::new(CloudConfig {
            url: "https://[2001:db8::1]:8443/nested%20base/?stale=true#old".into(),
            anon_key: ANON.into(),
        });
        assert_eq!(
            auth.endpoint("/auth/v1/token", &[("grant_type", "refresh token/+")],),
            "https://[2001:db8::1]:8443/auth/v1/token?grant_type=refresh+token%2F%2B"
        );
    }

    // -- session construction ---------------------------------------------

    #[tokio::test]
    async fn a_session_carries_the_tokens_and_a_computed_expiry() {
        let stub = Stub::start(vec![Reply::json(200, &session_body("acc", "ref", 3600))], 1).await;
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

    #[tokio::test]
    async fn a_signup_awaiting_confirmation_is_not_reported_as_malformed() {
        let stub = Stub::start(
            vec![Reply::json(
                200,
                r#"{"id":"user-uuid-9","email":"new@example.com","confirmation_sent_at":"2026-07-30T00:00:00Z"}"#,
            )],
            1,
        )
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
        let stub = Stub::start(vec![Reply::json(200, r#"{"nonsense":true}"#)], 1).await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        assert!(matches!(err, AuthError::Malformed), "{err:?}");
    }

    #[tokio::test]
    async fn a_non_json_2xx_is_malformed_not_a_panic() {
        let stub = Stub::start(vec![Reply::text(200, "<html>hello</html>")], 1).await;
        let err = client(&stub).sign_in("a@b.co", "x").await.unwrap_err();
        assert!(matches!(err, AuthError::Malformed), "{err:?}");
    }

    // -- redaction ----------------------------------------------------------

    #[test]
    fn debug_for_the_client_shows_no_key_material() {
        let auth = SupabaseAuth::new(CloudConfig {
            url: "https://project.supabase.co".into(),
            anon_key: "anon-secret-looking-key".into(),
        });
        let rendered = format!("{auth:?}");
        assert!(!rendered.contains("anon-secret-looking-key"), "{rendered}");
    }
}
