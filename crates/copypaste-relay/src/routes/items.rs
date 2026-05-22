use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use std::time::Instant;

use crate::auth::BearerToken;
use crate::error::RelayError;
use crate::models::{PollParams, PollResponse, UploadRequest, UploadResponse};
use crate::quota::{self, QuotaViolation, Tier};
use crate::state::{AppState, RelayItem};

pub async fn poll(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Query(params): Query<PollParams>,
) -> Result<Json<PollResponse>, RelayError> {
    let mut store = state.lock().expect("state mutex poisoned");
    store.verify_token(&device_id, &token)?;
    let items = store.poll_items(&device_id, params.since_lamport);
    Ok(Json(PollResponse { items }))
}

pub async fn upload(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Json(body): Json<UploadRequest>,
) -> Result<(StatusCode, Json<UploadResponse>), RelayError> {
    // Validate item_id is a valid UUID.
    if uuid::Uuid::parse_str(&body.item_id).is_err() {
        return Err(RelayError::BadRequest(
            "item_id must be a valid UUID".to_string(),
        ));
    }

    // Validate nonce decodes to exactly 24 bytes (XChaCha20-Poly1305 nonce).
    let nonce_bytes = B64
        .decode(&body.nonce_b64)
        .map_err(|_| RelayError::BadRequest("nonce_b64 must be valid base64".to_string()))?;
    if nonce_bytes.len() != 24 {
        return Err(RelayError::BadRequest(format!(
            "nonce_b64 must decode to exactly 24 bytes, got {}",
            nonce_bytes.len()
        )));
    }

    // Validate ciphertext is valid base64.
    let ct_bytes = B64
        .decode(&body.ciphertext_b64)
        .map_err(|_| RelayError::BadRequest("ciphertext_b64 must be valid base64".to_string()))?;

    // Validate lamport_ts > 0.
    if body.lamport_ts == 0 {
        return Err(RelayError::BadRequest(
            "lamport_ts must be > 0".to_string(),
        ));
    }

    // Validate content_type.
    if !matches!(body.content_type.as_str(), "text" | "image" | "file") {
        return Err(RelayError::BadRequest(
            "content_type must be 'text', 'image', or 'file'".to_string(),
        ));
    }

    // Enforce per-tier item size quota before acquiring the state lock.
    // The handler uses the free-tier limit as a conservative pre-check.
    // (A future enhancement could look up the sender's tier from the token.)
    // For safety, we always apply the most restrictive tier (Free) as a hard cap
    // at the HTTP layer; the content-type-aware limit is checked here.
    quota::check_item_size(Tier::Free, ct_bytes.len(), &body.content_type).map_err(
        |v| match v {
            QuotaViolation::ItemTooLarge { limit_bytes } => {
                RelayError::ItemSizeExceeded { limit_bytes }
            }
            _ => unreachable!(),
        },
    )?;

    let mut store = state.lock().expect("state mutex poisoned");
    store.verify_token(&device_id, &token)?;

    let item = RelayItem {
        item_id: body.item_id,
        ciphertext_b64: body.ciphertext_b64,
        nonce_b64: body.nonce_b64,
        sender_device_id: body.sender_device_id,
        lamport_ts: body.lamport_ts,
        content_type: body.content_type,
        uploaded_at: Instant::now(),
    };

    let config = crate::config::RelayConfig::default();
    let fanned_out_to = store.upload_item(item, &config);

    Ok((
        StatusCode::CREATED,
        Json(UploadResponse { fanned_out_to }),
    ))
}

pub async fn delete_item(
    State(state): State<AppState>,
    Path((device_id, item_id)): Path<(String, String)>,
    BearerToken(token): BearerToken,
) -> Result<StatusCode, RelayError> {
    let mut store = state.lock().expect("state mutex poisoned");
    store.verify_token(&device_id, &token)?;
    store.delete_item(&device_id, &item_id)?;
    Ok(StatusCode::NO_CONTENT)
}
