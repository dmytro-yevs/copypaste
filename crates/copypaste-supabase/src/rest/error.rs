//! Error type returned by [`super::client::RestClient`] operations.

use crate::error::AuthError;

/// Errors produced by the REST client.
///
/// Wraps [`AuthError`] for HTTP and credential errors plus PostgREST-specific
/// failures.
#[derive(Debug, thiserror::Error)]
pub enum RestError {
    /// An HTTP transport or status error.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// PostgREST returned an error body.
    #[error("postgrest error ({status}): {message}")]
    PostgRest { status: u16, message: String },

    /// JSON (de)serialisation failure.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// The caller supplied an invalid argument.
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
}

impl From<AuthError> for RestError {
    fn from(e: AuthError) -> Self {
        // HTTP transport errors from AuthError are also HTTP transport errors here.
        match e {
            AuthError::Http(inner) => RestError::Http(inner),
            other => RestError::PostgRest {
                status: 0,
                message: other.to_string(),
            },
        }
    }
}

pub type RestResult<T> = std::result::Result<T, RestError>;
