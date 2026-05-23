//! Per-source-IP token-bucket rate limiter for mDNS event processing.
//!
//! # Threat model
//!
//! Mitigates OI-3 from THREAT-MODEL: a malicious host on the local network
//! floods us with mDNS service-resolved events. Without throttling, every
//! event triggers HashMap mutations, callback invocations, and log writes,
//! which is an asymmetric amplifier — cheap to send, expensive to process.
//!
//! # Implementation notes
//!
//! The `mdns-sd` 0.19 crate does not expose a per-query hook on its built-in
//! responder, so we cannot gate outbound replies to mDNS *queries*. We
//! instead gate inbound `ServiceResolved` events surfaced to our event loop
//! — the only flood-vector we own. Entries unused for 60s are reaped to
//! bound memory.
//!
//! # Keying
//!
//! Buckets are keyed by an opaque `String`. The discovery layer feeds the
//! peer's `device_id` (its cert fingerprint as advertised in TXT) when
//! available, and falls back to a hash of the *sorted* address set when not
//! (security MED #11). That closes the dual-stack bypass where a peer with
//! both IPv4 and IPv6 addresses appeared as two distinct first-address
//! buckets and so got 2× the configured budget. `try_admit_ip` is preserved
//! as a thin wrapper for callers that genuinely only know the source IP.
//!
//! Parameters (per IP):
//! - `BURST_CAPACITY = 10` tokens
//! - `REFILL_RATE = 2 tokens / second` (1 token per 500ms)
//! - `CLEANUP_IDLE = 60s`

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Duration;

use tokio::time::Instant;

/// Maximum burst size per source IP.
pub const BURST_CAPACITY: u32 = 10;
/// Refill rate in tokens per second per source IP.
pub const REFILL_RATE_PER_SEC: f64 = 2.0;
/// Idle period after which a bucket is eligible for cleanup.
pub const CLEANUP_IDLE: Duration = Duration::from_secs(60);
/// How often to run the cleanup sweep.
pub const CLEANUP_INTERVAL: Duration = Duration::from_secs(60);
/// Log a sampled warning every Nth drop to avoid log spam.
pub const DROP_LOG_SAMPLE_RATE: u64 = 100;

#[derive(Debug, Clone, Copy)]
struct Bucket {
    /// Fractional tokens currently available (0.0 .. BURST_CAPACITY).
    tokens: f64,
    /// Last time the bucket was refilled or consumed.
    last_refill: Instant,
    /// Last time the bucket was touched at all (used for cleanup).
    last_used: Instant,
}

impl Bucket {
    fn new(now: Instant) -> Self {
        Self {
            tokens: f64::from(BURST_CAPACITY),
            last_refill: now,
            last_used: now,
        }
    }

    /// Refill the bucket based on elapsed time, capped at `BURST_CAPACITY`.
    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill);
        let added = elapsed.as_secs_f64() * REFILL_RATE_PER_SEC;
        if added > 0.0 {
            self.tokens = (self.tokens + added).min(f64::from(BURST_CAPACITY));
            self.last_refill = now;
        }
    }

    /// Consume one token if available. Returns `true` on success.
    fn try_consume(&mut self, now: Instant) -> bool {
        self.refill(now);
        self.last_used = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Per-identity token-bucket rate limiter with periodic cleanup of idle
/// buckets.
///
/// Thread-safe via an internal `Mutex`. Designed to be wrapped in `Arc`
/// and shared across the mDNS event handler.
#[derive(Debug, Default)]
pub struct MdnsRateLimiter {
    buckets: Mutex<HashMap<String, Bucket>>,
    /// Last time `cleanup_if_due` performed a sweep. `None` until first call.
    last_cleanup: Mutex<Option<Instant>>,
    /// Monotonic count of dropped events (for sampled logging).
    drops: Mutex<u64>,
}

impl MdnsRateLimiter {
    /// Construct an empty rate limiter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attempt to consume one token for an opaque `key` at the current
    /// monotonic time.
    ///
    /// Returns `true` if the event should be processed, `false` if it should
    /// be dropped. Callers should pass the most-stable identifier they have
    /// for the peer (preferably the device-id / fingerprint; see crate-level
    /// note) so dual-stack peers don't get a 2× budget from having two
    /// addresses (security MED #11).
    pub fn try_admit_key(&self, key: &str) -> bool {
        let now = Instant::now();
        let admitted = {
            let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
            let bucket = buckets
                .entry(key.to_owned())
                .or_insert_with(|| Bucket::new(now));
            bucket.try_consume(now)
        };
        if !admitted {
            self.record_drop(key);
        }
        self.cleanup_if_due(now);
        admitted
    }

    /// Backwards-compatible shim: key the bucket by the source IP's string
    /// form. Prefer [`Self::try_admit_key`] with the peer's device-id when
    /// that is known.
    pub fn try_admit(&self, ip: IpAddr) -> bool {
        self.try_admit_key(&ip.to_string())
    }

    /// Force-run the idle-bucket sweep regardless of last-cleanup timestamp.
    ///
    /// Exposed for tests; production callers rely on `try_admit`'s internal
    /// throttled invocation.
    pub fn cleanup_now(&self) {
        let now = Instant::now();
        self.cleanup_inner(now);
    }

    /// Current number of tracked source identities. Useful for tests and
    /// metrics.
    pub fn tracked_ip_count(&self) -> usize {
        self.buckets.lock().unwrap_or_else(|e| e.into_inner()).len()
    }

    /// Total number of events dropped since construction.
    pub fn total_drops(&self) -> u64 {
        *self.drops.lock().unwrap_or_else(|e| e.into_inner())
    }

    // ── internal helpers ────────────────────────────────────────────────

    fn cleanup_if_due(&self, now: Instant) {
        let mut last = self.last_cleanup.lock().unwrap_or_else(|e| e.into_inner());
        let due = match *last {
            Some(t) => now.saturating_duration_since(t) >= CLEANUP_INTERVAL,
            None => true, // first call seeds the timer; cheap to sweep an empty map
        };
        if due {
            *last = Some(now);
            drop(last);
            self.cleanup_inner(now);
        }
    }

    fn cleanup_inner(&self, now: Instant) {
        let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
        buckets.retain(|_key, b| now.saturating_duration_since(b.last_used) < CLEANUP_IDLE);
    }

    fn record_drop(&self, key: &str) {
        let mut drops = self.drops.lock().unwrap_or_else(|e| e.into_inner());
        *drops = drops.saturating_add(1);
        let count = *drops;
        drop(drops);
        // Always emit a trace-level breadcrumb…
        tracing::trace!(%key, "mDNS event dropped (rate limit)");
        // …and a sampled warn so operators see the trend without log flood.
        if count.is_multiple_of(DROP_LOG_SAMPLE_RATE) {
            tracing::warn!(
                %key,
                total_drops = count,
                "mDNS rate limit dropping events (sampled 1/{})",
                DROP_LOG_SAMPLE_RATE
            );
        }
    }
}

// ── unit tests (pure logic; integration tests live in tests/) ──────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[tokio::test(start_paused = true)]
    async fn fresh_bucket_admits_burst_capacity() {
        let rl = MdnsRateLimiter::new();
        let addr = ip("10.0.0.1");
        for i in 0..BURST_CAPACITY {
            assert!(rl.try_admit(addr), "should admit token #{i}");
        }
    }

    #[tokio::test(start_paused = true)]
    async fn drops_count_is_tracked() {
        let rl = MdnsRateLimiter::new();
        let addr = ip("10.0.0.2");
        for _ in 0..BURST_CAPACITY {
            rl.try_admit(addr);
        }
        for _ in 0..5 {
            assert!(!rl.try_admit(addr));
        }
        assert_eq!(rl.total_drops(), 5);
    }
}
