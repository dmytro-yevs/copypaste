use axum::extract::{Extension, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::auth::BearerToken;
use crate::config::RelayConfig;
use crate::error::RelayError;
use crate::models::{PullItem, PullParams, PushRequest, PushResponse};
use crate::quota::{self, QuotaViolation, Tier};
use crate::state::{AppState, DEFAULT_PULL_LIMIT, MAX_PULL_LIMIT};

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
    // Advance last_seen so an actively-deleting device is not evicted by
    // cleanup_inactive_devices (which reaps on last_seen.elapsed(), not
    // registered_at). Without this, a device that continuously polls and
    // deletes items still gets evicted once registered_at passes the threshold.
    store.update_last_seen(&device_id);
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
/// Quota: the decoded `content_b64` must satisfy two independent size limits:
///   1. The per-tier item-size quota (`quota::check_item_size`), checked here
///      *before* acquiring the store mutex so an oversized payload is rejected
///      cheaply with `413 ITEM_SIZE_EXCEEDED`. Free-tier limits are applied
///      conservatively (text ≤ 1 MiB, image ≤ 10 MiB) regardless of the
///      sender's tier — see the relay v2 quotas plan.
///   2. The operator-configured `RELAY_MAX_ITEM_BYTES` body cap, still
///      enforced inside `push_item` (returns `413 PAYLOAD_TOO_LARGE`).
pub async fn push(
    State(state): State<AppState>,
    Extension(config): Extension<RelayConfig>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
    Json(body): Json<PushRequest>,
) -> Result<(StatusCode, Json<PushResponse>), RelayError> {
    // Auth FIRST — verify the bearer token before doing any CPU/alloc work on
    // the request body. Previously the base64 decode (up to ~13 MiB) happened
    // before auth, letting unauthenticated callers trigger full decode work
    // (pre-auth CPU/alloc amplification). Authenticating first makes
    // unauthenticated pushes fail fast at the lock with a cheap map lookup.
    {
        // Short critical section: auth only, no decode under the lock.
        let store = state.lock().unwrap_or_else(|e| e.into_inner());
        store.verify_token(&device_id, &token)?;
    }

    // Per-tier item-size quota — checked after auth but before re-taking the
    // store mutex for the actual insert. We decode `content_b64` once here to
    // measure the true ciphertext size; `push_item_decoded` re-validates the
    // base64 (and the operator body cap) under the lock.
    let decoded_len = B64
        .decode(&body.content_b64)
        .map_err(|_| RelayError::BadRequest("content_b64 must be valid base64".to_string()))?
        .len();
    // Free-tier limits are applied conservatively for all senders (the relay
    // does not yet look up the sender's tier from the bearer token).
    quota::check_item_size(Tier::Free, decoded_len, &body.content_type).map_err(|v| match v {
        QuotaViolation::ItemTooLarge { limit_bytes } => {
            RelayError::ItemSizeExceeded { limit_bytes }
        }
        // `check_item_size` only ever returns `ItemTooLarge`.
        _ => RelayError::Internal("unexpected quota violation in item-size check".into()),
    })?;

    // Survive mutex poisoning (security HIGH #1).
    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());

    // TOCTOU guard: the background evictor (`cleanup_inactive_devices`) may
    // have removed the device record in the window between the first lock
    // (verify_token) and this second one. Re-check existence here so an
    // evicted-but-authenticated device is not surfaced as a 404
    // (DeviceNotFound from push_item_decoded). Collapse to Unauthorized for
    // consistency with verify_token_at's policy: token-guarded routes never
    // return a distinct 404 for a missing device (enumeration oracle — see
    // state.rs verify_token_at comment).
    if !store.devices.contains_key(&device_id) {
        return Err(RelayError::Unauthorized);
    }

    // Advance last_seen so an actively-pushing device is never evicted by
    // cleanup_inactive_devices — which reaps on last_seen.elapsed(), not
    // registered_at. Without this call, last_seen stays at registered_at
    // forever and the device is evicted after the inactivity threshold even
    // though it is actively syncing.
    store.update_last_seen(&device_id);

    // Honor the operator-configured RELAY_MAX_ITEM_BYTES rather than the
    // hardcoded default (security HIGH #2) — previously
    // `RelayConfig::default().max_item_bytes` silently ignored env vars.
    let max_item_bytes = config.max_item_bytes;

    let id = store.push_item_decoded(
        &device_id,
        body.content_type,
        body.content_b64,
        decoded_len,
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
    // Resolve the page size before taking the lock: default when absent,
    // clamped to the hard ceiling so a caller-supplied `limit` cannot force an
    // oversized clone under the global mutex (M4).
    let limit = params
        .limit
        .unwrap_or(DEFAULT_PULL_LIMIT)
        .min(MAX_PULL_LIMIT);

    // Survive mutex poisoning (security HIGH #1).
    // pull needs a mutable borrow to call update_last_seen after auth.
    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());

    // Auth: verify token belongs to this device.
    store.verify_token(&device_id, &token)?;

    // Advance last_seen so an actively-polling device (even one with an empty
    // inbox) is never reaped by cleanup_inactive_devices. Without this,
    // last_seen stays at registered_at forever and the device is evicted after
    // the inactivity threshold even though it is continuously polling.
    store.update_last_seen(&device_id);

    let items = store.pull_items(&device_id, params.since, params.since_id, limit)?;
    Ok(Json(items))
}
