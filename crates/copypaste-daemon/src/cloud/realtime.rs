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

use copypaste_cloud::{RealtimeEvent, RealtimeSubscription};
use tokio::sync::watch;
use tracing::{debug, info};

use crate::AppState;

/// How long to wait before rebuilding a subscription that ended or refused.
///
/// Bounded doubling rather than `backoff`'s full machinery: there is one
/// operation, one failure mode and no jitter requirement, and the subscription
/// does its own reconnects internally — this only covers the case where it gave
/// up entirely.
const RECONNECT_MIN: Duration = Duration::from_secs(5);
const RECONNECT_MAX: Duration = Duration::from_secs(300);

/// Hold a realtime subscription for as long as an account is signed in.
pub async fn run(state: Arc<AppState>, mut shutdown: watch::Receiver<bool>) {
    if !state.cloud.is_configured() {
        debug!("no cloud deployment configured; realtime is idle");
        return;
    }

    let mut backoff = RECONNECT_MIN;
    loop {
        if *shutdown.borrow() {
            break;
        }

        match subscribe(&state).await {
            Some(subscription) => {
                backoff = RECONNECT_MIN;
                // Returns when the socket ended for good, or on shutdown.
                pump(&state, subscription, &mut shutdown).await;
            }
            None => {
                // Not signed in, no token, or the join was refused. All three
                // recover the same way: wait and look again.
                tokio::select! {
                    biased;
                    _ = shutdown.changed() => break,
                    _ = tokio::time::sleep(backoff) => {}
                }
                backoff = (backoff * 2).min(RECONNECT_MAX);
            }
        }
    }

    debug!("cloud realtime loop stopped");
}

/// Open one subscription for the account that is signed in right now.
async fn subscribe(state: &Arc<AppState>) -> Option<RealtimeSubscription> {
    let config = state.cloud.config()?.clone();
    let driver = state.cloud.driver()?;
    // Read at connect time, never cached across a reconnect.
    let token = driver.inspect_session(|session| session.access_token.clone());

    match RealtimeSubscription::connect(&config, &token).await {
        Ok(subscription) => {
            info!("cloud realtime subscribed");
            Some(subscription)
        }
        Err(e) => {
            // `RealtimeError`'s payloads are `&'static str` by construction, so
            // this cannot carry the socket URL, the token or a frame body.
            debug!(error = %e, "could not subscribe to cloud realtime");
            None
        }
    }
}

/// Turn events into wakes until the subscription ends.
async fn pump(
    state: &Arc<AppState>,
    mut subscription: RealtimeSubscription,
    shutdown: &mut watch::Receiver<bool>,
) {
    loop {
        tokio::select! {
            biased;
            _ = shutdown.changed() => break,
            event = subscription.next_event() => match event {
                // Terminal: the task gave up. The outer loop rebuilds.
                None => break,
                Some(Ok(RealtimeEvent::Resubscribed)) => {
                    // The one moment at-most-once is known to have bitten.
                    debug!("cloud realtime re-joined; forcing a round");
                    state.cloud.wake();
                }
                Some(Ok(_)) => state.cloud.wake(),
                // Informational: the subscription keeps reconnecting itself.
                Some(Err(e)) => debug!(error = %e, "cloud realtime reported a failure"),
            },
        }
    }
    subscription.close().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{test_state, test_state_with_cloud};
    use crate::cloud::Cloud;

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
        let config = copypaste_cloud::CloudConfig {
            url: "https://example.invalid".into(),
            anon_key: "anon".into(),
        };
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
}
