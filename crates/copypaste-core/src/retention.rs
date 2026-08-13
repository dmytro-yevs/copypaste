//! When the retention bound runs, as distinct from what it deletes.
//!
//! The limits live in `storage::retention`. This module owns the schedule:
//! [`RetentionGate`] debounces bursts, [`RetentionBatch`] defers to one
//! end-of-batch sweep. Sweeping later deletes *less, later* (safe direction);
//! skipping is not, so both guarantee a sweep runs.

use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use tracing::warn;

use crate::storage::Store;

/// How long applied versions may coalesce onto one retention sweep.
pub(crate) const RETENTION_DEBOUNCE: Duration = Duration::from_millis(250);

/// Apply every local retention limit after a write. Best-effort: a cleanup
/// failure must not turn a stored capture into data loss.
pub fn enforce_retention(store: &Store, settings: &copypaste_ipc::ConfigData) {
    #[cfg(test)]
    sweeps::record();

    if let Err(e) = store.evict_over_cap(u64::from(settings.history_limit)) {
        warn!(error = ?e, "history cap eviction failed");
    }
    if let Err(e) = store.evict_over_byte_cap(settings.storage_quota_bytes) {
        warn!(error = ?e, "storage quota eviction failed");
    }
    // Measured from the wall clock: `created_at` is caller-supplied, and one
    // row stamped a year ahead wiped the whole history.
    if settings.retention_days > 0 {
        let cutoff = crate::now_ms() - i64::from(settings.retention_days) * 86_400_000;
        if let Err(e) = store.evict_older_than(cutoff) {
            warn!(error = ?e, "age-based retention failed");
        }
    }
}

/// Coalesces the sweeps of a burst of independent writes onto one run.
/// Used by the sync source, where items arrive one merge at a time.
#[derive(Default)]
pub struct RetentionGate {
    last_run: Mutex<Option<Instant>>,
    pub(crate) trailing_scheduled: AtomicBool,
}

impl RetentionGate {
    /// True when this caller owns the sweep for the current window.
    pub(crate) fn claim(&self) -> bool {
        let mut last = self.last_run.lock().unwrap_or_else(PoisonError::into_inner);
        if last.is_none_or(|at| at.elapsed() >= RETENTION_DEBOUNCE) {
            *last = Some(Instant::now());
            true
        } else {
            false
        }
    }

    pub(crate) fn stamp(&self) {
        *self.last_run.lock().unwrap_or_else(PoisonError::into_inner) = Some(Instant::now());
    }
}

/// One sweep for a whole batch of writes, run when the batch ends.
///
/// The token proves a sweep is owed; `Drop` pays it. Callers may [`disarm`]
/// when no rows were written or pin restoration failed (ADR-0023).
pub struct RetentionBatch<'a> {
    store: &'a Store,
    settings: &'a copypaste_ipc::ConfigData,
    swept: bool,
}

impl<'a> RetentionBatch<'a> {
    #[must_use]
    pub fn new(store: &'a Store, settings: &'a copypaste_ipc::ConfigData) -> Self {
        Self {
            store,
            settings,
            swept: false,
        }
    }

    pub fn store(&self) -> &Store {
        self.store
    }

    pub fn settings(&self) -> &copypaste_ipc::ConfigData {
        self.settings
    }

    /// DMY-156: prevent the sweep when no rows were written or pins failed.
    pub fn disarm(&mut self) {
        self.swept = true;
    }

    /// Sweep now rather than on drop, for a caller that wants the limits
    /// enforced before it reports success.
    pub fn finish(mut self) {
        self.sweep();
    }

    fn sweep(&mut self) {
        if std::mem::replace(&mut self.swept, true) {
            return;
        }
        enforce_retention(self.store, self.settings);
    }
}

impl Drop for RetentionBatch<'_> {
    fn drop(&mut self) {
        // DMY-156 B2: DB work in a destructor during unwinding can
        // double-panic and abort the process.
        if std::thread::panicking() {
            return;
        }
        self.sweep();
    }
}

/// Counts the sweeps run on this thread, so a test can prove a batch sweeps
/// once rather than once per item.
///
/// Thread-local rather than global: `cargo test` runs test functions in
/// parallel and a shared counter would make the assertion depend on whatever
/// else was running.
#[cfg(test)]
pub(crate) mod sweeps {
    use std::cell::Cell;

    thread_local! {
        static COUNT: Cell<u32> = const { Cell::new(0) };
    }

    pub(crate) fn record() {
        COUNT.with(|n| n.set(n.get() + 1));
    }

    /// Sweeps since the last call, resetting the count.
    pub(crate) fn take() -> u32 {
        COUNT.with(|n| n.replace(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::test_support::{item, store};

    const T0: i64 = 1_700_000_000_000;

    fn settings() -> copypaste_ipc::ConfigData {
        copypaste_ipc::ConfigData::default()
    }

    #[test]
    fn a_batch_sweeps_once_when_it_ends_not_once_per_write() {
        let s = store();
        let settings = settings();

        sweeps::take();
        {
            let _batch = RetentionBatch::new(&s, &settings);
            for n in 0..50 {
                s.insert(item(&format!("item-{n}"), T0 + n * 60_000))
                    .unwrap();
            }
            assert_eq!(sweeps::take(), 0, "a batch must not sweep while it is open");
        }
        assert_eq!(sweeps::take(), 1, "the sweep must run when the batch ends");
    }

    /// `finish` is for a caller that wants the limits enforced before it
    /// reports success; dropping the same batch again must not sweep twice.
    #[test]
    fn finishing_a_batch_sweeps_exactly_once() {
        let s = store();
        let settings = settings();

        sweeps::take();
        RetentionBatch::new(&s, &settings).finish();
        assert_eq!(sweeps::take(), 1);
    }

    /// The sweep is owed however the batch exits — a `?` in the middle of an
    /// import must still leave the history bounded.
    #[test]
    fn an_early_return_still_pays_the_sweep() {
        let s = store();
        let settings = settings();

        sweeps::take();
        let bail = |store: &Store| -> Result<(), ()> {
            let _batch = RetentionBatch::new(store, &settings);
            store.insert(item("only", T0)).unwrap();
            Err(())
        };
        assert!(bail(&s).is_err());
        assert_eq!(sweeps::take(), 1);
    }

    /// DMY-156 B1: a batch that never received a write can be disarmed, and
    /// its sweep never runs — so a zero-write import cannot evict existing
    /// history under a lowered cap.
    #[test]
    fn a_disarmed_batch_never_sweeps() {
        let s = store();
        let settings = settings();

        sweeps::take();
        let mut batch = RetentionBatch::new(&s, &settings);
        batch.disarm();
        batch.finish();
        assert_eq!(sweeps::take(), 0, "a disarmed batch must not sweep");
    }

    /// DMY-156 B2: a panic during the batch must not run the sweep, because
    /// pin restoration has not happened. DB work in a destructor during
    /// unwind can double-panic and abort the process.
    #[test]
    fn a_panic_during_a_batch_does_not_sweep() {
        let s = store();
        let settings = settings();

        sweeps::take();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _batch = RetentionBatch::new(&s, &settings);
            s.insert(item("before the panic", T0)).unwrap();
            panic!("simulated failure");
        }));
        assert!(result.is_err());
        assert_eq!(
            sweeps::take(),
            0,
            "a panic must not trigger a sweep in the destructor"
        );
    }

    #[test]
    fn the_gate_lets_one_caller_through_per_window() {
        let gate = RetentionGate::default();
        assert!(gate.claim(), "the first caller owns the window");
        assert!(!gate.claim(), "a second caller in the same window does not");
    }
}
