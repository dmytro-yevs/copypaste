use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub device_id: String,
    /// Human-readable device name (1-64 characters).
    pub device_name: String,
    /// Base64-standard-encoded X25519 public key (must decode to exactly 32 bytes).
    /// Accepted as both `public_key_b64` (preferred) and `public_key` (legacy alias).
    #[serde(alias = "public_key")]
    pub public_key_b64: String,
    /// Proof-of-possession (PoP): HMAC-SHA256(key=sync_key, msg="relay-registration-pop-v1:" ||
    /// device_id) base64-standard-encoded to exactly 32 raw bytes.
    ///
    /// Security: the relay verifies this field to ensure the registrant holds the sync key
    /// that the `device_id` was derived from. Registration is rejected when this field is
    /// absent, malformed, or (on co-registration) does not match the stored PoP.
    /// See CopyPaste-n2l and `copypaste_core::derive_relay_registration_pop`.
    pub pop_b64: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub device_id: String,
    /// Opaque bearer token the device must use for all subsequent requests.
    pub auth_token: String,
    /// RFC-3339 timestamp after which the token expires (1 year from registration).
    pub expires_at: String,
}

// ---------------------------------------------------------------------------
// Device info
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct DeviceInfoResponse {
    pub device_id: String,
    pub device_name: String,
    pub public_key_b64: String,
    pub registered_at: String,
    pub expires_at: String,
}

// ---------------------------------------------------------------------------
// Items — push (wall-clock sync protocol)
// ---------------------------------------------------------------------------

/// Request body for POST /devices/:id/items
#[derive(Debug, Deserialize)]
pub struct PushRequest {
    /// MIME-style content type: "text", "image", or "file".
    pub content_type: String,
    /// Encrypted payload, base64-standard encoded.
    pub content_b64: String,
    /// Sender wall-clock time (Unix epoch milliseconds).
    pub wall_time: u64,
}

/// Response body for POST /devices/:id/items
#[derive(Debug, Serialize)]
pub struct PushResponse {
    /// Auto-assigned integer ID for the stored item.
    pub id: i64,
}

// ---------------------------------------------------------------------------
// Items — pull (wall-clock sync protocol)
// ---------------------------------------------------------------------------

/// Serialize an `Arc<str>` as a plain JSON string. Used instead of enabling
/// serde's workspace-wide `rc` feature (which would also add Rc/Arc *de*serialize
/// impls we don't want). CopyPaste-ux2i.
fn serialize_arc_str<S>(value: &std::sync::Arc<str>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(value)
}

/// Single item returned by GET /devices/:id/items
#[derive(Debug, Clone, Serialize)]
pub struct PullItem {
    pub id: i64,
    pub content_type: String,
    /// Opaque ciphertext, base64-standard encoded. `Arc<str>` (CopyPaste-ux2i):
    /// the in-memory store holds the same `Arc<str>`, so building a `PullItem`
    /// under the global store mutex is a refcount bump rather than a full heap
    /// copy of the (up to ~13 MiB) base64 string. Serializes as a plain string.
    #[serde(serialize_with = "serialize_arc_str")]
    pub content_b64: std::sync::Arc<str>,
    pub wall_time: u64,
}

/// Query params for GET /devices/:id/items?since=<wall_time>&since_id=<id>&limit=<n>
#[derive(Debug, Deserialize)]
pub struct PullParams {
    /// Return only items past the `(since, since_id)` cursor (defaults to 0).
    #[serde(default)]
    pub since: u64,
    /// Composite-cursor companion to `since`: the `id` of the last item the
    /// client already has at `wall_time == since` (relay H-1 / audit finding G).
    /// Items qualify iff `(wall_time, id) > (since, since_id)`, a strictly
    /// monotonic order with no ties — so a page boundary mid-run of equal
    /// sender-supplied `wall_time` values can no longer silently drop the
    /// remaining tied items. Absent → legacy `wall_time`-only floor
    /// (`wall_time > since`), keeping pre-cursor clients backward-compatible.
    #[serde(default)]
    pub since_id: Option<i64>,
    /// Maximum number of items to return in this page (M4). Absent → server
    /// default (`DEFAULT_PULL_LIMIT`); clamped to `MAX_PULL_LIMIT`. Clients
    /// paginate by passing the last returned `(wall_time, id)` back as
    /// `(since, since_id)`.
    #[serde(default)]
    pub limit: Option<usize>,
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

/// Unauthenticated liveness response. Intentionally contains no device or
/// item counts — those are operational metrics that should not be visible
/// to unauthenticated observers. See CopyPaste-j21 (security hardening).
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
}
