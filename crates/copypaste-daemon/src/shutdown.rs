//! Observing shutdown from inside a round, and finishing teardown regardless.

use std::path::Path;
use std::time::Duration;

use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio::time::timeout;
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
pub async fn teardown(
    state: &AppState,
    loops: Vec<(&'static str, JoinHandle<()>)>,
    socket_path: &Path,
) {
    let joined = async {
        for (name, task) in loops {
            if let Err(e) = task.await {
                warn!(loop_name = name, error = ?e, "a background loop did not shut down cleanly");
            }
        }
    };
    if timeout(TEARDOWN_BUDGET, joined).await.is_err() {
        warn!("a background loop outlived the shutdown budget");
    }

    if let Err(e) = state.p2p.peers().flush() {
        warn!(error = ?e, "could not persist the paired-device list on shutdown");
    }
    remove_socket(socket_path);
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use super::*;
    use crate::testutil::{peer_at, test_state};

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
