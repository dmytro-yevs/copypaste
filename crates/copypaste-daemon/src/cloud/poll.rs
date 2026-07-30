//! The loop that decides when a round happens.
//!
//! The cadence belongs to `copypaste_cloud::sync::cadence`; this loop asks the
//! driver what to wait. Manifest 05 §4.8: missed ticks skip rather than burst,
//! so the wait is computed after the previous round finished and a slow round
//! does not queue up the ones it overlapped.

use std::sync::Arc;
use std::time::Duration;

use copypaste_cloud::sync::SyncError;
use copypaste_ipc::CloudSyncData;
use tokio::sync::watch;
use tracing::{debug, info, warn};

use crate::cloud::source::StoreSource;
use crate::cloud::KEY_UPLOAD_FLOOR;

use crate::AppState;

/// How long the loop waits when nobody is signed in.
///
/// Nothing polls it awake by itself — sign-in notifies — so this is only the
/// interval at which the loop re-checks a state it expects not to have changed.
pub const SIGNED_OUT_INTERVAL: Duration = Duration::from_secs(60);

/// Run cloud rounds until shutdown.
pub async fn run(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    if !state.cloud.is_configured() {
        debug!("no cloud deployment configured; the sync loop is idle");
    }

    loop {
        let wait = state
            .cloud
            .driver()
            .map_or(SIGNED_OUT_INTERVAL, |driver| driver.poll_interval());

        tokio::select! {
            // Shutdown first, so a wake storm cannot starve teardown
            // (manifest 05 §4.9's `biased` select, same reason).
            biased;
            _ = shutdown.changed() => break,
            _ = state.cloud.wake_signal() => {}
            _ = tokio::time::sleep(wait) => {}
        }

        if *shutdown.borrow() {
            break;
        }
        sync_round(&state).await;
    }

    debug!("cloud sync loop stopped");
}

/// One push-then-pull round, with the outcome recorded for `cloud status`.
///
/// Returns `None` when nobody is signed in. Every failure is reported and
/// swallowed: a backend that is down must not stop the daemon, and the next
/// tick will try again.
pub async fn sync_round(state: &Arc<AppState>) -> Option<Result<CloudSyncData, SyncError>> {
    let driver = state.cloud.driver()?;
    let source = StoreSource::new(Arc::clone(state));

    // Captured *before* the round, so that everything created while it runs is
    // still offered on the next one. Advancing the floor to "now" afterwards
    // would silently drop items captured mid-round.
    let started_ms = copypaste_core::now_ms();
    let outcome = driver.sync(&source).await;

    // The refresh token rotates on any 401 recovery inside that call, so it is
    // written back whether the round succeeded or not.
    state.cloud.persist_session(&state.meta);

    match &outcome {
        Ok(stats) => {
            state.cloud.note_success(copypaste_core::now_ms());
            if let Err(e) = state
                .meta
                .set_state_ms(KEY_UPLOAD_FLOOR, source.next_floor(started_ms))
            {
                warn!(error = ?e, "could not advance the upload floor");
            }
            if stats.changed() || stats.skipped_sensitive > 0 {
                info!(
                    uploaded = stats.uploaded,
                    tombstoned = stats.tombstoned,
                    applied = stats.applied,
                    withheld = stats.skipped_sensitive,
                    "cloud sync round"
                );
            }
        }
        Err(e) => {
            // `SyncError`'s payloads are `&'static str`, so this cannot carry a
            // path, a token or row content into a log or onto the wire.
            warn!(error = %e, "cloud sync round failed");
            state.cloud.note_failure(describe(e));
        }
    }

    Some(outcome.map(to_wire))
}

/// A fixed sentence per failure, for `cloud status` and the CLI.
///
/// Deliberately not `e.to_string()`: the wire type is a `String` and this is the
/// point at which something could start being interpolated into it. A `match`
/// with no `_` arm means a new variant has to be given a sentence here.
pub fn describe(error: &SyncError) -> &'static str {
    match error {
        SyncError::Source(_) => "the local history could not be read",
        SyncError::Encrypt => "an item could not be encrypted for upload",
        SyncError::Unauthorized => "the backend rejected this session even after a refresh",
        SyncError::InvalidCredentials => {
            "the stored account credentials were rejected; sign in again"
        }
        SyncError::SessionExpired => "the session expired; sign in again",
        SyncError::RateLimited => "the backend is rate limiting this account",
        SyncError::Transport(_) => "the sync backend could not be reached",
    }
}

fn to_wire(stats: copypaste_cloud::SyncStats) -> CloudSyncData {
    let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
    CloudSyncData {
        uploaded: count(stats.uploaded),
        tombstoned: count(stats.tombstoned),
        downloaded: count(stats.downloaded),
        applied: count(stats.applied),
        skipped_sensitive: count(stats.skipped_sensitive),
        skipped_undecryptable: count(stats.skipped_undecryptable),
        skipped_future: count(stats.skipped_future),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::test_state;

    #[tokio::test]
    async fn a_round_with_nobody_signed_in_does_nothing() {
        let (state, _dir) = test_state("alpha");
        assert!(sync_round(&state).await.is_none());
        assert_eq!(state.cloud.status().last_sync_ms, None);
    }

    #[tokio::test]
    async fn the_loop_stops_on_shutdown() {
        let (state, _dir) = test_state("alpha");
        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(run(Arc::clone(&state), rx));
        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .expect("the loop must observe shutdown")
            .expect("no panic");
    }

    #[test]
    fn every_failure_has_a_pathless_sentence() {
        for error in [
            SyncError::Source("x"),
            SyncError::Encrypt,
            SyncError::Unauthorized,
            SyncError::InvalidCredentials,
            SyncError::SessionExpired,
            SyncError::RateLimited,
            SyncError::Transport("x"),
        ] {
            let message = describe(&error);
            assert!(!message.contains('/'), "{message}");
            assert!(!message.contains('\\'), "{message}");
        }
    }
}
