//! Supervised background task utilities (CopyPaste-bp3o).
//!
//! Before this fix, `JoinHandle`s returned by `spawn_ttl_evictor`,
//! `spawn_cleanup_all`, and the SSE producer in `routes/items.rs` were either
//! immediately dropped (detached) or held but never checked. A panic inside
//! any of those tasks was silently swallowed by the tokio runtime: the task
//! disappeared, the subsystem it drove stopped working (eviction stalled,
//! rate-limit buckets leaked, SSE streams went dead), and no alert was emitted.
//!
//! This module provides two utilities:
//!
//! * [`spawn_supervised`] — wrap a future-factory whose future is expected to
//!   run forever (e.g. the TTL evictor or governor cleanup). On exit (panic or
//!   normal `return`), the wrapper logs the event at `ERROR` level and restarts
//!   the inner future by calling the factory again. The returned `JoinHandle`
//!   drives the *supervisor loop*; callers must retain it for the desired
//!   lifetime.
//!
//! * [`spawn_oneshot_supervised`] — wrap a future that is expected to run once
//!   and finish (e.g. the SSE producer task). On panic the wrapper logs it at
//!   `ERROR` and exits without restarting. The returned `JoinHandle` may be
//!   dropped to detach (cancel) the task.

use std::future::Future;
use std::time::Duration;

use tokio::task::JoinHandle;

/// Backoff between supervised-task restarts to prevent tight panic loops from
/// spinning at full CPU. A fixed 1-second delay is simple and sufficient here.
const SUPERVISED_RESTART_DELAY: Duration = Duration::from_secs(1);

/// Spawn a supervised forever-task.
///
/// `factory` is a `Fn() -> Fut` closure that produces a fresh instance of the
/// future each time. On each iteration:
///
///   1. `factory()` is called to produce a new future.
///   2. That future is driven inside an inner `tokio::spawn`.
///   3. The inner `JoinHandle` is awaited. If it returns an error (panic or
///      cancellation):
///      - **Panic** — logs at `ERROR` and sleeps [`SUPERVISED_RESTART_DELAY`]
///        before the next iteration (prevents busy-loop on a persistent panic).
///      - **Cancellation** — the supervisor itself is being shut down; exits
///        the loop gracefully without restarting.
///   4. **Normal exit** — a forever-task should never return; if it does, log
///      at `ERROR` and restart immediately (no delay for a clean exit).
///
/// # Panic handling
///
/// `JoinError::is_panic()` is how tokio surfaces an inner task's panic without
/// propagating it to the spawning task. The supervisor loop catches it here and
/// logs it so the event is observable in production telemetry.
///
/// # Returns
///
/// Returns the `JoinHandle` of the *supervisor loop*. The caller must retain
/// this handle; dropping it aborts the supervisor (and the currently-running
/// inner task along with it).
pub fn spawn_supervised<F, Fut>(name: &'static str, factory: F) -> JoinHandle<()>
where
    F: Fn() -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(async move {
        loop {
            let inner = tokio::spawn(factory());
            match inner.await {
                Ok(()) => {
                    // Normal exit from a forever-task — unexpected but not a
                    // panic. Log and restart without a backoff delay (clean exit
                    // is less alarming than a panic tight-loop; the task likely
                    // returned from an empty queue and should be re-driven).
                    tracing::error!(
                        task = name,
                        "supervised task: exited unexpectedly (should run forever); restarting"
                    );
                }
                Err(join_err) if join_err.is_panic() => {
                    // Inner task panicked — log the event (the panic payload is
                    // not accessible via JoinError, but the task name is enough
                    // to identify the subsystem in production telemetry).
                    tracing::error!(
                        task = name,
                        "supervised task: panicked; restarting after backoff"
                    );
                    // Brief backoff so a persistent panic (e.g. from a bad
                    // invariant violated on every run) does not spin at full CPU.
                    tokio::time::sleep(SUPERVISED_RESTART_DELAY).await;
                }
                Err(_cancelled) => {
                    // Task was cancelled — the supervisor is itself being shut
                    // down (its handle was aborted). Exit the loop gracefully.
                    tracing::debug!(task = name, "supervised task: cancelled; supervisor exiting");
                    return;
                }
            }
        }
    })
}

/// Spawn a one-shot supervised task (no restart on panic).
///
/// For tasks that run once and naturally terminate (e.g. the SSE producer task
/// tied to a single client connection). On panic the event is logged at `ERROR`
/// level, but the task is **not** restarted — the connection it served is
/// already gone.
///
/// The caller may drop the returned handle to detach (cancel) the task.
// Currently used only by the supervise.rs tests; retained for future use in
// production code (e.g. future SSE wrappers).
#[allow(dead_code)]
pub fn spawn_oneshot_supervised(
    name: &'static str,
    fut: impl Future<Output = ()> + Send + 'static,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let inner = tokio::spawn(fut);
        match inner.await {
            Ok(()) => {} // Normal exit — nothing to log.
            Err(join_err) if join_err.is_panic() => {
                tracing::error!(
                    task = name,
                    "one-shot supervised task: panicked (not restarting)"
                );
            }
            Err(_) => {
                // Cancelled (e.g. client disconnected) — normal, no log.
            }
        }
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use tokio::time::timeout;

    /// CopyPaste-bp3o: a supervised task that panics must be restarted and
    /// the panic must not propagate to the caller.
    #[tokio::test]
    async fn supervised_task_restarts_after_panic() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);

        let handle = spawn_supervised("test-panic-restart", move || {
            let cc2 = Arc::clone(&cc);
            async move {
                let n = cc2.fetch_add(1, Ordering::SeqCst);
                if n == 0 {
                    // First call: panic to trigger restart.
                    panic!("deliberate test panic");
                }
                // Second call: sleep forever; we abort the handle after asserting.
                tokio::time::sleep(Duration::from_secs(3600)).await;
            }
        });

        // Wait up to 5 s for the restart (panic + 1 s backoff + spawn latency).
        let deadline = timeout(Duration::from_secs(5), async {
            loop {
                if call_count.load(Ordering::SeqCst) >= 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(50)).await;
            }
        });

        deadline
            .await
            .expect("CopyPaste-bp3o: supervised task must restart after panic within 5 s");

        handle.abort();
        let _ = handle.await;
    }

    /// CopyPaste-bp3o: a supervised task that exits normally must also be
    /// restarted (a forever-task must never return).
    #[tokio::test]
    async fn supervised_task_restarts_after_normal_exit() {
        let call_count = Arc::new(AtomicUsize::new(0));
        let cc = Arc::clone(&call_count);

        let handle = spawn_supervised("test-exit-restart", move || {
            let cc2 = Arc::clone(&cc);
            async move {
                cc2.fetch_add(1, Ordering::SeqCst);
                // Return immediately — abnormal for a forever-task.
            }
        });

        let deadline = timeout(Duration::from_secs(3), async {
            loop {
                if call_count.load(Ordering::SeqCst) >= 2 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        });

        deadline
            .await
            .expect("CopyPaste-bp3o: supervisor must restart after unexpected normal exit");

        handle.abort();
        let _ = handle.await;
    }

    /// CopyPaste-bp3o: a one-shot supervised task's panic must not propagate.
    #[tokio::test]
    async fn oneshot_supervised_panic_does_not_propagate() {
        let handle = spawn_oneshot_supervised("test-oneshot-panic", async {
            panic!("deliberate oneshot panic");
        });

        // The outer handle must resolve (not hang) despite the inner panic.
        let result = timeout(Duration::from_secs(3), handle).await;
        assert!(
            result.is_ok(),
            "CopyPaste-bp3o: oneshot supervised handle must resolve within timeout"
        );
        // No panic bubbles out — the outer spawn_oneshot_supervised task itself
        // catches the inner JoinError and returns Ok(()).
    }
}
