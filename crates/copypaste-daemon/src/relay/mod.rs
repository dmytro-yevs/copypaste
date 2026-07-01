//! Relay-as-database sync client (sync path #2 of 3) — daemon side.
//!
//! This is the producer/consumer that makes the **relay-as-database** path work
//! end-to-end, independent of P2P and Supabase. It is gated behind the
//! `relay-sync` cargo feature and is active at runtime iff `config.relay_url`
//! is set.
//!
//! # Architecture — shared-account inbox
//!
//! ALL of an account's devices use ONE relay inbox `device_id`, derived
//! deterministically from the shared sync key
//! ([`copypaste_core::derive_relay_inbox_id`]). Every device co-registers that
//! id with the relay (each gets an INDEPENDENT auth token — R1a), then pushes to
//! and subscribes to it. The relay only ever sees opaque ciphertext + the opaque
//! inbox id.
//!
//! # Pipeline (mirrors [`crate::cloud`])
//!
//! - **register:** `POST {relay_url}/devices` `{device_id, device_name,
//!   public_key_b64}` → 201 `{auth_token}`. Token cached in a `0600` file. On a
//!   401 during push/pull the token is dropped and re-registered.
//! - **push:** subscribe the `new_item_tx` broadcast; for each local item reuse
//!   `sync_common::decrypt_item_plaintext` →
//!   `sync_common::wrap_and_check_cloud_upload_plaintext` →
//!   `encrypt_for_cloud(sync_key, item_id, ...)` (the SAME blob the Supabase path
//!   produces) → build the envelope → `POST {relay_url}/devices/{inbox}/items`.
//! - **receive:** poll `GET {relay_url}/devices/{inbox}/items?since=&since_id=`,
//!   decode each item's envelope, `decrypt_from_cloud`, then reuse
//!   `sync_common::build_local_item` + [`copypaste_core::insert_item`] with the
//!   exact LWW + quota-prune the Supabase poll path uses. A `(wall_time, id)`
//!   watermark is held in memory across ticks.
//! - **self-echo:** the daemon both pushes to and subscribes to the same inbox,
//!   so a row it pushed comes back on the next pull. LWW dedup on `item_id`
//!   makes that a no-op (the local copy has an equal `lamport_ts`, so it is
//!   skipped) — confirmed by the receive path's `<=` LWW guard.
//!
//! # Multi-transport topology (dtq3)
//!
//! Relay and Supabase (cloud) are **additive, independent transports**: both can
//! run simultaneously when `relay_url` is set AND `SUPABASE_URL` is set.  Each
//! subscribes to the same `new_item_tx` broadcast, so a locally-captured item is
//! published to both backends.
//!
//! **No duplicate-apply risk**: a peer that is subscribed to BOTH transports may
//! receive the same `item_id` twice (once from relay, once from Supabase).  The
//! LWW dedup guard in `ingest_page_blocking` (and its mirror in `cloud.rs`) uses
//! `get_item_by_item_id` + `remote_wins` on every ingested row.  The second
//! arrival for the same `item_id` sees `lamport_ts <= existing` and is skipped —
//! the DB is left with exactly one row per logical item regardless of how many
//! transports delivered it.  This is verified by the
//! `both_transports_deliver_same_item_inserts_exactly_once` unit test.
//!
//! **Android note (still needed — dtq3)**: Android currently models relay and
//! Supabase as mutually-exclusive `SyncBackend` enum variants and publishes to
//! exactly one.  The `RelaySubscriptionClient` may still receive items over relay
//! even when Supabase is the selected backend.  Android should be updated to apply
//! the same LWW dedup on the receiver side (the guard already exists in the Kotlin
//! relay SSE ingest path as an `item_id` check; confirm it fires on the cloud path
//! too and add a test).  No Kotlin changes are included here.
//!
//! # Security
//! - The inbox id is SECRET-derived (HKDF of the sync key) — NEVER logged.
//! - The auth token is a credential — NEVER logged; persisted `0600`.
//! - The relay sees only ciphertext; plaintext/key bytes are never logged.
//! - All HTTP is async (reqwest) — the tokio runtime is never blocked; the only
//!   blocking work (SQLCipher writes, AEAD) runs in `spawn_blocking`.

use std::sync::atomic::AtomicI64;
use std::sync::Arc;
use std::time::Duration;

use copypaste_core::{AppConfig, ClipboardItem, Database, SyncKey};
use tokio::sync::{Mutex, Notify};

// ── Sub-modules ───────────────────────────────────────────────────────────────

mod pasteboard;
mod push;
mod receive;
mod registration;
#[cfg(test)]
mod testutil;
mod token;
mod types;
mod watermark;
mod wire;

// ── Public re-exports ─────────────────────────────────────────────────────────

pub use types::{RelayError, RelayHandle};

// ── Poll-interval constants (shared between push and receive) ─────────────────

/// Minimum poll interval for the receive loop (applied when items are arriving
/// so cross-device latency stays low). After [`IDLE_EMPTY_POLL_THRESHOLD`]
/// consecutive empty polls the interval grows linearly up to [`POLL_INTERVAL_MAX`]
/// (CopyPaste-28br: idle back-off to reduce battery drain and relay load).
pub(super) const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Number of consecutive empty polls before the interval starts growing.
/// After this many no-op ticks the daemon clearly has no new items to fetch.
pub(super) const IDLE_EMPTY_POLL_THRESHOLD: u32 = 3;

/// Step size for each idle back-off increment (60 s per step, so the first
/// idle step immediately jumps to ≥ 60 s — satisfying the acceptance criterion
/// of "≥ 60 s after 3 consecutive empty polls"). The interval grows as
/// `IDLE_POLL_STEP * step_count`, capped at [`POLL_INTERVAL_MAX`].
pub(super) const IDLE_POLL_STEP: Duration = Duration::from_secs(60);

/// Maximum idle poll interval (5 minutes). The interval grows linearly in
/// `IDLE_POLL_STEP`-sized increments up to this cap.  A non-empty response
/// resets the interval immediately back to `POLL_INTERVAL`.
pub(super) const POLL_INTERVAL_MAX: Duration = Duration::from_secs(5 * 60);

/// Max items requested per pull tick. When a batch comes back full we re-poll
/// immediately (burst drain) rather than waiting a full interval.
pub(super) const PULL_LIMIT: usize = 50;

// ── relay_url sentinel ───────────────────────────────────────────────────────

/// Returns `true` when `url` represents an explicit "clear / disable relay"
/// intent, i.e. when it is `None` or an empty / whitespace-only string.
///
/// ## Sentinel contract
///
/// The IPC `set_config` handler cannot write `relay_url = None` to `config.toml`
/// directly because `update_core_config` only writes `Some(v)` values (the
/// field is optional and `None` is treated as "omitted / no change").  The
/// agreed sentinel for "clear the relay" is an **empty string** (`""`):
///
/// - **Caller (CLI / UI / ipc.rs):** send `set_config { relay_url: "" }`.
/// - **`update_core_config` (ipc.rs):** detects `Some("")` and sets
///   `core.relay_url = None` instead of `Some("")`; then saves the config.
///   *Until ipc.rs is updated this clearing step is SKIPPED — see note below.*
/// - **`set_config` handler (ipc.rs):** after writing config, checks
///   `relay_url_is_clear(incoming.relay_url.as_deref())` and, if true, drops
///   the live `RelayHandle` (which triggers shutdown via `Drop`).
/// - **`start_relay`:** returns `Err(RelayError::Disabled)` for an empty-string
///   URL so the caller never starts new relay loops for the cleared sentinel.
///
/// ## Current ipc.rs gap (what ipc.rs MUST do — but cannot be changed here)
///
/// `update_core_config` at ipc.rs:466 must be updated:
/// ```text
/// // Before (does not handle clear):
/// if let Some(ref v) = incoming.relay_url {
///     core.relay_url = Some(v.clone());
/// }
/// // After (treats "" as "clear"):
/// match incoming.relay_url.as_deref() {
///     Some("") => core.relay_url = None,          // sentinel → clear
///     Some(v)  => core.relay_url = Some(v.to_owned()), // normal set
///     None     => {}                               // omitted → no change
/// }
/// ```
/// And `merge_config` at ipc.rs:519 must be updated:
/// ```text
/// // Before:
/// relay_url: incoming.relay_url.or(existing.relay_url),
/// // After:
/// relay_url: if incoming.relay_url.as_deref() == Some("") {
///     None                                        // sentinel → clear
/// } else {
///     incoming.relay_url.or(existing.relay_url)   // normal merge
/// },
/// ```
/// And the `set_config` handler must drop the running `RelayHandle` when this
/// function returns `true` for the incoming `relay_url`.
pub fn relay_url_is_clear(url: Option<&str>) -> bool {
    url.is_none_or(|s| s.trim().is_empty())
}

// ── Utility ───────────────────────────────────────────────────────────────────

pub(super) fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ── Public entry point ────────────────────────────────────────────────────────

/// Start the relay orchestrator: a push loop (subscribes `new_item_rx`) and a
/// receive loop (polls the shared inbox). Active iff `relay_url` is a valid URL.
///
/// `device_name` is the human-readable name presented at registration (1..=64).
///
/// `device_id` is the daemon's own stable device UUID (from
/// `load_or_create_device_id`). It is bound into the relay token file's AEAD AAD
/// so a token written by device A cannot authenticate on device B even under a
/// shared `local_key` (CopyPaste-qvtg.4).
///
/// `auto_apply_change_count` — when `Some`, enables the Universal Clipboard
/// feature on the relay receive path: a freshly-synced text item is written to
/// NSPasteboard immediately after ingest, honoring the `auto_apply_synced_clip`
/// config flag.  The `Arc<AtomicI64>` is the SAME self-write sentinel the
/// `ClipboardMonitor` uses so the pasteboard write is not re-captured as a new
/// local item (loop prevention).  Pass the same `self_write_change_count_arc`
/// that the IPC server and sync_orch already share.  Pass `None` to disable
/// (non-macOS, tests, or callers that have not wired the sentinel).
// All params are distinct daemon-lifecycle handles (client, url, name, device_id,
// db, rx, sync_key, local_key, last_sync_ms, core_config, auto_apply_change_count)
// — no struct without reaching into daemon internals.
#[allow(clippy::too_many_arguments)]
pub fn start_relay(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    device_id: String,
    db: Arc<Mutex<Database>>,
    new_item_rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
    auto_apply_change_count: Option<Arc<AtomicI64>>,
    // CopyPaste-1jms.22: shared in-flight flag for SyncBadgeState::Syncing.
    sync_in_flight: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<RelayHandle, RelayError> {
    // Empty / whitespace-only URL is the sentinel for "relay disabled / cleared".
    // Return Disabled (not InvalidUrl) so the caller can distinguish a deliberate
    // clear from a malformed URL and act accordingly (e.g. stop relay fan-out).
    if relay_url_is_clear(Some(relay_url.as_str())) {
        tracing::info!("relay-sync: relay_url cleared (empty sentinel) — relay disabled");
        return Err(RelayError::Disabled);
    }
    let relay_url = relay_url.trim_end_matches('/').to_owned();
    if !registration::is_relay_url_ok(&relay_url) {
        return Err(RelayError::InvalidUrl);
    }
    let shutdown = Arc::new(Notify::new());

    // Truncate the device name to the relay's 1..=64 contract defensively.
    let device_name = {
        let t = device_name.trim();
        let t = if t.is_empty() { "copypaste" } else { t };
        t.chars().take(64).collect::<String>()
    };

    tokio::spawn(push::push_loop(
        client.clone(),
        relay_url.clone(),
        device_name.clone(),
        device_id.clone(),
        new_item_rx,
        shutdown.clone(),
        sync_key.clone(),
        local_key.clone(),
        last_sync_ms.clone(),
        core_config.clone(),
        sync_in_flight.clone(),
    ));
    tokio::spawn(receive::receive_loop(
        client,
        relay_url,
        device_name,
        device_id,
        shutdown.clone(),
        db,
        sync_key,
        local_key,
        last_sync_ms,
        core_config,
        auto_apply_change_count,
        sync_in_flight,
    ));

    tracing::info!("relay-sync: orchestrator started");
    Ok(RelayHandle { shutdown })
}

// ── Tests ─────────────────────────────────────────────────────────────────────
//
// CopyPaste-vp63.25: this module used to hold ~1485 lines of tests covering
// every submodule. They have been relocated to the `#[cfg(test)] mod tests`
// of the submodule they exercise (see each file for its share); only the two
// tests that cover mod.rs's OWN production code (the sentinel + start_relay's
// early-return) remain here. Shared test builders live in `relay/testutil.rs`.

#[cfg(test)]
mod tests {
    use super::*;

    // ── relay_url_is_clear sentinel tests ────────────────────────────────────

    /// `relay_url_is_clear` returns true for None, empty string, and
    /// whitespace-only strings (all mean "relay disabled / not configured").
    #[test]
    fn relay_url_is_clear_detects_disabled_sentinel() {
        assert!(relay_url_is_clear(None), "None → cleared");
        assert!(
            relay_url_is_clear(Some("")),
            "empty string → cleared (the clear sentinel)"
        );
        assert!(relay_url_is_clear(Some("   ")), "whitespace-only → cleared");
        assert!(
            !relay_url_is_clear(Some("https://relay.example.com")),
            "valid URL → NOT cleared"
        );
        assert!(
            !relay_url_is_clear(Some("http://127.0.0.1:8080")),
            "loopback URL → NOT cleared"
        );
    }

    /// `start_relay` returns `Err(RelayError::Disabled)` for an empty relay_url
    /// sentinel so the caller can distinguish a deliberate clear from an invalid URL.
    #[test]
    fn start_relay_empty_url_returns_disabled() {
        use copypaste_core::{AppConfig, Database};
        use std::sync::{Arc, RwLock};
        use tokio::sync::Mutex;

        // Minimal stubs — start_relay never reaches network code for the sentinel.
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("db")));
        let (tx, rx) = tokio::sync::broadcast::channel(1);
        drop(tx); // channel is open; rx is enough for the signature
        let sync_key: Arc<tokio::sync::Mutex<Option<copypaste_core::SyncKey>>> =
            Arc::new(tokio::sync::Mutex::new(None));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let last_sync = Arc::new(AtomicI64::new(0));
        let core_config = Arc::new(RwLock::new(AppConfig::default()));

        let client = reqwest::Client::new();

        // Empty string sentinel → Disabled, not InvalidUrl.
        let result = crate::relay::start_relay(
            client.clone(),
            "".to_owned(),
            "test-device".to_owned(),
            "device-test-uuid-a".to_owned(),
            db.clone(),
            rx,
            sync_key.clone(),
            local_key.clone(),
            last_sync.clone(),
            core_config.clone(),
            None,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(
            matches!(result, Err(RelayError::Disabled)),
            "empty relay_url must yield Err(RelayError::Disabled)"
        );

        // Whitespace-only is also the sentinel.
        let (_, rx2) = tokio::sync::broadcast::channel(1);
        let result2 = crate::relay::start_relay(
            client,
            "   ".to_owned(),
            "test-device".to_owned(),
            "device-test-uuid-b".to_owned(),
            db,
            rx2,
            sync_key,
            local_key,
            last_sync,
            core_config,
            None,
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
        );
        assert!(
            matches!(result2, Err(RelayError::Disabled)),
            "whitespace relay_url must yield Err(RelayError::Disabled)"
        );
    }
}
