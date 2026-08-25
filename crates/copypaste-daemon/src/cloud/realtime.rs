//! The push half of cloud sync: a Supabase Realtime subscription whose only job
//! is to wake the poll loop.
//!
//! `copypaste_cloud::realtime` was complete, tested, and instantiated nowhere.
//! That is not a missed optimisation. `copypaste_cloud::sync::cadence` justifies
//! its five-minute idle ceiling by saying realtime is "the thing that makes
//! latency short when anything is actually happening" — so shipping the ceiling
//! without the push channel left the poll interval defended by an argument that
//! was not true of the build we ship.
//!
//! # This loop moves no data
//!
//! An event is a signal, never a source: it calls [`Cloud::wake`] and the
//! ordinary round in [`super::poll`] does the reading, decrypting and merging.
//! Realtime is at-most-once — events that happen while the socket is down are
//! never replayed — so anything that treated an event as the delivery mechanism
//! would lose rows on every reconnect. [`RealtimeEvent::Resubscribed`] is the
//! moment that is *known* to have happened, and it wakes a round for exactly
//! that reason.
//!
//! # The token
//!
//! A subscription re-joins with the token it was created with, so one captured
//! at sign-in and held forever silently re-joins with a dead JWT. Every connect
//! here reads the driver's *current* access token, and the subscription is
//! dropped and rebuilt whenever the socket ends.

use std::sync::Arc;
use std::time::Duration;

use backon::BackoffBuilder;
use copypaste_cloud::{RealtimeError, RealtimeEvent, RealtimeSubscription};
use copypaste_retry::stream_reconnect_backoff;
use tokio::sync::watch;
use tracing::{debug, info};

use crate::cloud::{Cloud, Driver};
use crate::AppState;

#[derive(Debug, Eq, PartialEq)]
enum ReconnectWait {
    Retry,
    SessionChanged,
    Shutdown,
}

async fn wait_to_reconnect(
    delay: Duration,
    session_updates: &mut watch::Receiver<u64>,
    shutdown: &mut watch::Receiver<bool>,
) -> ReconnectWait {
    tokio::select! {
        biased;
        _ = shutdown.changed() => ReconnectWait::Shutdown,
        changed = session_updates.changed() => {
            if changed.is_ok() {
                ReconnectWait::SessionChanged
            } else {
                ReconnectWait::Shutdown
            }
        }
        _ = tokio::time::sleep(delay) => ReconnectWait::Retry,
    }
}

/// Hold a realtime subscription for as long as an account is signed in.
pub async fn run(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    if !state.cloud.is_configured() {
        debug!("no cloud deployment configured; realtime is idle");
        return;
    }

    let policy = stream_reconnect_backoff();
    let mut schedule = policy.build();
    let mut session_updates = state.cloud.session_updates();
    loop {
        if *shutdown.borrow() {
            break;
        }

        let subscription = tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            changed = session_updates.changed() => {
                if changed.is_err() {
                    break;
                }
                schedule = policy.build();
                continue;
            }
            subscription = subscribe(&state) => subscription,
        };

        if let Some((subscription, driver, user_id)) = subscription {
            schedule = policy.build();
            // Returns when the socket ended for good, or on shutdown.
            match pump(
                &state,
                subscription,
                driver,
                &mut session_updates,
                &user_id,
                &mut shutdown,
            )
            .await
            {
                PumpExit::Shutdown => break,
                PumpExit::SessionChanged => continue,
                PumpExit::Disconnected => {}
            }
        }

        let Some(delay) = schedule.next() else {
            break;
        };
        match wait_to_reconnect(delay, &mut session_updates, &mut shutdown).await {
            ReconnectWait::Retry => {}
            ReconnectWait::SessionChanged => schedule = policy.build(),
            ReconnectWait::Shutdown => break,
        }
    }

    debug!("cloud realtime loop stopped");
}

/// Open one subscription for the account that is signed in right now.
async fn subscribe(state: &Arc<AppState>) -> Option<(RealtimeSubscription, Arc<Driver>, String)> {
    let config = state.cloud.config()?.clone();
    let driver = state.cloud.driver()?;
    // Read at connect time, never cached across a reconnect.
    let (user_id, token) =
        driver.inspect_session(|session| (session.user_id.clone(), session.access_token.clone()));

    match RealtimeSubscription::connect(&config, &token).await {
        Ok(subscription) => {
            info!("cloud realtime subscribed");
            Some((subscription, driver, user_id))
        }
        Err(e) => {
            // `RealtimeError`'s payloads are `&'static str` by construction, so
            // this cannot carry the socket URL, the token or a frame body.
            debug!(error = %e, "could not subscribe to cloud realtime");
            None
        }
    }
}

trait Subscription {
    async fn next_event(&mut self) -> Option<Result<RealtimeEvent, RealtimeError>>;
    fn set_access_token(&self, access_token: &str);
    async fn close(self);
}

impl Subscription for RealtimeSubscription {
    async fn next_event(&mut self) -> Option<Result<RealtimeEvent, RealtimeError>> {
        RealtimeSubscription::next_event(self).await
    }

    fn set_access_token(&self, access_token: &str) {
        RealtimeSubscription::set_access_token(self, access_token);
    }

    async fn close(self) {
        RealtimeSubscription::close(self).await;
    }
}

struct PushChannel<'a> {
    cloud: &'a Cloud,
    driver: Arc<Driver>,
    live: bool,
}

#[derive(Debug, Eq, PartialEq)]
enum PumpExit {
    Disconnected,
    SessionChanged,
    Shutdown,
}

impl<'a> PushChannel<'a> {
    fn confirmed(cloud: &'a Cloud, driver: Arc<Driver>) -> Self {
        let live = driver.push_channel_live();
        let mut channel = Self {
            cloud,
            driver,
            live,
        };
        channel.set_live(true);
        channel
    }

    fn set_live(&mut self, live: bool) -> bool {
        if self.live == live {
            return false;
        }
        self.driver.note_push_channel(live);
        self.live = live;
        self.cloud.wake();
        true
    }
}

impl Drop for PushChannel<'_> {
    fn drop(&mut self) {
        self.set_live(false);
    }
}

/// Turn events into wakes until the subscription ends.
async fn pump<S: Subscription>(
    state: &Arc<AppState>,
    mut subscription: S,
    driver: Arc<Driver>,
    session_updates: &mut watch::Receiver<u64>,
    user_id: &str,
    shutdown: &mut watch::Receiver<bool>,
) -> PumpExit {
    let mut channel = PushChannel::confirmed(&state.cloud, Arc::clone(&driver));
    let exit = loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break PumpExit::Shutdown,
            changed = session_updates.changed() => {
                if changed.is_err() {
                    break PumpExit::Shutdown;
                }
                let Some(driver) = state.cloud.driver() else {
                    break PumpExit::SessionChanged;
                };
                if !Arc::ptr_eq(&driver, &channel.driver) {
                    break PumpExit::SessionChanged;
                }
                let (current_user_id, access_token) = driver.inspect_session(|session| {
                    (session.user_id.clone(), session.access_token.clone())
                });
                if current_user_id != user_id {
                    break PumpExit::SessionChanged;
                }
                subscription.set_access_token(&access_token);
            }
            event = subscription.next_event() => match event {
                // Terminal: the task gave up. The outer loop rebuilds.
                None => break PumpExit::Disconnected,
                Some(Ok(RealtimeEvent::Resubscribed)) => {
                    // The one moment at-most-once is known to have bitten.
                    debug!("cloud realtime re-joined; forcing a round");
                    if !channel.set_live(true) {
                        state.cloud.wake();
                    }
                }
                Some(Ok(_)) => state.cloud.wake(),
                // Informational: the subscription keeps reconnecting itself.
                Some(Err(e)) => {
                    channel.set_live(false);
                    debug!(error = %e, "cloud realtime reported a failure");
                }
            },
        }
    };
    channel.set_live(false);
    subscription.close().await;
    exit
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    use copypaste_cloud::auth::Session;
    use copypaste_cloud::sync::{MAX_POLL_INTERVAL, MAX_POLL_INTERVAL_WITHOUT_PUSH};
    use copypaste_cloud::{CloudConfig, SyncKey};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::mpsc;

    use crate::cloud::source::StoreSource;
    use crate::cloud::Cloud;
    use crate::testutil::{test_state, test_state_with_cloud};

    #[tokio::test]
    async fn shutdown_interrupts_the_longest_outer_reconnect_wait() {
        let (_session_tx, mut session_updates) = watch::channel(0_u64);
        let (shutdown_tx, mut shutdown) = watch::channel(false);
        let task = tokio::spawn(async move {
            wait_to_reconnect(
                copypaste_retry::STREAM_RECONNECT_MAX,
                &mut session_updates,
                &mut shutdown,
            )
            .await
        });
        tokio::task::yield_now().await;

        shutdown_tx.send(true).unwrap();
        let outcome = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("shutdown left the reconnect sleep running")
            .expect("wait task panicked");
        assert_eq!(outcome, ReconnectWait::Shutdown);
    }

    struct EmptyBackend {
        config: CloudConfig,
        task: tokio::task::JoinHandle<()>,
    }

    impl EmptyBackend {
        async fn start() -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
                .await
                .expect("bind backend");
            let config = CloudConfig::new_loopback(
                format!("http://{}", listener.local_addr().expect("backend address")),
                "anon",
            )
            .unwrap();
            let task = tokio::spawn(async move {
                loop {
                    let Ok((mut socket, _)) = listener.accept().await else {
                        return;
                    };
                    let mut request = Vec::new();
                    while let Ok(read) = socket.read_buf(&mut request).await {
                        if read == 0 || request.windows(4).any(|bytes| bytes == b"\r\n\r\n") {
                            break;
                        }
                    }
                    let _ = socket
                        .write_all(
                            b"HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: 2\r\nconnection: close\r\n\r\n[]",
                        )
                        .await;
                }
            });
            Self { config, task }
        }
    }

    impl Drop for EmptyBackend {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    struct FakeSubscription {
        events: mpsc::UnboundedReceiver<Result<RealtimeEvent, RealtimeError>>,
        closed: Arc<AtomicBool>,
    }

    impl Subscription for FakeSubscription {
        async fn next_event(&mut self) -> Option<Result<RealtimeEvent, RealtimeError>> {
            self.events.recv().await
        }

        fn set_access_token(&self, _access_token: &str) {}

        async fn close(self) {
            self.closed.store(true, Ordering::Release);
        }
    }

    async fn configured_state(name: &str) -> (Arc<AppState>, tempfile::TempDir, EmptyBackend) {
        let backend = EmptyBackend::start().await;
        let (state, dir) = test_state_with_cloud(name, Cloud::new(Some(backend.config.clone())));
        install_account(&state, backend.config.clone(), "access-1");
        (state, dir, backend)
    }

    fn install_account(state: &Arc<AppState>, config: CloudConfig, access_token: &str) {
        state.cloud.install(
            state,
            config,
            "a@example.com".into(),
            "user-1".into(),
            SyncKey::from_bytes([7; 32]),
            Session {
                access_token: access_token.into(),
                refresh_token: "refresh-1".into(),
                user_id: "user-1".into(),
                expires_at_ms: i64::MAX,
            },
        );
    }

    fn fake_subscription() -> (
        FakeSubscription,
        mpsc::UnboundedSender<Result<RealtimeEvent, RealtimeError>>,
        Arc<AtomicBool>,
    ) {
        let (events_tx, events) = mpsc::unbounded_channel();
        let closed = Arc::new(AtomicBool::new(false));
        (
            FakeSubscription {
                events,
                closed: Arc::clone(&closed),
            },
            events_tx,
            closed,
        )
    }

    async fn expect_wake(state: &AppState) {
        tokio::time::timeout(Duration::from_secs(1), state.cloud.wake_signal())
            .await
            .expect("channel transition did not wake the poll loop");
    }

    async fn drift_to_long_ceiling(state: &Arc<AppState>, driver: &Driver) {
        let source = StoreSource::new(Arc::clone(state));
        for _ in 0..8 {
            driver.sync(&source).await.expect("idle cloud round");
        }
        assert_eq!(driver.poll_interval(), MAX_POLL_INTERVAL);
    }

    #[tokio::test]
    async fn an_unconfigured_daemon_does_not_start_a_subscription() {
        let (state, _dir) = test_state("alpha");
        let (_tx, rx) = watch::channel(false);
        tokio::time::timeout(Duration::from_secs(5), run(Arc::clone(&state), rx))
            .await
            .expect("must return immediately when there is no deployment");
    }

    /// Signed out is an ordinary state: the loop waits rather than erroring, and
    /// it still observes shutdown while waiting.
    #[tokio::test]
    async fn a_signed_out_daemon_waits_and_still_shuts_down() {
        let config = copypaste_cloud::CloudConfig::new("https://example.invalid", "anon").unwrap();
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config)));
        assert!(subscribe(&state).await.is_none());

        let (tx, rx) = watch::channel(false);
        let task = tokio::spawn(run(Arc::clone(&state), rx));
        tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .expect("the loop must observe shutdown")
            .expect("no panic");
    }

    #[tokio::test]
    async fn join_disconnect_and_reconnect_switch_the_ceiling_and_wake() {
        let (state, _dir, _backend) = configured_state("alpha").await;
        let driver = state.cloud.driver().expect("installed driver");
        let (subscription, events, closed) = fake_subscription();
        let mut session_updates = state.cloud.session_updates();
        let (shutdown_tx, mut shutdown) = watch::channel(false);
        let task_state = Arc::clone(&state);
        let task_driver = Arc::clone(&driver);
        let task = tokio::spawn(async move {
            pump(
                &task_state,
                subscription,
                task_driver,
                &mut session_updates,
                "user-1",
                &mut shutdown,
            )
            .await;
        });

        expect_wake(&state).await;
        assert!(driver.push_channel_live());
        drift_to_long_ceiling(&state, &driver).await;

        events
            .send(Err(RealtimeError::Connect("test disconnect")))
            .unwrap();
        expect_wake(&state).await;
        assert!(!driver.push_channel_live());
        assert_eq!(driver.poll_interval(), MAX_POLL_INTERVAL_WITHOUT_PUSH);

        events.send(Ok(RealtimeEvent::Resubscribed)).unwrap();
        expect_wake(&state).await;
        assert!(driver.push_channel_live());
        assert_eq!(driver.poll_interval(), MAX_POLL_INTERVAL);

        shutdown_tx.send(true).unwrap();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("pump did not stop")
            .expect("pump panicked");
        expect_wake(&state).await;
        assert!(!driver.push_channel_live());
        assert_eq!(driver.poll_interval(), MAX_POLL_INTERVAL_WITHOUT_PUSH);
        assert!(closed.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn replacing_an_account_clears_the_old_channel_and_wakes() {
        let (state, _dir, backend) = configured_state("alpha").await;
        let old_driver = state.cloud.driver().expect("installed driver");
        let (subscription, _events, closed) = fake_subscription();
        let mut session_updates = state.cloud.session_updates();
        let (_shutdown_tx, mut shutdown) = watch::channel(false);
        let task_state = Arc::clone(&state);
        let task_driver = Arc::clone(&old_driver);
        let task = tokio::spawn(async move {
            pump(
                &task_state,
                subscription,
                task_driver,
                &mut session_updates,
                "user-1",
                &mut shutdown,
            )
            .await;
        });

        expect_wake(&state).await;
        drift_to_long_ceiling(&state, &old_driver).await;
        install_account(&state, backend.config.clone(), "access-2");

        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("account transition did not stop the old pump")
            .expect("pump panicked");
        expect_wake(&state).await;
        assert!(!old_driver.push_channel_live());
        assert_eq!(old_driver.poll_interval(), MAX_POLL_INTERVAL_WITHOUT_PUSH);
        assert!(!state
            .cloud
            .driver()
            .expect("replacement driver")
            .push_channel_live());
        assert!(closed.load(Ordering::Acquire));
    }
}
