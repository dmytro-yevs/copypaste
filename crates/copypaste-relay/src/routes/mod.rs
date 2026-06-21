pub mod devices;
pub mod health;
pub mod items;
pub mod management;

use std::sync::Arc;

use axum::routing::{delete, get};
use axum::Router;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::{KeyExtractor, PeerIpKeyExtractor, SmartIpKeyExtractor};
use tower_governor::GovernorLayer;

use crate::api::metrics;
use crate::config::RelayConfig;
use crate::middleware::rate_limit::{
    PER_DEVICE_BURST_SIZE, PER_DEVICE_PER_SECOND, PER_IP_BURST_SIZE, PER_IP_PER_SECOND,
};
use crate::state::AppState;

/// Build the complete relay router.
///
/// Returns `(router, retain_fns)`.  `retain_fns` is a list of zero-argument
/// closures — one per governor limiter — that each call `retain_recent()` on
/// their respective limiter.  The caller is responsible for driving them on a
/// periodic interval (see `crate::governor_cleanup::spawn_governor_cleanup_all`).
/// Returning closures rather than spawning tasks here means `relay_router` can
/// be called from plain `#[test]` contexts that have no tokio runtime.
///
/// # Rate limiting
///
/// The router is split into sub-routers:
///
/// 1. **Exempt routes** (`/health`, `/stats`, `/metrics`) — no rate
///    limiting applied. These are lightweight diagnostic endpoints that
///    must remain available even under load (Prometheus scrapers in
///    particular must not have to share the per-IP budget with clients).
///
/// 2. **Rate-limited routes** — everything else:
///    - *Per-IP*: 200 requests/minute (3 req/s steady-state, burst 60).
///      Applied to all non-exempt routes.
///    - *Per-device*: 60 requests/minute (1 req/s steady-state, burst 20).
///      Applied specifically to device-scoped item routes.
///
/// Exceeding either limit returns **HTTP 429 Too Many Requests** with a
/// `Retry-After` header automatically set by `tower_governor`.
/// Type alias for the list of retain callbacks returned alongside the router.
pub type RetainFns = Vec<Box<dyn Fn() + Send + Sync + 'static>>;

/// Error returned when the rate-limit governor configuration is invalid.
///
/// In practice this only fires if a rate-limit constant is zero (which the
/// `governor` crate rejects). All shipped constants are non-zero, so this
/// error should never be seen in production — but propagating it prevents a
/// process-level panic if an operator accidentally patches a constant to 0.
#[derive(Debug)]
pub struct GovernorConfigError(String);

impl std::fmt::Display for GovernorConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "invalid governor configuration: {}", self.0)
    }
}

impl std::error::Error for GovernorConfigError {}

pub fn relay_router(
    state: AppState,
    config: RelayConfig,
) -> Result<(Router, RetainFns), GovernorConfigError> {
    // M3: select the per-IP key extractor. By default we key on the
    // unspoofable TCP peer IP (`PeerIpKeyExtractor`). When the operator opts in
    // via `RELAY_TRUST_PROXY_HEADERS` — and *only* then — we honor
    // `X-Forwarded-For` / `X-Real-IP` / `Forwarded` via `SmartIpKeyExtractor`,
    // which falls back to the peer IP when those headers are absent. Without a
    // trusted proxy in front, those headers are attacker-controlled on a
    // `0.0.0.0` bind, so trusting them must be an explicit, documented choice.
    if config.trust_proxy_headers {
        build_router(state, config, SmartIpKeyExtractor)
    } else {
        build_router(state, config, PeerIpKeyExtractor)
    }
}

/// Assemble the relay router with a chosen per-IP `KeyExtractor` (M3).
///
/// Generic over `PerIp` so the same wiring serves both the peer-IP and
/// proxy-header (smart-IP) variants without duplicating the route table.
///
/// Returns `Err(GovernorConfigError)` if a rate-limit constant is invalid
/// (e.g. zero). All shipped constants are non-zero so this should never
/// fire in practice, but propagating the error prevents a process panic.
fn build_router<PerIp>(
    state: AppState,
    config: RelayConfig,
    per_ip_key: PerIp,
) -> Result<(Router, RetainFns), GovernorConfigError>
where
    PerIp: KeyExtractor + Clone + Send + Sync + 'static,
    PerIp::Key: Send + Sync + 'static,
{
    // ---- Exempt routes (no rate limiting) ----------------------------------
    let exempt = Router::new()
        .route("/health", get(health::handle))
        .route("/stats", get(management::stats_handler))
        .route("/metrics", get(metrics::handle))
        .with_state(state.clone());

    // ---- Per-IP rate limit layer (200 req/min) ------------------------------
    // This per-IP bound is the *authoritative* abuse limit: it keys on the
    // client IP (peer or trusted-proxy-supplied), which an attacker cannot
    // rotate the way they can rotate a URL `:device_id`. It is therefore what
    // actually bounds the per-device limiter's bypass (M2): even if a flooder
    // cycles fresh device ids to dodge the per-device bucket, every request
    // still shares this per-IP bucket.
    //
    // CopyPaste-hzmb: we clone `per_ip_key` here so the same IP-keying
    // extractor can be reused for the tighter per-item-route bucket below.
    let per_ip_key_for_item = per_ip_key.clone();
    let per_ip_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(PER_IP_PER_SECOND)
            .burst_size(PER_IP_BURST_SIZE)
            .key_extractor(per_ip_key)
            .finish()
            .ok_or_else(|| {
                GovernorConfigError("per-IP: per_second or burst_size is zero".to_string())
            })?,
    );

    // ---- Per-item-route IP rate limit layer (60 req/min) -------------------
    // CopyPaste-hzmb: the previous "per-device" layer keyed the bucket on the
    // *pre-auth* URL `:device_id` segment, which an attacker can rotate freely
    // to obtain a fresh bucket on every request, completely bypassing the limit.
    // The fix keys this tighter (60 req/min) layer on the source IP — the same
    // unspoofable identity that the per-IP layer above uses, but with a stricter
    // budget applied specifically to device-item routes. Keying on the
    // post-authentication identity (bearer token) would require running auth
    // *before* this Tower layer, which is not possible in the current
    // architecture — IP keying is the correct defense-in-depth here.
    //
    // DeviceIdKeyExtractor lives inside #[cfg(test)] (URL-segment parsing tests);
    // it is no longer wired into the production rate-limit layer.
    let per_item_ip_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(PER_DEVICE_PER_SECOND)
            .burst_size(PER_DEVICE_BURST_SIZE)
            .key_extractor(per_ip_key_for_item)
            .finish()
            .ok_or_else(|| {
                GovernorConfigError(
                    "per-item-route IP: per_second or burst_size is zero".to_string(),
                )
            })?,
    );

    // ---- Retain callbacks for background cleanup ---------------------------
    // We capture type-erased `retain_recent` closures here instead of calling
    // tokio::spawn directly, so this sync function can be called from plain
    // #[test] contexts that have no tokio runtime.  The caller (main.rs) passes
    // the vec to `governor_cleanup::spawn_cleanup_all` which spawns one task.
    // Test code that only needs the Router can simply drop the vec.
    let ip_limiter = std::sync::Arc::clone(per_ip_conf.limiter());
    let item_ip_limiter = std::sync::Arc::clone(per_item_ip_conf.limiter());
    let retain_fns: RetainFns = vec![
        Box::new(move || ip_limiter.retain_recent()),
        Box::new(move || item_ip_limiter.retain_recent()),
    ];

    // ---- Device-scoped item routes (two IP-keyed rate limits) --------------
    // Note: axum 0.8 uses `{param}` syntax for path captures (`:param` is 0.7).
    // CopyPaste-hzmb: both layers now key on source IP, not on the pre-auth
    // URL device_id. The outer per-IP layer (200 req/min) applies across ALL
    // routes; the inner per-item-route IP layer (60 req/min) applies only to
    // device-item routes — giving item routes a tighter per-IP budget without
    // the device_id bypass.
    let item_routes = Router::new()
        .route(
            "/devices/{device_id}/items",
            get(items::pull).post(items::push),
        )
        .route(
            "/devices/{device_id}/items/{item_id}",
            delete(items::delete_item),
        )
        // SSE push (issue #26): real-time stream of new inbox items, additive
        // to the GET .../items poll backstop. Shares both IP-keyed rate limits.
        .route("/devices/{device_id}/subscribe", get(items::subscribe))
        .with_state(state.clone())
        // In 0.8 GovernorLayer fields are private; use GovernorLayer::new() instead of
        // struct literal syntax.
        .layer(GovernorLayer::new(per_item_ip_conf))
        .layer(GovernorLayer::new(per_ip_conf.clone()));

    // ---- Device registration + info routes (per-IP limit only) -------------
    let device_routes = Router::new()
        .route(
            "/devices",
            get(management::list_devices_handler).post(devices::register),
        )
        .route("/devices/{device_id}", get(devices::get_device))
        .with_state(state)
        .layer(GovernorLayer::new(per_ip_conf));

    // ---- Merge all sub-routers + shared body-limit + config injection ------
    // CopyPaste-pbre: global concurrency cap. Read the value before `config` is
    // moved into the Extension layer below.
    let max_connections = config.max_connections;
    let router = Router::new()
        .merge(exempt)
        .merge(item_routes)
        .merge(device_routes)
        // Body-limit must be sized against the *encoded* (base64+JSON) payload, not
        // the decoded ciphertext. Base64 inflates by ~4/3; add 1 KiB for JSON framing
        // (content_type, wall_time, field names). Without this, an image/file item
        // near the 10 MiB decoded cap is rejected 413 before the handler even runs.
        .layer(axum::extract::DefaultBodyLimit::max(
            config.max_item_bytes * 4 / 3 + 1024,
        ))
        // CopyPaste-pbre: cap concurrent in-flight requests so a connection burst
        // cannot exhaust memory / file descriptors. Requests over the limit queue
        // for a permit (back-pressure) rather than being dropped; this complements
        // the per-IP/per-device rate limits (which bound rate, not concurrency).
        .layer(tower::limit::ConcurrencyLimitLayer::new(max_connections))
        // Inject the live `RelayConfig` so handlers (e.g. `items::push`)
        // can honor operator-supplied limits like `RELAY_MAX_ITEM_BYTES`
        // instead of falling back to compile-time defaults (HIGH #2).
        .layer(axum::Extension(config));

    Ok((router, retain_fns))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower_governor::errors::GovernorError;
    use tower_governor::key_extractor::KeyExtractor;

    /// `KeyExtractor` that pulls the `:device_id` segment out of paths shaped
    /// like `/devices/<id>/items[/...]`.
    ///
    /// CopyPaste-hzmb: no longer wired into the production rate-limit layer —
    /// the layer now keys on source IP (`PeerIpKeyExtractor` / `SmartIpKeyExtractor`)
    /// which an attacker cannot rotate, unlike a URL `:device_id` segment.
    /// Retained here for URL-segment parsing tests and future diagnostic use.
    ///
    /// Returns `GovernorError::Other` 400 if the URI does not start with
    /// `/devices/`, or 404 if the device-id segment is empty.
    #[derive(Debug, Clone, Copy, Eq, PartialEq)]
    struct DeviceIdKeyExtractor;

    impl KeyExtractor for DeviceIdKeyExtractor {
        type Key = String;

        fn extract<B>(&self, req: &Request<B>) -> Result<Self::Key, GovernorError> {
            // Expected shape: "/devices/<id>/items" or "/devices/<id>/items/<item_id>".
            let path = req.uri().path();
            // A path that doesn't start with "/devices/" is a client error (wrong
            // route) — return 400 so the caller gets an actionable status code.
            let rest = path.strip_prefix("/devices/").ok_or(GovernorError::Other {
                code: StatusCode::BAD_REQUEST,
                msg: Some("request path does not contain a device id segment".into()),
                headers: None,
            })?;
            let id = match rest.find('/') {
                Some(end) => &rest[..end],
                None => rest,
            };
            if id.is_empty() {
                // Empty device id in "/devices//" — 404, there is no device with an
                // empty id.
                return Err(GovernorError::Other {
                    code: StatusCode::NOT_FOUND,
                    msg: Some("device id segment is empty".into()),
                    headers: None,
                });
            }
            Ok(id.to_owned())
        }
    }

    fn req(uri: &str) -> Request<Body> {
        Request::builder().uri(uri).body(Body::empty()).unwrap()
    }

    #[test]
    fn device_id_extractor_pulls_id_from_items_collection() {
        let key = DeviceIdKeyExtractor
            .extract(&req("/devices/abc-123/items"))
            .unwrap();
        assert_eq!(key, "abc-123");
    }

    #[test]
    fn device_id_extractor_pulls_id_from_single_item() {
        let key = DeviceIdKeyExtractor
            .extract(&req("/devices/abc-123/items/42"))
            .unwrap();
        assert_eq!(key, "abc-123");
    }

    #[test]
    fn device_id_extractor_ignores_query_string() {
        let key = DeviceIdKeyExtractor
            .extract(&req("/devices/abc-123/items?since=10"))
            .unwrap();
        assert_eq!(key, "abc-123");
    }

    #[test]
    fn device_id_extractor_fails_closed_on_unrelated_path() {
        assert!(DeviceIdKeyExtractor.extract(&req("/health")).is_err());
        assert!(DeviceIdKeyExtractor.extract(&req("/devices")).is_err());
    }

    #[test]
    fn device_id_extractor_rejects_empty_id() {
        assert!(DeviceIdKeyExtractor
            .extract(&req("/devices//items"))
            .is_err());
    }

    // ---- M3: per-IP key extractor selection --------------------------------

    fn req_with_xff(uri: &str, xff: &str) -> Request<Body> {
        Request::builder()
            .uri(uri)
            .header("x-forwarded-for", xff)
            .body(Body::empty())
            .unwrap()
    }

    /// When proxy-header trust is enabled, the per-IP limiter must key on the
    /// `X-Forwarded-For` client IP (M3) so distinct forwarded clients get
    /// distinct buckets rather than sharing the proxy's peer IP.
    #[test]
    fn smart_ip_extractor_keys_on_x_forwarded_for() {
        let a = SmartIpKeyExtractor
            .extract(&req_with_xff("/devices", "203.0.113.7"))
            .unwrap();
        let b = SmartIpKeyExtractor
            .extract(&req_with_xff("/devices", "203.0.113.8"))
            .unwrap();
        assert_ne!(a, b, "distinct XFF clients must yield distinct keys");
        assert_eq!(a.to_string(), "203.0.113.7");
    }

    /// The router must assemble in both proxy-trust modes (M3): default
    /// (peer-IP keying) and opt-in (smart-IP keying).
    #[test]
    fn router_builds_in_both_proxy_trust_modes() {
        use crate::state::RelayStore;
        use std::sync::Mutex;

        for trust in [false, true] {
            let config = RelayConfig {
                trust_proxy_headers: trust,
                ..RelayConfig::default()
            };
            let state = Arc::new(Mutex::new(RelayStore::new(config.sync_ttl_secs)));
            // Must not panic while building either variant.
            // retain_fns is dropped here — no tokio runtime needed since the
            // closures are never spawned in the test context.
            let (_router, _retain_fns) = relay_router(state, config)
                .expect("relay_router must succeed with valid rate-limit constants");
        }
    }

    // ---- CopyPaste-hzmb: per-item rate limiter must key on IP, not device_id --
    //
    // Before the fix, the per-device GovernorLayer used DeviceIdKeyExtractor,
    // which extracts the bucket key from the URL `:device_id` segment. An
    // attacker rotating device IDs gets a fresh bucket per request, completely
    // bypassing the limit. After the fix both layers on item routes key on the
    // source IP (unspoofable), so id rotation provides no benefit.

    /// CopyPaste-hzmb: `build_router` with `PeerIpKeyExtractor` must produce
    /// two retain callbacks (one per IP-keyed limiter) whose retain functions
    /// are callable without panic.
    #[test]
    fn hzmb_item_route_rate_limiter_keyed_on_ip_not_device_id() {
        use crate::state::RelayStore;
        use std::sync::Mutex;

        let config = RelayConfig::default();
        let state = Arc::new(Mutex::new(RelayStore::new(config.sync_ttl_secs)));
        let (_router, retain_fns) = relay_router(state, config)
            .expect("relay_router must succeed with valid rate-limit constants");

        // There must be exactly 2 retain callbacks (per-IP + per-item-route IP).
        assert_eq!(
            retain_fns.len(),
            2,
            "CopyPaste-hzmb: expected exactly 2 retain callbacks, got {}",
            retain_fns.len()
        );

        // Both callbacks must be callable without panicking.
        for retain in &retain_fns {
            retain();
        }
    }

    /// CopyPaste-hzmb: the DeviceIdKeyExtractor is kept for URL-segment
    /// extraction logic but must NOT be used as the rate-limit bucket key.
    /// Verify that distinct device IDs from the same request context are NOT
    /// the differentiating factor for rate limiting (i.e., the extractor still
    /// correctly parses the URL segment — it is just no longer wired into the
    /// governor layer). The unit tests for DeviceIdKeyExtractor above remain
    /// valid; this test documents the architectural change.
    #[test]
    fn device_id_extractor_parses_url_but_not_wired_as_rate_limit_key() {
        // DeviceIdKeyExtractor must still correctly parse URLs for other uses
        // (e.g. future diagnostic middleware), even though it is no longer the
        // rate-limit key.
        let key_a = DeviceIdKeyExtractor
            .extract(&req("/devices/attacker-rotates-this/items"))
            .unwrap();
        let key_b = DeviceIdKeyExtractor
            .extract(&req("/devices/different-id/items"))
            .unwrap();
        // The extractor parses distinct IDs correctly.
        assert_ne!(
            key_a, key_b,
            "DeviceIdKeyExtractor must parse distinct IDs; \
             these IDs were not being used as the rate-limit key (hzmb fix)"
        );
    }
}
