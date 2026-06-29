//! Relay in-memory state: device registry, sync-item inboxes, auth, and persistence.
//!
//! This module is the single shared mutable state of the relay process, guarded
//! by `std::sync::Mutex<RelayStore>` (the [`AppState`] alias).  Sub-modules
//! own cohesive groups of types and `impl` blocks:
//!
//! - [`device`]      — `DeviceRecord`, `TokenEntry`, token auth & activity tracking
//! - [`registration`] — rate-limiting, `register_device_*` family
//! - [`inbox`]       — `SyncItem`, push/pull/delete, SSE notifications
//! - [`eviction`]    — TTL pruning, inactive-device cleanup, orphan reclamation
//! - [`persistence`] — `PushRetryQueue`, `PendingDbWrite`, retry-helper impl

use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use tokio::sync::{broadcast, Notify};

use crate::db::Db;
use crate::error::RelayError;
use crate::quota::Tier;

pub mod device;
pub mod eviction;
pub mod inbox;
pub mod persistence;
pub mod registration;

// Re-export the domain types so callers of `crate::state::*` see a flat namespace.
pub use device::{DeviceRecord, TokenEntry};
pub use inbox::SyncItem;
// PendingDbWrite is used by `retry.rs` (`crate::state::PendingDbWrite`) but the
// `#[path]`-include integration-test binaries that compile state/mod.rs without
// retry.rs never reference it — suppress the spurious unused-import lint.
#[allow(unused_imports)]
pub use persistence::{PendingDbWrite, PushRetryQueue};

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
/// applies). Enforced as a silent prune-oldest inside `RelayStore::push_item`
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

/// Default page size for `GET /devices/:id/items` when the caller does not
/// supply `limit`, and the absolute upper bound a single pull may return (M4).
/// Bounds the work done (clone + serialize) under the global store mutex on a
/// single request so one pull cannot amplify lock-hold time across a full
/// `MAX_PUSH_ITEMS_PER_DEVICE` inbox.
pub const DEFAULT_PULL_LIMIT: usize = 200;
/// Hard ceiling on a caller-supplied `limit`; larger values are clamped down.
pub const MAX_PULL_LIMIT: usize = 500;

/// Per-request byte-budget cap for `pull_items`.
///
/// Bounds the total bytes of `content_b64` cloned while the global store
/// mutex is held. Without this, a caller supplying `limit=500` against an
/// inbox full of 10 MiB items could force up to 5 GiB of cloning under the
/// lock, stalling every concurrent request (authenticated DoS). Expressed as
/// total base64-encoded bytes; items are accumulated in order and collection
/// stops once the running total would exceed this threshold. A legitimate
/// sync client fetching normal clipboard items (≤1 MiB each) hits this
/// limit only after >100 text items, well above typical usage.
pub const MAX_PULL_BYTES_BUDGET: usize = 128 * 1024 * 1024; // 128 MiB

/// Per-device registration-rate-limit window (security MEDIUM #13).
pub const REG_LIMIT_WINDOW: Duration = Duration::from_secs(60);
/// Maximum registration attempts allowed per device_id within `REG_LIMIT_WINDOW`.
pub const REG_LIMIT_MAX_ATTEMPTS: usize = 5;
/// Hard cap on the rate-limiter map size to bound memory if an attacker rotates device_ids.
const REG_LIMIT_MAX_KEYS: usize = 10_000;

/// Maximum number of pending DB writes queued for retry at any one time.
///
/// When the queue is at capacity, new failures are logged at WARN and discarded.
/// The item remains in the in-memory inbox so live pollers still see it, but
/// durability is lost for that item — same as the pre-fix behaviour, but only
/// as a last resort rather than always.
pub const PUSH_RETRY_QUEUE_CAP: usize = 64;

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
    pub(self) next_sync_id_per_device: HashMap<String, i64>,
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
    pub(self) reg_attempts: HashMap<(Option<IpAddr>, String), VecDeque<Instant>>,

    // -----------------------------------------------------------------------
    // Prometheus metrics counters (see api/metrics.rs)
    // -----------------------------------------------------------------------
    /// Monotonic counter: total sync items ever accepted by `push_item`.
    /// Never decremented (even when evicted) — this is a `counter` in
    /// Prometheus terms. Wrapped in `Arc<AtomicU64>` so the metrics
    /// endpoint can read it without holding the store mutex.
    pub(self) items_total: Arc<AtomicU64>,
    /// Monotonic counter: total sync items removed by `prune_expired`
    /// (TTL eviction). Counter — only ever incremented.
    pub(self) evictions_total: Arc<AtomicU64>,

    /// Operator-configured ceiling on how many items a single device inbox
    /// may hold. Sourced from `RelayConfig.max_items_per_device` (env:
    /// `RELAY_MAX_ITEMS_PER_DEVICE`). Defaults to
    /// `MAX_PUSH_ITEMS_PER_DEVICE` (500) when constructed via `new()`.
    /// The per-tier `effective_history_cap` still applies as a further
    /// tightener — this field is the *upper* ceiling over all tiers.
    pub(self) max_items_per_device: usize,

    /// Per-device SSE notification channels (issue #26). Each
    /// `broadcast::Sender<()>` is a *wake* signal: when an item is pushed into a
    /// device's inbox we `send(())` on that device's channel, waking every open
    /// `GET /devices/:id/subscribe` SSE stream so it re-reads the inbox from its
    /// own cursor and flushes the new item(s). Created lazily on first subscribe
    /// (see [`RelayStore::subscribe_notifier`]). The relay never sends item *data*
    /// over this channel — only a wake tick — so the broadcast value carries no
    /// plaintext and a lagged receiver simply re-reads the inbox (no data loss).
    /// Reclaimed alongside the device in `cleanup_inactive_devices` /
    /// `prune_expired` so the map stays bounded by the live device set.
    pub(self) sync_notifiers: HashMap<String, broadcast::Sender<()>>,

    /// Durable backing store (R1b). Every mutation to `devices` / their token
    /// sets / `sync_items` is written through to this SQLite database, and on
    /// construction (`new_persistent`) the in-memory maps above are rehydrated
    /// from it — so device records, token sets and inbox items survive a
    /// process restart. With the default `:memory:` path this is a private
    /// in-memory database (nothing persists across restart), preserving the
    /// pre-R1b behaviour for tests and ephemeral deploys.
    ///
    /// The connection lives inside this struct, which is itself behind the
    /// crate's `std::sync::Mutex<RelayStore>` ([`AppState`]); `Connection` is
    /// `Send` (not `Sync`), so the single shared `Mutex` serialises all access.
    /// Reads are served from memory, so the only SQLite work on the request
    /// path is a small bounded row write under the lock — matching the store's
    /// existing short-critical-section model. See `db.rs` for the full
    /// blocking/feature-unification rationale.
    pub(self) db: Db,

    /// Bounded queue of DB writes enqueued by `push_item_decoded`
    /// (CopyPaste-k4py / CopyPaste-crh3.70).
    ///
    /// Every `push_item_decoded` call enqueues its DB write here instead of
    /// performing it synchronously under the store mutex. The background
    /// `crate::retry::run_push_retry` task drains the queue, woken immediately
    /// by `db_write_notify` (or within `PUSH_RETRY_POLL_MS` as a safety net).
    /// This keeps the push handler's lock-hold to in-memory work only —
    /// SQLite I/O never happens while the store mutex is held by a push
    /// request.
    ///
    /// Bounded at `PUSH_RETRY_QUEUE_CAP`. When the queue is full, the DB
    /// write is logged at WARN and discarded — the item remains in the
    /// in-memory inbox for the process lifetime but will not survive a restart.
    pub push_retry_queue: PushRetryQueue,

    /// Wake signal for the DB-write task (CopyPaste-crh3.70).
    ///
    /// `push_item_decoded` calls `notify_one()` after every enqueue so the
    /// retry task drains the queue within microseconds rather than waiting for
    /// the `PUSH_RETRY_POLL_MS` fallback interval. Stored as `Arc<Notify>` so
    /// the retry task can clone it once at start-up and hold it OUTSIDE the
    /// store mutex — allowing it to `.await` the notification without a lock.
    pub(crate) db_write_notify: Arc<Notify>,
}

/// Map a [`Tier`] to its persisted string form.
pub(crate) fn tier_to_str(tier: Tier) -> &'static str {
    match tier {
        Tier::Free => "free",
        // `Tier::Pro` is cfg-gated; this arm only exists when the variant does.
        #[cfg(any(test, feature = "quota-tiers"))]
        Tier::Pro => "pro",
    }
}

/// Parse a persisted tier string back to a [`Tier`]. Unknown values fall back
/// to `Free` (the conservative default — never grant a wider quota than stored).
pub(crate) fn tier_from_str(s: &str) -> Tier {
    match s {
        // `Tier::Pro` is cfg-gated; fall back to `Free` in production builds
        // that don't enable the `quota-tiers` feature.
        #[cfg(any(test, feature = "quota-tiers"))]
        "pro" => Tier::Pro,
        _ => Tier::Free,
    }
}

impl RelayStore {
    /// Create a store with the default `MAX_PUSH_ITEMS_PER_DEVICE` inbox cap.
    // Used by unit/integration tests (`make_store()`) and integration test
    // binaries that `#[path]`-include state.rs. The production binary uses
    // `new_persistent` (via `main.rs`) so this constructor is never called
    // in production. When `quota-tiers` is enabled (e.g. `--all-features`)
    // the method is included but still has no non-test caller — allow suppresses
    // the dead_code lint while keeping the gating comment accurate.
    #[cfg(any(test, feature = "quota-tiers"))]
    #[allow(dead_code)] // intentional: test/integration helper, no production caller
    pub fn new(_sync_ttl_secs: u64) -> Self {
        Self::new_with_cap(_sync_ttl_secs, MAX_PUSH_ITEMS_PER_DEVICE)
    }

    /// Create a store with an explicit inbox cap (`max_items_per_device`),
    /// backed by an in-memory SQLite database (`:memory:`) — nothing persists
    /// across restart. Used by tests that verify the cap is honoured and by any
    /// caller that does not need durability. Opening an in-memory database is
    /// infallible in practice; a failure here is unrecoverable, so it panics
    /// with context (this constructor has no `Result` return for backward
    /// compatibility with the existing test call-sites). Production uses
    /// [`Self::new_persistent`], which surfaces open errors as `Result`.
    // Called only from `new` (cfg-gated) and test code; gated alongside `new`.
    // When `quota-tiers` is enabled (e.g. `--all-features`) the method is
    // included but still has no non-test caller — allow suppresses dead_code.
    #[cfg(any(test, feature = "quota-tiers"))]
    #[allow(dead_code)] // intentional: test helper, no production caller
    pub fn new_with_cap(sync_ttl_secs: u64, max_items_per_device: usize) -> Self {
        Self::new_persistent(
            sync_ttl_secs,
            max_items_per_device,
            crate::db::IN_MEMORY_PATH,
        )
        .expect("opening an in-memory relay database must not fail")
    }

    /// Create a store with an explicit inbox cap, backed by the SQLite database
    /// at `db_path`, rehydrating any persisted state into the in-memory maps
    /// (R1b). `":memory:"` selects a private in-memory database (no
    /// persistence); a file path makes device records, token sets and inbox
    /// items survive a process restart.
    ///
    /// Returns an error if the database cannot be opened or its existing
    /// contents cannot be loaded — callers (e.g. `main`) treat this as fatal
    /// rather than silently starting with an empty store.
    pub fn new_persistent(
        _sync_ttl_secs: u64,
        max_items_per_device: usize,
        db_path: &str,
    ) -> Result<Self, RelayError> {
        let db = Db::open(db_path)?;
        let mut store = Self {
            devices: HashMap::new(),
            sync_items: HashMap::new(),
            next_sync_id_per_device: HashMap::new(),
            reg_attempts: HashMap::new(),
            items_total: Arc::new(AtomicU64::new(0)),
            evictions_total: Arc::new(AtomicU64::new(0)),
            max_items_per_device,
            sync_notifiers: HashMap::new(),
            db,
            push_retry_queue: PushRetryQueue::new(),
            // CopyPaste-crh3.70: wake signal for the deferred DB-write task.
            db_write_notify: Arc::new(Notify::new()),
        };
        store.rehydrate_from_db()?;
        Ok(store)
    }

    /// Load every persisted device, token set and inbox from the backing
    /// database into the in-memory maps. Called once at construction.
    ///
    /// Stored Unix timestamps (`registered_at_unix`, `last_seen_unix`) are
    /// converted back to `Instant`s relative to the current monotonic clock:
    /// `instant = now - (now_unix - stored_unix)`, saturating at `now` for any
    /// timestamp that is in the future (clock skew). This preserves the
    /// inactivity-age semantics of `cleanup_inactive_devices` across restart —
    /// a device idle for N seconds before shutdown is still N seconds idle
    /// after reload (modulo downtime, which legitimately counts as idle).
    fn rehydrate_from_db(&mut self) -> Result<(), RelayError> {
        let loaded = self.db.load_all()?;
        if loaded.is_empty() {
            return Ok(());
        }
        let now_instant = Instant::now();
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        let to_instant = |stored_unix: i64| -> Instant {
            let age = now_unix.saturating_sub(stored_unix).max(0) as u64;
            now_instant
                .checked_sub(Duration::from_secs(age))
                .unwrap_or(now_instant)
        };

        for d in loaded {
            let device_id = d.device_id.clone();
            let registered_from_ip = d
                .registered_from_ip
                .as_deref()
                .and_then(|s| s.parse::<IpAddr>().ok());
            let tokens = d
                .tokens
                .into_iter()
                .map(|(token, expires_at_unix)| TokenEntry {
                    token,
                    expires_at_unix,
                })
                .collect();
            // Decode stored PoP from base64; fall back to zero-sentinel for
            // devices registered before the CopyPaste-n2l fix (NULL column).
            let registered_pop: [u8; 32] = d
                .registered_pop
                .as_deref()
                .and_then(|s| {
                    let bytes = B64.decode(s).ok()?;
                    if bytes.len() == 32 {
                        let mut arr = [0u8; 32];
                        arr.copy_from_slice(&bytes);
                        Some(arr)
                    } else {
                        None
                    }
                })
                .unwrap_or([0u8; 32]);
            self.devices.insert(
                device_id.clone(),
                DeviceRecord {
                    device_id: device_id.clone(),
                    device_name: d.device_name,
                    public_key_b64: d.public_key_b64,
                    registered_pop,
                    tokens,
                    registered_at: to_instant(d.registered_at_unix),
                    last_seen: to_instant(d.last_seen_unix),
                    tier: tier_from_str(&d.tier),
                    registered_from_ip,
                },
            );
            let inbox: Vec<SyncItem> = d
                .items
                .into_iter()
                .map(|it| SyncItem {
                    id: it.id,
                    content_type: it.content_type,
                    // CopyPaste-ux2i: own the base64 ciphertext as Arc<str>.
                    content_b64: std::sync::Arc::from(it.content_b64),
                    wall_time: it.wall_time,
                    inserted_at_unix: it.inserted_at_unix,
                })
                .collect();
            self.sync_items.insert(device_id.clone(), inbox);
            // next_sync_id is persisted; seed the in-memory counter from it so
            // restart never re-issues an id (security HIGH #3) even if the inbox
            // was fully drained before shutdown.
            self.next_sync_id_per_device
                .insert(device_id, d.next_sync_id.max(1));
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Stats
    // -----------------------------------------------------------------------

    // Not called from any current route handler (CopyPaste-j21: counts stripped
    // from unauthenticated endpoints). When `quota-tiers` is enabled (e.g.
    // --all-features) it is included but has no non-test caller — allow
    // suppresses dead_code.
    #[cfg(any(test, feature = "quota-tiers"))]
    #[allow(dead_code)] // intentional: test helper, no production caller today
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

pub(crate) fn hex_encode(bytes: &[u8]) -> String {
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
    /// A dummy 32-byte PoP for unit tests that don't exercise PoP verification.
    /// Uses a distinct non-zero byte pattern so first-registration stores it and
    /// subsequent calls using the same id can co-register with the same value.
    fn valid_pop_b64() -> String {
        B64.encode([0xDE_u8; 32])
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // CopyPaste-qvtg.3: 32-byte (256-bit) token → 64 hex chars.
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(expires_at > 0);
    }

    #[test]
    fn issued_token_is_not_born_expired() {
        // Guards the near-epoch-clock outage fix: under any correctly-set host
        // clock the issued token must expire in the future (~1 year out), never
        // at-or-before "now". A bogus near-epoch clock previously yielded
        // `expires_at_unix ≈ 365d`, which is far in the past today, so every
        // device would be Unauthorized on its next request.
        let mut store = make_store();
        let (token, expires_at) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let now_unix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("test host clock must be past the epoch")
            .as_secs() as i64;
        assert!(
            expires_at > now_unix,
            "token must not be born expired: expires_at={expires_at}, now={now_unix}"
        );
        // Roughly one year out (allow a few seconds of test scheduling slack).
        let one_year = 365 * 24 * 3600;
        assert!((expires_at - now_unix - one_year).abs() < 60);
        // And the freshly-issued token must actually verify.
        assert!(store.verify_token(&device_a_id(), &token).is_ok());
    }

    #[test]
    fn co_registration_mints_new_token_and_both_are_valid() {
        // R1a: re-registering an already-registered device_id no longer
        // conflicts — it co-registers, minting a fresh independent token while
        // keeping the original valid. BOTH tokens must authorize.
        let mut store = make_store();
        let (token1, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let (token2, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        assert_ne!(token1, token2, "co-registration must mint a distinct token");
        assert!(
            store.verify_token(&device_a_id(), &token1).is_ok(),
            "the original token must remain valid after co-registration"
        );
        assert!(
            store.verify_token(&device_a_id(), &token2).is_ok(),
            "the co-registered token must also be valid"
        );
        // Still one device record / one inbox — co-registration shares the inbox.
        assert_eq!(store.devices.len(), 1);
        assert_eq!(store.devices[&device_a_id()].tokens.len(), 2);
    }

    #[test]
    fn token_cap_evicts_oldest() {
        // Issuing more than MAX_TOKENS_PER_DEVICE tokens for one device_id
        // evicts the oldest (FIFO): the first-issued token stops authorizing
        // once the cap is exceeded, while the most recent cap-worth stay valid.
        let mut store = make_store();
        let mut tokens = Vec::new();
        for _ in 0..(device::MAX_TOKENS_PER_DEVICE + 1) {
            let (t, _) = store
                .register_device(
                    device_a_id(),
                    "Device A".into(),
                    valid_key_b64(),
                    valid_pop_b64(),
                )
                .unwrap();
            tokens.push(t);
        }
        assert_eq!(
            store.devices[&device_a_id()].tokens.len(),
            device::MAX_TOKENS_PER_DEVICE,
            "token set must be capped at MAX_TOKENS_PER_DEVICE"
        );
        // The very first token was evicted (oldest), the rest still authorize.
        assert!(
            store.verify_token(&device_a_id(), &tokens[0]).is_err(),
            "oldest token must be evicted past the cap"
        );
        for t in &tokens[1..] {
            assert!(
                store.verify_token(&device_a_id(), t).is_ok(),
                "the most recent cap-worth of tokens must remain valid"
            );
        }
    }

    #[test]
    fn expired_token_is_pruned_on_co_registration() {
        // add_token reclaims space from already-expired tokens before applying
        // the oldest-eviction cap: an expired token is dropped on the next
        // co-registration rather than counting toward the cap.
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // Forcibly expire the sole token.
        store.devices.get_mut(&device_a_id()).unwrap().tokens[0].expires_at_unix = 1;
        // Co-register: the expired entry is pruned, leaving only the new token.
        let (fresh, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let tokens = &store.devices[&device_a_id()].tokens;
        assert_eq!(tokens.len(), 1, "expired token must be pruned on add");
        assert_eq!(tokens[0].token, fresh);
        assert!(store.verify_token(&device_a_id(), &fresh).is_ok());
    }

    #[test]
    fn verify_token_ok() {
        let mut store = make_store();
        let (token, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        assert!(store.verify_token(&device_a_id(), &token).is_ok());
    }

    #[test]
    fn verify_token_wrong_token_is_unauthorized() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // Forcibly expire the device's sole token by rewinding `expires_at_unix`.
        let record = store.devices.get_mut(&device_a_id()).unwrap();
        let token = record.tokens[0].token.clone();
        record.tokens[0].expires_at_unix = 1; // 1970-01-01
        let err = store.verify_token(&device_a_id(), &token).unwrap_err();
        assert!(matches!(err, RelayError::Unauthorized));
    }

    #[test]
    fn get_device_returns_correct_info() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "My Mac".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let record = store.get_device(&device_a_id()).unwrap();
        assert_eq!(record.device_id, device_a_id());
        assert_eq!(record.device_name, "My Mac");
        assert_eq!(record.public_key_b64, valid_key_b64());
        assert!(record.latest_expires_at() > 0);
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        let id1 = push_text(&mut store, &device_a_id(), 1000);
        let id2 = push_text(&mut store, &device_a_id(), 2000);
        assert!(id2 > id1);
    }

    #[test]
    fn pull_returns_items_since_wall_time() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        push_text(&mut store, &device_a_id(), 1000);
        push_text(&mut store, &device_a_id(), 2000);
        push_text(&mut store, &device_a_id(), 3000);
        let items = store
            .pull_items(&device_a_id(), 1000, None, usize::MAX)
            .unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].wall_time, 2000);
        assert_eq!(items[1].wall_time, 3000);
    }

    #[test]
    fn pull_since_zero_returns_all() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        push_text(&mut store, &device_a_id(), 100);
        push_text(&mut store, &device_a_id(), 200);
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        assert_eq!(items.len(), 2);
    }

    #[test]
    fn pull_sorted_ascending_by_wall_time() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        push_text(&mut store, &device_a_id(), 3000);
        push_text(&mut store, &device_a_id(), 1000);
        push_text(&mut store, &device_a_id(), 2000);
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        let times: Vec<u64> = items.iter().map(|i| i.wall_time).collect();
        assert_eq!(times, vec![1000, 2000, 3000]);
    }

    #[test]
    fn push_rejects_unknown_content_type() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        for t in 1u64..=(MAX_PUSH_ITEMS_PER_DEVICE as u64 + 1) {
            push_text(&mut store, &device_a_id(), t);
        }
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        assert_eq!(items.len(), MAX_PUSH_ITEMS_PER_DEVICE);
        let min_wt = items.iter().map(|i| i.wall_time).min().unwrap();
        assert_eq!(min_wt, 2, "oldest item must be evicted");
    }

    #[test]
    fn pull_returns_device_not_found_for_unknown_device() {
        let store = make_store();
        let err = store
            .pull_items("unknown-device", 0, None, usize::MAX)
            .unwrap_err();
        assert!(matches!(err, RelayError::DeviceNotFound));
    }

    #[test]
    fn stats_counts_correctly() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        store
            .register_device(
                device_b_id(),
                "Device B".into(),
                B64.encode([1u8; 32]),
                valid_pop_b64(),
            )
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        store
            .register_device(
                device_b_id(),
                "Device B".into(),
                B64.encode([1u8; 32]),
                valid_pop_b64(),
            )
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        store
            .register_device(
                device_b_id(),
                "Device B".into(),
                B64.encode([1u8; 32]),
                valid_pop_b64(),
            )
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
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
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
                    valid_pop_b64(),
                    Tier::Free,
                )
                .unwrap();
        }
        let err = store
            .register_device_with_tier(
                unique_device_id(5),
                "Dev 5".into(),
                unique_key(5),
                valid_pop_b64(),
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
                    valid_pop_b64(),
                    Tier::Free,
                )
                .unwrap();
        }
        store
            .register_device_with_tier(
                unique_device_id(4),
                "Dev 4".into(),
                unique_key(4),
                valid_pop_b64(),
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
                    valid_pop_b64(),
                    Tier::Pro,
                )
                .unwrap();
        }
        let err = store
            .register_device_with_tier(
                unique_device_id(10),
                "Dev 10".into(),
                unique_key(10),
                valid_pop_b64(),
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
                    valid_pop_b64(),
                    Tier::Free,
                )
                .unwrap();
        }
        let err = store
            .register_device(
                unique_device_id(5),
                "Dev 5".into(),
                unique_key(5),
                valid_pop_b64(),
            )
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
                valid_pop_b64(),
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

        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
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
            .register_device_with_tier(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
                Tier::Pro,
            )
            .unwrap();

        for t in 1u64..=(MAX_PUSH_ITEMS_PER_DEVICE as u64 + 50) {
            push_text(&mut store, &device_a_id(), t);
        }

        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
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

    // ---- CopyPaste-1uqb: prune by server sync_id, not client wall_time --------

    /// CopyPaste-1uqb: When the inbox overflows its cap, the items evicted must
    /// be chosen by server-assigned `id` (smallest = earliest arrival), not by
    /// client-supplied `wall_time`. An intra-account attacker can forge a low
    /// `wall_time` to make their item appear "old" in wall_time order, displacing
    /// legitimate items during overflow eviction while their own survives.
    ///
    /// This test sets a cap of 2 and pushes three items: a "legitimate" item at
    /// wall_time=1000, then an "attacker" item at wall_time=1 (back-dated), then
    /// another "legitimate" item at wall_time=2000. Under wall_time eviction the
    /// item at wall_time=1 (the attacker's) would survive (it sorts as "newest" in
    /// the wall_time min-heap) while the original item at wall_time=1000 would be
    /// evicted. Under server-id eviction, the oldest server-assigned item is evicted
    /// regardless of its wall_time.
    #[test]
    fn inbox_overflow_evicts_by_server_id_not_client_wall_time() {
        let mut store = RelayStore::new_with_cap(3600, 2 /* cap = 2 items */);
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();

        // Push item 1 at wall_time=1000 (server id = 1).
        let id_first = store
            .push_item(
                &device_a_id(),
                "text".to_string(),
                B64.encode(b"first"),
                1000,
                10 * 1024 * 1024,
            )
            .unwrap();

        // Push attacker's item at wall_time=1 (back-dated; server id = 2).
        let id_attacker = store
            .push_item(
                &device_a_id(),
                "text".to_string(),
                B64.encode(b"attacker"),
                1, // deliberately old wall_time to appear "oldest" in wall_time order
                10 * 1024 * 1024,
            )
            .unwrap();

        // Push item 3 at wall_time=2000 (server id = 3); this should trigger eviction.
        // After eviction the inbox must have exactly 2 items.
        let _id_third = store
            .push_item(
                &device_a_id(),
                "text".to_string(),
                B64.encode(b"third"),
                2000,
                10 * 1024 * 1024,
            )
            .unwrap();

        let remaining: Vec<i64> = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap()
            .into_iter()
            .map(|it| it.id)
            .collect();

        // The server assigned id_first=1, id_attacker=2, id_third=3.
        // Eviction by id_ASC must remove id_first (smallest id = earliest arrival).
        // id_attacker and id_third must remain.
        assert_eq!(remaining.len(), 2, "inbox must be at cap after overflow");
        assert!(
            !remaining.contains(&id_first),
            "CopyPaste-1uqb: the earliest-arrived item (id={id_first}) must be evicted"
        );
        assert!(
            remaining.contains(&id_attacker),
            "CopyPaste-1uqb: attacker item (id={id_attacker}, wall_time=1) must NOT survive \
             by appearing oldest in wall_time order — it arrived AFTER id_first"
        );
        assert!(
            remaining.contains(&_id_third),
            "the third item must survive"
        );
    }

    // ---- H1: per-scope device quota ----------------------------------------

    use std::net::{IpAddr, Ipv4Addr};

    fn ip(n: u8) -> Option<IpAddr> {
        Some(IpAddr::V4(Ipv4Addr::new(10, 0, 0, n)))
    }

    /// The device-count quota must be *per scope* (per registering IP), not a
    /// global ceiling: distinct IPs each get their own full free-tier budget,
    /// so a global "5 devices total" cap can no longer reject legitimate users.
    #[test]
    fn device_quota_is_per_scope_not_global() {
        let mut store = make_store();
        // Scope A fills its 5-device free budget.
        for i in 0u8..5 {
            store
                .register_device_with_tier_scoped(
                    ip(1),
                    unique_device_id(i),
                    format!("A{i}"),
                    unique_key(i),
                    valid_pop_b64(),
                    Tier::Free,
                )
                .expect("scope A device must be accepted");
        }
        // A 6th device from scope A is rejected (its own scope is full)...
        let err = store
            .register_device_with_tier_scoped(
                ip(1),
                unique_device_id(5),
                "A5".into(),
                unique_key(5),
                valid_pop_b64(),
                Tier::Free,
            )
            .unwrap_err();
        assert!(matches!(err, RelayError::DeviceQuotaExceeded { limit: 5 }));

        // ...but a device from a *different* IP (scope B) still registers fine,
        // even though the relay already holds 5 devices globally. A global cap
        // would have rejected this; a per-scope cap accepts it.
        store
            .register_device_with_tier_scoped(
                ip(2),
                unique_device_id(6),
                "B0".into(),
                unique_key(6),
                valid_pop_b64(),
                Tier::Free,
            )
            .expect("a different scope must get its own device budget");
        assert_eq!(store.stats().0, 6, "relay holds 6 devices across 2 scopes");
    }

    // ---- H2: orphan-map reclamation ----------------------------------------

    /// `prune_expired` must reclaim `next_sync_id_per_device` counters and empty
    /// `sync_items` inboxes whose device record no longer exists, so those maps
    /// stay bounded by the live device set instead of leaking forever.
    #[test]
    fn prune_expired_reclaims_orphaned_maps() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        push_text(&mut store, &device_a_id(), 1000);
        // Counter + inbox now exist for device A.
        assert!(store.next_sync_id_per_device.contains_key(&device_a_id()));
        assert!(store.sync_items.contains_key(&device_a_id()));

        // Forcibly drop *only* the device record, simulating a record removed
        // by some path that left the side maps behind.
        store.devices.remove(&device_a_id());

        let now = 1_000_000u64;
        store.prune_expired(now, 60);

        assert!(
            !store.next_sync_id_per_device.contains_key(&device_a_id()),
            "orphaned id-counter entry must be reclaimed"
        );
        assert!(
            !store.sync_items.contains_key(&device_a_id()),
            "orphaned inbox must be reclaimed"
        );
    }

    /// Empty inboxes belonging to a *still-registered* device must be kept (the
    /// device retains its registration regardless of inbox activity).
    #[test]
    fn prune_expired_keeps_empty_inbox_of_registered_device() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        // Empty inbox, device still registered.
        store.prune_expired(u64::MAX, 1);
        assert!(store.sync_items.contains_key(&device_a_id()));
    }

    // ---- M4: paginated pull -------------------------------------------------

    /// `pull_items` must honor `limit`, returning at most `limit` items from the
    /// `since` window in ascending `wall_time` order.
    #[test]
    fn pull_items_respects_limit() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        for t in 1u64..=10 {
            push_text(&mut store, &device_a_id(), t);
        }
        let page = store.pull_items(&device_a_id(), 0, None, 3).unwrap();
        assert_eq!(page.len(), 3, "limit must cap the page size");
        assert_eq!(
            page.iter().map(|i| i.wall_time).collect::<Vec<_>>(),
            vec![1, 2, 3],
            "page must be the earliest items in the since-window, ascending"
        );
    }

    /// Pagination via `since` + `limit` must walk the whole window without gaps
    /// or duplicates when the client feeds the last `wall_time` back as `since`.
    #[test]
    fn pull_items_pagination_walks_window() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        for t in 1u64..=5 {
            push_text(&mut store, &device_a_id(), t);
        }
        let mut seen = Vec::new();
        let mut since = 0u64;
        loop {
            let page = store.pull_items(&device_a_id(), since, None, 2).unwrap();
            if page.is_empty() {
                break;
            }
            since = page.last().unwrap().wall_time;
            seen.extend(page.iter().map(|i| i.wall_time));
        }
        assert_eq!(seen, vec![1, 2, 3, 4, 5]);
    }

    /// Relay H-1 / audit finding G: pagination must not silently drop items
    /// when a page boundary falls in the middle of a run of *equal*
    /// sender-supplied `wall_time` values. Three items share `wall_time == 10`;
    /// with `limit == 2` the boundary lands mid-run.
    ///
    /// Teeth: under the OLD `wall_time`-only cursor the client would feed
    /// `since = 10` (the last page's `wall_time`) back with a strict `>` floor,
    /// which skips BOTH remaining `wall_time == 10` items → the third item is
    /// lost. The composite `(wall_time, id)` cursor (feeding back `since_id`)
    /// advances only past the exact item already seen, so the run is walked in
    /// full with no gaps and no duplicates. This test passes only with the tuple
    /// cursor; the `assert_eq!(seen_ids, [id1, id2, id3])` below fails under the
    /// legacy `since`-only pagination.
    #[test]
    fn pull_items_pagination_no_drop_on_tied_wall_times() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        // Three items, identical wall_time, distinct ascending ids.
        let id1 = push_text(&mut store, &device_a_id(), 10);
        let id2 = push_text(&mut store, &device_a_id(), 10);
        let id3 = push_text(&mut store, &device_a_id(), 10);

        // Page 1: limit 2 → the boundary splits the tied run.
        let page1 = store.pull_items(&device_a_id(), 0, None, 2).unwrap();
        assert_eq!(page1.len(), 2, "first page returns 2 of the 3 tied items");
        assert_eq!(
            page1.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![id1, id2]
        );

        // Page 2: feed back the composite cursor (wall_time, id) of the last
        // item seen. With the tuple cursor this returns the third item; with the
        // old wall_time-only `since = 10` strict-`>` floor it would return empty.
        let last = page1.last().unwrap();
        let page2 = store
            .pull_items(&device_a_id(), last.wall_time, Some(last.id), 2)
            .unwrap();
        assert_eq!(
            page2.iter().map(|i| i.id).collect::<Vec<_>>(),
            vec![id3],
            "composite cursor must return the remaining tied item, not drop it"
        );

        // Full walk must see every item exactly once (no gap, no dup).
        let mut seen_ids = Vec::new();
        let mut since = 0u64;
        let mut since_id: Option<i64> = None;
        loop {
            let page = store
                .pull_items(&device_a_id(), since, since_id, 2)
                .unwrap();
            if page.is_empty() {
                break;
            }
            let last = page.last().unwrap();
            since = last.wall_time;
            since_id = Some(last.id);
            seen_ids.extend(page.iter().map(|i| i.id));
        }
        assert_eq!(
            seen_ids,
            vec![id1, id2, id3],
            "tuple-cursor pagination must walk all tied-wall_time items with no drops"
        );
    }

    // ---- update_last_seen / cleanup_inactive_devices interaction ------------

    /// Positive: a device that calls `update_last_seen` after the inactivity
    /// threshold has elapsed must SURVIVE `cleanup_inactive_devices` — last_seen
    /// is reset to "now" so the threshold is no longer exceeded.
    ///
    /// This guards the fix for the prior bug where `update_last_seen` was
    /// defined but never called from the route handlers, so `last_seen` stayed
    /// equal to `registered_at` and active devices were evicted after 30 days.
    #[test]
    fn update_last_seen_prevents_eviction_after_threshold() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();

        // Simulate the device's last_seen being past the threshold by rewinding
        // it to a time far in the past (subtract threshold + 1 second).
        let threshold_secs = 60u64; // use a small finite threshold for the test
        {
            let record = store.devices.get_mut(&device_a_id()).unwrap();
            // Rewind last_seen by (threshold + 1) seconds so cleanup would
            // normally evict this device.
            record.last_seen = Instant::now() - Duration::from_secs(threshold_secs + 1);
        }

        // Verify that WITHOUT update_last_seen the device would be evicted.
        // (Snapshot the current last_seen before calling update.)
        let would_evict = store
            .devices
            .get(&device_a_id())
            .map(|r| r.last_seen.elapsed().as_secs() >= threshold_secs)
            .unwrap_or(false);
        assert!(
            would_evict,
            "precondition: device should be evictable before update_last_seen"
        );

        // Now call update_last_seen (simulating what the route handler does
        // after a successful verify_token).
        store.update_last_seen(&device_a_id());

        // cleanup_inactive_devices with the same finite threshold must NOT
        // evict the device whose last_seen was just refreshed.
        let removed = store.cleanup_inactive_devices(threshold_secs);
        assert_eq!(
            removed, 0,
            "device must survive cleanup after update_last_seen refreshes last_seen"
        );
        assert!(
            store.devices.contains_key(&device_a_id()),
            "device must still be registered"
        );
    }

    /// Negative: a device that does NOT call `update_last_seen` after the
    /// inactivity threshold has elapsed must be EVICTED by
    /// `cleanup_inactive_devices`.
    ///
    /// This is the counterpart to `update_last_seen_prevents_eviction_after_threshold`:
    /// it proves that cleanup actually reaps stale devices, so the positive test
    /// above is meaningful (it would trivially pass if cleanup never evicted anything).
    #[test]
    fn no_update_last_seen_causes_eviction_after_threshold() {
        let mut store = make_store();
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();

        let threshold_secs = 60u64;

        // Rewind last_seen past the threshold — no update_last_seen called.
        {
            let record = store.devices.get_mut(&device_a_id()).unwrap();
            record.last_seen = Instant::now() - Duration::from_secs(threshold_secs + 1);
        }

        // cleanup must evict the device because last_seen is stale and inbox
        // is empty.
        let removed = store.cleanup_inactive_devices(threshold_secs);
        assert_eq!(
            removed, 1,
            "stale device with no last_seen update must be evicted"
        );
        assert!(
            !store.devices.contains_key(&device_a_id()),
            "device must be gone after eviction"
        );
    }

    // ---- Fix 1: verify_token fail-closed on clock error --------------------

    /// When `verify_token` encounters a clock error it must fail CLOSED (return
    /// `Unauthorized`), not silently treat `now_unix=0` as "valid" (the old
    /// `unwrap_or_default()` behaviour that made `0 <= expires_at_unix` always
    /// true). We test the internal helper `verify_token_at` directly, injecting
    /// a simulated clock error via `None` for `now_unix`.
    #[test]
    fn verify_token_clock_error_returns_unauthorized() {
        let mut store = make_store();
        let (token, _) = store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // None = simulated clock error → must fail closed.
        let err = store
            .verify_token_at(&device_a_id(), &token, None)
            .unwrap_err();
        assert!(
            matches!(err, RelayError::Unauthorized),
            "clock error must fail closed: got {err:?}"
        );
    }

    // ---- Fix 3: max_items_per_device wired from config ----------------------

    /// The config `max_items_per_device` must govern the inbox cap. A store
    /// constructed with `new_with_cap(N)` must enforce N as the hard ceiling,
    /// not the compile-time `MAX_PUSH_ITEMS_PER_DEVICE` (500).
    #[test]
    fn max_items_per_device_config_governs_cap() {
        const CUSTOM_CAP: usize = 5;
        let mut store = RelayStore::new_with_cap(3600, CUSTOM_CAP);
        store
            .register_device(
                device_a_id(),
                "Device A".into(),
                valid_key_b64(),
                valid_pop_b64(),
            )
            .unwrap();
        // Push more items than the custom cap.
        for t in 1u64..=(CUSTOM_CAP as u64 + 3) {
            push_text(&mut store, &device_a_id(), t);
        }
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        assert_eq!(
            items.len(),
            CUSTOM_CAP,
            "inbox must be capped at the config-supplied max_items_per_device ({CUSTOM_CAP}), got {}",
            items.len()
        );
    }

    /// Out-of-order pushes must still be returned ascending by `wall_time`,
    /// proving the on-insert sort (M4) keeps the inbox ordered.
    #[test]
    fn pull_items_ordered_after_out_of_order_push() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();
        for t in [50u64, 10, 30, 20, 40] {
            push_text(&mut store, &device_a_id(), t);
        }
        let items = store
            .pull_items(&device_a_id(), 0, None, usize::MAX)
            .unwrap();
        assert_eq!(
            items.iter().map(|i| i.wall_time).collect::<Vec<_>>(),
            vec![10, 20, 30, 40, 50]
        );
    }

    // ---- CopyPaste-0y04: SSE per-device connection cap ----------------------

    /// CopyPaste-0y04: `notifier_receiver_count` must reflect the number of
    /// live SSE receivers for a device. Subscribing N times must increment the
    /// count, and dropping a receiver must decrement it (broadcast semantics).
    #[test]
    fn sse_receiver_count_tracks_live_subscriptions() {
        let mut store = make_store();
        store
            .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
            .unwrap();

        assert_eq!(
            store.notifier_receiver_count(&device_a_id()),
            0,
            "no subscriptions initially"
        );

        // Subscribe once: count must be 1.
        let rx1 = store.subscribe_notifier(&device_a_id());
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 1);

        // Subscribe again: count must be 2.
        let rx2 = store.subscribe_notifier(&device_a_id());
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 2);

        // Drop one receiver: count must drop to 1.
        drop(rx1);
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 1);

        // Drop the last: count must return to 0.
        drop(rx2);
        assert_eq!(store.notifier_receiver_count(&device_a_id()), 0);
    }

    /// CopyPaste-0y04: a fresh device starts with 0 SSE receivers, and calling
    /// `notifier_receiver_count` on an unknown device also returns 0 (no panic).
    #[test]
    fn sse_receiver_count_returns_zero_for_unknown_device() {
        let store = make_store();
        // Device "aaaa…" is never registered: must return 0, not panic.
        assert_eq!(
            store.notifier_receiver_count(&device_a_id()),
            0,
            "notifier_receiver_count for unknown device must be 0"
        );
    }

    // ---- CopyPaste-hf40: next_sync_id watermark persisted across restart ----

    /// CopyPaste-hf40: the `next_sync_id` counter (relay watermark) must be
    /// rehydrated from the database on startup. Simulated by:
    ///   1. Create store A (on-disk SQLite via `new_persistent`).
    ///   2. Push N items → counter advances to N+1.
    ///   3. Create store B reloading from the same on-disk DB → must seed from N+1.
    ///   4. Push one more item in store B → must get server id N+1 (not 1).
    ///
    /// Note: the default `make_store()` uses `:memory:` which is private to
    /// each open. We create a store with an on-disk temp DB here so we can
    /// actually reload it.
    #[test]
    fn next_sync_id_watermark_is_seeded_from_db_on_restart() {
        // Create a temp file path for the DB.
        let dir = std::env::temp_dir().join(format!(
            "copypaste-relay-hf40-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("relay.db").to_str().unwrap().to_string();

        // Store A: register device, push 3 items → last pushed id must be 3.
        let last_id_a = {
            let mut store = RelayStore::new_persistent(3600, 500, &db_path).unwrap();
            store
                .register_device(device_a_id(), "A".into(), valid_key_b64(), valid_pop_b64())
                .unwrap();
            push_text(&mut store, &device_a_id(), 10); // id=1
            push_text(&mut store, &device_a_id(), 20); // id=2
            push_text(&mut store, &device_a_id(), 30) // id=3
        };
        assert_eq!(last_id_a, 3, "first store: last pushed id must be 3");

        // Store B: open the same on-disk DB and reload via `new_persistent`.
        // The next push must continue from id=4, NOT restart from 1.
        let first_id_b = {
            let mut store = RelayStore::new_persistent(3600, 500, &db_path).unwrap();
            push_text(&mut store, &device_a_id(), 40) // must be id=4
        };
        assert_eq!(
            first_id_b,
            4,
            "CopyPaste-hf40: after restart the first new push must get id={} (continuation), \
             not 1 (restart from scratch); got {}",
            last_id_a + 1,
            first_id_b
        );

        // Clean up.
        std::fs::remove_dir_all(&dir).ok();
    }
}
