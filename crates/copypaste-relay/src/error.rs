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
    #[error("payload too large")]
    PayloadTooLarge,
    #[error("item not found")]
    ItemNotFound,
    #[error("device quota exceeded: maximum free-tier devices reached")]
    QuotaExceeded,
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
            RelayError::QuotaExceeded => (StatusCode::FORBIDDEN, "QUOTA_EXCEEDED"),
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
    fn quota_exceeded_is_403() {
        assert_eq!(status_of(RelayError::QuotaExceeded), StatusCode::FORBIDDEN);
    }
}
