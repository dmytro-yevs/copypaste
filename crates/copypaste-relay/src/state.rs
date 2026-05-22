use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use crate::error::RelayError;
use crate::models::PullItem;
use crate::quota::{self, QuotaViolation, Tier};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of push-sync items per device inbox.
/// When exceeded, the oldest items (lowest wall_time) are pruned on insert.
const MAX_PUSH_ITEMS_PER_DEVICE: usize = 500;

/// Maximum number of devices a single logical "account" can register (free tier).
#[allow(dead_code)]
pub const MAX_FREE_DEVICES: usize = 5;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DeviceRecord {
    pub device_id: String,
    pub device_name: String,
    #[allow(dead_code)]
    pub public_key_b64: String,
    /// Bearer token: first 32 hex characters of SHA-256(decoded_public_key_bytes).
    pub bearer_token: String,
    pub registered_at: Instant,
    /// Unix timestamp (seconds since epoch) when the token expires (1 year).
    pub expires_at_unix: i64,
    /// Subscription tier — determines device count and history quotas.
    #[allow(dead_code)]
    pub tier: Tier,
}

/// A single encrypted item in the wall-clock push/pull sync protocol.
pub struct SyncItem {
    /// Auto-incremented integer ID (unique per device inbox, ascending).
    pub id: i64,
    pub content_type: String,
    pub content_b64: String,
    /// Sender wall-clock time (Unix epoch milliseconds).
    pub wall_time: u64,
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

pub struct RelayStore {
    pub devices: HashMap<String, DeviceRecord>,
    /// Per-device inbox for the wall-clock push/pull sync protocol.
    pub sync_items: HashMap<String, Vec<SyncItem>>,
    /// Monotonically increasing counter used to assign IDs to sync items.
    next_sync_id: i64,
}

impl RelayStore {
    pub fn new(_sync_ttl_secs: u64) -> Self {
        Self {
            devices: HashMap::new(),
            sync_items: HashMap::new(),
            next_sync_id: 1,
        }
    }

    // -----------------------------------------------------------------------
    // Registration
    // -----------------------------------------------------------------------

    /// Register a new device with an explicit tier.
    ///
    /// Returns `(bearer_token, expires_at_unix)` on success.
    /// Returns `RelayError::DeviceConflict` if the device_id is already registered.
    /// Returns `RelayError::DeviceQuotaExceeded` if the device count limit for
    /// `tier` has been reached.
    pub fn register_device_with_tier(
        &mut self,
        device_id: String,
        device_name: String,
        public_key_b64: String,
        tier: Tier,
    ) -> Result<(String, i64), RelayError> {
        if self.devices.contains_key(&device_id) {
            return Err(RelayError::DeviceConflict);
        }

        // Enforce device-count quota before inserting.
        quota::check_device_quota(tier, self.devices.len()).map_err(|v| match v {
            QuotaViolation::MaxDevicesExceeded { limit } => {
                RelayError::DeviceQuotaExceeded { limit }
            }
            _ => unreachable!(),
        })?;

        // Derive bearer token from the decoded public key bytes.
        let key_bytes = B64
            .decode(&public_key_b64)
            .map_err(|_| RelayError::BadRequest("invalid base64 for public_key_b64".into()))?;

        let hash = Sha256::digest(&key_bytes);
        let hex = hex_encode(&hash);
        // First 32 hex characters = 16 bytes of entropy.
        let bearer_token = hex[..32].to_string();

        // Expiry: 1 year from now expressed as Unix seconds.
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let expires_at_unix = now_unix + 365 * 24 * 3600;

        self.devices.insert(
            device_id.clone(),
            DeviceRecord {
                device_id: device_id.clone(),
                device_name,
                public_key_b64,
                bearer_token: bearer_token.clone(),
                registered_at: Instant::now(),
                expires_at_unix,
                tier,
            },
        );
        // Pre-create an empty inbox so pull can work without a separate device-check.
        self.sync_items.entry(device_id).or_default();

        Ok((bearer_token, expires_at_unix))
    }

    /// Register a new device using the default tier (`Tier::Free`).
    ///
    /// Returns `(bearer_token, expires_at_unix)` on success.
    /// Convenience wrapper over [`register_device_with_tier`].
    pub fn register_device(
        &mut self,
        device_id: String,
        device_name: String,
        public_key_b64: String,
    ) -> Result<(String, i64), RelayError> {
        self.register_device_with_tier(device_id, device_name, public_key_b64, Tier::Free)
    }

    /// Return public info about a registered device. Bearer tokens are never included.
    pub fn get_device(&self, device_id: &str) -> Result<&DeviceRecord, RelayError> {
        self.devices.get(device_id).ok_or(RelayError::DeviceNotFound)
    }

    // -----------------------------------------------------------------------
    // Auth
    // -----------------------------------------------------------------------

    /// Verify that `token` matches the bearer token for `device_id`.
    /// Uses constant-time comparison to prevent timing-based token oracle attacks.
    pub fn verify_token(&self, device_id: &str, token: &str) -> Result<(), RelayError> {
        let record = self.devices.get(device_id).ok_or(RelayError::DeviceNotFound)?;
        if record.bearer_token.as_bytes().ct_eq(token.as_bytes()).into() {
            Ok(())
        } else {
            Err(RelayError::Unauthorized)
        }
    }

    // -----------------------------------------------------------------------
    // Push / Pull (wall-clock sync protocol)
    // -----------------------------------------------------------------------

    /// Store an encrypted item in `device_id`'s sync inbox.
    ///
    /// Validates that the decoded `content_b64` does not exceed `max_item_bytes`.
    /// Prunes the oldest item when the inbox exceeds `MAX_PUSH_ITEMS_PER_DEVICE`.
    /// Returns the auto-assigned integer ID.
    pub fn push_item(
        &mut self,
        device_id: &str,
        content_type: String,
        content_b64: String,
        wall_time: u64,
        max_item_bytes: usize,
    ) -> Result<i64, RelayError> {
        if !self.devices.contains_key(device_id) {
            return Err(RelayError::DeviceNotFound);
        }

        if !matches!(content_type.as_str(), "text" | "image" | "file") {
            return Err(RelayError::BadRequest(
                "content_type must be 'text', 'image', or 'file'".to_string(),
            ));
        }

        let decoded = B64
            .decode(&content_b64)
            .map_err(|_| RelayError::BadRequest("content_b64 must be valid base64".to_string()))?;
        if decoded.len() > max_item_bytes {
            return Err(RelayError::PayloadTooLarge);
        }

        let id = self.next_sync_id;
        self.next_sync_id += 1;

        let inbox = self.sync_items.entry(device_id.to_string()).or_default();
        inbox.push(SyncItem { id, content_type, content_b64, wall_time });

        if inbox.len() > MAX_PUSH_ITEMS_PER_DEVICE {
            let overflow = inbox.len() - MAX_PUSH_ITEMS_PER_DEVICE;
            inbox.drain(..overflow);
        }

        Ok(id)
    }

    /// Return items in `device_id`'s sync inbox with `wall_time > since`, sorted ascending.
    pub fn pull_items(&self, device_id: &str, since: u64) -> Result<Vec<PullItem>, RelayError> {
        let inbox = self
            .sync_items
            .get(device_id)
            .ok_or(RelayError::DeviceNotFound)?;

        let mut result: Vec<PullItem> = inbox
            .iter()
            .filter(|item| item.wall_time > since)
            .map(|item| PullItem {
                id: item.id,
                content_type: item.content_type.clone(),
                content_b64: item.content_b64.clone(),
                wall_time: item.wall_time,
            })
            .collect();

        result.sort_by_key(|r| r.wall_time);
        Ok(result)
    }

    // -----------------------------------------------------------------------
    // Delete
    // -----------------------------------------------------------------------

    /// Remove item `item_id` from `device_id`'s inbox (matched by id as string).
    pub fn delete_item(&mut self, device_id: &str, item_id: &str) -> Result<(), RelayError> {
        let parsed_id: i64 = item_id
            .parse()
            .map_err(|_| RelayError::BadRequest("item_id must be an integer".to_string()))?;

        let inbox = self
            .sync_items
            .get_mut(device_id)
            .ok_or(RelayError::DeviceNotFound)?;

        let before = inbox.len();
        inbox.retain(|item| item.id != parsed_id);
        if inbox.len() == before {
            return Err(RelayError::ItemNotFound);
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Cleanup
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub fn cleanup_inactive_devices(&mut self, inactive_threshold_secs: u64) -> usize {
        let inactive_ids: Vec<String> = self
            .devices
            .iter()
            .filter(|(id, record)| {
                let old_enough =
                    record.registered_at.elapsed().as_secs() >= inactive_threshold_secs;
                if !old_enough {
                    return false;
                }
                let inbox = self.sync_items.get(*id);
                let has_items = inbox.map_or(false, |items| !items.is_empty());
                !has_items
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = inactive_ids.len();
        for id in &inactive_ids {
            self.devices.remove(id);
            self.sync_items.remove(id);
        }
        count
    }

    // -----------------------------------------------------------------------
    // Devices listing
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub fn list_devices(&self) -> Vec<String> {
        let mut records: Vec<&DeviceRecord> = self.devices.values().collect();
        records.sort_by(|a, b| b.registered_at.cmp(&a.registered_at));
        records.into_iter().take(100).map(|r| r.device_id.clone()).collect()
    }

    // -----------------------------------------------------------------------
    // Stats
    // -----------------------------------------------------------------------

    pub fn stats(&self) -> (usize, usize) {
        let total = self.sync_items.values().map(|v| v.len()).sum();
        (self.devices.len(), total)
    }
}

// ---------------------------------------------------------------------------
// Shared state type alias
// ---------------------------------------------------------------------------

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

    fn make_store() -> RelayStore { RelayStore::new(3600) }
    fn valid_key_b64() -> String { B64.encode([0u8; 32]) }
    fn device_a_id() -> String { "11111111-1111-1111-1111-111111111111".to_string() }
    fn device_b_id() -> String { "22222222-2222-2222-2222-222222222222".to_string() }

    fn unique_device_id(n: u8) -> String {
        format!("{n:02x}{n:02x}{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}")
    }
    fn unique_key(seed: u8) -> String { B64.encode([seed; 32]) }

    fn push_text(store: &mut RelayStore, device_id: &str, wall_time: u64) -> i64 {
        store.push_item(device_id, "text".to_string(), B64.encode(b"hello"), wall_time, 10 * 1024 * 1024).unwrap()
    }

    #[test]
    fn register_returns_bearer_token() {
        let mut store = make_store();
        let (token, expires_at) = store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(expires_at > 0);
    }

    #[test]
    fn register_duplicate_is_conflict() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        let err = store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap_err();
        assert!(matches!(err, RelayError::DeviceConflict));
    }

    #[test]
    fn verify_token_ok() {
        let mut store = make_store();
        let (token, _) = store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        assert!(store.verify_token(&device_a_id(), &token).is_ok());
    }

    #[test]
    fn verify_token_wrong_token_is_unauthorized() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        let err = store.verify_token(&device_a_id(), "badtoken00000000000000000000000").unwrap_err();
        assert!(matches!(err, RelayError::Unauthorized));
    }

    #[test]
    fn get_device_returns_correct_info() {
        let mut store = make_store();
        store.register_device(device_a_id(), "My Mac".into(), valid_key_b64()).unwrap();
        let record = store.get_device(&device_a_id()).unwrap();
        assert_eq!(record.device_id, device_a_id());
        assert_eq!(record.device_name, "My Mac");
        assert_eq!(record.public_key_b64, valid_key_b64());
        assert!(record.expires_at_unix > 0);
    }

    #[test]
    fn get_device_missing_returns_not_found() {
        let store = make_store();
        let err = store.get_device("nonexistent-id").unwrap_err();
        assert!(matches!(err, RelayError::DeviceNotFound));
    }

    #[test]
    fn push_returns_ascending_ids() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        let id1 = push_text(&mut store, &device_a_id(), 1000);
        let id2 = push_text(&mut store, &device_a_id(), 2000);
        assert!(id2 > id1);
    }

    #[test]
    fn pull_returns_items_since_wall_time() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        push_text(&mut store, &device_a_id(), 1000);
        push_text(&mut store, &device_a_id(), 2000);
        push_text(&mut store, &device_a_id(), 3000);
        let items = store.pull_items(&device_a_id(), 1000).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].wall_time, 2000);
        assert_eq!(items[1].wall_time, 3000);
    }

    #[test]
    fn pull_since_zero_returns_all() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        push_text(&mut store, &device_a_id(), 100);
        push_text(&mut store, &device_a_id(), 200);
        let items = store.pull_items(&device_a_id(), 0).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn pull_sorted_ascending_by_wall_time() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        push_text(&mut store, &device_a_id(), 3000);
        push_text(&mut store, &device_a_id(), 1000);
        push_text(&mut store, &device_a_id(), 2000);
        let items = store.pull_items(&device_a_id(), 0).unwrap();
        let times: Vec<u64> = items.iter().map(|i| i.wall_time).collect();
        assert_eq!(times, vec![1000, 2000, 3000]);
    }

    #[test]
    fn push_rejects_unknown_content_type() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        let err = store.push_item(&device_a_id(), "video".to_string(), B64.encode(b"x"), 1000, 10 * 1024 * 1024).unwrap_err();
        assert!(matches!(err, RelayError::BadRequest(_)));
    }

    #[test]
    fn push_rejects_invalid_base64() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        let err = store.push_item(&device_a_id(), "text".to_string(), "!!!not-base64!!!".to_string(), 1000, 10 * 1024 * 1024).unwrap_err();
        assert!(matches!(err, RelayError::BadRequest(_)));
    }

    #[test]
    fn push_rejects_oversized_payload() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        let big = B64.encode(b"hello world");
        let err = store.push_item(&device_a_id(), "text".to_string(), big, 1000, 10).unwrap_err();
        assert!(matches!(err, RelayError::PayloadTooLarge));
    }

    #[test]
    fn push_quota_prunes_oldest_item() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        for t in 1u64..=(MAX_PUSH_ITEMS_PER_DEVICE as u64 + 1) {
            push_text(&mut store, &device_a_id(), t);
        }
        let items = store.pull_items(&device_a_id(), 0).unwrap();
        assert_eq!(items.len(), MAX_PUSH_ITEMS_PER_DEVICE);
        let min_wt = items.iter().map(|i| i.wall_time).min().unwrap();
        assert_eq!(min_wt, 2, "oldest item must be evicted");
    }

    #[test]
    fn pull_returns_device_not_found_for_unknown_device() {
        let store = make_store();
        let err = store.pull_items("unknown-device", 0).unwrap_err();
        assert!(matches!(err, RelayError::DeviceNotFound));
    }

    #[test]
    fn stats_counts_correctly() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        store.register_device(device_b_id(), "Device B".into(), B64.encode([1u8; 32])).unwrap();
        let (devices, items) = store.stats();
        assert_eq!(devices, 2);
        assert_eq!(items, 0);
        push_text(&mut store, &device_a_id(), 1000);
        let (_, items) = store.stats();
        assert_eq!(items, 1);
    }

    #[test]
    fn cleanup_removes_old_inactive_devices() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        store.register_device(device_b_id(), "Device B".into(), B64.encode([1u8; 32])).unwrap();
        let removed = store.cleanup_inactive_devices(0);
        assert_eq!(removed, 2);
        assert!(store.devices.is_empty());
        assert!(store.sync_items.is_empty());
    }

    #[test]
    fn cleanup_keeps_recently_registered_devices() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        store.register_device(device_b_id(), "Device B".into(), B64.encode([1u8; 32])).unwrap();
        let removed = store.cleanup_inactive_devices(u64::MAX);
        assert_eq!(removed, 0);
        assert!(store.devices.contains_key(&device_a_id()));
        assert!(store.devices.contains_key(&device_b_id()));
    }

    #[test]
    fn cleanup_keeps_devices_with_items() {
        let mut store = make_store();
        store.register_device(device_a_id(), "Device A".into(), valid_key_b64()).unwrap();
        push_text(&mut store, &device_a_id(), 1000);
        let removed = store.cleanup_inactive_devices(0);
        assert_eq!(removed, 0, "device with items must not be removed");
    }

    #[test]
    fn sixth_free_device_registration_fails_with_403() {
        let mut store = make_store();
        for i in 0u8..5 {
            store.register_device_with_tier(unique_device_id(i), format!("Dev {i}"), unique_key(i), Tier::Free).unwrap();
        }
        let err = store.register_device_with_tier(unique_device_id(5), "Dev 5".into(), unique_key(5), Tier::Free).unwrap_err();
        assert!(matches!(err, RelayError::DeviceQuotaExceeded { limit: 5 }), "got {err:?}");
    }

    #[test]
    fn fifth_free_device_registration_succeeds() {
        let mut store = make_store();
        for i in 0u8..4 {
            store.register_device_with_tier(unique_device_id(i), format!("Dev {i}"), unique_key(i), Tier::Free).unwrap();
        }
        store.register_device_with_tier(unique_device_id(4), "Dev 4".into(), unique_key(4), Tier::Free).expect("5th free device must be accepted");
    }

    #[test]
    fn eleventh_pro_device_registration_fails() {
        let mut store = make_store();
        for i in 0u8..10 {
            store.register_device_with_tier(unique_device_id(i), format!("Dev {i}"), unique_key(i), Tier::Pro).unwrap();
        }
        let err = store.register_device_with_tier(unique_device_id(10), "Dev 10".into(), unique_key(10), Tier::Pro).unwrap_err();
        assert!(matches!(err, RelayError::DeviceQuotaExceeded { limit: 10 }));
    }

    #[test]
    fn default_register_device_uses_free_tier() {
        let mut store = make_store();
        for i in 0u8..5 {
            store.register_device_with_tier(unique_device_id(i), format!("Dev {i}"), unique_key(i), Tier::Free).unwrap();
        }
        let err = store.register_device(unique_device_id(5), "Dev 5".into(), unique_key(5)).unwrap_err();
        assert!(matches!(err, RelayError::DeviceQuotaExceeded { limit: 5 }));
    }
}
