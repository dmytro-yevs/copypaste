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
// Health
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub devices: usize,
    pub total_items: usize,
}
