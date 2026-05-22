use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Registration
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct RegisterRequest {
    pub device_id: String,
    /// Base64-standard-encoded X25519 public key (must decode to exactly 32 bytes).
    pub public_key: String,
}

#[derive(Debug, Serialize)]
pub struct RegisterResponse {
    pub device_id: String,
    pub bearer_token: String,
}

// ---------------------------------------------------------------------------
// Items — push (simple wall-clock sync protocol)
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
// Items — pull (simple wall-clock sync protocol)
// ---------------------------------------------------------------------------

/// Single item returned by GET /devices/:id/items
#[derive(Debug, Clone, Serialize)]
pub struct PullItem {
    pub id: i64,
    pub content_type: String,
    pub content_b64: String,
    pub wall_time: u64,
}

/// Query params for GET /devices/:id/items?since=<wall_time>
#[derive(Debug, Deserialize)]
pub struct PullParams {
    /// Return only items with wall_time > since (defaults to 0).
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
