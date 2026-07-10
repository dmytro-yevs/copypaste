//! Clipboard-item read/query IPC verbs (split from handlers_items.rs,
//! ADR-017 daemon-ipc track, CopyPaste-vp63.15).
use super::*;

impl IpcServer {
    // c4q2.17: "list" is the legacy CLI verb. Response shape is now
    // unified under "history_page" (pinned-first, same fields).
    // CLI copypaste-cli was migrated to METHOD_HISTORY_PAGE (c4q2.17).
    // Kept as an explicit stub so old callers get a diagnosable error.
    pub(crate) async fn handle_list(&self, req: Request) -> Response {
        Response::err_with_code(
            req.id,
            ERR_CODE_NOT_IMPLEMENTED,
            "list is deprecated: use history_page with {limit, offset} — \
             the response shape is identical but pinned items appear first (c4q2.17)",
        )
    }

    pub(crate) async fn handle_count(&self, req: Request) -> Response {
        match self.with_read_db(|db| count_items(db)).await {
            Ok(Ok(n)) => Response::ok(req.id, serde_json::json!({"count": n})),
            Ok(Err(e)) => Response::err(req.id, e.to_string()),
            Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
        }
    }

    pub(crate) async fn handle_search(&self, req: Request) -> Response {
        // CopyPaste-kfe9: tag with ERR_CODE_INVALID_ARGUMENT so
        // machine clients can classify the error (follow-up of 8u2b).
        let query =
            match extract_str_param(&req.params, req.id.clone(), "query", "missing param: query") {
                Ok(s) => s,
                Err(resp) => return resp,
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
                let previews = fetch_text_previews_batch(db, &preview_ids).unwrap_or_default();
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
            Ok(Err(e)) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string()),
            // CopyPaste-crh3.86: with_read_db already formats the join
            // failure; surface it directly (no double "blocking task failed").
            Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
        }
    }

    pub(crate) async fn handle_stats(&self, req: Request) -> Response {
        match self
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
        }
    }

    pub(crate) async fn handle_history_page(&self, req: Request) -> Response {
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
        // CopyPaste-a3nu: optional keyset (seek) cursor — opt-in, dual-mode
        // alongside the `offset` param above. Present -> seek from the last
        // row of the previous page (immune to concurrent-insert drift);
        // absent -> existing offset path, byte-for-byte unchanged.
        // Flat structured JSON, not an opaque token: {wall_time, id, pinned,
        // pin_order} mirrors the fields already returned per-item, so a
        // client just echoes back the last item off the previous page.
        //
        // "cursor absent" (None) and "cursor present but unparseable" are
        // distinct: the former is a valid request for the first page (or a
        // caller intentionally using the offset path); the latter means a
        // client THINKS it sent a valid cursor and must be told it didn't —
        // silently falling back to offset-mode page 1 would mask that client
        // bug (matches the `extract_str_param` error style used elsewhere in
        // this file for malformed params).
        let cursor: Option<copypaste_core::PinnedCursor> = match req.params.get("cursor") {
            None => None,
            Some(c) => {
                let parsed = (|| {
                    let wall_time = c.get("wall_time")?.as_i64()?;
                    let id = c.get("id")?.as_str()?.to_string();
                    let pinned = c.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false);
                    let pin_order = c.get("pin_order").and_then(|v| v.as_f64());
                    Some(copypaste_core::PinnedCursor {
                        bucket: if pinned { 0 } else { 1 },
                        pin_order_is_null: if pin_order.is_none() { 1 } else { 0 },
                        pin_order,
                        wall_time,
                        id,
                    })
                })();
                match parsed {
                    Some(cursor) => Some(cursor),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "invalid cursor: expected {wall_time: number, id: string, \
                             pinned?: bool, pin_order?: number|null}",
                        )
                    }
                }
            }
        };
        // CopyPaste-crh3.86: with_read_db centralises the pool/writer
        // fallback; build_page already accepts &dyn DbRead so the branch
        // collapses to a single call.
        let join = self
            .with_read_db(move |db| {
                // Helper: build json_items + total from any DbRead source.
                // `cursor: Some(_)` seeks (keyset); `None` uses `offset`
                // (existing, unchanged path).
                fn build_page(
                    db: &dyn copypaste_core::DbRead,
                    limit: usize,
                    offset: usize,
                    cursor: Option<&copypaste_core::PinnedCursor>,
                ) -> anyhow::Result<(Vec<serde_json::Value>, i64, Option<serde_json::Value>)>
                {
                    let items = match cursor {
                        Some(c) => get_page_pinned_first_seek(db, limit, Some(c))?,
                        None => get_page_pinned_first(db, limit, offset)?,
                    };
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
                            let (preview, sensitive_spans): (String, Vec<serde_json::Value>) =
                                if !item.is_sensitive && item.content_type == "text" {
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
                            let origin_device_name: Option<&str> =
                                if item.origin_device_id.is_empty() {
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
                    // CopyPaste-a3nu: next_cursor is derived from the LAST
                    // returned item's own fields (not the internal
                    // bucket/pin_order_is_null encoding) — a full page
                    // (items.len() == limit) implies more rows may follow;
                    // a short page is the last one, so next_cursor is null.
                    let next_cursor = if items.len() == limit {
                        items.last().map(|item| {
                            serde_json::json!({
                                "wall_time": item.wall_time,
                                "id": item.id,
                                "pinned": item.pinned,
                                "pin_order": item.pin_order,
                            })
                        })
                    } else {
                        None
                    };
                    Ok((json_items, total, next_cursor))
                }

                build_page(db, limit, offset, cursor.as_ref())
            })
            .await;
        // Snapshot the own device id outside the blocking task (it lives on self).
        let own_device_id = self.local_device_id.clone().unwrap_or_default();
        match join {
            Ok(Ok((json_items, total, next_cursor))) => Response::ok(
                req.id,
                serde_json::json!({
                    "items": json_items,
                    "total": total,
                    "own_device_id": own_device_id,
                    "next_cursor": next_cursor,
                }),
            ),
            Ok(Err(e)) => Response::err(req.id, e.to_string()),
            // CopyPaste-crh3.86: with_read_db already formats the join failure.
            Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
        }
    }
}
