//! Configuration for the Supabase Realtime WebSocket client.
//!
//! Handles:
//! - [`RealtimeConfig`]: all tunables (URL, key, topic, timers, SPKI pins)
//! - `build_ws_url`: `https://` → `wss://` conversion with loopback bypass
//! - `build_ws_request`: HTTP upgrade request with `apikey` in a header
//!   (CopyPaste-lnjm: never in the URL query string)
//! - [`scrub_ws_url`]: strip query string before logging
//! - `redact_payload`: PII-safe payload summary for log sites
//! - `is_loopback_host`, `extract_host`, `is_truthy`: helpers

use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;

use crate::realtime::{RealtimeError, SpkiPins};

// ── Log redaction (Wave 2.7 sec #17) ─────────────────────────────────────────
//
// Raw Phoenix payloads embed clipboard record JSON (`record.content`, etc.)
// which is end-user plaintext. Logging the full `serde_json::Value` therefore
// leaks user data into the daemon log file. Replace any log site that
// previously emitted `payload = %msg.payload` with `payload = %redact_payload(...)`
// — same fields are still useful for triage (length, fixed-prefix fingerprint)
// without exposing content.

/// Render a JSON payload in a redaction-safe form: `len=<N>, prefix=<hex16>`.
///
/// The serialised representation length and a 16-byte hex fingerprint of the
/// payload are enough for log triage (size class, "is this the same event we
/// saw at 12:03?") while never revealing the underlying clipboard content.
///
/// Stable / deterministic: pure function of the JSON value's canonical
/// serialisation. Suitable for tests that pin the exact output.
pub(crate) fn redact_payload(value: &serde_json::Value) -> String {
    // `to_string` cannot fail for a well-formed `Value`; if it ever did, the
    // fallback `<unserialisable>` is still safe (no content leaked).
    let s = serde_json::to_string(value).unwrap_or_else(|_| String::from("<unserialisable>"));
    let bytes = s.as_bytes();
    let len = bytes.len();
    let take = bytes.len().min(16);
    let prefix_hex = bytes[..take]
        .iter()
        .fold(String::with_capacity(take * 2), |mut acc, b| {
            use std::fmt::Write as _;
            let _ = write!(acc, "{:02x}", b);
            acc
        });
    format!("len={}, prefix={}", len, prefix_hex)
}

// ── Configuration ─────────────────────────────────────────────────────────────

/// Configuration for the Supabase Realtime client.
#[derive(Debug, Clone)]
pub struct RealtimeConfig {
    /// Full WebSocket base URL (scheme + host + path + `?vsn=1.0.0`).
    ///
    /// The `apikey` is intentionally absent from this URL (CopyPaste-lnjm):
    /// it is injected as an HTTP request header during the WS handshake so
    /// it does not appear in proxy / access logs.
    ///
    /// Format: `wss://{project}.supabase.co/realtime/v1/websocket?vsn=1.0.0`
    pub ws_url: String,

    /// Supabase project URL (`https://{project}.supabase.co`).
    pub supabase_url: String,

    /// Supabase anonymous API key.
    pub anon_key: String,

    /// Channel topic to subscribe to (default: `"realtime:clipboard_items"`).
    pub topic: String,

    /// Heartbeat interval (default: 30 s).
    pub heartbeat_interval: Duration,

    /// Initial reconnect delay (default: 1 s). Doubles on each failure up to `max_backoff`.
    pub initial_backoff: Duration,

    /// Maximum reconnect delay (default: 60 s).
    pub max_backoff: Duration,

    /// Outbound event channel capacity (default: 256).
    pub channel_capacity: usize,

    /// Set to `false` to disable the Realtime client entirely (feature flag).
    pub enabled: bool,

    /// The current user JWT used as `Authorization: Bearer` in the channel join
    /// payload.  Wrapped in `Arc<RwLock<…>>` so the daemon can push a refreshed
    /// token without restarting the client — each reconnect's `run_session` call
    /// reads the lock to get the most-recent bearer before sending `phx_join`.
    ///
    /// An empty string means no per-user RLS (anon-key-only access).
    ///
    /// **Contract for the daemon agent:** call
    /// [`super::RealtimeClient::update_jwt`] with the new access token whenever the
    /// GoTrue session is refreshed.  The next WebSocket reconnect (or explicit
    /// disconnect + reconnect) will use the updated token.
    pub user_jwt: Arc<RwLock<String>>,

    /// GoTrue user UUID, used as `filter: "user_id=eq.<uuid>"` in the
    /// `postgres_changes` subscription so the Realtime server pre-filters rows
    /// server-side before RLS applies them.  `None` = anon / no filter.
    pub user_id: Option<String>,

    /// SPKI pin set for TLS certificate pinning of the Realtime WSS endpoint
    /// (CopyPaste-qkao).
    ///
    /// When non-empty, every TLS handshake additionally verifies that the
    /// server's end-entity certificate has an SPKI SHA-256 hash matching one
    /// of these pins. Standard WebPKI chain validation still runs regardless.
    ///
    /// An empty set (the default) skips the SPKI check — use this for
    /// local dev / self-signed setups. For production, populate with at least
    /// one pin derived from the current Supabase project certificate.
    pub spki_pins: SpkiPins,
}

impl RealtimeConfig {
    /// Default topic used for clipboard item synchronisation.
    pub const DEFAULT_TOPIC: &'static str = "realtime:clipboard_items";

    /// Build configuration from environment variables.
    ///
    /// Required env vars:
    /// - `SUPABASE_URL`  — project base URL, e.g. `https://abc.supabase.co`
    /// - `SUPABASE_ANON_KEY` — anon/public API key
    ///
    /// Optional:
    /// - `SUPABASE_REALTIME_TOPIC` — channel topic (default: `realtime:clipboard_items`)
    /// - `SUPABASE_REALTIME_DISABLED=1` — set to `1` to disable
    pub fn from_env() -> Result<Self, RealtimeError> {
        let supabase_url = std::env::var("SUPABASE_URL")
            .map_err(|_| RealtimeError::Config("SUPABASE_URL env var not set".into()))?;
        let anon_key = std::env::var("SUPABASE_ANON_KEY")
            .map_err(|_| RealtimeError::Config("SUPABASE_ANON_KEY env var not set".into()))?;

        // Disabled iff the var is set to a truthy value. The old check
        // (`v != "1"`) inverted this: `=true`/`=yes`/`=TRUE` all silently
        // ENABLED realtime. Treat any of "1"/"true"/"yes" (trimmed,
        // case-insensitive) as a request to disable; everything else enables.
        let enabled = std::env::var("SUPABASE_REALTIME_DISABLED")
            .map(|v| !is_truthy(&v))
            .unwrap_or(true);

        let topic = std::env::var("SUPABASE_REALTIME_TOPIC")
            .unwrap_or_else(|_| Self::DEFAULT_TOPIC.to_owned());

        Ok(Self::with_jwt_and_user_id(
            supabase_url,
            anon_key,
            topic,
            None,
            None,
            enabled,
        ))
    }

    /// Construct config programmatically (no user JWT — anon scope).
    pub fn new(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        topic: impl Into<String>,
        enabled: bool,
    ) -> Self {
        Self::with_jwt_and_user_id(supabase_url, anon_key, topic, None, None, enabled)
    }

    /// Construct config with an explicit user JWT for RLS-aware subscriptions.
    ///
    /// The `user_jwt` is sent as `params.user_token` in the `phx_join` payload
    /// so Supabase Realtime applies the authenticated user's RLS policies when
    /// filtering `postgres_changes` events.  Pass `None` to use anon scope.
    ///
    /// `user_id` is the GoTrue user UUID; when `Some` it is added as a
    /// `filter: "user_id=eq.<uuid>"` clause in the postgres_changes subscription
    /// so the Realtime server pre-filters rows before RLS.  Pass `None` for
    /// anon / single-user deployments.
    pub fn with_jwt(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        topic: impl Into<String>,
        user_jwt: Option<String>,
        enabled: bool,
    ) -> Self {
        Self::with_jwt_and_user_id(supabase_url, anon_key, topic, user_jwt, None, enabled)
    }

    /// Construct config with both JWT and user-id (for row-level filtering).
    ///
    /// Prefer this over [`Self::with_jwt`] when the GoTrue user UUID is available
    /// so that the `postgres_changes` subscription carries a server-side
    /// `filter: "user_id=eq.<uuid>"` clause (audit P1 fix).
    pub fn with_jwt_and_user_id(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        topic: impl Into<String>,
        user_jwt: Option<String>,
        user_id: Option<String>,
        enabled: bool,
    ) -> Self {
        let supabase_url = supabase_url.into();
        let anon_key = anon_key.into();
        let topic = topic.into();

        // Build the WebSocket URL from the REST URL.
        let ws_url = build_ws_url(&supabase_url, &anon_key);

        Self {
            ws_url,
            supabase_url,
            anon_key,
            topic,
            heartbeat_interval: Duration::from_secs(30),
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            channel_capacity: 256,
            enabled,
            user_jwt: Arc::new(RwLock::new(user_jwt.unwrap_or_default())),
            user_id,
            spki_pins: SpkiPins::default(),
        }
    }
}

/// Whether an env-var string represents a truthy/enabled flag value.
///
/// Accepts `1`, `true`, `yes` (trimmed, case-insensitive). Used to interpret
/// `SUPABASE_REALTIME_DISABLED` so that e.g. `=TRUE` disables realtime instead
/// of silently enabling it.
pub(crate) fn is_truthy(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "1" | "true" | "yes")
}

/// Strip the query string from a WebSocket URL so it is safe to log.
///
/// After CopyPaste-lnjm the `ws_url` no longer embeds `apikey` in the query
/// string (it is injected as a request header instead). This function is
/// retained as a belt-and-suspenders measure to scrub any remaining query
/// parameters (e.g. `vsn=1.0.0`) before logging, and to guard against any
/// future regression that inadvertently puts secrets back in the URL.
///
/// # Examples
/// ```
/// # use copypaste_supabase::realtime::scrub_ws_url;
/// let u = "wss://abc.supabase.co/realtime/v1/websocket?vsn=1.0.0";
/// assert_eq!(scrub_ws_url(u), "wss://abc.supabase.co/realtime/v1/websocket");
/// ```
pub fn scrub_ws_url(url: &str) -> &str {
    // Everything before the first `?` is the safe portion.
    match url.find('?') {
        Some(pos) => &url[..pos],
        None => url,
    }
}

/// Build a `tungstenite::http::Request` for the WebSocket handshake.
///
/// This is where the `apikey` header is injected (CopyPaste-lnjm fix):
/// the key is never placed in the URL query string; it travels in the
/// HTTP upgrade request headers only (not recorded by most access logs).
///
/// The `Authorization: Bearer <anon_key>` header is also set as a belt-and-
/// suspenders approach — some Supabase proxy deployments gate on it too.
pub(crate) fn build_ws_request(
    ws_url: &str,
    anon_key: &str,
) -> Result<tokio_tungstenite::tungstenite::http::Request<()>, RealtimeError> {
    use tokio_tungstenite::tungstenite::http::{HeaderValue, Request};
    let anon_hdr = HeaderValue::from_str(anon_key)
        .map_err(|e| RealtimeError::Config(format!("invalid anon key header value: {e}")))?;
    let bearer = HeaderValue::from_str(&format!("Bearer {anon_key}"))
        .map_err(|e| RealtimeError::Config(format!("invalid bearer header value: {e}")))?;

    let req = Request::builder()
        .uri(ws_url)
        // Supabase Realtime checks the `apikey` header for authentication.
        .header("apikey", anon_hdr)
        // Belt-and-suspenders: also set Authorization bearer for proxies
        // that enforce the standard HTTP auth header.
        .header("Authorization", bearer)
        .body(())
        .map_err(|e| RealtimeError::Config(format!("failed to build WS request: {e}")))?;
    Ok(req)
}

/// Return `true` if `host` refers to a loopback address (127.x.x.x,
/// `::1`, or the hostname `"localhost"`), with or without a port suffix.
pub(crate) fn is_loopback_host(host: &str) -> bool {
    // Strip port suffix if present, being careful with IPv6 literals like
    // `[::1]:4000` where we should not strip the last `:` inside the brackets.
    let addr_part = if let Some(idx) = host.rfind(':') {
        let candidate = &host[..idx];
        // Only strip the trailing `:port` if what remains is either a
        // bracketed IPv6 literal (`[...]`) or a plain hostname/IPv4 (no ':').
        if candidate.ends_with(']') || !candidate.contains(':') {
            candidate
        } else {
            host
        }
    } else {
        host
    };

    // Hostname check (most common dev case).
    if addr_part.eq_ignore_ascii_case("localhost") {
        return true;
    }

    // Strip surrounding brackets from IPv6 literals like `[::1]`.
    let addr_str = addr_part.trim_matches(|c| c == '[' || c == ']');
    if let Ok(ip) = addr_str.parse::<std::net::IpAddr>() {
        return ip.is_loopback();
    }
    false
}

/// Extract the host portion from a URL string (`scheme://host[:port]/...`).
pub(crate) fn extract_host(url: &str) -> &str {
    // Strip scheme.
    let after_scheme = if let Some(idx) = url.find("://") {
        &url[idx + 3..]
    } else {
        url
    };
    // Take up to the first '/' or end of string.
    match after_scheme.find('/') {
        Some(idx) => &after_scheme[..idx],
        None => after_scheme,
    }
}

/// Convert a Supabase REST URL to the Realtime WebSocket URL.
///
/// Security (CopyPaste-j21): a plain `http://` URL pointing at a non-loopback
/// host is silently upgraded to `wss://` so that a misconfigured
/// `SUPABASE_URL=http://…` cannot leak the anon API key over an unencrypted
/// connection. `ws://` (plain WebSocket) is only permitted when the host
/// resolves to loopback (127.x.x.x, `::1`, or `localhost`) — i.e. local dev.
pub(crate) fn build_ws_url(base_url: &str, _api_key: &str) -> String {
    // Determine whether the host is loopback-only (local dev).
    let host = extract_host(base_url);
    let host_is_loopback = is_loopback_host(host);

    // Replace http/https scheme with ws/wss.
    // For non-loopback hosts, http:// is upgraded to wss:// (not ws://)
    // to prevent the anon key from travelling in plaintext.
    let ws_base = if base_url.starts_with("https://") {
        base_url.replacen("https://", "wss://", 1)
    } else if base_url.starts_with("http://") {
        if host_is_loopback {
            // Local dev: allow plain ws:// so developers can run a local
            // Supabase instance without TLS.
            base_url.replacen("http://", "ws://", 1)
        } else {
            // Remote host: upgrade to wss:// to prevent the API key from
            // leaking over a plaintext connection.
            tracing::warn!(
                host = %host,
                "Supabase URL uses plain http:// for a non-loopback host; \
                 upgrading to wss:// to protect the API key. \
                 Set SUPABASE_URL to https:// to suppress this warning."
            );
            base_url.replacen("http://", "wss://", 1)
        }
    } else {
        format!("wss://{}", base_url)
    };

    // Strip trailing slash before appending path
    let ws_base = ws_base.trim_end_matches('/');

    // CopyPaste-lnjm: the API key MUST NOT appear in the URL query string —
    // it would be visible in proxy/access logs. Pass it via the `apikey`
    // request header instead (see `build_ws_request`).
    // Only the version parameter (`vsn`) remains in the URL.
    format!("{}/realtime/v1/websocket?vsn=1.0.0", ws_base)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    // ── build_ws_url (CopyPaste-lnjm) ────────────────────────────────────────
    //
    // The URL must NOT contain `apikey` — it is injected as a request header
    // by `build_ws_request` instead.

    #[test]
    fn build_ws_url_converts_https_no_apikey_in_url() {
        // CopyPaste-lnjm: apikey must NOT appear in the URL.
        let url = build_ws_url("https://abc.supabase.co", "mykey");
        assert!(
            !url.contains("apikey"),
            "apikey must not appear in the WS URL; got: {url}"
        );
        assert!(
            !url.contains("mykey"),
            "anon key must not appear in the WS URL; got: {url}"
        );
        assert_eq!(url, "wss://abc.supabase.co/realtime/v1/websocket?vsn=1.0.0");
    }

    #[test]
    fn build_ws_url_converts_http() {
        // Loopback host: plain ws:// is allowed for local dev.
        let url = build_ws_url("http://localhost:4000", "k");
        assert_eq!(url, "ws://localhost:4000/realtime/v1/websocket?vsn=1.0.0");
    }

    #[test]
    fn build_ws_url_upgrades_http_remote_to_wss() {
        // Non-loopback host: http:// must be silently upgraded to wss://
        // to protect the API key (CopyPaste-j21).
        let url = build_ws_url("http://abc.supabase.co", "k");
        assert_eq!(url, "wss://abc.supabase.co/realtime/v1/websocket?vsn=1.0.0");
    }

    #[test]
    fn build_ws_url_handles_trailing_slash() {
        let url = build_ws_url("https://abc.supabase.co/", "k");
        assert_eq!(url, "wss://abc.supabase.co/realtime/v1/websocket?vsn=1.0.0");
    }

    // ── build_ws_request injects apikey header (CopyPaste-lnjm) ──────────────

    #[test]
    fn build_ws_request_apikey_in_header_not_url() {
        let url = "wss://abc.supabase.co/realtime/v1/websocket?vsn=1.0.0";
        let req = build_ws_request(url, "test-anon-key").expect("request builds");

        // The URL must not contain the key.
        let uri_str = req.uri().to_string();
        assert!(
            !uri_str.contains("test-anon-key"),
            "API key must not appear in the URI; got: {uri_str}"
        );
        assert!(
            !uri_str.contains("apikey"),
            "no apikey query param expected in URI; got: {uri_str}"
        );

        // The header must carry the key.
        let apikey_header = req.headers().get("apikey").expect("apikey header missing");
        assert_eq!(
            apikey_header.to_str().unwrap(),
            "test-anon-key",
            "apikey header must equal the anon key"
        );

        // Authorization header must be set.
        let auth_header = req
            .headers()
            .get("Authorization")
            .expect("Authorization header missing");
        assert!(
            auth_header.to_str().unwrap().starts_with("Bearer "),
            "Authorization header must be Bearer; got: {:?}",
            auth_header
        );
    }

    // ── Disable-flag parsing (truthy detection) ───────────────────────────────

    #[test]
    fn is_truthy_recognises_enabled_values() {
        for v in [
            "1", "true", "TRUE", "True", "yes", "YES", " true ", "\tyes\n",
        ] {
            assert!(is_truthy(v), "{v:?} should be truthy (disable realtime)");
        }
    }

    #[test]
    fn is_truthy_rejects_other_values() {
        for v in ["0", "false", "no", "", "off", "2", "disabled", "enable"] {
            assert!(!is_truthy(v), "{v:?} should NOT be truthy");
        }
    }

    #[test]
    #[serial]
    fn disabled_flag_truthy_values_disable_realtime() {
        unsafe { std::env::set_var("SUPABASE_URL", "https://test.supabase.co") };
        unsafe { std::env::set_var("SUPABASE_ANON_KEY", "k") };
        for v in ["1", "true", "TRUE", "yes"] {
            unsafe { std::env::set_var("SUPABASE_REALTIME_DISABLED", v) };
            let cfg = RealtimeConfig::from_env().expect("config builds");
            assert!(
                !cfg.enabled,
                "SUPABASE_REALTIME_DISABLED={v} must DISABLE realtime"
            );
        }
        // Unset / falsey → enabled.
        unsafe { std::env::remove_var("SUPABASE_REALTIME_DISABLED") };
        assert!(
            RealtimeConfig::from_env().expect("config builds").enabled,
            "unset disable flag should leave realtime enabled"
        );
        unsafe { std::env::set_var("SUPABASE_REALTIME_DISABLED", "false") };
        assert!(
            RealtimeConfig::from_env().expect("config builds").enabled,
            "SUPABASE_REALTIME_DISABLED=false should leave realtime enabled"
        );
        unsafe { std::env::remove_var("SUPABASE_REALTIME_DISABLED") };
        unsafe { std::env::remove_var("SUPABASE_URL") };
        unsafe { std::env::remove_var("SUPABASE_ANON_KEY") };
    }

    // ── RealtimeConfig ────────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn config_from_env_requires_supabase_url() {
        // Remove env vars to test missing SUPABASE_URL
        unsafe { std::env::remove_var("SUPABASE_URL") };
        unsafe { std::env::remove_var("SUPABASE_ANON_KEY") };
        let result = RealtimeConfig::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("SUPABASE_URL"),
            "error should mention SUPABASE_URL, got: {err}"
        );
    }

    #[test]
    #[serial]
    fn config_from_env_requires_anon_key() {
        unsafe { std::env::set_var("SUPABASE_URL", "https://test.supabase.co") };
        unsafe { std::env::remove_var("SUPABASE_ANON_KEY") };
        let result = RealtimeConfig::from_env();
        unsafe { std::env::remove_var("SUPABASE_URL") };
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("SUPABASE_ANON_KEY"),
            "error should mention SUPABASE_ANON_KEY, got: {err}"
        );
    }

    #[test]
    fn config_new_defaults_are_sensible() {
        let config = RealtimeConfig::new(
            "https://abc.supabase.co",
            "anon-key",
            RealtimeConfig::DEFAULT_TOPIC,
            true,
        );
        assert_eq!(config.heartbeat_interval, Duration::from_secs(30));
        assert_eq!(config.initial_backoff, Duration::from_secs(1));
        assert_eq!(config.max_backoff, Duration::from_secs(60));
        assert_eq!(config.topic, "realtime:clipboard_items");
        assert!(config.enabled);
        assert!(config.ws_url.contains("vsn=1.0.0"));
        // CopyPaste-lnjm: apikey must NOT appear in the stored ws_url.
        assert!(
            !config.ws_url.contains("apikey"),
            "ws_url must not embed apikey; got: {}",
            config.ws_url
        );
        assert!(
            !config.ws_url.contains("anon-key"),
            "ws_url must not embed the anon key; got: {}",
            config.ws_url
        );
        // CopyPaste-qkao: default SPKI pins are empty (no pinning configured).
        assert!(
            config.spki_pins.is_empty(),
            "default spki_pins must be empty"
        );
    }

    #[test]
    fn config_disabled_feature_flag() {
        let config = RealtimeConfig::new(
            "https://abc.supabase.co",
            "k",
            RealtimeConfig::DEFAULT_TOPIC,
            false,
        );
        assert!(!config.enabled);
    }

    // ── scrub_ws_url ──────────────────────────────────────────────────────────

    #[test]
    fn scrub_ws_url_strips_query_string() {
        // After CopyPaste-lnjm the URL only contains vsn= in the query string,
        // but scrub_ws_url is retained as a belt-and-suspenders guard.
        let url = "wss://abc.supabase.co/realtime/v1/websocket?vsn=1.0.0";
        let scrubbed = scrub_ws_url(url);
        assert!(
            !scrubbed.contains("vsn"),
            "scrubbed URL must not contain query params, got: {scrubbed}"
        );
        assert!(
            scrubbed.contains("wss://abc.supabase.co"),
            "scrubbed URL must still contain the host, got: {scrubbed}"
        );
    }

    #[test]
    fn scrub_ws_url_guards_against_accidental_apikey_in_url() {
        // Belt-and-suspenders: even if a misconfiguration puts apikey back in the URL,
        // scrub_ws_url must strip it before logging.
        let url = "wss://abc.supabase.co/realtime/v1/websocket?apikey=accidental-key&vsn=1.0.0";
        let scrubbed = scrub_ws_url(url);
        assert!(
            !scrubbed.contains("apikey"),
            "scrubbed URL must not contain 'apikey', got: {scrubbed}"
        );
        assert!(
            !scrubbed.contains("accidental-key"),
            "scrubbed URL must not contain the key value, got: {scrubbed}"
        );
    }

    #[test]
    fn scrub_ws_url_no_query_unchanged() {
        let url = "wss://abc.supabase.co/realtime/v1/websocket";
        let scrubbed = scrub_ws_url(url);
        assert_eq!(scrubbed, url);
    }

    // ── Payload redaction (Wave 2.7 sec #17) ──────────────────────────────────

    /// `redact_payload` must NEVER include the raw record content (clipboard
    /// plaintext) in its output. It must surface only length + a fixed-size
    /// hex prefix of the JSON serialisation. This is the contract every
    /// log call site relies on for compliance with the user-data redaction
    /// requirement.
    #[test]
    fn payload_redacted_in_logs() {
        // Plaintext that MUST NOT appear in the redacted form.
        let secret = "super-secret-clipboard-contents-do-not-leak-abc123";
        let payload = serde_json::json!({
            "data": {
                "type": "INSERT",
                "table": "clipboard_items",
                "record": { "id": "abc", "content_type": "text", "content": secret },
            }
        });

        let redacted = redact_payload(&payload);

        // 1. No raw plaintext.
        assert!(
            !redacted.contains(secret),
            "redacted form must not contain raw payload content; got: {redacted}"
        );
        // 2. Also no obvious JSON keys from `record` that imply we dumped the value.
        assert!(
            !redacted.contains("content_type"),
            "redacted form must not include JSON keys from the original payload; got: {redacted}"
        );

        // 3. Must still carry usable triage signal (length + prefix).
        assert!(
            redacted.contains("len="),
            "expected length field in: {redacted}"
        );
        assert!(
            redacted.contains("prefix="),
            "expected prefix field in: {redacted}"
        );

        // 4. The prefix is a hex string of the first 16 bytes of the canonical
        //    JSON serialisation — deterministic, so we can pin it.
        let canonical = serde_json::to_string(&payload).expect("serialise");
        let expected_prefix: String = canonical
            .as_bytes()
            .iter()
            .take(16)
            .map(|b| format!("{:02x}", b))
            .collect();
        assert!(
            redacted.contains(&expected_prefix),
            "expected prefix {expected_prefix} in redacted: {redacted}"
        );
        // 5. The reported length must equal the serialised byte length.
        assert!(
            redacted.contains(&format!("len={}", canonical.len())),
            "expected len={} in redacted: {redacted}",
            canonical.len()
        );
    }

    /// Edge cases — empty object and short payloads must not panic and must
    /// still produce a coherent redacted form.
    #[test]
    fn payload_redaction_handles_short_and_empty() {
        let empty = serde_json::json!({});
        let r = redact_payload(&empty);
        assert!(
            r.contains("len=2"),
            "empty object serialises to '{{}}' (2 bytes); got: {r}"
        );

        let tiny = serde_json::json!("x");
        let r = redact_payload(&tiny);
        // "\"x\"" → 3 bytes
        assert!(
            r.contains("len=3"),
            "tiny string payload should be 3 bytes; got: {r}"
        );
    }
}
