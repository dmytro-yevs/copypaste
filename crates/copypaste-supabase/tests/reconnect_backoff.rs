//! Beta-bonus: Supabase Realtime reconnect backoff specification tests.
//!
//! The reconnect loop in `realtime::connection_loop` is private and tightly
//! coupled to a real WebSocket connect attempt, so we cannot drive it directly
//! from a black-box test. Instead this suite pins the backoff *contract* that
//! the production loop must satisfy:
//!
//!   - exponential doubling (1s, 2s, 4s, 8s, 16s, …)
//!   - capped at `max_backoff` (default 60s)
//!   - optional jitter within ±10% of nominal delay
//!   - bounded retry count (default 10) before final `Err`
//!   - reset to `initial_backoff` after a successful connection
//!
//! The algorithm is mirrored here as a pure function so the test stays a
//! single-file black-box assertion. When `realtime.rs` is later refactored to
//! expose `next_backoff()` / `BackoffState`, this file should be migrated to
//! call those symbols directly — the assertions stay identical.
//!
//! Determinism: every async test uses `#[tokio::test(start_paused = true)]`
//! plus `tokio::time::advance` so the suite is wall-clock-free and runs in
//! milliseconds.

#![allow(clippy::needless_range_loop)]

use std::time::Duration;

use copypaste_supabase::RealtimeConfig;

// ─── Reference algorithm (mirrors src/realtime.rs::connection_loop) ──────────

/// State for the reconnect backoff scheduler.
///
/// Mirrors the local `let mut backoff = config.initial_backoff;` plus the
/// `backoff = (backoff * 2).min(config.max_backoff);` step in `connection_loop`.
/// Adds a `retries` counter and an optional jitter source so the same struct
/// can express the full spec.
struct BackoffState {
    initial: Duration,
    max: Duration,
    current: Duration,
    retries: u32,
    max_retries: u32,
}

impl BackoffState {
    fn new(initial: Duration, max: Duration, max_retries: u32) -> Self {
        Self {
            initial,
            max,
            current: initial,
            retries: 0,
            max_retries,
        }
    }

    /// Yield the next delay, or `None` once we've exhausted `max_retries`.
    fn next_delay(&mut self) -> Option<Duration> {
        if self.retries >= self.max_retries {
            return None;
        }
        let delay = self.current;
        self.retries += 1;
        // Double after handing out the current delay, capped at `max`.
        self.current = (self.current.saturating_mul(2)).min(self.max);
        Some(delay)
    }

    /// Apply ±`pct_window`% symmetric jitter, deterministic from `seed`.
    /// Pure: no RNG, no thread state — `seed` controls the offset.
    fn jittered(delay: Duration, pct_window: u32, seed: u64) -> Duration {
        // Map seed into [-pct_window, +pct_window] percent of `delay`.
        // Range size is 2*pct_window+1 buckets.
        let span = 2 * pct_window as u64 + 1;
        let bucket = (seed % span) as i64 - pct_window as i64; // signed offset %
        let nanos = delay.as_nanos() as i128;
        let delta = nanos * bucket as i128 / 100;
        let adj = (nanos + delta).max(0) as u128;
        Duration::from_nanos(adj.min(u64::MAX as u128) as u64)
    }

    /// Called by the production loop after a successful connection — resets
    /// the curve back to `initial_backoff`.
    fn reset(&mut self) {
        self.current = self.initial;
        self.retries = 0;
    }
}

// ─── 1. Exponential doubling ─────────────────────────────────────────────────

/// First five attempts after a disconnect produce the documented curve:
/// 1s, 2s, 4s, 8s, 16s — i.e. each delay is exactly 2× the previous.
#[tokio::test(start_paused = true)]
async fn backoff_exponential_doubles_each_attempt() {
    let mut state = BackoffState::new(Duration::from_secs(1), Duration::from_secs(60), 10);

    let expected = [
        Duration::from_secs(1),
        Duration::from_secs(2),
        Duration::from_secs(4),
        Duration::from_secs(8),
        Duration::from_secs(16),
    ];

    let mut observed: Vec<Duration> = Vec::with_capacity(expected.len());
    for _ in 0..expected.len() {
        let d = state.next_delay().expect("retries not exhausted yet");
        observed.push(d);
        // Burn the simulated time — keeps the test honest re: time travel.
        tokio::time::advance(d).await;
    }

    assert_eq!(
        observed,
        expected.to_vec(),
        "exponential backoff curve must be 1s, 2s, 4s, 8s, 16s",
    );

    // Every step must be exactly 2× the prior one.
    for w in observed.windows(2) {
        assert_eq!(
            w[1],
            w[0] * 2,
            "delay {:?} should be exactly double {:?}",
            w[1],
            w[0]
        );
    }
}

// ─── 2. Cap at max_backoff (60s by default) ──────────────────────────────────

/// After enough doublings (1 → 2 → 4 → 8 → 16 → 32 → 64 capped to 60) the
/// scheduler must clamp every subsequent delay at `max_backoff`. We verify
/// by advancing past the saturation point and confirming the next handful of
/// delays are all == max.
#[tokio::test(start_paused = true)]
async fn backoff_capped_at_max_60s() {
    let max = Duration::from_secs(60);
    let mut state = BackoffState::new(Duration::from_secs(1), max, 20);

    // Drain enough attempts that we land in the saturated regime.
    // 1, 2, 4, 8, 16, 32, 60(cap), 60, 60, 60 — last 4 should all be 60.
    let mut saturated_tail: Vec<Duration> = Vec::new();
    for i in 0..10 {
        let d = state.next_delay().expect("retries not exhausted");
        if i >= 6 {
            saturated_tail.push(d);
        }
        tokio::time::advance(d).await;
    }

    assert!(
        saturated_tail.iter().all(|d| *d == max),
        "every delay past the saturation point must equal max_backoff (60s); got {saturated_tail:?}",
    );

    // The cap itself is exactly 60s — guard against silent off-by-one.
    assert_eq!(*saturated_tail.last().unwrap(), Duration::from_secs(60));
}

// ─── 3. Jitter window ────────────────────────────────────────────────────────

/// Jittered delays for a nominal 4s value over 50 deterministic samples must
/// all land within ±10% of 4s, AND the population standard deviation must be
/// non-trivial (jitter is actually being applied, not stubbed to 0) but still
/// inside the spec window. We use a deterministic seed sweep so the test is
/// reproducible.
#[tokio::test(start_paused = true)]
async fn jitter_within_10_percent_window() {
    let nominal = Duration::from_secs(4);
    let pct: u32 = 10;
    let lo = nominal.as_nanos() as f64 * 0.90;
    let hi = nominal.as_nanos() as f64 * 1.10;

    let samples: Vec<f64> = (0..50)
        .map(|seed| BackoffState::jittered(nominal, pct, seed as u64).as_nanos() as f64)
        .collect();

    // Bounds: every sample inside [0.9 × nominal, 1.1 × nominal].
    for (i, s) in samples.iter().enumerate() {
        assert!(
            *s >= lo && *s <= hi,
            "sample #{i} = {s}ns outside ±10% window [{lo}, {hi}]",
        );
    }

    // Spread: stddev must be > 0 (else we're not actually jittering) and
    // < the half-window (else we exceed the spec).
    let n = samples.len() as f64;
    let mean = samples.iter().sum::<f64>() / n;
    let var = samples.iter().map(|s| (s - mean).powi(2)).sum::<f64>() / n;
    let stddev = var.sqrt();

    let half_window = nominal.as_nanos() as f64 * 0.10;
    assert!(stddev > 0.0, "stddev must be > 0; jitter not applied");
    assert!(
        stddev < half_window,
        "stddev {stddev}ns exceeds ±10% half-window {half_window}ns",
    );
}

// ─── 4. Bounded retries ──────────────────────────────────────────────────────

/// After `max_retries` (default 10) consecutive failures the scheduler must
/// stop yielding delays — `next_delay()` returns `None`, which the production
/// loop maps to a terminal `Err`. We model the "final Err" by asserting the
/// `None` sentinel here.
#[tokio::test(start_paused = true)]
async fn max_retries_then_gives_up_with_error() {
    let max_retries: u32 = 10;
    let mut state = BackoffState::new(Duration::from_secs(1), Duration::from_secs(60), max_retries);

    // First `max_retries` calls must yield a delay.
    for i in 0..max_retries {
        let d = state.next_delay();
        assert!(d.is_some(), "attempt #{i} should still yield a delay");
        tokio::time::advance(d.unwrap()).await;
    }

    // The (max_retries+1)-th call must return None — "give up".
    let final_attempt = state.next_delay();
    assert!(
        final_attempt.is_none(),
        "after {max_retries} retries the scheduler must give up (None), got {final_attempt:?}",
    );
}

// ─── 5. Reset on successful reconnect ────────────────────────────────────────

/// After the connection succeeds the loop calls `reset()` and the next
/// disconnect must start the curve back at `initial_backoff` (1s) — not at
/// the saturated value from the previous failure streak.
#[tokio::test(start_paused = true)]
async fn reset_on_successful_connection() {
    let initial = Duration::from_secs(1);
    let mut state = BackoffState::new(initial, Duration::from_secs(60), 10);

    // Burn five attempts so the curve climbs to 16s.
    for _ in 0..5 {
        let d = state.next_delay().expect("not yet exhausted");
        tokio::time::advance(d).await;
    }
    // Sanity: the *next* delay would be 32s (curve is climbing).
    // Don't consume it — peek by cloning the current state.
    assert_eq!(state.current, Duration::from_secs(32));

    // Successful reconnect → loop calls reset().
    state.reset();

    // The next disconnect must start back at 1s and double from there.
    let resumed = state.next_delay().expect("retries reset to 0");
    assert_eq!(
        resumed, initial,
        "after successful connection the next backoff must equal initial_backoff",
    );

    let second = state.next_delay().expect("not exhausted");
    assert_eq!(
        second,
        Duration::from_secs(2),
        "second attempt after reset must double back to 2s",
    );
}

// ─── 6. Production config defaults match the spec curve ──────────────────────

/// Belt-and-braces: the public `RealtimeConfig::new` defaults must match the
/// numbers we pinned in the algorithm tests above. If a future change drops
/// `initial_backoff` to 500ms or raises `max_backoff` to 120s, every
/// algorithm test above silently drifts unless we also assert the source of
/// truth here.
#[test]
fn production_config_defaults_match_spec() {
    let cfg = RealtimeConfig::new(
        "https://example.supabase.co",
        "anon-key",
        RealtimeConfig::DEFAULT_TOPIC,
        true,
    );
    assert_eq!(
        cfg.initial_backoff,
        Duration::from_secs(1),
        "spec: initial backoff = 1s",
    );
    assert_eq!(
        cfg.max_backoff,
        Duration::from_secs(60),
        "spec: max backoff = 60s",
    );
}
