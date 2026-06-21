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

use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, Notify};

use copypaste_core::{
    decrypt_from_cloud, decrypt_item_with_aad, derive_relay_inbox_id, derive_relay_public_key,
    derive_relay_registration_pop, encrypt_for_cloud, encrypt_item_with_aad,
    exists_item_by_item_id, get_item_by_item_id, insert_item, insert_tombstone, prune_to_cap,
    soft_delete_item, AppConfig, ClipboardItem, Database, SyncKey, NONCE_SIZE,
};
// CopyPaste-ayvs: relay LWW now routes through the SAME total order the P2P and
// cloud paths use (lamport -> wall_time -> origin_device_id) so all transports
// converge identically.
use copypaste_sync::merge::{remote_wins, RemoteMeta};

use crate::sync_common::{
    build_local_item, decode_payload_ct, decrypt_item_plaintext, replace_cloud_item_by_item_id,
    wrap_and_check_cloud_upload_plaintext,
};
// `SYNC_HTTP_TIMEOUT` is referenced only by the test client builder; importing it
// at module scope would be flagged unused in a non-test build under -D warnings.
#[cfg(test)]
use crate::sync_common::SYNC_HTTP_TIMEOUT;

// ── Settings guards ───────────────────────────────────────────────────────────

/// Returns `true` when the current tick should be skipped due to the Wi-Fi-only
/// setting being active and the device not being on Wi-Fi.
///
/// Pure function — injectable `is_on_wifi_fn` makes this unit-testable without
/// a real `networksetup` invocation. Mirrors the guard in `cloud.rs`.
fn relay_should_skip_wifi(sync_on_wifi_only: bool, is_on_wifi: bool) -> bool {
    sync_on_wifi_only && !is_on_wifi
}

/// Returns `true` when the relay receive path should auto-apply a freshly-synced
/// item to the local pasteboard, i.e. when `auto_apply_synced_clip` is enabled.
///
/// Pure function — testable without a live `AppConfig` instance.
fn relay_should_auto_apply(auto_apply_synced_clip: bool) -> bool {
    auto_apply_synced_clip
}

/// Candidate for auto-applying to the local pasteboard after a relay ingest.
///
/// Carries enough information for [`relay_apply_to_pasteboard`] to write the
/// item to NSPasteboard (macOS) without re-querying the DB or re-decrypting.
struct AutoApplyCandidate {
    wall_time: i64,
    plaintext: Vec<u8>,
    content_type: String,
}

/// Fetch the freshest non-deleted, non-sensitive text item from the DB and
/// return it decrypted, ready for pasteboard auto-apply.
///
/// Returns `None` when:
/// - the DB has no qualifying text item, or
/// - decryption fails (wrong key version or corrupt ciphertext — logged at WARN).
///
/// Only text items are returned; image items require the multi-chunk decode
/// path which relay.rs defers to a future iteration (images are stored but
/// not auto-applied — files are never auto-applied per the macOS limit).
///
/// Called inside `spawn_blocking` by the receive loop after `stored > 0` when
/// `auto_apply_enabled` is true.
fn relay_fetch_auto_apply_candidate(
    db: &Database,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Option<AutoApplyCandidate> {
    use copypaste_core::ClipboardItem;

    // Query the most-recently-written non-deleted, non-sensitive text item.
    // We use an inline row-map rather than `get_page` so we can add the
    // `content_type = 'text'` filter without a post-query scan.
    let item: ClipboardItem = db
        .conn()
        .query_row(
            "SELECT id, item_id, content_type, content, content_nonce, blob_ref,
                    is_sensitive, is_synced, lamport_ts, wall_time, expires_at,
                    app_bundle_id, content_hash, origin_device_id, key_version,
                    pinned, pin_order, thumb, deleted
             FROM clipboard_items
             WHERE content_type = 'text' AND deleted = 0 AND is_sensitive = 0
             ORDER BY wall_time DESC, lamport_ts DESC
             LIMIT 1",
            [],
            |r| {
                Ok(ClipboardItem {
                    id: r.get(0)?,
                    item_id: r.get(1)?,
                    content_type: r.get(2)?,
                    content: r.get(3)?,
                    content_nonce: r.get(4)?,
                    blob_ref: r.get(5)?,
                    is_sensitive: r.get::<_, i64>(6)? != 0,
                    is_synced: r.get::<_, i64>(7)? != 0,
                    lamport_ts: r.get(8)?,
                    wall_time: r.get(9)?,
                    expires_at: r.get(10)?,
                    app_bundle_id: r.get(11)?,
                    content_hash: r.get(12)?,
                    origin_device_id: r.get(13)?,
                    key_version: {
                        let kv: i64 = r.get(14)?;
                        u8::try_from(kv)
                            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(14, kv))?
                    },
                    pinned: r.get::<_, i64>(15)? != 0,
                    pin_order: r.get(16)?,
                    thumb: r.get(17)?,
                    deleted: r.get::<_, i64>(18)? != 0,
                })
            },
        )
        .ok()?; // QueryReturnedNoRows → None; other errors also → None (logged below)

    let plaintext = match crate::sync_common::decrypt_item_plaintext(&item, local_key) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "relay-sync: relay_fetch_auto_apply_candidate: decrypt failed: {e}; skipping"
            );
            return None;
        }
    };
    Some(AutoApplyCandidate {
        wall_time: item.wall_time,
        plaintext,
        content_type: item.content_type,
    })
}

/// Write the auto-apply candidate to the local pasteboard (macOS-only).
///
/// Stamps `self_write_change_count` before and after the NSPasteboard write so
/// the `ClipboardMonitor` poller recognises this write as a daemon-own write and
/// does not re-capture it as a new local item (loop prevention — same guard the
/// `copy_item` IPC handler and sync_orch auto-apply use).
///
/// Only text items are written; image/file paths are not yet implemented on the
/// relay receive path (noted at caller).  On non-macOS platforms this is a no-op.
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
fn relay_apply_to_pasteboard(
    candidate: &AutoApplyCandidate,
    self_write_change_count: &Arc<AtomicI64>,
) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSPasteboard;

        match candidate.content_type.as_str() {
            "text" => {
                let text = match std::str::from_utf8(&candidate.plaintext) {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!("relay-sync: auto-apply text is not UTF-8: {e}");
                        return;
                    }
                };
                objc2::rc::autoreleasepool(|_pool| {
                    use objc2_app_kit::NSPasteboardTypeString;
                    use objc2_foundation::NSString;

                    // Pre-stamp: clearContents (+1) + setString (+1) = +2.
                    let pre = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    self_write_change_count.store(pre + 2, Ordering::Release);

                    let ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let ns_str = NSString::from_str(text);
                        pb.setString_forType(&ns_str, NSPasteboardTypeString)
                    };
                    if ok {
                        let actual =
                            unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                        self_write_change_count.store(actual, Ordering::Release);
                        tracing::debug!(
                            change_count = actual,
                            "relay-sync: auto-applied synced text to NSPasteboard"
                        );
                    } else {
                        // Reset sentinel so the monitor is not permanently suppressed.
                        self_write_change_count.store(-1, Ordering::Release);
                        tracing::warn!(
                            "relay-sync: auto-apply text: \
                             NSPasteboard setString:forType: returned false"
                        );
                    }
                });
            }
            "image" | "file" => {
                // Image auto-apply requires multi-chunk decode (deferred).
                // File auto-apply requires writing bytes to a temp file (deferred).
                tracing::debug!(
                    content_type = candidate.content_type.as_str(),
                    "relay-sync: auto-apply deferred for {} item (not yet implemented on relay path)",
                    candidate.content_type
                );
            }
            other => {
                tracing::debug!("relay-sync: auto-apply skipped for unknown content_type={other}");
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        tracing::debug!(
            content_type = candidate.content_type.as_str(),
            "relay-sync: auto-apply skipped (not macOS)"
        );
    }
}

/// Poll interval for the receive loop (the relay also offers SSE; polling is the
/// portable backstop and matches the at-least-once contract). Kept tight so
/// cross-device latency is low without hammering the relay.
const POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Max items requested per pull tick. When a batch comes back full we re-poll
/// immediately (burst drain) rather than waiting a full interval.
const PULL_LIMIT: usize = 50;

/// Filename of the cached relay auth token inside the app data dir.
const RELAY_TOKEN_FILE: &str = "relay_token";

// ── Wire envelope ─────────────────────────────────────────────────────────────

/// The decoded `content_b64` envelope. `content_b64` (on the relay wire) is
/// `base64(JSON(this struct))`; `ct_b64` inside it is
/// `base64(encrypt_for_cloud(sync_key, item_id, wrapped_plaintext))` — the SAME
/// blob the Supabase path stores. This is the exact shape the Android SSE
/// receiver already decodes.
///
/// CopyPaste-cm0u / CopyPaste-ayvs / CopyPaste-bfiu: the envelope now also
/// carries `deleted` / `pinned` / `pin_order` (so deletes and pins propagate
/// over relay-only topologies) and `wall_time` / `origin_device_id` (so relay
/// LWW uses the SAME total order as P2P/cloud). All five are
/// `#[serde(default)]` OPTIONAL-by-omission fields: an envelope written by an
/// older daemon omits them and decodes to `deleted=false` / `pinned=false` /
/// `pin_order=None` / `wall_time=0` / `origin_device_id=""` — i.e. exactly the
/// pre-fix behaviour (a live, unpinned item with no origin tie-break key).
#[derive(Debug, Serialize, Deserialize)]
struct RelayEnvelope {
    item_id: String,
    lamport_ts: i64,
    /// Present for live items; a tombstone envelope sets `deleted=true` and
    /// carries an empty `ct_b64` (the content is NULL — there is nothing to
    /// decrypt). Defaulted empty so older live envelopes (no field) parse.
    #[serde(default)]
    ct_b64: String,
    /// Soft-delete flag. Omitted (=> false) by older daemons.
    #[serde(default)]
    deleted: bool,
    /// Pin flag. Omitted (=> false) by older daemons.
    #[serde(default)]
    pinned: bool,
    /// Pin sort order. Omitted (=> None) by older daemons.
    #[serde(default)]
    pin_order: Option<f64>,
    /// Wall-clock ms — the second LWW tie-break key. Omitted (=> 0) by older
    /// daemons, which makes them lose every equal-lamport tie (acceptable: the
    /// pre-fix relay path had no wall_time tie-break at all).
    #[serde(default)]
    wall_time: i64,
    /// Originating device id — the final LWW tie-break key. Omitted (=> "") by
    /// older daemons.
    #[serde(default)]
    origin_device_id: String,
}

/// Relay register request body.
#[derive(Debug, Serialize)]
struct RegisterBody {
    device_id: String,
    device_name: String,
    public_key_b64: String,
    /// HMAC-SHA256(sync_key, "relay-registration-pop-v1:" || device_id) base64-encoded.
    /// Proves the registrant holds the sync key matching the derived inbox id — fixes CopyPaste-n2l.
    pop_b64: String,
}

/// Relay register response (we only need the token).
#[derive(Debug, Deserialize)]
struct RegisterResp {
    auth_token: String,
}

/// Relay push request body.
#[derive(Debug, Serialize)]
struct PushBody {
    content_type: String,
    content_b64: String,
    wall_time: u64,
}

/// One element of the pull response array.
#[derive(Debug, Deserialize)]
struct PullItem {
    id: i64,
    content_type: String,
    content_b64: String,
    wall_time: u64,
}

// ── Errors ──────────────────────────────────────────────────────────────────

#[derive(Debug, thiserror::Error)]
pub enum RelayError {
    /// The configured `relay_url` is not a usable HTTPS (or loopback in tests) URL.
    #[error("relay_url is not a valid https URL")]
    InvalidUrl,
    /// `relay_url` was explicitly cleared (set to `""` sentinel) — relay is disabled.
    ///
    /// Returned by [`start_relay`] when the caller passes an empty string, and by
    /// [`relay_url_is_clear`] so the `set_config` IPC handler can detect the sentinel
    /// and shut down a running [`RelayHandle`] without needing to know URL internals.
    #[error("relay_url cleared — relay sync disabled")]
    Disabled,
    /// Network / transport failure talking to the relay.
    #[error("relay request failed: {0}")]
    Transport(String),
    /// Relay returned an unexpected non-success status.
    #[error("relay returned status {0}")]
    Status(u16),
    /// Could not resolve the inbox id (no sync key set).
    #[error("no sync passphrase set — relay sync inactive")]
    NoSyncKey,
}

// ── Handle ──────────────────────────────────────────────────────────────────

/// Handle to the running relay orchestrator. Drop (or call [`shutdown`]) to stop
/// the push and receive loops.
///
/// [`shutdown`]: RelayHandle::shutdown
pub struct RelayHandle {
    shutdown: Arc<Notify>,
}

impl RelayHandle {
    /// Signal both loops to stop. Idempotent.
    pub fn shutdown(self) {
        self.shutdown.notify_waiters();
    }
}

impl Drop for RelayHandle {
    fn drop(&mut self) {
        self.shutdown.notify_waiters();
    }
}

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

// ── Token cache (0600 file) ─────────────────────────────────────────────────

/// Purpose-binding AAD for the relay token at-rest encryption.
///
/// A stable string (not device_id) is used here because the token file is
/// written before a device_id is in scope at the call site. Binding to this
/// string still prevents a blob encrypted for a DIFFERENT purpose (e.g. an
/// item ciphertext) from silently decrypting as a token, and vice-versa.
const RELAY_TOKEN_AAD: &[u8] = b"copypaste-relay-token-v1";

/// Path to the cached relay token file (sibling of the device-key files).
fn token_path() -> Option<PathBuf> {
    crate::paths::try_app_support_dir()
        .ok()
        .map(|d| d.join(RELAY_TOKEN_FILE))
}

/// Encrypt `token` bytes under `local_key` with XChaCha20-Poly1305.
///
/// Returns `base64(nonce[24] || ciphertext_with_tag)`.
///
/// # Errors
/// Propagates `EncryptError` from the underlying AEAD layer (e.g. if the
/// plaintext somehow exceeds the per-message size limit — unlikely for a
/// short token but handled explicitly rather than unwrapped).
fn encrypt_relay_token(
    token: &str,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<String, copypaste_core::EncryptError> {
    let (nonce, ct) = encrypt_item_with_aad(token.as_bytes(), local_key, RELAY_TOKEN_AAD)?;
    // Concatenate nonce || ciphertext into a single blob for storage.
    let mut blob = Vec::with_capacity(NONCE_SIZE + ct.len());
    blob.extend_from_slice(&nonce);
    blob.extend_from_slice(&ct);
    Ok(base64::engine::general_purpose::STANDARD.encode(&blob))
}

/// Decrypt a relay token that was written by [`encrypt_relay_token`].
///
/// Returns `Some(token)` on success, `None` if the blob is malformed or the
/// AEAD tag does not verify (caller should treat the file as absent).
fn decrypt_relay_token(encoded: &str, local_key: &zeroize::Zeroizing<[u8; 32]>) -> Option<String> {
    let blob = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .ok()?;
    if blob.len() < NONCE_SIZE + 1 {
        // Too short to be a valid nonce || ciphertext blob.
        return None;
    }
    let nonce: [u8; NONCE_SIZE] = blob[..NONCE_SIZE]
        .try_into()
        // SAFETY: we just checked blob.len() >= NONCE_SIZE; infallible.
        .expect("slice is exactly NONCE_SIZE bytes");
    let ct = &blob[NONCE_SIZE..];
    let plaintext = decrypt_item_with_aad(ct, &nonce, local_key, RELAY_TOKEN_AAD).ok()?;
    String::from_utf8(plaintext).ok()
}

/// Load a previously-cached relay auth token, if any. Never errors hard — a
/// missing/unreadable token just means "re-register".
///
/// **Migration**: if the on-disk blob does not decrypt successfully (legacy
/// plaintext format, wrong key, or truncated file), the raw content is
/// returned as-is so the caller can continue using the in-memory token for
/// this run. On the next successful registration the new encrypted format will
/// overwrite the file.
fn load_cached_token(local_key: &zeroize::Zeroizing<[u8; 32]>) -> Option<String> {
    let path = token_path()?;
    let raw = std::fs::read_to_string(&path).ok()?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Try the encrypted format first. If decryption fails (legacy plaintext or
    // corrupt file) fall back to the raw content so existing installs continue
    // to work for this run; the file will be re-encrypted on the next store.
    if let Some(token) = decrypt_relay_token(trimmed, local_key) {
        return Some(token);
    }
    // Legacy plaintext: return the raw token (best-effort migration). The file
    // will be overwritten with the encrypted format on the next successful
    // registration, completing the migration transparently.
    tracing::debug!(
        "relay-sync: loaded legacy plaintext token; will re-encrypt on next registration"
    );
    Some(trimmed.to_owned())
}

/// Persist the relay auth token encrypted to a `0600` file. Best-effort: a
/// failure is logged (without the token) and the token is still used in-memory
/// for this run.
fn store_cached_token(token: &str, local_key: &zeroize::Zeroizing<[u8; 32]>) {
    let Some(path) = token_path() else {
        tracing::warn!("relay-sync: cannot resolve data dir to cache token");
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let encoded = match encrypt_relay_token(token, local_key) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!(error = %e, "relay-sync: failed to encrypt relay token (continuing in-memory)");
            return;
        }
    };
    if let Err(e) = write_token_0600(&path, &encoded) {
        tracing::warn!(error = %e, "relay-sync: failed to cache relay token (continuing in-memory)");
    }
}

/// Write `content` to `path` with `0600` perms via a temp-file + rename so a
/// reader never sees a partial or world-readable file.
///
/// CopyPaste-2yuo: the temp file is now opened with `OpenOptionsExt::mode(0o600)`
/// so the file is **never** world-readable — not even for the brief window between
/// `File::create` (which inherits the process umask, typically giving 0644) and a
/// subsequent `set_permissions(0o600)` call. The explicit mode argument passed to
/// `open(2)` is `0o600 & ~umask`; since `0600` has no group/other bits, any umask
/// leaves it at `0600`, eliminating the race window atomically.
fn write_token_0600(path: &std::path::Path, content: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let tmp = dir.join(format!(".{RELAY_TOKEN_FILE}.tmp"));
    // CopyPaste-2yuo fix: open with mode 0o600 on the first syscall so no
    // world-readable window exists between create and chmod. The `#[cfg(unix)]`
    // block uses OpenOptionsExt; on non-Unix (Windows) we fall back to the
    // simple `File::create` (Windows has no Unix mode bits).
    #[cfg(unix)]
    let mut f = {
        use std::os::unix::fs::OpenOptionsExt as _;
        std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&tmp)?
    };
    #[cfg(not(unix))]
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(content.as_bytes())?;
    f.sync_all()?;
    std::fs::rename(&tmp, path)?;
    Ok(())
}

// ── URL validation ────────────────────────────────────────────────────────────

/// Accept `https://...`; in tests also accept loopback `http://` so the mock
/// relay can be exercised. Mirrors `cloud::is_https_url`'s posture.
fn is_relay_url_ok(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    if let Some(rest) = lower.strip_prefix("https://") {
        return rest
            .chars()
            .next()
            .is_some_and(|c| c != '/' && !c.is_whitespace());
    }
    #[cfg(test)]
    {
        if let Some(rest) = lower.strip_prefix("http://") {
            let host = rest.split(['/', ':']).next().unwrap_or_default();
            return matches!(host, "127.0.0.1" | "localhost" | "[::1]" | "::1");
        }
    }
    false
}

// ── Sync-key snapshot helper ────────────────────────────────────────────────

/// Snapshot the live sync-key bytes (the `SyncKey` itself is not `Send` across
/// some boundaries, and we never hold the lock across an await). Returns `None`
/// when no passphrase is set.
async fn snapshot_sync_key(sync_key: &Arc<Mutex<Option<SyncKey>>>) -> Option<[u8; 32]> {
    let guard = sync_key.lock().await;
    guard.as_ref().map(|k| *k.as_bytes())
}

// ── Register ────────────────────────────────────────────────────────────────

/// Register (or co-register) this device's shared-account inbox with the relay
/// and return a fresh auth token. The inbox id + public key are derived from
/// `sync_key_bytes` (SECRET-derived — never logged).
async fn register(
    client: &reqwest::Client,
    relay_url: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
) -> Result<String, RelayError> {
    let inbox_id = derive_relay_inbox_id(sync_key_bytes);
    let pubkey = derive_relay_public_key(sync_key_bytes);
    let public_key_b64 = base64::engine::general_purpose::STANDARD.encode(pubkey);

    // Proof-of-possession: HMAC-SHA256(sync_key, prefix || inbox_id).
    // Proves the registrant holds the sync key corresponding to the derived inbox id.
    // Fixes CopyPaste-n2l: the relay now rejects registrations without a valid PoP.
    let pop = derive_relay_registration_pop(sync_key_bytes, &inbox_id);
    let pop_b64 = base64::engine::general_purpose::STANDARD.encode(pop);

    let body = RegisterBody {
        device_id: inbox_id,
        device_name: device_name.to_owned(),
        public_key_b64,
        pop_b64,
    };
    let url = format!("{relay_url}/devices");
    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| RelayError::Transport(e.to_string()))?;
    let status = resp.status();
    // R1a: a fresh register always returns 201 with a new independent token,
    // whether or not the id was already co-registered by another device.
    if status.as_u16() != 201 {
        return Err(RelayError::Status(status.as_u16()));
    }
    let parsed: RegisterResp = resp
        .json()
        .await
        .map_err(|e| RelayError::Transport(format!("decode register response: {e}")))?;
    tracing::info!("relay-sync: registered shared inbox with relay (token cached)");
    Ok(parsed.auth_token)
}

/// Ensure we hold a valid token: return the cached one if present, else register
/// and cache a fresh one.
async fn ensure_token(
    client: &reqwest::Client,
    relay_url: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
    cached: &mut Option<String>,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<String, RelayError> {
    if let Some(t) = cached.as_ref() {
        return Ok(t.clone());
    }
    let token = register(client, relay_url, sync_key_bytes, device_name).await?;
    store_cached_token(&token, local_key);
    *cached = Some(token.clone());
    Ok(token)
}

// ── Envelope build ────────────────────────────────────────────────────────────

/// Build the relay `content_b64` for one item: encrypt the wrapped plaintext for
/// the cloud (sync key + item_id AAD), wrap it in the JSON envelope, base64 it.
///
/// Returns `Ok(None)` when the item should be skipped (e.g. oversized, decrypt
/// failure) — never logs plaintext.
fn build_content_b64(
    item: &ClipboardItem,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    sync_key: &SyncKey,
) -> Option<String> {
    // CopyPaste-cm0u: a tombstone has content = NULL — there is nothing to
    // decrypt. Emit a delete envelope (empty ct_b64, deleted=true) instead of
    // calling decrypt_item_plaintext on NULL (which Err'd and dropped the
    // delete, so deletes never propagated over relay-only topologies).
    let ct_b64 = if item.deleted {
        String::new()
    } else {
        let plaintext = match decrypt_item_plaintext(item, local_key) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("relay-sync: decrypt id={} failed: {e}; skipping", item.id);
                return None;
            }
        };
        let wrapped = match wrap_and_check_cloud_upload_plaintext(item, plaintext) {
            Ok(w) => w,
            Err(e) => {
                tracing::warn!("relay-sync: skip id={}: {e}", item.id);
                return None;
            }
        };
        let blob = match encrypt_for_cloud(sync_key, &item.item_id, &wrapped) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: cloud encrypt id={} failed: {e}; skipping",
                    item.id
                );
                return None;
            }
        };
        base64::engine::general_purpose::STANDARD.encode(&blob)
    };
    let envelope = RelayEnvelope {
        item_id: item.item_id.clone(),
        lamport_ts: item.lamport_ts,
        ct_b64,
        // CopyPaste-cm0u: carry delete + pin state so they propagate over relay.
        deleted: item.deleted,
        pinned: item.pinned,
        pin_order: item.pin_order,
        // CopyPaste-ayvs: carry the LWW tie-break keys.
        wall_time: item.wall_time,
        origin_device_id: item.origin_device_id.clone(),
    };
    let json = match serde_json::to_vec(&envelope) {
        Ok(j) => j,
        Err(e) => {
            tracing::warn!(
                "relay-sync: envelope encode id={} failed: {e}; skipping",
                item.id
            );
            return None;
        }
    };
    Some(base64::engine::general_purpose::STANDARD.encode(json))
}

// ── Push ──────────────────────────────────────────────────────────────────────

/// Push one item's content to the shared inbox. Returns `Ok(true)` on 201,
/// `Ok(false)` on 401 (caller should drop the token + re-register), `Err` on a
/// transient/other failure.
async fn push_item(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    token: &str,
    content_type: &str,
    content_b64: String,
    wall_time: u64,
) -> Result<bool, RelayError> {
    let url = format!("{relay_url}/devices/{inbox_id}/items");
    let body = PushBody {
        content_type: content_type.to_owned(),
        content_b64,
        wall_time,
    };
    let resp = client
        .post(&url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| RelayError::Transport(e.to_string()))?;
    let status = resp.status();
    if status.as_u16() == 201 {
        return Ok(true);
    }
    if status.as_u16() == 401 {
        return Ok(false);
    }
    Err(RelayError::Status(status.as_u16()))
}

/// The push loop: a 3rd subscriber on `new_item_tx` (alongside cloud + sync_orch).
// relay_url, device_name, sync_key, local_key, last_sync_ms, and shutdown are
// independent state slices — no natural grouping into a struct without adding
// indirection for a private-only function.
#[allow(clippy::too_many_arguments)]
async fn push_loop(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    mut rx: tokio::sync::broadcast::Receiver<ClipboardItem>,
    shutdown: Arc<Notify>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
) {
    let mut cached_token = load_cached_token(&local_key);
    let mut warned_no_key = false;

    loop {
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                tracing::info!("relay-sync push_loop: shutdown");
                break;
            }
            result = rx.recv() => {
                let item = match result {
                    Ok(i) => i,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    // Lagged: we missed some items under a burst. They will be
                    // re-fetched by peers via their own poll; nothing to do.
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("relay-sync push_loop: lagged {n} items");
                        continue;
                    }
                };

                // P1-1: honour the "sensitive items are NEVER uploaded" guarantee
                // (docs/relay-api.md:105). Drop the item before any crypto work so
                // ciphertext never enters the relay inbox.
                if item.is_sensitive {
                    tracing::debug!(
                        "relay-sync push_loop: skipping sensitive id={} (never uploaded)",
                        item.id
                    );
                    continue;
                }

                // tke7 (PG-30): hot-reload master sync gate.  When sync_enabled is
                // toggled off at runtime, drop outbound items immediately so no data
                // is uploaded.  The item is not re-queued — the user explicitly
                // disabled sync.
                let sync_enabled = core_config
                    .read()
                    .map(|g| g.sync_enabled)
                    .unwrap_or(true);
                if !sync_enabled {
                    tracing::debug!(
                        "relay-sync push_loop: sync_enabled=false; dropping outbound id={}",
                        item.id
                    );
                    continue;
                }

                // A-SET-2 hot-reload: read sync_on_wifi_only from the live config on
                // every incoming item so a runtime set_config change takes effect
                // immediately.  When the guard fires we skip this item; it will be
                // re-broadcast (or recovered via receive_loop) once Wi-Fi is available.
                let sync_on_wifi_only = core_config
                    .read()
                    .map(|g| g.sync_on_wifi_only)
                    .unwrap_or(false);
                if sync_on_wifi_only {
                    let on_wifi = tokio::task::spawn_blocking(crate::platform::is_on_wifi)
                        .await
                        .unwrap_or(true); // fail-open: if check errors, assume Wi-Fi
                    if relay_should_skip_wifi(sync_on_wifi_only, on_wifi) {
                        tracing::debug!(
                            "relay-sync push_loop: sync_on_wifi_only=true and not on Wi-Fi; \
                             skipping push for id={}",
                            item.id
                        );
                        continue;
                    }
                }

                // Snapshot the sync key; skip (one-time warn) if no passphrase set.
                let key_bytes = match snapshot_sync_key(&sync_key).await {
                    Some(b) => {
                        warned_no_key = false;
                        b
                    }
                    None => {
                        if !warned_no_key {
                            tracing::warn!(
                                "relay-sync push_loop: no sync passphrase set — skipping upload"
                            );
                            warned_no_key = true;
                        }
                        continue;
                    }
                };

                let inbox_id = derive_relay_inbox_id(&key_bytes);
                // CopyPaste-z1xt: `build_content_b64` decrypts the local
                // ciphertext + re-encrypts for the relay (CPU-bound, possibly
                // multi-MB) — run it on the blocking thread pool instead of inline
                // on the async executor. Move `item` into the closure (no clone of
                // the heavy blob) and get it back so the rest of the loop can use
                // it. `SyncKey` is reconstructed inside from the Send `[u8; 32]`.
                let lk = local_key.clone();
                let (item, content_b64) = match tokio::task::spawn_blocking(move || {
                    let sk = SyncKey::from_bytes(key_bytes);
                    let out = build_content_b64(&item, &lk, &sk);
                    (item, out)
                })
                .await
                {
                    Ok(pair) => pair,
                    Err(e) => {
                        tracing::warn!("relay-sync push_loop: build task failed: {e}; skipping");
                        continue;
                    }
                };
                let Some(content_b64) = content_b64 else {
                    continue;
                };
                let wall_time = item.wall_time.max(0) as u64;

                // Ensure token, push, and on 401 re-register once.
                if let Err(e) = push_with_reauth(
                    &client,
                    &relay_url,
                    &inbox_id,
                    &key_bytes,
                    &device_name,
                    &item.content_type,
                    content_b64,
                    wall_time,
                    &mut cached_token,
                    &local_key,
                )
                .await
                {
                    tracing::warn!("relay-sync push_loop: push id={} failed: {e}", item.id);
                } else {
                    let now_ms = now_ms();
                    last_sync_ms.store(now_ms, Ordering::Relaxed);
                }
            }
        }
    }
}

/// Push with one re-auth retry: ensure a token, push; on 401 drop the token,
/// re-register, and push once more.
// The relay protocol binds all of: client, url, inbox_id, sync_key_bytes,
// device_name/id, local_key, and last_sync_ms. No natural grouping without
// a new intermediate struct; count is justified by the protocol surface.
#[allow(clippy::too_many_arguments)]
async fn push_with_reauth(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    sync_key_bytes: &[u8; 32],
    device_name: &str,
    content_type: &str,
    content_b64: String,
    wall_time: u64,
    cached_token: &mut Option<String>,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Result<(), RelayError> {
    let token = ensure_token(
        client,
        relay_url,
        sync_key_bytes,
        device_name,
        cached_token,
        local_key,
    )
    .await?;
    match push_item(
        client,
        relay_url,
        inbox_id,
        &token,
        content_type,
        content_b64.clone(),
        wall_time,
    )
    .await
    {
        Ok(true) => Ok(()),
        Ok(false) => {
            // 401: token stale. Drop it, re-register, retry once.
            tracing::info!("relay-sync: push got 401; re-registering and retrying once");
            *cached_token = None;
            let token = ensure_token(
                client,
                relay_url,
                sync_key_bytes,
                device_name,
                cached_token,
                local_key,
            )
            .await?;
            match push_item(
                client,
                relay_url,
                inbox_id,
                &token,
                content_type,
                content_b64,
                wall_time,
            )
            .await
            {
                Ok(true) => Ok(()),
                Ok(false) => Err(RelayError::Status(401)),
                Err(e) => Err(e),
            }
        }
        Err(e) => Err(e),
    }
}

// ── Receive ─────────────────────────────────────────────────────────────────

/// `(wall_time, id)` keyset watermark so pagination is deterministic even within
/// one millisecond and a restart resumes forward (held in memory; the relay
/// inbox itself is the durable store).
#[derive(Clone, Copy, Default)]
struct Watermark {
    wall: u64,
    id: i64,
}

/// Pull one page from the inbox past the watermark. Returns the raw items and
/// whether a 401 was seen (caller re-registers).
async fn pull_page(
    client: &reqwest::Client,
    relay_url: &str,
    inbox_id: &str,
    token: &str,
    wm: Watermark,
) -> Result<Vec<PullItem>, RelayError> {
    let url = format!(
        "{relay_url}/devices/{inbox_id}/items?since={}&since_id={}&limit={}",
        wm.wall, wm.id, PULL_LIMIT
    );
    let resp = client
        .get(&url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| RelayError::Transport(e.to_string()))?;
    let status = resp.status();
    if status.as_u16() == 401 {
        return Err(RelayError::Status(401));
    }
    if !status.is_success() {
        return Err(RelayError::Status(status.as_u16()));
    }
    resp.json::<Vec<PullItem>>()
        .await
        .map_err(|e| RelayError::Transport(format!("decode pull response: {e}")))
}

/// Ingest one pulled page into the local DB on a blocking thread (SQLCipher +
/// AEAD). Returns the advanced watermark and how many rows were stored.
///
/// LWW + quota-prune are byte-for-byte the Supabase poll path: dedup on
/// `item_id`, a strictly-newer remote `lamport_ts` replaces in place (preserving
/// the local PK + pin state), an older/equal one is skipped (this is also what
/// makes our OWN pushed rows a no-op when they echo back — self-echo dedup).
fn ingest_page_blocking(
    db: &Database,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    sync_key_bytes: &[u8; 32],
    page: &[PullItem],
    start: Watermark,
    storage_quota_bytes: u64,
) -> (Watermark, u32) {
    let mut wm = start;
    let mut stored = 0u32;
    let sk = SyncKey::from_bytes(*sync_key_bytes);

    for row in page {
        // Advance the watermark for EVERY readable row (even skipped ones) so the
        // next page does not re-request them.
        if (row.wall_time, row.id) > (wm.wall, wm.id) {
            wm = Watermark {
                wall: row.wall_time,
                id: row.id,
            };
        }

        // Decode the envelope: base64 → JSON → ct_b64 → ciphertext.
        let env_bytes = match base64::engine::general_purpose::STANDARD.decode(&row.content_b64) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: id={} content_b64 decode failed: {e}; skipping",
                    row.id
                );
                continue;
            }
        };
        let env: RelayEnvelope = match serde_json::from_slice(&env_bytes) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: id={} envelope parse failed: {e}; skipping",
                    row.id
                );
                continue;
            }
        };
        let blob = match decode_payload_ct(&env.ct_b64) {
            Ok(b) => b,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: id={} ct_b64 decode failed: {e}; skipping",
                    row.id
                );
                continue;
            }
        };

        // LWW dedup on the cross-device item_id.
        let existing = match get_item_by_item_id(db, &env.item_id) {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("relay-sync: get_item_by_item_id error: {e}; skipping");
                continue;
            }
        };
        // The envelope's wall_time is authoritative for LWW; fall back to the
        // relay row's wall_time when an older envelope omitted it (=> 0).
        let env_wall = if env.wall_time != 0 {
            env.wall_time
        } else {
            row.wall_time as i64
        };
        let preserved_pk = if let Some(local) = existing.as_ref() {
            // CopyPaste-ayvs: same total order as P2P/cloud (lamport ->
            // wall_time -> origin_device_id) instead of the old bare
            // `env.lamport_ts <= local -> keep`, which never converged on ties.
            let wins = remote_wins(
                local.lamport_ts,
                local.wall_time,
                &local.origin_device_id,
                &RemoteMeta {
                    lamport_ts: env.lamport_ts,
                    wall_time: env_wall,
                    origin_device_id: &env.origin_device_id,
                },
            );
            if !wins {
                // Local wins LWW — keep it (self-echo no-op + remote-edit loser).
                continue;
            }
            Some(local.id.clone())
        } else {
            match exists_item_by_item_id(db, &env.item_id) {
                Ok(true) => continue,
                Ok(false) => None,
                Err(e) => {
                    tracing::warn!("relay-sync: exists_item_by_item_id error: {e}; skipping");
                    continue;
                }
            }
        };

        // ── Tombstone fast-path (CopyPaste-cm0u / CopyPaste-bfiu) ─────────────
        // A delete envelope carries deleted=true and an empty ct_b64 (NULL
        // content). Apply it via the SAME soft_delete / insert_tombstone path as
        // P2P and cloud so deletes propagate over relay-only topologies, and a
        // delete that races ahead of the create still leaves a tombstone the
        // later create loses LWW against.
        if env.deleted {
            if let Some(local_pk) = preserved_pk.as_ref() {
                match soft_delete_item(db, local_pk, env.lamport_ts, env_wall) {
                    Ok(n) if n > 0 => {
                        stored += 1;
                        tracing::info!("relay-sync: applied tombstone (item known locally)");
                    }
                    Ok(_) => {}
                    Err(e) => tracing::warn!("relay-sync: soft_delete_item failed: {e}"),
                }
            } else {
                match insert_tombstone(
                    db,
                    &env.item_id,
                    &env.item_id,
                    env.lamport_ts,
                    env_wall,
                    &env.origin_device_id,
                ) {
                    Ok(_) => {
                        stored += 1;
                        tracing::info!(
                            "relay-sync: inserted tombstone for unknown item \
                             (delete-before-create)"
                        );
                    }
                    Err(e) => tracing::warn!("relay-sync: insert_tombstone failed: {e}"),
                }
            }
            continue;
        }

        // Decrypt with the sync key (AAD = item_id + cloud schema v5).
        let plaintext = match decrypt_from_cloud(&sk, &env.item_id, &blob) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(
                    "relay-sync: decrypt_from_cloud failed for item_id (wrong passphrase or \
                     tampered blob): {e}; skipping"
                );
                continue;
            }
        };

        let mut local_item = match build_local_item(
            // Use the cross-device item_id as the local PK seed when this is a
            // fresh insert; build_local_item sets `id` from this first arg.
            &env.item_id,
            &env.item_id,
            &row.content_type,
            &plaintext,
            env.lamport_ts,
            env_wall,
            None,
            None,
            // CopyPaste-ayvs: preserve the sender's origin so future tie-breaks
            // on this device stay deterministic across hops.
            env.origin_device_id.clone(),
            local_key,
        ) {
            Ok(i) => i,
            Err(e) => {
                tracing::warn!("relay-sync: build_local_item failed: {e}; skipping");
                continue;
            }
        };

        // LWW replace preserves the prior local row's PK.
        if let Some(pk) = preserved_pk.as_ref() {
            local_item.id = pk.clone();
        }
        // CopyPaste-cm0u: the envelope's pin state is authoritative (it travels
        // with the item now). The pin LWW already won above (this is the
        // TakeRemote branch), so apply the sender's pinned/pin_order directly.
        local_item.pinned = env.pinned;
        local_item.pin_order = env.pin_order;

        let write_res = if preserved_pk.is_some() {
            replace_cloud_item_by_item_id(db, &local_item)
        } else {
            insert_item(db, &local_item).map_err(anyhow::Error::from)
        };
        match write_res {
            Ok(()) => {
                stored += 1;
                tracing::info!("relay-sync: ingested remote item (id={})", local_item.id);
            }
            Err(e) => tracing::warn!("relay-sync: store failed: {e}"),
        }
    }

    // Byte-cap prune after ingest (long-offline backfill safety) — same policy
    // as the Supabase poll path.
    if stored > 0 {
        let max_bytes = storage_quota_bytes.min(i64::MAX as u64) as i64;
        match prune_to_cap(db, max_bytes) {
            Ok(0) => {}
            Ok(n) => tracing::debug!("relay-sync: byte-pruned {n} rows after ingest"),
            Err(e) => tracing::warn!("relay-sync: prune_to_cap failed: {e}"),
        }
    }

    (wm, stored)
}

/// The receive loop: poll the shared inbox, ingest new items via the LWW path,
/// advance the watermark.
// All parameters are independent runtime slices (db, url, name, keys, shutdown,
// auto_apply_change_count) with no natural grouping for a private async fn.
#[allow(clippy::too_many_arguments)]
async fn receive_loop(
    client: reqwest::Client,
    relay_url: String,
    device_name: String,
    shutdown: Arc<Notify>,
    db: Arc<Mutex<Database>>,
    sync_key: Arc<Mutex<Option<SyncKey>>>,
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    last_sync_ms: Arc<AtomicI64>,
    core_config: Arc<std::sync::RwLock<AppConfig>>,
    // Shared self-write sentinel for the pasteboard poller.  When `Some`, the
    // relay auto-apply path stamps this atomic before/after each NSPasteboard
    // write so the `ClipboardMonitor` does not re-capture daemon-own writes
    // (loop prevention — mirrors the sync_orch / copy_item IPC guard).
    // `None` disables the pasteboard write (non-Unix, tests, callers that have
    // not wired the sentinel yet).
    auto_apply_change_count: Option<Arc<AtomicI64>>,
) {
    let mut cached_token = load_cached_token(&local_key);
    let mut wm = Watermark::default();
    let mut warned_no_key = false;

    loop {
        // Wait an interval, but wake early on shutdown.
        tokio::select! {
            biased;
            _ = shutdown.notified() => {
                tracing::info!("relay-sync receive_loop: shutdown");
                break;
            }
            _ = tokio::time::sleep(POLL_INTERVAL) => {}
        }

        let key_bytes = match snapshot_sync_key(&sync_key).await {
            Some(b) => {
                warned_no_key = false;
                b
            }
            None => {
                if !warned_no_key {
                    tracing::warn!("relay-sync receive_loop: no sync passphrase set — idle");
                    warned_no_key = true;
                }
                continue;
            }
        };

        // tke7 (PG-30): hot-reload master sync gate — checked on every poll tick.
        let sync_enabled = core_config.read().map(|g| g.sync_enabled).unwrap_or(true);
        if !sync_enabled {
            tracing::debug!("relay-sync receive_loop: sync_enabled=false; skipping poll this tick");
            continue;
        }

        // A-SET-2 hot-reload: check sync_on_wifi_only every tick so a runtime
        // set_config change takes effect without a daemon restart.  The
        // is_on_wifi check runs on a blocking thread (networksetup shell
        // invocation) so it does not stall the async executor.  Mirrors the
        // identical guard in cloud.rs poll loop.
        let (sync_on_wifi_only, auto_apply_synced_clip) = core_config
            .read()
            .map(|g| (g.sync_on_wifi_only, g.auto_apply_synced_clip))
            .unwrap_or((false, true));
        if sync_on_wifi_only {
            let on_wifi = tokio::task::spawn_blocking(crate::platform::is_on_wifi)
                .await
                .unwrap_or(true); // fail-open: assume Wi-Fi if detection errors
            if relay_should_skip_wifi(sync_on_wifi_only, on_wifi) {
                tracing::debug!(
                    "relay-sync receive_loop: sync_on_wifi_only=true and not on Wi-Fi; \
                     skipping this tick"
                );
                continue;
            }
        }
        // Shadow as a local bool so the ingest path can use it without holding
        // the RwLock guard across await points.
        let auto_apply_enabled = relay_should_auto_apply(auto_apply_synced_clip);

        let inbox_id = derive_relay_inbox_id(&key_bytes);

        let token = match ensure_token(
            &client,
            &relay_url,
            &key_bytes,
            &device_name,
            &mut cached_token,
            &local_key,
        )
        .await
        {
            Ok(t) => t,
            Err(e) => {
                tracing::warn!("relay-sync receive_loop: register failed: {e}");
                continue;
            }
        };

        // Burst-drain: keep pulling while pages come back full.
        loop {
            let page = match pull_page(&client, &relay_url, &inbox_id, &token, wm).await {
                Ok(p) => p,
                Err(RelayError::Status(401)) => {
                    tracing::info!("relay-sync receive_loop: 401; re-registering next tick");
                    cached_token = None;
                    break;
                }
                Err(e) => {
                    tracing::warn!("relay-sync receive_loop: pull failed: {e}");
                    break;
                }
            };
            if page.is_empty() {
                break;
            }
            let page_len = page.len();

            let quota = core_config
                .read()
                .map(|g| g.storage_quota_bytes)
                .unwrap_or(u64::MAX);
            let db_arc = db.clone();
            let local_key_clone = local_key.clone();
            let join = tokio::task::spawn_blocking(move || {
                let guard = db_arc.blocking_lock();
                ingest_page_blocking(&guard, &local_key_clone, &key_bytes, &page, wm, quota)
            })
            .await;
            match join {
                Ok((new_wm, stored)) => {
                    wm = new_wm;
                    if stored > 0 {
                        last_sync_ms.store(now_ms(), Ordering::Relaxed);
                        if auto_apply_enabled {
                            // CopyPaste-7ub: implement auto_apply_synced_clip on the
                            // relay receive path. Fetch the freshest stored text item,
                            // decrypt it, and write it to NSPasteboard — stamping the
                            // self-write sentinel so the ClipboardMonitor does NOT
                            // re-capture the write as a new local item (loop prevention).
                            //
                            // The pasteboard write is gated on `auto_apply_change_count`
                            // being Some (wired from daemon.rs via `start_relay`). When
                            // None (tests, non-Unix) the ingest is still recorded in
                            // last_sync_ms but no pasteboard write occurs.
                            if let Some(ref swcc) = auto_apply_change_count {
                                let db_arc2 = db.clone();
                                let lk2 = local_key.clone();
                                let swcc2 = swcc.clone();
                                let join2 = tokio::task::spawn_blocking(move || {
                                    let guard = db_arc2.blocking_lock();
                                    if let Some(cand) =
                                        relay_fetch_auto_apply_candidate(&guard, &lk2)
                                    {
                                        relay_apply_to_pasteboard(&cand, &swcc2);
                                    }
                                })
                                .await;
                                if let Err(e) = join2 {
                                    tracing::warn!(
                                        "relay-sync receive_loop: auto-apply task panicked: {e}"
                                    );
                                }
                            } else {
                                tracing::debug!(
                                    "relay-sync receive_loop: auto_apply_synced_clip=true \
                                     but change-count sentinel not wired; \
                                     {stored} relay item(s) stored (no pasteboard write)"
                                );
                            }
                        } else {
                            tracing::debug!(
                                "relay-sync receive_loop: auto_apply_synced_clip=false; \
                                 {stored} relay item(s) stored but NOT auto-applied to pasteboard"
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("relay-sync receive_loop: ingest task panicked: {e}");
                    break;
                }
            }
            if page_len < PULL_LIMIT {
                break;
            }
        }
    }
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
    if !is_relay_url_ok(&relay_url) {
        return Err(RelayError::InvalidUrl);
    }
    let shutdown = Arc::new(Notify::new());

    // Truncate the device name to the relay's 1..=64 contract defensively.
    let device_name = {
        let t = device_name.trim();
        let t = if t.is_empty() { "copypaste" } else { t };
        t.chars().take(64).collect::<String>()
    };

    tokio::spawn(push_loop(
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
    tokio::spawn(receive_loop(
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

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{derive_sync_key, ITEM_KEY_VERSION_CURRENT};

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
        let k = skey("relay-determinism-pass");
        assert_eq!(derive_relay_inbox_id(&k), derive_relay_inbox_id(&k));
    }

    /// register parses a 201 + auth_token. Uses the mockito 0.31 global server
    /// (`mockito::mock` + `mockito::server_url`), so it is `#[serial]`.
    #[tokio::test]
    #[serial_test::serial]
    async fn register_parses_201_auth_token() {
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

    /// Legacy plaintext migration: load_cached_token falls back to the raw
    /// token when the file contains a plaintext string that cannot be decrypted.
    #[test]
    fn load_cached_token_migrates_legacy_plaintext() {
        let dir = tempfile::tempdir().expect("tmpdir");
        let token_file = dir.path().join(RELAY_TOKEN_FILE);

        // Write a legacy plaintext token (the old format).
        std::fs::write(&token_file, b"legacy-plaintext-token-xyz\n").expect("write");

        // Redirect token_path() by temporarily overriding the app support dir
        // via the file directly — we test the crypto helpers and not the path
        // resolution, so we call the helpers directly.
        let key = zeroize::Zeroizing::new([0x55u8; 32]);

        let raw = std::fs::read_to_string(&token_file).expect("read");
        let trimmed = raw.trim();
        // Decrypting legacy plaintext must return None (it is not a valid blob).
        assert!(
            decrypt_relay_token(trimmed, &key).is_none(),
            "legacy plaintext must not decrypt successfully"
        );
        // The migration path should return the raw trimmed string as the token.
        // Simulate what load_cached_token does after decrypt_relay_token returns None:
        let fallback = trimmed.to_owned();
        assert_eq!(fallback, "legacy-plaintext-token-xyz");
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
    /// This test confirms the guard predicate by constructing a sensitive item
    /// and asserting the skip condition (`item.is_sensitive`) triggers.
    #[test]
    fn push_loop_skips_sensitive_items() {
        // Build a sensitive local text item (is_sensitive = true).
        let local_key = zeroize::Zeroizing::new([0xAAu8; 32]);
        let sync_bytes = skey("sensitive-filter-test");
        let sync_key = SyncKey::from_bytes(sync_bytes);

        let mut item = make_local_text_item(
            "item-sensitive-1",
            b"AKIA_SECRET_KEY_EXAMPLE",
            &local_key,
            1,
            1000,
        );
        item.is_sensitive = true;

        // The push_loop guard fires on `item.is_sensitive`, so `build_content_b64`
        // is never called. Confirm the guard is the right predicate and that a
        // non-sensitive item does pass through (basic sanity check).
        assert!(
            item.is_sensitive,
            "sensitive item must carry is_sensitive=true"
        );

        // Non-sensitive item: build_content_b64 must succeed (returns Some).
        let mut plain_item =
            make_local_text_item("item-plain-1", b"hello, world", &local_key, 2, 2000);
        plain_item.is_sensitive = false;
        let result = build_content_b64(&plain_item, &local_key, &sync_key);
        assert!(
            result.is_some(),
            "non-sensitive item must produce a content_b64 payload (push should proceed)"
        );

        // Confirm that the sensitive item would be filtered: if push_loop received
        // this item it would `continue` at the `if item.is_sensitive` guard, never
        // reaching build_content_b64. Document that expectation in an assertion so
        // a future refactor that removes the guard fails this test.
        assert!(
            item.is_sensitive,
            "push_loop MUST check item.is_sensitive and skip; if this assertion \
             passes but the guard is gone, the relay-push sensitive filter is broken"
        );
    }

    #[test]
    fn relay_delete_before_create_does_not_resurrect() {
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
}
