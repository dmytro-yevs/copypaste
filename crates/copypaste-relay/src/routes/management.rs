//! Management-plane endpoint handlers.
//!
//! These handlers serve operational/introspection endpoints that are **not**
//! part of the sync data path:
//!
//! * `GET /stats` — bare relay version probe (no counts for unauthenticated
//!   callers; see security note below).
//! * `GET /devices` — list device IDs scoped to the authenticated account.
//!
//! # Auth policy
//!
//! `stats_handler` is intentionally **unauthenticated** but intentionally
//! **count-free**: returning device or item counts to anonymous callers would
//! reveal the volume of clipboard traffic and the number of active devices,
//! which is operational data that must require authentication.  Only the relay
//! protocol version is returned (CopyPaste-j21 security hardening).  A future
//! phase may gate detailed counters behind an operator bearer token.
//!
//! `list_devices_handler` requires a valid `Authorization: Bearer <token>` and
//! returns **only** the device ID that belongs to the authenticated account
//! (CopyPaste-7185, P2 security fix — previously unauthenticated, allowing
//! cross-account inbox-UUID enumeration).

use axum::extract::State;
use axum::response::IntoResponse;

use crate::auth::BearerToken;
use crate::error::RelayError;
use crate::state::AppState;

/// `GET /stats` — bare version probe.
///
/// Device and item counts are intentionally omitted from this unauthenticated
/// endpoint. Leaking counts to anonymous callers reveals the number of active
/// devices and the volume of clipboard traffic, which is operational data that
/// should require authentication. Only the relay protocol version is returned
/// (CopyPaste-j21 security hardening). A future phase may gate detailed
/// counters behind an operator bearer token.
pub(crate) async fn stats_handler(State(_state): State<AppState>) -> impl IntoResponse {
    axum::Json(serde_json::json!({
        "version": "2"
    }))
}

/// GET /devices — list device IDs scoped to the authenticated account.
///
/// Requires a valid `Authorization: Bearer <token>` from any registered device.
/// The token is verified against the complete device set (P2 7185 — previously
/// unauthenticated, allowing enumeration of sync-key-derived inbox UUIDs).
///
/// CopyPaste-7185 (P2 security fix): returns ONLY the device_id that the
/// bearer token belongs to — not all device IDs across all accounts. A bearer
/// authenticates to exactly one account inbox UUID (`device_id`); returning
/// any other account's device UUID would enable cross-account inbox-UUID
/// enumeration and traffic analysis.
///
/// Returns only opaque device IDs. Bearer tokens are **never** included
/// (they would let anyone hijack the device). Other public fields like
/// `public_key_b64` are exposed via the per-device endpoint `GET /devices/:id`.
pub(crate) async fn list_devices_handler(
    State(state): State<AppState>,
    BearerToken(token): BearerToken,
) -> Result<impl IntoResponse, RelayError> {
    // Survive mutex poisoning (security INFO #21).
    let store = state.lock().unwrap_or_else(|e| e.into_inner());

    // P2 7185: authenticate the caller AND identify which account the bearer
    // belongs to. `verify_token_at` uses constant-time comparison and enforces
    // expiry (fail-closed on clock error). We iterate over every device to find
    // the one whose token set contains this bearer — `find` short-circuits after
    // the first match (unlike the previous `any`), giving us the authenticated
    // device_id for account-scoping below.
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|d| d.as_secs() as i64);
    let authenticated_device_id: Option<String> = store
        .devices
        .keys()
        .find(|id| store.verify_token_at(id, &token, now_unix).is_ok())
        .cloned();

    let device_id = match authenticated_device_id {
        Some(id) => id,
        None => return Err(RelayError::Unauthorized),
    };

    // Return ONLY the authenticated account's own device_id. In the relay
    // model, `device_id` is the account-inbox UUID (derived via HKDF from the
    // shared sync key); all devices on one account share a single device_id and
    // co-register with independent tokens. Returning any other account's
    // device_id would expose their inbox UUID — a P2 privacy/security issue.
    Ok(axum::Json(serde_json::json!({ "devices": [device_id] })))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use axum::body::Body;
    use axum::http::Request;

    use super::*;
    use crate::config::RelayConfig;

    /// Build a minimal test router that wires only `list_devices_handler` at
    /// `GET /devices` and `POST /devices` (registration) — no rate limiting so
    /// oneshot tests work without a peer IP.
    fn list_devices_test_router(state: AppState) -> axum::Router {
        axum::Router::new()
            .route(
                "/devices",
                axum::routing::post(crate::routes::devices::register).get(list_devices_handler),
            )
            .with_state(state)
            .layer(axum::Extension(RelayConfig::default()))
    }

    async fn call_get_devices(
        state: AppState,
        token: Option<&str>,
    ) -> (axum::http::StatusCode, serde_json::Value) {
        use axum::http::header;
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        let mut builder = Request::builder()
            .method(axum::http::Method::GET)
            .uri("/devices");
        if let Some(t) = token {
            builder = builder.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        let req = builder.body(Body::empty()).unwrap();
        let app = list_devices_test_router(state);
        let resp = app.oneshot(req).await.unwrap();
        let status = resp.status();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (status, body)
    }

    /// CopyPaste-7185: bearer for account A must NOT expose account B's device
    /// UUID in the GET /devices response. Each account owns exactly one device_id
    /// (the shared inbox UUID); the list must be scoped to the caller's own id.
    #[tokio::test]
    async fn get_devices_scoped_to_authenticated_account() {
        use crate::state::RelayStore;
        use axum::http::StatusCode;
        use base64::engine::general_purpose::STANDARD as B64;
        use base64::Engine;
        use std::sync::Mutex;

        const DEVICE_A: &str = "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa";
        const DEVICE_B: &str = "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb";

        let state = Arc::new(Mutex::new(RelayStore::new(3600)));

        let (token_a, _) = state
            .lock()
            .unwrap()
            .register_device(
                DEVICE_A.to_string(),
                "Device A".into(),
                B64.encode([0x00u8; 32]),
                B64.encode([0xDE_u8; 32]),
            )
            .unwrap();

        let (token_b, _) = state
            .lock()
            .unwrap()
            .register_device(
                DEVICE_B.to_string(),
                "Device B".into(),
                B64.encode([0x01u8; 32]),
                B64.encode([0xAB_u8; 32]),
            )
            .unwrap();

        // Account A sees only its own device_id, not device B.
        let (status, body) = call_get_devices(state.clone(), Some(&token_a)).await;
        assert_eq!(status, StatusCode::OK, "account A: expected 200");
        let devices = body["devices"].as_array().expect("`devices` must be array");
        assert!(
            devices.iter().any(|v| v.as_str() == Some(DEVICE_A)),
            "account A bearer must include its own device_id; got {devices:?}"
        );
        assert!(
            !devices.iter().any(|v| v.as_str() == Some(DEVICE_B)),
            "CopyPaste-7185: account A bearer must NOT expose device B UUID; got {devices:?}"
        );

        // Account B sees only its own device_id, not device A.
        let (status, body) = call_get_devices(state.clone(), Some(&token_b)).await;
        assert_eq!(status, StatusCode::OK, "account B: expected 200");
        let devices = body["devices"].as_array().expect("`devices` must be array");
        assert!(
            devices.iter().any(|v| v.as_str() == Some(DEVICE_B)),
            "account B bearer must include its own device_id; got {devices:?}"
        );
        assert!(
            !devices.iter().any(|v| v.as_str() == Some(DEVICE_A)),
            "CopyPaste-7185: account B bearer must NOT expose device A UUID; got {devices:?}"
        );

        // No bearer → 401.
        let (status, _) = call_get_devices(state.clone(), None).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "request without bearer must return 401"
        );

        // Wrong bearer → 401.
        let (status, _) = call_get_devices(state, Some("deadbeefdeadbeefdeadbeefdeadbeef")).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "invalid bearer must return 401"
        );
    }
}
