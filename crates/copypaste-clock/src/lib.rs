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

fn unix_millis(time: SystemTime) -> i64 {
    time.duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn conversion_is_deterministic_and_clamps_pre_epoch_time() {
        assert_eq!(unix_millis(UNIX_EPOCH), 0);
        assert_eq!(
            unix_millis(UNIX_EPOCH + Duration::from_millis(1_234)),
            1_234
        );
        assert_eq!(unix_millis(UNIX_EPOCH - Duration::from_millis(1)), 0);
    }
}
