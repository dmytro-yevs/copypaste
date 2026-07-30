//! The idle poll interval: how fast the correctness backstop ticks when
//! nothing is happening, and what pulls it back to the floor.
//!
//! Separate from [`super::retry`] on purpose. That file is about recovering one
//! failed request; this one is about the cost of asking at all. They move in
//! opposite directions — a failure shortens nothing, a quiet hour lengthens
//! everything — and merging them is how a backoff ends up applied to the wrong
//! condition.

use std::time::Duration;

use super::driver::CloudSync;
use super::transport::{AuthApi, RestApi};

/// Idle poll interval floor: the cadence right after something changed.
pub const MIN_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Idle poll interval ceiling.
///
/// A device whose clipboard nobody has touched should not wake the radio every
/// few seconds — on a phone that is measurable battery, and it is the reason
/// v1's cadence grew from five seconds toward five minutes rather than staying
/// fixed. Five minutes is the ceiling because [`crate::realtime`] is the thing
/// that makes latency short when anything is actually happening; the poll is
/// the correctness backstop, and a backstop can be slow.
pub const MAX_POLL_INTERVAL: Duration = Duration::from_secs(300);

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// How long to wait before the next [`CloudSync::sync`].
    ///
    /// Starts at [`MIN_POLL_INTERVAL`] and doubles toward
    /// [`MAX_POLL_INTERVAL`] while nothing is happening; any change on either
    /// side resets it to the floor. The poll is a correctness backstop, not the
    /// latency mechanism — [`crate::realtime`] is that — so a slow idle cadence
    /// costs nothing but is worth real battery on a phone.
    pub fn poll_interval(&self) -> Duration {
        *self.lock(&self.idle)
    }

    /// Reset the cadence to the floor.
    ///
    /// Call this when [`crate::realtime`] delivers an event: something is
    /// happening, so the backstop should tighten up rather than wait out
    /// whatever interval it had drifted to.
    pub fn wake(&self) {
        *self.lock(&self.idle) = MIN_POLL_INTERVAL;
    }

    pub(super) fn note_activity(&self, changed: bool) {
        let mut idle = self.lock(&self.idle);
        *idle = if changed {
            MIN_POLL_INTERVAL
        } else {
            (*idle * 2).min(MAX_POLL_INTERVAL)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::super::fakes::{cloud_row, driver, FakeAuth, FakeRest, FakeSource};
    use super::*;

    #[tokio::test]
    async fn the_idle_interval_grows_and_is_bounded() {
        let source = FakeSource::default();
        let sync = driver(FakeRest::default(), FakeAuth::default());

        assert_eq!(sync.poll_interval(), MIN_POLL_INTERVAL);

        let mut previous = sync.poll_interval();
        for _ in 0..20 {
            sync.sync(&source).await.unwrap();
            let now = sync.poll_interval();
            assert!(now >= previous, "the idle interval went backwards");
            assert!(
                now <= MAX_POLL_INTERVAL,
                "the idle interval passed its ceiling: {now:?}"
            );
            previous = now;
        }
        assert_eq!(
            previous, MAX_POLL_INTERVAL,
            "the idle interval never reached its ceiling"
        );
    }

    #[tokio::test]
    async fn any_change_resets_the_idle_interval() {
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("a", 1_000, "x")]),
            FakeAuth::default(),
        );

        // Idle for a while.
        for _ in 0..5 {
            sync.sync(&source).await.unwrap();
        }
        // The seeded row applies on the first sync, so drift only starts after
        // it; force some drift, then deliver a change.
        let drifted = sync.poll_interval();
        sync.rest
            .rows
            .lock()
            .unwrap()
            .insert("b".into(), cloud_row("b", 9_000, "new"));

        let stats = sync.sync(&source).await.unwrap();
        assert!(stats.changed());
        assert_eq!(sync.poll_interval(), MIN_POLL_INTERVAL);
        assert!(drifted > MIN_POLL_INTERVAL, "the interval never drifted");
    }

    #[tokio::test]
    async fn a_realtime_event_can_wake_the_poll_loop() {
        let source = FakeSource::default();
        let sync = driver(FakeRest::default(), FakeAuth::default());

        for _ in 0..4 {
            sync.sync(&source).await.unwrap();
        }
        assert!(sync.poll_interval() > MIN_POLL_INTERVAL);

        sync.wake();
        assert_eq!(sync.poll_interval(), MIN_POLL_INTERVAL);
    }
}
