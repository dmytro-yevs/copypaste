//! Database handle registry and DB-backed FFI exports.
//!
//! Covers: `DB_HANDLES`, `DB_BY_PATH`, `DB_HANDLE_TO_CACHE_KEY`, `with_cached_db`,
//! `open_database`, `close_database`, `add_clipboard_item`, `store_clipboard_item`,
//! `get_history_count`, `fts_search`, `get_history_page`, and related types
//! (`SearchResultItem`, `HistoryItem`).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

// SHA-256 for DB_BY_PATH cache key derivation (P1-8): hashing the raw 32-byte
// DB key so raw key material is not retained on the heap as a HashMap key.
use sha2::{Digest as _, Sha256};
// Zeroizing is used by open_database (unconditional) and the live DB path.
use zeroize::Zeroizing;

use crate::{panic_boundary, CopypasteError};

// These imports are only used by the live feature path.
#[cfg(feature = "android-uniffi-live")]
use copypaste_core::{
    build_item_aad_v2, encrypt_item_with_aad, is_sensitive_for_autowipe, AAD_SCHEMA_VERSION_V4,
    ITEM_KEY_VERSION_CURRENT,
};

// Database handle table. OnceLock is stable on Rust 1.70+ (our MSRV is 1.75).
static DB_HANDLES: OnceLock<Mutex<HashMap<u64, copypaste_core::Database>>> = OnceLock::new();
static NEXT_HANDLE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub fn db_handles() -> &'static Mutex<HashMap<u64, copypaste_core::Database>> {
    DB_HANDLES.get_or_init(|| Mutex::new(HashMap::new()))
}

// M5: path+key-keyed cache of open `Database` connections for the *live* FFI
// calls (`add_clipboard_item` / `get_history_count`). Previously each of those
// calls did `Database::open(...)` — a full SQLCipher open (PRAGMA key + key
// derivation + WAL setup) — and dropped the connection at function exit, i.e.
// one open+close per clipboard event. We now open once per `(db_path, key)`
// pair and reuse the connection for the life of the process.
//
// P1-8 fix: the cache key carries a SHA-256 HASH of the 32-byte DB key, NOT
// the raw key bytes. This prevents the key material from surviving on the heap
// as a HashMap key for the lifetime of the process. The hash still provides the
// "different key for the same path → fresh connection" discrimination because
// two distinct keys produce distinct hashes. The raw key is derived ephemerally
// inside `with_cached_db` via `Zeroizing<[u8;32]>` and never stored.
//
// `Database` wraps a `rusqlite::Connection` (Send, !Sync) — serialising all
// access behind this `Mutex` keeps it sound, exactly like the handle table.
// Path+key-hash keyed map of open `Database` connections. Aliased to keep the
// `static`/accessor signatures readable (and to satisfy clippy::type_complexity
// under newer toolchains). Keyed on (db_path, sha256(key)[..32]) so a different
// key for the same path opens a fresh connection rather than reusing one
// authenticated under another key, without retaining raw key material.
#[cfg(feature = "android-uniffi-live")]
type DbByPathMap = Mutex<HashMap<(String, [u8; 32]), copypaste_core::Database>>;

#[cfg(feature = "android-uniffi-live")]
static DB_BY_PATH: OnceLock<DbByPathMap> = OnceLock::new();

#[cfg(feature = "android-uniffi-live")]
pub fn db_by_path() -> &'static DbByPathMap {
    DB_BY_PATH.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Derive the cache key for `DB_BY_PATH` from a 32-byte DB key.
///
/// P1-8: we store SHA-256(key) rather than the raw key bytes so that raw key
/// material is NOT retained on the heap as part of the HashMap key. Two
/// distinct keys still produce distinct hashes (collision resistance), so the
/// "different key → fresh connection" discrimination is preserved.
///
/// The result is stack-allocated ([u8; 32]) and never written to a static —
/// only the 32-byte hash digest lives in the map key.
///
/// Not gated on `android-uniffi-live` because `open_database` and the
/// `DB_HANDLE_TO_CACHE_KEY` side-map machinery always need it, regardless of
/// whether the live DB cache path is compiled in.
pub fn key_cache_hash(key: &[u8; 32]) -> [u8; 32] {
    // SHA-256 output is 32 bytes; convert the GenericArray to a plain array.
    Sha256::digest(key.as_ref()).into()
}

/// Side-map from open_database handle → (path, key_hash) so `close_database`
/// can evict the corresponding DB_BY_PATH entry (P1-8 use-after-close fix).
///
/// Populated by `open_database`; cleared by `close_database`. Only active for
/// the handle-table path; the `with_cached_db` path does not use handles.
// The type is self-describing given the field-name comments; aliasing would
// not improve readability. Allow is here because the overall file size crossed
// the clippy::type_complexity token-count threshold after the PG-* additions.
#[allow(clippy::type_complexity)]
static DB_HANDLE_TO_CACHE_KEY: OnceLock<Mutex<HashMap<u64, (String, [u8; 32])>>> = OnceLock::new();

pub fn db_handle_to_cache_key() -> &'static Mutex<HashMap<u64, (String, [u8; 32])>> {
    DB_HANDLE_TO_CACHE_KEY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Run `f` against the cached `Database` for `(db_path, key)`, opening (and
/// caching) it on first use. The connection is reused across calls with the
/// same path **and** the same key; a different key for the same path opens a
/// separate connection instead of silently reusing the first one.
///
/// P1-8: the map key carries SHA-256(key) rather than the raw key bytes.
#[cfg(feature = "android-uniffi-live")]
pub fn with_cached_db<T>(
    db_path: &str,
    key: &[u8; 32],
    f: impl FnOnce(&copypaste_core::Database) -> Result<T, CopypasteError>,
) -> Result<T, CopypasteError> {
    let key_hash = key_cache_hash(key); // 32-byte hash; raw key not stored
    let cache_key = (db_path.to_string(), key_hash);
    let mut map = db_by_path().lock().unwrap_or_else(|e| e.into_inner());
    if !map.contains_key(&cache_key) {
        // #40b: evict any stale entry for the same path but a different key
        // before inserting the new connection. Without this, each key rotation
        // leaks a connection handle (the old (path, old_key_hash) entry stays in
        // the map forever). Entries for OTHER paths are unaffected.
        map.retain(|(p, h), _| p != db_path || h == &key_hash);
        let db =
            copypaste_core::Database::open(std::path::Path::new(db_path), key).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
        map.insert(cache_key.clone(), db);
    }
    // Just inserted or already present — but use ok_or instead of expect so a
    // logic error here (e.g. if HashMap::insert was somehow rolled back by a
    // reallocation failure) surfaces as a DatabaseError rather than unwinding
    // across the JNI boundary and aborting the JVM.
    let db = map
        .get(&cache_key)
        .ok_or_else(|| CopypasteError::DatabaseError {
            reason: "cache miss after insert — this is a bug".into(),
        })?;
    f(db)
}

// Stub shim so ffi_revocation.rs can import with_cached_db unconditionally.
// When the feature is off, all callers that actually invoke it are also gated
// on the feature, so this dead-code stub is never called.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn with_cached_db<T>(
    _db_path: &str,
    _key: &[u8; 32],
    _f: impl FnOnce(&copypaste_core::Database) -> Result<T, CopypasteError>,
) -> Result<T, CopypasteError> {
    Err(CopypasteError::DatabaseError {
        reason: "android-uniffi-live feature not enabled".into(),
    })
}

/// Open (or create) an encrypted SQLite database at `path` using the 32-byte `key`.
/// Returns an opaque handle for subsequent calls.
///
/// P1-8: records a `handle → (path, SHA-256(key))` entry in
/// `DB_HANDLE_TO_CACHE_KEY` so `close_database` can also evict the
/// corresponding `DB_BY_PATH` entry. The raw key bytes are never stored; only
/// the 32-byte hash is retained alongside the path.
pub fn open_database(path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        let db =
            copypaste_core::Database::open(std::path::Path::new(&path), &key_arr).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
        let handle = NEXT_HANDLE.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        // P1-8: record hash(key) for this handle so close_database can evict
        // the DB_BY_PATH entry without retaining raw key bytes.
        let key_hash = key_cache_hash(&key_arr);
        db_handle_to_cache_key()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(handle, (path.clone(), key_hash));
        // recover from mutex poison instead of panicking across FFI boundary
        db_handles()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(handle, db);
        Ok(handle)
    })
}

/// Release the handle-table entry for `handle`, allowing the `Database`
/// object to be dropped and its underlying SQLCipher connection closed.
///
/// P1-8 fix: also evicts the corresponding `DB_BY_PATH` entry (via the
/// `DB_HANDLE_TO_CACHE_KEY` side-map) so raw key material stored in the cache
/// (now a SHA-256 hash, not the raw bytes) is released, and the use-after-close
/// footgun is eliminated. If the handle was not opened via `open_database` (i.e.
/// it only ever went through `with_cached_db`) the side-map lookup is a no-op.
pub fn close_database(handle: u64) {
    // A poisoned mutex on the global handle table would otherwise abort the
    // JVM via `unwrap()`. Wrapping in `catch_result` converts that into a
    // `Result::Err(PanicError)` we then deliberately discard — `close_database`
    // is declared as void in the UDL and Kotlin callers cannot signal a
    // failure path, but at minimum we keep the process alive.
    let _ = panic_boundary::catch_result(|| {
        // P1-8: look up and remove the (path, key_hash) for this handle, then
        // evict the corresponding DB_BY_PATH entry so it is not kept alive
        // after the handle has been released.
        let _cache_key_opt = db_handle_to_cache_key()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&handle);
        #[cfg(feature = "android-uniffi-live")]
        if let Some(cache_key) = _cache_key_opt {
            db_by_path()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(&cache_key);
        }
        db_handles()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&handle); // recover from mutex poison instead of panicking across FFI boundary
        Ok::<(), CopypasteError>(())
    });
}

// ---------------------------------------------------------------------------
// Live binding for Android end-to-end clipboard flow.
//
// Behaviour:
//   * Feature `android-uniffi-live` ON  → open DB at `db_path`, encrypt the
//     text via `copypaste_core::encrypt_item`, build a `ClipboardItem`, and
//     persist via `copypaste_core::insert_item`. Returns the new row id, or
//     an empty string if the text was flagged as sensitive.
//   * Feature OFF (default)            → no DB I/O. Validates the key shape
//     and returns an empty string so Kotlin callers treat it as "not stored
//     natively" and fall through to the SharedPreferences repository.
//
// `key` must be the 32-byte device key (derived from Android Keystore by the
// caller; that derivation lives in Kotlin).
// ---------------------------------------------------------------------------

/// Process-scoped monotonic Lamport counter shared by the two capture paths
/// (`add_clipboard_item` and `store_clipboard_item`).
///
/// Using `SystemTime::now()` here produced timestamps ≈1.7×10^12 that the macOS
/// daemon's `MAX_LAMPORT_SKEW` clamp would eventually reject, and — more critically
/// — mixing wall-clock numbers with the daemon's small logical counter (starting at
/// 1 and ticking once per write) broke LWW ordering: Android items appeared causally
/// far ahead of every Mac item, so Mac writes could never win conflicts.
///
/// This function is the legacy UniFFI capture path (the primary Kotlin path goes
/// through ClipboardRepository.storeItem which manages the persistent LamportClock
/// in SharedPreferences). A per-process atomic gives correct logical ordering within
/// this process; cross-session ordering is maintained by the daemon's `observe()` on
/// ingest.
#[cfg(feature = "android-uniffi-live")]
fn next_android_lamport_ts() -> i64 {
    static LAMPORT_COUNTER: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1);
    LAMPORT_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// Internal shared implementation for `add_clipboard_item` and
/// `store_clipboard_item` (the new PG-3/349q path with explicit TTL).
///
/// `sensitive_ttl_secs`:
///   - `None`  → use the default (`SENSITIVE_TTL_SECS = 30 s`). This preserves
///     the pre-existing behaviour of `add_clipboard_item` callers that
///     have not yet been updated.
///   - `Some(0)` → auto-wipe disabled (no `expires_at`).
///   - `Some(n)` → stamp `expires_at = now_ms + n * 1000`.
#[cfg(feature = "android-uniffi-live")]
fn store_clipboard_item_inner(
    db_path: &str,
    key_arr: &Zeroizing<[u8; 32]>,
    text: String,
    sensitive_ttl_secs: Option<u64>,
) -> Result<String, CopypasteError> {
    // PG-3 (349q): determine sensitivity using the SAME gate as macOS daemon
    // (`is_sensitive_for_autowipe`, confidence >= 0.70). Do NOT use `detect()`
    // which fires on any pattern — that was the old bug that misaligned the
    // threshold with macOS and caused sensitive items to be dropped instead of
    // stored+marked.
    let is_sensitive = is_sensitive_for_autowipe(&text);

    // v0.3: pre-generate item_id so the AAD baked into the ciphertext matches
    // the value persisted in the row.
    //
    // IMPORTANT: use build_item_aad_v2(item_id, AAD_SCHEMA_VERSION_V4, 2) —
    // NOT the 2-arg build_item_aad(…, AAD_SCHEMA_VERSION=3). The item is
    // stamped with key_version=ITEM_KEY_VERSION_CURRENT=2 by ClipboardItem::new_text,
    // and the daemon decrypts key_version=2 rows with AAD "{item_id}|4|2"
    // (build_item_aad_v2). Using the 2-arg form ("{item_id}|3") causes an
    // auth-tag mismatch and makes every FFI-inserted item undecryptable on
    // the daemon side.
    let item_id = uuid::Uuid::new_v4().to_string();
    let aad = build_item_aad_v2(
        &item_id,
        AAD_SCHEMA_VERSION_V4,
        ITEM_KEY_VERSION_CURRENT as u32,
    );
    let (nonce, ciphertext) = encrypt_item_with_aad(text.as_bytes(), key_arr, &aad)
        .map_err(|_| CopypasteError::EncryptionFailed)?;

    let lamport_ts = next_android_lamport_ts();

    let mut item = copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), lamport_ts);
    item.item_id = item_id;

    // PG-3 (349q): stamp is_sensitive so the DB row, sync, and UI all agree.
    // macOS daemon.rs:2170 does the same: `item.is_sensitive = is_sensitive`.
    item.is_sensitive = is_sensitive;

    // PG-24 (5tnx): stamp expires_at for sensitive items, matching
    // daemon.rs:2183: `item.expires_at = Some(now_ms + ttl_secs * 1000)`.
    if is_sensitive {
        let ttl = sensitive_ttl_secs.unwrap_or(copypaste_core::config::SENSITIVE_TTL_SECS);
        if ttl > 0 {
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as i64)
                // Defensive: system clock behind epoch (impossible in practice).
                .unwrap_or(0);
            item.expires_at = Some(now_ms.saturating_add(ttl as i64 * 1000));
        }
        // ttl == 0 → "auto-wipe disabled" sentinel → leave expires_at = None.
    }

    let id = item.id.clone();

    // M5: reuse a cached connection instead of open-per-call.
    with_cached_db(db_path, key_arr, |db| {
        copypaste_core::insert_item(db, &item).map_err(|e| CopypasteError::DatabaseError {
            reason: e.to_string(),
        })
    })?;

    Ok(id)
}

/// Store a clipboard text item. Sensitive items are stored with
/// `is_sensitive = true` and `expires_at` stamped from `SENSITIVE_TTL_SECS`
/// (default 30 s). Returns the new row UUID.
///
/// PG-3 (349q): items are NO LONGER dropped on sensitivity — they are stored
/// encrypted with the `is_sensitive` flag set so the daemon, sync, and UI can
/// all handle them correctly. Use `store_clipboard_item` for the primary path
/// (it accepts an explicit `sensitive_ttl_secs`).
///
/// # GRADLE-REQUIRED
/// Kotlin's ClipboardService.kt must remove the early-return at lines 892-896
/// (text), 993 (image), 1169 (file) that dropped sensitive items before calling
/// this FFI, and instead call this function for ALL captures. The Rust side now
/// makes the store-or-drop decision.
#[cfg(feature = "android-uniffi-live")]
pub fn add_clipboard_item(
    db_path: String,
    key: &[u8],
    text: String,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        // Use None → falls back to SENSITIVE_TTL_SECS default (30 s).
        store_clipboard_item_inner(&db_path, &key_arr, text, None)
    })
}

/// Store a clipboard text item with an EXPLICIT sensitive auto-wipe TTL.
///
/// Primary capture path for PG-3/349q-compliant Kotlin code. Sensitive items
/// are stored encrypted with `is_sensitive = true` and `expires_at` stamped
/// from `sensitive_ttl_secs` (pass 0 to disable auto-wipe for this item).
/// Returns the new row UUID.
///
/// `sensitive_ttl_secs` should be `Config.sensitive_ttl_secs` from the current
/// user config (call `default_config()` for the app default).
///
/// # GRADLE-REQUIRED
/// ClipboardService.kt must:
///   1. Replace `add_clipboard_item` calls with `store_clipboard_item` passing
///      the user's configured TTL.
///   2. Remove the sensitive early-returns (892-896, 993, 1169) that previously
///      dropped sensitive content — the Rust side now stores+marks it.
///   3. On the Kotlin side, check `is_sensitive` from `sensitive_capture_decision`
///      to decide whether to show a masked preview in the notification.
#[cfg(feature = "android-uniffi-live")]
pub fn store_clipboard_item(
    db_path: String,
    key: &[u8],
    text: String,
    sensitive_ttl_secs: u64,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        store_clipboard_item_inner(&db_path, &key_arr, text, Some(sensitive_ttl_secs))
    })
}

#[cfg(not(feature = "android-uniffi-live"))]
pub fn add_clipboard_item(
    _db_path: String,
    key: &[u8],
    _text: String,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Validate key shape to mirror the live path's error surface.
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        // Return empty string so Kotlin callers treat this as "not stored
        // natively" and fall through to the SharedPreferences repository.
        // Previously returned "stub-uniffi-not-live" which was non-empty and
        // caused ClipboardService to skip the fallback store entirely (items lost).
        Ok(String::new())
    })
}

/// Stub `store_clipboard_item` (feature `android-uniffi-live` off): validates
/// the key shape and returns an empty string so Kotlin falls through to the
/// SharedPreferences repository. No DB I/O in stub mode.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn store_clipboard_item(
    _db_path: String,
    key: &[u8],
    _text: String,
    _sensitive_ttl_secs: u64,
) -> Result<String, CopypasteError> {
    panic_boundary::catch_result(|| {
        // Mirror the live path's key-shape error surface.
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(String::new())
    })
}

#[cfg(feature = "android-uniffi-live")]
pub fn get_history_count(db_path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        // M5: reuse a cached connection instead of open-per-call.
        let n = with_cached_db(&db_path, &key_arr, |db| {
            copypaste_core::count_items(db).map_err(|e| CopypasteError::DatabaseError {
                reason: e.to_string(),
            })
        })?;
        Ok(n.max(0) as u64)
    })
}

#[cfg(not(feature = "android-uniffi-live"))]
pub fn get_history_count(_db_path: String, key: &[u8]) -> Result<u64, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(0)
    })
}

// ---------------------------------------------------------------------------
// PG-17 (mxoq): FTS5 search over Android FFI
//
// Android ClipboardRepository.searchIds previously did a full-content O(N)
// decrypt scan. The daemon uses `search_items` (FTS5 indexed, O(log N)), so
// the two code paths produced different recall AND incurred vastly different
// CPU cost on large histories.
//
// This export delegates to `copypaste_core::search_items` — the SAME
// FTS5-backed function the daemon's `search` IPC handler uses. The FTS index
// (`clipboard_fts`) lives inside the encrypted SQLCipher database and is
// maintained by `upsert_fts` / `delete_fts` alongside every item write.
//
// SECURITY: only `item_id`, `content_type`, `lamport_ts`, `wall_time`, and
// `is_sensitive` are returned across the FFI boundary — no plaintext content,
// no nonces, no key material. The FTS plaintext index remains inside core /
// SQLCipher and never crosses the FFI.
//
// GRADLE-REQUIRED: the Kotlin call-site swap in ClipboardRepository (replacing
// the O(N) decrypt loop with `fts_search`) requires the Android Gradle build.
// ---------------------------------------------------------------------------

/// One item returned by [`fts_search`].
///
/// Only metadata fields are returned — no plaintext content, no nonces.
/// The FTS index stays inside core/SQLCipher. Kotlin uses `item_id` to look
/// up the stored ciphertext row it already holds locally for rendering or
/// decryption.
#[derive(Debug)]
pub struct SearchResultItem {
    /// The stable cross-device identity (maps to Kotlin's `item_id` column).
    pub item_id: String,
    /// One of "text", "image", "file".
    pub content_type: String,
    /// Lamport clock value — use for causal ordering if needed.
    pub lamport_ts: i64,
    /// Wall-clock capture time in milliseconds since Unix epoch.
    pub wall_time_ms: i64,
    /// Whether this item was flagged as sensitive at capture time.
    pub is_sensitive: bool,
}

/// Search the local FTS5 index for items matching `query`.
///
/// Delegates to `copypaste_core::search_items` — the SAME FTS5 search used by
/// the daemon's `search` IPC handler. Returns up to `limit` results ordered by
/// FTS5 rank (best match first). Returns an empty list when `query` is blank or
/// contains no valid tokens after sanitization.
///
/// # SECURITY
/// Only metadata fields are returned across the FFI boundary; the FTS index
/// and item content remain inside the encrypted SQLCipher database.
///
/// # GRADLE-REQUIRED
/// The Kotlin call-site swap in ClipboardRepository that replaces the O(N)
/// decrypt scan with this function requires the Android Gradle build.
#[cfg(feature = "android-uniffi-live")]
pub fn fts_search(
    db_path: String,
    key: &[u8],
    query: String,
    limit: u32,
) -> Result<Vec<SearchResultItem>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            let items = copypaste_core::search_items(db, &query, limit as usize).map_err(|e| {
                CopypasteError::DatabaseError {
                    reason: e.to_string(),
                }
            })?;
            Ok(items
                .into_iter()
                .map(|it| SearchResultItem {
                    item_id: it.item_id,
                    content_type: it.content_type,
                    lamport_ts: it.lamport_ts,
                    wall_time_ms: it.wall_time,
                    is_sensitive: it.is_sensitive,
                })
                .collect())
        })
    })
}

/// Stub (feature off): validates the key shape, then returns an empty list.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn fts_search(
    _db_path: String,
    key: &[u8],
    _query: String,
    _limit: u32,
) -> Result<Vec<SearchResultItem>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(Vec::new())
    })
}

// ---------------------------------------------------------------------------
// PG-19 (o0t3): Lamport-ordered history page for Android
//
// Android ClipboardRepository sorted the unpinned history by `wallTimeMs`.
// The correct ordering is `lamport_ts DESC` (with wall_time and origin_device_id
// as deterministic tie-breaks) because the Lamport clock advances monotonically
// on every write and sync — it correctly reflects causal ordering after
// cross-device sync, whereas wall-clock ordering can diverge when device clocks
// differ.
//
// This export delegates to `copypaste_core::get_page_pinned_first_lamport`
// which applies the CRDT-correct ordering: pinned items first (by pin_order),
// then unpinned items by `lamport_ts DESC, wall_time DESC, origin_device_id ASC`.
//
// The lamport_ts value is also returned in `HistoryItem` so Kotlin can validate
// or further sort client-side if needed.
//
// GRADLE-REQUIRED: the Kotlin call-site swap in ClipboardRepository (replacing
// the wall-time ORDER BY with a call to this FFI function) requires the Android
// Gradle build.
// ---------------------------------------------------------------------------

/// One item in the history page returned by [`get_history_page`].
///
/// Only metadata fields are returned — no plaintext content, no nonces, no
/// key material. Kotlin uses `item_id` to look up the stored ciphertext row
/// it already holds for rendering or decryption.
#[derive(Debug)]
pub struct HistoryItem {
    /// The stable cross-device identity (maps to Kotlin's `item_id` column).
    pub item_id: String,
    /// One of "text", "image", "file".
    pub content_type: String,
    /// Lamport clock value. Primary sort key for unpinned items (descending).
    /// Exposed here so Kotlin can verify or further sort if needed.
    pub lamport_ts: i64,
    /// Wall-clock capture time in milliseconds since Unix epoch.
    pub wall_time_ms: i64,
    /// Whether this item was flagged as sensitive at capture time.
    pub is_sensitive: bool,
    /// Whether this item is pinned.
    pub pinned: bool,
    /// Explicit pin sort order for pinned items; `None` for unpinned items.
    pub pin_order: Option<f64>,
}

/// Return a page of clipboard history ordered by:
///   1. Pinned items first, sorted by `pin_order ASC` (then `pin_order IS NULL`).
///   2. Unpinned items sorted by `lamport_ts DESC, wall_time DESC, origin_device_id ASC`.
///
/// This is the CRDT-correct ordering for cross-device sync: the Lamport clock
/// advances on every write/merge so post-sync ordering matches causal history
/// rather than wall-clock skew between devices.
///
/// `offset` / `limit` work identically to `get_page`. The caller should
/// mirror the daemon's `MAX_PAGE` cap (typically 200) when choosing `limit`.
///
/// # GRADLE-REQUIRED
/// The Kotlin call-site swap in ClipboardRepository that replaces the wall-time
/// ORDER BY with this function requires the Android Gradle build.
#[cfg(feature = "android-uniffi-live")]
pub fn get_history_page(
    db_path: String,
    key: &[u8],
    limit: u32,
    offset: u32,
) -> Result<Vec<HistoryItem>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        with_cached_db(&db_path, &key_arr, |db| {
            let items =
                copypaste_core::get_page_pinned_first_lamport(db, limit as usize, offset as usize)
                    .map_err(|e| CopypasteError::DatabaseError {
                        reason: e.to_string(),
                    })?;
            Ok(items
                .into_iter()
                .map(|it| HistoryItem {
                    item_id: it.item_id,
                    content_type: it.content_type,
                    lamport_ts: it.lamport_ts,
                    wall_time_ms: it.wall_time,
                    is_sensitive: it.is_sensitive,
                    pinned: it.pinned,
                    pin_order: it.pin_order,
                })
                .collect())
        })
    })
}

/// Stub (feature off): validates the key shape, then returns an empty list.
#[cfg(not(feature = "android-uniffi-live"))]
pub fn get_history_page(
    _db_path: String,
    key: &[u8],
    _limit: u32,
    _offset: u32,
) -> Result<Vec<HistoryItem>, CopypasteError> {
    panic_boundary::catch_result(|| {
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(Vec::new())
    })
}

// ---------------------------------------------------------------------------
// CopyPaste-bdac.42: DB vacuum — macOS parity (Settings → Storage → Compact)
//
// Runs `PRAGMA incremental_vacuum(0)` to reclaim ALL free pages in the
// SQLCipher database (bounded incremental, WAL-safe, equivalent to a full
// VACUUM but without the blocking full-table rebuild). Mirrors the macOS
// daemon's `ni` IPC verb (`METHOD_VACUUM`) which the macOS UI triggers on
// Settings → Storage → Compact. The `0` budget tells SQLite to reclaim every
// free page in one call — appropriate for an explicit user-initiated action
// (as opposed to the periodic incremental sweep with a small page budget that
// runs after every TTL cleanup in the daemon).
//
// The Android UI wires this via `onVacuumDatabase` in StorageTab so the user
// can trigger the same compaction as on macOS. The operation is safe to run
// at any time and is a no-op on databases that do not use `auto_vacuum = INCREMENTAL`
// (pre-migration DBs), so calling it on an older database never causes errors.
// ---------------------------------------------------------------------------

/// Run `PRAGMA incremental_vacuum(0)` on the encrypted SQLCipher database at
/// `db_path` to reclaim ALL free pages (WAL-safe, mirrors the macOS `ni` IPC
/// verb). Returns `Ok(())` on success. With `android-uniffi-live` feature off,
/// validates the key shape and returns `Ok(())` (no-op stub).
#[cfg(feature = "android-uniffi-live")]
pub fn db_vacuum(db_path: String, key: &[u8]) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let key_arr: Zeroizing<[u8; 32]> = Zeroizing::new(
            key.try_into()
                .map_err(|_| CopypasteError::InvalidKeyLength)?,
        );
        // Use with_cached_db to reuse the existing connection; vacuum is safe
        // on a cached connection (it runs PRAGMA incremental_vacuum(0) in-place).
        with_cached_db(&db_path, &key_arr, |db| {
            copypaste_core::incremental_vacuum(db, 0).map_err(|e| CopypasteError::DatabaseError {
                reason: e.to_string(),
            })
        })
    })
}

/// Stub (feature off): validates the key shape and returns `Ok(())` (no-op).
#[cfg(not(feature = "android-uniffi-live"))]
pub fn db_vacuum(db_path: String, key: &[u8]) -> Result<(), CopypasteError> {
    panic_boundary::catch_result(|| {
        let _ = db_path;
        let _: [u8; 32] = key
            .try_into()
            .map_err(|_| CopypasteError::InvalidKeyLength)?;
        Ok(())
    })
}
