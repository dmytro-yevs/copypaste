//! App-icon lookup + external file ingest IPC verbs (split from
//! handlers_items.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.15).
use super::*;

impl IpcServer {
    pub(crate) async fn handle_get_app_icon(&self, req: Request) -> Response {
        let bundle_id = match req.params.get("bundle_id").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => return Response::err(req.id, "missing param: bundle_id"),
        };
        // NSWorkspace / AppKit calls are blocking — offload to a
        // dedicated blocking thread so we never stall the async runtime.
        let join =
            tokio::task::spawn_blocking(move || crate::app_icon::get_app_icon_base64(&bundle_id))
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
    pub(crate) async fn handle_add_file_item(&self, req: Request) -> Response {
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
        let data_b64 = match extract_str_param(
            &req.params,
            req.id.clone(),
            "data_b64",
            "missing param: data_b64",
        ) {
            Ok(s) => s,
            Err(resp) => return resp,
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

            let blob = copypaste_core::chunks_to_blob(&chunks).context("chunks_to_blob failed")?;

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
}
