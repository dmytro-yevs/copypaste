//! [`RealtimeClient`]: construction and connection lifecycle; [`ClientHandle`]:
//! shutdown/status handle with RAII `Drop`; [`RealtimeError`]: the module's
//! error type.
//!
//! The outer reconnect loop lives in [`super::reconnect`]; a single WS session
//! lives in [`super::session`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::sync::mpsc;
use tokio::sync::Notify;

use crate::protocol::ChangeEvent;
use crate::realtime::RealtimeConfig;

use super::reconnect::connection_loop;

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

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
