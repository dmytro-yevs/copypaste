//! [`RestClient`]: construction and shared state.
//!
//! HTTP request-building helpers live in [`super::http`]; the read, write, and
//! reencrypt operations are additional `impl RestClient` blocks in
//! [`super::read`], [`super::write`], and [`super::reencrypt`] respectively.

use reqwest::Client;

use super::error::{RestError, RestResult};

/// Default HTTP timeout for all PostgREST requests.
///
/// Matches `SYNC_HTTP_TIMEOUT` in the daemon's `sync_common.rs` (30 s). A
/// single constant here avoids divergence: callers that construct `RestClient`
/// directly via [`RestClient::new`] get the same guard as the daemon push/poll
/// loops.
///
/// # CopyPaste-16vr
///
/// The previous `RestClient::new()` used `Client::new()` with no timeout, so
/// a stalled Supabase endpoint could block the `reencrypt_all_cloud_items` call
/// (invoked from `rotate_sync_key`) indefinitely. This constant applies the
/// same 30 s guard as the daemon push/poll loops.
const REST_HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// PostgREST client scoped to the `clipboard_items` table.
///
/// Requires:
/// - `supabase_url` — project REST URL (`https://{project}.supabase.co`)
/// - `anon_key` — anonymous API key (used as `apikey` header)
/// - `access_token` — user JWT (used as `Authorization: Bearer`)
///
/// The client is cheaply cloneable (all members are `Arc<_>` internally via
/// `reqwest::Client`).
#[derive(Clone)]
pub struct RestClient {
    pub(super) http: Client,
    pub(super) base_url: String,
    pub(super) anon_key: String,
    pub(super) access_token: String,
}

/// Manual `Debug` impl that redacts `anon_key` and `access_token`.
///
/// # CopyPaste-hp4h — secret leak via `{:?}`
///
/// The previous `#[derive(Debug)]` printed `anon_key`/`access_token` verbatim.
/// Any accidental `{:?}` logging of a `RestClient` (e.g. in a `tracing::debug!`
/// or a panic message) would leak the Supabase anon key or the user's bearer
/// JWT. Both secrets are now redacted; all other fields remain readable for
/// diagnostics.
impl std::fmt::Debug for RestClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RestClient")
            .field("http", &self.http)
            .field("base_url", &self.base_url)
            .field("anon_key", &"[redacted]")
            .field("access_token", &"[redacted]")
            .finish()
    }
}

impl RestClient {
    /// Construct from explicit credentials.
    ///
    /// `supabase_url` should be the HTTPS base URL (e.g.
    /// `https://abc.supabase.co`). The `/rest/v1/clipboard_items` path is
    /// appended automatically.
    ///
    /// # CopyPaste-16vr
    ///
    /// Uses a 30 s HTTP timeout (`REST_HTTP_TIMEOUT`) so a stalled Supabase
    /// endpoint cannot block a sync loop indefinitely.
    pub fn new(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        access_token: impl Into<String>,
    ) -> Self {
        // TLS cert-store load cannot fail on macOS/Linux in normal operation.
        // Propagate via expect rather than silently falling back to a no-timeout
        // client (which would be worse than aborting on a stalled endpoint).
        let http = Client::builder()
            .timeout(REST_HTTP_TIMEOUT)
            .build()
            .expect("reqwest Client::builder should not fail on supported platforms");
        Self {
            http,
            base_url: supabase_url.into().trim_end_matches('/').to_string(),
            anon_key: anon_key.into(),
            access_token: access_token.into(),
        }
    }

    /// Construct from explicit credentials with a caller-supplied HTTP client.
    ///
    /// Use this when the caller already holds a `reqwest::Client` configured
    /// with specific timeouts, TLS settings, or a connection pool — for example,
    /// the `rotate_sync_key` IPC handler that reuses the daemon's live bearer
    /// without going through `from_env()`.  Prefer [`RestClient::new`] for
    /// one-shot callers.
    ///
    /// # CopyPaste-n4dt
    ///
    /// The original `rotate_sync_key` path called `RestClient::from_env()`, which
    /// requires `SUPABASE_ACCESS_TOKEN` — an env var never set in production (auth
    /// is managed by GoTrue inside the daemon). Callers that hold the live bearer
    /// `Arc<RwLock<String>>` should construct via this method instead, bypassing
    /// the env-var requirement.
    pub fn with_http_client(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        access_token: impl Into<String>,
        http: Client,
    ) -> Self {
        Self {
            http,
            base_url: supabase_url.into().trim_end_matches('/').to_string(),
            anon_key: anon_key.into(),
            access_token: access_token.into(),
        }
    }

    /// Build from environment variables.
    ///
    /// Required env vars: `SUPABASE_URL`, `SUPABASE_ANON_KEY`,
    /// `SUPABASE_ACCESS_TOKEN`.
    pub fn from_env() -> RestResult<Self> {
        let url = std::env::var("SUPABASE_URL")
            .map_err(|_| RestError::InvalidArgument("SUPABASE_URL env var not set".into()))?;
        let key = std::env::var("SUPABASE_ANON_KEY")
            .map_err(|_| RestError::InvalidArgument("SUPABASE_ANON_KEY env var not set".into()))?;
        let token = std::env::var("SUPABASE_ACCESS_TOKEN").map_err(|_| {
            RestError::InvalidArgument("SUPABASE_ACCESS_TOKEN env var not set".into())
        })?;
        Ok(Self::new(url, key, token))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_client_new_trims_trailing_slash() {
        let client = RestClient::new("https://abc.supabase.co/", "anon-key", "access-token");
        // base_url must not have a trailing slash before /rest/v1/...
        assert_eq!(
            client.table_url(),
            "https://abc.supabase.co/rest/v1/clipboard_items"
        );
    }

    // -----------------------------------------------------------------------
    // CopyPaste-hp4h: RestClient's Debug impl must redact secrets
    // -----------------------------------------------------------------------

    /// `{:?}` on a `RestClient` must never print the anon key or the bearer
    /// access token verbatim.
    #[test]
    fn rest_client_debug_redacts_anon_key_and_access_token() {
        let client = RestClient::new(
            "https://abc.supabase.co",
            "super-secret-anon-key",
            "super-secret-access-token",
        );
        let debug_str = format!("{client:?}");
        assert!(
            !debug_str.contains("super-secret-anon-key"),
            "Debug output must not leak anon_key; got: {debug_str}"
        );
        assert!(
            !debug_str.contains("super-secret-access-token"),
            "Debug output must not leak access_token; got: {debug_str}"
        );
        assert!(
            debug_str.contains("[redacted]"),
            "Debug output must contain the redaction marker; got: {debug_str}"
        );
    }
}
