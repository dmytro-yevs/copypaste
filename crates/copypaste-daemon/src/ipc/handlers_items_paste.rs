//! NSPasteboard paste-back writer (split from handlers_items.rs, ADR-017
//! daemon-ipc track, CopyPaste-vp63.15). macOS-gated; no-op on other
//! platforms.
//!
//! SECURITY: the text branch dispatches decrypt on `item.key_version`
//! (v1 raw seed vs `derive_v2`, AAD via `decrypt_item_by_version`). That
//! dispatch MUST move verbatim — do not "simplify" the key_version branch
//! (ADR-017 review checkpoint).
use super::*;

impl IpcServer {
    /// Write a clipboard item's *decrypted* content back to NSPasteboard
    /// (macOS) or no-op on other platforms.
    ///
    /// Audit CRIT #1 fix: the daemon stores every clipboard item encrypted
    /// (XChaCha20-Poly1305 for text, chunked AEAD for images) — the legacy
    /// implementation wrote `item.content` raw, so users saw ciphertext on
    /// paste. This now:
    ///
    /// 1. Decrypts text via [`copypaste_core::decrypt_item_with_aad`] with the per-item nonce,
    ///    rebuilding the AAD from the row's `item_id` so a tampered or
    ///    misbound ciphertext surfaces as `AuthFailed` instead of garbage.
    /// 2. Reassembles + decrypts image chunks via [`chunks_from_blob`] +
    ///    [`decode_image`], using the `file_id` parsed out of `blob_ref`.
    /// 3. Maps the daemon's internal `content_type` to a real macOS UTI
    ///    (`"image"` is **not** a valid UTI — audit HIGH #2). Text uses
    ///    `NSPasteboardTypeString`; image always writes `public.png` since
    ///    `encode_image` re-encodes raw clipboard bytes to PNG before
    ///    chunking. Anything already shaped like a UTI (`public.*`,
    ///    `com.*`, `org.*`) is passed through unchanged.
    pub(crate) async fn write_to_pasteboard(
        &self,
        item: &copypaste_core::ClipboardItem,
    ) -> Result<(), PasteboardError> {
        #[cfg(target_os = "macos")]
        {
            // crh3.77: the file branch writes up to 100 MiB of decrypted data to
            // the local filesystem (create_dir_all + fs::write). Running that on
            // the tokio async worker stalls the IPC loop for seconds on slow APFS.
            // The file branch runs its decode (CPU) synchronously then offloads the
            // blocking I/O to spawn_blocking; the NSPasteboard write happens in a
            // separate autoreleasepool afterwards. Text, image, and unknown branches
            // have no blocking I/O and remain in the existing autoreleasepool below.
            if item.content_type == "file" {
                // ── Part A: parse + decrypt (CPU, sync) ────────────────────────────
                let content = match &item.content {
                    Some(bytes) => bytes.as_slice(),
                    None => return Err(PasteboardError::other("item has no content")),
                };
                let meta_json = item
                    .blob_ref
                    .as_deref()
                    .ok_or_else(|| PasteboardError::other("file item missing blob_ref metadata"))?;
                let file_meta = parse_file_meta(meta_json).map_err(|e| {
                    PasteboardError::other(format!("file item blob_ref parse error: {e}"))
                })?;
                let chunks = chunks_from_blob(content).map_err(|e| {
                    PasteboardError::other(format!("file chunks_from_blob failed: {e}"))
                })?;
                // Dispatch on key_version: v1 rows use the raw seed; v2 rows use derive_v2.
                // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop.
                let v1_key = zeroize::Zeroizing::new(**self.local_key);
                let v2_key = derive_v2(&v1_key);
                let key_to_use: &[u8; 32] = if item.key_version == 1 {
                    &v1_key
                } else {
                    &v2_key
                };
                let raw_bytes = decode_file(&chunks, key_to_use, &file_meta.file_id)
                    .map_err(|e| PasteboardError::decrypt(format!("file decode failed: {e}")))?;
                // Sanitise the filename: strip any leading path separators so the
                // stored name cannot escape the cache directory.
                let safe_name = std::path::Path::new(&file_meta.filename)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("paste-file") // infallible fallback — filename came from our own capture
                    .to_string();

                // ── Part B: blocking fs I/O on a spawn_blocking thread (crh3.77) ──
                // raw_bytes (up to 100 MiB) is moved into the closure so the large
                // allocation is written from a dedicated blocking thread, not the
                // async worker. The `?` propagates PasteboardError through the async
                // fn's return type.
                let dest = tokio::task::spawn_blocking(move || {
                    let paste_dir = paste_file_cache_dir();
                    // Prune stale entries before writing so the directory stays bounded;
                    // errors inside prune are logged at DEBUG and never propagate.
                    prune_old_paste_files(&paste_dir);
                    std::fs::create_dir_all(&paste_dir).map_err(|e| {
                        PasteboardError::other(format!(
                            "failed to create paste-files dir {paste_dir:?}: {e}"
                        ))
                    })?;
                    let dest = paste_dir.join(&safe_name);
                    std::fs::write(&dest, &raw_bytes).map_err(|e| {
                        PasteboardError::other(format!("failed to write paste file {dest:?}: {e}"))
                    })?;
                    Ok::<_, PasteboardError>(dest)
                })
                .await
                .map_err(|e| {
                    // JoinError: spawn_blocking panicked or runtime is shutting down.
                    self.self_write_change_count
                        .store(-1, std::sync::atomic::Ordering::Release);
                    PasteboardError::other(format!(
                        "write_to_pasteboard blocking task panicked: {e}"
                    ))
                })??; // outer ? = JoinError mapped above; inner ? = PasteboardError from closure

                // ── Part C: NSPasteboard write (quick Cocoa calls) in autoreleasepool ──
                // The file is already on disk; this only constructs the NSURL and
                // writes the file-url string to the pasteboard.
                return objc2::rc::autoreleasepool(|_pool| {
                    use objc2_app_kit::NSPasteboard;
                    use objc2_foundation::{NSString, NSURL};

                    // Fix-4 (dup-on-copy race): stamp the self-write sentinel
                    // BEFORE calling clearContents/setString.
                    let pre_count =
                        unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    let expected_after_write = pre_count + 2;
                    self.self_write_change_count
                        .store(expected_after_write, std::sync::atomic::Ordering::Release);
                    let post_stamp = |self_write_cc: &Arc<std::sync::atomic::AtomicI64>| {
                        let actual =
                            unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                        if actual == expected_after_write {
                            self_write_cc.store(actual, std::sync::atomic::Ordering::Release);
                        }
                        tracing::debug!(
                            change_count = actual,
                            expected = expected_after_write,
                            racing_write = actual != expected_after_write,
                            "clipboard: post-write changeCount check (self-write sentinel)"
                        );
                    };

                    // Build the file:// URL string for the temp file.
                    // `public.file-url` data is the absolute URL string (percent-encoded),
                    // e.g. "file:///Users/.../paste-files/foo.txt".  This is what Finder,
                    // Terminal, and most Cocoa apps accept when reading `public.file-url`
                    // from the pasteboard.  We construct it via NSURL so percent-encoding
                    // is handled correctly, then write the absolute-string as NSString data.
                    let file_url_str: String = unsafe {
                        let path_ns = NSString::from_str(
                            dest.to_str().unwrap_or_default(), // UTF-8 path; infallible on macOS
                        );
                        // fileURLWithPath: produces "file:///…" with proper percent-encoding.
                        let nsurl = NSURL::fileURLWithPath(&path_ns);
                        // absoluteString returns the full URL string; unwrap_or_default is
                        // infallible in practice — a file URL always has an absolute string.
                        nsurl
                            .absoluteString()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("file://{}", dest.display()))
                    };
                    let write_ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let uti = NSString::from_str("public.file-url");
                        let url_ns = NSString::from_str(&file_url_str);
                        pb.setString_forType(&url_ns, &uti)
                    };
                    if !write_ok {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        return Err(PasteboardError::other(
                            "NSPasteboard setString:forType: returned false for public.file-url",
                        ));
                    }
                    post_stamp(&self.self_write_change_count);
                    Ok(())
                });
            }

            // Non-file branches (text, image, unknown): synchronous Cocoa calls
            // with no blocking fs I/O. Drain the autorelease pool around the Cocoa
            // body to prevent leaks of autoreleased objects on the tokio worker
            // thread — the same leak class fixed in `clipboard.rs::poll`.
            objc2::rc::autoreleasepool(|_pool| {
                let content = match &item.content {
                    Some(bytes) => bytes.as_slice(),
                    None => return Err(PasteboardError::other("item has no content")),
                };

                use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
                use objc2_foundation::{NSData, NSString};

                // Fix-4 (dup-on-copy race): stamp the self-write sentinel
                // BEFORE calling clearContents/setString so the clipboard
                // monitor can never observe the new changeCount with a stale
                // (un-set) sentinel.
                //
                // Previous code read changeCount AFTER the write and stored
                // it — a poll arriving between the write and the store would
                // see an incremented changeCount with sentinel == -1 and
                // record the just-pasted item as a fresh capture.
                //
                // Fix: read the current changeCount, pre-stamp
                // `current + 2` as the expected post-write value
                // (`clearContents` adds 1, `setString_forType` /
                // `setData_forType` adds 1 more), then write. After the
                // write, overwrite with the actual new count (handles cases
                // where macOS increments by a different amount). On error,
                // reset the sentinel to -1 so the monitor is not permanently
                // suppressed.
                let pre_count = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                // Pre-stamp with current+2 (the expected post-clearContents +
                // post-setString count). The monitor polls only on a 500ms
                // interval so a pre-stamp that is off by one is still safer
                // than a window with no stamp at all.
                let expected_after_write = pre_count + 2;
                self.self_write_change_count
                    .store(expected_after_write, std::sync::atomic::Ordering::Release);

                // Helper to post-stamp with the actual post-write count.
                //
                // CopyPaste-8yzf: only overwrite the sentinel when the
                // post-write count equals `expected_after_write`. If a
                // third-party app wrote to the pasteboard between our write
                // and this read, `actual > expected_after_write`. In that
                // case we leave the sentinel at `expected_after_write` (which
                // the monitor may have already consumed or will not see again
                // because the count moved past it). Unconditionally storing
                // `actual` would stamp the third-party's count, causing the
                // monitor to suppress their content as a daemon self-write.
                let post_stamp = |self_write_cc: &Arc<std::sync::atomic::AtomicI64>| {
                    let actual = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    if actual == expected_after_write {
                        // Our write was the only one; safe to confirm the exact count.
                        self_write_cc.store(actual, std::sync::atomic::Ordering::Release);
                    }
                    // else: third-party wrote after us; leave the pre-stamp
                    // (`expected_after_write`) in place — it will either
                    // already have been consumed by the monitor, or it is
                    // stale and harmless (no future poll will see it).
                    tracing::debug!(
                        change_count = actual,
                        expected = expected_after_write,
                        racing_write = actual != expected_after_write,
                        "clipboard: post-write changeCount check (self-write sentinel)"
                    );
                };

                if item.content_type == "text" {
                    // ----- text: decrypt per-item ciphertext, then write -----
                    let nonce_vec = item
                        .content_nonce
                        .as_ref()
                        .ok_or_else(|| PasteboardError::other("text item missing content_nonce"))?;
                    let nonce: &[u8; 24] = nonce_vec.as_slice().try_into().map_err(|_| {
                        PasteboardError::other(format!(
                            "text item content_nonce wrong length: expected 24, got {}",
                            nonce_vec.len()
                        ))
                    })?;

                    // Dispatch decrypt on the row's key_version so ciphertexts
                    // produced under different HKDF key families are always
                    // decrypted with the matching key and AAD format:
                    //
                    //   key_version = 1 → v1 key (local_enc_key / HKDF-SHA-256),
                    //                     AAD = build_item_aad(item_id, 3)
                    //   key_version = 2 → v2 key (derive_v2 / HKDF-SHA-512),
                    //                     AAD = build_item_aad_v2(item_id, 4, 2)
                    //   other           → UnknownKeyVersion → auth_failed error
                    //
                    // Previously this always used the v1 AAD regardless of
                    // key_version, so any item written with key_version = 2 (the
                    // current default since ITEM_KEY_VERSION_CURRENT = 2) would
                    // fail with "authentication tag mismatch" on paste-back.
                    //
                    // Note: IpcServer only holds one key (local_key = v1 key from
                    // Keychain). key_version = 2 items are derived from the same
                    // seed via derive_v2; we derive it inline here so the server
                    // struct does not need a second Arc field.
                    // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop.
                    let v1_key = zeroize::Zeroizing::new(**self.local_key);
                    let v2_key = derive_v2(&v1_key);
                    let plaintext_bytes = decrypt_item_by_version(
                        item.key_version,
                        V1Key(&v1_key),
                        V2Key(&v2_key),
                        &item.item_id,
                        nonce,
                        content,
                    )
                    .map_err(|e| {
                        // On decrypt failure reset the sentinel so the monitor
                        // is not permanently suppressed (Fix-4 error path).
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        match e {
                            EncryptError::AuthFailed => PasteboardError::decrypt(
                                "Decryption failed: authentication tag mismatch".to_string(),
                            ),
                            EncryptError::UnknownKeyVersion(_) => PasteboardError::decrypt(
                                "Item encrypted with a previous key — cannot be recovered. \
                                 Clear history to start fresh."
                                    .to_string(),
                            ),
                            other => PasteboardError::decrypt(other.to_string()),
                        }
                    })?;
                    let text = std::str::from_utf8(&plaintext_bytes).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::decrypt(format!("decrypted content is not UTF-8: {e}"))
                    })?;

                    // paste_as_plain_text: read the live config flag. When true,
                    // write only `public.utf8-plain-text` (strips RTF/HTML/attributed
                    // strings from the pasteboard so the receiving app gets bare text).
                    // When false (default), use NSPasteboardTypeString which is the
                    // standard "general string" UTI that most apps expect.
                    let plain_only = self
                        .core_config
                        .as_ref()
                        .and_then(|arc| arc.read().ok())
                        .map(|cfg| cfg.paste_as_plain_text)
                        .unwrap_or(false);

                    unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let ns_str = NSString::from_str(text);
                        // `public.utf8-plain-text` is the "bare UTF-8" UTI that
                        // explicitly strips rich formatting (RTF, HTML, etc.) on
                        // paste. NSPasteboardTypeString is also `public.utf8-plain-text`
                        // on modern macOS, but using the explicit UTI literal when
                        // paste_as_plain_text=true makes the intent unambiguous and
                        // avoids any implicit coercion bridges the system type may carry.
                        let ok = if plain_only {
                            let plain_uti = NSString::from_str("public.utf8-plain-text");
                            pb.setString_forType(&ns_str, &plain_uti)
                        } else {
                            pb.setString_forType(&ns_str, NSPasteboardTypeString)
                        };
                        if !ok {
                            // Fix-4: reset the self-write sentinel on write failure so
                            // a failed paste does not leave a stale changeCount that
                            // suppresses a later genuine capture.
                            self.self_write_change_count
                                .store(-1, std::sync::atomic::Ordering::Release);
                            return Err(PasteboardError::other(
                                "NSPasteboard setString:forType: returned false",
                            ));
                        }
                    }
                    post_stamp(&self.self_write_change_count);
                    Ok(())
                } else if item.content_type == "image" {
                    // ----- image: reassemble chunks → decrypt → write as PNG -----
                    // `file_id` is embedded in the JSON metadata stored in
                    // `blob_ref` (see ClipboardItem::new_image in
                    // storage/items.rs).
                    let meta_json = item.blob_ref.as_deref().ok_or_else(|| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other("image item missing blob_ref metadata")
                    })?;
                    let file_id = parse_image_file_id(meta_json).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other(e)
                    })?;

                    let chunks = chunks_from_blob(content).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other(format!("image chunks_from_blob failed: {e}"))
                    })?;
                    // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop.
                    let wtp_v1_key = zeroize::Zeroizing::new(**self.local_key);
                    let wtp_v2_key = derive_v2(&wtp_v1_key);
                    let wtp_img_key: &[u8; 32] = if item.key_version == 1 {
                        &wtp_v1_key
                    } else {
                        &wtp_v2_key
                    };
                    let png_bytes = decode_image(&chunks, wtp_img_key, &file_id).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::decrypt(format!("image decode failed: {e}"))
                    })?;

                    let write_ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let type_str = NSString::from_str("public.png");
                        let data = NSData::with_bytes(&png_bytes);
                        pb.setData_forType(Some(&data), &type_str)
                    };
                    if !write_ok {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        return Err(PasteboardError::other(
                            "NSPasteboard setData:forType: returned false for public.png",
                        ));
                    }
                    post_stamp(&self.self_write_change_count);
                    Ok(())
                } else {
                    // Unknown content_type — keep a best-effort raw-bytes write,
                    // but map to a real UTI when possible. We do NOT attempt
                    // decryption here because we don't know the shape of the
                    // ciphertext (no nonce / no chunk metadata). Used only by
                    // future content_types added without updating this handler.
                    let uti = map_content_type_to_uti(&item.content_type);
                    let write_ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let type_str = NSString::from_str(&uti);
                        let data = NSData::with_bytes(content);
                        pb.setData_forType(Some(&data), &type_str)
                    };
                    if !write_ok {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        return Err(PasteboardError::other(format!(
                            "NSPasteboard setData:forType: returned false for type '{uti}'"
                        )));
                    }
                    post_stamp(&self.self_write_change_count);
                    Ok(())
                }
            })
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = item;
            // No clipboard support on non-macOS platforms in this crate
            Ok(())
        }
    }
}
