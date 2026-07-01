//! Outer reconnection loop: `RunningGuard` RAII cleanup, `connection_loop`
//! exponential-backoff orchestration, and the per-session `SessionResult`.
//!
//! A single WebSocket session is run by [`super::session::run_session`].

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::sync::Notify;

use crate::protocol::ChangeEvent;
use crate::realtime::{scrub_ws_url, RealtimeConfig};
use copypaste_ipc::backoff::BackoffScheduler;

use super::session::run_session;

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
pub(super) async fn connection_loop(
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
pub(super) enum SessionResult {
    /// Graceful shutdown was requested.
    Shutdown,
    /// Connection was lost unexpectedly after being established.
    /// Carries how long the session ran so the caller can reset backoff when
    /// the session was "stable" (ran longer than `max_backoff`).
    Disconnected(Duration),
    /// Could not establish the connection (pre-join failure).
    ConnectError(String),
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── BackoffScheduler consolidation (CopyPaste-nq31) ──────────────────────
    //
    // CopyPaste-vp63.26: the stale `backoff_doubles_and_caps` test (which
    // hand-rolled `(b*2).min(max)` against logic prod no longer implements
    // inline) has been deleted. The two tests below exercise the real
    // `BackoffScheduler` that `connection_loop` delegates to.

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
}
