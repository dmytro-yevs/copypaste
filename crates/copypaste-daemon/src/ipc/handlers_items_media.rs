//! Clipboard-item media (image/thumbnail/file) decrypt IPC verbs (split from
//! handlers_items.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.15).
//!
//! SECURITY: these handlers dispatch decrypt on the row's `key_version`
//! (v1 raw seed vs `derive_v2`). That dispatch MUST move verbatim — do not
//! "simplify" the key_version branch (ADR-017 review checkpoint).
use super::*;

impl IpcServer {
    // A. get_item_image — decrypt and return an IMAGE item as a data URI.
    //
    // Params: {"id": "<uuid>"}
    // Success: {"data_uri": "data:<content_type>;base64,<b64>"}
    // Error: item not found, non-image content_type, or decrypt failure.
    //
    // Reuses the same chunk-decrypt path as write_to_pasteboard for images
    // (chunks_from_blob → decode_image → PNG bytes), then base64-encodes
    // the raw PNG bytes for the UI to render as a thumbnail without having
    // to hit the pasteboard.
    pub(crate) async fn handle_get_item_image(&self, req: Request) -> Response {
        let id = match extract_uuid_param(&req.params, req.id.clone()) {
            Ok(id) => id,
            Err(resp) => return resp,
        };
        let db_arc = self.db.clone();
        let id_for_task = id.clone();
        // CopyPaste-z1xt: do the WHOLE pipeline — DB fetch, decrypt
        // (decode_image), and base64 — inside spawn_blocking. Previously
        // only the DB fetch ran on the blocking pool; the CPU-heavy
        // decrypt + base64 ran on the async executor thread, stalling it.
        // CopyPaste-eq9m: encode directly from the decrypted `png_bytes`
        // slice and DROP it before building the data URI so peak RAM is
        // one decoded copy + one base64 string, not both plus the URI;
        // we also move `item.content` out instead of `.clone()`-ing the
        // full encrypted blob.
        // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
        // even if the spawn_blocking worker panics or is cancelled.
        let v1_key = zeroize::Zeroizing::new(**self.local_key);
        // ItemImageResult mirrors the response branches so error mapping
        // stays on the async side (Response::* needs `req.id`).
        enum ItemImageResult {
            Ok(String),
            NotFound,
            NotImage(String),
            Internal(String),
            Auth(String),
        }
        let join = tokio::task::spawn_blocking(move || -> anyhow::Result<ItemImageResult> {
            let item = {
                let db = db_arc.blocking_lock();
                get_item_by_id(&*db, &id_for_task)?
            };
            let mut item = match item {
                Some(it) => it,
                None => return Ok(ItemImageResult::NotFound),
            };
            let is_image = item.content_type == "image" || item.content_type.starts_with("image/");
            if !is_image {
                return Ok(ItemImageResult::NotImage(format!(
                    "item {id_for_task} is not an image (content_type: {})",
                    item.content_type
                )));
            }
            // Move the encrypted blob out of the item — no extra clone.
            let content = match item.content.take() {
                Some(b) => b,
                None => {
                    return Ok(ItemImageResult::Internal(format!(
                        "image item {id_for_task} has no content blob"
                    )))
                }
            };
            let meta_json = match item.blob_ref.as_deref() {
                Some(s) => s,
                None => {
                    return Ok(ItemImageResult::Internal(format!(
                        "image item {id_for_task} missing blob_ref metadata"
                    )))
                }
            };
            let file_id = match parse_image_file_id(meta_json) {
                Ok(fid) => fid,
                Err(e) => {
                    return Ok(ItemImageResult::Internal(format!(
                        "image item {id_for_task} blob_ref parse error: {e}"
                    )))
                }
            };
            let chunks = match chunks_from_blob(&content) {
                Ok(c) => c,
                Err(e) => {
                    return Ok(ItemImageResult::Internal(format!(
                        "image item {id_for_task} chunks_from_blob failed: {e}"
                    )))
                }
            };
            let v2_key = derive_v2(&v1_key);
            let key_to_use: &[u8; 32] = if item.key_version == 1 {
                &v1_key
            } else {
                &v2_key
            };
            let png_bytes = match decode_image(&chunks, key_to_use, &file_id) {
                Ok(b) => b,
                Err(e) => {
                    return Ok(ItemImageResult::Auth(format!(
                        "image item {id_for_task} decode failed: {e}"
                    )))
                }
            };
            use base64::Engine as _;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
            // CopyPaste-eq9m: free the decoded image bytes before we build
            // the URI so the base64 string is the only large allocation
            // still alive when we format the data URI.
            drop(png_bytes);
            // The stored content_type is "image" (legacy) or a real MIME
            // type. For the data URI we always emit "image/png" because
            // decode_image always returns PNG bytes.
            let data_uri = format!("data:image/png;base64,{b64}");
            Ok(ItemImageResult::Ok(data_uri))
        })
        .await;
        match join {
            Ok(Ok(ItemImageResult::Ok(data_uri))) => {
                Response::ok(req.id, serde_json::json!({ "data_uri": data_uri }))
            }
            Ok(Ok(ItemImageResult::NotFound)) => {
                Response::err_with_code(req.id, ERR_CODE_NOT_FOUND, format!("item not found: {id}"))
            }
            Ok(Ok(ItemImageResult::NotImage(msg))) => {
                Response::err_with_code(req.id, ERR_CODE_INVALID_ARGUMENT, msg)
            }
            Ok(Ok(ItemImageResult::Internal(msg))) => {
                Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
            }
            Ok(Ok(ItemImageResult::Auth(msg))) => {
                Response::err_with_code(req.id, ERR_CODE_AUTH_FAILED, msg)
            }
            Ok(Err(e)) => Response::err(req.id, e.to_string()),
            Err(e) => Response::err_with_code(
                req.id,
                ERR_CODE_INTERNAL_ERROR,
                format!("blocking task failed: {e}"),
            ),
        }
    }

    // A'. get_item_thumbnail — decrypt and return the small capture-time
    // thumbnail as a data URI. Mirrors `get_item_image` but reads
    // `item.thumb` (keyed by the DISTINCT `thumb_file_id` in the meta)
    // instead of the full-res `item.content`.
    //
    // Params: {"id": "<uuid>"}
    // Success (thumb present): {"thumbnail": "data:image/png;base64,<b64>"}
    // Success (no thumb):      {"thumbnail": null}   ← UI falls back to
    //                          get_item_image (full-res).
    // Error: item not found, non-image content_type, parse/decode failure.
    pub(crate) async fn handle_get_item_thumbnail(&self, req: Request) -> Response {
        let id = match extract_uuid_param(&req.params, req.id.clone()) {
            Ok(id) => id,
            Err(resp) => return resp,
        };
        let db_arc = self.db.clone();
        let id_for_task = id.clone();
        // P2-iqkm: capture as Zeroizing so the key copy is wiped on drop
        // even if the spawn_blocking worker panics or is cancelled.
        // (Zeroizing<[u8;32]> is Send; the old "not Send" comment was incorrect.)
        let v1_key_thumb = zeroize::Zeroizing::new(**self.local_key);
        // All DB work — fetch + optional Phase-4 lazy backfill + decrypt —
        // runs in a single spawn_blocking so we hold the mutex for one
        // contiguous span and avoid async/sync boundary issues.
        // Returns: Ok(Some((png_bytes, data_uri_string))) on success,
        //          Ok(None) when item not found,
        //          Err for wrong content_type or missing blob_ref.
        let join = tokio::task::spawn_blocking(move || {
            let db = db_arc.blocking_lock();
            let item = match get_item_by_id(&*db, &id_for_task)? {
                Some(i) => i,
                None => return Ok::<_, anyhow::Error>(None),
            };
            // Dispatch on key_version: v1 rows use the raw seed; v2 rows use derive_v2.
            let v2_key_thumb = derive_v2(&v1_key_thumb);
            let decode_key: &[u8; 32] = if item.key_version == 1 {
                &v1_key_thumb
            } else {
                &v2_key_thumb
            };

            let is_image = item.content_type == "image" || item.content_type.starts_with("image/");
            if !is_image {
                return Err(anyhow::anyhow!(
                    "item {} is not an image (content_type: {})",
                    id_for_task,
                    item.content_type
                ));
            }

            let mut meta_json = item
                .blob_ref
                .as_deref()
                .ok_or_else(|| {
                    anyhow::anyhow!("image item {} missing blob_ref metadata", id_for_task)
                })?
                .to_owned();

            // Resolve the thumbnail blob: use the stored one when present
            // AND it conforms to the current THUMBNAIL_MAX_DIM cap.
            // Regenerate (Phase-4 backfill path) when either:
            //   * thumb IS NULL (legacy row, never had a thumbnail), or
            //   * the stored thumbnail was encoded under an older, larger
            //     cap (e.g. 680 px) and its recorded dims exceed the new
            //     cap — otherwise the UI would decode an oversized bitmap
            //     (HB-10, 350 MB image-memory regression).
            let stored_thumb: Option<Vec<u8>> = match item.thumb {
                Some(b) => {
                    let (tw, th) = parse_image_thumb_dims(&meta_json);
                    if copypaste_core::thumb_dims_exceed_cap(tw, th) {
                        tracing::debug!(
                            item_id = %id_for_task,
                            thumb_w = tw,
                            thumb_h = th,
                            "stored thumbnail exceeds current cap; regenerating"
                        );
                        None // fall through to regeneration below
                    } else {
                        Some(b)
                    }
                }
                None => None,
            };
            let thumb_blob: Vec<u8> = match stored_thumb {
                Some(b) => b,
                None => {
                    // Phase 4 lazy backfill: generate + persist a
                    // thumbnail on first display (NULL thumb) OR
                    // regenerate an oversized one at the current cap.
                    // `set_thumb` overwrites any existing row, so an
                    // oversized stored thumbnail is replaced in place.
                    // Returns both the
                    // encrypted blob and the updated meta_json (which
                    // now includes thumb_file_id / thumb_w / thumb_h)
                    // so the subsequent decode path reads the right AAD.
                    // Any failure is logged and falls back to the null
                    // sentinel — we never error the request.
                    // content is Option<Vec<u8>>; for image items it is
                    // always Some (set at capture), so None here means
                    // the row is corrupt — treat it as backfill failure.
                    let content_ref: &[u8] = match item.content.as_deref() {
                        Some(b) => b,
                        None => {
                            tracing::warn!(
                                item_id = %id_for_task,
                                "lazy thumbnail backfill: image item has no content blob"
                            );
                            return Ok(Some((Vec::<u8>::new(), String::new())));
                        }
                    };
                    match lazy_backfill_thumbnail(
                        &db,
                        &id_for_task,
                        content_ref,
                        // lazy_backfill_thumbnail dispatches on key_version
                        // INTERNALLY, so it needs the RAW v1 seed — not the
                        // already-derived `decode_key` (passing the latter
                        // would double-derive: derive_v2(derive_v2(seed))).
                        &meta_json,
                        &v1_key_thumb,
                        item.key_version,
                    ) {
                        Ok((blob, new_meta)) => {
                            // Overwrite the local meta_json so the
                            // thumb_file_id parse below reads the value
                            // we just persisted to the DB.
                            meta_json = new_meta;
                            blob
                        }
                        Err(e) => {
                            tracing::warn!(
                                item_id = %id_for_task,
                                err = %e,
                                "lazy thumbnail backfill failed; returning null sentinel"
                            );
                            // Signal null sentinel via a sentinel Ok(Some)
                            // with an empty bytes vec — caller checks.
                            // Cleaner than a custom error variant: the
                            // outer match maps empty bytes → null response.
                            return Ok(Some((Vec::<u8>::new(), String::new())));
                        }
                    }
                }
            };

            // The thumbnail is keyed by a DISTINCT thumb_file_id recorded
            // additively in blob_ref meta JSON (written at capture time or
            // by the backfill path above).
            let thumb_file_id = parse_image_thumb_file_id(&meta_json).map_err(|e| {
                anyhow::anyhow!("image item {} thumb meta parse error: {}", id_for_task, e)
            })?;

            // `decode_thumbnail` takes the serialized blob directly
            // (runs `chunks_from_blob` + decrypt internally).
            let png_bytes =
                copypaste_core::decode_thumbnail(&thumb_blob, decode_key, &thumb_file_id).map_err(
                    |e| anyhow::anyhow!("image item {} thumb decode failed: {}", id_for_task, e),
                )?;

            use base64::Engine as _;
            let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
            let data_uri = format!("data:image/png;base64,{b64}");
            Ok(Some((png_bytes, data_uri)))
        })
        .await;
        match join {
            Ok(Ok(Some((png_bytes, _data_uri)))) if png_bytes.is_empty() => {
                // Empty-bytes sentinel: backfill failed, return null.
                Response::ok(
                    req.id,
                    serde_json::json!({ "thumbnail": serde_json::Value::Null }),
                )
            }
            Ok(Ok(Some((_png_bytes, data_uri)))) => {
                Response::ok(req.id, serde_json::json!({ "thumbnail": data_uri }))
            }
            Ok(Ok(None)) => {
                Response::err_with_code(req.id, ERR_CODE_NOT_FOUND, format!("item not found: {id}"))
            }
            Ok(Err(e)) => Response::err(req.id, e.to_string()),
            Err(e) => Response::err_with_code(
                req.id,
                ERR_CODE_INTERNAL_ERROR,
                format!("blocking task failed: {e}"),
            ),
        }
    }

    // B. get_item_file — decrypt and return a FILE item as raw bytes.
    //
    // Params: {"id": "<uuid>"}
    // Success: {"filename": "<name>", "mime": "<type>", "data_b64": "<b64>"}
    // Error: item not found, non-file content_type, or decrypt failure.
    //
    // Mirrors `get_item_image` but uses `decode_file` (no decode/re-encode)
    // and returns the raw bytes as base64 plus the filename and MIME type
    // parsed from the `blob_ref` meta JSON.
    pub(crate) async fn handle_get_item_file(&self, req: Request) -> Response {
        let id = match extract_uuid_param(&req.params, req.id.clone()) {
            Ok(id) => id,
            Err(resp) => return resp,
        };
        let db_arc = self.db.clone();
        let id_for_task = id.clone();
        // CopyPaste-z1xt: run the full DB-fetch + decrypt + base64 pipeline
        // inside spawn_blocking (the decrypt + base64 previously ran on the
        // async executor thread).
        // CopyPaste-eq9m: move the encrypted blob out of the item (no
        // clone) and free the decrypted `raw_bytes` before building the
        // response so peak RAM is one decoded copy + one base64 string.
        // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
        // even if the spawn_blocking worker panics or is cancelled.
        let v1_key = zeroize::Zeroizing::new(**self.local_key);
        enum ItemFileResult {
            Ok {
                filename: String,
                mime: String,
                data_b64: String,
            },
            NotFound,
            NotFile(String),
            Internal(String),
            Auth(String),
        }
        let join = tokio::task::spawn_blocking(move || -> anyhow::Result<ItemFileResult> {
            let item = {
                let db = db_arc.blocking_lock();
                get_item_by_id(&*db, &id_for_task)?
            };
            let mut item = match item {
                Some(it) => it,
                None => return Ok(ItemFileResult::NotFound),
            };
            if item.content_type != "file" {
                return Ok(ItemFileResult::NotFile(format!(
                    "item {id_for_task} is not a file (content_type: {})",
                    item.content_type
                )));
            }
            let content = match item.content.take() {
                Some(b) => b,
                None => {
                    return Ok(ItemFileResult::Internal(format!(
                        "file item {id_for_task} has no content blob"
                    )))
                }
            };
            let meta_json = match item.blob_ref.as_deref() {
                Some(s) => s,
                None => {
                    return Ok(ItemFileResult::Internal(format!(
                        "file item {id_for_task} missing blob_ref metadata"
                    )))
                }
            };
            let file_meta = match parse_file_meta(meta_json) {
                Ok(m) => m,
                Err(e) => {
                    return Ok(ItemFileResult::Internal(format!(
                        "file item {id_for_task} blob_ref parse error: {e}"
                    )))
                }
            };
            let chunks = match chunks_from_blob(&content) {
                Ok(c) => c,
                Err(e) => {
                    return Ok(ItemFileResult::Internal(format!(
                        "file item {id_for_task} chunks_from_blob failed: {e}"
                    )))
                }
            };
            let v2_key = derive_v2(&v1_key);
            let key_to_use: &[u8; 32] = if item.key_version == 1 {
                &v1_key
            } else {
                &v2_key
            };
            let raw_bytes = match decode_file(&chunks, key_to_use, &file_meta.file_id) {
                Ok(b) => b,
                Err(e) => {
                    return Ok(ItemFileResult::Auth(format!(
                        "file item {id_for_task} decode failed: {e}"
                    )))
                }
            };
            use base64::Engine as _;
            let data_b64 = base64::engine::general_purpose::STANDARD.encode(&raw_bytes);
            // CopyPaste-eq9m: free the decoded file bytes before returning.
            drop(raw_bytes);
            Ok(ItemFileResult::Ok {
                filename: file_meta.filename,
                mime: file_meta.mime,
                data_b64,
            })
        })
        .await;
        match join {
            Ok(Ok(ItemFileResult::Ok {
                filename,
                mime,
                data_b64,
            })) => Response::ok(
                req.id,
                serde_json::json!({
                    "filename": filename,
                    "mime":     mime,
                    "data_b64": data_b64,
                }),
            ),
            Ok(Ok(ItemFileResult::NotFound)) => {
                Response::err_with_code(req.id, ERR_CODE_NOT_FOUND, format!("item not found: {id}"))
            }
            Ok(Ok(ItemFileResult::NotFile(msg))) => {
                Response::err_with_code(req.id, ERR_CODE_INVALID_ARGUMENT, msg)
            }
            Ok(Ok(ItemFileResult::Internal(msg))) => {
                Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
            }
            Ok(Ok(ItemFileResult::Auth(msg))) => {
                Response::err_with_code(req.id, ERR_CODE_AUTH_FAILED, msg)
            }
            Ok(Err(e)) => Response::err(req.id, e.to_string()),
            Err(e) => Response::err_with_code(
                req.id,
                ERR_CODE_INTERNAL_ERROR,
                format!("blocking task failed: {e}"),
            ),
        }
    }
}
