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

use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::{Notify, RwLock};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{ChangeEvent, PhoenixEvent, PhoenixMessage};
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
    /// Full WebSocket URL including API key query param.
    /// Format: `wss://{project}.supabase.co/realtime/v1/websocket?apikey={key}&vsn=1.0.0`
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

        Ok(Self::new(supabase_url, anon_key, topic, enabled))
    }

    /// Construct config programmatically (no user JWT — anon scope).
    pub fn new(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        topic: impl Into<String>,
        enabled: bool,
    ) -> Self {
        Self::with_jwt(supabase_url, anon_key, topic, None, enabled)
    }

    /// Construct config with an explicit user JWT for RLS-aware subscriptions.
    ///
    /// The `user_jwt` is sent as `params.user_token` in the `phx_join` payload
    /// so Supabase Realtime applies the authenticated user's RLS policies when
    /// filtering `postgres_changes` events.  Pass `None` to use anon scope.
    pub fn with_jwt(
        supabase_url: impl Into<String>,
        anon_key: impl Into<String>,
        topic: impl Into<String>,
        user_jwt: Option<String>,
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
/// The `ws_url` embeds `?apikey=<anon-key>&vsn=...`; logging the raw URL would
/// expose the anon key in the daemon log file.  This function returns only the
/// scheme + authority + path, dropping everything from `?` onward.
///
/// # Examples
/// ```
/// # use copypaste_supabase::realtime::scrub_ws_url;
/// let u = "wss://abc.supabase.co/realtime/v1/websocket?apikey=secret&vsn=1.0.0";
/// assert_eq!(scrub_ws_url(u), "wss://abc.supabase.co/realtime/v1/websocket");
/// ```
pub fn scrub_ws_url(url: &str) -> &str {
    // Everything before the first `?` is the safe portion.
    match url.find('?') {
        Some(pos) => &url[..pos],
        None => url,
    }
}

/// Build the Phoenix Channel join payload for a Supabase Realtime subscription.
///
/// # Bearer token
/// The `user_jwt` is placed under `config.access_token` so Supabase Realtime
/// authenticates the channel with the caller's RLS identity.  An empty string
/// disables per-user RLS (anonymous / anon-key-only access).
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
///       { "event": "*", "schema": "public", "table": "clipboard_items" }
///     ]
///   }
/// }
/// ```
pub(crate) fn build_join_payload(user_jwt: &str) -> serde_json::Value {
    serde_json::json!({
        "config": {
            "access_token": user_jwt,
            "postgres_changes": [
                {
                    "event": "*",
                    "schema": "public",
                    "table": "clipboard_items"
                }
            ]
        }
    })
}

/// Convert a Supabase REST URL to the Realtime WebSocket URL.
fn build_ws_url(base_url: &str, api_key: &str) -> String {
    // Replace http/https scheme with ws/wss
    let ws_base = if base_url.starts_with("https://") {
        base_url.replacen("https://", "wss://", 1)
    } else if base_url.starts_with("http://") {
        base_url.replacen("http://", "ws://", 1)
    } else {
        format!("wss://{}", base_url)
    };

    // Strip trailing slash before appending path
    let ws_base = ws_base.trim_end_matches('/');

    format!(
        "{}/realtime/v1/websocket?apikey={}&vsn=1.0.0",
        ws_base, api_key
    )
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
}

impl RealtimeClient {
    /// Create a new client.  Returns the client and the channel receiver for change events.
    pub fn new(config: RealtimeConfig) -> (Self, mpsc::Receiver<ChangeEvent>) {
        let (tx, rx) = mpsc::channel(config.channel_capacity);
        let shutdown = Arc::new(Notify::new());
        let running = Arc::new(AtomicBool::new(false));
        (
            Self {
                config,
                tx,
                shutdown,
                running,
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
            });
        }

        let handle = ClientHandle {
            shutdown: self.shutdown.clone(),
            running: self.running.clone(),
        };

        self.running.store(true, Ordering::SeqCst);

        // Spawn the reconnect loop
        tokio::spawn(connection_loop(
            self.config,
            self.tx,
            self.shutdown,
            self.running,
        ));

        Ok(handle)
    }
}

// ── ClientHandle ──────────────────────────────────────────────────────────────

/// Handle returned from [`RealtimeClient::connect`].  Use to check status or shut down.
pub struct ClientHandle {
    shutdown: Arc<Notify>,
    running: Arc<AtomicBool>,
}

impl ClientHandle {
    /// Signal the client to shut down and wait for acknowledgement.
    pub async fn shutdown(self) {
        self.shutdown.notify_waiters();
        // Brief yield to allow the background task to notice the shutdown signal.
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    /// Returns `true` if the background worker is still active.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
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
async fn connection_loop(
    config: RealtimeConfig,
    tx: mpsc::Sender<ChangeEvent>,
    shutdown: Arc<Notify>,
    running: Arc<AtomicBool>,
) {
    // Audit-concurrency HIGH #4: clear `running` on ALL exit paths (return,
    // panic, abort) via a Drop guard, not just the bottom of the function.
    let _guard = RunningGuard(running.clone());

    let mut backoff = config.initial_backoff;

    loop {
        // Check shutdown before attempting to connect.
        if !running.load(Ordering::SeqCst) {
            break;
        }

        // Fix HIGH #1: strip ?apikey=... so the anon key never appears in logs.
        tracing::info!(url = %scrub_ws_url(&config.ws_url), "Connecting to Supabase Realtime");

        match run_session(&config, &tx, &shutdown).await {
            SessionResult::Shutdown => {
                tracing::info!("Supabase Realtime client: shutdown requested");
                break;
            }
            SessionResult::Disconnected(session_age) => {
                // Reset backoff to initial when the session was "stable" — i.e.
                // it ran longer than `max_backoff`. A long session implies the
                // server is healthy and the disconnect is a transient blip, not
                // a systematic connection failure. Only applying backoff for
                // pre-message disconnects (ConnectError) or very short sessions
                // avoids penalising recoveries from brief network interruptions.
                if session_age >= config.max_backoff {
                    tracing::info!(
                        session_secs = session_age.as_secs_f64(),
                        "Supabase Realtime: long session ended; resetting backoff to initial"
                    );
                    backoff = config.initial_backoff;
                } else {
                    tracing::warn!(
                        backoff_secs = backoff.as_secs_f64(),
                        session_secs = session_age.as_secs_f64(),
                        "Supabase Realtime disconnected; reconnecting after backoff"
                    );
                }
            }
            SessionResult::ConnectError(e) => {
                tracing::error!(error = %e, "Supabase Realtime connect error");
            }
        }

        // Wait for backoff or shutdown signal.
        tokio::select! {
            _ = tokio::time::sleep(backoff) => {}
            _ = shutdown.notified() => {
                tracing::info!("Supabase Realtime client: shutdown during backoff");
                break;
            }
        }

        // Exponential backoff with cap (only increments for short/failed sessions;
        // long sessions already reset backoff above).
        backoff = (backoff * 2).min(config.max_backoff);
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
) -> SessionResult {
    // Parse the URL.
    let url = match config
        .ws_url
        .parse::<tokio_tungstenite::tungstenite::http::Uri>()
    {
        Ok(u) => u,
        Err(e) => return SessionResult::ConnectError(format!("bad URL: {e}")),
    };

    // Establish the WebSocket connection.
    let ws_stream = match connect_async(url).await {
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
    let current_jwt = config.user_jwt.read().await.clone();
    let join_payload = build_join_payload(&current_jwt);
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
                        if let Some(result) = handle_message(msg, tx, &config.topic).await {
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
                    dispatch_event(&phoenix_msg, tx, topic).await;
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
async fn dispatch_event(msg: &PhoenixMessage, tx: &mpsc::Sender<ChangeEvent>, topic: &str) {
    match msg.event.as_str() {
        PhoenixEvent::REPLY => {
            let status = msg.payload.get("status").and_then(|s| s.as_str());
            if status == Some("ok") {
                tracing::info!(topic = %msg.topic, "Phoenix Channel join confirmed (phx_reply ok)");
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

    // ── build_ws_url ──────────────────────────────────────────────────────────

    #[test]
    fn build_ws_url_converts_https() {
        let url = build_ws_url("https://abc.supabase.co", "mykey");
        assert_eq!(
            url,
            "wss://abc.supabase.co/realtime/v1/websocket?apikey=mykey&vsn=1.0.0"
        );
    }

    #[test]
    fn build_ws_url_converts_http() {
        let url = build_ws_url("http://localhost:4000", "k");
        assert_eq!(
            url,
            "ws://localhost:4000/realtime/v1/websocket?apikey=k&vsn=1.0.0"
        );
    }

    #[test]
    fn build_ws_url_handles_trailing_slash() {
        let url = build_ws_url("https://abc.supabase.co/", "k");
        assert_eq!(
            url,
            "wss://abc.supabase.co/realtime/v1/websocket?apikey=k&vsn=1.0.0"
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
        std::env::set_var("SUPABASE_URL", "https://test.supabase.co");
        std::env::set_var("SUPABASE_ANON_KEY", "k");
        for v in ["1", "true", "TRUE", "yes"] {
            std::env::set_var("SUPABASE_REALTIME_DISABLED", v);
            let cfg = RealtimeConfig::from_env().expect("config builds");
            assert!(
                !cfg.enabled,
                "SUPABASE_REALTIME_DISABLED={v} must DISABLE realtime"
            );
        }
        // Unset / falsey → enabled.
        std::env::remove_var("SUPABASE_REALTIME_DISABLED");
        assert!(
            RealtimeConfig::from_env().expect("config builds").enabled,
            "unset disable flag should leave realtime enabled"
        );
        std::env::set_var("SUPABASE_REALTIME_DISABLED", "false");
        assert!(
            RealtimeConfig::from_env().expect("config builds").enabled,
            "SUPABASE_REALTIME_DISABLED=false should leave realtime enabled"
        );
        std::env::remove_var("SUPABASE_REALTIME_DISABLED");
        std::env::remove_var("SUPABASE_URL");
        std::env::remove_var("SUPABASE_ANON_KEY");
    }

    // ── RealtimeConfig ────────────────────────────────────────────────────────

    #[test]
    #[serial]
    fn config_from_env_requires_supabase_url() {
        // Remove env vars to test missing SUPABASE_URL
        std::env::remove_var("SUPABASE_URL");
        std::env::remove_var("SUPABASE_ANON_KEY");
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
        std::env::set_var("SUPABASE_URL", "https://test.supabase.co");
        std::env::remove_var("SUPABASE_ANON_KEY");
        let result = RealtimeConfig::from_env();
        std::env::remove_var("SUPABASE_URL");
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

        dispatch_event(&msg, &tx, topic).await;

        let event = rx.try_recv().expect("event should be in channel");
        assert_eq!(event.change_type, ChangeType::Insert);
        assert_eq!(event.record["id"], "item-1");
    }

    #[tokio::test]
    async fn dispatch_phx_reply_ok_does_not_send_event() {
        let (tx, mut rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";

        let msg = PhoenixMessage {
            join_ref: Some("1".to_owned()),
            msg_ref: Some("1".to_owned()),
            topic: topic.to_owned(),
            event: PhoenixEvent::REPLY.to_owned(),
            payload: serde_json::json!({ "status": "ok", "response": {} }),
        };

        dispatch_event(&msg, &tx, topic).await;
        assert!(
            rx.try_recv().is_err(),
            "phx_reply should not produce a ChangeEvent"
        );
    }

    #[tokio::test]
    async fn dispatch_unknown_event_does_not_send_event() {
        let (tx, mut rx) = mpsc::channel(8);
        let topic = "realtime:clipboard_items";

        let msg = PhoenixMessage {
            join_ref: None,
            msg_ref: None,
            topic: topic.to_owned(),
            event: "presence_state".to_owned(),
            payload: serde_json::json!({}),
        };

        dispatch_event(&msg, &tx, topic).await;
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
        let url = "wss://abc.supabase.co/realtime/v1/websocket?apikey=secret-key&vsn=1.0.0";
        let scrubbed = scrub_ws_url(url);
        assert!(
            !scrubbed.contains("apikey"),
            "scrubbed URL must not contain 'apikey', got: {scrubbed}"
        );
        assert!(
            !scrubbed.contains("secret-key"),
            "scrubbed URL must not contain the key value, got: {scrubbed}"
        );
        assert!(
            scrubbed.contains("wss://abc.supabase.co"),
            "scrubbed URL must still contain the host, got: {scrubbed}"
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
        let payload = build_join_payload(jwt);
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
        let payload = build_join_payload("tok");
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
