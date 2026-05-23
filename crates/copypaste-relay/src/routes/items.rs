use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;

use crate::auth::BearerToken;
use crate::config::RelayConfig;
use crate::error::RelayError;
use crate::models::{PullItem, PullParams, PushRequest, PushResponse};
use crate::state::AppState;

/// DELETE /devices/:device_id/items/:item_id
///
/// Remove a single item from a device's legacy fanout inbox.
pub async fn delete_item(
    State(state): State<AppState>,
    Path((device_id, item_id)): Path<(String, String)>,
    BearerToken(token): BearerToken,
) -> Result<StatusCode, RelayError> {
    // Survive mutex poisoning (security HIGH #1, INFO #21): recover the
    // inner data rather than crashing the request. Matches the pattern
    // already used in devices.rs.
    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
    store.verify_token(&device_id, &token)?;
    store.delete_item(&device_id, &item_id)?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------------------
// Push / Pull — wall-clock sync routes
// POST /devices/:id/items  — push an encrypted item into the device's inbox.
// GET  /devices/:id/items?since=<wall_time> — pull items newer than wall_time.
// ---------------------------------------------------------------------------

/// POST /devices/:device_id/items
///
/// Body: `{ "content_type": "text"|"image"|"file", "content_b64": "<base64>", "wall_time": <u64> }`
/// Response (201): `{ "id": <i64> }`
///
/// Auth: `Authorization: Bearer <token>` — must match the token for `device_id`.
/// Quota: decoded `content_b64` must not exceed `max_item_bytes` from config.
pub async fn push(
    State(state): State<AppState>,
    Extension(config): Extension<RelayConfig>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Json(body): Json<PushRequest>,
) -> Result<(StatusCode, Json<PushResponse>), RelayError> {
    // Survive mutex poisoning (security HIGH #1).
    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());

    // Auth: verify token belongs to this device.
    store.verify_token(&device_id, &token)?;

    // Honor the operator-configured RELAY_MAX_ITEM_BYTES rather than the
    // hardcoded default (security HIGH #2) — previously
    // `RelayConfig::default().max_item_bytes` silently ignored env vars.
    let max_item_bytes = config.max_item_bytes;

    let id = store.push_item(
        &device_id,
        body.content_type,
        body.content_b64,
        body.wall_time,
        max_item_bytes,
    )?;

    Ok((StatusCode::CREATED, Json(PushResponse { id })))
}

/// GET /devices/:device_id/items?since=<wall_time>
///
/// Returns all items in `device_id`'s inbox with `wall_time > since`, ordered ascending.
/// Auth: `Authorization: Bearer <token>` — must match the token for `device_id`.
pub async fn pull(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Query(params): Query<PullParams>,
) -> Result<Json<Vec<PullItem>>, RelayError> {
    // Survive mutex poisoning (security HIGH #1).
    let store = state.lock().unwrap_or_else(|e| e.into_inner());

    // Auth: verify token belongs to this device.
    store.verify_token(&device_id, &token)?;

    let items = store.pull_items(&device_id, params.since)?;
    Ok(Json(items))
}
