//! When the retention bound runs, as distinct from what it deletes.
//!
//! The limits themselves live in `storage::retention`. This module owns only
//! the schedule, because "after every write" is wrong for the two callers that
//! write in bursts: a peer sync round and a history import each ran the same
//! `O(history)` sweep pair once per item and left the identical history.
//!
//! Sweeping later can only ever delete *less, later*, which is the safe
//! direction under AGENTS.md rule 4. Skipping a sweep is not, so both seams
//! here guarantee one runs: [`RetentionGate`] falls back to an inline sweep
//! when no reactor can carry the trailing one, and [`RetentionBatch`] sweeps on
//! drop, so an early return or a `?` still leaves the limits enforced.

use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use tracing::warn;

use crate::storage::Store;

/// How long applied versions may coalesce onto one retention sweep.
pub(crate) const RETENTION_DEBOUNCE: Duration = Duration::from_millis(250);

/// Apply every local retention limit after a write, regardless of whether it
/// arrived from capture, import, cloud, or a paired device.
///
/// Best-effort: failing a cleanup must not turn a successfully stored capture
/// or remote merge into data loss.
pub fn enforce_retention(store: &Store, settings: &copypaste_ipc::ConfigData) {
    #[cfg(test)]
    sweeps::record();

    if let Err(e) = store.evict_over_cap(u64::from(settings.history_limit)) {
        warn!(error = ?e, "history cap eviction failed");
    }
    if let Err(e) = store.evict_over_byte_cap(settings.storage_quota_bytes) {
        warn!(error = ?e, "storage quota eviction failed");
    }
    // Age-based retention, disabled by the `0` sentinel.
    //
    // Measured from the wall clock, never from `created_at`. `created_at` is
    // caller-supplied and, on the import path, comes straight out of a user's
    // JSON file: one row stamped a year ahead put the cutoff a year ahead too
    // and hard-deleted every unpinned item in the history. Both sync transports
    // already refuse an implausibly-future stamp; import is the third writer and
    // inherits neither guard, so this is where it holds.
    if settings.retention_days > 0 {
        let cutoff = crate::now_ms() - i64::from(settings.retention_days) * 86_400_000;
        if let Err(e) = store.evict_older_than(cutoff) {
            warn!(error = ?e, "age-based retention failed");
        }
    }
}

/// Coalesces the sweeps of a burst of *independent* writes onto one run.
///
/// Leading edge inline, so a lone write still returns with the limits enforced;
/// the rest of the burst rides a trailing run the caller schedules. Used by the
/// peer sync source, where items arrive one merge at a time and the session may
/// end by error or disconnect rather than by agreement.
///
/// A batch whose extent the caller already knows wants [`RetentionBatch`]
/// instead — it sweeps once, not once per debounce window.
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
/// Holding this is what lets [`crate::ingest::ingest_into_batched`] skip the
/// per-item sweep: the token proves a sweep is already owed, and `Drop` is what
/// pays it however the batch exits. A 10 000-item import ran 30 000 sweep
/// transactions for the history one sweep leaves.
///
/// The final history is the same either way. Every limit here is a function of
/// the rows that exist when it runs, not of the order they arrived in, and the
/// one rule that could have depended on order — the newest unpinned row is
/// never evicted — protects the newest row of the finished batch rather than of
/// each item in turn.
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

    #[test]
    fn the_gate_lets_one_caller_through_per_window() {
        let gate = RetentionGate::default();
        assert!(gate.claim(), "the first caller owns the window");
        assert!(!gate.claim(), "a second caller in the same window does not");
    }
}
