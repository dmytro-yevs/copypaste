// l07l: AtomicI64 is only exercised by the macOS pasteboard change-count path;
// allow it unused on non-macOS so -D warnings stays green.
// CopyPaste-54it #9: sentinel stamp/reset calls now go through
// `crate::fs_atomic::{sentinel_pre_stamp, sentinel_post_stamp, sentinel_reset}`
// (consolidated from the pattern in `ipc::mod::write_to_pasteboard`). The
// helper's post-stamp is the stronger CopyPaste-8yzf variant: it only updates
// the sentinel when `actual == expected`, preventing a racing third-party write
// from being silently suppressed.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
use std::sync::atomic::AtomicI64;
use std::sync::Arc;

use copypaste_core::Database;
// l07l: `warn` is only emitted from the macOS auto-apply path; allow it unused
// on non-macOS so -D warnings stays green (`debug` is used unconditionally).
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
use tracing::{debug, warn};

/// Write decrypted plaintext for a synced item directly to NSPasteboard.
///
/// Called from [`crate::sync_orch::merge::merge_incoming_with_crypto`] after determining that the
/// incoming item is the freshest thing in the DB and `auto_apply_synced_clip`
/// is enabled.
///
/// # Loop prevention
///
/// The self-write guard works identically to the `copy_item` IPC handler:
/// 1. Read the *current* NSPasteboard `changeCount` (pre-write).
/// 2. Pre-stamp the expected post-write value (`current + 2`) into
///    `self_write_change_count` **before** calling `clearContents` /
///    `setString_forType`, so no poll arriving between the write and the stamp
///    can slip through with a stale sentinel.
/// 3. After the write, overwrite with the *actual* new `changeCount` so a
///    macOS increment that differs from our prediction is handled correctly.
/// 4. On any failure reset the sentinel to `-1` to avoid permanent suppression.
///
/// # Content types
///
/// * `text` — writes `NSPasteboardTypeString`.  `plaintext` is the raw UTF-8
///   bytes returned by `rekey_inbound`.
/// * `image` — `plaintext` is an empty-vec sentinel (set in the merge loop
///   because the full PNG was not re-materialised there).  We re-decode it
///   here from the stored chunks in the DB row.
/// * `file` — **skipped** (a temp-file round-trip is required; deferred).
///   Logged at DEBUG.
///
/// All Cocoa calls are wrapped in `autoreleasepool` to prevent Objective-C
/// object leaks on this tokio blocking thread (mirrors `clipboard.rs::poll` and
/// `ipc.rs::write_to_pasteboard`).
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub(super) fn apply_to_pasteboard_if_fresh(
    db: &Database,
    content_type: &str,
    plaintext: Vec<u8>,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
    self_write_change_count: &Arc<AtomicI64>,
) {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::NSPasteboard;

        match content_type {
            "text" => {
                let text = match std::str::from_utf8(&plaintext) {
                    Ok(s) => s,
                    Err(e) => {
                        warn!("sync_orch: auto-apply text is not UTF-8: {e}");
                        return;
                    }
                };
                objc2::rc::autoreleasepool(|_pool| {
                    use objc2_app_kit::NSPasteboardTypeString;
                    use objc2_foundation::NSString;

                    // Pre-stamp expected changeCount before any NSPasteboard
                    // mutation (clearContents +1, setString +1 = pre+2).
                    let pre = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    let expected =
                        crate::fs_atomic::sentinel_pre_stamp(self_write_change_count, pre);

                    let ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let ns_str = NSString::from_str(text);
                        pb.setString_forType(&ns_str, NSPasteboardTypeString)
                    };
                    if ok {
                        // Post-stamp: only update if no third-party write raced
                        // (CopyPaste-8yzf guard — see fs_atomic::sentinel_post_stamp).
                        let actual =
                            unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                        crate::fs_atomic::sentinel_post_stamp(
                            self_write_change_count,
                            actual,
                            expected,
                        );
                        debug!(
                            change_count = actual,
                            "sync_orch: auto-applied synced text to NSPasteboard"
                        );
                    } else {
                        // Reset sentinel so the monitor is not permanently suppressed.
                        crate::fs_atomic::sentinel_reset(self_write_change_count);
                        warn!("sync_orch: auto-apply text: NSPasteboard setString:forType: returned false");
                    }
                });
            }
            "image" => {
                // `plaintext` is an empty-vec sentinel from the merge loop
                // (the PNG was not re-materialised there to avoid a second
                // decode pass).  Recover the PNG from the most-recent image
                // row in the DB — it was just inserted/updated by this merge.
                let png_bytes = recover_latest_image_png(db, local_key);
                let png_bytes = match png_bytes {
                    Some(b) => b,
                    None => {
                        warn!("sync_orch: auto-apply image: could not recover PNG from DB");
                        return;
                    }
                };
                objc2::rc::autoreleasepool(|_pool| {
                    use objc2_foundation::{NSData, NSString};

                    // Pre-stamp before any NSPasteboard mutation.
                    let pre = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    let expected =
                        crate::fs_atomic::sentinel_pre_stamp(self_write_change_count, pre);

                    let ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let type_str = NSString::from_str("public.png");
                        let data = NSData::with_bytes(&png_bytes);
                        pb.setData_forType(Some(&data), &type_str)
                    };
                    if ok {
                        // Post-stamp: only update if no third-party write raced
                        // (CopyPaste-8yzf guard — see fs_atomic::sentinel_post_stamp).
                        let actual =
                            unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                        crate::fs_atomic::sentinel_post_stamp(
                            self_write_change_count,
                            actual,
                            expected,
                        );
                        debug!(
                            change_count = actual,
                            "sync_orch: auto-applied synced image to NSPasteboard"
                        );
                    } else {
                        crate::fs_atomic::sentinel_reset(self_write_change_count);
                        warn!("sync_orch: auto-apply image: NSPasteboard setData:forType: returned false");
                    }
                });
            }
            "file" => {
                // Files require writing bytes to a temp file and placing its
                // file-URL on the pasteboard — deferred to a future iteration.
                debug!("sync_orch: auto-apply skipped for file item (not yet supported)");
            }
            other => {
                debug!(
                    content_type = other,
                    "sync_orch: auto-apply skipped for unknown content_type"
                );
            }
        }
    }

    #[cfg(not(target_os = "macos"))]
    {
        // Non-macOS: no NSPasteboard. No-op — called only on macOS in production.
        debug!(content_type, "sync_orch: auto-apply skipped (not macOS)");
    }
}

/// Recover PNG bytes for the most-recently-inserted image row from the DB.
///
/// Used by [`apply_to_pasteboard_if_fresh`] for the image auto-apply path.
/// Reads the newest image row's chunk blob + blob_ref, decodes with `local_key`
/// (v1 seed, the chunk-encryption key), and returns the raw PNG bytes.
/// Returns `None` on any parse/decrypt failure (logged at DEBUG).
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(super) fn recover_latest_image_png(
    db: &Database,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Option<Vec<u8>> {
    use copypaste_core::{chunks_from_blob, decode_image};

    // Fetch the most recent image row (just inserted by this merge).
    let (content, blob_ref): (Vec<u8>, String) = db
        .conn()
        .query_row(
            "SELECT content, blob_ref FROM clipboard_items \
             WHERE content_type = 'image' AND content IS NOT NULL AND blob_ref IS NOT NULL \
             ORDER BY wall_time DESC, id DESC LIMIT 1",
            [],
            |r| Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, String>(1)?)),
        )
        .ok()?;

    let file_id = crate::ipc::parse_image_file_id(&blob_ref)
        .map_err(|e| {
            debug!("sync_orch: auto-apply image: blob_ref parse failed: {e}");
        })
        .ok()?;

    let chunks = chunks_from_blob(&content)
        .map_err(|e| {
            debug!("sync_orch: auto-apply image: chunks_from_blob failed: {e}");
        })
        .ok()?;

    decode_image(&chunks, local_key, &file_id)
        .map_err(|e| {
            debug!("sync_orch: auto-apply image: decode_image failed: {e}");
        })
        .ok()
}
