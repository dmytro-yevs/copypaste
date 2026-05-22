/// Integration tests for AuthClient using mockito to mock the GoTrue HTTP API.
///
/// Uses mockito 0.31 global server API.

use copypaste_supabase::{AuthClient, AuthError};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fake_token_body(refresh_token: &str, expires_in: u64) -> String {
    format!(
        r#"{{
  "access_token": "eyJtest.access",
  "refresh_token": "{refresh_token}",
  "expires_in": {expires_in},
  "token_type": "bearer",
  "user": {{
    "id": "user-uuid-1234",
    "email": "user@example.com",
    "role": "authenticated",
    "created_at": "2024-01-01T00:00:00Z",
    "updated_at": "2024-01-01T00:00:00Z"
  }}
}}"#
    )
}

fn gotrue_error_body(msg: &str) -> String {
    format!(r#"{{"error": "invalid_grant", "error_description": "{msg}"}}"#)
}

/// Return the mockito global server URL.
fn server_url() -> String {
    mockito::server_url()
}

// ---------------------------------------------------------------------------
// sign_in tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sign_in_success_returns_session() {
    let _m = mockito::mock("POST", "/auth/v1/token?grant_type=password")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fake_token_body("rt-abc123", 3600))
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let session = client
        .sign_in("user@example.com", "password")
        .await
        .expect("sign_in should succeed");

    assert_eq!(session.access_token, "eyJtest.access");
    assert_eq!(session.refresh_token, "rt-abc123");
    assert_eq!(session.expires_in, 3600);
    assert_eq!(session.user.email.as_deref(), Some("user@example.com"));
    // expires_at must be in the future
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    assert!(session.expires_at > now);
}

#[tokio::test]
async fn sign_in_invalid_credentials_returns_error() {
    let _m = mockito::mock("POST", "/auth/v1/token?grant_type=password")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(gotrue_error_body("Invalid login credentials"))
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let err = client
        .sign_in("bad@example.com", "wrong")
        .await
        .expect_err("should fail");

    assert!(
        matches!(err, AuthError::InvalidCredentials(_)),
        "expected InvalidCredentials, got {err:?}"
    );
}

#[tokio::test]
async fn sign_in_saves_session_to_store() {
    let _m = mockito::mock("POST", "/auth/v1/token?grant_type=password")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fake_token_body("store-rt", 3600))
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    client.sign_in("u@example.com", "pass").await.unwrap();

    let stored = client.current_session().expect("session should be stored");
    assert_eq!(stored.refresh_token, "store-rt");
}

// ---------------------------------------------------------------------------
// refresh_session tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_session_success() {
    let _m = mockito::mock("POST", "/auth/v1/token?grant_type=refresh_token")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fake_token_body("new-rt-xyz", 3600))
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let session = client
        .refresh_session("old-rt")
        .await
        .expect("refresh should succeed");

    assert_eq!(session.refresh_token, "new-rt-xyz");
}

#[tokio::test]
async fn refresh_session_invalid_token_returns_error() {
    let _m = mockito::mock("POST", "/auth/v1/token?grant_type=refresh_token")
        .with_status(400)
        .with_header("content-type", "application/json")
        .with_body(r#"{"error":"invalid_grant","error_description":"Invalid refresh_token"}"#)
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let err = client
        .refresh_session("bad-token")
        .await
        .expect_err("should fail");

    assert!(
        matches!(err, AuthError::InvalidRefreshToken(_)),
        "expected InvalidRefreshToken, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// sign_out tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sign_out_success_clears_store() {
    // First sign in to populate the store.
    let _sign_in_mock = mockito::mock("POST", "/auth/v1/token?grant_type=password")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(fake_token_body("rt-to-clear", 3600))
        .create();

    let _logout_mock = mockito::mock("POST", "/auth/v1/logout")
        .with_status(204)
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let session = client.sign_in("u@example.com", "pass").await.unwrap();
    assert!(client.current_session().is_some());

    client.sign_out(&session.access_token).await.unwrap();
    assert!(
        client.current_session().is_none(),
        "store should be cleared after sign-out"
    );
}

// ---------------------------------------------------------------------------
// get_user tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_user_success() {
    let _m = mockito::mock("GET", "/auth/v1/user")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{
  "id": "user-uuid-1234",
  "email": "user@example.com",
  "role": "authenticated",
  "created_at": "2024-01-01T00:00:00Z",
  "updated_at": "2024-01-02T00:00:00Z"
}"#,
        )
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let user = client
        .get_user("eyJtest.access")
        .await
        .expect("get_user should succeed");

    assert_eq!(user.id, "user-uuid-1234");
    assert_eq!(user.email.as_deref(), Some("user@example.com"));
}

#[tokio::test]
async fn get_user_unauthorized_returns_error() {
    let _m = mockito::mock("GET", "/auth/v1/user")
        .with_status(401)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message": "JWT expired"}"#)
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let err = client
        .get_user("expired.token")
        .await
        .expect_err("should fail");

    assert!(
        matches!(err, AuthError::GoTrue { status: 401, .. }),
        "expected GoTrue 401, got {err:?}"
    );
}

// ---------------------------------------------------------------------------
// Session expiry helper tests (no network needed)
// ---------------------------------------------------------------------------

#[test]
fn session_is_expired_when_past_expiry() {
    use copypaste_supabase::models::Session;

    let past_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        .saturating_sub(10); // 10 s in the past

    let session = Session {
        access_token: "tok".into(),
        refresh_token: "rt".into(),
        expires_in: 3600,
        expires_at: past_ts,
        token_type: "bearer".into(),
        user: copypaste_supabase::User {
            id: "id".into(),
            email: None,
            role: None,
            created_at: None,
            updated_at: None,
        },
    };

    assert!(session.is_expired_with_margin(0));
}

#[test]
fn session_is_not_expired_when_future() {
    use copypaste_supabase::models::Session;

    let future_ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 3600; // 1 h in the future

    let session = Session {
        access_token: "tok".into(),
        refresh_token: "rt".into(),
        expires_in: 3600,
        expires_at: future_ts,
        token_type: "bearer".into(),
        user: copypaste_supabase::User {
            id: "id".into(),
            email: None,
            role: None,
            created_at: None,
            updated_at: None,
        },
    };

    assert!(!session.is_expired_with_margin(0));
    // With a 60 s margin it should still not trigger (3600 - 60 > 0).
    assert!(!session.is_expired_with_margin(60));
}

// ---------------------------------------------------------------------------
// Error handling: server 500
// ---------------------------------------------------------------------------

#[tokio::test]
async fn sign_in_server_error_returns_gotrue_error() {
    let _m = mockito::mock("POST", "/auth/v1/token?grant_type=password")
        .with_status(500)
        .with_header("content-type", "application/json")
        .with_body(r#"{"message": "internal server error"}"#)
        .create();

    let client = AuthClient::new(server_url(), "anon-key");
    let err = client
        .sign_in("u@example.com", "pass")
        .await
        .expect_err("should fail on 500");

    assert!(
        matches!(err, AuthError::GoTrue { status: 500, .. }),
        "expected GoTrue 500, got {err:?}"
    );
}
