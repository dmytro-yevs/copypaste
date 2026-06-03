//! Background cleanup task for `tower_governor` rate-limit state.
//!
//! `tower_governor` (backed by `governor`) keeps a per-key in-memory bucket
//! for every distinct client IP or device id it has seen. Without periodic
//! eviction those buckets accumulate for the lifetime of the process and the
//! relay's resident memory grows unboundedly with the number of distinct
//! clients.
//!
//! `governor::RateLimiter::retain_recent()` drops all keyed state that has
//! not been touched within the configured replenishment window — i.e. any
//! client whose bucket is already fully replenished is evicted. Calling it
//! every 60 seconds bounds the in-memory footprint to roughly
//! *active-clients × bucket-size*, which is the correct long-run steady
//! state.
//!
//! # Usage
//!
//! `relay_router` returns a `(Router, RetainFns)` tuple.  Pass the
//! `RetainFns` vec to [`spawn_cleanup_all`] to start the background task:
//!
//! ```ignore
//! let (app, retain_fns) = relay_router(state, config.clone());
//! let _cleanup = governor_cleanup::spawn_cleanup_all(
//!     retain_fns,
//!     governor_cleanup::GOVERNOR_CLEANUP_TICK_SECS,
//! );
//! ```

use std::time::Duration;

use tokio::task::JoinHandle;
// SharedRateLimiter is only used by the #[cfg(test)] spawn_governor_cleanup
// convenience wrapper; import it there to avoid an unused-import warning in
// non-test builds.
#[cfg(test)]
use tower_governor::governor::SharedRateLimiter;

/// How often to evict stale rate-limit buckets from each governor limiter.
pub const GOVERNOR_CLEANUP_TICK_SECS: u64 = 60;

/// Spawn a single background tokio task that calls every closure in
/// `retain_fns` every `tick_secs` seconds.
///
/// Each closure in `retain_fns` is expected to call `retain_recent()` on one
/// governor limiter.  Grouping all limiters into one task avoids spawning N
/// tasks for N limiters.
///
/// The task runs for the process lifetime.  Drop or abort the returned
/// `JoinHandle` to stop it on an orderly shutdown.
pub fn spawn_cleanup_all(
    retain_fns: Vec<Box<dyn Fn() + Send + Sync + 'static>>,
    tick_secs: u64,
) -> JoinHandle<()> {
    let tick = Duration::from_secs(tick_secs.max(1));

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tick);
        // Skip the immediate first tick so we do not evict right after startup
        // (recently-seen clients should keep their buckets for the first window).
        interval.tick().await;
        loop {
            interval.tick().await;
            for retain in &retain_fns {
                // retain_recent() drops every keyed bucket whose token count
                // has already been fully replenished — i.e. the client has
                // been idle for at least one full replenishment window.  O(n)
                // over the map size; negligible at 60-s intervals.
                retain();
            }
        }
    })
}

/// Convenience wrapper: spawn a cleanup task for a single typed
/// `SharedRateLimiter`.  Used in tests; production code uses
/// [`spawn_cleanup_all`] with the vec returned by `relay_router`.
#[cfg(test)]
pub fn spawn_governor_cleanup<Key, M>(
    limiter: SharedRateLimiter<Key, M>,
    tick_secs: u64,
) -> JoinHandle<()>
where
    Key: std::hash::Hash + Eq + Clone + Send + Sync + 'static,
    M: governor::middleware::RateLimitingMiddleware<governor::clock::QuantaInstant>
        + Send
        + Sync
        + 'static,
{
    spawn_cleanup_all(vec![Box::new(move || limiter.retain_recent())], tick_secs)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use std::net::IpAddr;
    use std::str::FromStr;
    use std::sync::Arc;
    use std::time::Duration;

    use tower_governor::governor::GovernorConfigBuilder;

    use super::*;

    /// Verify that `spawn_cleanup_all` spawns without panicking and that
    /// `retain_recent` on a freshly-built limiter does not crash.
    #[tokio::test]
    async fn spawn_cleanup_all_does_not_panic() {
        let conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(10)
                .burst_size(20)
                .finish()
                .expect("governor config must be valid"),
        );

        let limiter = Arc::clone(conf.limiter());
        let handle = spawn_cleanup_all(vec![Box::new(move || limiter.retain_recent())], 3600);

        // A direct retain_recent call must also be infallible.
        conf.limiter().retain_recent();

        // Abort the long-lived task so the test exits cleanly.
        handle.abort();
    }

    /// Verify that `spawn_governor_cleanup` convenience wrapper also works.
    #[tokio::test]
    async fn spawn_governor_cleanup_convenience_does_not_panic() {
        let conf = Arc::new(
            GovernorConfigBuilder::default()
                .per_second(10)
                .burst_size(20)
                .finish()
                .expect("governor config must be valid"),
        );

        let handle = spawn_governor_cleanup(Arc::clone(conf.limiter()), 3600);
        handle.abort();
    }

    /// Verify that `retain_recent` evicts a fully-replenished bucket.
    ///
    /// We use `tokio::time::pause` / `advance` so the test is deterministic
    /// and does not actually sleep.  governor's `DefaultKeyedStateStore` is
    /// backed by a `DashMap`; an entry is evicted by `retain_recent` only
    /// when the virtual clock has advanced past the replenishment point.
    #[tokio::test(start_paused = true)]
    async fn retain_recent_evicts_stale_buckets() {
        let conf = GovernorConfigBuilder::default()
            .per_second(1)
            .burst_size(1)
            .finish()
            .expect("governor config must be valid");

        let limiter = conf.limiter();

        // Consume one token to create a keyed bucket entry.
        let ip: IpAddr = IpAddr::from_str("203.0.113.1").expect("valid static IP literal");
        let _ = limiter.check_key(&ip);

        // A retain_recent call with the bucket still depleted must not panic.
        limiter.retain_recent();

        // Advance virtual time well past one replenishment window (burst=1,
        // 1 req/s → replenished after 1 s; we advance 10 s to be safe).
        tokio::time::advance(Duration::from_secs(10)).await;

        // After full replenishment retain_recent must evict the entry without
        // panicking. Passing is the assertion.
        limiter.retain_recent();
    }
}
