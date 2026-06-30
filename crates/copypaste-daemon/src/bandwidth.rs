//! Upload bandwidth throttler (token-bucket).
//!
//! When `max_bandwidth_kbps > 0`, upload calls acquire tokens from this
//! bucket before transmitting so the daemon's outbound sync throughput stays
//! at or below the configured ceiling.  `max_bandwidth_kbps = 0` (the
//! default) leaves the path completely unthrottled — [`TokenBucket::acquire`]
//! short-circuits immediately with zero state mutation.
//!
//! ## Mechanism
//!
//! A token bucket accumulates byte credits at `rate_bps` bytes/second up to
//! a one-second burst cap.  `acquire(n)` drains `n` credits; when the bucket
//! is empty it returns the `Duration` the caller must sleep before sending.
//! The push loops call `set_rate_kbps` on each item to honour hot-reloaded
//! config changes at runtime.
//!
//! Returned delays are capped at [`MAX_PACE_SECS`] seconds so a single large
//! payload at a very low limit cannot stall the push loop for minutes.
//!
//! ## Thread safety
//!
//! `TokenBucket` is deliberately NOT `Send + Sync` — each push loop owns one
//! instance as a local variable, avoiding all locking overhead.

use std::time::{Duration, Instant};

/// Upper bound on the pacing delay returned by [`TokenBucket::acquire`].
///
/// Prevents a single large payload at a very low rate limit from stalling
/// the push loop indefinitely.
pub const MAX_PACE_SECS: u64 = 30;

/// Token-bucket rate limiter for upload pacing.
///
/// Owned by a single push loop; not thread-safe by design (no locks needed).
pub struct TokenBucket {
    /// Upload ceiling in bytes per second.  0 = unlimited (no-op path).
    rate_bps: u64,
    /// Available token credits (bytes).
    tokens: u64,
    /// Maximum credit capacity: one second of data at `rate_bps`.
    cap: u64,
    /// Wall-clock timestamp of the last token refill.
    last_refill: Instant,
}

impl TokenBucket {
    /// Create a bucket limited to `rate_kbps` kilobits per second.
    ///
    /// The bucket starts full (one second of burst credit) so the first
    /// upload is not penalised.  Pass `rate_kbps = 0` for unlimited.
    pub fn new(rate_kbps: u32) -> Self {
        let rate_bps = kbps_to_bps(rate_kbps);
        Self {
            rate_bps,
            tokens: rate_bps,
            // .max(1): avoids divide-by-zero in refill; the zero path is
            // short-circuited in acquire before refill is ever called.
            cap: rate_bps.max(1),
            last_refill: Instant::now(),
        }
    }

    /// Update the rate ceiling in place (hot-reload from live config).
    ///
    /// Excess credits are clamped to the new cap.  O(1); safe to call on
    /// every item even when the rate has not changed.
    pub fn set_rate_kbps(&mut self, rate_kbps: u32) {
        let new_bps = kbps_to_bps(rate_kbps);
        self.rate_bps = new_bps;
        self.cap = new_bps.max(1);
        self.tokens = self.tokens.min(self.cap);
    }

    /// Consume `n` bytes of upload credit.
    ///
    /// Returns the `Duration` the caller **must sleep** before sending so the
    /// long-run throughput stays at or below the ceiling.  Returns
    /// `Duration::ZERO` immediately when credits are available or when the
    /// rate is unlimited (`rate_kbps = 0`).  The delay is capped at
    /// [`MAX_PACE_SECS`] seconds.
    pub fn acquire(&mut self, n: u64) -> Duration {
        if self.rate_bps == 0 || n == 0 {
            return Duration::ZERO;
        }
        self.refill();
        if self.tokens >= n {
            self.tokens -= n;
            return Duration::ZERO;
        }
        let deficit = n - self.tokens;
        self.tokens = 0;
        // delay_µs = deficit * 1_000_000 / rate_bps, capped at MAX_PACE_SECS.
        // Saturating mul guards against overflow for pathologically large payloads.
        let max_us: u64 = MAX_PACE_SECS * 1_000_000;
        let delay_us = (deficit.saturating_mul(1_000_000) / self.rate_bps).min(max_us);
        Duration::from_micros(delay_us)
    }

    // ── Private ───────────────────────────────────────────────────────────────

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed_us = now.duration_since(self.last_refill).as_micros() as u64;
        // new_tokens = elapsed_µs * rate_bps / 1_000_000; saturating_mul for safety.
        let new_tokens = elapsed_us.saturating_mul(self.rate_bps) / 1_000_000;
        if new_tokens > 0 {
            self.tokens = (self.tokens + new_tokens).min(self.cap);
            self.last_refill = now;
        }
    }
}

fn kbps_to_bps(rate_kbps: u32) -> u64 {
    // 1 kbps = 1 000 bits/sec = 125 bytes/sec
    (rate_kbps as u64) * 1_000 / 8
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Unlimited mode ────────────────────────────────────────────────────────

    /// rate_kbps = 0 → unlimited; acquire always returns ZERO with no state change.
    #[test]
    fn unlimited_returns_zero_delay_always() {
        let mut b = TokenBucket::new(0);
        assert_eq!(b.acquire(u64::MAX), Duration::ZERO, "unlimited: MAX bytes");
        assert_eq!(b.acquire(0), Duration::ZERO, "unlimited: 0 bytes");
        assert_eq!(b.acquire(1), Duration::ZERO, "unlimited: 1 byte");
    }

    // ── First-burst credit ────────────────────────────────────────────────────

    /// A fresh bucket holds 1 second of burst; the first upload up to the cap
    /// must be free.
    #[test]
    fn fresh_bucket_one_second_burst_is_free() {
        // 8000 kbps = 8_000 * 1000 / 8 = 1_000_000 bytes/sec
        let mut b = TokenBucket::new(8_000);
        assert_eq!(
            b.acquire(1_000_000),
            Duration::ZERO,
            "first full-burst upload must be free"
        );
    }

    // ── Exhausted bucket: credit math ─────────────────────────────────────────

    /// After draining the burst, the next acquire must return the correct delay.
    ///
    /// This is the primary regression guard for the token-bucket math
    /// (CopyPaste-crh3.107).
    #[test]
    fn exhausted_bucket_returns_expected_delay() {
        // 8000 kbps → 1_000_000 bytes/sec
        let mut b = TokenBucket::new(8_000);
        b.acquire(1_000_000); // drain the full burst; tokens = 0
        let delay = b.acquire(500_000); // 500 kB deficit
                                        // Expected: 500_000 * 1_000_000 / 1_000_000 = 500_000 µs = 500 ms.
                                        // Allow ±1 ms for integer division rounding.
        let us = delay.as_micros() as u64;
        assert!(
            (499_000..=501_000).contains(&us),
            "expected ~500 ms delay, got {us} µs"
        );
    }

    // ── Partial burst ─────────────────────────────────────────────────────────

    /// Consuming less than available tokens does not incur a delay.
    #[test]
    fn partial_burst_within_tokens_is_free() {
        let mut b = TokenBucket::new(8_000); // cap = 1_000_000
        b.acquire(400_000); // consume 400 kB; 600 kB left
        assert_eq!(
            b.acquire(600_000),
            Duration::ZERO,
            "consuming remaining tokens must be free"
        );
    }

    // ── Hard cap on delay ─────────────────────────────────────────────────────

    /// The returned delay is capped at MAX_PACE_SECS regardless of payload size.
    #[test]
    fn delay_capped_at_max_pace_secs() {
        // 8 kbps = 1 000 bytes/sec; cap = 1 000.
        let mut b = TokenBucket::new(8);
        b.acquire(1_000); // drain the burst
                          // 100 MB at 1000 bytes/sec = 100_000 s uncapped → must cap at 30 s.
        let delay = b.acquire(100_000_000);
        assert_eq!(
            delay.as_secs(),
            MAX_PACE_SECS,
            "delay must cap at {MAX_PACE_SECS}s"
        );
    }

    // ── Zero-byte acquire ─────────────────────────────────────────────────────

    /// n = 0 is always free, even when the bucket is exhausted.
    #[test]
    fn acquire_zero_bytes_is_always_free() {
        let mut b = TokenBucket::new(100);
        b.acquire(100_000_000); // drain everything
        assert_eq!(
            b.acquire(0),
            Duration::ZERO,
            "zero-byte acquire must never sleep"
        );
    }

    // ── Hot-reload ────────────────────────────────────────────────────────────

    /// set_rate_kbps updates the ceiling; a drained bucket then reflects the
    /// new rate.  This is the regression guard for the hot-reload path
    /// (CopyPaste-crh3.107).
    #[test]
    fn set_rate_kbps_hot_reload_changes_delay() {
        // Start at 8000 kbps (1 MB/s), drain the burst.
        let mut b = TokenBucket::new(8_000);
        b.acquire(1_000_000); // tokens = 0
                              // Drop to 8 kbps = 1000 bytes/sec.
        b.set_rate_kbps(8);
        // 1 kB at 1000 bytes/sec → 1 second delay.
        let delay = b.acquire(1_000);
        assert!(
            delay.as_secs() >= 1,
            "after rate drop to 8 kbps, 1 kB should take >=1 s; got {delay:?}"
        );
    }
}
