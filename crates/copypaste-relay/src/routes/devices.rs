use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::error::RelayError;
use crate::models::{RegisterRequest, RegisterResponse};
use crate::state::AppState;

pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), RelayError> {
    // Validate device_id is a valid UUID (basic format check).
    if uuid::Uuid::parse_str(&body.device_id).is_err() {
        return Err(RelayError::BadRequest(
            "device_id must be a valid UUID".to_string(),
        ));
    }

    // Validate public_key is valid base64 and decodes to exactly 32 bytes.
    let key_bytes = B64
        .decode(&body.public_key)
        .map_err(|_| RelayError::BadRequest("public_key must be valid base64".to_string()))?;

    if key_bytes.len() != 32 {
        return Err(RelayError::BadRequest(format!(
            "public_key must decode to exactly 32 bytes, got {}",
            key_bytes.len()
        )));
    }

    let mut store = state.lock().expect("state mutex poisoned");
    let bearer_token =
        store.register_device(body.device_id.clone(), body.public_key)?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            device_id: body.device_id,
            bearer_token,
        }),
    ))
}
