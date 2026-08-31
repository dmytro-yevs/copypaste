//! Observing shutdown from inside a round, and finishing teardown regardless.

use std::path::Path;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::{JoinError, JoinHandle};
#[cfg(test)]
use tokio::time::timeout;
use tokio::time::{timeout_at, Instant};
use tracing::warn;

use crate::startup::remove_socket;
use crate::AppState;

/// How long teardown waits for the background loops before it persists what it
/// has and leaves.
///
/// Not a nicety. Windows kills the process a few seconds after a console-close
/// event whatever the handler does, and the app force-stops a daemon that
/// outlives its own quit budget — and a kill reaches neither the peer flush nor
/// the socket removal below. A loop still running here has not observed
/// shutdown and is not going to.
pub const TEARDOWN_BUDGET: Duration = Duration::from_secs(5);

/// Resolves when shutdown has been asked for, and never when there is nothing
/// watching it — a `None` receiver is a caller with no teardown to observe,
/// such as a unit test driving one round.
pub async fn requested(shutdown: Option<watch::Receiver<bool>>) {
    let Some(mut shutdown) = shutdown else {
        std::future::pending::<()>().await;
        return;
    };
    if *shutdown.borrow() {
        return;
    }
    while shutdown.changed().await.is_ok() {
        if *shutdown.borrow() {
            return;
        }
    }
    std::future::pending::<()>().await;
}

/// Wait out the background loops, then persist and clean up either way.
///
/// `loops` contains cooperative async owners only. A started `spawn_blocking`
/// handle needs a lifecycle owner that can signal its closure; Tokio cannot
/// abort the blocking closure through its outer handle.
pub async fn teardown(
    state: &AppState,
    loops: Vec<(&'static str, JoinHandle<()>)>,
    socket_path: &Path,
) {
    stop_loops(loops).await;
    if let Err(e) = flush_peers(state).await {
        warn!(error = ?e, "could not persist the paired-device list on shutdown");
    }
    remove_socket(socket_path);
}

pub async fn stop_loops(loops: Vec<(&'static str, JoinHandle<()>)>) {
    let mut loops = loops;
    let deadline = Instant::now() + TEARDOWN_BUDGET;
    let mut joined = 0;
    for (name, task) in &mut loops {
        match timeout_at(deadline, task).await {
            Ok(result) => record_loop_result(*name, result),
            Err(_) => {
                warn!("a background loop outlived the shutdown budget");
                break;
            }
        }
        joined += 1;
    }
    if joined != loops.len() {
        for (_, task) in loops.iter().skip(joined) {
            task.abort();
        }
        for (name, task) in loops.iter_mut().skip(joined) {
            record_loop_result(*name, task.await);
        }
    }
}

pub async fn flush_peers(state: &AppState) -> anyhow::Result<()> {
    let node = std::sync::Arc::clone(state.p2p.node());
    flush_before_release(move || node.peers().flush().map_err(anyhow::Error::from)).await
}

pub async fn flush_peers_before_listener_release(
    state: &AppState,
    release_listener: impl FnOnce(),
) -> anyhow::Result<()> {
    let node = std::sync::Arc::clone(state.p2p.node());
    flush_before_listener_release(
        move || node.peers().flush().map_err(anyhow::Error::from),
        release_listener,
    )
    .await
}

async fn flush_before_release<F>(flush: F) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()> + Send + 'static,
{
    tokio::task::spawn_blocking(flush)
        .await
        .map_err(anyhow::Error::from)??;
    Ok(())
}

async fn flush_before_listener_release<F>(
    flush: F,
    release_listener: impl FnOnce(),
) -> anyhow::Result<()>
where
    F: FnOnce() -> anyhow::Result<()> + Send + 'static,
{
    let result = flush_before_release(flush).await;
    release_listener();
    result
}

pub fn release_endpoint(socket_path: &Path) {
    remove_socket(socket_path);
}

fn record_loop_result(name: &'static str, result: Result<(), JoinError>) {
    match result {
        Ok(()) => {}
        Err(error) if error.is_cancelled() => {
            warn!(
                loop_name = name,
                "a background loop was cancelled during shutdown"
            );
        }
        Err(error) if error.is_panic() => {
            warn!(loop_name = name, error = ?error, "a background loop panicked during shutdown");
        }
        Err(error) => {
            warn!(loop_name = name, error = ?error, "a background loop did not shut down cleanly");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    use std::sync::{Condvar, Mutex};

    use tokio::sync::oneshot;

    use super::*;
    use crate::testutil::{peer_at, test_state};

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// The property DMY-159 asks for from the daemon side: a loop that never
    /// observes shutdown must not cost the paired-device list or leave the
    /// socket behind. The app kills a daemon that overruns its quit budget, and
    /// a kill reaches neither.
    #[tokio::test(start_paused = true)]
    async fn a_loop_that_never_stops_still_leaves_a_clean_socket_and_peer_file() {
        let (state, dir) = test_state("teardown-budget");
        let socket = dir.path().join("copypaste.sock");
        std::fs::write(&socket, b"").unwrap();
        let peer = peer_at(&state, "the other laptop", "127.0.0.1:9");
        let peers_file = dir.path().join(copypaste_p2p::peers::DEFAULT_FILE_NAME);
        let wedged = tokio::spawn(std::future::pending::<()>());

        teardown(&state, vec![("wedged", wedged)], &socket).await;

        assert!(!socket.exists(), "the socket outlived the daemon");
        let persisted = std::fs::read_to_string(&peers_file).expect("the paired-device list");
        assert!(
            persisted.contains(&peer.pairing_id),
            "the pairing was lost on shutdown"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn a_loop_that_stops_is_waited_for_rather_than_abandoned() {
        let (state, dir) = test_state("teardown-graceful");
        let socket = dir.path().join("copypaste.sock");
        std::fs::write(&socket, b"").unwrap();
        let done = Arc::new(AtomicBool::new(false));
        let task = tokio::spawn({
            let done = Arc::clone(&done);
            async move {
                tokio::time::sleep(TEARDOWN_BUDGET / 2).await;
                done.store(true, Ordering::SeqCst);
            }
        });

        teardown(&state, vec![("slow", task)], &socket).await;

        assert!(done.load(Ordering::SeqCst), "teardown did not wait");
        assert!(!socket.exists());
    }

    #[tokio::test]
    async fn final_flush_failure_releases_the_listener_and_remains_an_error() {
        let flushed = Arc::new(AtomicBool::new(false));
        let released = Arc::new(AtomicBool::new(false));
        let result = flush_before_listener_release(
            {
                let flushed = Arc::clone(&flushed);
                move || {
                    flushed.store(true, Ordering::SeqCst);
                    Err(anyhow::anyhow!("test flush failure"))
                }
            },
            {
                let flushed = Arc::clone(&flushed);
                let released = Arc::clone(&released);
                move || {
                    assert!(
                        flushed.load(Ordering::SeqCst),
                        "listener released before final flush"
                    );
                    released.store(true, Ordering::SeqCst);
                }
            },
        )
        .await;

        assert!(
            result.is_err(),
            "final flush failure was reported as clean shutdown"
        );
        assert!(
            released.load(Ordering::SeqCst),
            "listener was not released after flush failure"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn timed_out_teardown_cancels_and_joins_every_owned_loop() {
        let (state, dir) = test_state("teardown-owned-tasks");
        let socket = dir.path().join("copypaste.sock");
        std::fs::write(&socket, b"").unwrap();
        let stalled_dropped = Arc::new(AtomicBool::new(false));
        let later_dropped = Arc::new(AtomicBool::new(false));
        let later_completed = Arc::new(AtomicBool::new(false));
        let release_later = Arc::new(tokio::sync::Notify::new());
        let (stalled_ready_tx, stalled_ready_rx) = oneshot::channel();
        let (later_ready_tx, later_ready_rx) = oneshot::channel();
        let stalled = tokio::spawn({
            let stalled_dropped = Arc::clone(&stalled_dropped);
            async move {
                let _drop = DropFlag(stalled_dropped);
                stalled_ready_tx.send(()).unwrap();
                std::future::pending::<()>().await;
            }
        });
        let later = tokio::spawn({
            let later_dropped = Arc::clone(&later_dropped);
            let later_completed = Arc::clone(&later_completed);
            let release_later = Arc::clone(&release_later);
            async move {
                let _drop = DropFlag(later_dropped);
                later_ready_tx.send(()).unwrap();
                release_later.notified().await;
                later_completed.store(true, Ordering::SeqCst);
            }
        });
        stalled_ready_rx.await.unwrap();
        later_ready_rx.await.unwrap();

        teardown(
            &state,
            vec![("stalled", stalled), ("later", later)],
            &socket,
        )
        .await;

        assert!(stalled_dropped.load(Ordering::SeqCst));
        assert!(later_dropped.load(Ordering::SeqCst));
        release_later.notify_one();
        tokio::task::yield_now().await;
        assert!(
            !later_completed.load(Ordering::SeqCst),
            "a handle after the timed-out loop kept running after teardown"
        );
    }

    #[tokio::test(start_paused = true)]
    async fn timed_out_teardown_cleans_up_before_an_aborted_outer_blocking_task_finishes() {
        let (state, dir) = test_state("teardown-blocking-child");
        let socket = dir.path().join("copypaste.sock");
        std::fs::write(&socket, b"").unwrap();
        let release = Arc::new((Mutex::new(false), Condvar::new()));
        let blocking_finished = Arc::new(AtomicBool::new(false));
        let (stalled_ready_tx, stalled_ready_rx) = oneshot::channel();
        let (blocking_ready_tx, blocking_ready_rx) = oneshot::channel();
        let (blocking_finished_tx, blocking_finished_rx) = oneshot::channel();
        let stalled = tokio::spawn(async move {
            stalled_ready_tx.send(()).unwrap();
            std::future::pending::<()>().await;
        });
        let blocking_outer = tokio::spawn({
            let release = Arc::clone(&release);
            let blocking_finished = Arc::clone(&blocking_finished);
            async move {
                tokio::task::spawn_blocking(move || {
                    blocking_ready_tx.send(()).unwrap();
                    let (released, wake) = &*release;
                    let mut released = released.lock().unwrap();
                    while !*released {
                        released = wake.wait(released).unwrap();
                    }
                    blocking_finished.store(true, Ordering::SeqCst);
                    blocking_finished_tx.send(()).unwrap();
                })
                .await
                .unwrap();
            }
        });
        stalled_ready_rx.await.unwrap();
        blocking_ready_rx.await.unwrap();
        let teardown = tokio::spawn({
            let state = Arc::clone(&state);
            let socket = socket.clone();
            async move {
                teardown(
                    &state,
                    vec![("stalled", stalled), ("blocking outer", blocking_outer)],
                    &socket,
                )
                .await;
            }
        });
        tokio::task::yield_now().await;
        tokio::time::advance(TEARDOWN_BUDGET).await;
        teardown.await.unwrap();

        let socket_removed = !socket.exists();
        let blocking_still_running = !blocking_finished.load(Ordering::SeqCst);
        let (released, wake) = &*release;
        *released.lock().unwrap() = true;
        wake.notify_one();
        blocking_finished_rx.await.unwrap();
        assert!(socket_removed, "cleanup waited for the blocking child");
        assert!(
            blocking_still_running,
            "aborting the outer handle was treated as stopping blocking work"
        );
    }

    #[tokio::test]
    async fn a_receiver_that_is_already_stopping_resolves_at_once() {
        let (tx, rx) = watch::channel(true);
        requested(Some(rx)).await;
        drop(tx);
    }

    /// A dropped sender must not read as "shutdown": a round would abandon
    /// itself the moment nothing was left holding the channel.
    #[tokio::test(start_paused = true)]
    async fn a_dropped_sender_is_not_a_shutdown() {
        let (tx, rx) = watch::channel(false);
        drop(tx);
        assert!(
            timeout(Duration::from_secs(30), requested(Some(rx)))
                .await
                .is_err(),
            "a closed channel was treated as a shutdown request"
        );
    }
}
