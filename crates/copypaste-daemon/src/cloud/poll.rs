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

use crate::cloud::refresh::is_terminal_auth_error;
use crate::cloud::source::StoreSource;
use crate::cloud::{Driver, MSG_ACCOUNT_CHANGED};

use crate::AppState;

/// How long the loop waits when nobody is signed in.
///
/// Nothing polls it awake by itself — sign-in notifies — so this is only the
/// interval at which the loop re-checks a state it expects not to have changed.
pub const SIGNED_OUT_INTERVAL: Duration = Duration::from_secs(60);

/// A round abandoned because the daemon is stopping.
///
/// Not a failure the user should be told to act on, and it is deliberately not
/// `MSG_ACCOUNT_CHANGED`: the account is fine, the process is leaving.
pub(crate) const MSG_SHUTTING_DOWN: &str = "the daemon is shutting down";

/// Resolves when shutdown has been asked for, and never when there is nothing
/// watching it — a `None` receiver is a caller with no teardown to observe,
/// such as a unit test driving one round.
async fn shutdown_requested(shutdown: Option<watch::Receiver<bool>>) {
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
        // A tick that collides with a round already in flight has nothing to
        // add: it would push the same window and pull behind the same cursor,
        // and the two completions would race to commit the upload floor.
        let Some(permit) = state.cloud.try_begin_round() else {
            debug!("a cloud round is already running; skipping this tick");
            continue;
        };
        round_with_permit(&state, permit, Some(shutdown.clone())).await;
    }

    debug!("cloud sync loop stopped");
}

/// One push-then-pull round, with the outcome recorded for `cloud status`.
///
/// Returns `None` when nobody is signed in. Every failure is reported and
/// swallowed: a backend that is down must not stop the daemon, and the next
/// tick will try again.
pub async fn sync_round(state: &Arc<AppState>) -> Option<Result<CloudSyncData, SyncError>> {
    // The explicit request waits for a running round rather than duplicating
    // it: the user asked after that round began its scan, so its result would
    // answer for a history older than the request.
    let permit = state.cloud.begin_round().await;
    round_with_permit(state, permit, Some(state.shutdown_rx())).await
}

async fn round_with_permit(
    state: &Arc<AppState>,
    permit: copypaste_core::sync::RoundGuard,
    shutdown: Option<watch::Receiver<bool>>,
) -> Option<Result<CloudSyncData, SyncError>> {
    // Held for the whole round, released when it returns, whichever arm of the
    // select below finishes first.
    let _permit = permit;
    if !state.settings.get().sync_enabled {
        return None;
    }
    let account = state.cloud.round()?;
    let driver = account.driver;
    let source = StoreSource::for_round(Arc::clone(state), Arc::clone(&driver));

    // Captured *before* the round, so that everything created while it runs is
    // still offered on the next one. Advancing the floor to "now" afterwards
    // would silently drop items captured mid-round.
    let started_ms = copypaste_core::now_ms();
    let outcome = tokio::select! {
        biased;
        _ = account.cancel.cancelled() => Err(SyncError::Source(MSG_ACCOUNT_CHANGED)),
        // Teardown does not wait out a round's network time. Abandoning one
        // costs nothing: the floor is only committed on success below, so the
        // next start re-derives exactly the same work from local state.
        () = shutdown_requested(shutdown) => Err(SyncError::Source(MSG_SHUTTING_DOWN)),
        outcome = driver.sync(&source) => outcome,
    };

    // The refresh token rotates on any 401 recovery inside that call, so it is
    // written back whether the round succeeded or not.
    state.cloud.persist_session(&state.store, &driver);

    match &outcome {
        Ok(stats) => {
            let at_ms = copypaste_core::now_ms();
            state.cloud.note_driver_success(&state.meta, &driver, at_ms);
            if let Err(e) = source.commit_upload_floor(started_ms) {
                warn!(error = ?e, "could not advance the upload floor");
            }
            if stats.applied > 0 {
                state.note_remote_change();
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
        // Abandoned rather than failed: recording it would leave "sync is
        // broken" on the status a restarted daemon reads back, for a round
        // that was cut short on purpose.
        Err(SyncError::Source(MSG_SHUTTING_DOWN)) => {
            debug!("a cloud round was abandoned for shutdown");
        }
        Err(e) => {
            // `SyncError`'s payloads are `&'static str`, so this cannot carry a
            // path, a token or row content into a log or onto the wire.
            warn!(error = %e, "cloud sync round failed");
            record_failure(state, &driver, e);
        }
    }

    Some(outcome.map(to_wire))
}

fn record_failure(state: &AppState, driver: &Arc<Driver>, error: &SyncError) {
    let message = describe(error);
    let revision = driver.session_revision();
    if is_terminal_auth_error(error) {
        state
            .cloud
            .invalidate_session(&state.store, driver, revision, message);
    } else {
        state.cloud.note_driver_failure(driver, revision, message);
    }
}

/// A fixed sentence per failure, for `cloud status` and the CLI.
///
/// Deliberately not `e.to_string()`: the wire type is a `String` and this is the
/// point at which something could start being interpolated into it. A `match`
/// with no `_` arm means a new variant has to be given a sentence here.
pub fn describe(error: &SyncError) -> &'static str {
    match error {
        SyncError::Source(MSG_SHUTTING_DOWN) => MSG_SHUTTING_DOWN,
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
        skipped_forged: count(stats.skipped_forged),
        skipped_future: count(stats.skipped_future),
        skipped_too_large: count(stats.skipped_too_large),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud::{KEY_REFRESH, KEY_SYNC_KEY, KEY_UPLOAD_FLOOR};
    use crate::testutil::{test_state, test_state_with_cloud};

    /// `cloud sync` used to call [`sync_round`] straight through while the poll
    /// loop was inside one of its own. Two rounds then pushed the same window,
    /// pulled behind the same cursor, and raced to commit the upload floor —
    /// and `commit_upload_floor` compares against the floor *its own* scan
    /// started from, so the loser silently discards its own progress.
    #[tokio::test]
    async fn a_second_round_cannot_run_beside_one_already_in_flight() {
        let (state, _dir) = test_state("alpha");
        let held = state.cloud.begin_round().await;
        assert!(state.cloud.round_in_flight());
        assert!(
            state.cloud.try_begin_round().is_none(),
            "the poll loop started a round beside a running one"
        );
        drop(held);
        assert!(state.cloud.try_begin_round().is_some());
    }

    /// The two callers want different things from a busy gate: a tick skips,
    /// an explicit request queues. A queued request must actually run — a
    /// `sync` that silently did nothing because a poll happened to be in
    /// flight is the same failure seen from the other side.
    #[tokio::test]
    async fn an_explicit_round_queues_behind_a_running_one_rather_than_duplicating_it() {
        let (state, _dir) = test_state("alpha");
        let held = state.cloud.begin_round().await;
        let queued = tokio::spawn({
            let state = Arc::clone(&state);
            async move {
                // Signed out, so this is the gate and nothing else.
                sync_round(&state).await.is_none()
            }
        });
        tokio::task::yield_now().await;
        assert!(!queued.is_finished(), "the request did not wait its turn");
        drop(held);
        assert!(queued.await.unwrap());
    }

    /// A round in flight when the daemon is asked to stop must not hold
    /// teardown for the length of its network time. Abandoning it costs
    /// nothing: the upload floor is only committed on success, so the next
    /// start re-derives the same work from local state (AT-33).
    #[tokio::test]
    async fn a_round_abandoned_for_shutdown_is_not_recorded_as_a_failure() {
        use copypaste_cloud::crypto::derive_sync_key;
        use copypaste_cloud::CloudConfig;

        // A URL that resolves nowhere, so the round is inside its transport
        // when shutdown arrives.
        let config = CloudConfig::new_loopback("http://127.0.0.1:1", "anon").unwrap();
        let (state, _dir) = crate::testutil::test_state_with_cloud(
            "alpha",
            crate::cloud::Cloud::new(Some(config.clone())),
        );
        state.cloud.install(
            &state,
            config,
            "a@example.com".into(),
            "user-1".into(),
            derive_sync_key("correct horse battery staple", "user-1").unwrap(),
            copypaste_cloud::auth::Session {
                access_token: "access-1".into(),
                refresh_token: "refresh-1".into(),
                user_id: "user-1".into(),
                expires_at_ms: i64::MAX,
            },
        );
        crate::testutil::add(&state, "captured before the daemon was stopped");
        let floor = state.meta.state_ms(KEY_UPLOAD_FLOOR).unwrap();

        // Subscribed first, as `main` does: `watch::Sender::send` reports
        // an error and leaves the value alone when nothing is listening.
        let _watching = state.shutdown_rx();
        state.request_shutdown();
        let outcome = sync_round(&state).await;

        assert!(
            matches!(outcome, Some(Err(SyncError::Source(MSG_SHUTTING_DOWN)))),
            "{outcome:?}"
        );
        assert_eq!(
            state.meta.state_ms(KEY_UPLOAD_FLOOR).unwrap(),
            floor,
            "an abandoned round moved the upload floor"
        );
        assert_eq!(
            state.cloud.status().last_error,
            None,
            "stopping the daemon was recorded as sync being broken"
        );
    }

    #[tokio::test]
    async fn a_round_with_nobody_signed_in_does_nothing() {
        let (state, _dir) = test_state("alpha");
        assert!(sync_round(&state).await.is_none());
        assert_eq!(state.cloud.status().last_sync_ms, None);
    }

    /// Manifest 05 AT-33 (`CopyPaste-1t38`), which v1 needed an in-memory retry
    /// queue for and v2 gets from the floor: a round against an unreachable
    /// backend must leave the cursor exactly where it was, so the next round
    /// re-derives the same work from local state. There is no queue to lose.
    #[tokio::test]
    async fn a_failed_round_leaves_the_upload_floor_where_it_was() {
        use copypaste_cloud::crypto::derive_sync_key;
        use copypaste_cloud::sync::CloudSource;
        use copypaste_cloud::CloudConfig;

        // A URL that resolves nowhere: the round must fail at the transport
        // rather than hang.
        let config = CloudConfig::new_loopback("http://127.0.0.1:1", "anon").unwrap();
        let (state, _dir) = crate::testutil::test_state_with_cloud(
            "alpha",
            crate::cloud::Cloud::new(Some(config.clone())),
        );
        state.cloud.install(
            &state,
            config,
            "a@example.com".into(),
            "user-1".into(),
            derive_sync_key("correct horse battery staple", "user-1").unwrap(),
            copypaste_cloud::auth::Session {
                access_token: "access-1".into(),
                refresh_token: "refresh-1".into(),
                user_id: "user-1".into(),
                expires_at_ms: i64::MAX,
            },
        );

        crate::testutil::add(&state, "captured while the backend was down");
        let floor = state.meta.state_ms(KEY_UPLOAD_FLOOR).unwrap();

        assert!(matches!(sync_round(&state).await, Some(Err(_))));

        assert_eq!(state.meta.state_ms(KEY_UPLOAD_FLOOR).unwrap(), floor);
        assert_eq!(
            StoreSource::new(Arc::clone(&state))
                .local_changes_since(floor)
                .unwrap()
                .len(),
            1,
            "the change stopped being offered after one failed round"
        );
        // And the user can tell, which is the other half: an upload that
        // silently never happens is the failure this guards.
        let status = state.cloud.status();
        assert_eq!(
            status.last_error.as_deref(),
            Some("the sync backend could not be reached")
        );
        assert_eq!(status.last_sync_ms, None);
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
    fn a_terminal_refresh_failure_invalidates_the_live_account() {
        use copypaste_cloud::crypto::derive_sync_key;
        use copypaste_cloud::CloudConfig;

        let config = CloudConfig::new("https://example.supabase.co", "anon").unwrap();
        let (state, _dir) =
            test_state_with_cloud("alpha", crate::cloud::Cloud::new(Some(config.clone())));
        let key = derive_sync_key("correct horse battery staple", "user-1").unwrap();
        let key_bytes = key.to_bytes();
        state.cloud.install(
            &state,
            config,
            "a@example.com".into(),
            "user-1".into(),
            key,
            copypaste_cloud::auth::Session {
                access_token: "access-1".into(),
                refresh_token: "refresh-1".into(),
                user_id: "user-1".into(),
                expires_at_ms: i64::MAX,
            },
        );
        state.cloud.persist(&state.store, &key_bytes).unwrap();
        let driver = state.cloud.driver().unwrap();
        let updates = state.cloud.session_updates();

        record_failure(&state, &driver, &SyncError::SessionExpired);

        let status = state.cloud.status();
        assert!(!status.signed_in);
        assert_eq!(status.email, None);
        assert_eq!(
            status.last_error.as_deref(),
            Some("the session expired; sign in again")
        );
        assert_eq!(state.meta.state(KEY_REFRESH).unwrap(), None);
        assert_eq!(state.meta.state(KEY_SYNC_KEY).unwrap(), None);
        assert!(updates.has_changed().unwrap());
    }

    /// The count that says something wrote into the account without the
    /// passphrase. It reached the log and stopped there, so the one signal that
    /// separates an attack from a quiet day was unreachable from any client.
    #[test]
    fn a_refused_forgery_reaches_the_wire() {
        let wire = to_wire(copypaste_cloud::SyncStats {
            downloaded: 4,
            skipped_forged: 3,
            ..Default::default()
        });
        assert_eq!(wire.skipped_forged, 3);
        assert_eq!(wire.applied, 0);
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
