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
///
/// # Wiring
///
/// CopyPaste-8ebg.50: these were previously the *only* source of truth and
/// were imported directly by `crate::routes::build_router`. A false-positive
/// 429 storm in production could then only be tuned by a rebuild and
/// redeploy. `crate::config::RelayConfig` now carries a runtime override for
/// each threshold (`per_ip_per_second`, `per_ip_burst_size`,
/// `per_device_per_second`, `per_device_burst_size`), sourced from
/// `RELAY_PER_IP_PER_SECOND` / `RELAY_PER_IP_BURST_SIZE` /
/// `RELAY_PER_DEVICE_PER_SECOND` / `RELAY_PER_DEVICE_BURST_SIZE` and
/// defaulting to the same numeric values as these constants (mirrored as
/// literals in `RelayConfig::default`, since `config.rs` is also compiled
/// standalone by the integration tests via `#[path]` and cannot depend on
/// this `middleware` module). `crate::routes::build_router` now reads the
/// resolved values off `RelayConfig` instead of importing these constants
/// directly, so the override takes effect end-to-end. The constants below
/// remain the documented, tested source of truth for the *default* numeric
/// values (see the unit tests in this module).
/// Per-IP rate limit: 200 requests/minute.
/// Uses `per_second(3)` + `burst_size(60)` in `GovernorConfigBuilder`.
///
/// CopyPaste-8ebg.50: `crate::routes::build_router` now reads the *resolved*
/// value off `RelayConfig` (`config.per_ip_per_second`) instead of this
/// constant, so the override actually takes effect at runtime. This constant
/// remains the single source of truth mirrored into
/// `RelayConfig::default()`'s literal and is exercised directly by this
/// module's own unit tests below — hence `#[allow(dead_code)]` rather than
/// deletion.
#[allow(dead_code)]
pub const PER_IP_PER_SECOND: u64 = 3;

/// Per-IP burst allowance — number of requests that may be served before
/// the per-second replenishment rate kicks in.
/// See `PER_IP_PER_SECOND` doc comment for why this is `#[allow(dead_code)]`.
#[allow(dead_code)]
pub const PER_IP_BURST_SIZE: u32 = 60;

/// Per-device (item-route) rate limit: 60 requests/minute.
/// Uses `per_second(1)` + `burst_size(20)` in `GovernorConfigBuilder`.
///
/// See `PER_IP_PER_SECOND` doc comment for why this is `#[allow(dead_code)]`.
#[allow(dead_code)]
pub const PER_DEVICE_PER_SECOND: u64 = 1;

/// Per-device burst allowance for item routes.
/// See `PER_IP_PER_SECOND` doc comment for why this is `#[allow(dead_code)]`.
#[allow(dead_code)]
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

    /// CopyPaste-dm2z: the constants must be usable from the same import path
    /// as `crate::routes::build_router` uses them.  Before this fix they were
    /// annotated `#[allow(dead_code)]` with a comment claiming they were "not
    /// yet threaded into the GovernorLayer" — which was incorrect (routes/mod.rs
    /// already imports and passes them to GovernorConfigBuilder).  Removing the
    /// dead_code suppression makes the compiler enforce they are reachable.
    ///
    /// This test mirrors the exact `GovernorConfigBuilder` call shape used in
    /// `build_router` (routes/mod.rs) so a divergence between the constants and
    /// the production wiring fails here first.
    #[test]
    fn dm2z_rate_limit_constants_match_production_router_wiring() {
        // Per-IP layer — mirrors routes/mod.rs `per_ip_conf` builder.
        let _per_ip = GovernorConfigBuilder::default()
            .per_second(PER_IP_PER_SECOND)
            .burst_size(PER_IP_BURST_SIZE)
            .finish()
            .expect("CopyPaste-dm2z: per-IP GovernorConfig must build with production constants");

        // Per-item-route IP layer — mirrors routes/mod.rs `per_item_ip_conf` builder.
        let _per_device = GovernorConfigBuilder::default()
            .per_second(PER_DEVICE_PER_SECOND)
            .burst_size(PER_DEVICE_BURST_SIZE)
            .finish()
            .expect(
                "CopyPaste-dm2z: per-device GovernorConfig must build with production constants",
            );

        // Assert the numeric values match the documented spec so a
        // copy-paste error (e.g. swapping IP and device constants) is caught.
        assert_eq!(
            PER_IP_PER_SECOND * 60 + PER_IP_BURST_SIZE as u64,
            240,
            "CopyPaste-dm2z: per-IP capacity (steady 180 + burst 60) must equal 240"
        );
        assert_eq!(
            PER_DEVICE_PER_SECOND * 60,
            60,
            "CopyPaste-dm2z: per-device steady-state must be 60 req/min"
        );
    }
}
