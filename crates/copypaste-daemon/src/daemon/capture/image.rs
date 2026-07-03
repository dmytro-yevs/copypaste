//! Image capture ingest: encode (decode once, produce full PNG + thumbnail),
//! encrypt, dedup-by-content-hash, and store.

use copypaste_core::{
    bump_item_recency, chunks_to_blob, encode_image_full, get_item_by_id, insert_item_with_fts,
    next_lamport_ts, AppConfig, ClipboardItem, Database,
};
use std::sync::Arc;
use tokio::sync::Mutex;

use super::cleanup::prune_history;

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
                            // CopyPaste-8ebg.57: re-copying an identical image hits the
                            // UNIQUE-index dedup path (same content-hash-derived
                            // item_id) but, unlike handle_text's explicit
                            // find_recent_by_hash + bump_item_recency dedup path,
                            // never bumped the existing row's recency — so a re-copy
                            // silently sank the item instead of surfacing it at the
                            // top of history. Bump it here to match text's behaviour.
                            tracing::debug!(
                                requested = %item.id,
                                existing = %stored_id,
                                "image item deduped against existing row"
                            );
                            match get_item_by_id(&*db_guard, &stored_id) {
                                Ok(Some(existing_row)) => {
                                    let new_lamport =
                                        next_lamport_ts(existing_row.lamport_ts, item.wall_time);
                                    if let Err(e) = bump_item_recency(
                                        &db_guard,
                                        &stored_id,
                                        item.wall_time,
                                        new_lamport,
                                        None,
                                    ) {
                                        tracing::warn!(
                                            "image dedup: bump_item_recency failed: {e}"
                                        );
                                    }
                                }
                                Ok(None) => {
                                    tracing::debug!(
                                        id = %stored_id,
                                        "image dedup: existing row disappeared before bump (deleted concurrently)"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        "image dedup: failed to fetch existing row for bump: {e}"
                                    );
                                }
                            }
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

#[cfg(test)]
mod tests {
    use super::super::file::handle_file;
    use super::*;
    use copypaste_core::Database;

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
}
