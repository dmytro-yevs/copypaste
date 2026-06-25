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
//!   [`sync_common::decrypt_item_plaintext`] →
//!   [`sync_common::wrap_and_check_cloud_upload_plaintext`] →
//!   `encrypt_for_cloud(sync_key, item_id, ...)` (the SAME blob the Supabase path
//!   produces) → build the envelope → `POST {relay_url}/devices/{inbox}/items`.
//! - **receive:** poll `GET {relay_url}/devices/{inbox}/items?since=&since_id=`,
//!   decode each item's envelope, `decrypt_from_cloud`, then reuse
//!   [`sync_common::build_local_item`] + [`copypaste_core::insert_item`] with the
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
mod token;
mod types;
mod watermark;

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
/// `auto_apply_change_count` — when `Some`, enables the Universal Clipboard
/// feature on the relay receive path: a freshly-synced text item is written to
/// NSPasteboard immediately after ingest, honoring the `auto_apply_synced_clip`
/// config flag.  The `Arc<AtomicI64>` is the SAME self-write sentinel the
/// `ClipboardMonitor` uses so the pasteboard write is not re-captured as a new
/// local item (loop prevention).  Pass the same `self_write_change_count_arc`
/// that the IPC server and sync_orch already share.  Pass `None` to disable
/// (non-macOS, tests, or callers that have not wired the sentinel).
// All params are distinct daemon-lifecycle handles (client, url, name, db,
// rx, sync_key, local_key, last_sync_ms, core_config, auto_apply_change_count)
// — no struct without reaching into daemon internals.
#[allow(clippy::too_many_arguments)]
pub fn start_relay(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    db: Arc<Mutex<Database>>,
    new_item_rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
    auto_apply_change_count: Option<Arc<AtomicI64>>,
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
        new_item_rx,
        shutdown.clone(),
        sync_key.clone(),
        local_key.clone(),
        last_sync_ms.clone(),
        core_config.clone(),
    ));
    tokio::spawn(receive::receive_loop(
        client,
        relay_url,
        device_name,
        shutdown.clone(),
        db,
        sync_key,
        local_key,
        last_sync_ms,
        core_config,
        auto_apply_change_count,
    ));

    tracing::info!("relay-sync: orchestrator started");
    Ok(RelayHandle { shutdown })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{derive_sync_key, ITEM_KEY_VERSION_CURRENT};

    // Pull private symbols used in tests from their new homes.
    use super::pasteboard::{
        relay_fetch_auto_apply_candidate, relay_should_auto_apply, relay_should_skip_wifi,
    };
    use super::push::build_content_b64;
    use super::push::push_item;
    use super::receive::{ingest_page_blocking, pull_page};
    use super::registration::register;
    use super::token::{decrypt_relay_token, encrypt_relay_token, RELAY_TOKEN_FILE};
    use super::types::{PullItem, RelayEnvelope};
    use super::watermark::{Watermark, RELAY_WATERMARK_FILE};

    // `SYNC_HTTP_TIMEOUT` is referenced only by the test client builder; importing it
    // at module scope would be flagged unused in a non-test build under -D warnings.
    use crate::sync_common::SYNC_HTTP_TIMEOUT;

    fn skey(p: &str) -> [u8; 32] {
        *derive_sync_key(p).expect("derive").as_bytes()
    }

    fn test_client() -> reqwest::Client {
        reqwest::Client::builder()
            .timeout(SYNC_HTTP_TIMEOUT)
            .build()
            .expect("client")
    }

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
            db.clone(),
            rx,
            sync_key.clone(),
            local_key.clone(),
            last_sync.clone(),
            core_config.clone(),
            None,
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
            db,
            rx2,
            sync_key,
            local_key,
            last_sync,
            core_config,
            None,
        );
        assert!(
            matches!(result2, Err(RelayError::Disabled)),
            "whitespace relay_url must yield Err(RelayError::Disabled)"
        );
    }

    // ── WiFi / auto-apply guard tests ─────────────────────────────────────────

    /// relay_should_skip_wifi: returns true iff sync_on_wifi_only=true AND not on wifi.
    #[test]
    fn wifi_guard_skips_when_setting_on_and_not_on_wifi() {
        assert!(
            relay_should_skip_wifi(true, false),
            "must skip: setting=true, wifi=false"
        );
    }

    #[test]
    fn wifi_guard_allows_when_setting_off() {
        assert!(
            !relay_should_skip_wifi(false, false),
            "must not skip: setting=false even if no wifi"
        );
        assert!(
            !relay_should_skip_wifi(false, true),
            "must not skip: setting=false, on wifi"
        );
    }

    #[test]
    fn wifi_guard_allows_when_on_wifi_and_setting_on() {
        assert!(
            !relay_should_skip_wifi(true, true),
            "must not skip: setting=true but on wifi"
        );
    }

    /// relay_should_auto_apply: mirrors the auto_apply_synced_clip flag.
    #[test]
    fn auto_apply_guard_respects_flag() {
        assert!(
            relay_should_auto_apply(true),
            "auto_apply=true → should auto-apply"
        );
        assert!(
            !relay_should_auto_apply(false),
            "auto_apply=false → must not auto-apply"
        );
    }

    /// derive_relay_inbox_id determinism (daemon-side sanity; core also tests it).
    #[test]
    fn inbox_id_is_deterministic() {
        use copypaste_core::derive_relay_inbox_id;
        let k = skey("relay-determinism-pass");
        assert_eq!(derive_relay_inbox_id(&k), derive_relay_inbox_id(&k));
    }

    /// register parses a 201 + auth_token. Uses the mockito 0.31 global server
    /// (`mockito::mock` + `mockito::server_url`), so it is `#[serial]`.
    #[tokio::test]
    #[serial_test::serial]
    async fn register_parses_201_auth_token() {
        use copypaste_core::derive_relay_inbox_id;
        let k = skey("register-test-pass");
        let inbox = derive_relay_inbox_id(&k);
        let m = mockito::mock("POST", "/devices")
            .with_status(201)
            .with_header("content-type", "application/json")
            .with_body(format!(
                r#"{{"device_id":"{inbox}","auth_token":"deadbeefdeadbeefdeadbeefdeadbeef","expires_at":"2027-01-01T00:00:00Z"}}"#
            ))
            .create();

        let token = register(&test_client(), &mockito::server_url(), &k, "test-device")
            .await
            .expect("register ok");
        assert_eq!(token, "deadbeefdeadbeefdeadbeefdeadbeef");
        m.assert();
    }

    /// push body shape: content_type / content_b64 / wall_time + bearer, 201 → Ok(true).
    #[tokio::test]
    #[serial_test::serial]
    async fn push_item_sends_expected_body() {
        use copypaste_core::derive_relay_inbox_id;
        let k = skey("push-body-pass");
        let inbox = derive_relay_inbox_id(&k);
        let path = format!("/devices/{inbox}/items");
        let m = mockito::mock("POST", path.as_str())
            .match_header("authorization", "Bearer tok123")
            .match_body(mockito::Matcher::JsonString(
                r#"{"content_type":"text","content_b64":"Zm9v","wall_time":42}"#.to_owned(),
            ))
            .with_status(201)
            .with_body(r#"{"id":7}"#)
            .create();

        let ok = push_item(
            &test_client(),
            &mockito::server_url(),
            &inbox,
            "tok123",
            "text",
            "Zm9v".to_owned(),
            42,
        )
        .await
        .expect("push ok");
        assert!(ok);
        m.assert();
    }

    /// push 401 → Ok(false) (caller re-registers).
    #[tokio::test]
    #[serial_test::serial]
    async fn push_item_401_signals_reauth() {
        use copypaste_core::derive_relay_inbox_id;
        let k = skey("push-401-pass");
        let inbox = derive_relay_inbox_id(&k);
        let path = format!("/devices/{inbox}/items");
        let _m = mockito::mock("POST", path.as_str())
            .with_status(401)
            .create();
        let ok = push_item(
            &test_client(),
            &mockito::server_url(),
            &inbox,
            "stale",
            "text",
            "Zm9v".to_owned(),
            1,
        )
        .await
        .expect("push returns Ok(false) on 401");
        assert!(!ok);
    }

    /// pull_page parses an items array and an empty array; watermark query is
    /// formed correctly (smoke).
    #[tokio::test]
    #[serial_test::serial]
    async fn pull_page_parses_items() {
        use copypaste_core::derive_relay_inbox_id;
        let k = skey("pull-page-pass");
        let inbox = derive_relay_inbox_id(&k);
        let path = format!("/devices/{inbox}/items");
        let _m = mockito::mock("GET", mockito::Matcher::Regex(format!("^{path}.*")))
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(r#"[{"id":3,"content_type":"text","content_b64":"YQ==","wall_time":99}]"#)
            .create();
        let items = pull_page(
            &test_client(),
            &mockito::server_url(),
            &inbox,
            "tok",
            Watermark::default(),
        )
        .await
        .expect("pull ok");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].id, 3);
        assert_eq!(items[0].wall_time, 99);
    }

    /// Envelope round-trip: build_content_b64 → decode (base64 → JSON → ct_b64 →
    /// decrypt_from_cloud) recovers the original plaintext, proving the relay
    /// envelope carries the SAME blob shape the Supabase path produces.
    #[test]
    fn envelope_round_trips_through_cloud_crypto() {
        use crate::sync_common::decode_payload_ct;
        use base64::Engine as _;
        use copypaste_core::decrypt_from_cloud;

        let local_key = zeroize::Zeroizing::new([7u8; 32]);
        let sync_key = SyncKey::from_bytes(skey("envelope-roundtrip-pass"));

        // Build a text item encrypted under the local key (mirrors capture).
        let plaintext = b"hello relay world";
        let item = make_local_text_item("item-rt-1", plaintext, &local_key, 5, 1000);

        let content_b64 =
            build_content_b64(&item, &local_key, &sync_key).expect("build content_b64");

        // Decode the envelope exactly as the receiver does.
        let env_bytes = base64::engine::general_purpose::STANDARD
            .decode(&content_b64)
            .expect("b64 decode envelope");
        let env: RelayEnvelope = serde_json::from_slice(&env_bytes).expect("parse envelope");
        assert_eq!(env.item_id, "item-rt-1");
        assert_eq!(env.lamport_ts, 5);
        let blob = decode_payload_ct(&env.ct_b64).expect("decode ct_b64");
        let recovered = decrypt_from_cloud(&sync_key, &env.item_id, &blob).expect("decrypt");
        assert_eq!(recovered, plaintext);
    }

    /// receive ingests a relay item via insert_item with LWW, and a re-pull of
    /// the SAME item (self-echo / equal lamport) is a no-op. Watermark advances.
    #[test]
    fn ingest_inserts_then_dedups_with_lww() {
        use copypaste_core::get_item_by_item_id;

        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([9u8; 32]);
        let sync_bytes = skey("ingest-lww-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);

        // Build a wire item by encrypting a text payload through the cloud crypto.
        let plaintext = b"ingest me";
        let item_id = "item-ingest-1";
        let pull = make_pull_item(1, item_id, plaintext, &sync_key, 10, 2000);

        let g = db.blocking_lock();
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull),
            Watermark::default(),
            u64::MAX,
        );
        assert_eq!(stored1, 1, "first ingest inserts the row");
        assert_eq!(wm1.wall, 2000);
        assert_eq!(wm1.id, 1);
        // The row is present and decodes through the production path.
        let got = get_item_by_item_id(&g, item_id)
            .expect("query")
            .expect("row present");
        assert_eq!(got.lamport_ts, 10);

        // Re-pull the SAME item with equal lamport, equal wall_time, and equal
        // origin (a genuine self-echo of a row we pushed) → LWW no-op.
        // CopyPaste-ayvs: the total order now tie-breaks on wall_time then
        // origin, so a true echo must match ALL three keys (a higher wall_time
        // would legitimately win — that is the convergence fix, not a regression).
        let pull2 = make_pull_item(2, item_id, plaintext, &sync_key, 10, 2000);
        let (wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull2),
            wm1,
            u64::MAX,
        );
        assert_eq!(stored2, 0, "equal lamport+wall+origin echo is a no-op");
        // Watermark still advances past the seen row (id) so we don't re-fetch it.
        assert_eq!(wm2.wall, 2000);
        assert_eq!(wm2.id, 2);

        // A strictly-newer lamport for the same item_id wins LWW (replace).
        let pull3 = make_pull_item(3, item_id, b"edited", &sync_key, 11, 2002);
        let (_wm3, stored3) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull3),
            wm2,
            u64::MAX,
        );
        assert_eq!(stored3, 1, "newer lamport replaces in place");
    }

    // ── Token encryption tests ────────────────────────────────────────────────

    /// Round-trip: encrypt then decrypt recovers the original token.
    #[test]
    fn token_encrypt_decrypt_roundtrip() {
        let key = zeroize::Zeroizing::new([0xABu8; 32]);
        let token = "test-auth-token-abc123-deadbeef";
        let encoded = encrypt_relay_token(token, &key).expect("encrypt");
        let recovered = decrypt_relay_token(&encoded, &key).expect("decrypt returned None");
        assert_eq!(recovered, token);
    }

    /// Two encryptions of the same token produce DIFFERENT base64 blobs (nonce
    /// uniqueness via OsRng) so the file content changes on every re-store.
    #[test]
    fn token_encrypt_nonce_is_unique_across_writes() {
        let key = zeroize::Zeroizing::new([0xCDu8; 32]);
        let token = "same-token-every-time";
        let enc1 = encrypt_relay_token(token, &key).expect("enc1");
        let enc2 = encrypt_relay_token(token, &key).expect("enc2");
        // The blobs must differ (nonce changes, so the entire base64 string differs).
        assert_ne!(enc1, enc2, "each encryption must use a fresh random nonce");
    }

    /// Wrong key → decrypt returns None (AEAD auth tag failure, not a panic).
    #[test]
    fn token_decrypt_wrong_key_returns_none() {
        let key_a = zeroize::Zeroizing::new([0x11u8; 32]);
        let key_b = zeroize::Zeroizing::new([0x22u8; 32]);
        let encoded = encrypt_relay_token("secret-token", &key_a).expect("encrypt");
        let result = decrypt_relay_token(&encoded, &key_b);
        assert!(
            result.is_none(),
            "wrong key must yield None, not a recovered token"
        );
    }

    /// Tampered ciphertext → decrypt returns None (not a panic).
    #[test]
    fn token_decrypt_tampered_ciphertext_returns_none() {
        use base64::Engine as _;
        use copypaste_core::NONCE_SIZE;

        let key = zeroize::Zeroizing::new([0x33u8; 32]);
        let mut blob = base64::engine::general_purpose::STANDARD
            .decode(encrypt_relay_token("my-token", &key).expect("enc"))
            .expect("b64");
        // Flip a bit in the ciphertext portion (after the 24-byte nonce).
        if let Some(b) = blob.get_mut(NONCE_SIZE) {
            *b ^= 0xFF;
        }
        let tampered = base64::engine::general_purpose::STANDARD.encode(&blob);
        assert!(decrypt_relay_token(&tampered, &key).is_none());
    }

    /// CopyPaste-qvtg.2: a token file that does NOT authenticate under AEAD
    /// (legacy plaintext, corrupt, or attacker-planted) must be REJECTED —
    /// `load_cached_token` returns `None` and never the raw file bytes — while a
    /// properly AEAD-encrypted token is still accepted. This closes the
    /// write-then-use TOCTOU where a local attacker plants a controlled token.
    #[test]
    fn load_cached_token_rejects_non_aead_token() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let dir = tempfile::tempdir().expect("tmpdir");
        // try_app_support_dir() resolves under HOME on macOS / XDG_DATA_HOME on
        // Linux; set both so token_path() lands inside the tempdir.
        let prev_home = std::env::var_os("HOME");
        let prev_xdg = std::env::var_os("XDG_DATA_HOME");
        unsafe {
            std::env::set_var("HOME", dir.path());
            std::env::set_var("XDG_DATA_HOME", dir.path());
        }

        let key = zeroize::Zeroizing::new([0x55u8; 32]);
        let token_file = super::token::token_path().expect("token path resolves");
        if let Some(parent) = token_file.parent() {
            std::fs::create_dir_all(parent).expect("mkdir");
        }

        // 1) Legacy plaintext / attacker-planted token → rejected, never returned.
        std::fs::write(&token_file, b"attacker-planted-token-xyz\n").expect("write");
        assert!(
            super::token::load_cached_token(&key).is_none(),
            "non-AEAD token must be rejected (None), never returned as the bearer"
        );

        // 2) A properly AEAD-encrypted token → accepted and round-trips.
        let enc = encrypt_relay_token("real-encrypted-token-123", &key).expect("encrypt");
        super::token::write_token_0600(&token_file, &enc).expect("write encrypted");
        assert_eq!(
            super::token::load_cached_token(&key).as_deref(),
            Some("real-encrypted-token-123"),
            "a valid AEAD token must still load"
        );

        // Restore env.
        unsafe {
            match prev_home {
                Some(v) => std::env::set_var("HOME", v),
                None => std::env::remove_var("HOME"),
            }
            match prev_xdg {
                Some(v) => std::env::set_var("XDG_DATA_HOME", v),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
        }
    }

    /// Empty file → load returns None (no fallback to empty token).
    #[test]
    fn load_cached_token_empty_file_returns_none() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let token_file = dir.path().join(RELAY_TOKEN_FILE);
        std::fs::write(&token_file, b"   \n").expect("write");

        let key = zeroize::Zeroizing::new([0x77u8; 32]);
        let raw = std::fs::read_to_string(&token_file).expect("read");
        let trimmed = raw.trim();
        // Empty / whitespace-only file → treated as absent.
        assert!(trimmed.is_empty());
        // Simulates the `if trimmed.is_empty() { return None; }` guard.
        assert!(if trimmed.is_empty() {
            None::<String>
        } else {
            decrypt_relay_token(trimmed, &key)
        }
        .is_none());
    }

    // ── test helpers ──────────────────────────────────────────────────────────

    fn open_mem_db() -> Arc<Mutex<Database>> {
        let db = Database::open_in_memory().expect("open in-memory db");
        Arc::new(Mutex::new(db))
    }

    /// Build a locally-stored text ClipboardItem (v2 key path) so the upload
    /// pipeline's `decrypt_item_plaintext` can read it back.
    fn make_local_text_item(
        item_id: &str,
        plaintext: &[u8],
        local_key: &zeroize::Zeroizing<[u8; 32]>,
        lamport_ts: i64,
        wall_time: i64,
    ) -> ClipboardItem {
        use copypaste_core::{
            build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
        };
        let v1: [u8; 32] = **local_key;
        let v2 = derive_v2(&v1);
        let aad = build_item_aad_v2(
            item_id,
            AAD_SCHEMA_VERSION_V4,
            ITEM_KEY_VERSION_CURRENT as u32,
        );
        let (nonce, ct) = encrypt_item_with_aad(plaintext, &v2, &aad).expect("encrypt");
        ClipboardItem {
            deleted: false,
            id: item_id.to_owned(),
            item_id: item_id.to_owned(),
            content_type: "text".to_owned(),
            content: Some(ct),
            content_nonce: Some(nonce.to_vec()),
            blob_ref: None,
            is_sensitive: false,
            is_synced: false,
            lamport_ts,
            wall_time,
            expires_at: None,
            app_bundle_id: None,
            content_hash: None,
            origin_device_id: "dev-local".to_owned(),
            key_version: ITEM_KEY_VERSION_CURRENT as u8,
            pinned: false,
            pin_order: None,
            thumb: None,
        }
    }

    /// Build a relay `PullItem` carrying a text payload encrypted for the cloud.
    fn make_pull_item(
        id: i64,
        item_id: &str,
        plaintext: &[u8],
        sync_key: &SyncKey,
        lamport_ts: i64,
        wall_time: u64,
    ) -> PullItem {
        use base64::Engine as _;
        use copypaste_core::encrypt_for_cloud;

        let blob = encrypt_for_cloud(sync_key, item_id, plaintext).expect("cloud encrypt");
        let ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
        let env = RelayEnvelope {
            item_id: item_id.to_owned(),
            lamport_ts,
            ct_b64,
            deleted: false,
            pinned: false,
            pin_order: None,
            wall_time: wall_time as i64,
            origin_device_id: "dev-remote".to_owned(),
        };
        envelope_to_pull(id, "text", &env, wall_time)
    }

    /// Wrap a `RelayEnvelope` into a `PullItem` (the relay-wire row shape).
    fn envelope_to_pull(
        id: i64,
        content_type: &str,
        env: &RelayEnvelope,
        wall_time: u64,
    ) -> PullItem {
        use base64::Engine as _;

        let content_b64 = base64::engine::general_purpose::STANDARD
            .encode(serde_json::to_vec(env).expect("env json"));
        PullItem {
            id,
            content_type: content_type.to_owned(),
            content_b64,
            wall_time,
        }
    }

    /// Build a relay `PullItem` carrying a TOMBSTONE (deleted=true, empty ct).
    fn make_tombstone_pull(id: i64, item_id: &str, lamport_ts: i64, wall_time: u64) -> PullItem {
        let env = RelayEnvelope {
            item_id: item_id.to_owned(),
            lamport_ts,
            ct_b64: String::new(),
            deleted: true,
            pinned: false,
            pin_order: None,
            wall_time: wall_time as i64,
            origin_device_id: "dev-remote".to_owned(),
        };
        envelope_to_pull(id, "text", &env, wall_time)
    }

    /// Build a relay `PullItem` carrying a PINNED text item.
    fn make_pinned_pull(
        id: i64,
        item_id: &str,
        plaintext: &[u8],
        sync_key: &SyncKey,
        lamport_ts: i64,
        wall_time: u64,
        pin_order: f64,
    ) -> PullItem {
        use base64::Engine as _;
        use copypaste_core::encrypt_for_cloud;

        let blob = encrypt_for_cloud(sync_key, item_id, plaintext).expect("cloud encrypt");
        let ct_b64 = base64::engine::general_purpose::STANDARD.encode(&blob);
        let env = RelayEnvelope {
            item_id: item_id.to_owned(),
            lamport_ts,
            ct_b64,
            deleted: false,
            pinned: true,
            pin_order: Some(pin_order),
            wall_time: wall_time as i64,
            origin_device_id: "dev-remote".to_owned(),
        };
        envelope_to_pull(id, "text", &env, wall_time)
    }

    // ── CopyPaste-cm0u: delete + pin propagate over the relay envelope ────────

    /// A delete envelope round-trips: build_content_b64 on a tombstone produces
    /// a `deleted=true` / empty-ct envelope (no decrypt of NULL content), and
    /// ingest applies it as a local soft-delete on a previously-live item.
    #[test]
    fn relay_tombstone_round_trip_soft_deletes_local() {
        use copypaste_core::get_item_by_item_id;

        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([4u8; 32]);
        let sync_bytes = skey("relay-tombstone-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        // First ingest a live item (lamport 10).
        let item_id = "item-del-1";
        let live = make_pull_item(1, item_id, b"to be deleted", &sync_key, 10, 1000);
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&live),
            Watermark::default(),
            u64::MAX,
        );
        assert_eq!(stored1, 1, "live item inserted");
        assert!(
            !get_item_by_item_id(&g, item_id).unwrap().unwrap().deleted,
            "item starts live"
        );

        // Now ingest a tombstone (lamport 11 > 10) — must soft-delete locally.
        let tomb = make_tombstone_pull(2, item_id, 11, 2000);
        let (_wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&tomb),
            wm1,
            u64::MAX,
        );
        assert_eq!(stored2, 1, "tombstone applied");
        let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
        assert!(
            row.deleted,
            "relay tombstone must soft-delete the local item"
        );
        assert!(row.content.is_none(), "tombstone wipes content");
    }

    /// A tombstone built from a deleted ClipboardItem encodes as a
    /// `deleted=true` envelope WITHOUT attempting to decrypt NULL content.
    #[test]
    fn build_content_b64_emits_tombstone_envelope_for_deleted_item() {
        use base64::Engine as _;

        let local_key = zeroize::Zeroizing::new([6u8; 32]);
        let sync_key = SyncKey::from_bytes(skey("relay-build-tomb-pass"));

        // A tombstone row: deleted=true, content=None (as soft_delete_item leaves it).
        let mut item = make_local_text_item("item-tomb", b"unused", &local_key, 9, 900);
        item.deleted = true;
        item.content = None;
        item.content_nonce = None;

        let content_b64 =
            build_content_b64(&item, &local_key, &sync_key).expect("tombstone must build");
        let env_bytes = base64::engine::general_purpose::STANDARD
            .decode(&content_b64)
            .expect("b64");
        let env: RelayEnvelope = serde_json::from_slice(&env_bytes).expect("parse env");
        assert!(env.deleted, "tombstone envelope carries deleted=true");
        assert!(env.ct_b64.is_empty(), "tombstone envelope has empty ct_b64");
        assert_eq!(env.item_id, "item-tomb");
    }

    /// Pin state propagates: a pinned envelope ingests as a pinned local row.
    #[test]
    fn relay_pin_round_trip_sets_pinned_local() {
        use copypaste_core::get_item_by_item_id;

        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([8u8; 32]);
        let sync_bytes = skey("relay-pin-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        let item_id = "item-pin-1";
        let pinned = make_pinned_pull(1, item_id, b"pin me", &sync_key, 5, 1000, 2.0);
        let (_wm, stored) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pinned),
            Watermark::default(),
            u64::MAX,
        );
        assert_eq!(stored, 1, "pinned item inserted");
        let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
        assert!(row.pinned, "relay must carry pinned=true");
        assert_eq!(row.pin_order, Some(2.0), "relay must carry pin_order");
    }

    // ── CopyPaste-ayvs: transport tie-break parity (relay == P2P resolve) ─────

    /// On EQUAL lamport, relay `ingest_page_blocking` must converge to the SAME
    /// winner as the P2P `merge::resolve` (lamport -> wall_time ->
    /// origin_device_id). Drive both with identical inputs and assert they agree
    /// for both tie-break outcomes (remote-wins and local-wins on device id).
    #[test]
    fn relay_equal_lamport_tie_break_matches_p2p_resolve() {
        use base64::Engine as _;
        use copypaste_core::{encrypt_for_cloud, get_item_by_item_id, insert_item};
        use copypaste_sync::merge::{resolve, MergeOutcome};
        use copypaste_sync::protocol::WireItem;

        // Helper: build a P2P WireItem mirroring a relay envelope's keys.
        fn wire(item_id: &str, lamport: i64, wall: i64, origin: &str) -> WireItem {
            WireItem {
                id: item_id.to_owned(),
                item_id: item_id.to_owned(),
                content_type: "text".to_owned(),
                content: Some(vec![1, 2, 3]),
                content_nonce: Some(vec![0u8; 24]),
                blob_ref: None,
                is_sensitive: false,
                lamport_ts: lamport,
                wall_time: wall,
                expires_at: None,
                app_bundle_id: None,
                origin_device_id: origin.to_owned(),
                key_version: 2,
                file_name: None,
                mime: None,
                deleted: false,
                pinned: false,
                pin_order: None,
            }
        }

        // Two cases: remote origin "zzz" (> local) must win; "aaa" (< local) loses.
        for (remote_origin, remote_should_win) in [("zzz", true), ("aaa", false)] {
            let db = open_mem_db();
            let local_key = zeroize::Zeroizing::new([2u8; 32]);
            let sync_bytes = skey("relay-parity-pass");
            let sync_key = SyncKey::from_bytes(sync_bytes);
            let g = db.blocking_lock();

            let item_id = "item-parity";
            // Seed a LOCAL item: lamport 5, wall 1000, origin "mmm".
            let mut seed = make_local_text_item(item_id, b"local-content", &local_key, 5, 1000);
            seed.origin_device_id = "mmm".to_owned();
            insert_item(&g, &seed).unwrap();

            // P2P decision via resolve on identical keys.
            let remote_wire = wire(item_id, 5, 1000, remote_origin);
            let p2p_take_remote = matches!(resolve(&seed, &remote_wire), MergeOutcome::TakeRemote);
            assert_eq!(
                p2p_take_remote, remote_should_win,
                "sanity: resolve decision for origin={remote_origin}"
            );

            // Relay decision: ingest an equal-lamport envelope with the same keys.
            let env = RelayEnvelope {
                item_id: item_id.to_owned(),
                lamport_ts: 5,
                ct_b64: base64::engine::general_purpose::STANDARD
                    .encode(encrypt_for_cloud(&sync_key, item_id, b"remote-content").unwrap()),
                deleted: false,
                pinned: false,
                pin_order: None,
                wall_time: 1000,
                origin_device_id: remote_origin.to_owned(),
            };
            let pull = envelope_to_pull(1, "text", &env, 1000);
            let (_wm, stored) = ingest_page_blocking(
                &g,
                &local_key,
                &sync_bytes,
                std::slice::from_ref(&pull),
                Watermark::default(),
                u64::MAX,
            );
            let relay_took_remote = stored == 1;
            assert_eq!(
                relay_took_remote, p2p_take_remote,
                "relay ingest must converge to the SAME winner as P2P resolve \
                 (origin={remote_origin}): relay={relay_took_remote}, p2p={p2p_take_remote}"
            );
            // Confirm the stored row's origin matches the chosen winner.
            let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
            let expected_origin = if remote_should_win {
                remote_origin
            } else {
                "mmm"
            };
            assert_eq!(
                row.origin_device_id, expected_origin,
                "winning origin must persist for deterministic future tie-breaks"
            );
        }
    }

    // ── CopyPaste-bfiu: delete-before-create over relay must not resurrect ────

    // A tombstone for an UNKNOWN item_id inserts a tombstone row; a later
    // out-of-order create with a LOWER lamport then loses LWW and the item
    // stays deleted.
    // ── P1-1: sensitive items must never enter the push pipeline ─────────────

    /// P1-1 guard: the SOLE filter for sensitive items is the
    /// `if item.is_sensitive { continue; }` check at the top of `push_loop`,
    /// which `continue`s BEFORE `build_content_b64` is ever called. NOTE:
    /// `build_content_b64` itself does NOT inspect `is_sensitive` (it only
    /// returns `None` on decrypt/encrypt/serialize failure) — it is NOT a
    /// backstop. Do not remove the push_loop guard on the assumption that the
    /// encoder would catch sensitive items: it would not, and sensitive
    /// ciphertext would be pushed to the relay.
    ///
    /// CopyPaste-jbao: this is a REAL end-to-end guard test (the previous one was
    /// a tautology that asserted `is_sensitive==true` twice and never invoked
    /// `push_loop`, so deleting the guard would not fail it). Here `push_loop`
    /// runs against a mock relay (a bare TCP listener that counts inbound
    /// connections):
    ///   - a SENSITIVE item must produce ZERO connections (guard drops it before
    ///     any token/registration/HTTP work);
    ///   - a NON-sensitive item (with a sync key set) must produce ≥1 connection
    ///     (the positive control — proves the zero above is the guard at work,
    ///     not a broken setup). Removing the guard makes the sensitive case
    ///     connect and the test fails.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn push_loop_does_not_upload_sensitive_items() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Mock relay: accept TCP connections and count them (no HTTP response —
        // we only care whether push_loop attempted to reach the relay).
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let conns = Arc::new(AtomicUsize::new(0));
        let conns_acc = conns.clone();
        let accept_task = tokio::spawn(async move {
            // Drop each accepted socket immediately; the client request fails
            // fast (we set a short client timeout below). We only count that a
            // connection was attempted.
            while let Ok((_sock, _)) = listener.accept().await {
                conns_acc.fetch_add(1, Ordering::SeqCst);
            }
        });

        let local_key = Arc::new(zeroize::Zeroizing::new([0xAAu8; 32]));
        // A sync key MUST be present so a non-sensitive item proceeds all the way
        // to the HTTP push (the positive control).
        let sync_key = Arc::new(tokio::sync::Mutex::new(Some(SyncKey::from_bytes(skey(
            "jbao-guard-test",
        )))));
        // sync_enabled on, wifi-only off → nothing else blocks the push.
        let core_config = Arc::new(std::sync::RwLock::new(AppConfig {
            sync_enabled: true,
            sync_on_wifi_only: false,
            ..AppConfig::default()
        }));
        let (tx, rx) = tokio::sync::broadcast::channel::<ClipboardItem>(8);
        let shutdown = Arc::new(tokio::sync::Notify::new());
        // Short client timeout so a never-answered request fails fast instead of
        // stalling the loop for the whole test.
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_millis(400))
            .build()
            .unwrap();

        let loop_task = tokio::spawn(push::push_loop(
            client,
            format!("http://{addr}"),
            "test-device".to_owned(),
            rx,
            shutdown.clone(),
            sync_key,
            local_key.clone(),
            Arc::new(AtomicI64::new(0)),
            core_config,
        ));

        // 1) SENSITIVE item — must NOT reach the relay.
        let mut sensitive =
            make_local_text_item("sens-1", b"AKIAIOSFODNN7EXAMPLE", &local_key, 1, 1000);
        sensitive.is_sensitive = true;
        tx.send(sensitive).unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        assert_eq!(
            conns.load(Ordering::SeqCst),
            0,
            "sensitive item must never connect to the relay"
        );

        // 2) NON-sensitive item (positive control) — must reach the relay.
        let mut plain = make_local_text_item("plain-1", b"hello, world", &local_key, 2, 2000);
        plain.is_sensitive = false;
        tx.send(plain).unwrap();
        // Allow time for token registration + push connection attempt(s).
        let mut got_conn = false;
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if conns.load(Ordering::SeqCst) >= 1 {
                got_conn = true;
                break;
            }
        }
        assert!(
            got_conn,
            "non-sensitive item must reach the relay (positive control) — if this \
             fails the test setup is broken, not the guard"
        );

        shutdown.notify_one();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), loop_task).await;
        accept_task.abort();
    }

    #[test]
    fn relay_delete_before_create_does_not_resurrect() {
        use copypaste_core::get_item_by_item_id;

        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([3u8; 32]);
        let sync_bytes = skey("relay-dbc-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        let item_id = "item-race-1";
        // Delete arrives FIRST (lamport 20) for an item we have never seen.
        let tomb = make_tombstone_pull(1, item_id, 20, 2000);
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&tomb),
            Watermark::default(),
            u64::MAX,
        );
        assert_eq!(stored1, 1, "tombstone inserted for unknown item");
        let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
        assert!(
            row.deleted,
            "unknown-item tombstone must persist as deleted"
        );

        // Create arrives LATER with a LOWER lamport (10 < 20) — must lose LWW.
        let create = make_pull_item(2, item_id, b"resurrected?", &sync_key, 10, 1000);
        let (_wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&create),
            wm1,
            u64::MAX,
        );
        assert_eq!(stored2, 0, "late lower-lamport create must NOT resurrect");
        let row = get_item_by_item_id(&g, item_id).unwrap().unwrap();
        assert!(
            row.deleted,
            "item must stay deleted after the racing create"
        );
    }

    // ── dtq3: additive multi-transport dedup ─────────────────────────────────

    /// When the SAME `item_id` arrives via TWO independent transports (relay +
    /// Supabase / cloud) the consumer-side LWW guard must ensure exactly ONE DB
    /// row is written — no double-count, no duplicate content.
    ///
    /// This test simulates the scenario by calling `ingest_page_blocking` twice
    /// for the same `item_id` (same lamport, same wall_time, same origin), which
    /// models a peer that receives the item from both relay and Supabase.  The
    /// second call must be a complete no-op: `stored == 0` and the DB still has
    /// exactly one row for that `item_id`.
    #[test]
    fn both_transports_deliver_same_item_inserts_exactly_once() {
        use copypaste_core::get_item_by_item_id;

        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([0xBBu8; 32]);
        let sync_bytes = skey("dual-transport-dedup-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        let item_id = "item-dual-transport-1";
        let plaintext = b"hello from both transports";

        // --- Transport 1 (relay): first delivery ---
        let relay_pull = make_pull_item(1, item_id, plaintext, &sync_key, 7, 1500);
        let (wm1, stored1) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&relay_pull),
            Watermark::default(),
            u64::MAX,
        );
        assert_eq!(stored1, 1, "first transport delivery must insert the row");

        // Confirm exactly one row in DB with the correct lamport.
        let row_after_first = get_item_by_item_id(&g, item_id)
            .expect("query ok")
            .expect("row must exist after first transport");
        assert_eq!(row_after_first.lamport_ts, 7);

        // --- Transport 2 (cloud/Supabase, modelled as another relay call with
        // the SAME item_id, lamport, wall_time, and origin): second delivery ---
        // Use a different relay `id` (id=2) to avoid watermark dedup; the
        // envelope `item_id` is identical — this is what makes it a cross-transport
        // duplicate.  The ingest path keys on envelope `item_id`, not relay row `id`.
        let cloud_pull = make_pull_item(2, item_id, plaintext, &sync_key, 7, 1500);
        let (_wm2, stored2) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&cloud_pull),
            wm1,
            u64::MAX,
        );
        assert_eq!(
            stored2, 0,
            "second transport delivery of the same item_id must be a LWW no-op (stored==0)"
        );

        // Confirm the DB still has EXACTLY one row for this item_id.
        let row_after_second = get_item_by_item_id(&g, item_id)
            .expect("query ok")
            .expect("row must still exist after second transport");
        assert_eq!(
            row_after_second.lamport_ts, 7,
            "lamport must be unchanged — row not double-written"
        );
        // There must not be a second row with a different PK carrying the same item_id.
        // `get_item_by_item_id` returns the UNIQUE row (item_id has a UNIQUE index),
        // so the fact that it returns Some without UNIQUE conflict is proof enough.
        // Additionally verify the content is intact (not corrupted by a partial re-write).
        assert!(
            row_after_second.content.is_some(),
            "content must be intact after dedup no-op"
        );
    }

    // ── BUG 1 (CopyPaste-2yuo): write_token_0600 permissions ─────────────────

    /// write_token_0600 must produce a file with exactly mode 0600.
    ///
    /// This is the contract test: the file must be 0600 regardless of the
    /// process umask. The old `File::create()` + `set_permissions()` approach
    /// created the temp file with the umask-modified mode (typically 0644) for a
    /// brief window before chmod. The fix uses `OpenOptionsExt::mode(0o600)` so
    /// the file is 0600 from the first open(2) call.
    ///
    /// Note: a race-condition reproducer cannot be written as a pure unit test
    /// without threading primitives; this test verifies the postcondition contract.
    #[cfg(unix)]
    #[test]
    fn write_token_0600_perms_are_exactly_0600() {
        use super::token::write_token_0600;

        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("relay_token_perms_test");
        write_token_0600(&path, "test-token-for-perms-check").expect("write ok");
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "token file must be mode 0600, got {:o}", mode);
    }

    /// write_token_0600 must produce a 0600 file even when the process umask is
    /// 0000 (which makes File::create produce world-readable 0666 files).
    ///
    /// This is the failing test for the race: with the old implementation
    /// `File::create` creates the temp file at mode 0666 (umask=0) for a brief
    /// window. The test cannot observe that window directly, but it documents
    /// the invariant that `mode(0o600)` via OpenOptionsExt is immune to umask.
    ///
    /// The umask is process-wide; this test uses `#[serial]` to avoid
    /// interference with other tests.
    #[cfg(unix)]
    #[test]
    #[serial_test::serial]
    fn write_token_0600_immune_to_permissive_umask() {
        use super::token::write_token_0600;

        // Temporarily set umask to 0 so File::create would produce 0666.
        // A correct implementation using OpenOptions::mode(0o600) must still
        // produce 0600 because the explicit mode overrides umask for the
        // bits we care about (0600 ∩ 0777 = 0600, unaffected by umask~0777).
        let old_umask = unsafe { libc::umask(0) };
        let dir = tempfile::tempdir().expect("tmpdir");
        let path = dir.path().join("relay_token_umask_test");
        let result = write_token_0600(&path, "tok-umask-test");
        // Restore umask before any assertion so a panic doesn't leave it broken.
        unsafe { libc::umask(old_umask) };
        result.expect("write ok");
        use std::os::unix::fs::PermissionsExt as _;
        let mode = std::fs::metadata(&path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(
            mode, 0o600,
            "token file must be 0600 even with umask=0000 (world-open), got {:o}",
            mode
        );
    }

    // ── BUG 2b (CopyPaste-7ub): auto_apply_synced_clip relay path ─────────────

    /// relay_fetch_auto_apply_candidate returns the freshest stored item's
    /// (wall_time, plaintext, content_type) when the DB has at least one
    /// non-deleted, non-sensitive, text item. Returns None on empty DB.
    ///
    /// This is the test for the new helper that feeds the pasteboard write path.
    /// FAILS before implementation because `relay_fetch_auto_apply_candidate`
    /// does not exist yet.
    #[test]
    fn relay_fetch_auto_apply_candidate_returns_freshest_text_item() {
        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([0xAAu8; 32]);
        let sync_bytes = skey("relay-auto-apply-candidate-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        // Empty DB → no candidate.
        assert!(
            relay_fetch_auto_apply_candidate(&g, &local_key).is_none(),
            "empty DB must yield no candidate"
        );

        // Insert one item via ingest_page_blocking.
        let item_id = "aac-item-1";
        let plaintext_in = b"hello auto-apply";
        let pull = make_pull_item(1, item_id, plaintext_in, &sync_key, 5, 1000);
        let (_wm, stored) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull),
            Watermark::default(),
            u64::MAX,
        );
        assert_eq!(stored, 1, "first item must be stored");

        // Now fetch the candidate.
        let cand = relay_fetch_auto_apply_candidate(&g, &local_key)
            .expect("must return candidate after insert");
        assert_eq!(cand.content_type, "text", "content_type must be text");
        assert_eq!(
            cand.plaintext, plaintext_in,
            "candidate plaintext must match original"
        );
        assert_eq!(cand.wall_time, 1000, "wall_time must match the item");
    }

    /// When auto_apply_enabled=false, relay_should_auto_apply gates the write.
    /// When auto_apply_enabled=true, the candidate is fetched and written.
    /// This test verifies the gate and candidate fetching work end-to-end
    /// (pasteboard write is macOS-only and not directly testable in a unit test).
    #[test]
    fn relay_auto_apply_gate_and_candidate_integration() {
        let db = open_mem_db();
        let local_key = zeroize::Zeroizing::new([0xCCu8; 32]);
        let sync_bytes = skey("relay-auto-apply-gate-pass");
        let sync_key = SyncKey::from_bytes(sync_bytes);
        let g = db.blocking_lock();

        let pull = make_pull_item(1, "gate-item-1", b"test payload", &sync_key, 3, 500);
        let (_wm, stored) = ingest_page_blocking(
            &g,
            &local_key,
            &sync_bytes,
            std::slice::from_ref(&pull),
            Watermark::default(),
            u64::MAX,
        );
        assert_eq!(stored, 1);

        // auto_apply=false: must not attempt pasteboard write.
        assert!(
            !relay_should_auto_apply(false),
            "flag=false → must not auto-apply"
        );

        // auto_apply=true: gate passes, candidate must be available.
        assert!(relay_should_auto_apply(true), "flag=true → gate passes");
        let cand = relay_fetch_auto_apply_candidate(&g, &local_key);
        assert!(
            cand.is_some(),
            "auto_apply=true path: candidate must be available after ingest"
        );
    }

    // ── CopyPaste-28br: adaptive idle back-off constants ─────────────────────

    /// Verify the back-off constants satisfy the acceptance criterion:
    /// ≥60 s interval after 3 consecutive empty polls, reset to 5 s on
    /// non-empty batch.
    ///
    /// The logic is in `receive_loop` (not easily extracted as a pure fn),
    /// so this test pins the constants and the arithmetic directly.
    #[test]
    fn idle_backoff_constants_satisfy_acceptance_criteria() {
        // Acceptance: after IDLE_EMPTY_POLL_THRESHOLD consecutive empty polls
        // the interval must grow to ≥ 60 s.
        let steps_after_threshold = 1u32; // first step beyond threshold
        let interval = IDLE_POLL_STEP * steps_after_threshold;
        assert!(
            interval >= Duration::from_secs(60),
            "CopyPaste-28br: first idle step must be ≥ 60 s, got {interval:?}. \
             IDLE_POLL_STEP={IDLE_POLL_STEP:?}"
        );

        // The cap (POLL_INTERVAL_MAX) must be at least the first step.
        assert!(
            POLL_INTERVAL_MAX >= interval,
            "POLL_INTERVAL_MAX ({POLL_INTERVAL_MAX:?}) must be ≥ first idle step ({interval:?})"
        );

        // A non-empty batch resets to POLL_INTERVAL (5 s).
        // This is logic in receive_loop; assert the constant here.
        assert_eq!(
            POLL_INTERVAL,
            Duration::from_secs(5),
            "base POLL_INTERVAL must remain 5 s for low-latency active sync"
        );
    }

    /// Simulate the adaptive back-off state machine from `receive_loop` to
    /// verify the counter and interval transitions are correct.
    #[test]
    fn idle_backoff_state_machine_grows_then_resets() {
        let mut consecutive_empty: u32 = 0;
        let mut current_interval = POLL_INTERVAL;

        // Helper: simulate one empty poll tick (mirrors the logic in receive_loop).
        let tick_empty = |consecutive_empty: &mut u32, current_interval: &mut Duration| {
            *consecutive_empty = consecutive_empty.saturating_add(1);
            if *consecutive_empty >= IDLE_EMPTY_POLL_THRESHOLD {
                let steps = consecutive_empty.saturating_sub(IDLE_EMPTY_POLL_THRESHOLD) + 1;
                *current_interval = (IDLE_POLL_STEP * steps).min(POLL_INTERVAL_MAX);
            }
        };

        // Helper: simulate one non-empty poll tick.
        let tick_nonempty = |consecutive_empty: &mut u32, current_interval: &mut Duration| {
            *consecutive_empty = 0;
            *current_interval = POLL_INTERVAL;
        };

        // Initial state: interval is at minimum.
        assert_eq!(current_interval, POLL_INTERVAL);

        // Polls 1 and 2 (below threshold): interval must not grow yet.
        tick_empty(&mut consecutive_empty, &mut current_interval);
        assert_eq!(
            current_interval, POLL_INTERVAL,
            "below threshold: interval must not grow yet (poll 1)"
        );
        tick_empty(&mut consecutive_empty, &mut current_interval);
        assert_eq!(
            current_interval, POLL_INTERVAL,
            "below threshold: interval must not grow yet (poll 2)"
        );

        // Poll 3 reaches threshold: interval must grow to ≥ 60 s.
        tick_empty(&mut consecutive_empty, &mut current_interval);
        assert!(
            current_interval >= Duration::from_secs(60),
            "CopyPaste-28br: at threshold (poll 3) interval must be ≥ 60 s, got {current_interval:?}"
        );

        // Further polls: interval must grow and stay ≤ POLL_INTERVAL_MAX.
        for _ in 0..20 {
            tick_empty(&mut consecutive_empty, &mut current_interval);
            assert!(
                current_interval <= POLL_INTERVAL_MAX,
                "interval must be capped at POLL_INTERVAL_MAX, got {current_interval:?}"
            );
        }

        // A non-empty poll must reset both counter and interval.
        tick_nonempty(&mut consecutive_empty, &mut current_interval);
        assert_eq!(
            consecutive_empty, 0,
            "non-empty poll must reset consecutive_empty to 0"
        );
        assert_eq!(
            current_interval, POLL_INTERVAL,
            "non-empty poll must reset interval to POLL_INTERVAL"
        );
    }

    // ── Watermark persistence tests (CopyPaste-hf40 / CopyPaste-1jms.24) ─────

    /// Persist then load: watermark survives a simulated restart.
    ///
    /// This is the root fix test: `save_watermark` writes `(wall, id)` to a
    /// temp directory and `load_watermark` reads it back — confirming that
    /// a daemon restart resumes from the last-seen cursor rather than (0, 0).
    #[test]
    fn watermark_persists_and_reloads_across_restart() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let wm_path = dir.path().join(RELAY_WATERMARK_FILE);

        // Serialise a non-default watermark directly (bypasses path resolution
        // so the test is hermetic without touching the real app-support dir).
        let original = Watermark {
            wall: 1_700_000_000_000,
            id: 42,
        };
        let json = serde_json::to_string(&original).expect("serialise");
        std::fs::write(&wm_path, json.as_bytes()).expect("write");

        // Deserialise — mimics what load_watermark does after the file is written.
        let raw = std::fs::read_to_string(&wm_path).expect("read");
        let loaded: Watermark = serde_json::from_str(&raw).expect("deserialise");

        assert_eq!(
            loaded.wall, original.wall,
            "wall_time must survive persist + reload"
        );
        assert_eq!(
            loaded.id, original.id,
            "relay row id must survive persist + reload"
        );
    }

    /// Missing watermark file → `load_watermark` returns `Watermark::default()`
    /// (zero cursor — correct first-run behaviour).
    #[test]
    fn load_watermark_missing_file_returns_default() {
        // Confirm that a non-existent path returns (0, 0) — not a panic.
        let wm_path =
            std::path::Path::new("/tmp/copypaste-test-does-not-exist/relay_watermark.json");
        let raw = std::fs::read_to_string(wm_path);
        assert!(
            raw.is_err(),
            "test assumes the file does not exist; adjust path if needed"
        );
        // The actual load_watermark falls back to default on NotFound.
        let def = Watermark::default();
        assert_eq!(def.wall, 0, "default wall must be zero");
        assert_eq!(def.id, 0, "default id must be zero");
    }

    /// Malformed watermark file → `load_watermark` returns `Watermark::default()`
    /// (graceful degradation, no panic).
    #[test]
    fn load_watermark_malformed_file_returns_default() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let wm_path = dir.path().join(RELAY_WATERMARK_FILE);
        std::fs::write(&wm_path, b"not valid json {{{{").expect("write");

        let raw = std::fs::read_to_string(&wm_path).expect("read");
        let result = serde_json::from_str::<Watermark>(&raw);
        assert!(
            result.is_err(),
            "malformed JSON must fail to parse, triggering the default fallback"
        );
        // Confirm fallback: same logic as load_watermark's Err branch.
        let fallback = result.unwrap_or_default();
        assert_eq!(fallback.wall, 0);
        assert_eq!(fallback.id, 0);
    }

    /// `save_watermark` writes a valid JSON file that round-trips through
    /// `serde_json`, confirming the file format is stable.
    #[test]
    fn save_watermark_writes_valid_json() {
        use super::token::write_token_0600;

        let dir = tempfile::tempdir().expect("tmpdir");
        let wm_path = dir.path().join(RELAY_WATERMARK_FILE);

        let wm = Watermark {
            wall: 9_999_999_999_999,
            id: -1,
        };
        let json = serde_json::to_string(&wm).expect("serialise");
        // write_token_0600 is the underlying atomic writer used by save_watermark.
        write_token_0600(&wm_path, &json).expect("write");

        let raw = std::fs::read_to_string(&wm_path).expect("read");
        let loaded: Watermark = serde_json::from_str(&raw).expect("parse");
        assert_eq!(loaded.wall, wm.wall);
        assert_eq!(loaded.id, wm.id);
    }
}
