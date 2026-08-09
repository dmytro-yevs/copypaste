//! Change detection — written once, used by every backend.
//!
//! This file is the point of the whole module. Port manifest 01 §6.7 found that
//! v1's monitor called `NSPasteboard.generalPasteboard` directly, so roughly two
//! thirds of the invariants in §2 could not be tested at all — which is
//! precisely why several of them regressed. Everything here is pure state: a
//! change-count cursor and one atomic cell. Both backends run *this* code, so
//! I-2, I-3, I-4, §3.2 and the whole §3.3 sentinel protocol are verifiable by
//! ordinary `cargo test` on a host with no window server.
//!
//! Nothing in this file reads a representation, or knows what a representation
//! is. It answers one question: *should the caller read the clipboard at all?*

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

/// `changeCount` delta at which a burst is reported (§4).
///
/// Kept at 3 although a paste-back is now known to move the count by 1 rather
/// than 2, so 2 no longer names anything in particular. It is a *telemetry*
/// threshold only — crossing it never changes what is captured (§3.2) — and
/// lowering it would report a burst for two ordinary copies in one poll window.
const BURST_THRESHOLD: i64 = 3;

/// "No pending self-write", and the initial change-count cursor (§4).
///
/// Must be outside the valid `changeCount` domain, which is non-negative and
/// monotonically increasing. The shared value also gives I-2 for free: the first
/// poll after startup cannot be mistaken for a burst.
const COUNT_NONE: i64 = -1;

/// A self-write moves `changeCount` by exactly this much (§4).
///
/// **Measured, not predicted.** The manifest recovered 2 from v1 —
/// `clearContents` (+1) then `set…:forType:` (+1) — and macos-14 moves by 1:
/// the write lands in the generation `clearContents` opened. Arming the
/// sentinel at `pre + 2` therefore missed our own paste-back, which came back
/// as a fresh capture, and left the cell armed one count ahead where the next
/// genuine copy was silently swallowed (run 30632553103).
///
/// Nothing in the write path predicts this any more — [`SelfWriteSentinel::arm`]
/// takes the count `clearContents` returns. It survives as the fake's model of
/// the OS and as the burst arithmetic below.
pub(super) const SELF_WRITE_DELTA: i64 = 1;

/// What a `changeCount` observation means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Change {
    /// Nothing moved. The caller MUST NOT read any representation (I-1).
    Unchanged,
    /// Our own write, coming back to us. Acknowledged and dropped (§3.3 step 6).
    SelfWrite,
    /// A genuine change. `lost_intermediates` is telemetry only — the caller
    /// falls through and captures the current value regardless (§3.2).
    Fresh { lost_intermediates: u64 },
}

/// The self-write sentinel of §3.3.
///
/// One atomic cell with acquire/release ordering, deliberately behind an `Arc`
/// and deliberately a *single* primitive: the manifest requires that sync
/// auto-apply and relay auto-apply reuse this same cell rather than growing
/// their own (CopyPaste-7ub), so remotely-synced items written to the pasteboard
/// are not re-captured as local ones. Clone it to share it; do not copy the
/// protocol.
#[derive(Debug, Clone)]
pub(super) struct SelfWriteSentinel(Arc<AtomicI64>);

impl SelfWriteSentinel {
    fn new() -> Self {
        Self(Arc::new(AtomicI64::new(COUNT_NONE)))
    }

    /// Step 2 — arm at the count the write will carry, before the content is
    /// visible. The caller passes what `clearContents` returned.
    ///
    /// * v1 stamped after the write, and a poll in that window recorded the item
    ///   we had just pasted as a capture (Fix-4 / "DUP-ON-COPY").
    /// * An armed count *above* the write swallows whichever genuine copy lands
    ///   there. One at or below it can never match again — `consume_if` compares
    ///   for equality and `changeCount` only rises — so it is inert. That
    ///   asymmetry is why this takes an observed count, not `pre + delta`.
    pub(super) fn arm(&self, count: i64) {
        self.0.store(count, Ordering::Release);
    }

    /// Step 5 — any write failure resets the cell.
    ///
    /// A stale sentinel that is never consumed permanently suppresses whichever
    /// future genuine capture happens to land on that change count.
    pub(super) fn clear(&self) {
        self.0.store(COUNT_NONE, Ordering::Release);
    }

    /// Step 6 — the suppression is for exactly one change.
    fn consume_if(&self, count: i64) -> bool {
        count >= 0
            && self
                .0
                .compare_exchange(count, COUNT_NONE, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }
}

/// The change-count cursor plus the sentinel: everything a backend needs to
/// decide *whether* to read the clipboard, and nothing about reading it.
#[derive(Debug)]
pub(super) struct ChangeTracker {
    /// I-2: starts at a sentinel distinguishable from every valid change count.
    cursor: i64,
    pub(super) sentinel: SelfWriteSentinel,
    pub(super) lost_intermediates: u64,
}

impl ChangeTracker {
    pub(super) fn new() -> Self {
        Self {
            cursor: COUNT_NONE,
            sentinel: SelfWriteSentinel::new(),
            lost_intermediates: 0,
        }
    }

    /// Whether this reading is the one already reported, so a caller can skip
    /// the work `observe` would do. Says nothing about *what* a change is —
    /// classifying it, and consuming the sentinel, stays in `observe`.
    pub(super) fn is_current(&self, count: i64) -> bool {
        count == self.cursor
    }

    /// Classify a `changeCount` reading.
    ///
    /// Every branch that returns something other than `Fresh` has already
    /// advanced the cursor (I-3): skipping without advancing re-offers the same
    /// change forever, and advancing without capturing is the intended
    /// "acknowledge and drop".
    pub(super) fn observe(&mut self, count: i64) -> Change {
        // I-1: the comparison is the first thing the poll does, so an unchanged
        // clipboard costs one field read and zero allocations.
        if count == self.cursor {
            return Change::Unchanged;
        }

        // §3.3 step 6: consume the sentinel exactly once.
        if self.sentinel.consume_if(count) {
            self.cursor = count;
            return Change::SelfWrite;
        }

        // I-4: the delta comes from the *old* cursor value, before advancing.
        // I-2: a cursor still at the sentinel is startup, not a burst.
        let lost = if self.cursor < 0 {
            0
        } else {
            let delta = count - self.cursor;
            if delta >= BURST_THRESHOLD {
                // The current value survives; only the ones in between are lost.
                (delta - 1) as u64
            } else {
                0
            }
        };
        self.cursor = count;
        self.lost_intermediates += lost;

        // §3.2: fall through to capture. Never return "a burst happened"
        // *instead of* the content — that bug permanently lost the most recent
        // item every time a user copied three things quickly.
        Change::Fresh {
            lost_intermediates: lost,
        }
    }
}

// Tests — the invariants that used to be untestable
//
// This is the state machine the macOS backend runs; testing it here is what
// makes I-2, I-3, I-4, §3.2 and the whole §3.3 sentinel protocol verifiable on a
// host with no window server. v1 could not write any of these: it had no seam,
// so its "contract tests" re-stated the rule in a comment and asserted something
// trivially true (§6.7).

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_count_is_never_re_offered() {
        let mut t = ChangeTracker::new();
        assert!(matches!(t.observe(7), Change::Fresh { .. }));
        assert_eq!(t.observe(7), Change::Unchanged);
        assert_eq!(t.observe(7), Change::Unchanged);
    }

    /// I-2 / T-2 — a first poll at an arbitrary change count is not a burst.
    #[test]
    fn first_observation_is_not_a_burst() {
        let mut t = ChangeTracker::new();
        assert_eq!(
            t.observe(4321),
            Change::Fresh {
                lost_intermediates: 0
            }
        );
        assert_eq!(t.lost_intermediates, 0);
    }

    /// T-7 — the threshold boundary. Delta 2 is a paste-back pair, not a burst.
    #[test]
    fn burst_threshold_boundary() {
        let mut t = ChangeTracker::new();
        t.observe(10);
        assert_eq!(
            t.observe(12),
            Change::Fresh {
                lost_intermediates: 0
            },
            "a delta of 2 is the clear+set pair, not a burst"
        );
        assert_eq!(
            t.observe(15),
            Change::Fresh {
                lost_intermediates: 2
            },
            "a delta of 3 loses the two values in between"
        );
    }

    /// I-4 — the delta is computed from the old cursor, and the cursor advances
    /// afterwards.
    #[test]
    fn cursor_advances_after_the_delta_is_computed() {
        let mut t = ChangeTracker::new();
        t.observe(100);
        assert_eq!(
            t.observe(110),
            Change::Fresh {
                lost_intermediates: 9
            }
        );
        assert_eq!(t.cursor, 110);
        assert_eq!(t.observe(110), Change::Unchanged);
    }

    /// T-8 / T-9 — the sentinel suppresses exactly one change and then a
    /// genuine copy is captured. I-3 — the drop path advanced the cursor.
    #[test]
    fn self_write_sentinel_suppresses_exactly_one_change() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        let landed = 10 + SELF_WRITE_DELTA;
        t.sentinel.arm(landed); // step 2

        assert_eq!(t.observe(landed), Change::SelfWrite);
        assert_eq!(
            t.observe(landed),
            Change::Unchanged,
            "cursor advanced (I-3)"
        );
        assert!(matches!(t.observe(landed + 1), Change::Fresh { .. }));
    }

    /// T-11 / CopyPaste-8yzf — a third-party app writing during ours must not
    /// have its content suppressed as if it were ours.
    ///
    /// The armed count is the one *our* write landed in, so every later change
    /// is someone else's and cannot match. Repairing the cell to the observed
    /// count is what would drop their copy, which is why nothing does.
    #[test]
    fn third_party_write_during_ours_is_still_captured() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        let ours = 10 + SELF_WRITE_DELTA;
        t.sentinel.arm(ours);

        assert_eq!(t.observe(ours), Change::SelfWrite);
        assert!(matches!(t.observe(ours + 1), Change::Fresh { .. }));
        assert!(matches!(t.observe(ours + 2), Change::Fresh { .. }));
    }

    /// The asymmetry [`SelfWriteSentinel::arm`] is built on: a cell armed above
    /// the write swallows the genuine copy that lands there, and that is the
    /// defect run 30632553103 found. One armed at or below it is inert.
    #[test]
    fn an_arm_above_the_write_would_swallow_a_genuine_copy() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        t.sentinel.arm(12); // what `pre + 2` used to store
        assert!(
            matches!(t.observe(11), Change::Fresh { .. }),
            "our own paste-back came back as a capture"
        );
        assert_eq!(
            t.observe(12),
            Change::SelfWrite,
            "and the next genuine copy was swallowed"
        );
    }

    /// T-12 — a failed write clears the sentinel, so it cannot suppress some
    /// unrelated future capture that happens to land on that change count.
    #[test]
    fn failed_write_clears_the_sentinel() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        t.sentinel.arm(10);
        t.sentinel.clear(); // step 5

        assert!(matches!(t.observe(12), Change::Fresh { .. }));
    }

    /// The sentinel is one shared primitive (§3.3, CopyPaste-7ub): sync
    /// auto-apply arms the same cell the poller consumes, rather than growing a
    /// second copy of the protocol.
    #[test]
    fn sentinel_is_shared_not_duplicated() {
        let mut t = ChangeTracker::new();
        t.observe(10);

        let landed = 10 + SELF_WRITE_DELTA;
        let elsewhere = t.sentinel.clone();
        elsewhere.arm(landed);

        assert_eq!(t.observe(landed), Change::SelfWrite);
        // Consumed once, by whoever polls first.
        assert!(!elsewhere.consume_if(landed));
    }
}
