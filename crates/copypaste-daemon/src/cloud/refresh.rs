//! Keep the live cloud session ahead of its access-token expiry.
//!
//! This task owns only refresh timing. GoTrue request retries remain in
//! `copypaste_cloud::auth`, rotated-session storage remains in [`super::Cloud`],
//! and the session revision watch is shared with Realtime.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use backon::{BackoffBuilder, ExponentialBuilder};
use copypaste_clock::{SystemWallClock, WallClock};
use copypaste_cloud::auth::{REFRESH_MARGIN_MS, Session};
use copypaste_cloud::sync::SyncError;
use tokio::sync::watch;
use tracing::{debug, warn};

use super::{CREDENTIAL_KEYS, Cloud, Driver, poll};
use crate::AppState;
use crate::meta::Meta;

const MIN_REFRESH_INTERVAL: Duration = Duration::from_secs(5);
const MIN_REFRESH_INTERVAL_MS: i64 = 5_000;
const SIGNED_OUT_INTERVAL: Duration = Duration::from_secs(10);
const RETRY_MIN: Duration = Duration::from_secs(30);
const RETRY_MAX: Duration = Duration::from_secs(300);

pub(crate) fn is_terminal_auth_error(error: &SyncError) -> bool {
    matches!(
        error,
        SyncError::InvalidCredentials | SyncError::SessionExpired
    )
}

impl Cloud {
    fn session_refreshed(&self, meta: &Meta, expected: &Arc<Driver>) {
        let account_guard = self.lock_account();
        let Some(account) = account_guard
            .as_ref()
            .filter(|account| Arc::ptr_eq(&account.driver, expected))
        else {
            return;
        };
        if let Err(error) = super::write_session(meta, &account.driver) {
            warn!(error = ?error, "could not persist the rotated cloud session");
        }
        *self.lock_error() = None;
        drop(account_guard);
        self.notify_session_changed();
    }

    fn is_current_driver(&self, expected: &Arc<Driver>) -> bool {
        self.lock_account()
            .as_ref()
            .is_some_and(|account| Arc::ptr_eq(&account.driver, expected))
    }

    pub(crate) fn note_driver_failure(&self, expected: &Arc<Driver>, message: &'static str) {
        let account = self.lock_account();
        if account
            .as_ref()
            .is_some_and(|account| Arc::ptr_eq(&account.driver, expected))
        {
            *self.lock_error() = Some(message);
        }
    }

    pub(crate) fn invalidate_session(
        &self,
        meta: &Meta,
        expected: &Arc<Driver>,
        message: &'static str,
    ) {
        let mut account = self.lock_account();
        if !account
            .as_ref()
            .is_some_and(|account| Arc::ptr_eq(&account.driver, expected))
        {
            return;
        }
        let _expired = account.take();
        if let Err(error) = meta.clear_state(CREDENTIAL_KEYS) {
            warn!(error = ?error, "could not clear the expired cloud account");
        }
        self.last_sync_ms.store(0, Ordering::Release);
        *self.lock_error() = Some(message);
        drop(account);
        self.notify_session_changed();
    }
}

trait RefreshDriver: Send + Sync {
    fn next_refresh(&self, now_ms: i64) -> Duration;
    fn refresh(&self) -> Pin<Box<dyn Future<Output = Result<(), SyncError>> + Send + '_>>;
}

impl RefreshDriver for Driver {
    fn next_refresh(&self, now_ms: i64) -> Duration {
        self.inspect_session(|session| refresh_delay(session, now_ms))
    }

    fn refresh(&self) -> Pin<Box<dyn Future<Output = Result<(), SyncError>> + Send + '_>> {
        Box::pin(self.refresh_session())
    }
}

trait RefreshTarget: Send + Sync {
    type Driver: RefreshDriver + 'static;

    fn driver(&self) -> Option<Arc<Self::Driver>>;
    fn session_updates(&self) -> watch::Receiver<u64>;
    fn is_current(&self, driver: &Arc<Self::Driver>) -> bool;
    fn refreshed(&self, driver: &Arc<Self::Driver>);
    fn failed(&self, driver: &Arc<Self::Driver>, message: &'static str);
    fn invalidate(&self, driver: &Arc<Self::Driver>, message: &'static str);
}

impl RefreshTarget for AppState {
    type Driver = Driver;

    fn driver(&self) -> Option<Arc<Self::Driver>> {
        self.cloud.driver()
    }

    fn session_updates(&self) -> watch::Receiver<u64> {
        self.cloud.session_updates()
    }

    fn is_current(&self, driver: &Arc<Self::Driver>) -> bool {
        self.cloud.is_current_driver(driver)
    }

    fn refreshed(&self, driver: &Arc<Self::Driver>) {
        self.cloud.session_refreshed(&self.meta, driver);
    }

    fn failed(&self, driver: &Arc<Self::Driver>, message: &'static str) {
        self.cloud.note_driver_failure(driver, message);
    }

    fn invalidate(&self, driver: &Arc<Self::Driver>, message: &'static str) {
        self.cloud.invalidate_session(&self.meta, driver, message);
    }
}

pub async fn run(state: Arc<AppState>, shutdown: watch::Receiver<bool>) {
    maintain(state, SystemWallClock, shutdown).await;
}

async fn maintain<T: RefreshTarget, C: WallClock>(
    target: Arc<T>,
    clock: C,
    mut shutdown: watch::Receiver<bool>,
) {
    let mut session_updates = target.session_updates();

    'session: loop {
        if *shutdown.borrow() {
            break;
        }

        let Some(driver) = target.driver() else {
            tokio::select! {
                biased;
                _ = shutdown.changed() => break,
                changed = session_updates.changed() => {
                    if changed.is_err() {
                        break;
                    }
                }
                _ = tokio::time::sleep(SIGNED_OUT_INTERVAL) => {}
            }
            continue;
        };

        let delay = driver.next_refresh(clock.now_ms());
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            changed = session_updates.changed() => {
                if changed.is_err() {
                    break;
                }
                continue;
            }
            _ = tokio::time::sleep(delay) => {}
        }

        let mut retry = refresh_backoff();

        loop {
            if !target.is_current(&driver) {
                continue 'session;
            }

            let result = tokio::select! {
                biased;
                _ = shutdown.changed() => break 'session,
                result = driver.refresh() => result,
            };

            match result {
                Ok(()) => {
                    target.refreshed(&driver);
                    debug!("cloud session refreshed");
                    continue 'session;
                }
                Err(error) if is_terminal_auth_error(&error) => {
                    let message = poll::describe(&error);
                    warn!(error = %error, "cloud session cannot be refreshed");
                    target.invalidate(&driver, message);
                    continue 'session;
                }
                Err(error) => {
                    let message = poll::describe(&error);
                    let delay = retry.next().unwrap_or(RETRY_MAX);
                    warn!(error = %error, retry_secs = delay.as_secs(), "cloud refresh failed");
                    target.failed(&driver, message);
                    tokio::select! {
                        biased;
                        _ = shutdown.changed() => break 'session,
                        changed = session_updates.changed() => {
                            if changed.is_err() {
                                break 'session;
                            }
                            continue 'session;
                        }
                        _ = tokio::time::sleep(delay) => {}
                    }
                }
            }
        }
    }

    debug!("cloud refresh loop stopped");
}

fn refresh_backoff() -> impl Iterator<Item = Duration> {
    ExponentialBuilder::new()
        .with_min_delay(RETRY_MIN)
        .with_max_delay(RETRY_MAX)
        .without_max_times()
        .build()
}

fn refresh_delay(session: &Session, now_ms: i64) -> Duration {
    if session.needs_refresh(now_ms) {
        return MIN_REFRESH_INTERVAL;
    }
    let delay_ms = session
        .expires_at_ms
        .saturating_sub(now_ms)
        .saturating_sub(REFRESH_MARGIN_MS)
        .max(MIN_REFRESH_INTERVAL_MS);
    Duration::from_millis(u64::try_from(delay_ms).unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Mutex, MutexGuard};

    use super::*;

    enum Reply {
        Ok(Session),
        Err(SyncError),
        Pending,
    }

    struct FakeDriver {
        session: Mutex<Session>,
        replies: Mutex<VecDeque<Reply>>,
        attempts: AtomicUsize,
    }

    impl FakeDriver {
        fn new(session: Session, replies: impl IntoIterator<Item = Reply>) -> Arc<Self> {
            Arc::new(Self {
                session: Mutex::new(session),
                replies: Mutex::new(replies.into_iter().collect()),
                attempts: AtomicUsize::new(0),
            })
        }

        fn lock_session(&self) -> MutexGuard<'_, Session> {
            self.session
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        }

        fn lock_replies(&self) -> MutexGuard<'_, VecDeque<Reply>> {
            self.replies
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        }
    }

    impl RefreshDriver for FakeDriver {
        fn next_refresh(&self, now_ms: i64) -> Duration {
            refresh_delay(&self.lock_session(), now_ms)
        }

        fn refresh(&self) -> Pin<Box<dyn Future<Output = Result<(), SyncError>> + Send + '_>> {
            Box::pin(async move {
                self.attempts.fetch_add(1, Ordering::SeqCst);
                let reply = self.lock_replies().pop_front().unwrap_or(Reply::Pending);
                match reply {
                    Reply::Ok(session) => {
                        *self.lock_session() = session;
                        Ok(())
                    }
                    Reply::Err(error) => Err(error),
                    Reply::Pending => std::future::pending().await,
                }
            })
        }
    }

    struct FakeTarget {
        driver: Mutex<Option<Arc<FakeDriver>>>,
        revision: watch::Sender<u64>,
        persisted_refresh: Mutex<Option<String>>,
        last_error: Mutex<Option<&'static str>>,
    }

    struct FixedWallClock(i64);

    impl WallClock for FixedWallClock {
        fn now_ms(&self) -> i64 {
            self.0
        }
    }

    impl FakeTarget {
        fn new(driver: Arc<FakeDriver>) -> Arc<Self> {
            Arc::new(Self {
                driver: Mutex::new(Some(driver)),
                revision: watch::channel(0).0,
                persisted_refresh: Mutex::new(None),
                last_error: Mutex::new(None),
            })
        }

        fn lock_driver(&self) -> MutexGuard<'_, Option<Arc<FakeDriver>>> {
            self.driver
                .lock()
                .unwrap_or_else(|error| error.into_inner())
        }

        fn current(&self, expected: &Arc<FakeDriver>) -> bool {
            self.lock_driver()
                .as_ref()
                .is_some_and(|driver| Arc::ptr_eq(driver, expected))
        }
    }

    impl RefreshTarget for FakeTarget {
        type Driver = FakeDriver;

        fn driver(&self) -> Option<Arc<Self::Driver>> {
            self.lock_driver().clone()
        }

        fn session_updates(&self) -> watch::Receiver<u64> {
            self.revision.subscribe()
        }

        fn is_current(&self, driver: &Arc<Self::Driver>) -> bool {
            self.current(driver)
        }

        fn refreshed(&self, driver: &Arc<Self::Driver>) {
            if !self.current(driver) {
                return;
            }
            *self
                .persisted_refresh
                .lock()
                .unwrap_or_else(|error| error.into_inner()) =
                Some(driver.lock_session().refresh_token.clone());
            *self
                .last_error
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = None;
            self.revision
                .send_modify(|revision| *revision = revision.wrapping_add(1));
        }

        fn failed(&self, driver: &Arc<Self::Driver>, message: &'static str) {
            if self.current(driver) {
                *self
                    .last_error
                    .lock()
                    .unwrap_or_else(|error| error.into_inner()) = Some(message);
            }
        }

        fn invalidate(&self, driver: &Arc<Self::Driver>, message: &'static str) {
            let mut current = self.lock_driver();
            if !current
                .as_ref()
                .is_some_and(|candidate| Arc::ptr_eq(candidate, driver))
            {
                return;
            }
            current.take();
            *self
                .last_error
                .lock()
                .unwrap_or_else(|error| error.into_inner()) = Some(message);
            drop(current);
            self.revision
                .send_modify(|revision| *revision = revision.wrapping_add(1));
        }
    }

    fn session(refresh_token: &str, expires_at_ms: i64) -> Session {
        Session {
            access_token: format!("access-{refresh_token}"),
            refresh_token: refresh_token.into(),
            user_id: "user-1".into(),
            expires_at_ms,
        }
    }

    async fn advance(duration: Duration) {
        tokio::time::advance(duration).await;
        tokio::task::yield_now().await;
    }

    #[test]
    fn expiry_schedule_keeps_the_margin_and_floor() {
        assert_eq!(
            refresh_delay(&session("r", 120_000), 0),
            Duration::from_secs(60)
        );
        assert_eq!(
            refresh_delay(&session("r", 65_000), 0),
            MIN_REFRESH_INTERVAL
        );
        assert_eq!(
            refresh_delay(&session("r", 60_000), 0),
            MIN_REFRESH_INTERVAL
        );
        assert_eq!(refresh_delay(&session("r", -1), 0), MIN_REFRESH_INTERVAL);
    }

    #[test]
    fn retry_schedule_starts_at_thirty_seconds_and_caps_at_five_minutes() {
        let delays: Vec<_> = refresh_backoff().take(7).collect();
        assert_eq!(
            delays,
            [
                Duration::from_secs(30),
                Duration::from_secs(60),
                Duration::from_secs(120),
                Duration::from_secs(240),
                Duration::from_secs(300),
                Duration::from_secs(300),
                Duration::from_secs(300),
            ]
        );
    }

    #[test]
    fn only_unrecoverable_refresh_failures_are_terminal() {
        assert!(is_terminal_auth_error(&SyncError::InvalidCredentials));
        assert!(is_terminal_auth_error(&SyncError::SessionExpired));
        assert!(!is_terminal_auth_error(&SyncError::Unauthorized));
        assert!(!is_terminal_auth_error(&SyncError::Transport("offline")));
    }

    #[tokio::test(start_paused = true)]
    async fn rotation_is_persisted_and_wakes_session_watchers() {
        let driver = FakeDriver::new(
            session("refresh-1", 65_000),
            [Reply::Ok(session("refresh-2", 600_000))],
        );
        let target = FakeTarget::new(Arc::clone(&driver));
        let mut updates = target.session_updates();
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(maintain(
            Arc::clone(&target),
            FixedWallClock(0),
            shutdown_rx,
        ));
        tokio::task::yield_now().await;

        advance(Duration::from_secs(4)).await;
        assert_eq!(driver.attempts.load(Ordering::SeqCst), 0);
        advance(Duration::from_secs(1)).await;
        updates.changed().await.unwrap();

        assert_eq!(driver.attempts.load(Ordering::SeqCst), 1);
        assert_eq!(
            target
                .persisted_refresh
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .as_deref(),
            Some("refresh-2")
        );
        shutdown_tx.send(true).unwrap();
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn transient_backoff_is_bounded_and_resets_after_success() {
        let driver = FakeDriver::new(
            session("refresh-1", 0),
            [
                Reply::Err(SyncError::Transport("offline")),
                Reply::Err(SyncError::Transport("offline")),
                Reply::Ok(session("refresh-2", 0)),
                Reply::Err(SyncError::Transport("offline")),
                Reply::Ok(session("refresh-3", 600_000)),
            ],
        );
        let target = FakeTarget::new(Arc::clone(&driver));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(maintain(
            Arc::clone(&target),
            FixedWallClock(0),
            shutdown_rx,
        ));
        tokio::task::yield_now().await;

        advance(MIN_REFRESH_INTERVAL).await;
        assert_eq!(driver.attempts.load(Ordering::SeqCst), 1);
        advance(RETRY_MIN - Duration::from_secs(1)).await;
        assert_eq!(driver.attempts.load(Ordering::SeqCst), 1);
        advance(Duration::from_secs(1)).await;
        assert_eq!(driver.attempts.load(Ordering::SeqCst), 2);
        advance(Duration::from_secs(60)).await;
        assert_eq!(driver.attempts.load(Ordering::SeqCst), 3);

        advance(MIN_REFRESH_INTERVAL).await;
        assert_eq!(driver.attempts.load(Ordering::SeqCst), 4);
        advance(RETRY_MIN).await;
        assert_eq!(driver.attempts.load(Ordering::SeqCst), 5);

        shutdown_tx.send(true).unwrap();
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn terminal_failure_invalidates_the_target() {
        let driver = FakeDriver::new(session("dead", 0), [Reply::Err(SyncError::SessionExpired)]);
        let target = FakeTarget::new(Arc::clone(&driver));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(maintain(
            Arc::clone(&target),
            FixedWallClock(0),
            shutdown_rx,
        ));
        tokio::task::yield_now().await;

        advance(MIN_REFRESH_INTERVAL).await;
        assert!(target.driver().is_none());
        assert_eq!(
            *target
                .last_error
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
            Some("the session expired; sign in again")
        );

        shutdown_tx.send(true).unwrap();
        task.await.unwrap();
    }

    #[tokio::test(start_paused = true)]
    async fn shutdown_cancels_an_in_flight_refresh() {
        let driver = FakeDriver::new(session("refresh-1", 0), [Reply::Pending]);
        let target = FakeTarget::new(Arc::clone(&driver));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let task = tokio::spawn(maintain(target, FixedWallClock(0), shutdown_rx));
        tokio::task::yield_now().await;

        advance(MIN_REFRESH_INTERVAL).await;
        assert_eq!(driver.attempts.load(Ordering::SeqCst), 1);
        shutdown_tx.send(true).unwrap();
        task.await.unwrap();
    }
}
