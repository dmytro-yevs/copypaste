//! Realtime only wakes the ordinary encrypted push/pull driver.
use crate::Driver;
use backon::BackoffBuilder;
use copypaste_cloud::RealtimeSubscription;
use std::{sync::Arc, time::Duration};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

pub async fn run(driver: Arc<Driver>, wake: Arc<Notify>, cancel: CancellationToken) {
    let policy = copypaste_retry::stream_reconnect_backoff();
    let mut schedule = policy.build();
    loop {
        let token =
            driver.inspect_session(|session| zeroize::Zeroizing::new(session.access_token.clone()));
        let connected = tokio::select! {
            biased;
            _ = cancel.cancelled() => break,
            result = RealtimeSubscription::connect(driver.config(), &token) => result,
        };
        if let Ok(mut subscription) = connected {
            schedule = policy.build();
            driver.note_push_channel(true);
            wake.notify_one();
            let mut revision = driver.session_revision();
            loop {
                let next = driver.session_revision();
                if next != revision {
                    driver.inspect_session(|session| {
                        subscription.set_access_token(&session.access_token)
                    });
                    revision = next;
                }
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => {
                        driver.note_push_channel(false);
                        subscription.close().await;
                        return;
                    }
                    event = subscription.next_event() => {
                        match event {
                            Some(Ok(_)) => { driver.note_push_channel(true); wake.notify_one(); }
                            Some(Err(_)) => { driver.note_push_channel(false); wake.notify_one(); }
                            None => break,
                        }
                    }
                    _ = tokio::time::sleep(Duration::from_secs(1)) => {
                        let next = driver.session_revision();
                        if next != revision {
                            driver.inspect_session(|session| subscription.set_access_token(&session.access_token));
                            revision = next;
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
        tokio::select! { biased; _ = cancel.cancelled() => break, _ = tokio::time::sleep(delay) => {} }
    }
    driver.note_push_channel(false);
}
