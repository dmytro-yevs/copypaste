use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("device not found")]
    DeviceNotFound,
    // No longer constructed in production since R1a shared-account
    // co-registration: a duplicate `device_id` co-registers (mints an
    // additional token) instead of conflicting. The variant + its 409 mapping
    // are retained for the error-mapping test and any future re-introduction of
    // a conflict path.
    #[allow(dead_code)]
    #[error("device already registered")]
    DeviceConflict,
    #[error("unauthorized")]
    Unauthorized,
    #[error("bad request: {0}")]
    BadRequest(String),
    #[allow(dead_code)]
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("item not found")]
    ItemNotFound,
    /// Returned when the account has reached the maximum number of registered devices.
    #[error("device quota exceeded: maximum {limit} devices allowed for this tier")]
    DeviceQuotaExceeded { limit: usize },
    /// Returned when a clipboard item exceeds the size limit for its content type.
    #[error("item size exceeds the {limit_bytes}-byte limit for this content type")]
    ItemSizeExceeded { limit_bytes: usize },
    /// Returned when a device inbox has reached its maximum history count.
    ///
    /// History-quota enforcement is currently a *silent drop* (the fan-out
    /// sender cannot know which recipient inboxes are full — see the relay v2
    /// quotas plan), so this error is never returned over the wire today. It
    /// is retained for the planned future per-recipient error path and is
    /// covered by `error.rs` status-mapping tests.
    #[allow(dead_code)]
    #[error("history quota exceeded: maximum {limit} items allowed for this tier")]
    HistoryQuotaExceeded { limit: usize },
    /// CopyPaste-h7i8: returned when a device has reached the concurrent SSE
    /// connection cap. Maps to 429 — the client should back off and reconnect
    /// after other streams are closed. The limit is per-device.
    #[error("too many concurrent SSE connections for this device (limit: {limit})")]
    TooManyConnections { limit: usize },
    /// Returned for unrecoverable server-internal failures (e.g. counter
    /// overflow). Maps to 500 — the client cannot fix this by changing
    /// the request.
    #[error("internal server error: {0}")]
    Internal(String),
    /// A persistence-layer (SQLite) failure. Maps to 500 — the durable store
    /// could not be read/written. The message is the rusqlite error rendered
    /// to a string; it never contains ciphertext (only SQL/IO failure context),
    /// so it is safe to surface in the 500 body and logs.
    #[error("storage error: {0}")]
    Storage(String),
}

impl From<rusqlite::Error> for RelayError {
    fn from(e: rusqlite::Error) -> Self {
        // rusqlite errors describe SQL/IO failures (constraint, type, IO), never
        // payload bytes — safe to stringify. Map to 500 via the `Storage` arm.
        RelayError::Storage(e.to_string())
    }
}

impl RelayError {
    fn status_and_code(&self) -> (StatusCode, &'static str) {
        match self {
            RelayError::DeviceNotFound => (StatusCode::NOT_FOUND, "DEVICE_NOT_FOUND"),
            RelayError::DeviceConflict => (StatusCode::CONFLICT, "DEVICE_CONFLICT"),
            RelayError::Unauthorized => (StatusCode::UNAUTHORIZED, "UNAUTHORIZED"),
            RelayError::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            RelayError::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "PAYLOAD_TOO_LARGE"),
            RelayError::ItemNotFound => (StatusCode::NOT_FOUND, "ITEM_NOT_FOUND"),
            RelayError::DeviceQuotaExceeded { .. } => {
                (StatusCode::FORBIDDEN, "DEVICE_QUOTA_EXCEEDED")
            }
            RelayError::ItemSizeExceeded { .. } => {
                (StatusCode::PAYLOAD_TOO_LARGE, "ITEM_SIZE_EXCEEDED")
            }
            RelayError::HistoryQuotaExceeded { .. } => {
                (StatusCode::FORBIDDEN, "HISTORY_QUOTA_EXCEEDED")
            }
            // CopyPaste-h7i8: SSE per-device connection limit → 429.
            RelayError::TooManyConnections { .. } => {
                (StatusCode::TOO_MANY_REQUESTS, "TOO_MANY_CONNECTIONS")
            }
            RelayError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL"),
            RelayError::Storage(_) => (StatusCode::INTERNAL_SERVER_ERROR, "STORAGE"),
        }
    }
}

impl IntoResponse for RelayError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        // CopyPaste-vt0p: never echo raw storage/internal detail to the client.
        // rusqlite error strings carry schema paths, table names, and SQL
        // fragments that constitute an information leak (the bug title).
        // Log the detail server-side so it is available for diagnostics, then
        // return a generic message in the response body.
        let client_message: &'static str = match &self {
            RelayError::Storage(_) | RelayError::Internal(_) => {
                // Log the full detail so operators can investigate without
                // the client ever seeing the rusqlite string.
                tracing::error!(error = %self, "relay internal error");
                "internal server error"
            }
            _ => {
                // All other variants are safe to surface verbatim.
                // We can't use `self.to_string()` as a &'static str, so we
                // fall through to the `body` construction below with an empty
                // placeholder and override it per-branch.
                ""
            }
        };
        let body = if client_message.is_empty() {
            // Non-storage variants: surface the message (no schema/path leak).
            json!({
                "error": self.to_string(),
                "code": code,
            })
        } else {
            // Storage/Internal: generic message only.
            json!({
                "error": client_message,
                "code": code,
            })
        };
        (status, axum::Json(body)).into_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    fn status_of(e: RelayError) -> StatusCode {
        let resp = e.into_response();
        resp.status()
    }

    #[test]
    fn device_not_found_is_404() {
        assert_eq!(status_of(RelayError::DeviceNotFound), StatusCode::NOT_FOUND);
    }

    #[test]
    fn device_conflict_is_409() {
        assert_eq!(status_of(RelayError::DeviceConflict), StatusCode::CONFLICT);
    }

    #[test]
    fn unauthorized_is_401() {
        assert_eq!(
            status_of(RelayError::Unauthorized),
            StatusCode::UNAUTHORIZED
        );
    }

    #[test]
    fn bad_request_is_400() {
        assert_eq!(
            status_of(RelayError::BadRequest("test".into())),
            StatusCode::BAD_REQUEST
        );
    }

    #[test]
    fn payload_too_large_is_413() {
        assert_eq!(
            status_of(RelayError::PayloadTooLarge),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[test]
    fn item_not_found_is_404() {
        assert_eq!(status_of(RelayError::ItemNotFound), StatusCode::NOT_FOUND);
    }

    #[test]
    fn device_quota_exceeded_is_403() {
        assert_eq!(
            status_of(RelayError::DeviceQuotaExceeded { limit: 5 }),
            StatusCode::FORBIDDEN
        );
    }

    #[test]
    fn item_size_exceeded_is_413() {
        assert_eq!(
            status_of(RelayError::ItemSizeExceeded {
                limit_bytes: 1024 * 1024
            }),
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[test]
    fn history_quota_exceeded_is_403() {
        assert_eq!(
            status_of(RelayError::HistoryQuotaExceeded { limit: 1000 }),
            StatusCode::FORBIDDEN
        );
    }

    /// CopyPaste-h7i8: SSE connection-limit error must map to 429.
    #[test]
    fn too_many_connections_is_429() {
        assert_eq!(
            status_of(RelayError::TooManyConnections { limit: 8 }),
            StatusCode::TOO_MANY_REQUESTS
        );
    }

    // ---- CopyPaste-vt0p: storage/internal 500 must NOT echo rusqlite detail ----

    /// Extract the JSON body from a relay error response. Uses the tokio
    /// runtime already present under `#[tokio::test]`.
    async fn body_of_async(e: RelayError) -> serde_json::Value {
        use http_body_util::BodyExt;

        let resp = e.into_response();
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null)
    }

    /// CopyPaste-vt0p: Storage 500 must NOT leak rusqlite detail in the body.
    #[tokio::test]
    async fn storage_500_does_not_echo_rusqlite_detail() {
        let rusqlite_detail = "SqliteFailure(Error { code: CannotOpen, extended_code: 14 }, \
                               Some(\"unable to open database file: /secret/schema.db\"))";
        let body = body_of_async(RelayError::Storage(rusqlite_detail.to_string())).await;

        let error_field = body["error"].as_str().unwrap_or("");
        assert!(
            !error_field.contains(rusqlite_detail),
            "CopyPaste-vt0p: Storage 500 must not echo rusqlite detail; got: {error_field:?}"
        );
        assert!(
            !error_field.contains("schema.db"),
            "CopyPaste-vt0p: Storage 500 must not echo db path; got: {error_field:?}"
        );
        assert_eq!(
            body["code"].as_str(),
            Some("STORAGE"),
            "Storage error must still carry its code"
        );
    }

    /// CopyPaste-vt0p: Internal 500 must NOT leak internal detail in the body.
    #[tokio::test]
    async fn internal_500_does_not_echo_detail() {
        let detail = "sync id counter exhausted for device abc-xyz";
        let body = body_of_async(RelayError::Internal(detail.to_string())).await;

        let error_field = body["error"].as_str().unwrap_or("");
        assert!(
            !error_field.contains(detail),
            "CopyPaste-vt0p: Internal 500 must not echo detail; got: {error_field:?}"
        );
        assert_eq!(body["code"].as_str(), Some("INTERNAL"));
    }

    /// CopyPaste-vt0p: non-storage errors (e.g. Unauthorized) must still be
    /// surfaced in the response so clients can act on them.
    #[tokio::test]
    async fn non_storage_errors_are_surfaced_verbatim() {
        let body = body_of_async(RelayError::Unauthorized).await;
        let error_field = body["error"].as_str().unwrap_or("");
        assert!(
            !error_field.is_empty(),
            "non-storage errors must include a message"
        );
    }
}
