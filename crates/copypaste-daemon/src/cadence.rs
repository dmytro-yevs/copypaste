//! How long a background sync loop waits when nothing is happening.
//!
//! The rule — start at the floor, double while idle, snap back to the floor on
//! any activity — is `copypaste_cloud::sync::cadence`'s, and its bounds are
//! imported from there rather than re-chosen here. What could not be imported
//! is the state: the cloud crate keeps it in a private field of `CloudSync`, so
//! there is no type to reuse, only a shape.
//!
//! **This should become one implementation.** `copypaste-cloud` belongs to
//! another owner right now; the intended end state is that `CloudSync` holds an
//! [`Idle`] and this file is the only place the doubling is written (CLAUDE.md
//! rule 1).

use std::sync::Mutex;
use std::time::Duration;

pub use copypaste_cloud::sync::{MAX_POLL_INTERVAL, MIN_POLL_INTERVAL};

/// An idle interval that grows while nothing happens.
#[derive(Debug)]
pub struct Idle {
    current: Mutex<Duration>,
}

impl Default for Idle {
    fn default() -> Self {
        Self {
            current: Mutex::new(MIN_POLL_INTERVAL),
        }
    }
}

impl Idle {
    /// How long to wait before the next round.
    pub fn interval(&self) -> Duration {
        *self.lock()
    }

    /// Record what a round did. Any change resets to the floor; a round that
    /// moved nothing doubles the wait, up to the ceiling.
    pub fn note_activity(&self, changed: bool) {
        let mut current = self.lock();
        *current = if changed {
            MIN_POLL_INTERVAL
        } else {
            (*current * 2).min(MAX_POLL_INTERVAL)
        };
    }

    /// Something happened that the loop has not seen yet — a local capture, a
    /// push from the other side. Tighten up rather than wait out whatever
    /// interval the loop had drifted to.
    pub fn reset(&self) {
        *self.lock() = MIN_POLL_INTERVAL;
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Duration> {
        self.current
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interval_grows_while_idle_and_is_bounded() {
        let idle = Idle::default();
        assert_eq!(idle.interval(), MIN_POLL_INTERVAL);

        let mut previous = idle.interval();
        for _ in 0..20 {
            idle.note_activity(false);
            let now = idle.interval();
            assert!(now >= previous, "the interval went backwards");
            assert!(now <= MAX_POLL_INTERVAL, "past the ceiling: {now:?}");
            previous = now;
        }
        assert_eq!(previous, MAX_POLL_INTERVAL, "never reached the ceiling");
    }

    #[test]
    fn activity_and_an_explicit_wake_both_return_to_the_floor() {
        let idle = Idle::default();
        for _ in 0..8 {
            idle.note_activity(false);
        }
        assert!(idle.interval() > MIN_POLL_INTERVAL);
        idle.note_activity(true);
        assert_eq!(idle.interval(), MIN_POLL_INTERVAL);

        for _ in 0..8 {
            idle.note_activity(false);
        }
        idle.reset();
        assert_eq!(idle.interval(), MIN_POLL_INTERVAL);
    }
}
