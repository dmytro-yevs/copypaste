#![forbid(unsafe_code)]

use std::time::{SystemTime, UNIX_EPOCH};

/// Wall time for persisted Unix-millisecond stamps.
///
/// It may move backwards. Deadlines and elapsed time must use a monotonic clock.
pub trait WallClock: Send + Sync {
    fn now_ms(&self) -> i64;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemWallClock;

impl WallClock for SystemWallClock {
    fn now_ms(&self) -> i64 {
        unix_millis(SystemTime::now())
    }
}

/// [`SystemWallClock::now_ms`] without a value to hold, for the callers that
/// only ever read the real clock.
#[must_use]
pub fn now_ms() -> i64 {
    SystemWallClock.now_ms()
}

/// Saturating in both directions.
///
/// A clock before the epoch reads as `0`, which loses every comparison, and
/// losing is the safe direction for expiry, discovery and pairing deadlines. A
/// clock past the wire integer limit reads as [`i64::MAX`] rather than wrapping
/// to a negative stamp, which would read as *older than everything* and invert
/// every one of those comparisons.
#[must_use]
pub fn unix_millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(saturating_millis)
        .unwrap_or(0)
}

fn saturating_millis(duration: std::time::Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn conversion_is_deterministic_and_saturates_at_both_ends() {
        assert_eq!(unix_millis(UNIX_EPOCH), 0);
        assert_eq!(
            unix_millis(UNIX_EPOCH + Duration::from_millis(1_234)),
            1_234
        );
        assert_eq!(unix_millis(UNIX_EPOCH - Duration::from_millis(1)), 0);
        // Wrapping here would produce a negative stamp, which sorts as older
        // than every real item rather than newer.
        // Windows FILETIME cannot represent the enormous SystemTime needed to
        // reach this branch, so assert the conversion before constructing it.
        let overflow = Duration::from_millis(i64::MAX as u64 + 1);
        assert_eq!(saturating_millis(overflow), i64::MAX);
    }
}
