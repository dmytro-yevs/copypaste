//! Realtime only wakes the ordinary encrypted push/pull driver.
use crate::Driver;
use backon::BackoffBuilder;
use copypaste_cloud::RealtimeSubscription;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub async fn run(driver: Arc<Driver>, wake: Arc<Notify>, cancel: CancellationToken) {
    let policy = copypaste_retry::stream_reconnect_backoff();
    let mut schedule = policy.build();
    let mut session_events = driver.subscribe_session();
    'session: loop {
        if driver.session_is_fenced() {
            break;
        }
        let token =
            driver.inspect_session(|session| zeroize::Zeroizing::new(session.access_token.clone()));
        let connected = tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            changed = session_events.changed() => {
                if changed.is_err() { break; }
                continue;
            },
            result = RealtimeSubscription::connect_with_groups(driver.config(), &token) => result,
        };
        if let Ok(mut subscription) = connected {
            schedule = policy.build();
            driver.note_push_channel(true);
            wake.notify_one();
            loop {
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        driver.note_push_channel(false);
                        subscription.close().await;
                        return;
                    }
                    changed = session_events.changed() => {
                        if changed.is_err() || driver.session_is_fenced() {
                            subscription.close().await;
                            break 'session;
                        }
                        driver.inspect_session(|session| subscription.set_access_token(&session.access_token));
                    }
                    event = subscription.next_event() => {
                        match event {
                            Some(Ok(_)) => { driver.note_push_channel(true); wake.notify_one(); }
                            Some(Err(_)) => { driver.note_push_channel(false); wake.notify_one(); }
                            None => break,
                        }
                    }
                }
            }
            subscription.close().await;
        }
        driver.note_push_channel(false);
        wake.notify_one();
        let Some(delay) = schedule.next() else {
            break;
        };
        tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            changed = session_events.changed() => { if changed.is_err() { break; } },
            _ = tokio::time::sleep(delay) => {}
        }
    }
    driver.note_push_channel(false);
}
