use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    #[error("device not found")]
    DeviceNotFound,
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
    #[allow(dead_code)]
    #[error("history quota exceeded: maximum {limit} items allowed for this tier")]
    HistoryQuotaExceeded { limit: usize },
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
        }
    }
}

impl IntoResponse for RelayError {
    fn into_response(self) -> Response {
        let (status, code) = self.status_and_code();
        let body = json!({
            "error": self.to_string(),
            "code": code,
        });
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
        assert_eq!(status_of(RelayError::Unauthorized), StatusCode::UNAUTHORIZED);
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
}
