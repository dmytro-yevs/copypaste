//! Frontmost-app bundle-id cache (macOS `lsappinfo` TTL cache).
//!
//! CopyPaste-44rq.33: caches the frontmost-app query so `lsappinfo` is not
//! forked on every 500 ms clipboard tick.

/// How long (in milliseconds) the frontmost-app bundle ID obtained from
/// `lsappinfo` is considered fresh.  Caching avoids forking a new subprocess on
/// every 500 ms clipboard tick (CopyPaste-44rq.33).
///
/// CopyPaste-8ebg.57: the original TTL was 2000 ms (4x the default 500 ms tick
/// interval), which meant a copy made shortly after an app switch could reuse
/// a stale bundle ID for up to ~1.5 s, bypassing `excluded_app_bundle_ids` /
/// `is_sensitive_app` for that window. Tightened to 750 ms — just above the
/// default tick period, so at most one extra tick can observe a stale value —
/// while still amortizing the `lsappinfo` fork across consecutive ticks of the
/// same foreground app instead of forking on literally every tick.
#[cfg(target_os = "macos")]
pub(crate) const FRONTMOST_APP_CACHE_TTL_MS: u64 = 750;

/// Per-tick cache for the frontmost application's bundle ID on macOS.
///
/// Populated by `handle_tick` at most once every `FRONTMOST_APP_CACHE_TTL_MS`
/// milliseconds; stale when `expires_at` is in the past.
///
/// `cached_value` is `None` either when `lsappinfo` failed (we cache the
/// failure too so we do not retry on the very next tick) or when the cache has
/// not been primed yet.  `is_failure` distinguishes "not yet primed" from "we
/// tried and lsappinfo returned nothing", which matters for the P1-2 fail-closed
/// gate.
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub(crate) struct FrontmostAppCache {
    /// The cached bundle ID (None when lsappinfo failed or cache not yet primed).
    pub(crate) cached_value: Option<String>,
    /// Whether the last lsappinfo invocation failed (vs. cache simply being cold).
    pub(crate) is_failure: bool,
    /// When the cache entry expires and must be refreshed.
    pub(crate) expires_at: std::time::Instant,
}

#[cfg(target_os = "macos")]
impl FrontmostAppCache {
    /// Create a new, already-expired cache so the first tick always populates it.
    pub(crate) fn new() -> Self {
        Self {
            cached_value: None,
            is_failure: false,
            // Subtract 1 s so the cache is considered expired on the first call.
            expires_at: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                // `checked_sub` can only fail if Instant::now() is within 1 s of
                // the monotonic clock epoch, which is impossible in practice.
                .unwrap_or_else(std::time::Instant::now),
        }
    }

    /// Returns `true` if the cached value is still within the TTL window.
    pub(crate) fn is_fresh(&self) -> bool {
        std::time::Instant::now() < self.expires_at
    }
}

/// Parse the bundleID field out of `lsappinfo front` output.
///
/// Extracted from the inline closure that used to live in `handle_tick` so it
/// can be unit-tested without forking a real subprocess. Looks for a line of
/// the form: `"bundleID" = "com.example.app"`.
#[cfg(target_os = "macos")]
fn parse_lsappinfo_bundle_id(text: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim();
        // Match: "bundleID" = "com.example.app"
        if let Some(rest) = trimmed.strip_prefix("\"bundleID\" = \"") {
            if let Some(bid) = rest.strip_suffix('"') {
                return Some(bid.to_owned());
            }
        }
    }
    None
}

/// Resolve the frontmost application's bundle ID, using `cache` to avoid
/// forking `lsappinfo` more often than [`FRONTMOST_APP_CACHE_TTL_MS`].
///
/// Shared between the exclusion check and the `is_sensitive_app` check in
/// `handle_tick` so lsappinfo is invoked AT MOST ONCE per TTL window
/// regardless of which check fires. See the `handle_tick` doc comments
/// (moved to `tick.rs`) for the full P1-2/P1-3/44rq.43 rationale — this
/// function's behavior is unchanged from the code formerly inlined there.
#[cfg(target_os = "macos")]
pub(crate) async fn resolve_frontmost_bundle_id(cache: &mut FrontmostAppCache) -> Option<String> {
    if cache.is_fresh() {
        // Cache hit: reuse the previously-resolved bundle ID (may be None if
        // lsappinfo failed on the last refresh — we cache failures too so we
        // do not hammer the subprocess on every tick during a transient error).
        tracing::trace!(
            cached = ?cache.cached_value,
            "lsappinfo: cache hit — skipping subprocess"
        );
        return cache.cached_value.clone();
    }

    // Cache miss (cold or expired): spawn lsappinfo and populate the cache.
    let lsappinfo_result = tokio::task::spawn_blocking(|| {
        // `lsappinfo front` prints a record for the frontmost process.
        // We extract the bundleID field from lines like:
        //   "bundleID" = "com.1password.1password"
        std::process::Command::new("lsappinfo")
            .args(["front"])
            .output()
            .ok()
            .and_then(|out| {
                let text = String::from_utf8_lossy(&out.stdout).into_owned();
                parse_lsappinfo_bundle_id(&text)
            })
    })
    .await;

    // Flatten the JoinError and inner Option, then populate the cache.
    let resolved = match lsappinfo_result {
        Ok(opt) => {
            cache.is_failure = opt.is_none();
            opt
        }
        Err(join_err) => {
            // spawn_blocking task panicked — treat as subprocess failure.
            tracing::warn!(
                error = %join_err,
                "lsappinfo: blocking task panicked; failing closed to protect excluded apps"
            );
            cache.is_failure = true;
            None
        }
    };

    // Store the result (success or failure) and set the expiry so we
    // do not fork again until FRONTMOST_APP_CACHE_TTL_MS have elapsed.
    cache.cached_value = resolved.clone();
    cache.expires_at =
        std::time::Instant::now() + std::time::Duration::from_millis(FRONTMOST_APP_CACHE_TTL_MS);
    tracing::trace!(
        bundle_id = ?resolved,
        ttl_ms = FRONTMOST_APP_CACHE_TTL_MS,
        "lsappinfo: cache refreshed"
    );

    resolved
}

#[cfg(test)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // CopyPaste-44rq.33: FrontmostAppCache TTL behaviour
    // -----------------------------------------------------------------------

    /// Verifies `FrontmostAppCache` TTL logic: a newly-created cache is cold
    /// (not fresh), a populated cache within the TTL is fresh, and a cache
    /// with an `expires_at` in the past is stale.
    ///
    /// This test does NOT spawn lsappinfo — it exercises only the cache
    /// bookkeeping (TTL arithmetic) that wraps the subprocess, which is the
    /// correctness-critical part of the CopyPaste-44rq.33 fix.
    #[cfg(target_os = "macos")]
    #[test]
    fn frontmost_app_cache_ttl_logic() {
        // Cold cache (constructed via `new()`) must be stale so the first
        // call to handle_tick always refreshes it.
        let cache = FrontmostAppCache::new();
        assert!(
            !cache.is_fresh(),
            "newly-created cache must not be fresh (forces first-tick refresh)"
        );
        assert!(
            cache.cached_value.is_none(),
            "newly-created cache must have no value"
        );
        assert!(
            !cache.is_failure,
            "newly-created cache must not be marked as a failure"
        );

        // A cache populated with a future expiry must be reported as fresh.
        let mut hot_cache = FrontmostAppCache {
            cached_value: Some("com.apple.finder".to_string()),
            is_failure: false,
            expires_at: std::time::Instant::now()
                + std::time::Duration::from_millis(FRONTMOST_APP_CACHE_TTL_MS),
        };
        assert!(
            hot_cache.is_fresh(),
            "cache with future expiry must be fresh"
        );
        assert_eq!(
            hot_cache.cached_value.as_deref(),
            Some("com.apple.finder"),
            "cached bundle ID must be returned unchanged"
        );

        // Simulate TTL expiry by back-dating expires_at.
        hot_cache.expires_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(1))
            // Impossible in practice; fall back to a fresh-but-immediate expiry.
            .unwrap_or_else(std::time::Instant::now);
        assert!(
            !hot_cache.is_fresh(),
            "cache with past expiry must not be fresh (must trigger refresh)"
        );

        // Failure result (lsappinfo returned None) must be cached too.
        let failure_cache = FrontmostAppCache {
            cached_value: None,
            is_failure: true,
            expires_at: std::time::Instant::now()
                + std::time::Duration::from_millis(FRONTMOST_APP_CACHE_TTL_MS),
        };
        assert!(
            failure_cache.is_fresh(),
            "a cached failure within the TTL must still be considered fresh \
             so we do not re-spawn lsappinfo on every tick during a transient error"
        );
        assert!(
            failure_cache.is_failure,
            "is_failure flag must be preserved"
        );
        assert!(
            failure_cache.cached_value.is_none(),
            "cached_value must be None for a cached failure"
        );
    }

    /// Characterization test (CopyPaste-vp63.11 split): pins the bundleID
    /// extraction contract that `resolve_frontmost_bundle_id` depends on.
    #[cfg(target_os = "macos")]
    #[test]
    fn parse_lsappinfo_bundle_id_extracts_bundle_id() {
        let sample =
            "ASN Base=0x...\n  \"bundleID\" = \"com.1password.1password\"\n  \"pid\" = 123\n";
        assert_eq!(
            parse_lsappinfo_bundle_id(sample),
            Some("com.1password.1password".to_string())
        );
    }

    /// Fail-closed classification: unparsable / unexpected lsappinfo output
    /// must yield `None`, never a panic or a bogus bundle id.
    #[cfg(target_os = "macos")]
    #[test]
    fn parse_lsappinfo_bundle_id_none_on_unrecognised_output() {
        assert_eq!(parse_lsappinfo_bundle_id(""), None);
        assert_eq!(
            parse_lsappinfo_bundle_id("garbage, no bundle id here"),
            None
        );
    }
}
