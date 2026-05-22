use axum::extract::State;
use axum::Json;

use crate::models::HealthResponse;
use crate::state::AppState;

pub async fn handle(State(state): State<AppState>) -> Json<HealthResponse> {
    let store = state.lock().expect("state mutex poisoned");
    let (devices, total_items) = store.stats();
    Json(HealthResponse {
        status: "ok".to_string(),
        devices,
        total_items,
    })
}
