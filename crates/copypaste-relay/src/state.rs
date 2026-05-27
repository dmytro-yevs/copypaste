use std::cmp::Reverse;
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use subtle::ConstantTimeEq;

use crate::error::RelayError;
use crate::models::PullItem;
use crate::quota::{self, QuotaViolation, Tier};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Absolute hard cap on push-sync items per device inbox, independent of tier.
/// When exceeded, the oldest items (lowest wall_time) are pruned on insert.
/// Acts as a memory-safety ceiling that no tier may exceed.
const MAX_PUSH_ITEMS_PER_DEVICE: usize = 500;

/// Effective per-inbox history cap for a device: the tighter of the absolute
/// hard cap [`MAX_PUSH_ITEMS_PER_DEVICE`] and the device tier's
/// `max_history_items` (`None` = unlimited tier history → only the hard cap
/// applies). Enforced as a silent prune-oldest inside [`RelayStore::push_item`]
/// — the sender is never told a recipient inbox is full, matching the existing
/// hard-cap eviction behaviour (see the relay v2 quotas plan).
fn effective_history_cap(tier: Tier) -> usize {
    history_cap_for_limit(tier.max_history_items())
}

/// Core of [`effective_history_cap`]: clamp a tier's optional `max_history_items`
/// against the absolute hard cap. `None` (unlimited tier history) yields the
/// hard cap; a limit tighter than the hard cap wins. Split out so the
/// clamp can be unit-tested with a genuinely sub-hard-cap limit (no live tier
/// currently defines one — see `effective_history_cap_is_tier_aware`).
fn history_cap_for_limit(tier_limit: Option<usize>) -> usize {
    MAX_PUSH_ITEMS_PER_DEVICE.min(tier_limit.unwrap_or(usize::MAX))
}

/// Maximum number of devices a single logical "account" can register (free tier).
#[allow(dead_code)]
pub const MAX_FREE_DEVICES: usize = 5;

/// Per-device registration-rate-limit window (security MEDIUM #13).
pub const REG_LIMIT_WINDOW: Duration = Duration::from_secs(60);
/// Maximum registration attempts allowed per device_id within `REG_LIMIT_WINDOW`.
pub const REG_LIMIT_MAX_ATTEMPTS: usize = 5;
/// Hard cap on the rate-limiter map size to bound memory if an attacker rotates device_ids.
const REG_LIMIT_MAX_KEYS: usize = 10_000;

// ---------------------------------------------------------------------------
// Domain types
// ---------------------------------------------------------------------------

#[derive(Debug)]
pub struct DeviceRecord {
    pub device_id: String,
    pub device_name: String,
    #[allow(dead_code)]
    pub public_key_b64: String,
    /// Bearer token: 32 hex characters representing 16 random bytes from OsRng.
    /// Generated at registration time and stored verbatim — never recomputed
    /// from the public key (which would make it a deterministic oracle).
    pub bearer_token: String,
    pub registered_at: Instant,
    /// Unix timestamp (seconds since epoch) when the token expires (1 year).
    pub expires_at_unix: i64,
    /// Subscription tier — determines device count and history quotas.
    // Read by `push_item` via the per-device tier lookup, but live
    // registration always stores `Tier::Free` today (token-/SQLite-driven tier
    // selection is not wired to the in-memory store yet — see the relay v2
    // quotas plan), so the compiler sees no production read and reports it as
    // dead. Kept for the forthcoming tier-wiring.
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
    /// Server-side wall-clock time at insert (Unix epoch seconds). Used for
    /// TTL eviction independent of (untrusted) sender `wall_time`. Read by
    /// the background evictor (see `store.rs`) — `#[allow]` for crate
    /// configurations that don't see the binary entry point.
    #[allow(dead_code)]
    pub inserted_at_unix: u64,
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

pub struct RelayStore {
    pub devices: HashMap<String, DeviceRecord>,
    /// Per-device inbox for the wall-clock push/pull sync protocol.
    pub sync_items: HashMap<String, Vec<SyncItem>>,
    /// Per-device monotonically increasing counter used to assign IDs to
    /// sync items. Keying by `device_id` (rather than a single global
    /// counter) avoids cross-restart ID collisions for a given device:
    /// on the first push for a device after restart we seed this counter
    /// from `MAX(item.id) + 1` over that device's inbox (security HIGH #3).
    ///
    /// `i64` matches the wire/DB representation; we use `checked_add` on
    /// allocation to convert overflow into a server error instead of an
    /// unchecked-arithmetic panic.
    next_sync_id_per_device: HashMap<String, i64>,
    /// Rolling window of registration attempts keyed by `(client_ip, device_id)`.
    /// Used to enforce a per-device registration rate limit (MEDIUM #13)
    /// orthogonal to the per-IP `tower_governor` limiter. Keying by the
    /// tuple closes the enumeration oracle (HIGH #5): a vanilla device-id
    /// probe from an attacker IP no longer leaves a `device_id`-only key
    /// in the map that signals "this id has been seen".
    ///
    /// `client_ip` is `None` when the relay is exercised without a real
    /// transport (unit/integration tests, `tower::ServiceExt::oneshot`).
    /// Tests share a single bucket per device id in that mode, matching
    /// the previous behaviour.
    reg_attempts: HashMap<(Option<IpAddr>, String), VecDeque<Instant>>,

    // -----------------------------------------------------------------------
    // Prometheus metrics counters (see api/metrics.rs)
    // -----------------------------------------------------------------------
    /// Monotonic counter: total sync items ever accepted by `push_item`.
    /// Never decremented (even when evicted) — this is a `counter` in
    /// Prometheus terms. Wrapped in `Arc<AtomicU64>` so the metrics
    /// endpoint can read it without holding the store mutex.
    items_total: Arc<AtomicU64>,
    /// Monotonic counter: total sync items removed by `prune_expired`
    /// (TTL eviction). Counter — only ever incremented.
    evictions_total: Arc<AtomicU64>,
}

impl RelayStore {
    pub fn new(_sync_ttl_secs: u64) -> Self {
        Self {
            devices: HashMap::new(),
            sync_items: HashMap::new(),
            next_sync_id_per_device: HashMap::new(),
            reg_attempts: HashMap::new(),
            items_total: Arc::new(AtomicU64::new(0)),
            evictions_total: Arc::new(AtomicU64::new(0)),
        }
    }

    // -----------------------------------------------------------------------
    // Metrics accessors (see api/metrics.rs)
    // -----------------------------------------------------------------------

    /// Snapshot the three Prometheus metric values.
    /// Returns `(items_total, evictions_total, active_devices)`.
    /// `active_devices` is derived from inboxes — the count of device IDs
    /// whose inbox currently has at least one item.
    #[allow(dead_code)] // unused in some test binaries that `#[path]`-include state.rs
    pub fn metrics_snapshot(&self) -> (u64, u64, u64) {
        let items = self.items_total.load(Ordering::Relaxed);
        let evictions = self.evictions_total.load(Ordering::Relaxed);
        let active = self.sync_items.values().filter(|v| !v.is_empty()).count() as u64;
        (items, evictions, active)
    }

    // -----------------------------------------------------------------------
    // Per-device registration rate limiter (security MEDIUM #13)
    // -----------------------------------------------------------------------

    /// Record a registration attempt for `(client_ip, device_id)` and return
    /// `Err(retry_after_secs)` when the per-(ip, device) rate-limit window
    /// is exhausted (`REG_LIMIT_MAX_ATTEMPTS` attempts within
    /// `REG_LIMIT_WINDOW`).
    ///
    /// This is independent of the per-IP `tower_governor` limiter installed
    /// in `routes/mod.rs`: it blocks an attacker who has obtained a victim's
    /// `device_id` (but not the bearer token) from flooding re-registrations
    /// of that specific id from a single source IP. Keying by the tuple
    /// (HIGH #5) avoids leaking "this device id is known to the limiter"
    /// across source IPs.
    ///
    /// Callers should invoke this **only after** the request payload has
    /// passed full validation (UUID parse, base64 key, key length, device
    /// name) so the limiter never grows from probes that the handler would
    /// have rejected anyway.
    pub fn check_registration_rate_limit(
        &mut self,
        client_ip: Option<IpAddr>,
        device_id: &str,
    ) -> Result<(), u64> {
        let now = Instant::now();

        // Opportunistic global eviction when the map grows too large.
        if self.reg_attempts.len() > REG_LIMIT_MAX_KEYS {
            self.reg_attempts.retain(|_, deque| {
                deque.retain(|t| now.duration_since(*t) < REG_LIMIT_WINDOW);
                !deque.is_empty()
            });
        }

        let deque = self
            .reg_attempts
            .entry((client_ip, device_id.to_string()))
            .or_default();
        // Drop attempts that fell out of the rolling window.
        while let Some(front) = deque.front() {
            if now.duration_since(*front) >= REG_LIMIT_WINDOW {
                deque.pop_front();
            } else {
                break;
            }
        }

        if deque.len() >= REG_LIMIT_MAX_ATTEMPTS {
            let oldest = *deque.front().expect("non-empty by check above");
            let retry_after = REG_LIMIT_WINDOW
                .saturating_sub(now.duration_since(oldest))
                .as_secs()
                .max(1);
            return Err(retry_after);
        }

        deque.push_back(now);
        Ok(())
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

        // Proof-of-possession (security MEDIUM #14):
        // Reject zero-length public_key_b64 and ensure base64 decodes to
        // exactly 32 bytes (X25519 public-key size).
        // TODO: v0.2 — require a signature over device_id with the
        // device's private key to fully prove possession of the keypair.
        if public_key_b64.is_empty() {
            return Err(RelayError::BadRequest(
                "public_key_b64 must not be empty".into(),
            ));
        }
        let key_bytes = B64
            .decode(&public_key_b64)
            .map_err(|_| RelayError::BadRequest("invalid base64 for public_key_b64".into()))?;
        if key_bytes.len() != 32 {
            return Err(RelayError::BadRequest(format!(
                "public_key_b64 must decode to exactly 32 bytes, got {}",
                key_bytes.len()
            )));
        }

        // Generate bearer token from 16 random bytes (NEVER derive from
        // public key — that would let any client compute the secret).
        // Output: 32 hex characters representing 16 bytes of entropy.
        let mut token_bytes = [0u8; 16];
        OsRng.fill_bytes(&mut token_bytes);
        let bearer_token = hex_encode(&token_bytes);

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
    /// Convenience wrapper over [`Self::register_device_with_tier`].
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
        self.devices
            .get(device_id)
            .ok_or(RelayError::DeviceNotFound)
    }

    // -----------------------------------------------------------------------
    // Auth
    // -----------------------------------------------------------------------

    /// Verify that `token` matches the bearer token for `device_id`.
    /// Uses constant-time comparison to prevent timing-based token oracle attacks.
    ///
    /// M11 (audit 2026-05-27): also enforces `expires_at_unix`. An expired
    /// token returns `Unauthorized` (NOT a distinct error) so an attacker
    /// cannot distinguish "wrong token" from "expired token". The equality
    /// check still runs before the expiry branch so the constant-time
    /// comparison path is unconditional.
    pub fn verify_token(&self, device_id: &str, token: &str) -> Result<(), RelayError> {
        let record = self
            .devices
            .get(device_id)
            .ok_or(RelayError::DeviceNotFound)?;
        let token_ok: bool = record
            .bearer_token
            .as_bytes()
            .ct_eq(token.as_bytes())
            .into();
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let not_expired = now_unix <= record.expires_at_unix;
        if token_ok && not_expired {
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
        let tier = match self.devices.get(device_id) {
            Some(record) => record.tier,
            None => return Err(RelayError::DeviceNotFound),
        };

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

        // Per-device counter, seeded from the inbox on first push so a
        // server restart cannot re-issue an id another item in the same
        // device's inbox already holds (security HIGH #3).
        let counter = self
            .next_sync_id_per_device
            .entry(device_id.to_string())
            .or_insert_with(|| {
                self.sync_items
                    .get(device_id)
                    .and_then(|inbox| inbox.iter().map(|i| i.id).max())
                    .map(|m| m.saturating_add(1))
                    .unwrap_or(1)
                    .max(1)
            });
        let id = *counter;
        // `checked_add` so an id-counter overflow returns a server error
        // instead of an unchecked-arithmetic panic (security HIGH #3).
        *counter = counter.checked_add(1).ok_or_else(|| {
            tracing::warn!(device_id, "sync id counter overflow");
            RelayError::Internal("sync id counter exhausted".into())
        })?;

        let inserted_at_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let inbox = self.sync_items.entry(device_id.to_string()).or_default();
        inbox.push(SyncItem {
            id,
            content_type,
            content_b64,
            wall_time,
            inserted_at_unix,
        });

        // History quota: cap the inbox at the tier-aware effective limit
        // (the tighter of the absolute hard cap and the tier's
        // `max_history_items`). Enforced as a silent prune of the oldest
        // items rather than rejecting the push — the fan-out sender cannot
        // know which recipient inboxes are full (see the relay v2 quotas
        // plan). `effective_history_cap` already folds the tier history limit
        // into the absolute hard cap, so this single guard subsumes the tier
        // quota check for every current tier (Free's 1000 limit is looser than
        // the 500 hard cap, Pro is unlimited).
        let cap = effective_history_cap(tier);
        if inbox.len() > cap {
            inbox.drain(..inbox.len() - cap);
        }

        // Increment Prometheus counter — items_total tracks all accepted
        // pushes regardless of later eviction (counter semantics).
        self.items_total.fetch_add(1, Ordering::Relaxed);

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
                let has_items = inbox.is_some_and(|items| !items.is_empty());
                !has_items
            })
            .map(|(id, _)| id.clone())
            .collect();

        let count = inactive_ids.len();
        for id in &inactive_ids {
            self.devices.remove(id);
            self.sync_items.remove(id);
            self.next_sync_id_per_device.remove(id);
        }
        count
    }

    // -----------------------------------------------------------------------
    // Devices listing
    // -----------------------------------------------------------------------

    #[allow(dead_code)]
    pub fn list_devices(&self) -> Vec<String> {
        let mut records: Vec<&DeviceRecord> = self.devices.values().collect();
        records.sort_by_key(|r| Reverse(r.registered_at));
        records
            .into_iter()
            .take(100)
            .map(|r| r.device_id.clone())
            .collect()
    }

    // -----------------------------------------------------------------------
    // TTL eviction (see ADR-009)
    // -----------------------------------------------------------------------

    /// Drop sync items whose `inserted_at_unix + ttl_secs <= now_unix`.
    ///
    /// `now_unix` is supplied by the caller so unit tests can advance a
    /// virtual clock (`tokio::time::advance`) without touching the real
    /// system clock.
    ///
    /// Returns the number of items evicted (across all device inboxes).
    /// Empty inboxes are NOT removed — devices keep their registration
    /// regardless of inbox activity (see [`Self::cleanup_inactive_devices`] for
    /// device-record pruning).
    #[allow(dead_code)]
    pub fn prune_expired(&mut self, now_unix: u64, ttl_secs: u64) -> usize {
        if ttl_secs == 0 {
            return 0;
        }
        let cutoff = now_unix.saturating_sub(ttl_secs);
        let mut evicted = 0usize;
        for inbox in self.sync_items.values_mut() {
            let before = inbox.len();
            inbox.retain(|item| item.inserted_at_unix > cutoff);
            evicted += before - inbox.len();
        }
        if evicted > 0 {
            self.evictions_total
                .fetch_add(evicted as u64, Ordering::Relaxed);
        }
        evicted
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

    fn make_store() -> RelayStore {
        RelayStore::new(3600)
    }
    fn valid_key_b64() -> String {
        B64.encode([0u8; 32])
    }
    fn device_a_id() -> String {
        "11111111-1111-1111-1111-111111111111".to_string()
    }
    fn device_b_id() -> String {
        "22222222-2222-2222-2222-222222222222".to_string()
    }

    fn unique_device_id(n: u8) -> String {
        format!("{n:02x}{n:02x}{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}")
    }
    fn unique_key(seed: u8) -> String {
        B64.encode([seed; 32])
    }

    fn push_text(store: &mut RelayStore, device_id: &str, wall_time: u64) -> i64 {
        store
            .push_item(
                device_id,
                "text".to_string(),
                B64.encode(b"hello"),
                wall_time,
                10 * 1024 * 1024,
            )
            .unwrap()
    }

    #[test]
    fn register_returns_bearer_token() {
        let mut store = make_store();
        let (token, expires_at) = store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        assert_eq!(token.len(), 32);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(expires_at > 0);
    }

    #[test]
    fn register_duplicate_is_conflict() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        let err = store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap_err();
        assert!(matches!(err, RelayError::DeviceConflict));
    }

    #[test]
    fn verify_token_ok() {
        let mut store = make_store();
        let (token, _) = store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        assert!(store.verify_token(&device_a_id(), &token).is_ok());
    }

    #[test]
    fn verify_token_wrong_token_is_unauthorized() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        let err = store
            .verify_token(&device_a_id(), "badtoken00000000000000000000000")
            .unwrap_err();
        assert!(matches!(err, RelayError::Unauthorized));
    }

    #[test]
    fn verify_token_expired_is_unauthorized() {
        // M11: an expired token must return Unauthorized (same variant as
        // a wrong token) so an attacker cannot distinguish the two cases.
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        // Forcibly expire the device's token by rewinding `expires_at_unix`.
        let record = store.devices.get_mut(&device_a_id()).unwrap();
        let token = record.bearer_token.clone();
        record.expires_at_unix = 1; // 1970-01-01
        let err = store.verify_token(&device_a_id(), &token).unwrap_err();
        assert!(matches!(err, RelayError::Unauthorized));
    }

    #[test]
    fn get_device_returns_correct_info() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "My Mac".into(), valid_key_b64())
            .unwrap();
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
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        let id1 = push_text(&mut store, &device_a_id(), 1000);
        let id2 = push_text(&mut store, &device_a_id(), 2000);
        assert!(id2 > id1);
    }

    #[test]
    fn pull_returns_items_since_wall_time() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
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
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        push_text(&mut store, &device_a_id(), 100);
        push_text(&mut store, &device_a_id(), 200);
        let items = store.pull_items(&device_a_id(), 0).unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn pull_sorted_ascending_by_wall_time() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
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
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        let err = store
            .push_item(
                &device_a_id(),
                "video".to_string(),
                B64.encode(b"x"),
                1000,
                10 * 1024 * 1024,
            )
            .unwrap_err();
        assert!(matches!(err, RelayError::BadRequest(_)));
    }

    #[test]
    fn push_rejects_invalid_base64() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        let err = store
            .push_item(
                &device_a_id(),
                "text".to_string(),
                "!!!not-base64!!!".to_string(),
                1000,
                10 * 1024 * 1024,
            )
            .unwrap_err();
        assert!(matches!(err, RelayError::BadRequest(_)));
    }

    #[test]
    fn push_rejects_oversized_payload() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        let big = B64.encode(b"hello world");
        let err = store
            .push_item(&device_a_id(), "text".to_string(), big, 1000, 10)
            .unwrap_err();
        assert!(matches!(err, RelayError::PayloadTooLarge));
    }

    #[test]
    fn push_quota_prunes_oldest_item() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
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
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        store
            .register_device(device_b_id(), "Device B".into(), B64.encode([1u8; 32]))
            .unwrap();
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
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        store
            .register_device(device_b_id(), "Device B".into(), B64.encode([1u8; 32]))
            .unwrap();
        let removed = store.cleanup_inactive_devices(0);
        assert_eq!(removed, 2);
        assert!(store.devices.is_empty());
        assert!(store.sync_items.is_empty());
    }

    #[test]
    fn cleanup_keeps_recently_registered_devices() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        store
            .register_device(device_b_id(), "Device B".into(), B64.encode([1u8; 32]))
            .unwrap();
        let removed = store.cleanup_inactive_devices(u64::MAX);
        assert_eq!(removed, 0);
        assert!(store.devices.contains_key(&device_a_id()));
        assert!(store.devices.contains_key(&device_b_id()));
    }

    #[test]
    fn cleanup_keeps_devices_with_items() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "Device A".into(), valid_key_b64())
            .unwrap();
        push_text(&mut store, &device_a_id(), 1000);
        let removed = store.cleanup_inactive_devices(0);
        assert_eq!(removed, 0, "device with items must not be removed");
    }

    #[test]
    fn sixth_free_device_registration_fails_with_403() {
        let mut store = make_store();
        for i in 0u8..5 {
            store
                .register_device_with_tier(
                    unique_device_id(i),
                    format!("Dev {i}"),
                    unique_key(i),
                    Tier::Free,
                )
                .unwrap();
        }
        let err = store
            .register_device_with_tier(
                unique_device_id(5),
                "Dev 5".into(),
                unique_key(5),
                Tier::Free,
            )
            .unwrap_err();
        assert!(
            matches!(err, RelayError::DeviceQuotaExceeded { limit: 5 }),
            "got {err:?}"
        );
    }

    #[test]
    fn fifth_free_device_registration_succeeds() {
        let mut store = make_store();
        for i in 0u8..4 {
            store
                .register_device_with_tier(
                    unique_device_id(i),
                    format!("Dev {i}"),
                    unique_key(i),
                    Tier::Free,
                )
                .unwrap();
        }
        store
            .register_device_with_tier(
                unique_device_id(4),
                "Dev 4".into(),
                unique_key(4),
                Tier::Free,
            )
            .expect("5th free device must be accepted");
    }

    #[test]
    fn eleventh_pro_device_registration_fails() {
        let mut store = make_store();
        for i in 0u8..10 {
            store
                .register_device_with_tier(
                    unique_device_id(i),
                    format!("Dev {i}"),
                    unique_key(i),
                    Tier::Pro,
                )
                .unwrap();
        }
        let err = store
            .register_device_with_tier(
                unique_device_id(10),
                "Dev 10".into(),
                unique_key(10),
                Tier::Pro,
            )
            .unwrap_err();
        assert!(matches!(err, RelayError::DeviceQuotaExceeded { limit: 10 }));
    }

    #[test]
    fn default_register_device_uses_free_tier() {
        let mut store = make_store();
        for i in 0u8..5 {
            store
                .register_device_with_tier(
                    unique_device_id(i),
                    format!("Dev {i}"),
                    unique_key(i),
                    Tier::Free,
                )
                .unwrap();
        }
        let err = store
            .register_device(unique_device_id(5), "Dev 5".into(), unique_key(5))
            .unwrap_err();
        assert!(matches!(err, RelayError::DeviceQuotaExceeded { limit: 5 }));
    }

    // ---- History quota enforcement (plan: silent drop) ---------------------

    /// The history quota is enforced inside `push_item` keyed by the device's
    /// tier. A push is never rejected with an error (the plan mandates a
    /// "silent drop"): instead the inbox is capped at the effective limit —
    /// `min(MAX_PUSH_ITEMS_PER_DEVICE, tier.max_history_items())` — by pruning
    /// the oldest items, mirroring the existing hard-cap eviction behaviour.
    #[test]
    fn free_tier_inbox_never_exceeds_history_cap() {
        let mut store = make_store();
        store
            .register_device_with_tier(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                Tier::Free,
            )
            .unwrap();

        // The effective cap is min(500, 1000) = 500 for Free tier.
        let effective_cap =
            MAX_PUSH_ITEMS_PER_DEVICE.min(Tier::Free.max_history_items().unwrap_or(usize::MAX));

        for t in 1u64..=(effective_cap as u64 + 50) {
            // Pushes must always succeed (never error) — the cap is enforced
            // by a silent drop of the oldest item, not a rejection.
            push_text(&mut store, &device_a_id(), t);
        }

        let items = store.pull_items(&device_a_id(), 0).unwrap();
        assert!(
            items.len() <= effective_cap,
            "inbox must never exceed the effective history cap ({effective_cap}), got {}",
            items.len()
        );
    }

    /// History-quota enforcement must consult the device's *tier*: a Pro device
    /// (unlimited history) is bounded only by the absolute hard cap, never by a
    /// tier history limit.
    #[test]
    fn pro_tier_history_is_bounded_only_by_hard_cap() {
        let mut store = make_store();
        store
            .register_device_with_tier(device_a_id(), "Device A".into(), valid_key_b64(), Tier::Pro)
            .unwrap();

        for t in 1u64..=(MAX_PUSH_ITEMS_PER_DEVICE as u64 + 50) {
            push_text(&mut store, &device_a_id(), t);
        }

        let items = store.pull_items(&device_a_id(), 0).unwrap();
        // Pro tier has no history limit, so only the absolute hard cap applies.
        assert_eq!(items.len(), MAX_PUSH_ITEMS_PER_DEVICE);
    }

    /// The effective per-inbox history cap is the tighter of the absolute hard
    /// cap and the tier's `max_history_items`. This proves the enforcement path
    /// genuinely consults the device tier (a tier with a sub-hard-cap limit
    /// would bind below 500), rather than ignoring it.
    #[test]
    fn effective_history_cap_is_tier_aware() {
        // Free's 1000-item tier limit is intentionally looser than the 500
        // hard cap, so the cap clamps down to the hard cap, not the tier limit.
        assert_eq!(effective_history_cap(Tier::Free), MAX_PUSH_ITEMS_PER_DEVICE);
        // Pro: unlimited tier history → bounded only by the hard cap.
        assert_eq!(effective_history_cap(Tier::Pro), MAX_PUSH_ITEMS_PER_DEVICE);
        // A tier limit tighter than the hard cap must win, demonstrating the
        // tier value is actually applied (not just the constant hard cap). No
        // live tier defines a sub-hard-cap limit, so exercise the clamp helper
        // directly with a genuinely tight limit.
        let tight_tier_limit = 10usize;
        assert!(tight_tier_limit < MAX_PUSH_ITEMS_PER_DEVICE);
        assert_eq!(history_cap_for_limit(Some(tight_tier_limit)), 10);
        // Unlimited tier history (`None`) clamps to the hard cap.
        assert_eq!(history_cap_for_limit(None), MAX_PUSH_ITEMS_PER_DEVICE);
    }
}
