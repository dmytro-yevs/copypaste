//! Clipboard/pasteboard helpers for the relay receive path, plus the Wi-Fi and
//! auto-apply guards.

// l07l: `Ordering` is only referenced from the macOS auto-apply store path;
// allow it unused on non-macOS so -D warnings stays green.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
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
    use super::{
        relay_fetch_auto_apply_candidate, relay_should_auto_apply, relay_should_skip_wifi,
    };
    use crate::relay::receive::ingest_page_blocking;
    use crate::relay::testutil::{make_pull_item, open_mem_db, skey};
    use crate::relay::watermark::Watermark;
    use copypaste_core::SyncKey;

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
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
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
            copypaste_core::config::MAX_DECODED_IMAGE_MB,
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
