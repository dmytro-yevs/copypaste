//! WebSocket client, connection lifecycle, and Phoenix Channel event handling.
//!
//! Handles:
//! - [`RealtimeClient`]: top-level client; spawns the connection loop
//! - [`ClientHandle`]: shutdown + status handle with RAII Drop guard
//! - [`RunningGuard`]: RAII flag-clear on task exit
//! - [`connection_loop`]: outer reconnect + backoff loop
//! - [`run_session`]: single WS session (connect → join → heartbeat + recv)
//! - [`handle_message`]: per-frame dispatcher
//! - [`dispatch_event`]: Phoenix event router (REPLY/ERROR/CLOSE/POSTGRES_CHANGES)
//! - [`build_join_payload`]: `phx_join` payload (event:"*", mandatory user_id filter)

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::Notify;
use tokio_tungstenite::connect_async_tls_with_config;
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{ChangeEvent, PhoenixEvent, PhoenixMessage};
use crate::realtime::{
    build_rustls_connector, build_ws_request, redact_payload, scrub_ws_url, RealtimeConfig,
};
use copypaste_sync::backoff::BackoffScheduler;
use futures_util::{SinkExt, StreamExt};

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
    pub(crate) config: RealtimeConfig,
    tx: mpsc::Sender<ChangeEvent>,
    pub(crate) shutdown: Arc<Notify>,
    pub(crate) running: Arc<AtomicBool>,
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
    pub(crate) shutdown: Arc<Notify>,
    pub(crate) running: Arc<AtomicBool>,
    /// Shared with the background `connection_loop` task.  Notified once when
    /// the Phoenix Channel join is confirmed (`phx_reply` `status == "ok"`).
    /// See [`channel_joined`](Self::channel_joined).
    pub(crate) channel_joined: Arc<Notify>,
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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{ChangeType, PhoenixEvent};
    use crate::realtime::RealtimeConfig;
    use tokio::sync::mpsc;

    // ── BackoffScheduler consolidation (CopyPaste-nq31) ──────────────────────

    /// Verify that `connection_loop` now uses `BackoffScheduler` semantics:
    /// after a long session (`session_age >= max_backoff`) the schedule resets
    /// to the initial delay, not to the accumulated position.
    ///
    /// We test this by inspecting `BackoffScheduler` directly — the same
    /// logic that `connection_loop` now delegates to.
    #[test]
    fn backoff_scheduler_resets_after_long_session() {
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
        let initial = Duration::from_secs(1);
        let max = Duration::from_secs(60);
        let mut sched = BackoffScheduler::new(initial, max, max);

        assert_eq!(sched.next_delay(), initial);
        sched.on_failure();
        assert_eq!(sched.next_delay(), Duration::from_secs(2));
        sched.on_failure();
        assert_eq!(sched.next_delay(), Duration::from_secs(4));
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
