//! Exponential backoff scheduler for transport reconnect loops.
//!
//! Designed to be used by a long-lived sync client (relay or P2P) that
//! repeatedly attempts to (re)establish a connection. The scheduler is a
//! pure state machine — it owns no I/O, no clock, no async. Callers feed
//! `on_failure()` / `on_success_held()` and ask for `next_delay()`.
//!
//! # Algorithm
//!
//! ```text
//! attempt 1 →  1 s
//! attempt 2 →  2 s
//! attempt 3 →  4 s
//! attempt 4 →  8 s
//! attempt 5 → 16 s
//! attempt N ≥ 6 → 30 s (cap)
//! ```
//!
//! After a connection holds for at least `success_hold_threshold` (default
//! 60 s), the next failure resets the schedule back to `1 s`. Callers signal
//! "the connection held long enough to be considered healthy" by calling
//! [`BackoffScheduler::on_success_held`].
//!
//! # Why a separate state machine?
//!
//! - **Testable.** No tokio, no real time — tests run in microseconds.
//! - **Reusable.** The relay client (in `copypaste-daemon`) and any future
//!   P2P reconnect loop share the same policy.
//! - **No new deps.** Hand-rolled with `Duration` arithmetic; we deliberately
//!   avoid pulling in `tokio-retry` / `backoff` crates to keep the dep tree
//!   minimal (see CLAUDE.md "prefer hand-rolling" rule).
//!
//! # Usage sketch
//!
//! ```rust,no_run
//! use copypaste_ipc::backoff::BackoffScheduler;
//! # fn try_connect() -> Result<(), std::io::Error> { Ok(()) }
//! # fn run_session() -> std::time::Duration { std::time::Duration::from_secs(0) }
//! // CopyPaste-crh3.53: this crate (copypaste-ipc) has no async runtime dep, so
//! // the sketch uses std primitives; real callers (relay/P2P/Supabase reconnect
//! // loops) drive the same API from their own tokio task + tracing.
//! let mut backoff = BackoffScheduler::default();
//! loop {
//!     match try_connect() {
//!         Ok(()) => {
//!             // connection established — drive the session
//!             let held = run_session();
//!             if held >= backoff.success_hold_threshold() {
//!                 backoff.on_success_held();
//!             }
//!         }
//!         Err(_e) => {
//!             let delay = backoff.next_delay();
//!             let _attempt = backoff.attempt();
//!             std::thread::sleep(delay);
//!             backoff.on_failure();
//!         }
//!     }
//! }
//! ```
//!
//! Note the order: `next_delay()` returns the delay *for the current*
//! attempt, then `on_failure()` advances the schedule for the *next* one.

use std::time::Duration;

/// Default base delay for the first retry attempt.
pub const DEFAULT_BASE_DELAY: Duration = Duration::from_secs(1);

/// Default maximum delay between retries (cap).
pub const DEFAULT_MAX_DELAY: Duration = Duration::from_secs(30);

/// Default minimum time a connection must hold to be considered healthy
/// enough to reset the backoff schedule.
pub const DEFAULT_SUCCESS_HOLD_THRESHOLD: Duration = Duration::from_secs(60);

/// Exponential backoff state machine.
///
/// See module docs for the algorithm. Construction defaults match the
/// constants above (1s base, 30s cap, 60s success-hold).
#[derive(Debug, Clone)]
pub struct BackoffScheduler {
    /// Next-attempt counter, 1-based. `1` means "about to make the first
    /// retry after a failure" — `next_delay()` returns the base delay.
    attempt: u32,
    base: Duration,
    cap: Duration,
    success_hold_threshold: Duration,
}

impl Default for BackoffScheduler {
    fn default() -> Self {
        Self::new(
            DEFAULT_BASE_DELAY,
            DEFAULT_MAX_DELAY,
            DEFAULT_SUCCESS_HOLD_THRESHOLD,
        )
    }
}

impl BackoffScheduler {
    /// Construct a scheduler with explicit base / cap / success-hold.
    ///
    /// Both `base` and `cap` must be non-zero; `cap` is clamped to be at
    /// least `base`. `success_hold_threshold` may be zero (= reset after
    /// any successful session), but production callers should keep it at
    /// 60 s or higher so a flapping peer doesn't keep resetting the
    /// schedule.
    pub fn new(base: Duration, cap: Duration, success_hold_threshold: Duration) -> Self {
        let base = if base.is_zero() {
            DEFAULT_BASE_DELAY
        } else {
            base
        };
        let cap = if cap < base { base } else { cap };
        Self {
            attempt: 1,
            base,
            cap,
            success_hold_threshold,
        }
    }

    /// Current attempt number (1-based). After `new()` or a reset this is
    /// `1`; each `on_failure()` increments it.
    pub fn attempt(&self) -> u32 {
        self.attempt
    }

    /// Threshold at which a successful connection is considered "held long
    /// enough" to warrant a reset.
    pub fn success_hold_threshold(&self) -> Duration {
        self.success_hold_threshold
    }

    /// Return the delay that should precede the *current* attempt.
    ///
    /// This is `base * 2^(attempt-1)`, saturated at `cap`. It does **not**
    /// advance the scheduler — call [`Self::on_failure`] afterwards to
    /// move to the next attempt.
    pub fn next_delay(&self) -> Duration {
        // Compute 2^(attempt - 1) as a multiplier without overflow:
        // - attempts in practice stop being meaningful past ~30 (cap kicks
        //   in around attempt 6 for the default base of 1s).
        // - we saturate the shift so a pathological caller can't crash us.
        let shift = self.attempt.saturating_sub(1).min(31);
        let multiplier: u64 = 1u64 << shift;
        let base_ns = self.base.as_nanos() as u64;
        // saturating_mul prevents overflow if a caller passes a huge base.
        let delay_ns = base_ns.saturating_mul(multiplier);
        let delay = Duration::from_nanos(delay_ns);
        if delay > self.cap {
            self.cap
        } else {
            delay
        }
    }

    /// Mark the previous attempt as failed. Advances `attempt` by one
    /// (saturating, so the counter cannot wrap).
    pub fn on_failure(&mut self) {
        self.attempt = self.attempt.saturating_add(1);
    }

    /// Mark that the most recent connection held for at least
    /// `success_hold_threshold`. Resets the scheduler back to attempt 1
    /// so the next disconnect starts from `base` again.
    ///
    /// Callers that want unconditional reset can pass
    /// `success_hold_threshold = Duration::ZERO` at construction time.
    pub fn on_success_held(&mut self) {
        self.attempt = 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sub A acceptance #1 — backoff doubles each attempt up to the cap.
    #[test]
    fn backoff_doubles_each_attempt() {
        let mut b = BackoffScheduler::default();

        // attempt 1 → 1s
        assert_eq!(b.next_delay(), Duration::from_secs(1));
        b.on_failure();

        // attempt 2 → 2s
        assert_eq!(b.next_delay(), Duration::from_secs(2));
        b.on_failure();

        // attempt 3 → 4s
        assert_eq!(b.next_delay(), Duration::from_secs(4));
        b.on_failure();

        // attempt 4 → 8s
        assert_eq!(b.next_delay(), Duration::from_secs(8));
        b.on_failure();

        // attempt 5 → 16s
        assert_eq!(b.next_delay(), Duration::from_secs(16));
        b.on_failure();
    }

    /// Sub A acceptance #2 — schedule caps at the configured maximum
    /// (default 30s) regardless of how many failures pile up.
    #[test]
    fn backoff_caps_at_max() {
        let mut b = BackoffScheduler::default();

        // Advance well past the natural doubling that would exceed 30s.
        for _ in 0..20 {
            b.on_failure();
        }

        // attempt is now 21; raw doubling would be 1s << 20 = ~1M s.
        assert_eq!(
            b.next_delay(),
            Duration::from_secs(30),
            "must saturate at the 30s cap"
        );

        // Even at the saturating-shift boundary (attempt > 31) we never panic
        // and never exceed the cap.
        for _ in 0..50 {
            b.on_failure();
        }
        assert_eq!(b.next_delay(), Duration::from_secs(30));
    }

    /// Sub A acceptance #3 — after a connection holds long enough, the
    /// next failure starts again from the base delay (not from wherever
    /// we left off).
    #[test]
    fn backoff_resets_after_success_held() {
        let mut b = BackoffScheduler::default();

        // Climb to attempt 4 (8s).
        b.on_failure();
        b.on_failure();
        b.on_failure();
        assert_eq!(b.next_delay(), Duration::from_secs(8));

        // Connection eventually came up and held — caller signals success.
        b.on_success_held();

        // Next delay must be back to the base (1s) and attempt must be 1.
        assert_eq!(b.attempt(), 1);
        assert_eq!(b.next_delay(), Duration::from_secs(1));

        // And the doubling resumes from there on subsequent failures.
        b.on_failure();
        assert_eq!(b.next_delay(), Duration::from_secs(2));
    }

    #[test]
    fn custom_parameters_respected() {
        // base 500ms, cap 2s, hold 10s
        let mut b = BackoffScheduler::new(
            Duration::from_millis(500),
            Duration::from_secs(2),
            Duration::from_secs(10),
        );
        assert_eq!(b.success_hold_threshold(), Duration::from_secs(10));

        assert_eq!(b.next_delay(), Duration::from_millis(500));
        b.on_failure();
        assert_eq!(b.next_delay(), Duration::from_secs(1));
        b.on_failure();
        assert_eq!(b.next_delay(), Duration::from_secs(2));
        b.on_failure();
        // Capped.
        assert_eq!(b.next_delay(), Duration::from_secs(2));
    }

    #[test]
    fn cap_below_base_is_clamped_to_base() {
        // Invalid config: cap < base. We clamp cap up to base rather than
        // panic, so misconfigured callers still get sane behaviour.
        let b = BackoffScheduler::new(
            Duration::from_secs(5),
            Duration::from_secs(1),
            Duration::from_secs(60),
        );
        assert_eq!(b.next_delay(), Duration::from_secs(5));
    }

    #[test]
    fn zero_base_falls_back_to_default() {
        // Zero base would make every delay zero forever; we substitute the
        // default 1s so callers can't accidentally tight-loop.
        let b = BackoffScheduler::new(
            Duration::ZERO,
            Duration::from_secs(30),
            Duration::from_secs(60),
        );
        assert_eq!(b.next_delay(), DEFAULT_BASE_DELAY);
    }

    #[test]
    fn on_failure_saturates_counter() {
        let mut b = BackoffScheduler::default();
        // Cannot realistically reach u32::MAX but verify saturating_add never wraps.
        for _ in 0..10_000 {
            b.on_failure();
        }
        // Still in a sensible state — delay capped, counter monotonic.
        assert!(b.attempt() >= 10_000);
        assert_eq!(b.next_delay(), Duration::from_secs(30));
    }
}
