//! Clipboard copy/paste-back IPC verbs (split from handlers_items.rs,
//! ADR-017 daemon-ipc track, CopyPaste-vp63.15).
use super::*;

impl IpcServer {
    /// Shared body for the "copy" and "paste" verbs (identical behaviour).
    pub(crate) async fn handle_copy_or_paste(&self, req: Request) -> Response {
        // P2-8u2b: tag with ERR_CODE_INVALID_ARGUMENT so machine
        // clients can classify the error.
        let id = match extract_str_param(&req.params, req.id.clone(), "id", "missing param: id") {
            Ok(s) => s,
            Err(resp) => return resp,
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
                    // CopyPaste-8ebg.2: read the live sensitive TTL so a
                    // re-copied sensitive item's expires_at is recomputed
                    // from now instead of staying pinned to the original
                    // capture's deadline (the CopyPaste-89ib fix never took
                    // effect because every call site passed None). 0 is the
                    // "auto-wipe disabled" sentinel — pass None for that case
                    // so bump_item_recency leaves expires_at untouched.
                    let sensitive_ttl_ms = self
                        .core_config
                        .as_ref()
                        .and_then(|arc| arc.read().ok())
                        .and_then(|cfg| {
                            if cfg.sensitive_ttl_secs == 0 {
                                None
                            } else {
                                Some(cfg.sensitive_ttl_secs as i64 * 1000)
                            }
                        });
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
                        let new_lamport = copypaste_core::next_lamport_ts(prev_lamport, now_ms);
                        bump_item_recency(&db, &item_id_bump, now_ms, new_lamport, sensitive_ttl_ms)
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
            Ok(Ok(None)) => {
                Response::err_with_code(req.id, ERR_CODE_NOT_FOUND, format!("item not found: {id}"))
            }
            Ok(Err(e)) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string()),
            Err(e) => Response::err_with_code(
                req.id,
                ERR_CODE_INTERNAL_ERROR,
                format!("blocking task failed: {e}"),
            ),
        }
    }

    // T5.x — copy an item back to the system clipboard by id. Same
    // paste-back path as `copy`/`paste` (decrypt → NSPasteboard) but
    // surfaces typed `invalid_argument` / `not_found` error codes so
    // the UI can branch on `error_code` rather than parsing strings.
    pub(crate) async fn handle_copy_item(&self, req: Request) -> Response {
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
                    // CopyPaste-8ebg.2: read the live sensitive TTL so a
                    // re-copied sensitive item's expires_at is recomputed
                    // from now instead of staying pinned to the original
                    // capture's deadline (the CopyPaste-89ib fix never took
                    // effect because every call site passed None). 0 is the
                    // "auto-wipe disabled" sentinel — pass None for that case
                    // so bump_item_recency leaves expires_at untouched.
                    let sensitive_ttl_ms = self
                        .core_config
                        .as_ref()
                        .and_then(|arc| arc.read().ok())
                        .and_then(|cfg| {
                            if cfg.sensitive_ttl_secs == 0 {
                                None
                            } else {
                                Some(cfg.sensitive_ttl_secs as i64 * 1000)
                            }
                        });
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
                        let new_lamport = copypaste_core::next_lamport_ts(prev_lamport, now_ms);
                        bump_item_recency(&db, &item_id_bump, now_ms, new_lamport, sensitive_ttl_ms)
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
            Ok(Ok((None, _))) => {
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
}
