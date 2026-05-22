use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use sha2::{Digest, Sha256};

use crate::config::RelayConfig;
use crate::error::RelayError;
use crate::models::RelayItemResponse;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

pub struct DeviceRecord {
    pub device_id: String,
    pub public_key_b64: String,
    /// Bearer token: first 32 hex characters of SHA-256(decoded_public_key_bytes).
    pub bearer_token: String,
    pub registered_at: Instant,
}

pub struct RelayItem {
    pub item_id: String,
    pub ciphertext_b64: String,
    pub nonce_b64: String,
    pub sender_device_id: String,
    pub lamport_ts: u64,
    pub content_type: String,
    pub uploaded_at: Instant,
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

pub struct RelayStore {
    pub devices: HashMap<String, DeviceRecord>,
    /// Per-device inbox: items uploaded by OTHER devices for this device to poll.
    pub items: HashMap<String, Vec<RelayItem>>,
    pub sync_ttl_secs: u64,
}

impl RelayStore {
    pub fn new(sync_ttl_secs: u64) -> Self {
        Self {
            devices: HashMap::new(),
            items: HashMap::new(),
            sync_ttl_secs,
        }
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Register a new device. Returns the bearer token on success.
    /// Returns `RelayError::DeviceConflict` if the device_id is already registered.
    pub fn register_device(
        &mut self,
        device_id: String,
        public_key_b64: String,
    ) -> Result<String, RelayError> {
        if self.devices.contains_key(&device_id) {
            return Err(RelayError::DeviceConflict);
        }

        // Derive bearer token from the decoded public key bytes.
        let key_bytes = B64
            .decode(&public_key_b64)
            .map_err(|_| RelayError::BadRequest("invalid base64 for public_key".into()))?;

        let hash = Sha256::digest(&key_bytes);
        let hex = hex_encode(&hash);
        // First 32 hex characters = 16 bytes of entropy — sufficient for Phase 2b.
        let bearer_token = hex[..32].to_string();

        self.devices.insert(
            device_id.clone(),
            DeviceRecord {
                device_id: device_id.clone(),
                public_key_b64,
                bearer_token: bearer_token.clone(),
                registered_at: Instant::now(),
            },
        );
        // Pre-create an empty inbox so poll can work without a separate device-check.
        self.items.entry(device_id).or_default();

        Ok(bearer_token)
    }

    // -----------------------------------------------------------------------
    // Auth
    // -----------------------------------------------------------------------

    /// Verify that `token` matches the bearer token for `device_id`.
    pub fn verify_token(&self, device_id: &str, token: &str) -> Result<(), RelayError> {
        let record = self.devices.get(device_id).ok_or(RelayError::DeviceNotFound)?;
        if record.bearer_token == token {
            Ok(())
        } else {
            Err(RelayError::Unauthorized)
        }
    }

    // -----------------------------------------------------------------------
    // Upload (fan-out)
    // -----------------------------------------------------------------------

    /// Fan out `item` into the inbox of every device EXCEPT the sender.
    /// Returns the number of inboxes the item was delivered to.
    pub fn upload_item(&mut self, item: RelayItem, _config: &RelayConfig) -> usize {
        // Collect all device IDs that are not the sender.
        let targets: Vec<String> = self
            .devices
            .keys()
            .filter(|id| *id != &item.sender_device_id)
            .cloned()
            .collect();

        let count = targets.len();
        for target_id in targets {
            let inbox = self.items.entry(target_id).or_default();
            // Build a copy of the item for each target (clone fields individually).
            inbox.push(RelayItem {
                item_id: item.item_id.clone(),
                ciphertext_b64: item.ciphertext_b64.clone(),
                nonce_b64: item.nonce_b64.clone(),
                sender_device_id: item.sender_device_id.clone(),
                lamport_ts: item.lamport_ts,
                content_type: item.content_type.clone(),
                uploaded_at: item.uploaded_at,
            });
        }
        count
    }

    // -----------------------------------------------------------------------
    // Poll
    // -----------------------------------------------------------------------

    /// Return items in `device_id`'s inbox with `lamport_ts > since_lamport`,
    /// sorted ascending. Prunes TTL-expired items first (lazy expiry).
    pub fn poll_items(
        &mut self,
        device_id: &str,
        since_lamport: u64,
    ) -> Vec<RelayItemResponse> {
        let ttl = self.sync_ttl_secs;
        let inbox = self.items.entry(device_id.to_string()).or_default();

        // Lazy TTL prune.
        inbox.retain(|item| item.uploaded_at.elapsed().as_secs() < ttl);

        let mut result: Vec<RelayItemResponse> = inbox
            .iter()
            .filter(|item| item.lamport_ts > since_lamport)
            .map(|item| RelayItemResponse {
                item_id: item.item_id.clone(),
                ciphertext_b64: item.ciphertext_b64.clone(),
                nonce_b64: item.nonce_b64.clone(),
                sender_device_id: item.sender_device_id.clone(),
                lamport_ts: item.lamport_ts,
                content_type: item.content_type.clone(),
            })
            .collect();

        result.sort_by_key(|r| r.lamport_ts);
        result
    }

    // -----------------------------------------------------------------------
    // Delete
    // -----------------------------------------------------------------------

    /// Remove item `item_id` from `device_id`'s inbox.
    pub fn delete_item(&mut self, device_id: &str, item_id: &str) -> Result<(), RelayError> {
        let inbox = self
            .items
            .get_mut(device_id)
            .ok_or(RelayError::DeviceNotFound)?;

        let before = inbox.len();
        inbox.retain(|item| item.item_id != item_id);
        if inbox.len() == before {
            return Err(RelayError::ItemNotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Stats
    // -----------------------------------------------------------------------

    /// Returns `(device_count, total_item_count)`.
    pub fn stats(&self) -> (usize, usize) {
        let total = self.items.values().map(|v| v.len()).sum();
        (self.devices.len(), total)
    }
}

// ---------------------------------------------------------------------------
// Shared state type alias
// ---------------------------------------------------------------------------

/// The shared application state passed to all axum handlers.
pub type AppState = Arc<Mutex<RelayStore>>;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RelayConfig;

    fn make_store() -> RelayStore {
        RelayStore::new(3600)
    }

    /// Valid 32-byte X25519 key encoded as standard base64.
    fn valid_key_b64() -> String {
        B64.encode([0u8; 32])
    }

    fn device_a_id() -> String {
        "11111111-1111-1111-1111-111111111111".to_string()
    }

    fn device_b_id() -> String {
        "22222222-2222-2222-2222-222222222222".to_string()
    }

    #[test]
    fn register_returns_bearer_token() {
        let mut store = make_store();
        let token = store
            .register_device(device_a_id(), valid_key_b64())
            .unwrap();
        assert_eq!(token.len(), 32, "token must be 32 hex chars");
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn register_duplicate_is_conflict() {
        let mut store = make_store();
        store.register_device(device_a_id(), valid_key_b64()).unwrap();
        let err = store
            .register_device(device_a_id(), valid_key_b64())
            .unwrap_err();
        assert!(matches!(err, RelayError::DeviceConflict));
    }

    #[test]
    fn verify_token_ok() {
        let mut store = make_store();
        let token = store
            .register_device(device_a_id(), valid_key_b64())
            .unwrap();
        assert!(store.verify_token(&device_a_id(), &token).is_ok());
    }

    #[test]
    fn verify_token_wrong_token_is_unauthorized() {
        let mut store = make_store();
        store.register_device(device_a_id(), valid_key_b64()).unwrap();
        let err = store
            .verify_token(&device_a_id(), "badtoken00000000000000000000000")
            .unwrap_err();
        assert!(matches!(err, RelayError::Unauthorized));
    }

    #[test]
    fn upload_fans_out_to_other_devices() {
        let mut store = make_store();
        store.register_device(device_a_id(), valid_key_b64()).unwrap();
        store
            .register_device(device_b_id(), B64.encode([1u8; 32]))
            .unwrap();

        let config = RelayConfig::default();
        let item = RelayItem {
            item_id: "item-1".to_string(),
            ciphertext_b64: B64.encode(b"ciphertext"),
            nonce_b64: B64.encode([0u8; 24]),
            sender_device_id: device_a_id(),
            lamport_ts: 1,
            content_type: "text".to_string(),
            uploaded_at: Instant::now(),
        };

        let count = store.upload_item(item, &config);
        assert_eq!(count, 1, "only device B should receive it");
    }

    #[test]
    fn upload_does_not_fan_out_to_sender() {
        let mut store = make_store();
        store.register_device(device_a_id(), valid_key_b64()).unwrap();

        let config = RelayConfig::default();
        let item = RelayItem {
            item_id: "item-2".to_string(),
            ciphertext_b64: B64.encode(b"data"),
            nonce_b64: B64.encode([0u8; 24]),
            sender_device_id: device_a_id(),
            lamport_ts: 1,
            content_type: "text".to_string(),
            uploaded_at: Instant::now(),
        };
        store.upload_item(item, &config);

        // Sender's own inbox should be empty.
        let items = store.poll_items(&device_a_id(), 0);
        assert!(items.is_empty());
    }

    #[test]
    fn poll_since_lamport_filters_correctly() {
        let mut store = make_store();
        store.register_device(device_a_id(), valid_key_b64()).unwrap();
        store
            .register_device(device_b_id(), B64.encode([1u8; 32]))
            .unwrap();

        let config = RelayConfig::default();
        for ts in [1u64, 2, 3] {
            let item = RelayItem {
                item_id: format!("item-{ts}"),
                ciphertext_b64: B64.encode(b"x"),
                nonce_b64: B64.encode([0u8; 24]),
                sender_device_id: device_a_id(),
                lamport_ts: ts,
                content_type: "text".to_string(),
                uploaded_at: Instant::now(),
            };
            store.upload_item(item, &config);
        }

        let results = store.poll_items(&device_b_id(), 1);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].lamport_ts, 2);
        assert_eq!(results[1].lamport_ts, 3);
    }

    #[test]
    fn delete_item_removes_from_inbox() {
        let mut store = make_store();
        store.register_device(device_a_id(), valid_key_b64()).unwrap();
        store
            .register_device(device_b_id(), B64.encode([1u8; 32]))
            .unwrap();

        let config = RelayConfig::default();
        let item = RelayItem {
            item_id: "del-item".to_string(),
            ciphertext_b64: B64.encode(b"data"),
            nonce_b64: B64.encode([0u8; 24]),
            sender_device_id: device_a_id(),
            lamport_ts: 1,
            content_type: "text".to_string(),
            uploaded_at: Instant::now(),
        };
        store.upload_item(item, &config);
        store.delete_item(&device_b_id(), "del-item").unwrap();

        let items = store.poll_items(&device_b_id(), 0);
        assert!(items.is_empty());
    }

    #[test]
    fn stats_counts_correctly() {
        let mut store = make_store();
        store.register_device(device_a_id(), valid_key_b64()).unwrap();
        store
            .register_device(device_b_id(), B64.encode([1u8; 32]))
            .unwrap();

        let (devices, items) = store.stats();
        assert_eq!(devices, 2);
        assert_eq!(items, 0);
    }
}
