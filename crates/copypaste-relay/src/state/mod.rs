//! Relay in-memory state: device registry, sync-item inboxes, auth, and persistence.
//!
//! This module is the single shared mutable state of the relay process, guarded
//! by `std::sync::Mutex<RelayStore>` (the [`AppState`] alias).  Sub-modules
//! own cohesive groups of types and `impl` blocks:
//!
//! - [`device`]       — `DeviceRecord`, `TokenEntry`, token auth & activity tracking
//! - [`registration`] — rate-limiting, `register_device_*` family
//! - [`inbox`]        — `SyncItem`, push/pull/delete, SSE notifications
//! - [`eviction`]     — TTL pruning, inactive-device cleanup, orphan reclamation
//! - [`persistence`]  — `PushRetryQueue`, `PendingDbWrite`, retry-helper impl
//! - [`quota`]        — tier serialisation helpers, effective-cap computation, quota queries

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

pub mod device;
pub mod eviction;
pub mod inbox;
pub mod persistence;
pub mod quota;
pub mod registration;

// Re-export the domain types so callers of `crate::state::*` see a flat namespace.
pub use device::{DeviceRecord, TokenEntry};
pub use inbox::SyncItem;
// PendingDbWrite is used by `retry.rs` (`crate::state::PendingDbWrite`) but the
// `#[path]`-include integration-test binaries that compile state/mod.rs without
// retry.rs never reference it — suppress the spurious unused-import lint.
#[allow(unused_imports)]
pub use persistence::{PendingDbWrite, PushRetryQueue};
// Tier helpers: used by registration.rs (`super::tier_to_str`) and
// rehydrate_from_db below (`tier_from_str`).
pub(crate) use quota::{tier_from_str, tier_to_str};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Absolute hard cap on push-sync items per device inbox, independent of tier.
/// When exceeded, the oldest items (lowest wall_time) are pruned on insert.
/// Acts as a memory-safety ceiling that no tier may exceed.
// pub(crate) so submodule test blocks can import it directly.
pub(crate) const MAX_PUSH_ITEMS_PER_DEVICE: usize = 500;

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
pub(super) const REG_LIMIT_MAX_KEYS: usize = 10_000;

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
// Shared test helpers (all submodule test blocks import this)
// ---------------------------------------------------------------------------

/// Shared fixtures for state-module unit tests.
///
/// Importing `use super::test_helpers::*;` (or `use crate::state::test_helpers::*;`
/// from a sibling submodule) gives access to all helpers below.
#[cfg(test)]
pub(crate) mod test_helpers {
    use crate::state::RelayStore;

    use base64::engine::general_purpose::STANDARD as B64;
    use base64::Engine;

    pub fn make_store() -> RelayStore {
        RelayStore::new(3600)
    }

    pub fn valid_key_b64() -> String {
        B64.encode([0u8; 32])
    }

    /// A dummy 32-byte PoP for unit tests that don't exercise PoP verification.
    /// Uses a distinct non-zero byte pattern so first-registration stores it and
    /// subsequent calls using the same id can co-register with the same value.
    pub fn valid_pop_b64() -> String {
        B64.encode([0xDE_u8; 32])
    }

    pub fn device_a_id() -> String {
        "11111111-1111-1111-1111-111111111111".to_string()
    }

    pub fn device_b_id() -> String {
        "22222222-2222-2222-2222-222222222222".to_string()
    }

    pub fn unique_device_id(n: u8) -> String {
        format!("{n:02x}{n:02x}{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}-{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}{n:02x}")
    }

    pub fn unique_key(seed: u8) -> String {
        B64.encode([seed; 32])
    }

    pub fn push_text(store: &mut RelayStore, device_id: &str, wall_time: u64) -> i64 {
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
}
