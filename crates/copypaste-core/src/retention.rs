//! When the retention bound runs, as distinct from what it deletes.
//!
//! The limits live in `storage::retention`. This module owns the schedule:
//! [`RetentionGate`] debounces bursts, [`RetentionBatch`] defers to one
//! end-of-batch sweep. Sweeping later deletes *less, later* (safe direction),
//! so a batch that ends any other way still sweeps.

use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use scopeguard::{guard_on_success, OnSuccess, ScopeGuard};
use tracing::warn;

use crate::storage::Store;

/// How long applied versions may coalesce onto one retention sweep.
pub(crate) const RETENTION_DEBOUNCE: Duration = Duration::from_millis(250);

pub fn policy_tightened(
    before: &copypaste_ipc::ConfigData,
    after: &copypaste_ipc::ConfigData,
) -> bool {
    after.storage_quota_bytes < before.storage_quota_bytes
        || after.history_limit < before.history_limit
        || (after.retention_days > 0
            && (before.retention_days == 0 || after.retention_days < before.retention_days))
}

pub fn enforce_retention_report(store: &Store, settings: &copypaste_ipc::ConfigData) -> u64 {
    #[cfg(test)]
    sweeps::record();

    let mut removed = 0;
    match store.evict_over_cap(u64::from(settings.history_limit)) {
        Ok(count) => removed += count,
        Err(e) => warn!(error = ?e, "history cap eviction failed"),
    }
    match store.evict_over_byte_cap(settings.storage_quota_bytes) {
        Ok(count) => removed += count,
        Err(e) => warn!(error = ?e, "storage quota eviction failed"),
    }
    // Measured from the wall clock: `created_at` is caller-supplied, and one
    // row stamped a year ahead wiped the whole history.
    if settings.retention_days > 0 {
        let cutoff = crate::now_ms() - i64::from(settings.retention_days) * 86_400_000;
        match store.evict_older_than(cutoff) {
            Ok(count) => removed += count,
            Err(e) => warn!(error = ?e, "age-based retention failed"),
        }
    }
    removed
}

pub fn enforce_retention(store: &Store, settings: &copypaste_ipc::ConfigData) {
    let _ = enforce_retention_report(store, settings);
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

/// The store and settings one batch will sweep.
///
/// Reached only through the batch that owns it, so a caller cannot sweep a
/// store it did not write to (ADR-0023).
pub struct BatchScope<'a> {
    store: &'a Store,
    settings: &'a copypaste_ipc::ConfigData,
}

impl<'a> BatchScope<'a> {
    pub fn store(&self) -> &'a Store {
        self.store
    }

    pub fn settings(&self) -> &'a copypaste_ipc::ConfigData {
        self.settings
    }
}

/// One sweep for a whole batch of writes, run when the batch ends.
///
/// `OnSuccess`, not `Always`: a sweep reached by unwinding would run database
/// work in a destructor, which can double-panic into an abort, and the caller's
/// pin restoration has not run either (DMY-156).
pub type RetentionBatch<'a> = ScopeGuard<BatchScope<'a>, fn(BatchScope<'a>), OnSuccess>;

/// Opens a batch. The sweep runs when it drops, unless [`finish`] or
/// [`disarm`] resolves it first.
#[must_use]
pub fn batch<'a>(store: &'a Store, settings: &'a copypaste_ipc::ConfigData) -> RetentionBatch<'a> {
    guard_on_success(BatchScope { store, settings }, sweep as fn(BatchScope<'a>))
}

fn sweep(scope: BatchScope<'_>) {
    enforce_retention(scope.store, scope.settings);
}

/// Sweep now rather than on drop, for a caller that wants the limits enforced
/// before it reports success.
pub fn finish(batch: RetentionBatch<'_>) {
    sweep(ScopeGuard::into_inner(batch));
}

/// Retire a batch without sweeping, for a caller that has established no sweep
/// is owed. Getting this wrong deletes history the user still has
/// (DMY-156), so the predicate belongs at the call site, not here.
pub fn disarm(batch: RetentionBatch<'_>) {
    let _ = ScopeGuard::into_inner(batch);
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
    fn a_tighter_policy_is_the_only_settings_change_that_needs_a_sweep() {
        let before = copypaste_ipc::ConfigData {
            retention_days: 3,
            ..settings()
        };

        let mut after = before.clone();
        after.poll_interval_ms = 250;
        assert!(!policy_tightened(&before, &after));

        after = before.clone();
        after.history_limit -= 1;
        assert!(policy_tightened(&before, &after));

        after = before.clone();
        after.storage_quota_bytes -= 1;
        assert!(policy_tightened(&before, &after));

        after = before.clone();
        after.retention_days = before.retention_days - 1;
        assert!(policy_tightened(&before, &after));

        after = before.clone();
        after.retention_days = 0;
        assert!(!policy_tightened(&before, &after));

        let disabled = copypaste_ipc::ConfigData {
            retention_days: 0,
            ..before.clone()
        };
        assert!(policy_tightened(&disabled, &before));
    }

    #[test]
    fn a_report_counts_only_real_removals_and_keeps_pins_and_the_newest() {
        let store = store();
        let pinned = store.insert(item("pinned", T0)).unwrap();
        let removed = store.insert(item("removable", T0 + 1)).unwrap();
        let newest = store.insert(item("newest", T0 + 2)).unwrap();
        store.set_pinned(&pinned.id, true).unwrap();
        let settings = copypaste_ipc::ConfigData {
            history_limit: 1,
            ..settings()
        };

        assert_eq!(enforce_retention_report(&store, &settings), 1);
        assert_eq!(store.count().unwrap(), 2);
        assert!(store.get(&pinned.id).unwrap().is_some());
        assert!(store.get(&newest.id).unwrap().is_some());
        assert!(store.get(&removed.id).unwrap().is_none());
        assert_eq!(enforce_retention_report(&store, &settings), 0);
    }

    #[test]
    fn a_batch_sweeps_once_when_it_ends_not_once_per_write() {
        let s = store();
        let settings = settings();

        sweeps::take();
        {
            let _batch = batch(&s, &settings);
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
        finish(batch(&s, &settings));
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
            let _batch = batch(store, &settings);
            store.insert(item("only", T0)).unwrap();
            Err(())
        };
        assert!(bail(&s).is_err());
        assert_eq!(sweeps::take(), 1);
    }

    /// DMY-156: a batch that owes no sweep can be disarmed, and then nothing
    /// runs one — so an import that wrote no rows cannot evict existing history
    /// under a lowered cap.
    #[test]
    fn a_disarmed_batch_never_sweeps() {
        let s = store();
        let settings = settings();

        sweeps::take();
        disarm(batch(&s, &settings));
        assert_eq!(sweeps::take(), 0, "a disarmed batch must not sweep");
    }

    /// DMY-156: a panic during the batch must not run the sweep. The caller's
    /// pin restoration has not happened, and database work in a destructor
    /// during unwind can double-panic and abort the process.
    #[test]
    fn a_panic_during_a_batch_does_not_sweep() {
        let s = store();
        let settings = settings();

        sweeps::take();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _batch = batch(&s, &settings);
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

    /// The batch must sweep the store it was opened on, not one a caller passed
    /// separately — that is what makes the token a proof rather than a flag.
    #[test]
    fn a_batch_sweeps_the_store_it_was_opened_on() {
        let s = store();
        let settings = settings();

        let open = batch(&s, &settings);
        s.insert(item("only", T0)).unwrap();
        assert_eq!(open.store().count().unwrap(), 1);
        assert_eq!(open.settings().history_limit, settings.history_limit);
        finish(open);
    }

    #[test]
    fn the_gate_lets_one_caller_through_per_window() {
        let gate = RetentionGate::default();
        assert!(gate.claim(), "the first caller owns the window");
        assert!(!gate.claim(), "a second caller in the same window does not");
    }
}
