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

        // RFC 6750 §2.1: the "Bearer" scheme token is case-insensitive.
        // Accept "Bearer ", "bearer ", "BEARER ", etc. so standard-conforming
        // clients that lowercase the scheme are not rejected.
        let token = header
            .get(..7)
            .filter(|scheme| scheme.eq_ignore_ascii_case("bearer "))
            .map(|_| &header[7..])
            .ok_or(RelayError::Unauthorized)?;

        if token.is_empty() {
            return Err(RelayError::Unauthorized);
        }

        Ok(BearerToken(token.to_string()))
    }
}
