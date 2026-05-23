//! Beta-bonus: auth token refresh-on-401 behavior.
//!
//! Models the documented refresh flow when a protected GoTrue call returns 401:
//!   1. Client calls `get_user(old_access_token)` → server returns 401 "JWT expired"
//!   2. Caller invokes `refresh_session(refresh_token)` → server returns new token
//!   3. Caller retries `get_user(new_access_token)` → 200 OK
//!
//! `AuthClient` does not auto-retry today (a deliberate design choice — the
//! caller drives the refresh). These tests pin the contract that *all the
//! pieces a caller needs* are present and correct:
//!   * `get_user` surfaces a `GoTrue { status: 401, .. }` error
//!   * `refresh_session` returns a brand-new `access_token` + `refresh_token`
//!   * The refreshed `access_token` is then accepted on the retry
//!
//! Uses mockito 0.31 global server API.

use copypaste_supabase::{AuthClient, AuthError};
use mockito::Matcher;

// ---------------------------------------------------------------------------
// Test bodies
// ---------------------------------------------------------------------------

fn refreshed_token_body() -> &'static str {
    r#"{
  "access_token": "new.access.token",
  "refresh_token": "rt-NEW",
  "expires_in": 3600,
  "token_type": "bearer",
  "user": {
    "id": "user-uuid-1234",
    "email": "user@example.com",
    "role": "authenticated",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
  }
}"#
}

fn user_body() -> &'static str {
    r#"{
  "id": "user-uuid-1234",
  "email": "user@example.com",
  "role": "authenticated",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-01T00:00:00Z"
}"#
}

fn server_url() -> String {
    mockito::server_url()
}

// ---------------------------------------------------------------------------
// 1. get_user → 401 surfaces as GoTrue { status: 401, .. }
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_user_returns_401_when_token_expired() {
    let _m = mockito::mock("GET", "/auth/v1/user")
        .match_header("authorization", "Bearer expired.token")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message": "JWT expired"}"#)
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let err = client
        .get_user("expired.token")
        .await
        .expect_err("expired token must surface as error");

    assert!(
        matches!(err, AuthError::GoTrue { status: 401, .. }),
        "expected GoTrue 401, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// 2. refresh_session returns a fresh access_token + refresh_token
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_session_after_401_yields_new_tokens() {
    let _m = mockito::mock("POST", "/auth/v1/token?grant_type=refresh_token")
        .match_body(Matcher::PartialJsonString(
            r#"{"refresh_token": "rt-OLD"}"#.to_string(),
        ))
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(refreshed_token_body())
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let session = client
        .refresh_session("rt-OLD")
        .await
        .expect("refresh must succeed");

    assert_eq!(session.access_token, "new.access.token");
    assert_eq!(session.refresh_token, "rt-NEW");
    // Sanity: brand-new tokens, not the old ones.
    assert_ne!(session.refresh_token, "rt-OLD");
}

// ---------------------------------------------------------------------------
// 3. Full retry: 401 → refresh → 200 with new bearer
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retry_with_refreshed_token_succeeds() {
    // Phase A: old token is rejected.
    let _m_old = mockito::mock("GET", "/auth/v1/user")
        .match_header("authorization", "Bearer old.access.token")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message":"JWT expired"}"#)
        .expect(1)
        .create();

    // Phase B: refresh swap returns the new token pair.
    let _m_refresh = mockito::mock("POST", "/auth/v1/token?grant_type=refresh_token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(refreshed_token_body())
        .expect(1)
        .create();

    // Phase C: retry with the new bearer must hit the success path.
    let _m_new = mockito::mock("GET", "/auth/v1/user")
        .match_header("authorization", "Bearer new.access.token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(user_body())
        .expect(1)
        .create();

    let client = AuthClient::new(server_url(), "anon-key");

    // A: rejected
    let first = client.get_user("old.access.token").await;
    assert!(
        matches!(first, Err(AuthError::GoTrue { status: 401, .. })),
        "first attempt must be 401, got {first:?}"
    );

    // B: refresh
    let session = client
        .refresh_session("rt-OLD")
        .await
        .expect("refresh must succeed");
    assert_eq!(session.access_token, "new.access.token");

    // C: retry with the refreshed bearer
    let user = client
        .get_user(&session.access_token)
        .await
        .expect("retry with new token must succeed");
    assert_eq!(user.id, "user-uuid-1234");
}

// ---------------------------------------------------------------------------
// 4. Refresh failure path: server says invalid_grant, caller must NOT silently
//    succeed with stale data.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_failure_propagates_invalid_refresh_token() {
    let _m = mockito::mock("POST", "/auth/v1/token?grant_type=refresh_token")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"invalid_grant","error_description":"Invalid refresh_token"}"#)
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let err = client
        .refresh_session("rt-revoked")
        .await
        .expect_err("revoked refresh token must error");

    assert!(
        matches!(err, AuthError::InvalidRefreshToken(_)),
        "expected InvalidRefreshToken, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// 5. Sign-in then refresh — store reflects the latest tokens.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn store_reflects_refreshed_session() {
    let _m_login = mockito::mock("POST", "/auth/v1/token?grant_type=password")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
  "access_token": "first.access",
  "refresh_token": "rt-first",
  "expires_in": 3600,
  "token_type": "bearer",
  "user": {
    "id": "user-uuid-1234",
    "email": "user@example.com",
    "role": "authenticated",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
  }
}"#,
        )
        .create();

    let _m_refresh = mockito::mock("POST", "/auth/v1/token?grant_type=refresh_token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(refreshed_token_body())
        .create();

    let client = AuthClient::new(server_url(), "anon-key");

    let first = client.sign_in("u@example.com", "pass").await.unwrap();
    assert_eq!(first.refresh_token, "rt-first");
    assert_eq!(
        client
            .current_session()
            .expect("session stored")
            .refresh_token,
        "rt-first"
    );

    let refreshed = client
        .refresh_session(&first.refresh_token)
        .await
        .expect("refresh must succeed");
    assert_eq!(refreshed.refresh_token, "rt-NEW");

    // Store must now hold the refreshed pair, not the original.
    let stored = client.current_session().expect("session still stored");
    assert_eq!(stored.access_token, "new.access.token");
    assert_eq!(stored.refresh_token, "rt-NEW");
}
