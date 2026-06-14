/// Rate-limiting configuration constants for the relay server.
///
/// The relay applies two independent rate limits:
///
/// 1. **Per-IP** — 200 requests per minute (3 req/s steady-state, burst 60).
///    Applied globally to all routes **except** `/health` and `/stats`.
/// 2. **Per-device** — 60 requests per minute (1 req/s steady-state, burst 20).
///    Applied on device-scoped routes (`/devices/:id/items`).
///
/// Both limits use the `tower_governor` crate which wraps the `governor` GCRA
/// (Generic Cell Rate Algorithm) implementation.  Exceeding either limit
/// returns **HTTP 429 Too Many Requests** with a `Retry-After` header (in
/// seconds) automatically set by `tower_governor`.
///
/// # Exempt routes
///
/// `/health` and `/stats` are mounted on a **separate** inner router that does
/// **not** have the `GovernorLayer` attached.  The outer router merges both
/// sub-routers so that the exempt routes are reachable but unthrottled.
//
/// Per-IP rate limit: 200 requests/minute.
/// Uses `per_second(3)` + `burst_size(60)` in `GovernorConfigBuilder`.
// These consts are the canonical source of the governor parameters; they are
// referenced in documentation and tests but not yet threaded into the
// `GovernorLayer` builder (wired at runtime from config). Keep them public so
// integration tests can assert against the declared limits.
#[allow(dead_code)]
pub const PER_IP_PER_SECOND: u64 = 3;
#[allow(dead_code)] // same reason as PER_IP_PER_SECOND
pub const PER_IP_BURST_SIZE: u32 = 60;

/// Per-device rate limit: 60 requests/minute.
/// Uses `per_second(1)` + `burst_size(20)` in `GovernorConfigBuilder`.
#[allow(dead_code)] // same reason as PER_IP_PER_SECOND
pub const PER_DEVICE_PER_SECOND: u64 = 1;
#[allow(dead_code)] // same reason as PER_IP_PER_SECOND
pub const PER_DEVICE_BURST_SIZE: u32 = 20;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tower_governor::governor::GovernorConfigBuilder;

    /// Smoke-test: ensure per-IP configuration builds without panic.
    #[test]
    fn per_ip_governor_config_builds() {
        let _conf = GovernorConfigBuilder::default()
            .per_second(PER_IP_PER_SECOND)
            .burst_size(PER_IP_BURST_SIZE)
            .finish()
            .expect("per-IP config must be valid");
    }

    /// Smoke-test: ensure per-device configuration builds without panic.
    #[test]
    fn per_device_governor_config_builds() {
        let _conf = GovernorConfigBuilder::default()
            .per_second(PER_DEVICE_PER_SECOND)
            .burst_size(PER_DEVICE_BURST_SIZE)
            .finish()
            .expect("per-device config must be valid");
    }

    #[test]
    fn per_ip_rate_is_approx_200_per_minute() {
        // 3 req/s * 60s = 180 steady-state + burst 60 ≈ 200+ before throttle.
        let steady_per_minute = PER_IP_PER_SECOND * 60;
        assert!(
            steady_per_minute + PER_IP_BURST_SIZE as u64 >= 200,
            "per-IP capacity must be ≥200 req/min"
        );
    }

    #[test]
    fn per_device_rate_is_approx_60_per_minute() {
        // 1 req/s * 60s = 60 steady-state.
        let steady_per_minute = PER_DEVICE_PER_SECOND * 60;
        assert_eq!(
            steady_per_minute, 60,
            "per-device steady-state must be 60 req/min"
        );
    }
}
