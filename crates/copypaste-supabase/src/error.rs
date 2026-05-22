use thiserror::Error;

/// Errors produced by the Supabase auth client.
#[derive(Debug, Error)]
pub enum AuthError {
    /// The credentials were rejected by GoTrue (HTTP 400 / 422).
    #[error("invalid credentials: {0}")]
    InvalidCredentials(String),

    /// The session token has expired and could not be refreshed.
    #[error("token expired")]
    TokenExpired,

    /// The refresh token is invalid or revoked.
    #[error("invalid refresh token: {0}")]
    InvalidRefreshToken(String),

    /// An HTTP transport error from reqwest.
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),

    /// GoTrue returned an unexpected error body.
    #[error("gotrue error ({status}): {message}")]
    GoTrue { status: u16, message: String },

    /// Environment variable is missing.
    #[error("missing environment variable: {0}")]
    MissingEnv(String),
}

/// Convenience `Result` alias.
pub type AuthResult<T> = std::result::Result<T, AuthError>;
