use axum::Json;

use crate::models::HealthResponse;

/// `GET /health` — bare liveness probe.
///
/// Returns only `{"status": "ok"}`. Device and item counts are intentionally
/// omitted from this unauthenticated endpoint to avoid leaking operational
/// metrics to anonymous observers (CopyPaste-j21 security hardening).
/// Authenticated operators can query `/stats` or `/metrics` if they need
/// detailed counters (those endpoints remain rate-limit-exempt but their
/// count fields may be gated behind auth in a future phase).
pub async fn handle() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
    })
}
