pub mod devices;
pub mod health;
pub mod items;

use std::sync::Arc;

use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{delete, get};
use axum::Router;
use tower_governor::errors::GovernorError;
use tower_governor::governor::GovernorConfigBuilder;
use tower_governor::key_extractor::{KeyExtractor, PeerIpKeyExtractor, SmartIpKeyExtractor};
use tower_governor::GovernorLayer;

use crate::api::metrics;
use crate::config::RelayConfig;
use crate::middleware::rate_limit::{
    PER_DEVICE_BURST_SIZE, PER_DEVICE_PER_SECOND, PER_IP_BURST_SIZE, PER_IP_PER_SECOND,
};
use crate::state::AppState;

/// `KeyExtractor` that pulls the `:device_id` segment out of paths shaped like
/// `/devices/<id>/items[/...]`. Used by the per-device `GovernorLayer` so the
/// rate limit is genuinely per-device (HIGH #4) — `PeerIpKeyExtractor` keyed
/// the bucket by client IP, which means a single NAT'd network shared one
/// per-device bucket while a single attacker on many IPs got a fresh bucket
/// per IP. Both directions of that error are now closed.
///
/// Returns a `GovernorError::Other` with 400 BAD_REQUEST if the URI does not
/// start with `/devices/`, or 404 NOT_FOUND if the device id segment is empty.
/// In practice neither case arises because this extractor is only attached to
/// the `item_routes` sub-router, but explicit error codes are returned so that
/// misdirected requests produce actionable status codes rather than 500.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct DeviceIdKeyExtractor;

impl KeyExtractor for DeviceIdKeyExtractor {
    type Key = String;

    fn extract<B>(&self, req: &Request<B>) -> Result<Self::Key, GovernorError> {
        // Expected shape: "/devices/<id>/items" or "/devices/<id>/items/<item_id>".
        // No allocation in the happy path beyond the returned `String`.
        let path = req.uri().path();
        // A path that doesn't start with "/devices/" is a client error (wrong
        // route), not a server fault — return 400 rather than 500 so the caller
        // gets an actionable status code and monitoring doesn't fire server-error
        // alerts for misdirected requests.
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
            // Empty device id in a well-formed "/devices//" path — 404 because
            // there is no device with an empty id, and the caller can determine
            // what went wrong from the error message.
            return Err(GovernorError::Other {
                code: StatusCode::NOT_FOUND,
                msg: Some("device id segment is empty".into()),
                headers: None,
            });
        }
        Ok(id.to_owned())
    }
}

/// Build the complete relay router.
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
pub fn relay_router(state: AppState, config: RelayConfig) -> Router {
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
fn build_router<PerIp>(state: AppState, config: RelayConfig, per_ip_key: PerIp) -> Router
where
    PerIp: KeyExtractor + Send + Sync + 'static,
    PerIp::Key: Send + Sync + 'static,
{
    // ---- Exempt routes (no rate limiting) ----------------------------------
    let exempt = Router::new()
        .route("/health", get(health::handle))
        .route("/stats", get(stats_handler))
        .route("/metrics", get(metrics::handle))
        .with_state(state.clone());

    // ---- Per-IP rate limit layer (200 req/min) ------------------------------
    // This per-IP bound is the *authoritative* abuse limit: it keys on the
    // client IP (peer or trusted-proxy-supplied), which an attacker cannot
    // rotate the way they can rotate a URL `:device_id`. It is therefore what
    // actually bounds the per-device limiter's bypass (M2): even if a flooder
    // cycles fresh device ids to dodge the per-device bucket, every request
    // still shares this per-IP bucket.
    let per_ip_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(PER_IP_PER_SECOND)
            .burst_size(PER_IP_BURST_SIZE)
            .key_extractor(per_ip_key)
            .finish()
            .expect("invalid per-IP governor configuration"),
    );

    // ---- Per-device rate limit layer (60 req/min) ---------------------------
    // HIGH #4: key the bucket by the `:device_id` URL segment via a custom
    // `KeyExtractor`. The previous default keyed by peer IP, so this layer
    // was effectively a *second* per-IP limit, not a per-device one.
    //
    // M2: this `:device_id` is the *pre-auth* URL segment, so a flooder can
    // rotate ids to get a fresh per-device bucket each time. That is acceptable
    // because this layer is defense-in-depth only — the per-IP layer above
    // (applied to the same routes) bounds id-rotation abuse since the source IP
    // cannot be rotated. Keying on the authenticated identity instead would
    // require running auth before the layer (the bearer token is verified
    // inside the handler), which the tower layer stack cannot do here.
    let per_device_conf = Arc::new(
        GovernorConfigBuilder::default()
            .per_second(PER_DEVICE_PER_SECOND)
            .burst_size(PER_DEVICE_BURST_SIZE)
            .key_extractor(DeviceIdKeyExtractor)
            .finish()
            .expect("invalid per-device governor configuration"),
    );

    // ---- Device-scoped item routes (per-device + per-IP limits) ------------
    // Note: axum 0.8 uses `{param}` syntax for path captures (`:param` is 0.7).
    let item_routes = Router::new()
        .route(
            "/devices/{device_id}/items",
            get(items::pull).post(items::push),
        )
        .route(
            "/devices/{device_id}/items/{item_id}",
            delete(items::delete_item),
        )
        .with_state(state.clone())
        // In 0.8 GovernorLayer fields are private; use GovernorLayer::new() instead of
        // struct literal syntax.
        .layer(GovernorLayer::new(per_device_conf))
        .layer(GovernorLayer::new(per_ip_conf.clone()));

    // ---- Device registration + info routes (per-IP limit only) -------------
    let device_routes = Router::new()
        .route(
            "/devices",
            get(list_devices_handler).post(devices::register),
        )
        .route("/devices/{device_id}", get(devices::get_device))
        .with_state(state)
        .layer(GovernorLayer::new(per_ip_conf));

    // ---- Merge all sub-routers + shared body-limit + config injection ------
    Router::new()
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
        // Inject the live `RelayConfig` so handlers (e.g. `items::push`)
        // can honor operator-supplied limits like `RELAY_MAX_ITEM_BYTES`
        // instead of falling back to compile-time defaults (HIGH #2).
        .layer(axum::Extension(config))
}

async fn stats_handler(State(state): State<AppState>) -> impl IntoResponse {
    // Survive mutex poisoning (security INFO #21).
    let store = state.lock().unwrap_or_else(|e| e.into_inner());
    let (devices, items) = store.stats();
    axum::Json(serde_json::json!({
        "devices": devices,
        "total_items": items,
        "version": "2"
    }))
}

/// GET /devices — list registered device IDs only.
///
/// Returns only opaque device IDs. Bearer tokens are **never** included
/// (they would let anyone hijack the device). Other public fields like
/// `public_key_b64` are exposed via the per-device endpoint `GET /devices/:id`.
async fn list_devices_handler(State(state): State<AppState>) -> impl IntoResponse {
    // Survive mutex poisoning (security INFO #21).
    let store = state.lock().unwrap_or_else(|e| e.into_inner());
    let device_ids = store.list_devices();
    axum::Json(serde_json::json!({ "devices": device_ids }))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;

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
            let _router = relay_router(state, config);
        }
    }
}
