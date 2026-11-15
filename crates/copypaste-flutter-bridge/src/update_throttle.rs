//! Rate limiting for progress updates sent to Dart.

use std::time::Instant;

/// Passes at most one progress update to Dart per
/// [`copypaste_core::transfer::PROGRESS_INTERVAL`], remembering the latest.
#[derive(Default)]
pub(crate) struct UpdateThrottle {
    sent: Option<Instant>,
    /// The latest observed processed and total units.
    pub(crate) last: (u64, u64),
}

impl UpdateThrottle {
    /// Record an update and say whether it should be passed on.
    pub(crate) fn observe(&mut self, processed: u64, total: u64) -> bool {
        self.last = (processed, total);
        let now = Instant::now();
        if self.sent.is_some_and(|sent| {
            now.duration_since(sent) < copypaste_core::transfer::PROGRESS_INTERVAL
        }) {
            return false;
        }
        self.sent = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_burst_passes_its_first_update_and_remembers_the_latest() {
        let mut throttle = UpdateThrottle::default();
        assert!(throttle.observe(1, 10));
        assert!(!throttle.observe(5, 10));
        assert!(!throttle.observe(10, 10));
        assert_eq!(throttle.last, (10, 10));
    }
}
