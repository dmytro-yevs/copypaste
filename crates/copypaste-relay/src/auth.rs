use axum::extract::FromRequestParts;
use axum::http::request::Parts;

use crate::error::RelayError;

/// Custom axum extractor that pulls the bearer token from the
/// `Authorization: Bearer <token>` header.
pub struct BearerToken(pub String);

impl<S> FromRequestParts<S> for BearerToken
where
    S: Send + Sync,
{
    type Rejection = RelayError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(RelayError::Unauthorized)?;

        let token = header
            .strip_prefix("Bearer ")
            .ok_or(RelayError::Unauthorized)?;

        if token.is_empty() {
            return Err(RelayError::Unauthorized);
        }

        Ok(BearerToken(token.to_string()))
    }
}
