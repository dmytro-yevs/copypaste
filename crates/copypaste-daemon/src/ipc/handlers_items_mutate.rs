//! Clipboard-item mutation IPC verbs — delete/pin/reorder (split from
//! handlers_items.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.15).
use super::*;

impl IpcServer {
    pub(crate) async fn handle_delete(&self, req: Request) -> Response {
        // P2-8u2b: tag with ERR_CODE_INVALID_ARGUMENT so machine
        // clients can classify the error rather than getting a bare
        // untyped error string.
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
        match self.soft_delete_and_broadcast(&id).await {
            Ok(_) => Response::ok(req.id, serde_json::Value::Null),
            Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
        }
    }

    pub(crate) async fn handle_delete_all(&self, req: Request) -> Response {
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
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
                })?
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
                let new_lamport = copypaste_core::next_lamport_ts(*prev_lamport, now_ms);
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

    pub(crate) async fn handle_pin(&self, req: Request) -> Response {
        // Pin an item (remove expiry so it's never auto-deleted)
        // CopyPaste-kfe9: tag with ERR_CODE_INVALID_ARGUMENT so
        // machine clients can classify the error (follow-up of 8u2b).
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
            Ok(Err(e)) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string()),
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
    pub(crate) async fn handle_pin_item(&self, req: Request) -> Response {
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
    pub(crate) async fn handle_reorder_pinned(&self, req: Request) -> Response {
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
            let mut rows: Vec<copypaste_core::ClipboardItem> = Vec::with_capacity(id_refs.len());
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
    pub(crate) async fn handle_delete_item(&self, req: Request) -> Response {
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
}
