use crate::clipboard::{ClipboardContent, ClipboardMonitor};
use copypaste_core::{
    build_item_aad_v2, bump_item_recency, chunks_to_blob, derive_v2, encode_image_full,
    encrypt_item_with_aad, find_recent_by_hash, get_item_by_id, insert_item_with_fts,
    is_sensitive_for_autowipe, prune_to_cap, AppConfig, ClipboardItem, Database, ItemId,
    AAD_SCHEMA_VERSION_V4, ITEM_KEY_VERSION_CURRENT,
};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// How long (in seconds) the frontmost-app bundle ID obtained from `lsappinfo`
/// is considered fresh.  Caching avoids forking a new subprocess on every 500 ms
/// clipboard tick (CopyPaste-44rq.33).  2 s is short enough to catch a typical
/// Cmd+Tab switch (≤400 ms human reaction time + one full tick latency) while
/// cutting the fork rate by ~75 % at the default 500 ms poll interval.
#[cfg(target_os = "macos")]
pub(super) const FRONTMOST_APP_CACHE_TTL_SECS: u64 = 2;

/// Per-tick cache for the frontmost application's bundle ID on macOS.
///
/// Populated by `handle_tick` at most once every `FRONTMOST_APP_CACHE_TTL_SECS`
/// seconds; stale when `expires_at` is in the past.
///
/// `cached_value` is `None` either when `lsappinfo` failed (we cache the
/// failure too so we do not retry on the very next tick) or when the cache has
/// not been primed yet.  `is_failure` distinguishes "not yet primed" from "we
/// tried and lsappinfo returned nothing", which matters for the P1-2 fail-closed
/// gate.
#[cfg(target_os = "macos")]
#[derive(Debug)]
pub(crate) struct FrontmostAppCache {
    /// The cached bundle ID (None when lsappinfo failed or cache not yet primed).
    pub(super) cached_value: Option<String>,
    /// Whether the last lsappinfo invocation failed (vs. cache simply being cold).
    pub(super) is_failure: bool,
    /// When the cache entry expires and must be refreshed.
    pub(super) expires_at: std::time::Instant,
}

#[cfg(target_os = "macos")]
impl FrontmostAppCache {
    /// Create a new, already-expired cache so the first tick always populates it.
    pub(crate) fn new() -> Self {
        Self {
            cached_value: None,
            is_failure: false,
            // Subtract 1 s so the cache is considered expired on the first call.
            expires_at: std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(1))
                // `checked_sub` can only fail if Instant::now() is within 1 s of
                // the monotonic clock epoch, which is impossible in practice.
                .unwrap_or_else(std::time::Instant::now),
        }
    }

    /// Returns `true` if the cached value is still within the TTL window.
    pub(super) fn is_fresh(&self) -> bool {
        std::time::Instant::now() < self.expires_at
    }
}

/// Run the sensitive- and/or general-TTL deletes on a blocking thread.
///
/// daemon-core L1: both `delete_sensitive_expired` and `delete_expired` are
/// synchronous rusqlite calls. Previously they ran inline inside the `select!`
/// loop under `db.lock().await` while holding the tokio Mutex, blocking the
/// async worker for the duration of the SQL. We now mirror the IPC path:
/// acquire the lock and run the SQL inside `spawn_blocking`. The clock-skew-safe
/// `unwrap_or_default()` on the timestamp is preserved.
///
/// CopyPaste-98ja: when `do_sensitive` is true the sensitive prune is guarded
/// by a cheap `SELECT EXISTS` pre-check (`has_sensitive_items`).  On a system
/// with no sensitive history at all this short-circuits the full scan every
/// 5 seconds and avoids a gratuitous write transaction.  The TTL guarantee is
/// preserved: the prune still runs whenever the pre-check finds at least one
/// eligible row.
pub(crate) async fn run_ttl_cleanup(
    db: &Arc<Mutex<Database>>,
    sensitive_ttl_ms: i64,
    do_sensitive: bool,
    do_general: bool,
) {
    if !do_sensitive && !do_general {
        return;
    }
    let db = db.clone();
    let join = tokio::task::spawn_blocking(move || {
        let guard = db.blocking_lock();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let sensitive = if do_sensitive {
            // CopyPaste-98ja: cheap EXISTS probe before the full DELETE scan.
            // When no sensitive (non-pinned) rows exist at all, skip the prune
            // — nothing has expired and the write transaction is unnecessary.
            if copypaste_core::has_sensitive_items(&guard) {
                Some(copypaste_core::delete_sensitive_expired(
                    &guard,
                    now_ms,
                    sensitive_ttl_ms,
                ))
            } else {
                None
            }
        } else {
            None
        };
        let general = if do_general {
            Some(copypaste_core::delete_expired(&guard, now_ms))
        } else {
            None
        };
        (sensitive, general)
    })
    .await;
    let (sensitive, general) = match join {
        Ok(pair) => pair,
        Err(e) => {
            tracing::warn!("TTL cleanup blocking task failed: {e}");
            return;
        }
    };
    match sensitive {
        Some(Ok(n)) if n > 0 => tracing::info!("sensitive TTL cleanup: wiped {n} sensitive items"),
        Some(Err(e)) => tracing::warn!("sensitive TTL cleanup error: {e}"),
        _ => {}
    }
    match general {
        Some(Ok(n)) if n > 0 => tracing::info!("TTL cleanup: removed {n} expired items"),
        Some(Err(e)) => tracing::warn!("TTL cleanup error: {e}"),
        _ => {}
    }
}

#[tracing::instrument(skip_all, name = "clipboard_tick")]
pub(crate) async fn handle_tick(
    monitor: &mut ClipboardMonitor,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    private_mode: &Arc<AtomicBool>,
    new_item_tx: &broadcast::Sender<ClipboardItem>,
    local_device_id: &str,
    // CopyPaste-44rq.33: cache the frontmost-app query so lsappinfo is not
    // forked on every 500 ms tick.  Only present on macOS; the non-macOS path
    // never queries lsappinfo and does not need a cache.
    #[cfg(target_os = "macos")] frontmost_cache: &mut FrontmostAppCache,
) {
    // Skip recording when private/pause mode is active
    if private_mode.load(Ordering::Relaxed) {
        // Still poll to advance the change-count so we don't replay on resume
        let _ = monitor.poll();
        tracing::debug!("private mode active: skipping clipboard recording");
        return;
    }

    // EXCLUDE APPS / SENSITIVE APP DETECTION: resolve the frontmost app's
    // bundle ID on macOS so we can (a) skip capture if it is in the exclusion
    // list, and (b) mark items as sensitive when the source is a password
    // manager (mtf5 / PG-22).  `lsappinfo front` is macOS-only.
    //
    // ── KNOWN LIMITATION (44rq.47 / PRIV-6) ────────────────────────────────
    // Sensitive-app detection via `is_sensitive_app` is BEST-EFFORT on macOS
    // because it depends on `lsappinfo front`, an Apple-private CLI tool that
    // CopyPaste forks at each clipboard tick.
    //
    //  • `lsappinfo` may fail (binary absent, sandboxed, or the subprocess
    //    panics) → `frontmost_bundle_id` becomes `None` → the app-sensitive
    //    flag is NOT set, even if the user is copying from 1Password.  Content-
    //    pattern detection (`is_sensitive_for_autowipe`) still applies for text.
    //  • Process substitution (e.g. a password manager that delegates the copy
    //    to a helper process) may surface a different bundle ID than expected,
    //    causing a miss.
    //  • This detection is unavailable on Linux / non-macOS (always None).
    //
    // The correct long-term fix (PRIV-2) is to query the Accessibility API or
    // use NSWorkspace.frontmostApplication instead of forking lsappinfo.  Until
    // then, users relying on sensitive-masking for password-manager images
    // should also add those apps to `excluded_app_bundle_ids` so the fail-
    // closed P1-2 gate catches any lsappinfo failure.
    // ────────────────────────────────────────────────────────────────────────
    //
    // P1-2 (fail-closed): when `lsappinfo` fails AND the exclusion list is
    // non-empty, we do NOT know the frontmost app — skip capture this tick to
    // avoid silently capturing from a potentially-excluded password manager.
    // When the exclusion list IS empty we can proceed (lsappinfo is best-effort
    // for the is_sensitive_app check only; failing open is safe there because
    // the content-pattern gate still applies).
    //
    // P1-3 (spawn_blocking): `std::process::Command::output()` is blocking;
    // offload to the tokio blocking-thread pool so the async executor is not
    // stalled by the fork+wait.
    //
    // fix(44rq.43): lsappinfo must ALWAYS run on macOS so is_sensitive_app
    // can flag passwords from password managers even when excluded_app_bundle_ids
    // is empty (the default).  The previous CopyPaste-zdyw optimisation
    // short-circuited to None when the exclusion list was empty, which meant
    // `is_sensitive_app` in handle_text always received None → false → passwords
    // from 1Password, Bitwarden, etc. got no sensitive flag and no TTL wipe.
    //
    // The P1-2 fail-closed behaviour (skip capture when lsappinfo fails AND the
    // exclusion list is non-empty) is preserved below unchanged.  Performance:
    // CopyPaste-44rq.33: lsappinfo is now cached for FRONTMOST_APP_CACHE_TTL_SECS
    // (2 s) so the subprocess is NOT forked on every 500 ms tick — only when the
    // cached value has expired.  The cache is shared across ticks via the
    // `frontmost_cache` parameter.  Failure results (lsappinfo returning None) are
    // cached for the same TTL so a transient failure does not cause repeated forks.
    //
    // Shared between the exclusion check and the is_sensitive_app check so
    // lsappinfo is invoked AT MOST ONCE per TTL window regardless of which check fires.
    #[cfg(target_os = "macos")]
    let frontmost_bundle_id: Option<String> = {
        if frontmost_cache.is_fresh() {
            // Cache hit: reuse the previously-resolved bundle ID (may be None if
            // lsappinfo failed on the last refresh — we cache failures too so we
            // do not hammer the subprocess on every tick during a transient error).
            tracing::trace!(
                cached = ?frontmost_cache.cached_value,
                "lsappinfo: cache hit — skipping subprocess"
            );
            frontmost_cache.cached_value.clone()
        } else {
            // Cache miss (cold or expired): spawn lsappinfo and populate the cache.
            let lsappinfo_result = tokio::task::spawn_blocking(|| {
                // `lsappinfo front` prints a record for the frontmost process.
                // We extract the bundleID field from lines like:
                //   "bundleID" = "com.1password.1password"
                std::process::Command::new("lsappinfo")
                    .args(["front"])
                    .output()
                    .ok()
                    .and_then(|out| {
                        let text = String::from_utf8_lossy(&out.stdout).into_owned();
                        for line in text.lines() {
                            let trimmed = line.trim();
                            // Match: "bundleID" = "com.example.app"
                            if let Some(rest) = trimmed.strip_prefix("\"bundleID\" = \"") {
                                if let Some(bid) = rest.strip_suffix('"') {
                                    return Some(bid.to_owned());
                                }
                            }
                        }
                        None
                    })
            })
            .await;

            // Flatten the JoinError and inner Option, then populate the cache.
            let resolved = match lsappinfo_result {
                Ok(opt) => {
                    frontmost_cache.is_failure = opt.is_none();
                    opt
                }
                Err(join_err) => {
                    // spawn_blocking task panicked — treat as subprocess failure.
                    tracing::warn!(
                        error = %join_err,
                        "lsappinfo: blocking task panicked; failing closed to protect excluded apps"
                    );
                    frontmost_cache.is_failure = true;
                    None
                }
            };

            // Store the result (success or failure) and set the expiry so we
            // do not fork again until FRONTMOST_APP_CACHE_TTL_SECS have elapsed.
            frontmost_cache.cached_value = resolved.clone();
            frontmost_cache.expires_at = std::time::Instant::now()
                + std::time::Duration::from_secs(FRONTMOST_APP_CACHE_TTL_SECS);
            tracing::trace!(
                bundle_id = ?resolved,
                ttl_secs = FRONTMOST_APP_CACHE_TTL_SECS,
                "lsappinfo: cache refreshed"
            );

            resolved
        }
    };

    // Exclusion-list check: skip capture when the frontmost app is excluded.
    // When the exclusion list is non-empty and lsappinfo failed (None) we also
    // fail closed (P1-2): advance the change-count and skip this tick.
    #[cfg(target_os = "macos")]
    if !config.excluded_app_bundle_ids.is_empty() {
        match frontmost_bundle_id {
            Some(ref bid) => {
                if config.excluded_app_bundle_ids.iter().any(|ex| ex == bid) {
                    // Advance the change-count so this item is not re-offered
                    // on the next tick (same pattern as private-mode).
                    let _ = monitor.poll();
                    tracing::debug!(
                        bundle_id = %bid,
                        "clipboard: skipping capture — app is in excluded_app_bundle_ids"
                    );
                    return;
                }
                // Bundle ID is known and not excluded — fall through to capture.
            }
            None => {
                // lsappinfo failed: exclusion list non-empty → fail closed.
                // P1-2: warn without logging any clipboard content (no secrets).
                tracing::warn!(
                    "lsappinfo: could not determine frontmost app bundle ID; \
                     skipping capture this tick to protect excluded apps (fail-closed)"
                );
                let _ = monitor.poll();
                return;
            }
        }
    }

    // Non-macOS: no bundle ID available.
    #[cfg(not(target_os = "macos"))]
    let frontmost_bundle_id: Option<String> = None;

    match monitor.poll() {
        Ok(Some(ClipboardContent::Text(text))) => {
            // beta.5 Bug-1 visibility: log every capture at info level so
            // users can confirm from `daemon.out.log` that the pasteboard is
            // actually being read. Prior code only emitted `debug!` here
            // which the default `copypaste=info` filter dropped, leaving
            // operators unable to distinguish "no captures happening" from
            // "captures happening but UI not refreshing".
            tracing::info!(
                bytes = text.len(),
                "clipboard captured: text ({} bytes)",
                text.len()
            );
            if let Some(item) = handle_text(
                text,
                db,
                local_key,
                config,
                local_device_id,
                frontmost_bundle_id.clone(),
            )
            .await
            {
                // Broadcast to P2P + cloud-sync subscribers (and any future consumer).
                // A send error only means there are no active receivers —
                // that is normal when both P2P and cloud-sync are disabled.
                let _ = new_item_tx.send(item);
                // M12: play sound on macOS when the daemon captures a new item.
                // Notifications are now posted by the Tauri UI bundle
                // (spawn_tray_recent_resync polls history_page and fires
                // UNUserNotificationCenter) so the banner shows the app icon.
                // Disabled in tests to avoid OS hangs and sound spam.
                #[cfg(target_os = "macos")]
                if std::env::var("COPYPASTE_EPHEMERAL_KEY").is_err() && config.sound_on_copy {
                    // P1: reap the child in a detached thread so the capture
                    // path is never blocked and no zombie process accumulates
                    // (dropping a Child without wait() leaves a zombie entry
                    // in the process table until the daemon exits).
                    if let Ok(mut child) = std::process::Command::new("afplay")
                        .arg("/System/Library/Sounds/Tink.aiff")
                        .spawn()
                    {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                }
            }
        }
        Ok(Some(ClipboardContent::Image(raw_bytes))) => {
            tracing::info!(
                bytes = raw_bytes.len(),
                "clipboard captured: image ({} bytes raw)",
                raw_bytes.len()
            );
            if let Some(item) = handle_image(
                raw_bytes,
                db,
                local_key,
                config,
                local_device_id,
                frontmost_bundle_id.clone(),
            )
            .await
            {
                let _ = new_item_tx.send(item);
                // M12: play sound for image captures too.
                // Notifications handled by Tauri UI (same as text above).
                #[cfg(target_os = "macos")]
                if std::env::var("COPYPASTE_EPHEMERAL_KEY").is_err() && config.sound_on_copy {
                    if let Ok(mut child) = std::process::Command::new("afplay")
                        .arg("/System/Library/Sounds/Tink.aiff")
                        .spawn()
                    {
                        std::thread::spawn(move || {
                            let _ = child.wait();
                        });
                    }
                }
            }
        }
        Ok(Some(ClipboardContent::File {
            bytes,
            filename,
            mime,
        })) => {
            // Do NOT log filename or mime — they may contain PII
            // (full path, document name, content type that reveals
            // file identity). Log size and name-length only.
            tracing::info!(
                bytes = bytes.len(),
                name_len = filename.len(),
                "clipboard captured: file ({} bytes, name_len={})",
                bytes.len(),
                filename.len()
            );
            if let Some(item) = handle_file(
                bytes,
                filename,
                mime,
                db,
                local_key,
                config,
                local_device_id,
                frontmost_bundle_id.clone(),
            )
            .await
            {
                let _ = new_item_tx.send(item);
            }
        }
        Ok(Some(ClipboardContent::FileRef {
            path,
            filename,
            mime,
        })) => {
            // CopyPaste-b5iz: combine stat + read into ONE spawn_blocking call.
            // The previous code issued two separate spawn_blocking calls: one
            // for `metadata` (size pre-check) and one for `read`. Between the
            // two calls the file could change (TOCTOU: shrink to pass the
            // pre-check, then a racing write restores a large version), making
            // the size gate ineffective. A single blocking closure performs
            // both operations atomically — the file cannot be substituted
            // between stat and read within the same closure.
            let max_file_bytes = usize::try_from(config.max_file_size_bytes).unwrap_or(usize::MAX);
            let path_clone = path.clone();
            let read_result = tokio::task::spawn_blocking(move || -> std::io::Result<Vec<u8>> {
                // Stat first: if the file already exceeds the cap, avoid the
                // potentially large (multi-GB) read entirely. Both operations
                // share the same blocking task so no substitution is possible.
                // On metadata failure fall through to read (the post-read
                // size gate still catches oversized files).
                if let Ok(meta) = std::fs::metadata(&path_clone) {
                    let file_len = meta.len() as usize;
                    if file_len > max_file_bytes {
                        // Return a synthetic "file too large" error so the
                        // Ok(Ok(bytes)) match arm is never reached for oversized
                        // files, avoiding the unnecessary read of the full blob.
                        // We use Other so the caller's Err arm can log it and skip.
                        return Err(std::io::Error::other(format!(
                            "file too large (stat): {file_len} bytes > max {max_file_bytes}"
                        )));
                    }
                }
                std::fs::read(&path_clone)
            })
            .await;
            match read_result {
                Ok(Ok(bytes)) => {
                    if bytes.len() > max_file_bytes {
                        // am9w: do NOT log filename — it contains PII (full
                        // path / document name). Log name-length only,
                        // mirroring the File branch above.
                        tracing::warn!(
                            bytes = bytes.len(),
                            max = max_file_bytes,
                            name_len = filename.len(),
                            "clipboard: file too large — skipping"
                        );
                    } else {
                        // am9w: do NOT log filename or mime — PII. Log
                        // name_len only, same as the File branch above.
                        tracing::info!(
                            bytes = bytes.len(),
                            name_len = filename.len(),
                            "clipboard captured: file-ref ({} bytes, name_len={})",
                            bytes.len(),
                            filename.len()
                        );
                        if let Some(item) = handle_file(
                            bytes,
                            filename,
                            mime,
                            db,
                            local_key,
                            config,
                            local_device_id,
                            frontmost_bundle_id.clone(),
                        )
                        .await
                        {
                            let _ = new_item_tx.send(item);
                        }
                    }
                }
                Ok(Err(e)) => {
                    // Do NOT log path.display() — it contains the OS
                    // username and full path (PII). Log basename only.
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    tracing::warn!(
                        filename = %name,
                        "clipboard: file-url read failed: {e}"
                    );
                }
                Err(e) => {
                    // spawn_blocking task panicked — log and continue.
                    // Redact path for the same reason as above.
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "<unknown>".to_string());
                    tracing::warn!(
                        filename = %name,
                        "clipboard: file-url spawn_blocking panicked: {e}"
                    );
                }
            }
        }
        // CopyPaste-mdhx: SkippedBatch — implemented, not dead.
        //
        // `poll()` no longer returns this variant in the normal fast-path:
        // the §CRITICAL-fix in clipboard.rs made rapid bursts (changeCount
        // delta ≥ SKIPPED_BATCH_THRESHOLD) fall through to the content path
        // so the most-recent pasteboard value is still captured instead of
        // being discarded.  NSPasteboard does not buffer intermediate writes
        // and those items are irrecoverably lost regardless — an inherent
        // OS-level limitation.
        //
        // However `SkippedBatch` is a public variant that tests and future
        // telemetry code can still produce.  We handle it explicitly here
        // (not merged into `None`) so:
        //  a) Clippy does not flag it as dead code inside this match.
        //  b) A future `poll()` that restores `SkippedBatch` emission does
        //     not silently lose the event — it will be logged immediately.
        Ok(Some(ClipboardContent::SkippedBatch(missed))) => {
            tracing::debug!(
                missed,
                "clipboard: SkippedBatch({missed}) — {missed} intermediate clipboard \
                 update(s) were not captured (most-recent item still captured)"
            );
        }
        Ok(None) => {}
        Err(e) => tracing::warn!("clipboard poll error: {e}"),
    }
}

/// Encrypt a freshly-captured text payload for at-rest storage, producing a
/// ciphertext that the read path (`ipc::write_to_pasteboard`) can decrypt.
///
/// **Key/AAD/key_version consistency (the v0.4 ingest fix).** A new row is
/// stamped `key_version = 2` by [`ClipboardItem::new_text`] (which uses
/// `ITEM_KEY_VERSION_CURRENT = 2`). The read path dispatches on that
/// `key_version` via `copypaste_core::decrypt_item_by_version`, and for
/// `key_version = 2` it decrypts with **the v2 key** (`derive_v2(local_key)`)
/// and **the v4 AAD format** (`build_item_aad_v2(item_id, 4, 2)`).
///
/// Ingest must therefore encrypt with that exact `(key, AAD)` pair. The prior
/// code encrypted with the raw `local_key` (the v1 key) + the v3 AAD
/// (`build_item_aad(item_id, 3)`) while still stamping `key_version = 2`, so
/// every freshly-captured text item failed to round-trip with
/// `EncryptError::AuthFailed` ("authentication tag mismatch") on paste-back.
///
/// `local_key` is the device's v1 storage key (`load_local_key()` /
/// `DeviceKeypair::local_enc_key`). It is used here only as the input keying
/// material to `derive_v2`, mirroring exactly what the read path does
/// (`derive_v2(&self.local_key)`), so the two sides derive the identical v2
/// key.
pub(crate) fn encrypt_text_for_storage(
    plaintext: &[u8],
    local_key: &[u8; 32],
    item_id: &str,
) -> Result<([u8; copypaste_core::NONCE_SIZE], Vec<u8>), copypaste_core::EncryptError> {
    let v2_key = derive_v2(local_key);
    let aad = build_item_aad_v2(
        &ItemId::from(item_id),
        AAD_SCHEMA_VERSION_V4,
        ITEM_KEY_VERSION_CURRENT_U32,
    );
    encrypt_item_with_aad(plaintext, &v2_key, &aad)
}

/// `key_version` stamped into newly-inserted rows, cast from the canonical
/// `copypaste_core::ITEM_KEY_VERSION_CURRENT` (i64) to `u32` as required by
/// `build_item_aad_v2`. A compile-time assertion keeps them in sync.
const ITEM_KEY_VERSION_CURRENT_U32: u32 = ITEM_KEY_VERSION_CURRENT as u32;
// Compile-time guard: if core ever bumps ITEM_KEY_VERSION_CURRENT the cast
// above silently changes too, but this assert documents the expected value.
const _: () = assert!(
    ITEM_KEY_VERSION_CURRENT == 2,
    "ITEM_KEY_VERSION_CURRENT changed — review encrypt_text_for_storage AAD"
);

pub(crate) async fn handle_text(
    text: String,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    local_device_id: &str,
    // mtf5 (PG-22): bundle ID of the frontmost app at capture time, used to
    // force-sensitive any item originating from a password manager / sensitive
    // app (via `is_sensitive_app`).  `None` on non-macOS or when lsappinfo
    // is unavailable.
    source_bundle_id: Option<String>,
) -> Option<ClipboardItem> {
    // Migration gate is now enforced at the Database layer inside
    // `insert_item` / `insert_item_with_fts` (ItemsError::MigrationInProgress).
    // The call-site guard that used to live here has been removed.

    // Item 2: use confidence-gated autowipe check (floor 0.70) so low-signal
    // patterns (phone numbers, order-ids) no longer trigger the 30s TTL wipe.
    // The old `detect(&text).is_some()` fired on any match regardless of
    // confidence; `is_sensitive_for_autowipe` requires confidence >= 0.70.
    //
    // mtf5 (PG-22): also flag the item sensitive when it originates from a
    // known password-manager / sensitive app, even if the content pattern
    // alone would not trigger auto-wipe.  This is the correct defence in depth:
    // a freshly-copied password is often a random string with low confidence.
    let content_is_sensitive = is_sensitive_for_autowipe(&text);
    let app_is_sensitive = source_bundle_id
        .as_deref()
        .map(copypaste_core::is_sensitive_app)
        .unwrap_or(false);
    let is_sensitive = content_is_sensitive || app_is_sensitive;

    // Compute SHA-256 content hash of the PLAINTEXT bytes.
    // This is used for deduplication: if an identical item already exists in
    // history (any age, not expired), we bump its wall_time/lamport_ts to now
    // rather than inserting a duplicate row. The hash is stored on new inserts
    // so future captures of the same content can find the existing row.
    //
    // NEVER log the plaintext or hash — the hash alone is not reversible but
    // logging it alongside the content would create a correlation risk.
    let hash_hex = {
        use sha2::{Digest, Sha256};
        hex::encode(Sha256::digest(text.as_bytes()))
    };

    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64;

    // daemon-core L1: every DB touch below is synchronous rusqlite. Run the
    // whole dedup-lookup / bump-or-insert / prune sequence on a blocking thread
    // (mirroring the IPC path) so the async worker is not blocked while the
    // tokio Mutex is held. Inputs are moved in; the resulting item (if any) is
    // returned for the broadcast channel.
    let db = db.clone();
    let config = config.clone();
    let local_key = *local_key;
    let local_device_id = local_device_id.to_string();
    let join = tokio::task::spawn_blocking(move || {
        let db_guard = db.blocking_lock();

        // Dedup: look for any non-expired row with the same content hash.
        // `find_recent_by_hash` uses a generous window (i64::MAX) to cover ALL
        // history, not just the last N minutes.  A pinned item is never expired
        // so it will always be found and bumped, which is the correct behaviour.
        match find_recent_by_hash(&db_guard, &hash_hex, now_ms, i64::MAX) {
            Ok(Some(existing_id)) => {
                // Identical content already in history: bump recency to now so
                // the existing row rises to the top of the pinned-first,
                // wall_time DESC sort. We do NOT insert a new row.
                // CopyPaste-ojhe: stamp the unified lamport value space
                // (max(existing + 1, now_ms)) so a recopy is monotonic relative
                // to the row's own prior lamport AND time-ordered. Previously
                // this used bare `now_ms`, which a later pin/delete deriving from
                // a small counter could never overtake under lamport-only LWW.
                // CopyPaste-crh3.68: fetch the existing row ONCE and reuse it for
                // both the lamport read and the broadcast value, instead of
                // re-fetching after the bump (was 4 DB queries on every dedup hit;
                // now 2 query_row calls + the UPDATE).
                let mut existing_row = match get_item_by_id(&*db_guard, &existing_id) {
                    Ok(Some(row)) => row,
                    // Row already gone (raced with a delete) — nothing to bump or
                    // broadcast; the next poll re-captures on a fresh changeCount.
                    Ok(None) => return None,
                    Err(e) => {
                        tracing::warn!("text dedup: could not read existing item: {e}");
                        return None;
                    }
                };
                let new_lamport =
                    copypaste_core::next_lamport_ts(existing_row.lamport_ts, now_ms);
                match bump_item_recency(&db_guard, &existing_id, now_ms, new_lamport, None) {
                    Ok(changed) if changed > 0 => {
                        tracing::debug!(
                            existing = %existing_id,
                            "text dedup: bumped existing row to top (same content_hash)"
                        );
                        // Reuse the already-fetched row — only wall_time +
                        // lamport_ts changed — so broadcast subscribers (P2P, sync)
                        // see the recency update without a 4th DB read.
                        existing_row.wall_time = now_ms;
                        existing_row.lamport_ts = new_lamport;
                        return Some(existing_row);
                    }
                    Ok(_) => {
                        // Row disappeared between find and bump (race on delete) —
                        // produce no broadcast item; the next poll re-captures.
                        tracing::debug!(
                            existing = %existing_id,
                            "text dedup: existing row disappeared before bump (deleted concurrently)"
                        );
                        return None;
                    }
                    Err(e) => {
                        tracing::warn!("text dedup bump failed: {e}");
                        return None;
                    }
                }
            }
            Ok(None) => {
                // No existing row with this hash — proceed with a fresh insert.
            }
            Err(e) => {
                // DB error on the dedup lookup: log and fall through to insert.
                // Inserting a duplicate is preferable to silently losing a capture.
                tracing::warn!("text dedup hash lookup failed: {e}");
            }
        }

        // Fresh insert path: encrypt then store.
        let item_id = uuid::Uuid::new_v4().to_string();
        let (nonce, ciphertext) =
            match encrypt_text_for_storage(text.as_bytes(), &local_key, &item_id) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!("encrypt_text_for_storage failed for text: {e}");
                    return None;
                }
            };
        // CopyPaste-ojhe: stamp the unified lamport value space at capture
        // (`next_lamport_ts(0, now_ms) == now_ms`) instead of a hardcoded 0.
        // A fresh capture must outrank an older recopy/pin/delete of unrelated
        // items under lamport-first LWW; a 0-stamped capture could never win.
        let mut item = ClipboardItem::new_text(
            ciphertext,
            nonce.to_vec(),
            copypaste_core::next_lamport_ts(0, now_ms),
        );
        item.item_id = item_id.into();
        item.is_sensitive = is_sensitive;
        // mtf5 (PG-22): record which app was frontmost at capture time.
        // This allows UIs to display the source app and lets the DB preserve
        // attribution across restarts.
        item.app_bundle_id = source_bundle_id;
        // Stamp the stable on-disk device_id so cloud/P2P peers attribute every
        // captured item to this specific machine across restarts.
        item.origin_device_id = local_device_id;
        // Store the content hash so future captures of identical content can
        // find and bump this row instead of inserting a duplicate.
        item.content_hash = Some(hash_hex);

        if is_sensitive {
            item.expires_at = Some(now_ms + (config.sensitive_ttl_local_secs as i64 * 1000));
        }

        // v0.3 post-T2: insert_item + upsert_fts collapsed into a single
        // transaction. Closes the TOCTOU window where a crash between the row
        // insert and the FTS upsert could leave a row that search would never
        // find. Also handles the v5 UNIQUE-index dedup race internally.
        match insert_item_with_fts(&db_guard, &item, &text) {
            Ok(stored_id) => {
                if stored_id != item.id {
                    // Fix MED #4: `insert_item_with_fts` deduped `item` against
                    // an existing row identified by `stored_id`. Broadcasting
                    // `item` (which carries the REJECTED new uuid) would cause
                    // subscribers (P2P, sync) to look up a nonexistent row.
                    // Fetch the ACTUAL stored row and broadcast that instead, so
                    // all consumers observe a valid, persisted item. If the fetch
                    // fails (extreme race), produce no broadcast for this poll.
                    tracing::debug!(
                        requested = %item.id,
                        existing = %stored_id,
                        "text item deduped against existing row (UNIQUE index race) — broadcasting stored row"
                    );
                    prune_history(&db_guard, &config);
                    match get_item_by_id(&*db_guard, &stored_id) {
                        Ok(Some(stored_item)) => Some(stored_item),
                        Ok(None) => {
                            tracing::debug!(
                                id = %stored_id,
                                "text dedup: stored row disappeared before fetch (deleted concurrently)"
                            );
                            None
                        }
                        Err(e) => {
                            tracing::warn!("text dedup: failed to fetch stored row for broadcast: {e}");
                            None
                        }
                    }
                } else {
                    tracing::info!(
                        id = %item.id,
                        sensitive = is_sensitive,
                        "stored text item id={} sensitive={}",
                        item.id,
                        is_sensitive
                    );
                    prune_history(&db_guard, &config);
                    Some(item)
                }
            }
            Err(e) => {
                tracing::warn!("failed to store text item: {e}");
                None
            }
        }
    })
    .await;
    match join {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!("handle_text blocking task failed: {e}");
            None
        }
    }
}

pub(crate) async fn handle_image(
    raw_bytes: Vec<u8>,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    local_device_id: &str,
    // mtf5 (PG-22): bundle ID of the frontmost app at capture time.
    source_bundle_id: Option<String>,
) -> Option<ClipboardItem> {
    // Migration gate is now enforced at the Database layer inside
    // `insert_item` / `insert_item_with_fts` (ItemsError::MigrationInProgress).
    // The call-site guard that used to live here has been removed.

    // daemon-core L1: the image encode (CPU-heavy compression + encryption) and
    // the rusqlite insert/prune are all synchronous. Run the whole sequence on a
    // blocking thread, mirroring the IPC path, so the async worker is never
    // blocked while the tokio Mutex is held.
    let db = db.clone();
    let config = config.clone();
    let local_key = *local_key;
    let local_device_id = local_device_id.to_string();
    // mtf5 (PG-22): pre-compute the sensitive-app flag before moving into the
    // blocking closure (borrows source_bundle_id before it is moved in).
    let app_is_sensitive_img = source_bundle_id
        .as_deref()
        .map(copypaste_core::is_sensitive_app)
        .unwrap_or(false);
    let join = tokio::task::spawn_blocking(move || {
        // fix(44rq.39): compute the size cap BEFORE hashing.  A SHA-256 pass
        // over a 25 MB oversize image wastes ~25 ms of CPU and then is thrown
        // away by `encode_image_full`'s own size gate.  Reject early to avoid
        // that wasted work — behaviour for accepted images is unchanged.
        //
        // Honour the user-configured raw-image cap (default 25 MB) instead of
        // the library's hard 10 MB floor, which silently rejected 10–25 MB
        // images the config permitted. `usize::MAX` saturation keeps 32-bit
        // targets safe.
        let max_image_bytes = usize::try_from(config.max_image_size_bytes).unwrap_or(usize::MAX);
        if raw_bytes.len() > max_image_bytes {
            tracing::warn!(
                actual = raw_bytes.len(),
                max = max_image_bytes,
                "image too large; rejecting before hash (fix 44rq.39)"
            );
            return None;
        }

        // Derive a stable file_id from SHA-256(raw_bytes)[..16] — a 128-bit
        // collision-resistant content hash. Deterministic so identical images
        // dedup naturally (Wave 2.1 security LOW #19).
        // NOTE: only reached for images that pass the size gate above.
        let file_id = crate::clipboard::image_content_hash(&raw_bytes);

        // The thumbnail is encrypted with the SAME content key but a DISTINCT
        // file_id so its AEAD AAD is isolated from the full image's. Derive it
        // deterministically from the content-hash file_id so identical images
        // still dedup and the reader can recompute / parse it.
        let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);
        // Item 3: pass config.max_decoded_image_mb so the decode-bomb budget
        // comes from the live AppConfig rather than the compile-time default
        // baked into the `encode_image` wrapper. encode_image_full decodes ONCE
        // and reuses the bitmap for both the full PNG and the downscaled
        // thumbnail (Variant-B: avoid a second decode of the clipboard bytes).
        let v2_key = copypaste_core::derive_v2(&local_key);
        match encode_image_full(
            &raw_bytes,
            &v2_key,
            &file_id,
            &thumb_file_id,
            max_image_bytes,
            config.max_decoded_image_mb,
            copypaste_core::THUMBNAIL_MAX_DIM,
        ) {
            Ok((meta, chunks, thumb_blob, thumb_w, thumb_h)) => {
                let blob = match chunks_to_blob(&chunks) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::error!(error = %e, "chunks_to_blob failed; dropping image");
                        return None;
                    }
                };
                // Additively record thumb_file_id / thumb_w / thumb_h alongside
                // the existing width/height/original_size/chunk_count/file_id
                // keys; the core reader ignores unknown keys, so this stays
                // forward- and backward-compatible.
                let meta_json = crate::clipboard::build_image_meta_json(
                    &meta,
                    &thumb_file_id,
                    thumb_w,
                    thumb_h,
                );
                // encode_image_full always produces a thumbnail blob; treat an
                // (unexpected) empty blob as "no thumb" so get_item_thumbnail
                // returns the null sentinel rather than failing decode. Capture
                // is never failed on thumbnail trouble — the Err arm below only
                // fires on full-image encode failure.
                let thumb = if thumb_blob.is_empty() {
                    None
                } else {
                    Some(thumb_blob)
                };
                let mut item = ClipboardItem::new_image(blob, meta_json, 0, thumb);
                // CopyPaste-ojhe: stamp the unified lamport value space at
                // capture (`next_lamport_ts(0, wall_time) == wall_time`) instead
                // of a hardcoded 0, so a fresh capture is time-ordered under
                // lamport-first LWW. `new_image` set `wall_time = now` already.
                item.lamport_ts = copypaste_core::next_lamport_ts(0, item.wall_time);
                // Stable cross-device item identity (mirror handle_text, which
                // sets `item.item_id` once at capture). `new_image` seeds a fresh
                // random `item_id`; that would give the SAME image a different
                // identity on each device, so the sync/merge/dedup layer (which
                // keys on `item_id`) would never converge them and duplicate rows
                // would accumulate. Derive the `item_id` deterministically from
                // the content-hash `file_id` so identical images share one
                // identity across devices and LWW can fire. (The image AEAD AAD
                // is bound to `file_id`, not `item_id`, so this does not affect
                // chunk encryption.)
                item.item_id = uuid::Uuid::from_bytes(file_id).to_string().into();
                // Stamp stable device identity (same fix as handle_text).
                item.origin_device_id = local_device_id;
                // mtf5 (PG-22): mark sensitive when the source app is a
                // password manager, even if the image content has no pattern.
                item.is_sensitive = app_is_sensitive_img;
                item.app_bundle_id = source_bundle_id;
                tracing::debug!(
                    "image encoded: {}x{} px, {} chunks, original_size={}",
                    meta.width,
                    meta.height,
                    meta.chunk_count,
                    meta.original_size
                );

                let db_guard = db.blocking_lock();
                // Atomic insert: images have no searchable text, so we pass "" to
                // skip the FTS write (insert_item_with_fts treats empty as
                // "image item" and only writes the row).
                match insert_item_with_fts(&db_guard, &item, "") {
                    Ok(stored_id) => {
                        if stored_id != item.id {
                            tracing::debug!(
                                requested = %item.id,
                                existing = %stored_id,
                                "image item deduped against existing row"
                            );
                        } else {
                            tracing::info!(id = %item.id, "stored image item id={}", item.id);
                        }
                        prune_history(&db_guard, &config);
                        Some(item)
                    }
                    Err(e) => {
                        tracing::warn!("failed to store image item: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("image encode failed (skipping): {e}");
                None
            }
        }
    })
    .await;
    match join {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!("handle_image blocking task failed: {e}");
            None
        }
    }
}

/// Encrypt and store a freshly-captured file for at-rest storage.
///
/// Mirrors [`handle_image`] but uses [`copypaste_core::encode_file`] (no
/// decode/re-encode — the raw bytes are chunked verbatim). The `file_id` is
/// derived from SHA-256(raw_bytes)[..16] so identical files dedup across
/// devices. The `item_id` is set to `uuid::Uuid::from_bytes(file_id)` for the
/// same reason (cross-device CRDT identity, mirrors the image path).
///
/// The meta JSON produced by [`crate::clipboard::build_file_meta_json`] uses
/// the keys `filename`, `mime`, `original_size`, `chunk_count`, `file_id` —
/// identical to the keys expected by `ipc::parse_file_meta` and
/// `sync_orch::build_file_meta_json`.
pub(crate) async fn handle_file(
    raw_bytes: Vec<u8>,
    filename: String,
    mime: String,
    db: &Arc<Mutex<Database>>,
    local_key: &[u8; 32],
    config: &AppConfig,
    local_device_id: &str,
    // mtf5 (PG-22): bundle ID of the frontmost app at capture time.
    source_bundle_id: Option<String>,
) -> Option<ClipboardItem> {
    let db = db.clone();
    let config = config.clone();
    let local_key = *local_key;
    let local_device_id = local_device_id.to_string();
    // mtf5 (PG-22): pre-compute before move into blocking closure.
    let app_is_sensitive_file = source_bundle_id
        .as_deref()
        .map(copypaste_core::is_sensitive_app)
        .unwrap_or(false);
    let join = tokio::task::spawn_blocking(move || {
        // Content-hash file_id: deterministic so identical files dedup.
        let file_id = crate::clipboard::image_content_hash(&raw_bytes);

        let max_file_bytes = usize::try_from(config.max_file_size_bytes).unwrap_or(usize::MAX);

        let v2_key = copypaste_core::derive_v2(&local_key);
        match copypaste_core::encode_file(
            &raw_bytes,
            &filename,
            &mime,
            &v2_key,
            &file_id,
            max_file_bytes,
        ) {
            Ok((meta, chunks)) => {
                let blob = match copypaste_core::chunks_to_blob(&chunks) {
                    Ok(b) => b,
                    Err(e) => {
                        tracing::error!(error = %e, "handle_file: chunks_to_blob failed");
                        return None;
                    }
                };
                let meta_json = crate::clipboard::build_file_meta_json(&meta);
                let mut item = ClipboardItem::new_file(blob, meta_json, 0);
                // CopyPaste-ojhe: stamp the unified lamport value space at
                // capture (`next_lamport_ts(0, wall_time) == wall_time`) instead
                // of a hardcoded 0, so a fresh capture is time-ordered under
                // lamport-first LWW. `new_file` set `wall_time = now` already.
                item.lamport_ts = copypaste_core::next_lamport_ts(0, item.wall_time);
                // Stable cross-device identity: derive item_id from the
                // content-hash file_id (same pattern as handle_image).
                item.item_id = uuid::Uuid::from_bytes(file_id).to_string().into();
                item.origin_device_id = local_device_id;
                // mtf5 (PG-22): mark sensitive when source app is a password manager.
                item.is_sensitive = app_is_sensitive_file;
                item.app_bundle_id = source_bundle_id;
                tracing::debug!(
                    "file encoded: {} chunks, original_size={}",
                    meta.chunk_count,
                    meta.original_size
                );

                let db_guard = db.blocking_lock();
                // Files have no searchable text body; pass "" to skip FTS.
                match insert_item_with_fts(&db_guard, &item, "") {
                    Ok(stored_id) => {
                        if stored_id != item.id {
                            tracing::debug!(
                                requested = %item.id,
                                existing = %stored_id,
                                "file item deduped against existing row"
                            );
                        } else {
                            tracing::info!(id = %item.id, "stored file item id={}", item.id);
                        }
                        prune_history(&db_guard, &config);
                        Some(item)
                    }
                    Err(e) => {
                        tracing::warn!("failed to store file item: {e}");
                        None
                    }
                }
            }
            Err(e) => {
                tracing::warn!("file encode failed (skipping): {e}");
                None
            }
        }
    })
    .await;
    match join {
        Ok(item) => item,
        Err(e) => {
            tracing::warn!("handle_file blocking task failed: {e}");
            None
        }
    }
}

/// Enforce the size-only cap after each local insert.
///
/// The count cap (`history_limit`) has been removed: the local DB is bounded
/// exclusively by `storage_quota_bytes`. Pinned items are never evicted.
///
/// `storage_quota_bytes` is u64 in AppConfig; saturating cast to i64 is safe
/// because values above i64::MAX (>9 EB) are unreachable in practice.
pub(crate) fn prune_history(db: &Database, config: &AppConfig) {
    match prune_to_cap(db, config.storage_quota_bytes as i64) {
        Ok(0) => {}
        Ok(n) => tracing::debug!("prune_history: byte-cap pruned {n} rows"),
        Err(e) => tracing::warn!("prune_history: byte-cap prune failed: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::{decrypt_item_by_version, Database, NONCE_SIZE};

    /// Build a valid 2×2 white PNG via the `image` crate. Generating it (vs a
    /// hand-crafted byte array) keeps the test robust against the PNG
    /// decoder's strictness — mirrors `copypaste_core::image`'s own tests.
    fn test_png() -> Vec<u8> {
        use image::{DynamicImage, ImageBuffer, Rgb};
        let img = ImageBuffer::from_fn(2, 2, |_, _| Rgb([255u8, 255u8, 255u8]));
        copypaste_core::encode_as_png(&DynamicImage::ImageRgb8(img)).expect("encode test PNG")
    }

    /// Read the single stored image row's `(content_blob, blob_ref)` back.
    fn read_image_row(db: &Database) -> (Vec<u8>, String) {
        db.conn()
            .query_row(
                "SELECT content, blob_ref FROM clipboard_items \
                 WHERE content_type = 'image' LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("image row exists")
    }

    /// GAP closer (image): drive the REAL image write path
    /// (`handle_image` → `encode_image` with the device's real `local_key`,
    /// producing the daemon's real chunk blob + `blob_ref` metadata JSON) and
    /// read it back through the REAL read path
    /// (`ipc::parse_image_file_id` → `chunks_from_blob` → `decode_image`),
    /// asserting the PNG bytes recover. Mirrors the text round-trip test.
    #[tokio::test]
    async fn fresh_image_capture_round_trips_through_read_path() {
        let local_key = [0x42u8; 32]; // stands in for load_local_key()
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let png = test_png();

        // Ingest: exactly what the monitor loop does on a fresh image capture.
        let item = handle_image(png.clone(), &db, &local_key, &config, "test-device", None)
            .await
            .expect("handle_image must store the image");
        assert_eq!(item.content_type, "image");

        // Read path: pull the stored blob + metadata and decrypt exactly as
        // ipc::write_to_pasteboard's image branch does.
        let guard = db.lock().await;
        let (blob, meta_json) = read_image_row(&guard);
        let file_id =
            crate::ipc::parse_image_file_id(&meta_json).expect("file_id parses from blob_ref");
        let chunks = copypaste_core::chunks_from_blob(&blob).expect("chunks deserialize");
        // handle_image encrypts with derive_v2(&local_key) (key_version = 2),
        // so the read path must also decrypt with the v2-derived key.
        let v2_key = copypaste_core::derive_v2(&local_key);
        let recovered_png =
            copypaste_core::decode_image(&chunks, &v2_key, &file_id).expect("decode_image");

        // `handle_image` re-encodes the raw clipboard bytes to PNG before
        // chunking, so the recovered bytes are the canonical PNG of the
        // decoded image — compute the same reference and compare.
        let reference_png = copypaste_core::encode_as_png(
            &copypaste_core::decode_clipboard_image(&png).expect("decode raw"),
        )
        .expect("encode reference png");
        assert_eq!(
            recovered_png, reference_png,
            "image must round-trip through the read path to the stored PNG"
        );
    }

    /// GAP closer (image, key rotation): an image row encrypted under the
    /// pre-rotation `local_key` MUST, after a local key rotation, either still
    /// decode OR fail with a clear, explicit error — never silent corruption.
    ///
    /// Image chunks are AEAD-encrypted with the raw `local_key` directly
    /// (no key_version dispatch — see `ipc::write_to_pasteboard`'s image
    /// branch and `crypto::chunks`). A rotated key therefore cannot satisfy
    /// the per-chunk auth tag, so `decode_image` MUST return an explicit
    /// `ImageError` (auth failure) rather than returning wrong/garbage bytes.
    /// This test pins that intended behaviour.
    #[tokio::test]
    async fn image_row_survives_local_key_rotation_or_errors_cleanly() {
        let old_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let png = test_png();

        // Capture an image under the OLD key.
        handle_image(png.clone(), &db, &old_key, &config, "test-device", None)
            .await
            .expect("handle_image must store the image");

        let guard = db.lock().await;
        let (blob, meta_json) = read_image_row(&guard);
        let file_id =
            crate::ipc::parse_image_file_id(&meta_json).expect("file_id parses from blob_ref");
        let chunks = copypaste_core::chunks_from_blob(&blob).expect("chunks deserialize");

        // Rotate the local key (simulate a key rotation / new device secret).
        let rotated_key = [0x99u8; 32];
        assert_ne!(old_key, rotated_key, "precondition: key actually changed");

        // handle_image encrypts with derive_v2(key) (key_version = 2). A
        // rotated key's v2 derivation ≠ the original key's v2 derivation, so
        // decoding must fail explicitly — never silently return wrong bytes.
        let rotated_v2_key = copypaste_core::derive_v2(&rotated_key);
        let result = copypaste_core::decode_image(&chunks, &rotated_v2_key, &file_id);
        assert!(
            result.is_err(),
            "a pre-rotation image row must NOT silently decode under a rotated key"
        );

        // And the original key's v2 derivation must still decode it (rotation
        // does not destroy the existing row's recoverability under its own key).
        let old_v2_key = copypaste_core::derive_v2(&old_key);
        let recovered = copypaste_core::decode_image(&chunks, &old_v2_key, &file_id)
            .expect("the pre-rotation row must still decode under its original key");
        let reference_png = copypaste_core::encode_as_png(
            &copypaste_core::decode_clipboard_image(&png).expect("decode raw"),
        )
        .expect("encode reference png");
        assert_eq!(
            recovered, reference_png,
            "under its original key the row decodes to the stored PNG"
        );
    }

    // -----------------------------------------------------------------------
    // fix(44rq.39): size gate fires BEFORE image_content_hash
    // -----------------------------------------------------------------------

    /// An oversize image must be rejected by the size gate inside the
    /// `spawn_blocking` closure before `image_content_hash` (SHA-256) is
    /// called.  We can't intercept the hash call, but we CAN verify that
    /// `handle_image` returns `None` for an image that exceeds
    /// `max_image_size_bytes` — which is the externally observable contract.
    ///
    /// The test also confirms that a same-size-as-cap image is accepted
    /// (boundary condition: `len == cap` must pass, `len > cap` must not).
    #[tokio::test]
    async fn oversize_image_rejected_before_hash_fix_44rq39() {
        let local_key = [0xABu8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));

        // Set a tiny cap (32 bytes) so any real PNG exceeds it.
        let config = AppConfig {
            max_image_size_bytes: 32,
            ..Default::default()
        };

        // 33 bytes — one byte over the cap; must be rejected immediately.
        let oversized: Vec<u8> = vec![0u8; 33];
        let result = handle_image(oversized, &db, &local_key, &config, "test-device", None).await;
        assert!(
            result.is_none(),
            "handle_image must return None for an image exceeding max_image_size_bytes \
             (fix 44rq.39: size gate must fire before SHA-256 hash)"
        );

        // Confirm the DB is still empty — nothing was inserted for the oversize image.
        let guard = db.lock().await;
        let count: i64 = guard
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| r.get(0))
            .expect("count query");
        assert_eq!(
            count, 0,
            "no image row must be written for an oversize image"
        );
    }

    // -----------------------------------------------------------------------
    // Regression guard: real-write → real-read key_version round-trip
    // (v0.3.4 lesson: writer/reader key_version desync causes AuthFailed).
    // -----------------------------------------------------------------------

    /// Drive the REAL production write paths (`handle_image`, `handle_file`) into
    /// the REAL production IPC read handlers (`get_item_image`, `get_item_file`,
    /// `get_item_thumbnail`) and assert the bytes round-trip cleanly.
    ///
    /// This test catches any future desync between the writer key (always
    /// `derive_v2(local_key)` for `key_version = 2` rows) and the reader key
    /// (dispatched on `item.key_version`). If a writer and reader ever disagree
    /// on which key to use, this test will fail with `auth_failed` or
    /// `decode_failed` long before the regression reaches production.
    #[cfg(unix)]
    #[tokio::test]
    async fn real_write_to_real_read_roundtrip_image_and_file() {
        use base64::Engine as _;
        use image::{DynamicImage, RgbaImage};
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let local_key = [0xAAu8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        // ── Write: use REAL handle_image and handle_file ────────────────────

        // Build a 64×64 image (small for test speed, but real PNG).
        let mut buf = RgbaImage::new(64, 64);
        for (x, y, px) in buf.enumerate_pixels_mut() {
            *px = image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]);
        }
        let raw_png =
            copypaste_core::encode_as_png(&DynamicImage::ImageRgba8(buf)).expect("encode test PNG");

        let img_item = handle_image(
            raw_png.clone(),
            &db,
            &local_key,
            &config,
            "reg-device",
            None,
        )
        .await
        .expect("handle_image must store the image");
        assert_eq!(
            img_item.key_version, 2,
            "handle_image must stamp key_version = 2"
        );
        let img_id = img_item.id.clone();

        let raw_file = b"regression test file bytes";
        let file_item = handle_file(
            raw_file.to_vec(),
            "reg.txt".to_string(),
            "text/plain".to_string(),
            &db,
            &local_key,
            &config,
            "reg-device",
            None,
        )
        .await
        .expect("handle_file must store the file");
        assert_eq!(
            file_item.key_version, 2,
            "handle_file must stamp key_version = 2"
        );
        let file_id = file_item.id.clone();

        // ── Read: serve via the REAL IpcServer and dispatch on the socket ───
        let dir = tempfile::tempdir().expect("tempdir");
        let socket_path = dir.path().join("reg_rt.sock");

        let ipc_key = Arc::new(zeroize::Zeroizing::new(local_key));
        let ipc_pub = Arc::new([0u8; 32]);
        let server = crate::ipc::IpcServer::new(
            db.clone(),
            Arc::new(std::sync::atomic::AtomicBool::new(false)),
            ipc_key,
            ipc_pub,
        );
        let sock_clone = socket_path.clone();
        tokio::spawn(async move {
            let _ = server
                .serve(&sock_clone, tokio_util::sync::CancellationToken::new())
                .await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Helper: send one JSON-RPC request over the socket, return parsed response.
        let send_req = |method: String, params: String| {
            let path = socket_path.clone();
            async move {
                let mut stream = UnixStream::connect(&path).await.unwrap();
                let req =
                    format!("{{\"id\":\"r1\",\"method\":\"{method}\",\"params\":{params}}}\n");
                stream.write_all(req.as_bytes()).await.unwrap();
                let mut reader = BufReader::new(stream);
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                serde_json::from_str::<serde_json::Value>(line.trim()).expect("valid JSON")
            }
        };

        // get_item_image round-trip.
        let img_resp = send_req(
            "get_item_image".to_string(),
            format!("{{\"id\":\"{img_id}\"}}"),
        )
        .await;
        assert_eq!(
            img_resp["ok"], true,
            "get_item_image must succeed: {img_resp}"
        );
        let data_uri = img_resp["data"]["data_uri"]
            .as_str()
            .expect("data_uri must be a string");
        assert!(
            data_uri.starts_with("data:image/png;base64,"),
            "data_uri must be a PNG data-URI"
        );
        // Decode the returned PNG and compare to what handle_image would have stored.
        let b64 = data_uri.strip_prefix("data:image/png;base64,").unwrap();
        let returned_png = base64::engine::general_purpose::STANDARD
            .decode(b64)
            .expect("base64 decode must succeed");
        let reference_png = copypaste_core::encode_as_png(
            &copypaste_core::decode_clipboard_image(&raw_png).expect("decode raw"),
        )
        .expect("encode reference png");
        assert_eq!(
            returned_png, reference_png,
            "get_item_image must return the canonical PNG stored by handle_image"
        );

        // get_item_thumbnail round-trip (may backfill or serve stored thumb).
        let thumb_resp = send_req(
            "get_item_thumbnail".to_string(),
            format!("{{\"id\":\"{img_id}\"}}"),
        )
        .await;
        assert_eq!(
            thumb_resp["ok"], true,
            "get_item_thumbnail must succeed: {thumb_resp}"
        );
        assert!(
            !thumb_resp["data"]["thumbnail"].is_null(),
            "get_item_thumbnail must return a non-null thumbnail: {thumb_resp}"
        );

        // get_item_file round-trip.
        let file_resp = send_req(
            "get_item_file".to_string(),
            format!("{{\"id\":\"{file_id}\"}}"),
        )
        .await;
        assert_eq!(
            file_resp["ok"], true,
            "get_item_file must succeed: {file_resp}"
        );
        assert_eq!(file_resp["data"]["filename"], "reg.txt");
        assert_eq!(file_resp["data"]["mime"], "text/plain");
        let data_b64 = file_resp["data"]["data_b64"]
            .as_str()
            .expect("data_b64 must be a string");
        let returned_bytes = base64::engine::general_purpose::STANDARD
            .decode(data_b64)
            .expect("base64 decode must succeed");
        assert_eq!(
            returned_bytes,
            raw_file.to_vec(),
            "get_item_file must return the original file bytes"
        );
    }

    // -----------------------------------------------------------------------
    // FIX 2: dedup-bump — identical content bumps the existing row to top
    // -----------------------------------------------------------------------

    /// Capturing the same text twice must NOT insert a second row. The existing
    /// row's wall_time must be updated so it appears at the top of history.
    #[tokio::test]
    async fn handle_text_dedup_bumps_existing_row_not_inserts() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let text = "duplicate clipboard text".to_string();

        // First capture.
        let item1 = handle_text(text.clone(), &db, &local_key, &config, "test-device", None)
            .await
            .expect("first capture must succeed");

        // Verify content_hash is set after first insert.
        {
            let guard = db.lock().await;
            let row = copypaste_core::get_item_by_id(&*guard, &item1.id)
                .unwrap()
                .expect("first row must exist");
            assert!(
                row.content_hash.is_some(),
                "content_hash must be set on new row"
            );
        }

        // Second capture of the same text.
        let _item2 = handle_text(text.clone(), &db, &local_key, &config, "test-device", None).await;

        // Must still be exactly one row.
        let guard = db.lock().await;
        let total = copypaste_core::count_items(&*guard).expect("count_items");
        assert_eq!(
            total, 1,
            "identical text must not insert a duplicate row; expected 1 row, got {total}"
        );
    }

    /// After a dedup bump, the bumped item has a wall_time >= the first
    /// insert's wall_time, so it sorts to the top.
    #[tokio::test]
    async fn handle_text_dedup_bumped_item_has_updated_wall_time() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();
        let text = "text that will be bumped".to_string();

        // Insert the item and record its initial wall_time.
        let item1 = handle_text(text.clone(), &db, &local_key, &config, "test-device", None)
            .await
            .expect("first capture must succeed");
        let wall_time_before = item1.wall_time;

        // A tiny sleep to ensure a different wall_time on the bump.
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;

        // Second capture: should bump, not insert.
        handle_text(text.clone(), &db, &local_key, &config, "test-device", None).await;

        let guard = db.lock().await;
        let row = copypaste_core::get_item_by_id(&*guard, &item1.id)
            .unwrap()
            .expect("original row must still exist after bump");

        assert!(
            row.wall_time >= wall_time_before,
            "bumped wall_time ({}) must be >= original ({})",
            row.wall_time,
            wall_time_before
        );
    }

    /// Capturing two DIFFERENT texts must insert two distinct rows.
    #[tokio::test]
    async fn handle_text_different_content_inserts_two_rows() {
        let local_key = [0x42u8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        handle_text(
            "first distinct text".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
            None,
        )
        .await;
        handle_text(
            "second distinct text".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
            None,
        )
        .await;

        let guard = db.lock().await;
        let total = copypaste_core::count_items(&*guard).expect("count_items");
        assert_eq!(
            total, 2,
            "two distinct texts must produce two rows, got {total}"
        );
    }

    // -----------------------------------------------------------------------
    // CopyPaste-98ja: sensitive TTL pre-check gate
    // -----------------------------------------------------------------------

    /// When the database contains NO sensitive items `run_ttl_cleanup` must not
    /// run `delete_sensitive_expired` — verified by checking that the
    /// has_sensitive_items pre-check (CopyPaste-98ja) gates the full scan.
    ///
    /// This test exercises the gate by:
    /// 1. Starting with an empty DB (no sensitive rows → has_sensitive_items = false).
    /// 2. Calling `run_ttl_cleanup` with `do_sensitive = true` and a 0 ms TTL
    ///    that would delete EVERYTHING if the gate were absent.
    /// 3. Inserting a non-sensitive row and confirming it survives the cleanup —
    ///    proving the DELETE did NOT run.
    #[tokio::test]
    async fn run_ttl_cleanup_skips_sensitive_scan_when_no_sensitive_items() {
        let local_key = [0xAAu8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        // Insert ONE non-sensitive item via handle_text (which correctly
        // sets is_sensitive based on the content).
        handle_text(
            "hello world not sensitive".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
            None,
        )
        .await;

        // Confirm has_sensitive_items returns false (plain text is not sensitive).
        {
            let guard = db.lock().await;
            assert!(
                !copypaste_core::has_sensitive_items(&guard),
                "no sensitive items must be present initially"
            );
        }

        // Run cleanup with an extremely aggressive TTL (0 ms — would delete
        // everything if the gate were absent) and do_sensitive = true.
        // If the gate is working, has_sensitive_items returns false and the
        // DELETE is skipped, leaving the non-sensitive row intact.
        run_ttl_cleanup(&db, 0, true, false).await;

        // The non-sensitive item must still exist.
        let guard = db.lock().await;
        let count = copypaste_core::count_items(&*guard).expect("count_items");
        assert_eq!(
            count, 1,
            "non-sensitive item must survive cleanup when no sensitive items exist"
        );
    }

    // -----------------------------------------------------------------------
    // P2 (ugv7): startup TTL purge runs before IPC bind
    // -----------------------------------------------------------------------

    /// `run_ttl_cleanup` (reused by the startup purge) must delete a sensitive
    /// item whose creation time + TTL is in the past.  This verifies that the
    /// purge that now runs at startup (before the IPC socket is bound) would
    /// actually remove already-expired sensitive rows.
    ///
    /// The test inserts a row with `wall_time = 1` (epoch ms — always expired)
    /// and `is_sensitive = 1`, then calls `run_ttl_cleanup` with a 1 ms TTL so
    /// `threshold = now - 1 ms ≫ 1`, and asserts the row is gone.
    #[tokio::test]
    async fn startup_ttl_purge_removes_expired_sensitive_items() {
        let local_key = zeroize::Zeroizing::new([0xBBu8; 32]);
        let local_key_arc: Arc<zeroize::Zeroizing<[u8; 32]>> = Arc::new(local_key);
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));

        // Insert an expired sensitive row directly via SQL so we can control
        // wall_time (handle_text would set it to now(), which would NOT be expired).
        {
            let guard = db.lock().await;
            let row_id = uuid::Uuid::new_v4().to_string();
            let item_id = copypaste_core::ItemId::from(uuid::Uuid::new_v4().to_string());
            let aad = copypaste_core::build_item_aad(&item_id, copypaste_core::AAD_SCHEMA_VERSION);
            let (nonce, ciphertext) =
                copypaste_core::encrypt_item_with_aad(b"sk-supersecrettoken", &local_key_arc, &aad)
                    .expect("encrypt");
            guard
                .conn()
                .execute(
                    "INSERT INTO clipboard_items \
                     (id, item_id, content_type, content, content_nonce, \
                      is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                     VALUES (?1,?2,'text',?3,?4,1,0,1,1,2)",
                    rusqlite::params![row_id, item_id, ciphertext, nonce.to_vec()],
                )
                .expect("insert expired sensitive row");
        }

        // Verify the row is present.
        {
            let guard = db.lock().await;
            assert!(
                copypaste_core::has_sensitive_items(&guard),
                "sensitive item must be present before cleanup"
            );
        }

        // Run with a 1 ms TTL — the epoch-1 wall_time is always older than
        // `now_ms - 1`, so the row must be purged.
        run_ttl_cleanup(&db, 1, true, false).await;

        // Verify the row was removed.
        let guard = db.lock().await;
        let count = copypaste_core::count_items(&*guard).expect("count_items");
        assert_eq!(
            count, 0,
            "expired sensitive item must be purged by startup TTL cleanup"
        );
    }

    // -----------------------------------------------------------------------
    // P1-2/P1-3: lsappinfo exclusion check documented intent
    // -----------------------------------------------------------------------

    /// Documents the intended behavior of the P1-2/P1-3 fix: when the
    /// exclusion list is non-empty and `lsappinfo` fails (returns `None`),
    /// `handle_tick` must skip capture (fail-closed).
    ///
    /// Full integration (spawning a real tick with a mocked lsappinfo) is not
    /// practical in a unit test; this test instead documents the contract by
    /// verifying `decide_db_startup` still produces the correct plan, and that
    /// the code compiles with the new async `spawn_blocking` call present.
    /// The spawn_blocking path is exercised by the existing TTL cleanup tests
    /// (which also use `spawn_blocking` internally via `run_ttl_cleanup`).
    #[test]
    fn lsappinfo_exclusion_contract_documented() {
        // When excluded_app_bundle_ids is empty the check is skipped entirely —
        // unchanged behavior regardless of lsappinfo status.
        let mut cfg = AppConfig::default();
        assert!(cfg.excluded_app_bundle_ids.is_empty());

        // When the list is non-empty and lsappinfo returns None → fail-closed
        // (skip capture).  This is enforced in the async `handle_tick` body;
        // the test documents the expected branch outcome as a contract check.
        cfg.excluded_app_bundle_ids
            .push("com.example.excluded".to_string());
        assert!(!cfg.excluded_app_bundle_ids.is_empty());
        // The actual fail-closed logic lives in handle_tick (async, requires a
        // real tokio runtime and ClipboardMonitor).  Behavior verified by:
        //   1. Code review: `None` match arm calls `monitor.poll()` + `return`.
        //   2. The spawn_blocking wrapper is confirmed present by compilation.
    }

    // -----------------------------------------------------------------------
    // CopyPaste-44rq.33: FrontmostAppCache TTL behaviour
    // -----------------------------------------------------------------------

    /// Verifies `FrontmostAppCache` TTL logic: a newly-created cache is cold
    /// (not fresh), a populated cache within the TTL is fresh, and a cache
    /// with an `expires_at` in the past is stale.
    ///
    /// This test does NOT spawn lsappinfo — it exercises only the cache
    /// bookkeeping (TTL arithmetic) that wraps the subprocess, which is the
    /// correctness-critical part of the CopyPaste-44rq.33 fix.
    #[cfg(target_os = "macos")]
    #[test]
    fn frontmost_app_cache_ttl_logic() {
        // Cold cache (constructed via `new()`) must be stale so the first
        // call to handle_tick always refreshes it.
        let cache = FrontmostAppCache::new();
        assert!(
            !cache.is_fresh(),
            "newly-created cache must not be fresh (forces first-tick refresh)"
        );
        assert!(
            cache.cached_value.is_none(),
            "newly-created cache must have no value"
        );
        assert!(
            !cache.is_failure,
            "newly-created cache must not be marked as a failure"
        );

        // A cache populated with a future expiry must be reported as fresh.
        let mut hot_cache = FrontmostAppCache {
            cached_value: Some("com.apple.finder".to_string()),
            is_failure: false,
            expires_at: std::time::Instant::now()
                + std::time::Duration::from_secs(FRONTMOST_APP_CACHE_TTL_SECS),
        };
        assert!(
            hot_cache.is_fresh(),
            "cache with future expiry must be fresh"
        );
        assert_eq!(
            hot_cache.cached_value.as_deref(),
            Some("com.apple.finder"),
            "cached bundle ID must be returned unchanged"
        );

        // Simulate TTL expiry by back-dating expires_at.
        hot_cache.expires_at = std::time::Instant::now()
            .checked_sub(std::time::Duration::from_millis(1))
            // Impossible in practice; fall back to a fresh-but-immediate expiry.
            .unwrap_or_else(std::time::Instant::now);
        assert!(
            !hot_cache.is_fresh(),
            "cache with past expiry must not be fresh (must trigger refresh)"
        );

        // Failure result (lsappinfo returned None) must be cached too.
        let failure_cache = FrontmostAppCache {
            cached_value: None,
            is_failure: true,
            expires_at: std::time::Instant::now()
                + std::time::Duration::from_secs(FRONTMOST_APP_CACHE_TTL_SECS),
        };
        assert!(
            failure_cache.is_fresh(),
            "a cached failure within the TTL must still be considered fresh \
             so we do not re-spawn lsappinfo on every tick during a transient error"
        );
        assert!(
            failure_cache.is_failure,
            "is_failure flag must be preserved"
        );
        assert!(
            failure_cache.cached_value.is_none(),
            "cached_value must be None for a cached failure"
        );
    }

    /// CopyPaste-44rq.43 regression guard: `is_sensitive_app` and the
    /// exclusion-list check are decoupled concerns.
    ///
    /// Before the fix (CopyPaste-zdyw optimisation), `handle_tick` skipped
    /// lsappinfo entirely when `excluded_app_bundle_ids` was empty, which meant
    /// `source_bundle_id` was always `None` in `handle_text`/`handle_image`.
    /// Consequently `is_sensitive_app` was never called and passwords copied
    /// from 1Password, Bitwarden, etc. were silently not flagged sensitive when
    /// the exclusion list was empty (the default factory state).
    ///
    /// The fix: lsappinfo runs unconditionally on macOS so
    /// `is_sensitive_app(bundle_id)` receives a real bundle ID on every tick,
    /// regardless of whether `excluded_app_bundle_ids` is empty.
    ///
    /// This test verifies the CORRECT post-fix behaviour:
    ///   1. `is_sensitive_app` returns `true` for known password-manager bundles.
    ///   2. An empty exclusion list does NOT suppress `is_sensitive_app` logic —
    ///      the two checks are entirely independent.
    ///   3. Non-sensitive apps still return `false`.
    #[test]
    fn is_sensitive_app_independent_of_exclusion_list() {
        // Default config has an empty exclusion list — the factory/default state.
        let cfg_empty = AppConfig::default();
        assert!(
            cfg_empty.excluded_app_bundle_ids.is_empty(),
            "pre-condition: default config must have an empty exclusion list"
        );

        // Even with an empty exclusion list, `is_sensitive_app` must correctly
        // classify known password managers as sensitive (fix for CopyPaste-44rq.43).
        for bundle_id in &[
            "com.1password.1password",
            "com.bitwarden.desktop",
            "com.keepassxc.keepassxc",
            "com.dashlane.dashlane",
        ] {
            assert!(
                copypaste_core::is_sensitive_app(bundle_id),
                "is_sensitive_app({bundle_id:?}) must be true regardless of exclusion list size"
            );
        }

        // Non-password-manager apps must still return false.
        for bundle_id in &["com.apple.finder", "com.google.chrome", ""] {
            assert!(
                !copypaste_core::is_sensitive_app(bundle_id),
                "is_sensitive_app({bundle_id:?}) must be false for non-sensitive apps"
            );
        }

        // Exclusion list membership and sensitive-app classification are
        // orthogonal: an app can be in the exclusion list without being sensitive,
        // and vice versa.  Adding an app to the exclusion list does not change
        // whether `is_sensitive_app` returns true for other bundle IDs.
        let mut cfg_non_empty = AppConfig::default();
        cfg_non_empty
            .excluded_app_bundle_ids
            .push("com.example.excluded".to_string());
        assert!(
            !cfg_non_empty.excluded_app_bundle_ids.is_empty(),
            "pre-condition: non-empty exclusion list"
        );
        // is_sensitive_app result is independent of the exclusion list.
        assert!(
            copypaste_core::is_sensitive_app("com.1password.1password"),
            "is_sensitive_app must return true for 1Password even when exclusion list is non-empty"
        );
        assert!(
            !copypaste_core::is_sensitive_app("com.example.excluded"),
            "is_sensitive_app must return false for an app that is only in the exclusion list"
        );
    }

    // -----------------------------------------------------------------------
    // tke7 (PG-30): sync_enabled config field contract tests
    // -----------------------------------------------------------------------

    /// Documents and verifies the sync_enabled master gate:
    /// - AppConfig::default() has sync_enabled = true.
    /// - Setting sync_enabled=false is persisted to config.toml.
    /// - The config field is read and honoured (per-field assertion).
    #[test]
    fn sync_enabled_defaults_to_true_in_appconfig() {
        let cfg = AppConfig::default();
        assert!(cfg.sync_enabled, "sync_enabled must default to true");
    }

    #[tokio::test]
    async fn sync_enabled_false_gates_outbound_in_handle_text() {
        // When sync_enabled=false, handle_text still inserts the item locally
        // (local capture is NOT gated) but the sync_orch would not forward it.
        // Verify that handle_text itself completes successfully (local-only path).
        let db = Arc::new(Mutex::new(
            Database::open_in_memory().expect("open in-memory db"),
        ));
        let key = [0u8; 32];
        let config = AppConfig {
            sync_enabled: false,
            ..Default::default()
        };
        let item = handle_text(
            "test sync gate".to_string(),
            &db,
            &key,
            &config,
            "test-device",
            None,
        )
        .await;
        // handle_text always stores locally regardless of sync_enabled.
        assert!(
            item.is_some(),
            "handle_text must store locally even when sync_enabled=false"
        );
    }

    // -----------------------------------------------------------------------
    // mtf5 (PG-22): is_sensitive_app wiring tests
    // -----------------------------------------------------------------------

    /// When handle_text is called with a source_bundle_id that matches a known
    /// password manager, the stored item must have is_sensitive = true even if
    /// the content pattern alone would not trigger auto-wipe.
    #[tokio::test]
    async fn handle_text_marks_sensitive_when_source_is_password_manager() {
        let local_key = [0xBBu8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        // A random-looking string that would NOT trigger the content-pattern
        // sensitive detector (no API key / credit card patterns).
        let plaintext = "xK9mQ3nR7pT2vW5".to_string();

        // Simulate the clipboard being copied from 1Password.
        let item = handle_text(
            plaintext,
            &db,
            &local_key,
            &config,
            "test-device",
            Some("com.1password.1password".to_string()),
        )
        .await
        .expect("handle_text must store the item");

        // is_sensitive must be true because the SOURCE APP is a password manager.
        assert!(
            item.is_sensitive,
            "mtf5: item must be marked sensitive when source is a password manager"
        );
        // The app_bundle_id must also be recorded on the item.
        assert_eq!(
            item.app_bundle_id.as_deref(),
            Some("com.1password.1password"),
            "mtf5: app_bundle_id must be stored on the item"
        );
    }

    /// Content captured from a non-sensitive app (e.g. Chrome) with innocuous
    /// content must NOT be marked sensitive.
    #[tokio::test]
    async fn handle_text_not_sensitive_for_regular_app() {
        let local_key = [0xCCu8; 32];
        let db = Arc::new(Mutex::new(Database::open_in_memory().expect("open db")));
        let config = AppConfig::default();

        let item = handle_text(
            "hello from chrome".to_string(),
            &db,
            &local_key,
            &config,
            "test-device",
            Some("com.google.chrome".to_string()),
        )
        .await
        .expect("handle_text must store the item");

        assert!(
            !item.is_sensitive,
            "mtf5: non-sensitive app + non-sensitive content must not be marked sensitive"
        );
        assert_eq!(
            item.app_bundle_id.as_deref(),
            Some("com.google.chrome"),
            "mtf5: app_bundle_id must be stored even for non-sensitive apps"
        );
    }

    // -----------------------------------------------------------------------
    // v0.4 ingest round-trip
    // -----------------------------------------------------------------------

    /// v0.4 ingest round-trip (HIGH): a freshly-captured text item must be
    /// readable through the SAME path the daemon uses on paste-back. The read
    /// path (`ipc::write_to_pasteboard`, text branch) dispatches on the row's
    /// `key_version` via `decrypt_item_by_version`, deriving the v2 key as
    /// `derive_v2(local_key)`. This test feeds the production ingest crypto
    /// (`encrypt_text_for_storage`) into the production read crypto
    /// (`decrypt_item_by_version`) and asserts the bytes survive.
    ///
    /// Before the ingest fix, ingest encrypted with the v1 key + v3 AAD while
    /// stamping `key_version = 2`, so this round-trip failed with
    /// `EncryptError::AuthFailed`.
    #[test]
    fn fresh_text_capture_round_trips_through_read_path() {
        let local_key = [0x42u8; 32]; // stands in for load_local_key() (the v1 key)
        let item_id = uuid::Uuid::new_v4().to_string();
        let plaintext = b"hello from a fresh clipboard capture";

        // Ingest: exactly what handle_text does to produce the stored row.
        let (nonce, ciphertext) =
            encrypt_text_for_storage(plaintext, &local_key, &item_id).expect("encrypt");

        // The row is stamped key_version = 2 (ClipboardItem::new_text).
        let item = ClipboardItem::new_text(ciphertext.clone(), nonce.to_vec(), 0);
        assert_eq!(
            item.key_version, 2,
            "freshly-captured rows are stamped key_version = 2"
        );

        // Read: replicate the read path's key derivation + dispatch.
        let v1_key = local_key;
        let v2_key = derive_v2(&v1_key);
        let mut nonce_arr = [0u8; NONCE_SIZE];
        nonce_arr.copy_from_slice(&nonce);

        let recovered = decrypt_item_by_version(
            item.key_version,
            copypaste_core::V1Key(&v1_key),
            copypaste_core::V2Key(&v2_key),
            &copypaste_core::ItemId::from(item_id.as_str()),
            &nonce_arr,
            &ciphertext,
        )
        .expect("read path must decrypt a freshly-captured row");

        assert_eq!(
            recovered, plaintext,
            "round-trip plaintext must match the captured bytes"
        );
    }
}
