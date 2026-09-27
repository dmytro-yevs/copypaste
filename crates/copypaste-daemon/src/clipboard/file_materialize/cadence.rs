//! When the next sweep runs.

use std::time::Duration;

use backon::{BackoffBuilder, ExponentialBuilder};

use super::report::SweepReport;

/// The first retry after a sweep that left work behind. `interval` is the whole
/// `MAX_AGE`, so waiting it out doubles the exposure of anything still on disk.
/// The ladder climbs back to `interval`, so a file nothing can ever delete
/// costs a short burst of wake-ups rather than a permanent 30-second timer.
pub(super) const RETRY_MIN: Duration = Duration::from_secs(30);

/// Never more than a quarter of the ordinary interval: a "retry" scheduled at
/// the interval is not a retry, and the sweeper is constructed with short
/// intervals under test.
pub(super) fn retry_floor(interval: Duration) -> Duration {
    RETRY_MIN.min(interval / 4)
}

/// One schedule, from the workspace's only retry crate; the worker's condvar
/// stays the sole timer.
pub(super) struct SweepCadence {
    interval: Duration,
    policy: ExponentialBuilder,
    schedule: <ExponentialBuilder as BackoffBuilder>::Backoff,
}

impl SweepCadence {
    pub(super) fn new(interval: Duration) -> Self {
        let policy = ExponentialBuilder::new()
            .with_min_delay(retry_floor(interval))
            .with_max_delay(interval)
            .without_max_times();
        Self {
            interval,
            policy,
            schedule: policy.build(),
        }
    }

    pub(super) fn after(&mut self, report: &SweepReport) -> Duration {
        if !report.unfinished() {
            self.schedule = self.policy.build();
            return self.interval;
        }
        self.schedule
            .next()
            .unwrap_or(self.interval)
            .min(self.interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unfinished_sweep_is_retried_long_before_the_next_interval() {
        let interval = Duration::from_secs(600);
        let mut cadence = SweepCadence::new(interval);
        let clean = SweepReport::default();
        let blocked = SweepReport {
            retained: 1,
            ..SweepReport::default()
        };

        assert_eq!(cadence.after(&clean), interval);
        let first = cadence.after(&blocked);
        assert_eq!(first, RETRY_MIN);

        let mut previous = first;
        for _ in 0..12 {
            let next = cadence.after(&blocked);
            assert!(next >= previous, "the ladder went backwards");
            assert!(next <= interval, "a retry outran the ordinary interval");
            previous = next;
        }
        assert_eq!(previous, interval, "the ladder must settle at the interval");

        assert_eq!(cadence.after(&clean), interval);
        assert_eq!(
            cadence.after(&blocked),
            RETRY_MIN,
            "a clean sweep must reset the ladder"
        );
    }

    #[test]
    fn a_short_interval_never_produces_a_longer_retry() {
        let interval = Duration::from_millis(10);
        let mut cadence = SweepCadence::new(interval);
        let blocked = SweepReport {
            unreadable: 1,
            ..SweepReport::default()
        };

        for _ in 0..5 {
            let next = cadence.after(&blocked);
            assert!(next <= interval);
            assert!(next >= retry_floor(interval));
        }
    }
}
