//! Import/export IPC handlers (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_transfer(&self, req: Request) -> Response {
        match req.method.as_str() {
            // ----------------------------------------------------------------
            // `import` — bulk-insert items previously exported by another
            // CopyPaste instance. The CLI sends a list of `ImportItem`
            // records; each is hashed (SHA-256 of the decoded bytes) and
            // deduplicated against rows inserted in the last 5 minutes.
            //
            // Request params:
            //   {
            //     "items": [
            //       { "content_type": "text",
            //         "content_bytes_b64": "...",
            //         "created_at_ms": 1234567890,
            //         "metadata": null | { ... } }
            //     ]
            //   }
            //
            // Response data:
            //   { "inserted": <u32>, "skipped": <u32> }
            //
            // Errors:
            //   * `invalid_argument` — missing `items`, missing required field,
            //     or `content_bytes_b64` failed to decode.
            //   * `internal_error` — SQLite failure or task panic.
            // ----------------------------------------------------------------
            "import" => {
                use base64::Engine as _;
                use sha2::{Digest, Sha256};

                // 1. Parse params.items into Vec<ImportItem>.
                let items_value = match req.params.get("items") {
                    Some(v) => v,
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: items",
                        );
                    }
                };
                let raw_items: &[serde_json::Value] = match items_value.as_array() {
                    Some(a) => a.as_slice(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "param 'items' must be an array",
                        );
                    }
                };

                // 2. Validate + decode each item up-front so a malformed entry
                //    aborts the whole import with a clear error (rather than
                //    silently skipping or partially inserting).
                let b64 = base64::engine::general_purpose::STANDARD;
                #[derive(Clone)]
                struct DecodedImport {
                    content_type: String,
                    bytes: Vec<u8>,
                    created_at_ms: i64,
                    /// Caller-supplied `is_sensitive` flag from the export JSON.
                    /// Used as a floor (OR) during import — the daemon always
                    /// recomputes sensitivity from the plaintext so a tampered
                    /// export cannot smuggle a credential in as non-sensitive.
                    caller_is_sensitive: bool,
                    #[allow(dead_code)]
                    metadata: Option<serde_json::Value>,
                }
                let mut decoded: Vec<DecodedImport> = Vec::with_capacity(raw_items.len());
                for (idx, raw) in raw_items.iter().enumerate() {
                    let content_type = match raw.get("content_type").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("item[{idx}]: missing 'content_type'"),
                            );
                        }
                    };
                    let b64_str = match raw.get("content_bytes_b64").and_then(|v| v.as_str()) {
                        Some(s) => s,
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("item[{idx}]: missing 'content_bytes_b64'"),
                            );
                        }
                    };
                    let bytes = match b64.decode(b64_str) {
                        Ok(b) => b,
                        Err(e) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("item[{idx}]: invalid base64 in 'content_bytes_b64': {e}"),
                            );
                        }
                    };
                    // Audit MED #4: enforce per-item ceiling BEFORE storage so
                    // a hostile/corrupt export cannot exhaust daemon memory or
                    // SQLite blob limits. Reject the whole import on first
                    // oversized item — matches the "malformed entry aborts
                    // the batch" contract documented above.
                    if bytes.len() > MAX_IMPORT_ITEM_BYTES {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!(
                                "item[{idx}]: decoded payload {} bytes exceeds max {} bytes",
                                bytes.len(),
                                MAX_IMPORT_ITEM_BYTES
                            ),
                        );
                    }
                    let created_at_ms = match raw.get("created_at_ms").and_then(|v| v.as_i64()) {
                        Some(n) => n,
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("item[{idx}]: missing or non-integer 'created_at_ms'"),
                            );
                        }
                    };
                    let metadata = raw.get("metadata").cloned();
                    // PG-26: read the caller-supplied flag but treat it only as
                    // a floor — the daemon recomputes sensitivity from plaintext
                    // below and ORs the two values so a tampered export file
                    // cannot downgrade a credential to non-sensitive.
                    let caller_is_sensitive = raw
                        .get("is_sensitive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    decoded.push(DecodedImport {
                        content_type,
                        bytes,
                        created_at_ms,
                        caller_is_sensitive,
                        metadata,
                    });
                }

                // 3. Persist on the blocking pool — SQLite is sync.
                //    For each item: hash; if a row with the same hash exists
                //    within the dedupe window, skip; otherwise insert.
                let db_arc = self.db.clone();
                // Move a copy of the device's v1 storage key into the blocking
                // task so imported content can be ENCRYPTED with the same
                // (key, AAD, key_version) the normal ingest path uses — see
                // the per-item block below.
                // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
                // even if the spawn_blocking worker panics or is cancelled.
                let local_key_v1 = zeroize::Zeroizing::new(**self.local_key);
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // v0.3 post-T2: dedup is now enforced atomically by the
                    // v5 UNIQUE indexes (content_hash + minute_bucket) inside
                    // insert_item_with_fts. The previous explicit
                    // `find_recent_by_hash` precheck created a TOCTOU window
                    // — two concurrent imports of the same payload could both
                    // pass the precheck and then race on insert. The new
                    // path returns the existing row's id on a unique-violation,
                    // which we treat as a dedup skip.
                    let mut inserted: u32 = 0;
                    let mut skipped: u32 = 0;
                    // P2P Phase 3: collect successfully-inserted rows so the
                    // handler can broadcast them to the sync orchestrator (which
                    // re-keys + pushes them to paired peers).
                    let mut inserted_clips: Vec<copypaste_core::ClipboardItem> = Vec::new();
                    // Derive the v2 storage key once: imported content is
                    // encrypted exactly as `daemon::encrypt_text_for_storage`
                    // does (v2 key + v4 AAD, stamped key_version = 2), so the
                    // read path (`decrypt_item_by_version`, dispatched by the
                    // `copy`/`paste` IPC verb) can decrypt it.
                    let v2_key = derive_v2(&local_key_v1);
                    for item in decoded {
                        let mut hasher = Sha256::new();
                        hasher.update(&item.bytes);
                        let hash_hex = hex::encode(hasher.finalize());

                        // Audit fix (import round-trip): previously imported
                        // bytes were stored VERBATIM with an EMPTY nonce while
                        // `ClipboardItem::new_text` stamped key_version = 2.
                        // The read path then tried to XChaCha20-Poly1305-decrypt
                        // them under the v2 key and failed with AuthFailed, so
                        // imported items could never be retrieved.
                        //
                        // Now we ENCRYPT the content the same way fresh ingest
                        // does: build the AAD from the row's own item_id with
                        // the v4 schema + key_version 2, encrypt with the v2
                        // key, and store the real (nonce, ciphertext). The row
                        // stays at key_version = 2 (set by new_text) so the
                        // read path selects the matching key/AAD.
                        //
                        // lamport_ts = 0 is a deliberate "imported, unknown
                        // origin" sentinel; sync will reassign on first push.
                        let item_id = uuid::Uuid::new_v4().to_string();
                        let aad = copypaste_core::build_item_aad_v2(
                            &copypaste_core::ItemId::from(item_id.as_str()),
                            copypaste_core::AAD_SCHEMA_VERSION_V4,
                            copypaste_core::ITEM_KEY_VERSION_CURRENT as u32,
                        );
                        let (nonce, ciphertext) =
                            match copypaste_core::encrypt_item_with_aad(&item.bytes, &v2_key, &aad)
                            {
                                Ok(v) => v,
                                Err(e) => {
                                    return Err::<
                                        (u32, u32, Vec<copypaste_core::ClipboardItem>),
                                        anyhow::Error,
                                    >(anyhow::anyhow!(
                                        "encrypt imported item failed: {e}"
                                    ));
                                }
                            };
                        let mut clip =
                            copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
                        clip.item_id = copypaste_core::ItemId::from(item_id);
                        clip.content_type = item.content_type.clone();
                        clip.wall_time = item.created_at_ms;
                        clip.content_hash = Some(hash_hex);

                        // PG-26: recompute sensitivity from the decrypted
                        // plaintext so a tampered export file cannot smuggle a
                        // credential in with `is_sensitive=false` and bypass the
                        // auto-wipe TTL.  Only text items carry detectable
                        // credentials (images have no text to scan).
                        // OR semantics: we never DOWNGRADE a caller-flagged
                        // item — a legitimate sensitive export stays sensitive;
                        // a credential falsely marked false is caught here.
                        clip.is_sensitive = if item.content_type == "text" {
                            let text = std::str::from_utf8(&item.bytes).unwrap_or("");
                            is_sensitive_for_autowipe(text) || item.caller_is_sensitive
                        } else {
                            // Non-text: trust caller flag only (no text to scan).
                            item.caller_is_sensitive
                        };

                        // FTS indexing: pass "" to skip the FTS write. The
                        // searchable plaintext is no longer available as a
                        // stored column (content is now ciphertext), matching
                        // the image path semantics — search over imported
                        // items is out of scope for this fix.
                        let requested_id = clip.id.clone();
                        match copypaste_core::insert_item_with_fts(&db, &clip, "") {
                            Ok(stored_id) if stored_id == requested_id => {
                                inserted += 1;
                                inserted_clips.push(clip);
                            }
                            Ok(_) => {
                                // Returned id differs => dedup hit (existing
                                // row with same content_hash/item_id).
                                skipped += 1;
                            }
                            Err(e) => {
                                return Err::<
                                    (u32, u32, Vec<copypaste_core::ClipboardItem>),
                                    anyhow::Error,
                                >(e.into());
                            }
                        }
                    }
                    Ok::<(u32, u32, Vec<copypaste_core::ClipboardItem>), anyhow::Error>((
                        inserted,
                        skipped,
                        inserted_clips,
                    ))
                })
                .await;

                match join {
                    Ok(Ok((inserted, skipped, inserted_clips))) => {
                        // P2P Phase 3: notify the sync orchestrator of each newly
                        // imported row so it is re-keyed and pushed to paired
                        // peers (a closed/absent channel is a no-op — no peers).
                        if let Some(ref tx) = self.new_item_tx {
                            for clip in inserted_clips {
                                let _ = tx.send(clip);
                            }
                        }
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "inserted": inserted,
                                "skipped": skipped,
                            }),
                        )
                    }
                    Ok(Err(e)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("import failed: {e}"),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // export — return all decrypted items so the CLI backup command
            // can serialise them for `import`.
            //
            // Params: {} (no params required)
            // Success: {"items": [ {
            //     "id": "<row-uuid>",
            //     "item_id": "<item-uuid>",
            //     "content_type": "text"|...,
            //     "content_bytes_b64": "<base64 plaintext>",
            //     "created_at_ms": <i64 unix-ms>,
            //     "wall_time": <i64>,
            //     "lamport_ts": <i64>,
            //     "is_sensitive": <bool>
            // }, ... ]}
            //
            // Non-text items (images, etc.) are skipped — their chunked
            // ciphertext cannot be trivially re-imported by the CLI `import`
            // path (which only handles `content_bytes_b64`).
            //
            // Gated behind `requires_db` (see above) so it returns
            // IPC_NOT_READY during degraded/pre-ready startup.
            // ------------------------------------------------------------------
            "export" => {
                use base64::Engine as _;
                // `limit` > 0 → export the most-recent N items (DESC LIMIT in a
                // subquery, then re-order ASC for deterministic import order).
                // `limit` == 0 or absent → export ALL (legacy / unlimited).
                let export_limit = req
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                // P2-tj9s: `include_sensitive` defaults to false — sensitive items
                // are excluded by default to avoid bulk-exporting secrets via a
                // single IPC call. Callers that genuinely need them must opt in
                // explicitly. Note: the socket is 0600 so this is defence-in-depth,
                // not an authentication boundary.
                let include_sensitive = req
                    .params
                    .get("include_sensitive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let db_arc = self.db.clone();
                // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
                // even if the spawn_blocking worker panics or is cancelled.
                let local_key_v1 = zeroize::Zeroizing::new(**self.local_key);
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let v2_key = derive_v2(&local_key_v1);
                    // When a limit is requested we select the most-recent N rows
                    // via a DESC subquery and then re-order ASC so the exported
                    // JSON can be re-imported in chronological order.  When no
                    // limit (or limit == 0) we return everything, oldest first.
                    let sql = if export_limit > 0 {
                        "SELECT id, item_id, content_type, content, content_nonce, \
                         is_sensitive, is_synced, lamport_ts, wall_time, key_version \
                         FROM ( \
                             SELECT id, item_id, content_type, content, content_nonce, \
                                    is_sensitive, is_synced, lamport_ts, wall_time, key_version \
                             FROM clipboard_items \
                             ORDER BY wall_time DESC \
                             LIMIT ?1 \
                         ) ORDER BY wall_time ASC"
                            .to_string()
                    } else {
                        "SELECT id, item_id, content_type, content, content_nonce, \
                         is_sensitive, is_synced, lamport_ts, wall_time, key_version \
                         FROM clipboard_items \
                         ORDER BY wall_time ASC"
                            .to_string()
                    };
                    let mut stmt = db.conn().prepare(&sql)?;
                    let b64 = base64::engine::general_purpose::STANDARD;
                    let mut items: Vec<serde_json::Value> = Vec::new();
                    // CopyPaste-93yr: count non-text items skipped so the CLI
                    // can warn the user rather than silently dropping them.
                    let mut skipped_non_text: u64 = 0;
                    let map_row = |row: &rusqlite::Row<'_>| {
                        // key_version can be NULL for genuine v1 rows written
                        // before the column was added.  We read it as Option<i64>
                        // and keep None distinct from a stored value of 1 or 2 so
                        // we can log it clearly rather than silently guessing.
                        let key_version_opt: Option<i64> = row.get(9)?;
                        Ok((
                            row.get::<_, String>(0)?,  // id
                            row.get::<_, String>(1)?,  // item_id
                            row.get::<_, String>(2)?,  // content_type
                            row.get::<_, Option<Vec<u8>>>(3)?,  // content
                            row.get::<_, Option<Vec<u8>>>(4)?,  // content_nonce
                            row.get::<_, bool>(5)?,    // is_sensitive
                            row.get::<_, bool>(6)?,    // is_synced
                            row.get::<_, i64>(7)?,     // lamport_ts
                            row.get::<_, i64>(8)?,     // wall_time
                            key_version_opt,
                        ))
                    };
                    // Cap export_limit to i64::MAX before casting: u64 values
                    // above i64::MAX would wrap negative after `as i64`, which
                    // SQLite treats as unlimited — silently exporting everything
                    // instead of the requested limit.
                    let lim = export_limit.min(i64::MAX as u64) as i64;
                    let rows = if export_limit > 0 {
                        stmt.query_map([lim], map_row)?
                    } else {
                        stmt.query_map([], map_row)?
                    };
                    for row_result in rows {
                        let (id, item_id, content_type, content_opt, nonce_opt,
                             is_sensitive, _is_synced, lamport_ts, wall_time, key_version_opt)
                            = row_result?;
                        // Only export text items — the CLI import path only
                        // accepts content_bytes_b64 (raw bytes), and images are
                        // stored as chunked AEAD blobs that require extra context.
                        // CopyPaste-93yr: count skipped non-text items so the
                        // response can include the count and the CLI can warn.
                        if content_type != "text" {
                            skipped_non_text += 1;
                            continue;
                        }
                        // P2-tj9s: skip sensitive items unless the caller opts in.
                        if is_sensitive && !include_sensitive {
                            continue;
                        }
                        let Some(content) = content_opt else { continue };
                        let Some(nonce_vec) = nonce_opt else { continue };
                        // Resolve key_version: NULL in the DB means the row
                        // predates the key_version column (genuine v1 row).
                        // Log NULL distinctly so mismatches are diagnosable;
                        // assume v1 rather than silently guessing v2 (which
                        // would produce an authentication-tag mismatch).
                        let key_version: u8 = match key_version_opt {
                            Some(v) => match u8::try_from(v) {
                                Ok(kv) => kv,
                                Err(_) => {
                                    tracing::warn!(
                                        id = %id,
                                        key_version = v,
                                        "export: out-of-range key_version {v}, skipping"
                                    );
                                    continue;
                                }
                            },
                            None => {
                                tracing::debug!(
                                    id = %id,
                                    "export: key_version is NULL (pre-column row); \
                                     attempting decrypt as v1"
                                );
                                1
                            }
                        };
                        let nonce: &[u8; 24] = match nonce_vec.as_slice().try_into() {
                            Ok(n) => n,
                            Err(_) => {
                                tracing::warn!(
                                    id = %id,
                                    "export: skipping item with invalid nonce length {}", nonce_vec.len()
                                );
                                continue;
                            }
                        };
                        // P2-zpd1: wrap plaintext in Zeroizing so the decrypted
                        // bytes are wiped on drop, even on early-exit paths
                        // (encode errors, serialisation failure, etc.).
                        let plaintext = match decrypt_item_by_version(
                            key_version,
                            V1Key(&local_key_v1),
                            V2Key(&v2_key),
                            &ItemId::from(item_id.as_str()),
                            nonce,
                            &content,
                        ) {
                            Ok(p) => zeroize::Zeroizing::new(p),
                            Err(e) => {
                                tracing::warn!(
                                    id = %id,
                                    "export: decrypt failed for item ({e}); skipping"
                                );
                                continue;
                            }
                        };
                        items.push(serde_json::json!({
                            "id": id,
                            "item_id": item_id,
                            "content_type": content_type,
                            "content_bytes_b64": b64.encode(&plaintext),
                            "created_at_ms": wall_time,
                            "wall_time": wall_time,
                            "lamport_ts": lamport_ts,
                            "is_sensitive": is_sensitive,
                        }));
                    }
                    // CopyPaste-93yr: return skipped_non_text alongside items
                    // so the CLI can warn the user.
                    Ok::<(Vec<serde_json::Value>, u64), anyhow::Error>((items, skipped_non_text))
                })
                .await;
                match join {
                    Ok(Ok((items, skipped_non_text))) => {
                        let count = items.len();
                        // P2-tj9s: audit log — record item COUNT only, never
                        // content. include_sensitive is logged so operators can
                        // detect unusual export calls in the daemon log.
                        tracing::info!(
                            count,
                            skipped_non_text,
                            include_sensitive,
                            "export: completed (item count only; content not logged)"
                        );
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "items": items,
                                // CopyPaste-93yr: non-zero means some image/file
                                // items were silently skipped; the CLI warns.
                                "skipped_non_text": skipped_non_text,
                            }),
                        )
                    }
                    Ok(Err(e)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("export failed: {e}"),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }

            _ => self.dispatch_items_extra(req).await,
        }
    }
}
