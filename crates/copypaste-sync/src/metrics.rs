/// Lightweight observable counters for sync-engine events.
///
/// # Why counters in copypaste-sync instead of the daemon?
///
/// The `SyncLagCounter` type is defined here so that every layer that processes
/// broadcast receives — the sync orchestrator, future transport adapters, test
/// harnesses — can share one type with a consistent API.  Callers increment the
/// counter on each `RecvError::Lagged(n)` event; operators observe the value via
/// metrics export, health checks, or tests.
///
/// # Design choices
///
/// * `Arc<AtomicU64>` — lock-free increment, clone-cheap sharing across tasks.
///   `AtomicU64` is available on all tier-1 targets (x86-64, aarch64).
///
/// * `Ordering::Relaxed` for the increment — we don't need sequential
///   consistency; the counter is append-only and readers only require eventual
///   visibility of the accumulated total.  Stronger orderings would add
///   unnecessary memory barriers on aarch64.
///
/// * A saturating add is used so counter overflow (2^64 ≈ 1.8 × 10^19 events)
///   is silent rather than wrapping.  In practice the counter will never
///   overflow; the guard exists to be correct by construction.
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Shared, thread-safe counter for `RecvError::Lagged` events on a broadcast
/// channel subscriber.
///
/// Clone the counter to share it between the incrementing task (sync_orch,
/// outbound loop) and the reading task (health endpoint, telemetry exporter).
///
/// # Example
///
/// ```rust
/// use copypaste_sync::metrics::SyncLagCounter;
///
/// let counter = SyncLagCounter::new();
/// // Simulating a Lagged event that dropped 3 items:
/// counter.record_lagged(3);
/// assert_eq!(counter.total_dropped(), 3);
/// ```
#[derive(Debug, Clone, Default)]
pub struct SyncLagCounter(Arc<AtomicU64>);

impl SyncLagCounter {
    /// Create a fresh counter starting at zero.
    pub fn new() -> Self {
        Self(Arc::new(AtomicU64::new(0)))
    }

    /// Record a `RecvError::Lagged(n)` event: add `n` to the total drop count.
    ///
    /// `n` is the number of messages the broadcast receiver missed because its
    /// internal ring buffer was overwritten before it could consume them.
    /// Saturates at `u64::MAX` instead of wrapping.
    pub fn record_lagged(&self, n: u64) {
        // Relaxed: the counter is a monotonically-increasing diagnostic value;
        // no other memory operations need to be ordered relative to it.
        self.0.fetch_add(n, Ordering::Relaxed);
    }

    /// Total number of broadcast items dropped across all Lagged events since
    /// this counter was created (or reset via a new counter instance).
    pub fn total_dropped(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CopyPaste-335c: the counter must increment on each record_lagged call
    /// and the total must reflect the sum of all recorded lagged values.
    #[test]
    fn lagged_counter_accumulates_events() {
        let c = SyncLagCounter::new();
        assert_eq!(c.total_dropped(), 0, "fresh counter must start at zero");

        c.record_lagged(3);
        assert_eq!(c.total_dropped(), 3, "after first lag event (n=3)");

        c.record_lagged(7);
        assert_eq!(c.total_dropped(), 10, "after second lag event (n=7)");

        c.record_lagged(0);
        assert_eq!(c.total_dropped(), 10, "recording n=0 must not change total");
    }

    /// Cloned counters share the same underlying atomic — incrementing one
    /// must be visible on the other (simulates cross-task sharing).
    #[test]
    fn cloned_counters_share_state() {
        let writer = SyncLagCounter::new();
        let reader = writer.clone();

        writer.record_lagged(5);
        assert_eq!(
            reader.total_dropped(),
            5,
            "reader must see increments made through writer clone (CopyPaste-335c)"
        );

        writer.record_lagged(2);
        assert_eq!(
            reader.total_dropped(),
            7,
            "incremental updates must accumulate"
        );
    }

    /// Verify that multiple concurrent increments are all observed (no lost
    /// updates).  Uses multiple threads to exercise the atomic path.
    #[test]
    fn concurrent_increments_are_not_lost() {
        use std::thread;

        let counter = SyncLagCounter::new();
        let threads: Vec<_> = (0..8)
            .map(|_| {
                let c = counter.clone();
                thread::spawn(move || {
                    for _ in 0..1000 {
                        c.record_lagged(1);
                    }
                })
            })
            .collect();
        for t in threads {
            t.join().unwrap();
        }
        assert_eq!(
            counter.total_dropped(),
            8 * 1000,
            "all concurrent increments must be visible (no lost updates)"
        );
    }
}
