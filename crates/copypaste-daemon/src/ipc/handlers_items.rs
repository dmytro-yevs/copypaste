//! Clipboard/history IPC handlers + pasteboard write (split from ipc god-module, ra15.1).
use super::*;

impl IpcServer {
    pub(crate) async fn dispatch_items(&self, req: Request) -> Response {
        match req.method.as_str() {
            // c4q2.17: "list" is the legacy CLI verb. Response shape is now
            // unified under "history_page" (pinned-first, same fields).
            // CLI copypaste-cli was migrated to METHOD_HISTORY_PAGE (c4q2.17).
            // Kept as an explicit stub so old callers get a diagnosable error.
            "list" => Response::err_with_code(
                req.id,
                ERR_CODE_NOT_IMPLEMENTED,
                "list is deprecated: use history_page with {limit, offset} — \
                 the response shape is identical but pinned items appear first (c4q2.17)",
            ),
            "delete" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    // P2-8u2b: tag with ERR_CODE_INVALID_ARGUMENT so machine
                    // clients can classify the error rather than getting a bare
                    // untyped error string.
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: id",
                        )
                    }
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "invalid param: id must be a valid UUID",
                    );
                }
                match self.soft_delete_and_broadcast(&id).await {
                    Ok(_) => Response::ok(req.id, serde_json::Value::Null),
                    Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
                }
            }
            "count" => match self.with_read_db(|db| count_items(db)).await {
                Ok(Ok(n)) => Response::ok(req.id, serde_json::json!({"count": n})),
                Ok(Err(e)) => Response::err(req.id, e.to_string()),
                Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
            },
            "search" => {
                let query = match req.params.get("query").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    // CopyPaste-kfe9: tag with ERR_CODE_INVALID_ARGUMENT so
                    // machine clients can classify the error (follow-up of 8u2b).
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: query",
                        )
                    }
                };
                // Clamp to MAX_PAGE like `list` / `history_page` so an oversized
                // `limit` cannot make `search_items` allocate/scan unbounded rows.
                let limit = (req
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize)
                    .min(MAX_PAGE);
                // CopyPaste-tteo: optional content_type filter (CLI --kind flag).
                // Accepted values mirror clipboard_items.content_type: "text",
                // "image", "file". An unknown value simply returns no results
                // (the filter is passed directly to the SQL WHERE clause via a
                // parameterised query — no injection risk).
                let kind_filter: Option<String> = req
                    .params
                    .get("kind")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());

                // CopyPaste-crh3.86: single-body closure via with_read_db — the
                // pool/writer branches used to duplicate this query verbatim.
                let join = self
                    .with_read_db(move |db| {
                        let kf = kind_filter.as_deref();
                        // CopyPaste-tteo: batch-fetch previews from FTS after the
                        // search so search response matches history_page.
                        let items = search_items_filtered(db, &query, limit, kf)?;
                        let preview_ids: Vec<&str> = items
                            .iter()
                            .filter(|it| !it.is_sensitive && it.content_type == "text")
                            .map(|it| it.id.as_str())
                            .collect();
                        let previews =
                            fetch_text_previews_batch(db, &preview_ids).unwrap_or_default();
                        Ok::<
                            (
                                Vec<copypaste_core::ClipboardItem>,
                                std::collections::HashMap<String, String>,
                            ),
                            copypaste_core::ItemsError,
                        >((items, previews))
                    })
                    .await;
                match join {
                    Ok(Ok((items, preview_map))) => {
                        let json_items: Vec<_> = items
                            .iter()
                            .map(|item| {
                                // CopyPaste-tteo: include preview, kind, pinned in
                                // search results to match history_page field parity.
                                let preview = if item.is_sensitive {
                                    format!("[sensitive — id:{}]", &item.id[..8])
                                } else if item.content_type == "text" {
                                    preview_map
                                        .get(item.id.as_str())
                                        .cloned()
                                        .unwrap_or_else(|| format!("[text — id:{}]", &item.id[..8]))
                                } else if item.content_type == "file" {
                                    let name = item
                                        .blob_ref
                                        .as_deref()
                                        .and_then(|j| parse_file_meta(j).ok())
                                        .map(|m| m.filename)
                                        .unwrap_or_else(|| format!("id:{}", &item.id[..8]));
                                    format!("[file: {name}]")
                                } else {
                                    format!("[image — id:{}]", &item.id[..8])
                                };
                                let kind: &str = if item.content_type == "text" {
                                    copypaste_core::text_kind::classify_text(&preview).label()
                                } else if item.content_type == "file" {
                                    "FILE"
                                } else {
                                    "IMAGE"
                                };
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "is_sensitive": item.is_sensitive,
                                    "wall_time": item.wall_time,
                                    "lamport_ts": item.lamport_ts,
                                    "preview": preview,
                                    "pinned": item.pinned,
                                    "kind": kind,
                                    // Daemon-computed single source of truth: true when
                                    // this item exceeds the local sync size ceiling and
                                    // therefore won't be synced. UIs badge it. Same
                                    // shape as the `list`/`history_page` arms.
                                    "too_large_to_sync": too_large_to_sync(item),
                                })
                            })
                            .collect();
                        Response::ok(req.id, serde_json::json!({"items": json_items}))
                    }
                    // CopyPaste-kfe9: tag with ERR_CODE_INTERNAL_ERROR so clients
                    // get a machine-readable code (follow-up of 8u2b).
                    Ok(Err(e)) => {
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string())
                    }
                    // CopyPaste-crh3.86: with_read_db already formats the join
                    // failure; surface it directly (no double "blocking task failed").
                    Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
                }
            }
            "copy" | "paste" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    // P2-8u2b: tag with ERR_CODE_INVALID_ARGUMENT so machine
                    // clients can classify the error.
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: id",
                        )
                    }
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "invalid param: id must be a valid UUID",
                    );
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Resolve directly by primary key — paging + linear scan
                    // silently missed any item past position 1000 (data loss).
                    let item = get_item_by_id(&*db, &id_for_task)?;
                    Ok::<_, anyhow::Error>(item)
                })
                .await;
                match join {
                    Ok(Ok(Some(item))) => match self.write_to_pasteboard(&item).await {
                        Ok(()) => {
                            // C. PROMOTE-ON-COPY: bump wall_time/lamport so this
                            // item sorts to the top of history_page on the next
                            // request, matching Maccy-style recency ordering.
                            let db_arc2 = self.db.clone();
                            let item_id_bump = item.id.clone();
                            // P1: surface bump errors via tracing instead of
                            // double-swallowing (let _ spawn + let _ inside).
                            // Promote-on-copy is best-effort — a failure must
                            // not abort the copy response — but silent failures
                            // make it impossible to diagnose why items don't
                            // reorder after being copied.
                            match tokio::task::spawn_blocking(move || {
                                let db = db_arc2.blocking_lock();
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                // CopyPaste-ojhe: unified lamport value space —
                                // max(existing + 1, now_ms) keeps the promote
                                // monotonic vs the row's own prior lamport so a
                                // later pin/delete (also unified) can overtake it.
                                let prev_lamport = get_item_by_id(&*db, &item_id_bump)
                                    .ok()
                                    .flatten()
                                    .map(|r| r.lamport_ts)
                                    .unwrap_or(0);
                                let new_lamport =
                                    copypaste_core::next_lamport_ts(prev_lamport, now_ms);
                                // Pass None: ipc recopy path doesn't know sensitive TTL;
                                // delete_expired picks up expires_at set at capture time.
                                bump_item_recency(&db, &item_id_bump, now_ms, new_lamport, None)
                            })
                            .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    tracing::warn!(
                                        id = %item.id,
                                        "bump_item_recency failed: {e}"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        id = %item.id,
                                        "bump_item_recency task join error: {e}"
                                    );
                                }
                            }
                            Response::ok(
                                req.id,
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "written": true,
                                }),
                            )
                        }
                        Err(PasteboardError::DecryptFailed(msg)) => Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("paste decrypt failed: {msg}"),
                        ),
                        // CopyPaste-kfe9: tag pasteboard-write failures with
                        // ERR_CODE_INTERNAL_ERROR for machine-readable classification.
                        Err(PasteboardError::Other(msg)) => Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("pasteboard write failed: {msg}"),
                        ),
                    },
                    // CopyPaste-kfe9: not_found so clients can distinguish
                    // "item missing" from other internal errors (follow-up of 8u2b).
                    Ok(Ok(None)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
                    Ok(Err(e)) => {
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string())
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "delete_all" => {
                // CopyPaste-cb7u: previously this called soft_delete_and_broadcast
                // once per item, each with its own spawn_blocking. On large histories
                // (hundreds of items) that is hundreds of async context switches and
                // lock acquisitions. Fix: ONE spawn_blocking that holds the DB lock
                // for the entire batch and performs all soft-deletes in a single
                // SQLite transaction.  Tombstones are then broadcast (fire-and-
                // forget, no blocking) from the async context.
                let db_arc = self.db.clone();
                let now_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64;
                let batch_result = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let conn = db.conn();
                    // Fetch every non-pinned, non-deleted item in one query.
                    let mut stmt = conn.prepare(
                        "SELECT id, lamport_ts FROM clipboard_items WHERE pinned = 0 AND deleted = 0",
                    )?;
                    let rows: Vec<(String, i64)> = stmt
                        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
                        .filter_map(|r| r.ok())
                        .collect();

                    if rows.is_empty() {
                        // anyhow error so both rusqlite::Error and the core
                        // ItemsError from soft_delete_item_in_tx convert via `?`.
                        return Ok::<_, anyhow::Error>(vec![]);
                    }

                    // CopyPaste-jvzm.3: soft-delete every item in ONE transaction
                    // by reusing the canonical core tombstone definition
                    // (soft_delete_item_in_tx) instead of hand-rolling the
                    // UPDATE + FTS + pending_uploads cleanup here — so the batch
                    // path can never drift from the single-item path (and now
                    // also resets is_synced=0, which the old inline copy missed).
                    // The previous O(n) "FTS orphan purge" cross-table scan is
                    // dropped: the per-item FTS DELETE inside the helper already
                    // keeps the index consistent.
                    let tx = conn.unchecked_transaction()?;
                    for (id, prev_lamport) in &rows {
                        let new_lamport =
                            copypaste_core::next_lamport_ts(*prev_lamport, now_ms);
                        copypaste_core::storage::items::soft_delete_item_in_tx(
                            &tx,
                            id,
                            new_lamport,
                            now_ms,
                        )?;
                    }
                    tx.commit()?;

                    // Return the IDs so the async caller can re-read tombstones
                    // and broadcast them to P2P/cloud sync peers.
                    let ids: Vec<String> = rows.into_iter().map(|(id, _)| id).collect();
                    Ok(ids)
                })
                .await;

                match batch_result {
                    Ok(Ok(ids)) => {
                        let count = ids.len();
                        // Re-read each tombstone and broadcast (fire-and-forget).
                        // This mirrors soft_delete_and_broadcast's broadcast step
                        // but avoids re-acquiring spawn_blocking per item.
                        if let Some(ref tx) = self.new_item_tx {
                            let db_arc2 = self.db.clone();
                            let tx2 = tx.clone();
                            tokio::spawn(async move {
                                let guard = db_arc2.lock().await;
                                for id in &ids {
                                    if let Ok(Some(tombstone)) = get_item_by_id(&*guard, id) {
                                        let _ = tx2.send(tombstone);
                                    }
                                }
                            });
                        }
                        Response::ok(req.id, serde_json::json!({ "deleted": count }))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "stats" => match self
                .with_read_db(|db| {
                    // CopyPaste-crh3.86: single-body closure over &dyn DbRead
                    // (the sensitive count is a raw query on the trait's conn()).
                    let total = copypaste_core::count_items(db).unwrap_or(0);
                    let sensitive_count: i64 = db
                        .conn()
                        .query_row(
                            "SELECT COUNT(*) FROM clipboard_items WHERE is_sensitive = 1",
                            [],
                            |row| row.get(0),
                        )
                        .unwrap_or(0);
                    Ok::<_, std::convert::Infallible>((total, sensitive_count))
                })
                .await
            {
                Ok(Ok((total, sensitive_count))) => Response::ok(
                    req.id,
                    serde_json::json!({
                        "total_items": total,
                        "sensitive_items": sensitive_count,
                        "version": "1",
                        "build_version": BUILD_VERSION,
                    }),
                ),
                // `Infallible` — the closure never returns Err.
                Ok(Err(never)) => match never {},
                Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
            },
            "pin" => {
                // Pin an item (remove expiry so it's never auto-deleted)
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    // CopyPaste-kfe9: tag with ERR_CODE_INVALID_ARGUMENT so
                    // machine clients can classify the error (follow-up of 8u2b).
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: id",
                        )
                    }
                };
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "invalid param: id must be a valid UUID",
                    );
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    copypaste_core::pin_item(&db, &id_for_task)?;
                    // Re-read the updated row so the broadcast carries the new
                    // pinned=true / pin_order for LWW propagation to peers.
                    let row = get_item_by_id(&*db, &id_for_task)?;
                    Ok::<_, copypaste_core::storage::items::ItemsError>(row)
                })
                .await;
                match join {
                    Ok(Ok(row_opt)) => {
                        // Propagate pin state to peers via the sync channel.
                        if let (Some(row), Some(ref tx)) = (row_opt, &self.new_item_tx) {
                            let _ = tx.send(row);
                        }
                        Response::ok(req.id, serde_json::json!({"pinned": true, "id": id}))
                    }
                    // CopyPaste-kfe9: tag DB errors with ERR_CODE_INTERNAL_ERROR
                    // for machine-readable classification (follow-up of 8u2b).
                    Ok(Err(e)) => {
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string())
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            // T5.x — pin or unpin an item by id. Unlike the legacy `pin`
            // verb (pin-only), this takes an explicit `pinned: bool` so the
            // UI can toggle from a single callback. A `pinned=false` request
            // clears the pin flag (restoring normal TTL behaviour).
            "pin_item" => {
                let id = match extract_uuid_param(&req.params, req.id.clone()) {
                    Ok(id) => id,
                    Err(resp) => return resp,
                };
                let pinned = match req.params.get("pinned").and_then(|v| v.as_bool()) {
                    Some(b) => b,
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: pinned (bool)",
                        )
                    }
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    if pinned {
                        pin_item(&db, &id_for_task)?;
                    } else {
                        unpin_item(&db, &id_for_task)?;
                    }
                    // Re-read the updated row so the broadcast carries the new
                    // pinned / pin_order for LWW propagation to peers.
                    let row = get_item_by_id(&*db, &id_for_task)?;
                    Ok::<_, copypaste_core::storage::items::ItemsError>(row)
                })
                .await;
                match join {
                    Ok(Ok(row_opt)) => {
                        // Propagate pin-state change to peers via the sync channel.
                        if let (Some(row), Some(ref tx)) = (row_opt, &self.new_item_tx) {
                            let _ = tx.send(row);
                        }
                        Response::ok(req.id, serde_json::json!({"pinned": pinned, "id": id}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            // A1 — reorder pinned items by providing their ids in the desired
            // display order. Accepts `params.ids: [String]` (primary-key `id`
            // values, not `item_id`) in the desired order. Assigns consecutive
            // `pin_order` values (1.0, 2.0, …) inside a single transaction.
            // Returns `{ "ok": true }`.
            "reorder_pinned" => {
                let ids: Vec<String> = match req.params.get("ids").and_then(|v| v.as_array()) {
                    Some(arr) => {
                        let mut out = Vec::with_capacity(arr.len());
                        for v in arr {
                            match v.as_str() {
                                Some(s) => out.push(s.to_string()),
                                None => {
                                    return Response::err_with_code(
                                        req.id,
                                        ERR_CODE_INVALID_ARGUMENT,
                                        "ids must be an array of strings",
                                    )
                                }
                            }
                        }
                        out
                    }
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: ids (array of item id strings)",
                        )
                    }
                };
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let id_refs: Vec<&str> = ids.iter().map(String::as_str).collect();
                    reorder_pinned(&db, &id_refs)?;
                    // Re-read each reordered row so the broadcast carries the
                    // updated pin_order for LWW convergence on peers.
                    let mut rows: Vec<copypaste_core::ClipboardItem> =
                        Vec::with_capacity(id_refs.len());
                    for id in &id_refs {
                        if let Some(row) = get_item_by_id(&*db, id)? {
                            rows.push(row);
                        }
                    }
                    Ok::<_, copypaste_core::storage::items::ItemsError>(rows)
                })
                .await;
                match join {
                    Ok(Ok(rows)) => {
                        // Broadcast every reordered item so peers converge on
                        // the new pin_order via LWW.
                        if let Some(ref tx) = self.new_item_tx {
                            for row in rows {
                                let _ = tx.send(row);
                            }
                        }
                        Response::ok(req.id, serde_json::json!({"ok": true}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            // T5.x — delete a single item by id. Mirrors the legacy `delete`
            // verb but uses the typed `invalid_argument` error code (the UI
            // branches on `error_code`) and returns a structured `{deleted,
            // id}` payload. FTS cleanup is best-effort (logged on failure).
            "delete_item" => {
                let id = match extract_uuid_param(&req.params, req.id.clone()) {
                    Ok(id) => id,
                    Err(resp) => return resp,
                };
                match self.soft_delete_and_broadcast(&id).await {
                    Ok((changed, _)) => Response::ok(
                        req.id,
                        serde_json::json!({"deleted": changed > 0, "id": id}),
                    ),
                    Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
                }
            }
            // T5.x — copy an item back to the system clipboard by id. Same
            // paste-back path as `copy`/`paste` (decrypt → NSPasteboard) but
            // surfaces typed `invalid_argument` / `not_found` error codes so
            // the UI can branch on `error_code` rather than parsing strings.
            "copy_item" => {
                let id = match extract_uuid_param(&req.params, req.id.clone()) {
                    Ok(id) => id,
                    Err(resp) => return resp,
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Resolve the row directly by primary key. Previously this
                    // paged `get_page(1000, 0)` and linear-scanned, so any item
                    // beyond position 1000 silently returned `not_found`
                    // (data-loss for power users). `get_item_by_id` is a single
                    // indexed `SELECT ... WHERE id = ?1` with no window cap.
                    let item = get_item_by_id(&*db, &id_for_task)?;
                    // Also fetch the short text preview while we hold the db
                    // lock; this is used by the UI to build a rich notification.
                    let preview: Option<String> = item.as_ref().and_then(|it| {
                        if it.content_type == "text" && !it.is_sensitive {
                            fetch_text_preview(&*db, &it.id).ok().flatten()
                        } else if it.content_type == "file" {
                            it.blob_ref
                                .as_deref()
                                .and_then(|j| parse_file_meta(j).ok())
                                .map(|m| format!("[file: {}]", m.filename))
                        } else {
                            None // image and unknown: body is set by the UI
                        }
                    });
                    Ok::<_, anyhow::Error>((item, preview))
                })
                .await;
                match join {
                    Ok(Ok((Some(item), preview))) => match self.write_to_pasteboard(&item).await {
                        Ok(()) => {
                            // C. PROMOTE-ON-COPY: bump wall_time/lamport so this
                            // item sorts to the top of history_page on the next
                            // request, matching Maccy-style recency ordering.
                            let db_arc2 = self.db.clone();
                            let item_id_bump = item.id.clone();
                            // P1: surface bump errors via tracing instead of
                            // double-swallowing (let _ spawn + let _ inside).
                            match tokio::task::spawn_blocking(move || {
                                let db = db_arc2.blocking_lock();
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                // CopyPaste-ojhe: unified lamport value space —
                                // max(existing + 1, now_ms) keeps the promote
                                // monotonic vs the row's own prior lamport so a
                                // later pin/delete (also unified) can overtake it.
                                let prev_lamport = get_item_by_id(&*db, &item_id_bump)
                                    .ok()
                                    .flatten()
                                    .map(|r| r.lamport_ts)
                                    .unwrap_or(0);
                                let new_lamport =
                                    copypaste_core::next_lamport_ts(prev_lamport, now_ms);
                                // Pass None: ipc recopy path doesn't know sensitive TTL;
                                // delete_expired picks up expires_at set at capture time.
                                bump_item_recency(&db, &item_id_bump, now_ms, new_lamport, None)
                            })
                            .await
                            {
                                Ok(Ok(_)) => {}
                                Ok(Err(e)) => {
                                    tracing::warn!(
                                        id = %item.id,
                                        "bump_item_recency failed: {e}"
                                    );
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        id = %item.id,
                                        "bump_item_recency task join error: {e}"
                                    );
                                }
                            }
                            Response::ok(
                                req.id,
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    // Short preview for rich notifications — text
                                    // items get plaintext; files get "[file: name]";
                                    // images are null (the UI uses "Image" fallback).
                                    "preview": preview,
                                    "written": true,
                                }),
                            )
                        }
                        Err(PasteboardError::DecryptFailed(msg)) => Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("paste decrypt failed: {msg}"),
                        ),
                        Err(PasteboardError::Other(msg)) => {
                            Response::err(req.id, format!("pasteboard write failed: {msg}"))
                        }
                    },
                    Ok(Ok((None, _))) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
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
            "get_item_image" => {
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
                let join =
                    tokio::task::spawn_blocking(move || -> anyhow::Result<ItemImageResult> {
                        let item = {
                            let db = db_arc.blocking_lock();
                            get_item_by_id(&*db, &id_for_task)?
                        };
                        let mut item = match item {
                            Some(it) => it,
                            None => return Ok(ItemImageResult::NotFound),
                        };
                        let is_image =
                            item.content_type == "image" || item.content_type.starts_with("image/");
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
                    Ok(Ok(ItemImageResult::NotFound)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
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
            "get_item_thumbnail" => {
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

                    let is_image =
                        item.content_type == "image" || item.content_type.starts_with("image/");
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
                        copypaste_core::decode_thumbnail(&thumb_blob, decode_key, &thumb_file_id)
                            .map_err(|e| {
                            anyhow::anyhow!("image item {} thumb decode failed: {}", id_for_task, e)
                        })?;

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
                    Ok(Ok(None)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
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
            "get_item_file" => {
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
                let join =
                    tokio::task::spawn_blocking(move || -> anyhow::Result<ItemFileResult> {
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
                    Ok(Ok(ItemFileResult::NotFound)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
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
            "history_page" => {
                // Paginated history with content preview — used by UI (HistoryWindow)
                let raw_limit = req
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(50) as usize;
                let limit = raw_limit.min(MAX_PAGE);
                let offset = req
                    .params
                    .get("offset")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize;
                // CopyPaste-crh3.86: with_read_db centralises the pool/writer
                // fallback; build_page already accepts &dyn DbRead so the branch
                // collapses to a single call.
                let join = self
                    .with_read_db(move |db| {
                        // Helper: build json_items + total from any DbRead source.
                        fn build_page(
                            db: &dyn copypaste_core::DbRead,
                            limit: usize,
                            offset: usize,
                        ) -> anyhow::Result<(Vec<serde_json::Value>, i64)> {
                            let items = get_page_pinned_first(db, limit, offset)?;
                            let total = count_items(db).unwrap_or(0);
                            // Build a device-id → name map once per page so we can
                            // resolve each item's origin without a per-row JOIN.
                            let device_names = get_device_names(db).unwrap_or_default();
                            // CopyPaste-mnte: batch the text-preview fetch into ONE
                            // `SELECT ... WHERE id IN (...)` instead of one round-trip
                            // per text item (a 50-item page was 51 SQL round-trips).
                            // Only non-sensitive text items need an FTS lookup.
                            let preview_ids: Vec<&str> = items
                                .iter()
                                .filter(|it| !it.is_sensitive && it.content_type == "text")
                                .map(|it| it.id.as_str())
                                .collect();
                            let preview_map =
                                fetch_text_previews_batch(db, &preview_ids).unwrap_or_default();
                            // CopyPaste-mnte: the detector is a zero-sized unit struct
                            // over process-wide lazy `RegexSet` statics; construct once
                            // per page (not per item).
                            let detector = SensitiveDetector::new();
                            let json_items: Vec<serde_json::Value> = items
                                .iter()
                                .map(|item| {
                                    let preview = if item.is_sensitive {
                                        format!("[sensitive — id:{}]", &item.id[..8])
                                    } else if item.content_type == "text" {
                                        preview_map.get(item.id.as_str()).cloned().unwrap_or_else(
                                            || format!("[text — id:{}]", &item.id[..8]),
                                        )
                                    } else if item.content_type == "file" {
                                        let name = item
                                            .blob_ref
                                            .as_deref()
                                            .and_then(|j| parse_file_meta(j).ok())
                                            .map(|m| m.filename)
                                            .unwrap_or_else(|| format!("id:{}", &item.id[..8]));
                                        format!("[file: {name}]")
                                    } else {
                                        format!("[image — id:{}]", &item.id[..8])
                                    };
                                    let (preview, sensitive_spans): (
                                        String,
                                        Vec<serde_json::Value>,
                                    ) = if !item.is_sensitive && item.content_type == "text" {
                                        // CopyPaste-mnte: normalise ONCE here (we
                                        // need the normalised string to map byte→char
                                        // offsets below); `detect_normalised` then
                                        // skips the redundant second NFKC pass that
                                        // `detect()` would do internally.
                                        let normalised =
                                            copypaste_core::sensitive::nfkc_normalize(&preview);
                                        let spans = detector
                                            .detect_normalised(&normalised)
                                            .into_iter()
                                            .map(|m| {
                                                let start = byte_to_char_offset(
                                                    &normalised,
                                                    m.matched_range.start,
                                                );
                                                let end = byte_to_char_offset(
                                                    &normalised,
                                                    m.matched_range.end,
                                                );
                                                serde_json::json!([start, end])
                                            })
                                            .collect();
                                        (normalised, spans)
                                    } else {
                                        (preview, vec![])
                                    };
                                    let kind: &str = if item.content_type == "text" {
                                        copypaste_core::text_kind::classify_text(&preview).label()
                                    } else if item.content_type == "file" {
                                        "FILE"
                                    } else {
                                        "IMAGE"
                                    };
                                    // Resolve the human-readable device name.
                                    // `None` when the device was never paired on
                                    // this machine (e.g. synced from a third device)
                                    // or for pre-v3 rows with an empty origin id.
                                    let origin_device_name: Option<&str> = if item
                                        .origin_device_id
                                        .is_empty()
                                    {
                                        None
                                    } else {
                                        device_names.get(&item.origin_device_id).map(|s| s.as_str())
                                    };
                                    serde_json::json!({
                                        "id": item.id,
                                        "content_type": item.content_type,
                                        "is_sensitive": item.is_sensitive,
                                        "wall_time": item.wall_time,
                                        "lamport_ts": item.lamport_ts,
                                        "preview": preview,
                                        "pinned": item.pinned,
                                        "pin_order": item.pin_order,
                                        "sensitive_spans": sensitive_spans,
                                        "too_large_to_sync": too_large_to_sync(item),
                                        "origin_device_id": item.origin_device_id,
                                        "origin_device_name": origin_device_name,
                                        "kind": kind,
                                    })
                                })
                                .collect();
                            Ok((json_items, total))
                        }

                        build_page(db, limit, offset)
                    })
                    .await;
                // Snapshot the own device id outside the blocking task (it lives on self).
                let own_device_id = self.local_device_id.clone().unwrap_or_default();
                match join {
                    Ok(Ok((json_items, total))) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "items": json_items,
                            "total": total,
                            "own_device_id": own_device_id,
                        }),
                    ),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    // CopyPaste-crh3.86: with_read_db already formats the join failure.
                    Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
                }
            }
            _ => self.dispatch_config(req).await,
        }
    }

    pub(crate) async fn dispatch_items_extra(&self, req: Request) -> Response {
        match req.method.as_str() {
            "get_app_icon" => {
                let bundle_id = match req.params.get("bundle_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: bundle_id"),
                };
                // NSWorkspace / AppKit calls are blocking — offload to a
                // dedicated blocking thread so we never stall the async runtime.
                let join = tokio::task::spawn_blocking(move || {
                    crate::app_icon::get_app_icon_base64(&bundle_id)
                })
                .await;
                match join {
                    Ok(png_b64) => Response::ok(req.id, serde_json::json!({ "png_b64": png_b64 })),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }

            // ── File ingest (desktop UI file picker / drag-drop) ───────────────────
            // Takes { filename, mime, data_b64 } where data_b64 is standard
            // base64. Encrypts and stores the file exactly as handle_file does
            // for pasteboard-captured files. Returns { id } on success.
            "add_file_item" => {
                let filename = match req.params.get("filename").and_then(|v| v.as_str()) {
                    Some(s) if !s.is_empty() => s.to_string(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or empty param: filename",
                        )
                    }
                };
                let mime = req
                    .params
                    .get("mime")
                    .and_then(|v| v.as_str())
                    .unwrap_or("application/octet-stream")
                    .to_string();
                let data_b64 = match req.params.get("data_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: data_b64",
                        )
                    }
                };

                use base64::Engine as _;
                let raw_bytes = match base64::engine::general_purpose::STANDARD.decode(&data_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!("data_b64 decode error: {e}"),
                        )
                    }
                };

                let db_arc = self.db.clone();
                // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
                // even if the spawn_blocking worker panics or is cancelled.
                let local_key = zeroize::Zeroizing::new(**self.local_key);
                let join = tokio::task::spawn_blocking(move || {
                    // Read config on blocking thread — same pattern as set_config.
                    let config = read_config();
                    // Content-hash file_id: deterministic so identical files dedup
                    // across captures (mirrors handle_file in daemon.rs).
                    let file_id = crate::clipboard::image_content_hash(&raw_bytes);
                    let max_file_bytes = config
                        .max_file_size_bytes
                        .and_then(|v| usize::try_from(v).ok())
                        .unwrap_or(usize::MAX);

                    let (meta, chunks) = copypaste_core::encode_file(
                        &raw_bytes,
                        &filename,
                        &mime,
                        &local_key,
                        &file_id,
                        max_file_bytes,
                    )
                    .context("encode_file failed")?;

                    let blob =
                        copypaste_core::chunks_to_blob(&chunks).context("chunks_to_blob failed")?;

                    let meta_json = crate::clipboard::build_file_meta_json(&meta);
                    let mut item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
                    // Stable cross-device identity: derive item_id from the
                    // content-hash file_id (mirrors handle_file in daemon.rs).
                    item.item_id = uuid::Uuid::from_bytes(file_id).to_string().into();

                    let db_guard = db_arc.blocking_lock();
                    let stored_id = copypaste_core::insert_item_with_fts(&db_guard, &item, "")
                        .context("insert_item_with_fts failed")?;

                    Ok::<String, anyhow::Error>(stored_id)
                })
                .await;

                match join {
                    Ok(Ok(id)) => Response::ok(req.id, serde_json::json!({ "id": id })),
                    Ok(Err(e)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("add_file_item failed: {e}"),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            other => Response::err(req.id, format!("unknown method: {other}")),
        }
    }

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
