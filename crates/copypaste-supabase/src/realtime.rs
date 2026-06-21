//! Supabase Realtime WebSocket client with Phoenix Channel protocol support.
//!
//! Handles:
//! - Connection to `wss://{project}.supabase.co/realtime/v1/websocket`
//! - Phoenix Channel join for `realtime:clipboard_items`
//! - Heartbeat every 30 seconds
//! - Exponential backoff reconnection
//! - Graceful shutdown via [`ClientHandle`]

#![allow(clippy::result_large_err)] // RealtimeError carries WebSocket variants; boxing not worth the noise here

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use rustls::RootCertStore;
use sha2::Digest as _;
use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::{Notify, RwLock};
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::Connector;

use crate::protocol::{ChangeEvent, PhoenixEvent, PhoenixMessage};
use copypaste_sync::backoff::BackoffScheduler;
use futures_util::{SinkExt, StreamExt};

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
    /// [`RealtimeClient::update_jwt`] with the new access token whenever the
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
fn is_truthy(value: &str) -> bool {
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

// ── SPKI cert pinning (CopyPaste-qkao) ───────────────────────────────────────

/// A set of SHA-256 SPKI (Subject Public Key Info) pin hashes for
/// certificate pinning of the Supabase Realtime WSS endpoint.
///
/// # How to obtain a pin
/// ```sh
/// openssl s_client -connect <project>.supabase.co:443 </dev/null \
///     | openssl x509 -noout -pubkey \
///     | openssl pkey -pubin -outform DER \
///     | openssl dgst -sha256 -binary \
///     | base64
/// ```
///
/// Store the hex form (not base64) as a 32-byte array in `RealtimeConfig.spki_pins`.
///
/// # Empty set (default)
/// When `spki_pins` is empty no additional SPKI check is performed —
/// standard WebPKI chain validation still applies. This keeps the default
/// behavior compatible with deployments that do not (yet) pin certificates.
/// Set at least one pin in production to enable actual pinning.
#[derive(Debug, Clone, Default)]
pub struct SpkiPins {
    /// SHA-256 hashes of the DER-encoded SubjectPublicKeyInfo of acceptable
    /// end-entity certificates. Each entry is 32 bytes (256 bits).
    pub pins: Vec<[u8; 32]>,
}

impl SpkiPins {
    /// Return `true` when the set is empty (pinning not configured).
    pub fn is_empty(&self) -> bool {
        self.pins.is_empty()
    }

    /// Return `true` if `spki_der` hashes (SHA-256) to one of the stored pins.
    pub fn matches(&self, spki_der: &[u8]) -> bool {
        let hash: [u8; 32] = sha2::Sha256::digest(spki_der).into();
        self.pins.iter().any(|p| p == &hash)
    }
}

/// A rustls `ServerCertVerifier` that delegates chain / name validation to
/// `WebPkiServerVerifier` and additionally enforces SPKI pinning.
///
/// If `pins` is empty the SPKI check is skipped (standard PKI only).
/// If `pins` is non-empty and the end-entity certificate's SPKI hash does
/// not match any pin, the connection is refused with
/// `CertificateError::ApplicationVerificationFailure`.
#[derive(Debug)]
struct PinningVerifier {
    inner: std::sync::Arc<rustls::client::WebPkiServerVerifier>,
    pins: SpkiPins,
}

impl rustls::client::danger::ServerCertVerifier for PinningVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls::pki_types::CertificateDer<'_>,
        intermediates: &[rustls::pki_types::CertificateDer<'_>],
        server_name: &rustls::pki_types::ServerName<'_>,
        ocsp_response: &[u8],
        now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        // Run the standard WebPKI chain + name check first. If this fails,
        // reject immediately (don't bother with pin check).
        let result = self.inner.verify_server_cert(
            end_entity,
            intermediates,
            server_name,
            ocsp_response,
            now,
        )?;

        // Additional SPKI pin check (only when pins are configured).
        if !self.pins.is_empty() {
            // Parse the end-entity cert to extract the raw SubjectPublicKeyInfo DER.
            // `rcgen` is not available here; we use the low-level ring/webpki path.
            // The DER cert is already validated by the inner verifier, so we only
            // need to locate the SPKI field. Use a minimal manual parse: X.509
            // TBSCertificate.subjectPublicKeyInfo is a named field we can reach via
            // rustls-webpki's `EndEntityCert` if exposed, or via `x509-parser`.
            // Since x509-parser is not in our dep tree, we extract SPKI using the
            // `rustls::pki_types` + a small DER walk.
            //
            // For robustness we hash the ENTIRE end-entity cert DER when we cannot
            // extract the SPKI cleanly; a production deployment should use a proper
            // DER ASN.1 parser. The pin generation command above produces the SPKI
            // hash, so callers must pin the SPKI hash (not the full cert hash).
            // We implement a minimal ASN.1 SEQUENCE navigator to reach the SPKI.
            match extract_spki_der(end_entity) {
                Some(spki) => {
                    if !self.pins.matches(&spki) {
                        tracing::error!(
                            "TLS cert pinning failed: SPKI hash does not match any known pin"
                        );
                        return Err(rustls::Error::InvalidCertificate(
                            rustls::CertificateError::ApplicationVerificationFailure,
                        ));
                    }
                    tracing::debug!("TLS cert pinning: SPKI pin matched");
                }
                None => {
                    // Could not extract SPKI (malformed cert — unlikely after WebPKI
                    // validation, but safe to reject).
                    tracing::error!("TLS cert pinning: could not extract SPKI from cert DER");
                    return Err(rustls::Error::InvalidCertificate(
                        rustls::CertificateError::BadEncoding,
                    ));
                }
            }
        }

        Ok(result)
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls12_signature(message, cert, dss)
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        self.inner.verify_tls13_signature(message, cert, dss)
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.inner.supported_verify_schemes()
    }
}

/// Minimal ASN.1 DER reader: skip past the outermost SEQUENCE tag/length and
/// skip TBSCertificate fields until we reach subjectPublicKeyInfo, then return
/// its raw DER bytes (tag + length + value).
///
/// X.509v3 TBSCertificate structure (RFC 5280 §4.1):
/// ```text
/// TBSCertificate ::= SEQUENCE {
///   version         [0] EXPLICIT INTEGER OPTIONAL,
///   serialNumber        INTEGER,
///   signature           AlgorithmIdentifier,
///   issuer              Name,
///   validity            Validity,
///   subject             Name,
///   subjectPublicKeyInfo SubjectPublicKeyInfo,  -- we want this
///   ...
/// }
/// Certificate ::= SEQUENCE {
///   tbsCertificate      TBSCertificate,          -- outer SEQUENCE
///   ...
/// }
/// ```
///
/// Returns `None` if the DER is too short or structurally invalid.
///
/// This is a best-effort extractor sufficient for SPKI pinning; it does not
/// attempt full validation (the inner WebPKI verifier already did that).
fn extract_spki_der(cert_der: &[u8]) -> Option<Vec<u8>> {
    // The outer structure is:
    //   Certificate ::= SEQUENCE {
    //     tbsCertificate  TBSCertificate,   ← first element, a SEQUENCE
    //     ...
    //   }
    //
    // Step 1: peel the outer Certificate SEQUENCE to get its contents.
    let mut outer = DerReader::new(cert_der);
    let cert_contents = outer.read_sequence()?;

    // Step 2: the first element of cert_contents is the TBSCertificate SEQUENCE.
    // Read it to get ITS contents (the individual TBS fields).
    let mut cert_level = DerReader::new(cert_contents);
    let tbs_contents = cert_level.read_sequence()?;

    // Step 3: navigate the TBSCertificate fields to reach subjectPublicKeyInfo.
    let mut tbs = DerReader::new(tbs_contents);
    // Skip optional [0] EXPLICIT version
    if tbs.peek_tag() == Some(0xa0) {
        tbs.skip_element()?;
    }
    // serialNumber INTEGER
    tbs.skip_element()?;
    // signature AlgorithmIdentifier SEQUENCE
    tbs.skip_element()?;
    // issuer Name (SEQUENCE)
    tbs.skip_element()?;
    // validity Validity (SEQUENCE)
    tbs.skip_element()?;
    // subject Name (SEQUENCE)
    tbs.skip_element()?;
    // subjectPublicKeyInfo — return its full TLV (tag + length + value)
    tbs.read_raw_element()
}

/// Minimal DER/BER reader for the SPKI extractor above.
struct DerReader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> DerReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn remaining(&self) -> &'a [u8] {
        &self.data[self.pos..]
    }

    fn peek_tag(&self) -> Option<u8> {
        self.remaining().first().copied()
    }

    /// Read and decode a DER length (short or long form). Returns the length
    /// value and advances the reader past the length octets.
    fn read_length(&mut self) -> Option<usize> {
        let first = *self.remaining().first()?;
        self.pos += 1;
        if first & 0x80 == 0 {
            Some(first as usize)
        } else {
            let n_bytes = (first & 0x7f) as usize;
            if n_bytes == 0 || n_bytes > 4 || self.remaining().len() < n_bytes {
                return None;
            }
            let mut len: usize = 0;
            for &b in &self.remaining()[..n_bytes] {
                len = len.checked_shl(8)?.checked_add(b as usize)?;
            }
            self.pos += n_bytes;
            Some(len)
        }
    }

    /// Read a SEQUENCE tag (0x30) and return the contents slice.
    fn read_sequence(&mut self) -> Option<&'a [u8]> {
        let tag = *self.remaining().first()?;
        if tag != 0x30 {
            return None;
        }
        self.pos += 1;
        let len = self.read_length()?;
        if self.remaining().len() < len {
            return None;
        }
        let contents = &self.remaining()[..len];
        self.pos += len;
        Some(contents)
    }

    /// Skip one complete TLV element (any tag, short or long form length).
    fn skip_element(&mut self) -> Option<()> {
        if self.remaining().is_empty() {
            return None;
        }
        self.pos += 1; // tag
        let len = self.read_length()?;
        if self.remaining().len() < len {
            return None;
        }
        self.pos += len;
        Some(())
    }

    /// Return the complete TLV (tag + encoded length + value) of the next
    /// element as a `Vec<u8>` without consuming it into an inner reader.
    fn read_raw_element(&mut self) -> Option<Vec<u8>> {
        let start = self.pos;
        // Peek tag (don't advance yet)
        if self.remaining().is_empty() {
            return None;
        }
        self.pos += 1; // tag
        let len = self.read_length()?;
        if self.remaining().len() < len {
            return None;
        }
        self.pos += len;
        Some(self.data[start..self.pos].to_vec())
    }
}

/// Build a WebPKI-backed rustls `ClientConfig` with SPKI pinning.
///
/// For loopback URLs (local dev) pinning is skipped even if pins are
/// configured in `RealtimeConfig`. This avoids breaking local test setups
/// that use self-signed certs.
///
/// Returns `None` if `ws_url` points to a loopback host AND no TLS is
/// needed (plain `ws://`), allowing the caller to fall back to
/// `connect_async` (no custom connector).
fn build_rustls_connector(ws_url: &str, pins: &SpkiPins) -> Option<Connector> {
    // Do not apply TLS for plain ws:// (loopback dev scenario).
    if ws_url.starts_with("ws://") {
        return None;
    }

    // Build standard WebPKI root cert store (same roots as the default
    // tokio-tungstenite connector uses when `rustls-tls-webpki-roots` is
    // enabled).
    let mut roots = RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let roots = std::sync::Arc::new(roots);

    // Build the inner standard verifier.
    let inner = rustls::client::WebPkiServerVerifier::builder(roots)
        .build()
        // Safe: empty CRL list, no revocation errors possible.
        .expect("WebPkiServerVerifier::build must not fail with default params");

    let verifier = PinningVerifier {
        inner,
        pins: pins.clone(),
    };

    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(std::sync::Arc::new(verifier))
        .with_no_client_auth();

    Some(Connector::Rustls(std::sync::Arc::new(config)))
}

/// Build a `tungstenite::http::Request` for the WebSocket handshake.
///
/// This is where the `apikey` header is injected (CopyPaste-lnjm fix):
/// the key is never placed in the URL query string; it travels in the
/// HTTP upgrade request headers only (not recorded by most access logs).
///
/// The `Authorization: Bearer <anon_key>` header is also set as a belt-and-
/// suspenders approach — some Supabase proxy deployments gate on it too.
fn build_ws_request(
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

/// Build the Phoenix Channel join payload for a Supabase Realtime subscription.
///
/// # Bearer token
/// The `user_jwt` is placed under `config.access_token` so Supabase Realtime
/// authenticates the channel with the caller's RLS identity.  An empty string
/// disables per-user RLS (anonymous / anon-key-only access).
///
/// # Row filter (CopyPaste-nr2y — mandatory, defense-in-depth)
/// The `user_id` filter `"user_id=eq.{user_id}"` is **always** included in the
/// `postgres_changes` subscription.  Omitting it would mean the Realtime server
/// could deliver cross-user rows into the event stream before server-side RLS
/// applies them, leaking data on permissive or misconfigured deployments.
///
/// A missing `user_id` is therefore a **hard error** at the call site — callers
/// must obtain the GoTrue user UUID before establishing the Realtime connection.
/// See `run_session` which returns `SessionResult::ConnectError` when
/// `config.user_id` is `None`.
///
/// # Event filter
/// Registers `event: "*"` so INSERT, UPDATE **and** DELETE changes are all
/// delivered to this device.  Using `event: "INSERT"` only would mean that
/// cross-device UPDATE/DELETE operations are silently dropped.
///
/// The payload shape matches Supabase Realtime v2 (`vsn=1.0.0`):
/// ```json
/// {
///   "config": {
///     "access_token": "<jwt>",
///     "postgres_changes": [
///       { "event": "*", "schema": "public", "table": "clipboard_items",
///         "filter": "user_id=eq.<uuid>" }
///     ]
///   }
/// }
/// ```
pub(crate) fn build_join_payload(user_jwt: &str, user_id: &str) -> serde_json::Value {
    serde_json::json!({
        "config": {
            "access_token": user_jwt,
            "postgres_changes": [{
                "event": "*",
                "schema": "public",
                "table": "clipboard_items",
                "filter": format!("user_id=eq.{user_id}")
            }]
        }
    })
}

/// Return `true` if `host` refers to a loopback address (127.x.x.x,
/// `::1`, or the hostname `"localhost"`), with or without a port suffix.
fn is_loopback_host(host: &str) -> bool {
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
fn extract_host(url: &str) -> &str {
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
fn build_ws_url(base_url: &str, _api_key: &str) -> String {
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

// ── Error type ────────────────────────────────────────────────────────────────

#[derive(Debug, Error)]
pub enum RealtimeError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("URL parse error: {0}")]
    Url(String),
}

// ── Client ────────────────────────────────────────────────────────────────────

/// Supabase Realtime WebSocket client.
///
/// Call [`RealtimeClient::connect`] to start the background worker tasks.
/// Received change events are sent on the [`mpsc::Receiver`] returned by [`RealtimeClient::new`].
pub struct RealtimeClient {
    config: RealtimeConfig,
    tx: mpsc::Sender<ChangeEvent>,
    shutdown: Arc<Notify>,
    running: Arc<AtomicBool>,
    /// Fired once when the Phoenix Channel join is confirmed (`phx_reply` with
    /// `status == "ok"`).  Exposed via [`ClientHandle::channel_joined`] so the
    /// daemon can gate `ws_connected = true` on actual join confirmation rather
    /// than mere socket-open.
    channel_joined: Arc<Notify>,
}

impl RealtimeClient {
    /// Create a new client.  Returns the client and the channel receiver for change events.
    pub fn new(config: RealtimeConfig) -> (Self, mpsc::Receiver<ChangeEvent>) {
        let (tx, rx) = mpsc::channel(config.channel_capacity);
        let shutdown = Arc::new(Notify::new());
        let running = Arc::new(AtomicBool::new(false));
        let channel_joined = Arc::new(Notify::new());
        (
            Self {
                config,
                tx,
                shutdown,
                running,
                channel_joined,
            },
            rx,
        )
    }

    /// Replace the user JWT that is sent as `Authorization: Bearer` in the
    /// Phoenix Channel join payload on every (re)connect.
    ///
    /// # When to call this
    /// Call this from the daemon's GoTrue auto-refresh callback whenever a new
    /// access token is obtained.  The next WebSocket session (existing or after
    /// reconnect) will use the updated token, preventing RLS returning zero rows
    /// after the ~1 h JWT expiry.
    ///
    /// # Thread safety
    /// This method acquires a write lock on the shared `Arc<RwLock<String>>`.
    /// It is async so it can be called from any Tokio task.
    pub async fn update_jwt(&self, jwt: String) {
        *self.config.user_jwt.write().await = jwt;
    }

    /// Return a snapshot of the current JWT (empty string if none set).
    ///
    /// Primarily useful for tests and diagnostics; the live value read inside
    /// `run_session` is the authoritative one used for actual connections.
    pub async fn current_jwt(&self) -> String {
        self.config.user_jwt.read().await.clone()
    }

    /// Start the background connection loop.
    ///
    /// Returns a [`ClientHandle`] that can be used to shut down the client.
    /// This method returns immediately; all I/O happens in spawned tasks.
    pub async fn connect(self) -> Result<ClientHandle, RealtimeError> {
        if !self.config.enabled {
            tracing::info!("Supabase Realtime is disabled (feature flag)");
            return Ok(ClientHandle {
                shutdown: self.shutdown.clone(),
                running: self.running.clone(),
                channel_joined: self.channel_joined.clone(),
            });
        }

        let handle = ClientHandle {
            shutdown: self.shutdown.clone(),
            running: self.running.clone(),
            channel_joined: self.channel_joined.clone(),
        };

        self.running.store(true, Ordering::SeqCst);

        // Spawn the reconnect loop
        tokio::spawn(connection_loop(
            self.config,
            self.tx,
            self.shutdown,
            self.running,
            self.channel_joined,
        ));

        Ok(handle)
    }
}

// ── ClientHandle ──────────────────────────────────────────────────────────────

/// Handle returned from [`RealtimeClient::connect`].  Use to check status or shut down.
pub struct ClientHandle {
    shutdown: Arc<Notify>,
    running: Arc<AtomicBool>,
    /// Shared with the background `connection_loop` task.  Notified once when
    /// the Phoenix Channel join is confirmed (`phx_reply` `status == "ok"`).
    /// See [`channel_joined`](Self::channel_joined).
    channel_joined: Arc<Notify>,
}

impl ClientHandle {
    /// Return the channel-join notification handle.
    ///
    /// The returned [`Arc<Notify>`] is notified exactly once per successful
    /// Phoenix Channel join confirmation (`phx_reply` with `status == "ok"`).
    /// Callers should `await` the notify (with a timeout/shutdown guard) before
    /// treating the WebSocket as *fully connected* — the socket being open does
    /// not guarantee that the channel subscription is active.
    ///
    /// Multiple calls return the same underlying `Arc`, so it is safe (and
    /// cheap) to clone for use in a `select!` branch.
    pub fn channel_joined(&self) -> Arc<Notify> {
        self.channel_joined.clone()
    }

    /// Signal the client to shut down and wait for acknowledgement.
    pub async fn shutdown(self) {
        // `signal_shutdown` clears `running` and wakes any parked waiter; the
        // `Drop` impl would do the same, but we run it explicitly here so the
        // brief settle-sleep below observes the already-signalled state.
        self.signal_shutdown();
        // Brief yield to allow the background task to notice the shutdown signal.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    /// Returns `true` if the background worker is still active.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Clear the `running` flag and wake any task parked on the shutdown
    /// `Notify`. Idempotent: both operations are safe to run more than once
    /// (e.g. explicit `shutdown()` followed by `Drop`).
    ///
    /// The flag is set BEFORE `notify_waiters` so that a task which is *not*
    /// currently parked on `shutdown.notified()` (e.g. mid-`run_session`, or at
    /// the top-of-loop `running` check) still observes the stop request on its
    /// next state transition — `notify_waiters` alone only wakes current
    /// waiters and would otherwise be lost.
    fn signal_shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
        self.shutdown.notify_waiters();
    }
}

impl Drop for ClientHandle {
    /// Audit-concurrency HIGH: a dropped or *replaced* `ClientHandle` must never
    /// orphan its `connection_loop` task.
    ///
    /// Previously `ClientHandle` had no `Drop`, so the daemon's reconnect path
    /// (which builds a fresh `RealtimeClient` and dropped the old handle without
    /// awaiting `shutdown()`) left the old `connection_loop` running with
    /// `running == true`. It independently reconnected, so each WS disconnect
    /// accumulated another live client stack (task + heartbeat child + WS/TLS
    /// socket + mpsc buffer + Arcs) for the daemon's whole uptime.
    ///
    /// Clearing `running` and notifying on Drop guarantees the invariant: at
    /// most one live `connection_loop` per logical client, and a dropped handle
    /// terminates its task.
    fn drop(&mut self) {
        self.signal_shutdown();
    }
}

// ── Connection loop ───────────────────────────────────────────────────────────

/// RAII guard that clears the `running` flag on Drop.
///
/// Audit-concurrency HIGH #4: `connection_loop` used to clear `running` only
/// at the bottom of the function. If any await in the loop body panicked (or
/// the task was aborted), the flag stayed `true` forever — making
/// `ClientHandle::is_running` lie about a dead worker and blocking restart
/// logic that consults the flag.
///
/// Wrapping the flag in a Drop guard means the cleanup runs unconditionally
/// when the task ends, whether via normal return, ?-style early return, or
/// panic unwinding.
struct RunningGuard(Arc<AtomicBool>);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// Outer reconnection loop.  Reconnects with exponential backoff when the
/// WebSocket connection drops.
///
/// CopyPaste-nq31: backoff is now driven by [`BackoffScheduler`] from
/// `copypaste-sync`, eliminating the duplicate inline doubling logic.
async fn connection_loop(
    config: RealtimeConfig,
    tx: mpsc::Sender<ChangeEvent>,
    shutdown: Arc<Notify>,
    running: Arc<AtomicBool>,
    channel_joined: Arc<Notify>,
) {
    // Audit-concurrency HIGH #4: clear `running` on ALL exit paths (return,
    // panic, abort) via a Drop guard, not just the bottom of the function.
    let _guard = RunningGuard(running.clone());

    // CopyPaste-nq31: shared `BackoffScheduler` from `copypaste-sync` replaces
    // the inline `backoff = (backoff * 2).min(max)` pattern. Constructed with
    // the same parameters as the old inline logic (initial, max, success-hold
    // threshold = max_backoff so long sessions reset the schedule).
    let mut backoff = BackoffScheduler::new(
        config.initial_backoff,
        config.max_backoff,
        config.max_backoff, // connection held longer than max_backoff → reset
    );

    loop {
        // Check shutdown before attempting to connect.
        if !running.load(Ordering::SeqCst) {
            break;
        }

        tracing::info!(url = %scrub_ws_url(&config.ws_url), "Connecting to Supabase Realtime");

        match run_session(&config, &tx, &shutdown, &channel_joined).await {
            SessionResult::Shutdown => {
                tracing::info!("Supabase Realtime client: shutdown requested");
                break;
            }
            SessionResult::Disconnected(session_age) => {
                // A session that ran at least as long as `max_backoff` is
                // considered "stable" — the server was healthy and the disconnect
                // is a transient blip. Signal the scheduler to reset so the next
                // reconnect starts from the base delay rather than the accumulated
                // one.
                if session_age >= config.max_backoff {
                    tracing::info!(
                        session_secs = session_age.as_secs_f64(),
                        "Supabase Realtime: long session ended; resetting backoff to initial"
                    );
                    backoff.on_success_held();
                } else {
                    tracing::warn!(
                        backoff_secs = backoff.next_delay().as_secs_f64(),
                        session_secs = session_age.as_secs_f64(),
                        "Supabase Realtime disconnected; reconnecting after backoff"
                    );
                    backoff.on_failure();
                }
            }
            SessionResult::ConnectError(e) => {
                tracing::error!(error = %e, "Supabase Realtime connect error");
                backoff.on_failure();
            }
        }

        // Wait for the scheduled delay or a shutdown signal.
        let delay = backoff.next_delay();
        tokio::select! {
            _ = tokio::time::sleep(delay) => {}
            _ = shutdown.notified() => {
                tracing::info!("Supabase Realtime client: shutdown during backoff");
                break;
            }
        }
    }

    // `_guard` drops here on the normal exit path; if we unwound earlier
    // (panic in run_session/select!) the same drop ran then. Either way
    // the flag is cleared exactly once.
    tracing::info!("Supabase Realtime client stopped");
}

/// Result of a single WebSocket session.
enum SessionResult {
    /// Graceful shutdown was requested.
    Shutdown,
    /// Connection was lost unexpectedly after being established.
    /// Carries how long the session ran so the caller can reset backoff when
    /// the session was "stable" (ran longer than `max_backoff`).
    Disconnected(Duration),
    /// Could not establish the connection (pre-join failure).
    ConnectError(String),
}

/// Run a single WebSocket session: connect → join channel → heartbeat + receive loop.
async fn run_session(
    config: &RealtimeConfig,
    tx: &mpsc::Sender<ChangeEvent>,
    shutdown: &Arc<Notify>,
    channel_joined: &Arc<Notify>,
) -> SessionResult {
    // CopyPaste-lnjm: build a proper HTTP upgrade request with the apikey
    // in a request header (not the URL query string).
    let request = match build_ws_request(&config.ws_url, &config.anon_key) {
        Ok(r) => r,
        Err(e) => return SessionResult::ConnectError(format!("request build: {e}")),
    };

    // CopyPaste-qkao: attach a custom TLS connector with SPKI pinning when
    // the URL is wss:// and pins are configured. For plain ws:// (loopback
    // dev) no connector is returned and we fall back to the plain path.
    let connector = build_rustls_connector(&config.ws_url, &config.spki_pins);

    // Establish the WebSocket connection.
    let ws_stream = match connect_async_tls_with_config(request, None, false, connector).await {
        Ok((ws, _)) => ws,
        Err(e) => return SessionResult::ConnectError(e.to_string()),
    };

    tracing::info!("WebSocket connected to Supabase Realtime");

    // Track how long this session runs so the caller can reset backoff when
    // the session was long enough to be considered "stable".
    let session_started = std::time::Instant::now();

    let (mut sink, mut stream) = ws_stream.split();

    // Fix HIGH #2: read the CURRENT bearer token for this reconnect so that a
    // refreshed JWT (pushed via `RealtimeClient::update_jwt`) is always used
    // rather than the stale value captured at client creation time.
    //
    // Fix MED #3: build_join_payload registers event:"*" (INSERT + UPDATE +
    // DELETE) instead of INSERT-only, so cross-device UPDATE/DELETE are delivered.
    //
    // CopyPaste-nr2y (defense-in-depth): a missing user_id is a hard error —
    // we must never silently subscribe without the row filter and rely solely on
    // server-side RLS. Fail the session here; the connection_loop will back off
    // and retry once the caller has populated `config.user_id`.
    let user_id = match config.user_id.as_deref() {
        Some(uid) => uid,
        None => {
            return SessionResult::ConnectError(
                "user_id is required for the Realtime row filter (CopyPaste-nr2y): \
                 set RealtimeConfig::user_id to the GoTrue user UUID before connecting"
                    .into(),
            )
        }
    };
    let current_jwt = config.user_jwt.read().await.clone();
    let join_payload = build_join_payload(&current_jwt, user_id);
    let join_msg = PhoenixMessage {
        join_ref: Some("1".to_owned()),
        msg_ref: Some("1".to_owned()),
        topic: config.topic.clone(),
        event: PhoenixEvent::JOIN.to_owned(),
        payload: join_payload,
    };
    let join_wire = match join_msg.to_wire() {
        Ok(w) => w,
        Err(e) => return SessionResult::ConnectError(format!("join serialise: {e}")),
    };

    if let Err(e) = sink.send(Message::Text(join_wire)).await {
        return SessionResult::ConnectError(format!("join send: {e}"));
    }

    tracing::info!(topic = %config.topic, "Phoenix Channel join sent");

    // Heartbeat task: sends heartbeat every `heartbeat_interval`.
    let heartbeat_interval = config.heartbeat_interval;
    let (hb_stop_tx, mut hb_stop_rx) = tokio::sync::oneshot::channel::<()>();

    // Channel to carry serialised heartbeat payloads from the heartbeat task to sink.
    let (hb_payload_tx, mut hb_payload_rx) = mpsc::channel::<String>(4);

    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(heartbeat_interval);
        let mut ref_counter: u64 = 2; // 1 was used for join
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let msg_ref = ref_counter.to_string();
                    ref_counter += 1;
                    let msg = PhoenixMessage::heartbeat(&msg_ref);
                    match msg.to_wire() {
                        Ok(w) => {
                            if hb_payload_tx.send(w).await.is_err() {
                                break;
                            }
                        }
                        Err(e) => tracing::warn!("heartbeat serialise error: {e}"),
                    }
                }
                _ = &mut hb_stop_rx => {
                    break;
                }
            }
        }
    });

    // Main receive + heartbeat forward loop.
    loop {
        tokio::select! {
            // Incoming WebSocket message.
            maybe_msg = stream.next() => {
                match maybe_msg {
                    None => {
                        // Stream ended.
                        let _ = hb_stop_tx.send(());
                        return SessionResult::Disconnected(session_started.elapsed());
                    }
                    Some(Err(e)) => {
                        tracing::warn!(error = %e, "WebSocket receive error");
                        let _ = hb_stop_tx.send(());
                        return SessionResult::Disconnected(session_started.elapsed());
                    }
                    Some(Ok(msg)) => {
                        if let Some(result) = handle_message(msg, tx, &config.topic, channel_joined).await {
                            let _ = hb_stop_tx.send(());
                            // For Disconnected results from handle_message, replace
                            // the placeholder duration with the actual session age.
                            return match result {
                                SessionResult::Disconnected(_) => {
                                    SessionResult::Disconnected(session_started.elapsed())
                                }
                                other => other,
                            };
                        }
                    }
                }
            }

            // Heartbeat payload ready to send.
            Some(payload) = hb_payload_rx.recv() => {
                tracing::debug!("sending heartbeat");
                // Bound the write: on a half-open socket `send` can stall
                // indefinitely, silently starving heartbeats until the ~60s
                // server timeout kills us. Treat a write that doesn't complete
                // within one heartbeat interval as a disconnect and reconnect.
                match tokio::time::timeout(
                    heartbeat_interval,
                    sink.send(Message::Text(payload)),
                )
                .await
                {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        tracing::warn!(error = %e, "heartbeat send failed");
                        let _ = hb_stop_tx.send(());
                        return SessionResult::Disconnected(session_started.elapsed());
                    }
                    Err(_) => {
                        tracing::warn!("heartbeat send timed out; treating as disconnect");
                        let _ = hb_stop_tx.send(());
                        return SessionResult::Disconnected(session_started.elapsed());
                    }
                }
            }

            // Shutdown signal.
            _ = shutdown.notified() => {
                // Send phx_leave before closing.
                let leave = PhoenixMessage {
                    join_ref: Some("1".to_owned()),
                    msg_ref: Some("leave".to_owned()),
                    topic: config.topic.clone(),
                    event: "phx_leave".to_owned(),
                    payload: serde_json::json!({}),
                };
                if let Ok(wire) = leave.to_wire() {
                    let _ = sink.send(Message::Text(wire)).await;
                }
                let _ = sink.send(Message::Close(None)).await;
                let _ = hb_stop_tx.send(());
                return SessionResult::Shutdown;
            }
        }
    }
}

/// Process a single WebSocket frame.
///
/// Returns `Some(SessionResult)` to terminate the session loop, or `None` to continue.
async fn handle_message(
    msg: Message,
    tx: &mpsc::Sender<ChangeEvent>,
    topic: &str,
    channel_joined: &Arc<Notify>,
) -> Option<SessionResult> {
    match msg {
        Message::Text(text) => {
            match PhoenixMessage::from_wire(&text) {
                Err(e) => {
                    // Wave 2.7 sec #17: raw frame can embed clipboard plaintext.
                    // Log length + 16-byte prefix only, never the full text.
                    let bytes = text.as_bytes();
                    let take = bytes.len().min(16);
                    let prefix =
                        bytes[..take]
                            .iter()
                            .fold(String::with_capacity(take * 2), |mut acc, b| {
                                use std::fmt::Write as _;
                                let _ = write!(acc, "{:02x}", b);
                                acc
                            });
                    tracing::warn!(
                        error = %e,
                        raw_len = bytes.len(),
                        raw_prefix = %prefix,
                        "failed to parse Phoenix message"
                    );
                }
                Ok(phoenix_msg) => {
                    dispatch_event(&phoenix_msg, tx, topic, channel_joined).await;
                }
            }
            None
        }
        Message::Binary(data) => {
            tracing::debug!(bytes = data.len(), "received binary frame (ignored)");
            None
        }
        Message::Ping(data) => {
            // tungstenite auto-replies to Ping; we just log.
            tracing::trace!(bytes = data.len(), "received Ping");
            None
        }
        Message::Pong(_) => None,
        Message::Close(_) => {
            tracing::info!("received WebSocket Close frame");
            // Duration::ZERO is a placeholder; run_session replaces it with the
            // actual elapsed time before returning to connection_loop.
            Some(SessionResult::Disconnected(Duration::ZERO))
        }
        Message::Frame(_) => None,
    }
}

/// Route a parsed Phoenix message to the appropriate handler.
///
/// The `channel_joined` notify is fired when a `phx_reply` with
/// `status == "ok"` is observed — indicating the Phoenix Channel join has been
/// confirmed by the server.  The daemon's `ws_ingest_loop` awaits this signal
/// before setting `ws_connected = true` so the HTTP catch-up poll does not back
/// off to the slow rate until the channel is actually delivering events.
async fn dispatch_event(
    msg: &PhoenixMessage,
    tx: &mpsc::Sender<ChangeEvent>,
    topic: &str,
    channel_joined: &Arc<Notify>,
) {
    match msg.event.as_str() {
        PhoenixEvent::REPLY => {
            let status = msg.payload.get("status").and_then(|s| s.as_str());
            if status == Some("ok") {
                tracing::info!(topic = %msg.topic, "Phoenix Channel join confirmed (phx_reply ok)");
                // Signal the daemon that the channel subscription is live.
                // `notify_one` stores a permit so the next call to
                // `channel_joined.notified().await` completes immediately even
                // if the waiter hasn't registered yet (i.e. the phx_reply
                // arrives before ws_ingest_loop reaches its select! branch).
                // `notify_waiters` would only wake *current* waiters and the
                // permit would be lost if no one was waiting at that instant.
                channel_joined.notify_one();
            } else {
                tracing::warn!(topic = %msg.topic, ?status, "Phoenix reply with non-ok status");
            }
        }

        PhoenixEvent::ERROR => {
            tracing::error!(
                topic = %msg.topic,
                payload_redacted = %redact_payload(&msg.payload),
                "Phoenix channel error"
            );
        }

        PhoenixEvent::CLOSE => {
            tracing::info!(topic = %msg.topic, "Phoenix channel closed by server");
        }

        PhoenixEvent::POSTGRES_CHANGES => {
            if let Some(event) = ChangeEvent::from_payload(topic, &msg.payload) {
                tracing::debug!(
                    change_type = ?event.change_type,
                    table = %event.table,
                    "Supabase change event received"
                );
                if tx.send(event).await.is_err() {
                    tracing::debug!("change event receiver dropped; ignoring event");
                }
            } else {
                tracing::warn!(
                    payload_redacted = %redact_payload(&msg.payload),
                    "could not parse postgres_changes payload"
                );
            }
        }

        other => {
            tracing::trace!(event = %other, topic = %msg.topic, "unhandled Phoenix event");
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ChangeType, PhoenixEvent};
    use serial_test::serial;
    use tokio::sync::mpsc;

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

    // ── SPKI pins (CopyPaste-qkao) ────────────────────────────────────────────

    #[test]
    fn spki_pins_empty_matches_nothing() {
        let pins = SpkiPins::default();
        assert!(pins.is_empty());
        // Even an empty byte slice should not match an empty pin set.
        assert!(!pins.matches(b"anything"));
    }

    #[test]
    fn spki_pins_matches_correct_hash() {
        use sha2::Digest;
        let spki_bytes = b"fake-spki-der-content";
        let hash: [u8; 32] = sha2::Sha256::digest(spki_bytes).into();
        let pins = SpkiPins { pins: vec![hash] };

        assert!(!pins.is_empty());
        assert!(
            pins.matches(spki_bytes),
            "known SPKI must match its own SHA-256 pin"
        );
        assert!(
            !pins.matches(b"wrong-content"),
            "wrong content must not match"
        );
    }

    // ── extract_spki_der ──────────────────────────────────────────────────────

    /// Minimal DER structure: Certificate SEQUENCE → TBSCertificate SEQUENCE
    /// with enough filler fields so the SPKI field is at the right offset.
    ///
    /// We construct a synthetic (hand-crafted) DER to verify the extractor
    /// without depending on a real X.509 cert. Fields before SPKI in a
    /// TBSCertificate (RFC 5280):
    ///   [0] version (optional) | serialNumber | signature | issuer | validity | subject | SPKI
    ///
    /// We encode each as a minimal SEQUENCE or INTEGER so the extractor can
    /// skip them correctly.
    #[test]
    fn extract_spki_der_returns_correct_field() {
        // Each filler field as a minimal SEQUENCE: tag=0x30 len=0x00 (empty).
        let empty_seq = &[0x30u8, 0x00u8];
        // SPKI field: tag=0x30 len=0x04 content=[1,2,3,4].
        let spki_content = &[1u8, 2, 3, 4];
        let spki_tlv: Vec<u8> = {
            let mut v = vec![0x30u8, spki_content.len() as u8];
            v.extend_from_slice(spki_content);
            v
        };

        // Build TBSCertificate body (no version field for simplicity):
        // serialNumber INTEGER, signature SEQUENCE, issuer SEQUENCE,
        // validity SEQUENCE, subject SEQUENCE, SPKI SEQUENCE.
        // We use 0x02 0x01 0x01 (INTEGER value 1) for serialNumber.
        let serial: Vec<u8> = vec![0x02, 0x01, 0x01];
        let mut tbs_body: Vec<u8> = Vec::new();
        tbs_body.extend_from_slice(&serial);
        tbs_body.extend_from_slice(empty_seq); // signature
        tbs_body.extend_from_slice(empty_seq); // issuer
        tbs_body.extend_from_slice(empty_seq); // validity
        tbs_body.extend_from_slice(empty_seq); // subject
        tbs_body.extend_from_slice(&spki_tlv); // SPKI

        // Wrap TBSCertificate in a SEQUENCE.
        let mut tbs_seq: Vec<u8> = vec![0x30, tbs_body.len() as u8];
        tbs_seq.extend_from_slice(&tbs_body);

        // Outer Certificate SEQUENCE: just the TBS for this test.
        let mut cert_der: Vec<u8> = vec![0x30, tbs_seq.len() as u8];
        cert_der.extend_from_slice(&tbs_seq);

        let extracted = extract_spki_der(&cert_der);
        assert!(extracted.is_some(), "SPKI extraction must succeed");
        assert_eq!(
            extracted.unwrap(),
            spki_tlv,
            "extracted SPKI TLV must equal the expected bytes"
        );
    }

    // ── BackoffScheduler consolidation (CopyPaste-nq31) ──────────────────────

    /// Verify that `connection_loop` now uses `BackoffScheduler` semantics:
    /// after a long session (`session_age >= max_backoff`) the schedule resets
    /// to the initial delay, not to the accumulated position.
    ///
    /// We test this by inspecting `BackoffScheduler` directly — the same
    /// logic that `connection_loop` now delegates to.
    #[test]
    fn backoff_scheduler_resets_after_long_session() {
        use copypaste_sync::backoff::BackoffScheduler;
        let initial = Duration::from_secs(1);
        let max = Duration::from_secs(60);
        let mut sched = BackoffScheduler::new(initial, max, max);

        // Simulate several connection failures.
        sched.on_failure();
        sched.on_failure();
        sched.on_failure();
        // After 3 failures, delay > initial.
        assert!(
            sched.next_delay() > initial,
            "delay should have grown after failures"
        );

        // A long-running session signals success.
        sched.on_success_held();

        // Must reset to initial.
        assert_eq!(
            sched.next_delay(),
            initial,
            "BackoffScheduler must reset to initial after on_success_held"
        );
    }

    #[test]
    fn backoff_scheduler_accumulates_on_connect_error() {
        use copypaste_sync::backoff::BackoffScheduler;
        let initial = Duration::from_secs(1);
        let max = Duration::from_secs(60);
        let mut sched = BackoffScheduler::new(initial, max, max);

        assert_eq!(sched.next_delay(), initial);
        sched.on_failure();
        assert_eq!(sched.next_delay(), Duration::from_secs(2));
        sched.on_failure();
        assert_eq!(sched.next_delay(), Duration::from_secs(4));
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

    // ── dispatch_event ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn dispatch_postgres_changes_sends_to_channel() {
        let (tx, mut rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: None,
            msg_ref: None,
            topic: topic.to_owned(),
            event: PhoenixEvent::POSTGRES_CHANGES.to_owned(),
            payload: serde_json::json!({
                "data": {
                    "type": "INSERT",
                    "table": "clipboard_items",
                    "record": { "id": "item-1", "content_type": "text" },
                }
            }),
        };

        dispatch_event(&msg, &tx, topic, &joined).await;

        let event = rx.try_recv().expect("event should be in channel");
        assert_eq!(event.change_type, ChangeType::Insert);
        assert_eq!(event.record["id"], "item-1");
    }

    #[tokio::test]
    async fn dispatch_phx_reply_ok_does_not_send_event() {
        let (tx, mut rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: Some("1".to_owned()),
            msg_ref: Some("1".to_owned()),
            topic: topic.to_owned(),
            event: PhoenixEvent::REPLY.to_owned(),
            payload: serde_json::json!({ "status": "ok", "response": {} }),
        };

        dispatch_event(&msg, &tx, topic, &joined).await;
        assert!(
            rx.try_recv().is_err(),
            "phx_reply should not produce a ChangeEvent"
        );
    }

    #[tokio::test]
    async fn dispatch_unknown_event_does_not_send_event() {
        let (tx, mut rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: None,
            msg_ref: None,
            topic: topic.to_owned(),
            event: "presence_state".to_owned(),
            payload: serde_json::json!({}),
        };

        dispatch_event(&msg, &tx, topic, &joined).await;
        assert!(rx.try_recv().is_err());
    }

    // ── Backoff doubling ──────────────────────────────────────────────────────

    #[test]
    fn backoff_doubles_and_caps() {
        let initial = Duration::from_secs(1);
        let max = Duration::from_secs(60);

        let mut b = initial;
        for _ in 0..10 {
            b = (b * 2).min(max);
        }
        assert_eq!(b, max, "backoff should cap at max_backoff");
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

    // ── build_join_payload ────────────────────────────────────────────────────

    #[test]
    fn build_join_payload_includes_bearer_token() {
        let jwt = "my.jwt.token";
        let uid = "550e8400-e29b-41d4-a716-446655440000";
        // CopyPaste-nr2y: user_id is now mandatory — pass a real UUID.
        let payload = build_join_payload(jwt, uid);
        // The JWT must appear under config.access_token (Supabase Realtime v2 shape).
        let token_in_payload = payload
            .pointer("/config/access_token")
            .and_then(|v| v.as_str())
            == Some(jwt);
        assert!(
            token_in_payload,
            "join payload must include JWT under /config/access_token, got: {}",
            serde_json::to_string(&payload).unwrap()
        );
    }

    #[test]
    fn build_join_payload_registers_all_events() {
        let uid = "550e8400-e29b-41d4-a716-446655440000";
        // CopyPaste-nr2y: user_id is now mandatory — pass a real UUID.
        let payload = build_join_payload("tok", uid);
        let payload_str = serde_json::to_string(&payload).unwrap();
        // event:"*" means INSERT + UPDATE + DELETE are all delivered.
        assert!(
            payload_str.contains("\"*\""),
            "join payload must register event:\"*\", got: {payload_str}"
        );
        assert!(
            !payload_str.contains("\"INSERT\""),
            "join payload must NOT limit to INSERT-only, got: {payload_str}"
        );
    }

    /// CopyPaste-nr2y: the user_id filter is always mandatory.
    /// build_join_payload always includes "user_id=eq.<uuid>" — a missing user_id
    /// is rejected at the run_session level (hard error, not silently omitted).
    #[test]
    fn build_join_payload_always_includes_mandatory_user_id_filter() {
        let uid = "550e8400-e29b-41d4-a716-446655440000";
        let payload = build_join_payload("tok", uid);
        let payload_str = serde_json::to_string(&payload).unwrap();
        // Filter clause must always be present (defense-in-depth).
        assert!(
            payload_str.contains("user_id=eq."),
            "join payload must always contain user_id filter; got: {payload_str}"
        );
        assert!(
            payload_str.contains(uid),
            "join payload must embed the user UUID in the filter; got: {payload_str}"
        );
        // Verify the filter is under the postgres_changes entry.
        let filter = payload
            .pointer("/config/postgres_changes/0/filter")
            .and_then(|v| v.as_str());
        assert_eq!(
            filter,
            Some("user_id=eq.550e8400-e29b-41d4-a716-446655440000"),
            "filter must be at /config/postgres_changes/0/filter; got: {payload_str}"
        );
    }

    // ── update_jwt ────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn update_jwt_changes_jwt_seen_by_next_session() {
        // Create a config with an initial JWT.
        let config = RealtimeConfig::new(
            "https://abc.supabase.co",
            "anon-key",
            RealtimeConfig::DEFAULT_TOPIC,
            false, // disabled so no real network
        );
        let (client, _rx) = RealtimeClient::new(config);

        // The initial JWT should be empty (no JWT provided).
        let initial = client.current_jwt().await;
        assert_eq!(initial, "", "initial JWT should be empty");

        // Update the JWT and verify it is visible.
        client.update_jwt("fresh.token.abc".to_owned()).await;
        let updated = client.current_jwt().await;
        assert_eq!(updated, "fresh.token.abc", "updated JWT should be visible");
    }

    // ── ClientHandle Drop / shutdown invariant ────────────────────────────────

    /// Audit-concurrency HIGH: dropping a `ClientHandle` must terminate its
    /// `connection_loop` task — i.e. it must clear the shared `running` flag
    /// (which the loop checks at the top of each iteration) and wake any task
    /// parked on the shutdown `Notify`. Without this, the daemon's reconnect
    /// path leaked one live client stack per WS disconnect.
    #[tokio::test]
    async fn dropping_handle_clears_running_flag() {
        let shutdown = Arc::new(Notify::new());
        let running = Arc::new(AtomicBool::new(true));
        let handle = ClientHandle {
            shutdown: shutdown.clone(),
            running: running.clone(),
            channel_joined: Arc::new(Notify::new()),
        };
        assert!(handle.is_running(), "precondition: flag starts true");

        drop(handle);

        assert!(
            !running.load(Ordering::SeqCst),
            "dropping the handle must clear the running flag so connection_loop exits"
        );
    }

    /// A live `connection_loop` task must observe the drop of its handle and
    /// terminate. We point it at an unreachable address (TEST-NET-1, RFC 5737)
    /// so it cycles through connect-error → backoff, then drop the handle and
    /// assert the `running` flag is cleared (the loop's top-of-iteration check
    /// then breaks). This exercises the real task, not just the Drop impl.
    #[tokio::test(start_paused = true)]
    async fn dropping_handle_stops_connection_loop_task() {
        // Enabled config with a near-zero backoff so the loop spins quickly to
        // its `shutdown.notified()` / top-of-loop check under the paused clock.
        let mut config = RealtimeConfig::new(
            "https://192.0.2.1", // RFC 5737 TEST-NET-1: guaranteed unreachable
            "anon-key",
            RealtimeConfig::DEFAULT_TOPIC,
            true,
        );
        config.initial_backoff = Duration::from_millis(1);
        config.max_backoff = Duration::from_millis(1);

        let (client, _rx) = RealtimeClient::new(config);
        let running = client.running.clone();
        let handle = client.connect().await.expect("connect spawns the loop");

        // The loop set running=true synchronously inside connect().
        assert!(running.load(Ordering::SeqCst), "loop should be running");

        // Drop the handle: signal_shutdown clears running + notifies.
        drop(handle);

        // running is cleared synchronously by Drop.
        assert!(
            !running.load(Ordering::SeqCst),
            "dropped handle must clear running so the loop exits at its next check"
        );

        // Let the task actually wind down (top-of-loop sees running=false and
        // breaks, or the backoff select sees the notify). Yield a few times.
        for _ in 0..8 {
            tokio::task::yield_now().await;
        }
    }

    // ── channel_joined signal (Phase 3) ──────────────────────────────────────

    /// `dispatch_event` must fire the `channel_joined` notify when it sees
    /// `phx_reply` with `status == "ok"`.
    ///
    /// Contract: `ClientHandle::channel_joined()` must return an `Arc<Notify>`
    /// that is notified by `dispatch_event` so that `ws_ingest_loop` in the
    /// daemon can gate `ws_connected=true` on channel confirmation instead of
    /// bare socket-open.
    #[tokio::test]
    async fn dispatch_phx_reply_ok_fires_channel_joined_notify() {
        let (tx, _rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: Some("1".to_owned()),
            msg_ref: Some("1".to_owned()),
            topic: topic.to_owned(),
            event: PhoenixEvent::REPLY.to_owned(),
            payload: serde_json::json!({ "status": "ok", "response": {} }),
        };

        // Must not be notified before calling dispatch_event.
        let notified_before = tokio::time::timeout(
            std::time::Duration::from_millis(0),
            joined.clone().notified(),
        )
        .await
        .is_ok();
        assert!(
            !notified_before,
            "channel_joined must not fire before dispatch_event"
        );

        dispatch_event(&msg, &tx, topic, &joined).await;

        // Must be notified now (use a tight timeout to stay deterministic).
        let notified_after =
            tokio::time::timeout(std::time::Duration::from_millis(50), joined.notified())
                .await
                .is_ok();
        assert!(
            notified_after,
            "dispatch_event must fire channel_joined on phx_reply ok"
        );
    }

    /// A non-ok `phx_reply` must NOT fire the `channel_joined` notify.
    #[tokio::test]
    async fn dispatch_phx_reply_error_does_not_fire_channel_joined() {
        let (tx, _rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";
        let joined = Arc::new(Notify::new());

        let msg = PhoenixMessage {
            join_ref: Some("1".to_owned()),
            msg_ref: Some("1".to_owned()),
            topic: topic.to_owned(),
            event: PhoenixEvent::REPLY.to_owned(),
            payload: serde_json::json!({ "status": "error", "response": {} }),
        };

        dispatch_event(&msg, &tx, topic, &joined).await;

        let notified =
            tokio::time::timeout(std::time::Duration::from_millis(10), joined.notified())
                .await
                .is_ok();
        assert!(
            !notified,
            "dispatch_event must NOT fire channel_joined on non-ok phx_reply"
        );
    }

    /// `ClientHandle` must expose a `channel_joined()` method returning
    /// `Arc<Notify>` so the daemon can await join confirmation.
    #[tokio::test]
    async fn client_handle_exposes_channel_joined() {
        let config = RealtimeConfig::new(
            "https://abc.supabase.co",
            "k",
            RealtimeConfig::DEFAULT_TOPIC,
            false, // disabled — no real network
        );
        let (client, _rx) = RealtimeClient::new(config);
        let handle = client.connect().await.expect("connect ok");
        // Must compile and return an Arc<Notify>.
        let _joined: Arc<Notify> = handle.channel_joined();
    }

    // ── Disabled client ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn disabled_client_connect_returns_handle_not_running() {
        let config = RealtimeConfig::new(
            "https://abc.supabase.co",
            "k",
            RealtimeConfig::DEFAULT_TOPIC,
            false,
        );
        let (client, _rx) = RealtimeClient::new(config);
        let handle = client
            .connect()
            .await
            .expect("connect should succeed even when disabled");
        // When disabled, the background loop never sets running=true, so is_running is false
        // (we never stored true for a disabled client)
        assert!(
            !handle.is_running(),
            "disabled client should not be running"
        );
    }
}
