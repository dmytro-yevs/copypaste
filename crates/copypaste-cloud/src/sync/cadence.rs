//! The idle poll interval: how fast the correctness backstop ticks when
//! nothing is happening, and what pulls it back to the floor.
//!
//! Separate from [`super::retry`] on purpose. That file is about recovering one
//! failed request; this one is about the cost of asking at all. They move in
//! opposite directions — a failure shortens nothing, a quiet hour lengthens
//! everything — and merging them is how a backoff ends up applied to the wrong
//! condition.

use std::sync::atomic::Ordering;
use std::time::Duration;

use super::driver::CloudSync;
use super::transport::{AuthApi, RestApi};

/// Idle poll interval floor: the cadence right after something changed.
pub const MIN_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Idle poll interval ceiling **while the push channel is confirmed**.
///
/// A device whose clipboard nobody has touched should not wake the radio every
/// few seconds — on a phone that is measurable battery, and it is the reason
/// v1's cadence grew from five seconds toward five minutes rather than staying
/// fixed. Five minutes is only defensible because [`crate::realtime`] is
/// carrying the latency: the poll is the correctness backstop, and a backstop
/// can be slow *when there is something in front of it*.
///
/// Manifest 05 §4.8 puts this at 60 s rather than 300 s. The deviation is
/// recorded there, and it rests on two things v1 did not have: a `Resubscribed`
/// event that forces a round the moment a dropped channel comes back, so the
/// window this interval bounds is "an event the server never sent" rather than
/// "every event since the socket died", and the fallback below.
pub const MAX_POLL_INTERVAL: Duration = Duration::from_secs(300);

/// Idle poll interval ceiling **while it is not**.
///
/// Manifest 05 §4.8's number, unchanged: with no push channel the poll is the
/// sole download path, and the ceiling above would make the worst-case latency
/// between two idle devices the sum of two five-minute waits. A subscriber that
/// never calls [`CloudSync::note_push_channel`] gets this, which is the safe
/// direction for a caller that has not wired Realtime up yet.
pub const MAX_POLL_INTERVAL_WITHOUT_PUSH: Duration = Duration::from_secs(10);

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// How long to wait before the next [`CloudSync::sync`].
    ///
    /// Starts at [`MIN_POLL_INTERVAL`] and doubles toward the ceiling while
    /// nothing is happening; any change on either side resets it to the floor.
    /// The ceiling is whichever of [`MAX_POLL_INTERVAL`] and
    /// [`MAX_POLL_INTERVAL_WITHOUT_PUSH`] the push channel's state selects, and
    /// it is applied on read as well as on drift, so a channel that just died
    /// shortens the *current* wait rather than the one after it.
    pub fn poll_interval(&self) -> Duration {
        (*self.lock(&self.idle)).min(self.idle_ceiling())
    }

    /// Reset the cadence to the floor.
    ///
    /// Call this when [`crate::realtime`] delivers an event: something is
    /// happening, so the backstop should tighten up rather than wait out
    /// whatever interval it had drifted to. A caller whose poll loop is already
    /// asleep must also interrupt that sleep — resetting the interval a loop is
    /// not reading changes nothing.
    pub fn wake(&self) {
        *self.lock(&self.idle) = MIN_POLL_INTERVAL;
    }

    /// Tell the driver whether a Realtime channel is currently confirmed.
    ///
    /// `true` on a confirmed *join*, never on a socket that is merely open — a
    /// socket whose channel never joined delivers nothing, and slowing the poll
    /// down for it halves the sync rate for no reason (manifest 05 §4.8). Pair
    /// it with `false` on disconnect, or the ceiling keeps promising a latency
    /// mechanism that is gone.
    pub fn note_push_channel(&self, live: bool) {
        self.push_channel.store(live, Ordering::Relaxed);
    }

    /// Whether a push channel was last reported live.
    pub fn push_channel_live(&self) -> bool {
        self.push_channel.load(Ordering::Relaxed)
    }

    fn idle_ceiling(&self) -> Duration {
        if self.push_channel_live() {
            MAX_POLL_INTERVAL
        } else {
            MAX_POLL_INTERVAL_WITHOUT_PUSH
        }
    }

    pub(super) fn note_activity(&self, changed: bool) {
        let ceiling = self.idle_ceiling();
        let mut idle = self.lock(&self.idle);
        *idle = if changed {
            MIN_POLL_INTERVAL
        } else {
            (*idle * 2).min(ceiling)
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
        sync.note_push_channel(true);

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

    /// The premise of the five-minute ceiling is that something else is
    /// carrying the latency. Without a push channel it is not, and two idle
    /// devices would be ten minutes apart end to end.
    #[tokio::test]
    async fn without_a_push_channel_the_ceiling_is_the_short_one() {
        let source = FakeSource::default();
        let sync = driver(FakeRest::default(), FakeAuth::default());

        for _ in 0..20 {
            sync.sync(&source).await.unwrap();
        }
        assert!(
            !sync.push_channel_live(),
            "the safe default is `no channel`"
        );
        assert_eq!(sync.poll_interval(), MAX_POLL_INTERVAL_WITHOUT_PUSH);
    }

    /// A channel that dies must shorten the wait the loop is about to take, not
    /// the one after it: the loop reads `poll_interval` once per round.
    #[tokio::test]
    async fn losing_the_channel_shortens_the_current_wait() {
        let source = FakeSource::default();
        let sync = driver(FakeRest::default(), FakeAuth::default());
        sync.note_push_channel(true);

        for _ in 0..20 {
            sync.sync(&source).await.unwrap();
        }
        assert_eq!(sync.poll_interval(), MAX_POLL_INTERVAL);

        sync.note_push_channel(false);
        assert_eq!(sync.poll_interval(), MAX_POLL_INTERVAL_WITHOUT_PUSH);
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
