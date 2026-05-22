use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub device_id: String,
    /// Human-readable device name (1–64 characters).
    pub device_name: String,
    /// Base64-standard-encoded X25519 public key (must decode to exactly 32 bytes).
    /// Accepted as both `public_key_b64` (preferred) and `public_key` (legacy alias).
    #[serde(alias = "public_key")]
    pub public_key_b64: String,
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
// Items — upload
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UploadRequest {
    pub item_id: String,
    pub ciphertext_b64: String,
    pub nonce_b64: String,
    pub sender_device_id: String,
    pub lamport_ts: u64,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub fanned_out_to: usize,
}

// ---------------------------------------------------------------------------
// Items — poll
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayItemResponse {
    pub item_id: String,
    pub ciphertext_b64: String,
    pub nonce_b64: String,
    pub sender_device_id: String,
    pub lamport_ts: u64,
    pub content_type: String,
}

#[derive(Debug, Serialize)]
pub struct PollResponse {
    pub items: Vec<RelayItemResponse>,
}

#[derive(Debug, Deserialize)]
pub struct PollParams {
    #[serde(default)]
    pub since_lamport: u64,
}

// ---------------------------------------------------------------------------
// Items — push/pull (wall-clock sync protocol)
// ---------------------------------------------------------------------------

/// Request body for POST /devices/:id/items (push protocol)
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PushRequest {
    pub content_type: String,
    pub content_b64: String,
    pub wall_time: u64,
}

/// Response body for POST /devices/:id/items
#[derive(Debug, Serialize)]
#[allow(dead_code)]
pub struct PushResponse {
    pub id: i64,
}

/// Single item returned by GET /devices/:id/items (pull protocol)
#[derive(Debug, Clone, Serialize)]
#[allow(dead_code)]
pub struct PullItem {
    pub id: i64,
    pub content_type: String,
    pub content_b64: String,
    pub wall_time: u64,
}

/// Query params for GET /devices/:id/items?since=<wall_time>
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct PullParams {
    #[serde(default)]
    pub since: u64,
}

// ---------------------------------------------------------------------------
// Health
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub devices: usize,
    pub total_items: usize,
}
