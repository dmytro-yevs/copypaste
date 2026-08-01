//! What a failed PostgREST call means, and how a status becomes it.
//!
//! The enum and the mapping live together because the mapping *is* the
//! contract: which statuses the caller must not retry, which one means the
//! deployment is missing an index, and which are transient. Splitting the
//! `match` away from the variants it produces is how a new variant ends up
//! never being constructed.

use reqwest::{Response, StatusCode};

use crate::auth::retry_after_secs;

/// Why a PostgREST call failed.
#[derive(Debug, thiserror::Error)]
pub enum RestError {
    /// `401` — the access token was refused.
    ///
    /// Distinct on purpose: this is the caller's cue to refresh **once** and
    /// retry **once**. It is never retried inside this module.
    #[error("the access token was refused")]
    Unauthorized,

    /// `403` — authenticated, but row-level security refused. Refreshing the
    /// token cannot help; the deployment or the row is wrong.
    #[error("the account is not allowed to touch these rows")]
    Forbidden,

    /// `429`, with `Retry-After` in its delta-seconds form when present.
    #[error("the backend is rate limiting this client")]
    RateLimited { retry_after_secs: Option<u64> },

    /// The request never got an answer. The URL is stripped before the error is
    /// kept.
    #[error("could not reach the sync backend")]
    Network(#[source] reqwest::Error),

    /// `5xx`. Transient: retried under the shared policy before it surfaces.
    #[error("the sync backend returned status {status}")]
    Server { status: u16 },

    /// A `4xx` that is not 401, 403 or 429 — most usefully a `409`, which means
    /// the unique index behind [`CONFLICT_TARGET`](super::CONFLICT_TARGET) is
    /// missing from the deployment.
    #[error("the sync backend rejected the request (status {status})")]
    Rejected { status: u16 },

    /// A 2xx whose body was not the shape PostgREST promises, or a row whose
    /// base64 does not decode.
    #[error("the sync backend returned an unexpected response")]
    Malformed,

    /// A client-side precondition failed and nothing was sent. `reason` is a
    /// `&'static str` written in this module, so it cannot carry a path, a
    /// token or user content.
    #[error("item rejected before sending: {reason}")]
    InvalidItem { reason: &'static str },
}

impl RestError {
    pub(super) fn from_reqwest(err: reqwest::Error) -> Self {
        RestError::Network(err.without_url())
    }

    pub(super) fn is_transient(&self) -> bool {
        matches!(self, RestError::Network(_) | RestError::Server { .. })
    }
}

/// Map a response to its body, or to the error its status means.
pub(super) async fn classify(response: Response) -> Result<String, RestError> {
    let status = response.status();
    if status.is_success() {
        return response.text().await.map_err(|_| RestError::Malformed);
    }

    let retry_after = retry_after_secs(response.headers());
    let code = status.as_u16();
    let detail = truncate_body(&response.text().await.unwrap_or_default());

    match status {
        StatusCode::UNAUTHORIZED => {
            tracing::debug!(status = code, "access token refused");
            Err(RestError::Unauthorized)
        }
        StatusCode::FORBIDDEN => {
            tracing::warn!(status = code, detail = %detail, "row-level security refused the request");
            Err(RestError::Forbidden)
        }
        StatusCode::TOO_MANY_REQUESTS => {
            tracing::warn!(status = code, retry_after_secs = ?retry_after, "rate limited");
            Err(RestError::RateLimited {
                retry_after_secs: retry_after,
            })
        }
        StatusCode::CONFLICT => {
            tracing::error!(
                status = code,
                detail = %detail,
                "upsert conflict did not resolve — the deployment is probably missing the \
                 unique index on (user_id, item_id)"
            );
            Err(RestError::Rejected { status: code })
        }
        _ if status.is_server_error() => {
            tracing::warn!(status = code, detail = %detail, "sync backend faulted");
            Err(RestError::Server { status: code })
        }
        _ => {
            tracing::warn!(status = code, detail = %detail, "sync backend rejected the request");
            Err(RestError::Rejected { status: code })
        }
    }
}

/// A short snippet of an error body, for logs only. PostgREST error bodies are
/// JSON with `message`/`details`/`hint`, but a proxy in front of it can return
/// anything, so this does not assume a shape.
fn truncate_body(body: &str) -> String {
    const MAX: usize = 200;
    let body = body.trim();
    if body.is_empty() {
        return "<empty body>".to_string();
    }
    if body.len() <= MAX {
        return body.to_string();
    }
    let mut end = MAX;
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &body[..end])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// The status rules are asserted through a real `reqwest` round trip against
// wiremock, because the thing under test is what a status *does* to the retry
// envelope as much as which variant it produces.

#[cfg(test)]
mod tests {
    use super::super::testkit::{client, fast_retry, item, ANON, TOKEN};
    use super::*;
    use crate::auth::stub::{Reply, Stub};
    use crate::rest::SupabaseRest;
    use crate::CloudConfig;

    #[tokio::test]
    async fn a_401_is_distinct_and_is_not_retried() {
        let stub = Stub::start(vec![Reply::json(401, r#"{"message":"JWT expired"}"#)], 1).await;
        let err = client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::Unauthorized), "{err:?}");
        assert_eq!(
            stub.request_count().await,
            1,
            "the caller refreshes once and retries once; retrying here would spin"
        );
    }

    #[tokio::test]
    async fn a_401_on_the_write_path_is_the_same_error() {
        let stub = Stub::start(vec![Reply::json(401, "{}")], 1).await;
        let err = client(&stub)
            .upsert(TOKEN, &[item("a1")])
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::Unauthorized), "{err:?}");
        assert_eq!(stub.request_count().await, 1);
    }

    #[tokio::test]
    async fn a_403_is_not_confused_with_a_401() {
        let stub = Stub::start(vec![Reply::json(403, r#"{"message":"RLS"}"#)], 1).await;
        let err = client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .unwrap_err();
        assert!(
            matches!(err, RestError::Forbidden),
            "refreshing a token cannot fix a policy refusal: {err:?}"
        );
    }

    #[tokio::test]
    async fn a_429_carries_retry_after_and_is_not_slept_on_here() {
        let stub = Stub::start(
            vec![Reply::json(429, "{}").with_header("retry-after", "17")],
            1,
        )
        .await;
        let err = client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .unwrap_err();
        match err {
            RestError::RateLimited { retry_after_secs } => assert_eq!(retry_after_secs, Some(17)),
            other => panic!("expected RateLimited, got {other:?}"),
        }
        assert_eq!(stub.request_count().await, 1);
    }

    #[tokio::test]
    async fn a_429_without_a_header_still_says_rate_limited() {
        let stub = Stub::start(vec![Reply::json(429, "{}")], 1).await;
        let err = client(&stub)
            .upsert(TOKEN, &[item("a1")])
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                RestError::RateLimited {
                    retry_after_secs: None
                }
            ),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn a_5xx_is_retried_and_then_succeeds() {
        let stub = Stub::start(vec![Reply::text(503, "down"), Reply::json(200, "[]")], 2).await;
        let rows = client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .expect("fetch");
        assert!(rows.is_empty());
        assert_eq!(stub.request_count().await, 2);
    }

    #[tokio::test]
    async fn a_persistent_5xx_gives_up_with_the_status() {
        let stub = Stub::start(vec![Reply::text(500, "boom")], 1..).await;
        let err = client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .unwrap_err();
        assert!(matches!(err, RestError::Server { status: 500 }), "{err:?}");
        assert!(stub.request_count().await > 1);
    }

    #[tokio::test]
    async fn a_409_says_the_conflict_target_did_not_resolve() {
        let stub = Stub::start(
            vec![Reply::json(
                409,
                r#"{"code":"23505","message":"duplicate key value violates unique constraint"}"#,
            )],
            1,
        )
        .await;
        let err = client(&stub)
            .upsert(TOKEN, &[item("a1")])
            .await
            .unwrap_err();
        assert!(
            matches!(err, RestError::Rejected { status: 409 }),
            "{err:?}"
        );
        assert_eq!(stub.request_count().await, 1, "a 409 is not transient");
    }

    // -- no paths, no tokens ------------------------------------------------

    #[tokio::test]
    async fn errors_carry_neither_a_path_nor_a_token() {
        let stub = Stub::start(
            vec![Reply::json(
                401,
                r#"{"message":"failed at /Users/dmytro/copypaste.db with token user-access-token"}"#,
            )],
            1,
        )
        .await;
        let err = client(&stub)
            .fetch_since(TOKEN, 0, None, 10)
            .await
            .unwrap_err();
        let rendered = format!("{err} {err:?}");
        assert!(!rendered.contains("/Users/"), "path leaked: {rendered}");
        assert!(!rendered.contains(TOKEN), "token leaked: {rendered}");
    }

    #[tokio::test]
    async fn a_network_failure_reports_no_url() {
        let rest = SupabaseRest::new(CloudConfig {
            url: "http://127.0.0.1:1".into(),
            anon_key: ANON.into(),
        })
        .with_retry_policy(fast_retry());
        let err = rest.fetch_since(TOKEN, 0, None, 10).await.unwrap_err();
        assert!(matches!(err, RestError::Network(_)), "{err:?}");
        assert!(!format!("{err}").contains('/'));
    }

    #[test]
    fn an_error_body_is_truncated_for_the_log() {
        let long = "y".repeat(4000);
        let detail = truncate_body(&long);
        assert!(detail.len() <= 204);
        assert!(detail.ends_with('…'));
        assert_eq!(truncate_body("  "), "<empty body>");
    }
}
