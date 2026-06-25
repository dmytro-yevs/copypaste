//! Clipboard/pasteboard helpers for the relay receive path, plus the Wi-Fi and
//! auto-apply guards.

use std::sync::{
    atomic::{AtomicI64, Ordering},
    Arc,
};

use copypaste_core::{ClipboardItem, Database};

// ── Settings guards ───────────────────────────────────────────────────────────

/// Returns `true` when the current tick should be skipped due to the Wi-Fi-only
/// setting being active and the device not being on Wi-Fi.
///
/// Pure function — injectable `is_on_wifi_fn` makes this unit-testable without
/// a real `networksetup` invocation. Mirrors the guard in `cloud.rs`.
///
/// Delegates to [`crate::sync_common::should_skip_on_cellular`], which is the
/// canonical single implementation of this logic shared by the cloud, relay, and
/// P2P paths (CopyPaste-hao6 de-dup).
pub(super) fn relay_should_skip_wifi(sync_on_wifi_only: bool, is_on_wifi: bool) -> bool {
    crate::sync_common::should_skip_on_cellular(sync_on_wifi_only, is_on_wifi)
}

/// Returns `true` when the relay receive path should auto-apply a freshly-synced
/// item to the local pasteboard, i.e. when `auto_apply_synced_clip` is enabled.
///
/// Pure function — testable without a live `AppConfig` instance.
pub(super) fn relay_should_auto_apply(auto_apply_synced_clip: bool) -> bool {
    auto_apply_synced_clip
}

/// Candidate for auto-applying to the local pasteboard after a relay ingest.
///
/// Carries enough information for [`relay_apply_to_pasteboard`] to write the
/// item to NSPasteboard (macOS) without re-querying the DB or re-decrypting.
pub(super) struct AutoApplyCandidate {
    pub(super) wall_time: i64,
    pub(super) plaintext: Vec<u8>,
    pub(super) content_type: String,
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
pub(super) fn relay_fetch_auto_apply_candidate(
    db: &Database,
    local_key: &zeroize::Zeroizing<[u8; 32]>,
) -> Option<AutoApplyCandidate> {
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
pub(super) fn relay_apply_to_pasteboard(
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

#[cfg(test)]
mod tests {
    use super::relay_should_skip_wifi;

    /// Parity contract: relay_should_skip_wifi must agree with
    /// sync_common::should_skip_on_cellular on all four (bool, bool) inputs
    /// (CopyPaste-hao6 de-dup guard — catches any future body drift between the
    /// relay wrapper and the canonical shared helper).
    #[test]
    fn relay_skip_wifi_delegates_to_canonical_helper() {
        let cases = [(true, false), (true, true), (false, false), (false, true)];
        for (wifi_only, on_wifi) in cases {
            assert_eq!(
                relay_should_skip_wifi(wifi_only, on_wifi),
                crate::sync_common::should_skip_on_cellular(wifi_only, on_wifi),
                "relay_should_skip_wifi({wifi_only}, {on_wifi}) must equal \
                 should_skip_on_cellular({wifi_only}, {on_wifi})"
            );
        }
    }
}
