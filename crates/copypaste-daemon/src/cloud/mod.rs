//! Cloud sync orchestrator for Supabase.
//!
//! Enabled at runtime when `SUPABASE_URL` and `SUPABASE_ANON_KEY` environment
//! variables are set (regardless of whether the `cloud-sync` Cargo feature is
//! compiled in — the feature gate controls whether the `reqwest` dep is present).
//!
//! Two background tasks are spawned:
//! - **push_loop**: receives new [`copypaste_core::ClipboardItem`]s from a broadcast channel and
//!   POSTs them to `POST /rest/v1/clipboard_items`.
//! - **realtime_loop**: polls `GET /rest/v1/clipboard_items?order=wall_time.asc&limit=20`
//!   every 10 seconds (forward pagination from a persisted watermark) and inserts
//!   any unknown items into the local DB.
//!   (Full WebSocket realtime requires the separate `copypaste-supabase` crate;
//!   this implementation uses polling so the daemon compiles without extra deps.)
//!
//! ## Security (Wave 1.6 fail-closed hardening)
//!
//! - **Auth fail-closed**: if `SUPABASE_EMAIL`/`SUPABASE_PASSWORD` are set and
//!   sign-in fails, cloud sync aborts entirely instead of silently falling back
//!   to the public anon key (which would downgrade auth scope without the
//!   operator's knowledge). See [`CloudError::AuthFailed`].
//! - **HTTPS-only**: `SUPABASE_URL` must use the `https://` scheme. Any other
//!   scheme (including plain `http://`) is rejected at init.  See
//!   [`CloudError::InsecureUrl`].
//! - **Encrypted-DB sanity**: if an existing local database file is present
//!   AND has the SQLite/SQLCipher magic header, we refuse to proceed with an
//!   ephemeral encryption key (which would render the DB unreadable). The
//!   ephemeral-key path is only safe for a fresh, empty DB. See
//!   [`preflight_encrypted_db_check`].
//! - **Keychain degraded mode**: keychain access is probed with an explicit
//!   one-shot retry (3 attempts, exponential backoff). On persistent failure
//!   the daemon enters degraded mode — cloud sync is disabled, the error is
//!   surfaced, and we do NOT crash-loop. See [`probe_keychain_with_retry`].

pub(crate) mod auth;
pub(crate) mod backlog;
pub(crate) mod config;
pub(crate) mod handle;
pub(crate) mod ingest;
pub(crate) mod lifecycle;
pub(crate) mod poll;
pub(crate) mod push;
pub(crate) mod ws;

pub use config::{
    preflight_encrypted_db_check, probe_keychain_with_retry, CloudConfig, CloudError,
};
pub use handle::CloudHandle;
pub use ingest::exists_item;
pub use lifecycle::start_cloud;

// ── Test-only re-exports of private submodule items ───────────────────────────
//
// The three test modules below (tests, e2e_live, bytea_e2e) use private items
// from multiple submodules. Exposing them via `pub(crate)` here lets the test
// modules reach them through `use super::*;` without making them part of the
// public API.
#[cfg(test)]
pub(crate) use auth::{refresh_bearer, sign_in_with_password};
#[cfg(test)]
pub(crate) use config::{is_https_url, probe_with_retry, redact_email, SQLITE_MAGIC};
#[cfg(test)]
pub(crate) use ingest::encode_payload_ct_hex;
#[cfg(test)]
pub(crate) use poll::{build_poll_url, PollCursor};
#[cfg(test)]
pub(crate) use poll::{
    fetch_remote_rows, fetch_remote_rows_with_refresh, load_poll_watermark, poll_once,
    save_poll_watermark, FetchOutcome,
};
#[cfg(test)]
pub(crate) use push::{
    enqueue_for_retry, parse_retry_after_secs, push_item_with_retries,
    MUTATION_QUEUE_DRAIN_INTERVAL, PUSH_RETRY_QUEUE_CAP,
};

// ── CopyPaste-jdq5: v2 cloud-write opt-in gate ──────────────────────────────────

/// Environment flag that opts NEW cloud (Supabase) writes into the **v2
/// per-account-salt** key. Default OFF.
///
/// Reading (downloads) ALWAYS tries v2-then-v1 regardless of this flag — that is
/// pure forward-compatibility and cannot break anything. WRITING under v2,
/// however, makes a row unreadable by any peer that does not yet derive v2
/// (notably Android, whose Kotlin layer has no Supabase account id until its FFI
/// is updated, and any un-upgraded macOS device). CopyPaste-jdq5's acceptance
/// explicitly requires macOS<->Android interop to be preserved, so v2 writes stay
/// OFF until an operator confirms the whole fleet derives v2 and flips this flag.
/// This realises the standard "deploy readers before writers" format migration.
const CLOUD_V2_WRITES_ENV: &str = "COPYPASTE_CLOUD_KEY_V2_WRITES";

/// Returns `true` when new cloud writes should use the v2 per-account key.
///
/// Honours the truthy values `1` / `true` / `yes` / `on` (case-insensitive) so
/// the flag is forgiving to set; anything else (including unset) is `false`.
pub(crate) fn cloud_v2_writes_enabled() -> bool {
    std::env::var(CLOUD_V2_WRITES_ENV)
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

/// Snapshot the ordered cloud **READ** key candidates as a flat byte buffer of
/// concatenated 32-byte keys: the v2 per-account key FIRST (when present), then
/// the v1 key (when present). The download path passes this to `poll_once` /
/// trial-decrypts it so a row written under EITHER scheme is recovered (v2 for
/// post-cutover rows, v1 for legacy rows) — the AEAD auth tag rejects the wrong
/// key, so order only affects which is tried first, never correctness.
///
/// Each lock is taken and released independently (never both at once) and the
/// returned bytes are the caller's responsibility to zeroize.
pub(crate) async fn snapshot_cloud_read_key_bytes(
    sync_key: &std::sync::Arc<tokio::sync::Mutex<Option<copypaste_core::SyncKey>>>,
    sync_key_v2: &std::sync::Arc<tokio::sync::Mutex<Option<copypaste_core::SyncKey>>>,
) -> Vec<u8> {
    let mut out = Vec::with_capacity(64);
    {
        let g = sync_key_v2.lock().await;
        if let Some(k) = g.as_ref() {
            out.extend_from_slice(k.as_bytes());
        }
    }
    {
        let g = sync_key.lock().await;
        if let Some(k) = g.as_ref() {
            out.extend_from_slice(k.as_bytes());
        }
    }
    out
}

/// Snapshot the cloud **WRITE** key: the v2 per-account key when v2 writes are
/// enabled (`COPYPASTE_CLOUD_KEY_V2_WRITES`) AND a v2 key is installed, otherwise
/// the v1 key. Returns `None` only when NO key is set (no passphrase), which the
/// caller treats as "skip upload". The returned `u32` is the derivation version
/// (1 or 2) for logging only — never the key bytes.
pub(crate) async fn snapshot_cloud_write_key_bytes(
    sync_key: &std::sync::Arc<tokio::sync::Mutex<Option<copypaste_core::SyncKey>>>,
    sync_key_v2: &std::sync::Arc<tokio::sync::Mutex<Option<copypaste_core::SyncKey>>>,
) -> Option<([u8; 32], u32)> {
    if cloud_v2_writes_enabled() {
        let g = sync_key_v2.lock().await;
        if let Some(k) = g.as_ref() {
            return Some((
                *k.as_bytes(),
                copypaste_core::SYNC_KEY_DERIVATION_VERSION_V2,
            ));
        }
    }
    let g = sync_key.lock().await;
    g.as_ref().map(|k| {
        (
            *k.as_bytes(),
            copypaste_core::SYNC_KEY_DERIVATION_VERSION_V1,
        )
    })
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests;

// ════════════════════════════════════════════════════════════════════════════
// REAL Supabase cloud-sync e2e (against a LIVE local stack)
// ════════════════════════════════════════════════════════════════════════════
//
// These tests exercise the *product* cloud-sync code paths — the real
// `push_item_with_retries` push pipeline and the real `fetch_remote_rows` +
// `decrypt_from_cloud` + `build_local_item` + `insert_item` download pipeline —
// against a genuine Supabase stack reachable over HTTP on localhost. They are
// NOT mocked: rows really transit Postgres, RLS is really enforced by GoTrue
// JWTs, and the round-trip is proven by reading the item back into a second
// daemon's local SQLCipher store.
//
// Every test is `#[ignore]` so `cargo test` in CI (no Supabase) skips them.
// They additionally no-op (with a printed notice) unless `SUPABASE_TEST_ANON_KEY`
// is set — no key is baked into the source. Run explicitly against a live stack:
//
//   COPYPASTE_EPHEMERAL_KEY=1 \
//   SUPABASE_TEST_URL=http://127.0.0.1:54321 \
//   SUPABASE_TEST_ANON_KEY=<local-dev-anon-key> \
//   cargo test -p copypaste-daemon --features cloud-sync \
//       --lib --test-threads=1 -- --ignored e2e_live
//
// `SUPABASE_TEST_URL` defaults to the standard `supabase start` URL
// (`http://127.0.0.1:54321`); the anon key MUST be supplied via env so no
// credential is committed. A fresh GoTrue user is created per test via
// `/auth/v1/signup`, so no account credentials are committed either.
//
// ── WHY THIS MODULE LIVES IN cloud.rs (not tests/) ──────────────────────────
// `start_cloud` hard-rejects any non-`https://` URL (fail-closed, by design),
// so it cannot be pointed at a local `http://127.0.0.1` stack. To validate the
// product *without* re-implementing the REST calls, the test drives the same
// internal functions the loops call (`push_item_with_retries`, private
// `fetch_remote_rows`, `build_local_item`). Those are `pub(crate)` / private,
// reachable only from a child module of `cloud`. The codebase already follows
// this convention (the Wave 2.7 mockito tests above).
#[cfg(all(test, feature = "cloud-sync"))]
mod e2e_live;

// ════════════════════════════════════════════════════════════════════════════
// BYTEA-FAITHFUL Supabase e2e round-trip (no live stack, runs in CI)
// ════════════════════════════════════════════════════════════════════════════
//
// This module encodes the WIRE CONTRACT the Android `SupabaseClient` MUST match:
//
//     payload_ct = "\x" + lower-hex(nonce[24] || ciphertext)
//
// i.e. a Postgres `bytea` hex-INPUT literal on write, and PostgREST renders the
// same column back in hex-OUTPUT form (`\x<hex>`) on read regardless of how the
// bytes got in. The cross-platform cloud bug that this test backfills was hidden
// because the older tests were EITHER pure-crypto (no transport) OR mockito mocks
// that only assert status codes — neither emulated Postgres `bytea` semantics, so
// a writer that sent BARE BASE64 (the Android regression) looked identical on the
// wire to a writer that sent `\x<hex>`. The fake PostgREST below is the missing
// piece: it stores raw ciphertext bytes and ALWAYS serves them back as `\x<hex>`,
// so an encoding mismatch on either side surfaces as a decrypt failure.
//
// It runs over loopback HTTP via the `#[cfg(test)]`-only HTTPS-gate relaxation
// (`test_only_allows_local_http`); production still requires HTTPS. We drive the
// REAL product functions — `push_item_with_retries` (POST) and `fetch_remote_rows`
// (GET) — plus the real `encode_payload_ct_hex` / `decode_payload_ct` / cloud AEAD,
// so the bytes genuinely transit an HTTP socket and a bytea-semantics store.
#[cfg(all(test, feature = "cloud-sync"))]
mod bytea_e2e;
