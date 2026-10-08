//! Shared adaptive sync cadence.
use std::{
    sync::{Mutex, MutexGuard},
    time::Duration,
};
/// Idle poll interval floor: the cadence right after something changed.
pub const MIN_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Long idle ceiling only while the provider's push subscription is confirmed.
/// Reconnection wakes an immediate round; polling remains the correctness backstop.
pub const MAX_POLL_INTERVAL: Duration = Duration::from_secs(300);

/// Idle poll interval ceiling **while it is not**.
///
/// Manifest 05 §4.8's number, unchanged: with no push channel the poll is the
/// sole download path, and the ceiling above would make the worst-case latency
/// between two idle devices the sum of two five-minute waits. A subscriber that
/// never calls push readiness gets this, which is the safe
/// direction for a caller that has not wired Realtime up yet.
pub const MAX_POLL_INTERVAL_WITHOUT_PUSH: Duration = Duration::from_secs(10);

/// A round-driven interval that lengthens while idle and resets on activity.
#[derive(Debug)]
pub struct AdaptiveCadence {
    floor: Duration,
    ceiling: Duration,
    current: Mutex<Duration>,
}

impl Default for AdaptiveCadence {
    fn default() -> Self {
        Self::new(MIN_POLL_INTERVAL, MAX_POLL_INTERVAL)
    }
}

impl AdaptiveCadence {
    /// Start a cadence at `floor` and cap idle growth at `ceiling`.
    pub fn new(floor: Duration, ceiling: Duration) -> Self {
        assert!(
            floor <= ceiling,
            "cadence floor must not exceed its ceiling"
        );
        Self {
            floor,
            ceiling,
            current: Mutex::new(floor),
        }
    }

    /// Return the interval selected for the next round.
    pub fn interval(&self) -> Duration {
        *self.lock()
    }

    /// Record whether the last round observed activity.
    pub fn note_activity(&self, changed: bool) {
        self.note_activity_with_ceiling(changed, self.ceiling);
    }

    pub fn interval_with_ceiling(&self, ceiling: Duration) -> Duration {
        self.interval().min(ceiling)
    }

    pub fn note_activity_with_ceiling(&self, changed: bool, ceiling: Duration) {
        assert!(
            self.floor <= ceiling,
            "cadence floor must not exceed its ceiling"
        );
        let mut current = self.lock();
        *current = if changed {
            self.floor
        } else {
            current.saturating_mul(2).min(self.ceiling).min(ceiling)
        };
    }

    /// Return the cadence to its floor immediately.
    pub fn reset(&self) {
        *self.lock() = self.floor;
    }

    fn lock(&self) -> MutexGuard<'_, Duration> {
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}
