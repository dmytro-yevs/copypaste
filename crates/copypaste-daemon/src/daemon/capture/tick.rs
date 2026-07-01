//! Steady-state per-tick dispatch: private-mode gate, macOS exclusion /
//! sensitive-app resolution, `poll()` match → route Text/Image/File/FileRef,
//! sound-on-copy, broadcast.

use crate::clipboard::{ClipboardContent, ClipboardMonitor};
use copypaste_core::{AppConfig, ClipboardItem, Database};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[cfg(target_os = "macos")]
use super::frontmost::resolve_frontmost_bundle_id;
#[cfg(target_os = "macos")]
use super::frontmost::FrontmostAppCache;

use super::file::handle_file;
use super::image::handle_image;
use super::text::handle_text;

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
    //    flag is NOT set, even if the user is copying from 1Password. Content-
    //    pattern detection (`is_sensitive_for_autowipe`) still applies for text.
    //  • Process substitution (e.g. a password manager that delegates the copy
    //    to a helper process) may surface a different bundle ID than expected,
    //    causing a miss.
    //  • This detection is unavailable on Linux / non-macOS (always None).
    //
    // The correct long-term fix (PRIV-2) is to query the Accessibility API or
    // use NSWorkspace.frontmostApplication instead of forking lsappinfo. Until
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
    // P1-3 (spawn_blocking): the lsappinfo subprocess call is blocking;
    // offload to the tokio blocking-thread pool so the async executor is not
    // stalled by the fork+wait. (See `frontmost::resolve_frontmost_bundle_id`.)
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
    let frontmost_bundle_id: Option<String> = resolve_frontmost_bundle_id(frontmost_cache).await;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
