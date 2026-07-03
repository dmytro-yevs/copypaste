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

/// Maximum number of distinct per-key buckets retained at once.
///
/// Security MED: the per-key bucket map is keyed on the *unauthenticated*,
/// rotatable TXT `device_id`. A LAN attacker rotating that field would
/// otherwise mint an unbounded number of fresh buckets, exhausting memory.
/// Once this cap is reached we refuse to mint new buckets (existing keys keep
/// working) and rely on the global bucket below to bound aggregate admission.
pub const MAX_TRACKED_KEYS: usize = 4096;

/// Global (across-all-keys) burst size. Sized well above a legitimate LAN's
/// peer churn but low enough that id-rotation can't amplify processing cost.
pub const GLOBAL_BURST_CAPACITY: u32 = 256;

/// Global refill rate in tokens per second, shared across every key.
pub const GLOBAL_REFILL_RATE_PER_SEC: f64 = 64.0;

/// Env-configurable thresholds for mDNS event admission (CopyPaste-1d5l.4).
///
/// The `pub const` compile-time values above remain the defaults — used by
/// [`Default`] and every existing test, none of which sets the env vars
/// below — but production construction ([`MdnsRateLimiter::new`]) goes
/// through [`Self::from_env`] so an operator can relieve a false-positive
/// mDNS-flood drop storm (or tighten the thresholds) without a rebuild.
/// Mirrors the `RELAY_*` env-config pattern in
/// `copypaste-relay/src/config.rs` (CopyPaste-8ebg.50).
#[derive(Debug, Clone, Copy, PartialEq)]
struct MdnsRateLimitConfig {
    burst_capacity: u32,
    refill_rate_per_sec: f64,
    max_tracked_keys: usize,
    global_burst_capacity: u32,
    global_refill_rate_per_sec: f64,
}

impl Default for MdnsRateLimitConfig {
    fn default() -> Self {
        Self {
            burst_capacity: BURST_CAPACITY,
            refill_rate_per_sec: REFILL_RATE_PER_SEC,
            max_tracked_keys: MAX_TRACKED_KEYS,
            global_burst_capacity: GLOBAL_BURST_CAPACITY,
            global_refill_rate_per_sec: GLOBAL_REFILL_RATE_PER_SEC,
        }
    }
}

impl MdnsRateLimitConfig {
    /// Load configuration from environment variables. Falls back to the
    /// compile-time defaults for any variable that is absent or unparseable.
    ///
    /// Recognised variables:
    /// - `COPYPASTE_MDNS_BURST_CAPACITY`             — per-key burst size (u32, default 10)
    /// - `COPYPASTE_MDNS_REFILL_RATE_PER_SEC`        — per-key refill rate, tokens/s (f64, default 2.0)
    /// - `COPYPASTE_MDNS_MAX_TRACKED_KEYS`           — per-key bucket-map cap (usize, default 4096)
    /// - `COPYPASTE_MDNS_GLOBAL_BURST_CAPACITY`      — global burst size (u32, default 256)
    /// - `COPYPASTE_MDNS_GLOBAL_REFILL_RATE_PER_SEC` — global refill rate, tokens/s (f64, default 64.0)
    ///
    /// Rates are clamped to a minimum positive value (`>= 1` for counts,
    /// `> 0.0` for refill rates): a `0` burst/tracked-keys cap would silently
    /// drop every mDNS event and a `0` refill rate would never replenish a
    /// bucket, effectively wedging discovery — the same "clamp rather than
    /// let a misconfig brick the feature" rationale as the relay's
    /// `RELAY_MAX_CONNECTIONS`/`RELAY_PER_IP_PER_SECOND` handling.
    fn from_env() -> Self {
        let mut cfg = Self::default();
        if let Ok(v) = std::env::var("COPYPASTE_MDNS_BURST_CAPACITY") {
            if let Ok(n) = v.parse::<u32>() {
                cfg.burst_capacity = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("COPYPASTE_MDNS_REFILL_RATE_PER_SEC") {
            if let Ok(n) = v.parse::<f64>() {
                if n > 0.0 {
                    cfg.refill_rate_per_sec = n;
                }
            }
        }
        if let Ok(v) = std::env::var("COPYPASTE_MDNS_MAX_TRACKED_KEYS") {
            if let Ok(n) = v.parse::<usize>() {
                cfg.max_tracked_keys = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("COPYPASTE_MDNS_GLOBAL_BURST_CAPACITY") {
            if let Ok(n) = v.parse::<u32>() {
                cfg.global_burst_capacity = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("COPYPASTE_MDNS_GLOBAL_REFILL_RATE_PER_SEC") {
            if let Ok(n) = v.parse::<f64>() {
                if n > 0.0 {
                    cfg.global_refill_rate_per_sec = n;
                }
            }
        }
        cfg
    }
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    /// Fractional tokens currently available (0.0 .. `capacity`).
    tokens: f64,
    /// Burst capacity / refill ceiling for this bucket.
    capacity: f64,
    /// Refill rate in tokens per second.
    refill_per_sec: f64,
    /// Last time the bucket was refilled or consumed.
    last_refill: Instant,
    /// Last time the bucket was touched at all (used for cleanup).
    last_used: Instant,
}

impl Bucket {
    fn with_params(now: Instant, capacity: f64, refill_per_sec: f64) -> Self {
        Self {
            tokens: capacity,
            capacity,
            refill_per_sec,
            last_refill: now,
            last_used: now,
        }
    }

    /// Refill the bucket based on elapsed time, capped at `capacity`.
    fn refill(&mut self, now: Instant) {
        let elapsed = now.saturating_duration_since(self.last_refill);
        let added = elapsed.as_secs_f64() * self.refill_per_sec;
        if added > 0.0 {
            self.tokens = (self.tokens + added).min(self.capacity);
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
    /// Admission thresholds — env-overridable defaults (CopyPaste-1d5l.4).
    /// `Default::default()` (used by tests) resolves to the compile-time
    /// `pub const` values; [`Self::new`] resolves them via
    /// [`MdnsRateLimitConfig::from_env`] instead.
    config: MdnsRateLimitConfig,
    buckets: Mutex<HashMap<String, Bucket>>,
    /// Global token bucket shared across all keys. Bounds aggregate admission
    /// so a peer rotating the unauthenticated `device_id` TXT field cannot get
    /// unlimited fresh per-key budget (security MED). Lazily initialised on
    /// first admission so the struct stays `Default`-constructible.
    global: Mutex<Option<Bucket>>,
    /// Last time `cleanup_if_due` performed a sweep. `None` until first call.
    last_cleanup: Mutex<Option<Instant>>,
    /// Monotonic count of dropped events (for sampled logging).
    drops: Mutex<u64>,
}

impl MdnsRateLimiter {
    /// Construct an empty rate limiter, sourcing its admission thresholds
    /// from the `COPYPASTE_MDNS_*` env vars (falling back to the compile-time
    /// defaults — see [`MdnsRateLimitConfig::from_env`]).
    pub fn new() -> Self {
        Self {
            config: MdnsRateLimitConfig::from_env(),
            ..Self::default()
        }
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

        // Global admission gate first: a single shared bucket caps the
        // aggregate event rate regardless of how many distinct keys an
        // attacker rotates through (security MED). If the global budget is
        // exhausted we drop without even touching the per-key map, which also
        // means key-rotation cannot be used to grow the map past the gate.
        let global_ok = {
            let mut global = self.global.lock().unwrap_or_else(|e| e.into_inner());
            let bucket = global.get_or_insert_with(|| {
                Bucket::with_params(
                    now,
                    f64::from(self.config.global_burst_capacity),
                    self.config.global_refill_rate_per_sec,
                )
            });
            bucket.try_consume(now)
        };
        if !global_ok {
            self.record_drop(key);
            self.cleanup_if_due(now);
            return false;
        }

        let admitted = {
            let mut buckets = self.buckets.lock().unwrap_or_else(|e| e.into_inner());
            match buckets.get_mut(key) {
                Some(bucket) => bucket.try_consume(now),
                None => {
                    // Cap the number of distinct buckets. Refuse to mint a new
                    // one past the cap rather than evicting (eviction here would
                    // let an attacker flush legitimate peers' buckets). The
                    // global gate above already bounds aggregate throughput.
                    if buckets.len() >= self.config.max_tracked_keys {
                        false
                    } else {
                        let mut bucket = Bucket::with_params(
                            now,
                            f64::from(self.config.burst_capacity),
                            self.config.refill_rate_per_sec,
                        );
                        let admitted = bucket.try_consume(now);
                        buckets.insert(key.to_owned(), bucket);
                        admitted
                    }
                }
            }
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
    use std::sync::{Mutex as StdMutex, OnceLock};

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    /// Process-wide mutex for tests that mutate `COPYPASTE_MDNS_*` env vars.
    /// Held for the duration of the mutation + read so parallel test threads
    /// cannot race each other (mirrors `copypaste-relay/src/config.rs`'s
    /// `env_lock`).
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static ENV_MUTEX: OnceLock<StdMutex<()>> = OnceLock::new();
        ENV_MUTEX.get_or_init(|| StdMutex::new(())).lock().unwrap()
    }

    /// CopyPaste-1d5l.4: mDNS-flood thresholds must be tunable via env vars
    /// without a rebuild, so an operator can relieve a false-positive drop
    /// storm (or tighten the thresholds) by restarting with new env.
    #[test]
    fn mdns_rate_limit_config_reads_from_env() {
        let _guard = env_lock();
        // SAFETY: serialised via env_lock; removed before the guard drops.
        unsafe {
            std::env::set_var("COPYPASTE_MDNS_BURST_CAPACITY", "20");
            std::env::set_var("COPYPASTE_MDNS_REFILL_RATE_PER_SEC", "5");
            std::env::set_var("COPYPASTE_MDNS_MAX_TRACKED_KEYS", "8192");
            std::env::set_var("COPYPASTE_MDNS_GLOBAL_BURST_CAPACITY", "512");
            std::env::set_var("COPYPASTE_MDNS_GLOBAL_REFILL_RATE_PER_SEC", "128");
        }
        let cfg = MdnsRateLimitConfig::from_env();
        // SAFETY: serialised via env_lock.
        unsafe {
            std::env::remove_var("COPYPASTE_MDNS_BURST_CAPACITY");
            std::env::remove_var("COPYPASTE_MDNS_REFILL_RATE_PER_SEC");
            std::env::remove_var("COPYPASTE_MDNS_MAX_TRACKED_KEYS");
            std::env::remove_var("COPYPASTE_MDNS_GLOBAL_BURST_CAPACITY");
            std::env::remove_var("COPYPASTE_MDNS_GLOBAL_REFILL_RATE_PER_SEC");
        }

        assert_eq!(cfg.burst_capacity, 20);
        assert_eq!(cfg.refill_rate_per_sec, 5.0);
        assert_eq!(cfg.max_tracked_keys, 8192);
        assert_eq!(cfg.global_burst_capacity, 512);
        assert_eq!(cfg.global_refill_rate_per_sec, 128.0);
    }

    /// A `0` count/rate must be clamped rather than passed through, mirroring
    /// the relay's `RELAY_MAX_CONNECTIONS`/`RELAY_PER_IP_PER_SECOND` handling
    /// — a `0` burst or refill rate would otherwise silently wedge admission.
    #[test]
    fn mdns_rate_limit_config_zero_is_clamped_or_ignored() {
        let _guard = env_lock();
        // SAFETY: serialised via env_lock; removed before the guard drops.
        unsafe {
            std::env::set_var("COPYPASTE_MDNS_BURST_CAPACITY", "0");
            std::env::set_var("COPYPASTE_MDNS_REFILL_RATE_PER_SEC", "0");
            std::env::set_var("COPYPASTE_MDNS_MAX_TRACKED_KEYS", "0");
        }
        let cfg = MdnsRateLimitConfig::from_env();
        unsafe {
            std::env::remove_var("COPYPASTE_MDNS_BURST_CAPACITY");
            std::env::remove_var("COPYPASTE_MDNS_REFILL_RATE_PER_SEC");
            std::env::remove_var("COPYPASTE_MDNS_MAX_TRACKED_KEYS");
        }

        assert_eq!(cfg.burst_capacity, 1, "a 0 burst capacity must clamp to 1");
        assert_eq!(
            cfg.max_tracked_keys, 1,
            "a 0 tracked-keys cap must clamp to 1"
        );
        // A non-positive refill rate is rejected outright (kept at default)
        // rather than clamped, since there is no sane "minimum positive f64".
        assert_eq!(
            cfg.refill_rate_per_sec, REFILL_RATE_PER_SEC,
            "a 0 refill rate must fall back to the compile-time default"
        );
    }

    #[test]
    fn mdns_rate_limiter_new_matches_defaults_when_env_absent() {
        let _guard = env_lock();
        std::env::remove_var("COPYPASTE_MDNS_BURST_CAPACITY");
        std::env::remove_var("COPYPASTE_MDNS_REFILL_RATE_PER_SEC");
        std::env::remove_var("COPYPASTE_MDNS_MAX_TRACKED_KEYS");
        std::env::remove_var("COPYPASTE_MDNS_GLOBAL_BURST_CAPACITY");
        std::env::remove_var("COPYPASTE_MDNS_GLOBAL_REFILL_RATE_PER_SEC");
        let rl = MdnsRateLimiter::new();
        assert_eq!(rl.config, MdnsRateLimitConfig::default());
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

    /// Security MED: rotating the (unauthenticated) key must not yield unlimited
    /// admissions — the global token bucket caps the aggregate at its burst
    /// capacity even when every request uses a brand-new key.
    #[tokio::test(start_paused = true)]
    async fn global_bucket_caps_rotating_keys() {
        let rl = MdnsRateLimiter::new();
        // Each fresh key has its own full per-key bucket, so without the global
        // gate this loop would admit every single request.
        let mut admitted = 0u32;
        for i in 0..(GLOBAL_BURST_CAPACITY + 50) {
            if rl.try_admit_key(&format!("rotating-id-{i}")) {
                admitted += 1;
            }
        }
        assert_eq!(
            admitted, GLOBAL_BURST_CAPACITY,
            "global bucket must cap aggregate admissions at its burst capacity"
        );
    }

    /// Security MED: the per-key bucket map must not grow without bound when an
    /// attacker rotates keys. Pump enough distinct keys (slowly, so the global
    /// bucket refills and keeps admitting) and confirm the map size is bounded.
    #[tokio::test(start_paused = true)]
    async fn tracked_keys_are_bounded() {
        use tokio::time::{advance, Duration};
        let rl = MdnsRateLimiter::new();
        // Far more distinct keys than the cap. Advance time between batches so
        // the global bucket refills and lets new keys reach the map.
        for batch in 0..((MAX_TRACKED_KEYS / GLOBAL_BURST_CAPACITY as usize) + 4) {
            for i in 0..GLOBAL_BURST_CAPACITY {
                let _ = rl.try_admit_key(&format!("k-{batch}-{i}"));
            }
            // Refill the global bucket fully before the next batch.
            advance(Duration::from_secs(10)).await;
        }
        assert!(
            rl.tracked_ip_count() <= MAX_TRACKED_KEYS,
            "tracked-key map must stay at or below MAX_TRACKED_KEYS, got {}",
            rl.tracked_ip_count()
        );
    }
}
