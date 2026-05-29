use axum::extract::State;
use axum::Json;

use crate::models::HealthResponse;
use crate::state::AppState;

pub async fn handle(State(state): State<AppState>) -> Json<HealthResponse> {
    // Survive mutex poisoning (security INFO #21, M6): recover the inner data
    // rather than panicking the request, matching every other handler. A
    // poisoned mutex must not turn a liveness probe into a crash loop.
    let store = state.lock().unwrap_or_else(|e| e.into_inner());
    let (devices, total_items) = store.stats();
    Json(HealthResponse {
        status: "ok".to_string(),
        devices,
        total_items,
    })
}
