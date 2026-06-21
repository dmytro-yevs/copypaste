use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

use crate::auth::BearerToken;
use crate::error::RelayError;
use crate::models::{DeviceInfoResponse, RegisterRequest, RegisterResponse};
use crate::state::AppState;

/// POST /devices — register (or co-register) a device and issue an auth token.
///
/// Body: `{ device_id, device_name, public_key_b64 }`
/// Response (201): `{ device_id, auth_token, expires_at }`
///
/// Shared-account co-registration (R1a): a `device_id` that is *already*
/// registered is NOT rejected — the relay mints a fresh, independent token for
/// it (still 201) and keeps every previously-issued token valid. This lets all
/// devices on one account co-register the same secret account-inbox id (derived
/// via HKDF of the shared sync key, never sent in cleartext) and thereby push
/// to / read the one shared inbox — the mechanism for cross-device delivery.
/// The relay only ever stores opaque ciphertext.
///
/// Errors:
/// - 400 Bad Request — invalid UUID, invalid base64, key length mismatch, blank name
/// - 403 Forbidden — free-tier device quota exhausted (NEW device records only)
/// - 429 Too Many Requests — per-(ip, device) registration rate limit (5/min) tripped
pub async fn register(
    State(state): State<AppState>,
    // In axum 0.8, `Option<ConnectInfo<T>>` no longer implements FromRequestParts.
    // ConnectInfo is stored as an Extension, so we extract it as
    // `Option<Extension<ConnectInfo<SocketAddr>>>` which uses the
    // OptionalFromRequestParts impl on Extension.
    connect_info: Option<Extension<ConnectInfo<SocketAddr>>>,
    Json(body): Json<RegisterRequest>,
) -> Result<(StatusCode, Json<RegisterResponse>), Response> {
    // Validate device_id is a valid UUID (basic format check).
    if uuid::Uuid::parse_str(&body.device_id).is_err() {
        return Err(
            RelayError::BadRequest("device_id must be a valid UUID".to_string()).into_response(),
        );
    }

    // Validate device_name is non-empty and within reasonable length.
    let device_name = body.device_name.trim().to_string();
    if device_name.is_empty() {
        return Err(
            RelayError::BadRequest("device_name must not be empty".to_string()).into_response(),
        );
    }
    if device_name.len() > 64 {
        return Err(RelayError::BadRequest(
            "device_name must be 64 characters or fewer".to_string(),
        )
        .into_response());
    }

    // Validate public_key_b64 is valid base64 and decodes to exactly 32 bytes.
    let key_bytes = B64.decode(&body.public_key_b64).map_err(|_| {
        RelayError::BadRequest("public_key_b64 must be valid base64".to_string()).into_response()
    })?;

    if key_bytes.len() != 32 {
        return Err(RelayError::BadRequest(format!(
            "public_key_b64 must decode to exactly 32 bytes, got {}",
            key_bytes.len()
        ))
        .into_response());
    }

    // Per-(ip, device) rate limit (HIGH #5 / MEDIUM #13).
    //
    // Runs *after* structural payload validation (UUID, name, key) but
    // *before* PoP extraction, so the 429 fires even when pop_b64 is absent.
    // The limiter is keyed by `(client_ip, device_id)` so a `device_id`-only
    // enumeration probe from an attacker IP cannot collide with the bucket the
    // legitimate owner builds up from a different IP. When `ConnectInfo` is
    // absent (tests using `tower::ServiceExt::oneshot`) the IP becomes `None`,
    // preserving the previous per-device-only fallback for that path.
    let client_ip = connect_info.map(|Extension(ConnectInfo(addr))| addr.ip());

    // Survive mutex poisoning (security INFO #21): if another thread panicked
    // while holding the lock, recover the inner data rather than crashing this
    // request. The data is still consistent because all writes are atomic.
    //
    // Hold a single guard across both the rate-limit check and the register
    // call: re-locking between the two opened a needless drop/re-acquire window
    // (lock churn, plus a benign TOCTOU gap) with no behavioural benefit. The
    // limiter mutation and the registration are now one atomic critical section.
    let mut store = state.lock().unwrap_or_else(|e| e.into_inner());
    if let Err(retry_after) = store.check_registration_rate_limit(client_ip, &body.device_id) {
        let body = serde_json::json!({
            "error": "too many registration attempts",
            "code": "RATE_LIMITED",
            "retry_after_secs": retry_after,
        });
        let resp = (
            StatusCode::TOO_MANY_REQUESTS,
            [(axum::http::header::RETRY_AFTER, retry_after.to_string())],
            Json(body),
        )
            .into_response();
        return Err(resp);
    }

    // Validate pop_b64 is present (required — fixes CopyPaste-n2l).
    // Extracted *after* the rate-limit check so that 429 fires even when the
    // field is absent (e.g. rate-limit probes without a valid pop_b64).
    // The state layer performs the deeper PoP verification (constant-time
    // compare on co-registration); here we just surface a clear error when the
    // field is entirely absent so callers get a descriptive 400.
    let pop_b64 = body.pop_b64.ok_or_else(|| {
        RelayError::BadRequest(
            "pop_b64 is required: provide HMAC-SHA256(sync_key, \
             \"relay-registration-pop-v1:\" || device_id) base64-encoded"
                .to_string(),
        )
        .into_response()
    })?;

    // Scope the per-account device quota (H1) to the registering client IP so
    // it is a per-source cap, not a global ceiling that would reject the 6th
    // device across all users. `client_ip` is reused from the rate-limit check.
    let (auth_token, expires_at_unix) = store
        .register_device_scoped(
            client_ip,
            body.device_id.clone(),
            device_name,
            body.public_key_b64,
            pop_b64,
        )
        .map_err(|e| e.into_response())?;

    // Format expires_at as RFC-3339.
    let expires_at = unix_to_rfc3339(expires_at_unix);

    Ok((
        StatusCode::CREATED,
        Json(RegisterResponse {
            device_id: body.device_id,
            auth_token,
            expires_at,
        }),
    ))
}

/// GET /devices/:device_id — retrieve info about a registered device.
///
/// Requires a valid `Authorization: Bearer <token>` matching the requested
/// `device_id`. CopyPaste-44rq.52: previously unauthenticated, leaking
/// `device_name`, `public_key_b64`, and timestamps to any caller who had
/// observed a `device_id` from traffic. Now gated: callers must prove they
/// hold the correct bearer token for that device.
///
/// Response (200): `{ device_id, device_name, public_key_b64, registered_at, expires_at }`
/// Error (401): missing or invalid bearer token.
/// Error (404): device not found (only after auth succeeds).
pub async fn get_device(
    State(state): State<AppState>,
    Path(device_id): Path<String>,
    BearerToken(token): BearerToken,
) -> Result<Json<DeviceInfoResponse>, RelayError> {
    // Survive mutex poisoning (security INFO #21).
    let store = state.lock().unwrap_or_else(|e| e.into_inner());
    // CopyPaste-44rq.52: authenticate before returning any device data.
    // verify_token uses constant-time comparison (subtle crate) and enforces
    // token expiry (fail-closed on clock error). We use Unauthorized (not
    // DeviceNotFound) on a bad token so callers cannot enumerate device IDs
    // by probing with a garbage token — see verify_token_at comment in state.rs.
    store.verify_token(&device_id, &token)?;
    let record = store.get_device(&device_id)?;

    // Convert Instant → wall-clock by computing elapsed and subtracting from now.
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let elapsed_secs = record.registered_at.elapsed().as_secs() as i64;
    let registered_at_unix = now_unix - elapsed_secs;

    Ok(Json(DeviceInfoResponse {
        device_id: record.device_id.clone(),
        device_name: record.device_name.clone(),
        public_key_b64: record.public_key_b64.clone(),
        registered_at: unix_to_rfc3339(registered_at_unix),
        // A device_id now holds a SET of co-registered tokens (R1a); surface
        // the latest expiry across them as the record's `expires_at`.
        expires_at: unix_to_rfc3339(record.latest_expires_at()),
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn unix_to_rfc3339(unix_secs: i64) -> String {
    let secs = unix_secs.max(0) as u64;
    let (year, month, day, hour, min, sec) = epoch_to_date(secs);
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{min:02}:{sec:02}Z")
}

/// Convert Unix epoch seconds to (year, month, day, hour, min, sec) in UTC.
fn epoch_to_date(mut secs: u64) -> (u32, u32, u32, u32, u32, u32) {
    let sec = (secs % 60) as u32;
    secs /= 60;
    let min = (secs % 60) as u32;
    secs /= 60;
    let hour = (secs % 24) as u32;
    secs /= 24;

    // Days since 1970-01-01.
    let mut days = secs;

    // Gregorian calendar: 400-year cycle = 97 leap years.
    let year_400 = days / 146097;
    days %= 146097;
    let year_100 = (days / 36524).min(3);
    days -= year_100 * 36524;
    let year_4 = days / 1461;
    days %= 1461;
    let year_1 = (days / 365).min(3);
    days -= year_1 * 365;

    let year = (year_400 * 400 + year_100 * 100 + year_4 * 4 + year_1 + 1970) as u32;
    let leap =
        (year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))) as u64;

    let month_days: [u64; 12] = [31, 28 + leap, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 1u32;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }

    (year, month, days as u32 + 1, hour, min, sec)
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_to_rfc3339_epoch() {
        assert_eq!(unix_to_rfc3339(0), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn unix_to_rfc3339_known_date() {
        // 2024-01-01T00:00:00Z = 1704067200
        assert_eq!(unix_to_rfc3339(1_704_067_200), "2024-01-01T00:00:00Z");
    }
}
