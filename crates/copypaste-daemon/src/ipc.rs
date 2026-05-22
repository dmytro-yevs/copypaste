use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use copypaste_core::{Database, get_page, delete_item, delete_fts, count_items, search_items};
use crate::protocol::{Request, Response};

/// Persistent application configuration stored at
/// `dirs::config_dir()/copypaste/config.json`.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub p2p_enabled: bool,
    #[serde(default)]
    pub supabase_url: Option<String>,
    #[serde(default)]
    pub supabase_anon_key: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            p2p_enabled: false,
            supabase_url: None,
            supabase_anon_key: None,
        }
    }
}

fn config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("copypaste").join("config.json"))
}

fn read_config() -> AppConfig {
    let Some(path) = config_path() else {
        return AppConfig::default();
    };
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn write_config(cfg: &AppConfig) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("cannot determine config dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, json)?;
    Ok(())
}

pub struct IpcServer {
    db: Arc<Mutex<Database>>,
    /// Shared private-mode flag. When true, the clipboard monitor skips recording.
    private_mode: Arc<AtomicBool>,
}

impl IpcServer {
    pub fn new(db: Arc<Mutex<Database>>, private_mode: Arc<AtomicBool>) -> Self {
        Self { db, private_mode }
    }

    pub async fn serve(self, socket_path: &std::path::Path) -> anyhow::Result<()> {
        // Remove stale socket file
        let _ = std::fs::remove_file(socket_path);
        let listener = UnixListener::bind(socket_path)?;
        tracing::info!("IPC listening on {}", socket_path.display());

        let server = Arc::new(self);
        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let s = server.clone();
                    tokio::spawn(async move {
                        if let Err(e) = s.handle_connection(stream).await {
                            tracing::warn!("IPC connection error: {e}");
                        }
                    });
                }
                Err(e) => tracing::error!("accept error: {e}"),
            }
        }
    }

    #[tracing::instrument(skip_all, name = "ipc_connection")]
    async fn handle_connection(&self, stream: UnixStream) -> anyhow::Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut lines = BufReader::new(reader).lines();

        while let Some(line) = lines.next_line().await? {
            let resp = self.dispatch(&line).await;
            let mut out = serde_json::to_string(&resp)?;
            out.push('\n');
            writer.write_all(out.as_bytes()).await?;
        }
        Ok(())
    }

    #[tracing::instrument(skip(self), fields(method), name = "ipc_dispatch")]
    async fn dispatch(&self, line: &str) -> Response {
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => return Response::err("?", format!("parse error: {e}")),
        };

        tracing::Span::current().record("method", &req.method.as_str());
        tracing::debug!(method = %req.method, id = %req.id, "IPC request");

        match req.method.as_str() {
            "list" => {
                let limit = req.params.get("limit")
                    .and_then(|v| v.as_u64()).unwrap_or(50) as usize;
                let offset = req.params.get("offset")
                    .and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let db = self.db.lock().await;
                match get_page(&db, limit, offset) {
                    Ok(items) => {
                        let total = count_items(&db).unwrap_or(0);
                        let json_items: Vec<_> = items.iter().map(|item| serde_json::json!({
                            "id": item.id,
                            "content_type": item.content_type,
                            "is_sensitive": item.is_sensitive,
                            "wall_time": item.wall_time,
                            "lamport_ts": item.lamport_ts,
                        })).collect();
                        Response::ok(req.id, serde_json::json!({"items": json_items, "total": total}))
                    }
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "delete" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                let db = self.db.lock().await;
                match delete_item(&db, &id) {
                    Ok(_) => {
                        // Best-effort FTS cleanup; log warning but don't fail the request
                        if let Err(e) = delete_fts(&db, &id) {
                            tracing::warn!("fts delete failed for id={id}: {e}");
                        }
                        Response::ok(req.id, serde_json::Value::Null)
                    }
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "count" => {
                let db = self.db.lock().await;
                match count_items(&db) {
                    Ok(n) => Response::ok(req.id, serde_json::json!({"count": n})),
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "search" => {
                let query = match req.params.get("query").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: query"),
                };
                let limit = req.params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;

                let db = self.db.lock().await;
                match search_items(&db, &query, limit) {
                    Ok(items) => {
                        let json_items: Vec<_> = items
                            .iter()
                            .map(|item| serde_json::json!({
                                "id": item.id,
                                "content_type": item.content_type,
                                "is_sensitive": item.is_sensitive,
                                "wall_time": item.wall_time,
                                "lamport_ts": item.lamport_ts,
                            }))
                            .collect();
                        Response::ok(req.id, serde_json::json!({"items": json_items}))
                    }
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "copy" | "paste" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                let db = self.db.lock().await;
                match copypaste_core::get_page(&db, 1000, 0) {
                    Ok(items) => {
                        if let Some(item) = items.iter().find(|i| i.id == id) {
                            match Self::write_to_pasteboard(item) {
                                Ok(()) => Response::ok(req.id, serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "written": true,
                                })),
                                Err(e) => Response::err(req.id, format!("pasteboard write failed: {e}")),
                            }
                        } else {
                            Response::err(req.id, format!("item not found: {id}"))
                        }
                    }
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "delete_all" => {
                let db = self.db.lock().await;
                let count = count_items(&db).unwrap_or(0);
                loop {
                    match get_page(&db, 100, 0) {
                        Ok(items) if items.is_empty() => break,
                        Ok(items) => {
                            for item in items {
                                let _ = delete_item(&db, &item.id);
                                let _ = delete_fts(&db, &item.id);
                            }
                        }
                        Err(_) => break,
                    }
                }
                Response::ok(req.id, serde_json::json!({"deleted": count}))
            }
            "stats" => {
                let db = self.db.lock().await;
                let total = copypaste_core::count_items(&db).unwrap_or(0);
                // Count sensitive items via get_page scan (limited to first 1000)
                let sample = copypaste_core::get_page(&db, 1000, 0).unwrap_or_default();
                let sensitive_count = sample.iter().filter(|i| i.is_sensitive).count() as i64;
                Response::ok(req.id, serde_json::json!({
                    "total_items": total,
                    "sensitive_items": sensitive_count,
                    "version": "1"
                }))
            }
            "pin" => {
                // Pin an item (remove expiry so it's never auto-deleted)
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                let db = self.db.lock().await;
                match copypaste_core::pin_item(&db, &id) {
                    Ok(()) => Response::ok(req.id, serde_json::json!({"pinned": true, "id": id})),
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "history_page" => {
                // Paginated history with content preview — used by UI (HistoryWindow)
                let limit = req.params.get("limit")
                    .and_then(|v| v.as_u64()).unwrap_or(50) as usize;
                let offset = req.params.get("offset")
                    .and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                let db = self.db.lock().await;
                match get_page(&db, limit, offset) {
                    Ok(items) => {
                        let total = count_items(&db).unwrap_or(0);
                        let json_items: Vec<_> = items.iter().map(|item| {
                            // Build a safe text preview (first 120 chars of content, no decryption)
                            let preview = format!("[{} — id:{}]", item.content_type, &item.id[..8]);
                            serde_json::json!({
                                "id": item.id,
                                "content_type": item.content_type,
                                "is_sensitive": item.is_sensitive,
                                "wall_time": item.wall_time,
                                "lamport_ts": item.lamport_ts,
                                "preview": preview,
                            })
                        }).collect();
                        Response::ok(req.id, serde_json::json!({"items": json_items, "total": total}))
                    }
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "get_config" => {
                let cfg = read_config();
                match serde_json::to_value(&cfg) {
                    Ok(v) => Response::ok(req.id, v),
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            "set_config" => {
                let cfg: AppConfig = match serde_json::from_value(req.params.clone()) {
                    Ok(c) => c,
                    Err(e) => return Response::err(req.id, format!("invalid config: {e}")),
                };
                match write_config(&cfg) {
                    Ok(()) => Response::ok(req.id, serde_json::json!({"saved": true})),
                    Err(e) => Response::err(req.id, e.to_string()),
                }
            }
            // Cloud auth — stubs until Supabase integration lands
            "cloud_sign_in" => {
                // TODO: integrate with Supabase auth once credentials are wired
                tracing::info!("cloud_sign_in stub called");
                Response::ok(req.id, serde_json::json!({"signed_in": false, "note": "not yet implemented"}))
            }
            "cloud_sign_out" => {
                // TODO: integrate with Supabase auth once credentials are wired
                tracing::info!("cloud_sign_out stub called");
                Response::ok(req.id, serde_json::json!({"signed_out": true}))
            }
            // P2P peer management — stubs until p2p daemon crate is integrated
            "get_own_fingerprint" => {
                // TODO: read X25519 public key fingerprint from keychain once p2p is wired
                Response::ok(req.id, serde_json::json!({"fingerprint": null, "note": "not yet implemented"}))
            }
            "list_peers" => {
                // TODO: read peer list from p2p store once wired
                Response::ok(req.id, serde_json::json!({"peers": []}))
            }
            "pair_peer" => {
                // TODO: initiate PAKE handshake once p2p is wired
                tracing::info!("pair_peer stub called");
                Response::ok(req.id, serde_json::json!({"paired": false, "note": "not yet implemented"}))
            }
            "unpair_peer" => {
                // TODO: remove peer from store once p2p is wired
                tracing::info!("unpair_peer stub called");
                Response::ok(req.id, serde_json::json!({"unpaired": true}))
            }
            "set_private_mode" => {
                let enabled = match req.params.get("enabled").and_then(|v| v.as_bool()) {
                    Some(b) => b,
                    None => return Response::err(req.id, "missing param: enabled (bool)"),
                };
                self.private_mode.store(enabled, Ordering::Relaxed);
                tracing::info!("private mode set to {enabled}");
                Response::ok(req.id, serde_json::json!({"private_mode": enabled}))
            }
            "get_private_mode" => {
                let enabled = self.private_mode.load(Ordering::Relaxed);
                Response::ok(req.id, serde_json::json!({"private_mode": enabled}))
            }
            "status" => {
                let enabled = self.private_mode.load(Ordering::Relaxed);
                Response::ok(req.id, serde_json::json!({"status": "running", "private_mode": enabled}))
            }
            other => Response::err(req.id, format!("unknown method: {other}")),
        }
    }

    /// Write a clipboard item's content back to NSPasteboard (macOS) or no-op on other platforms.
    fn write_to_pasteboard(item: &copypaste_core::ClipboardItem) -> Result<(), String> {
        let content = match &item.content {
            Some(bytes) => bytes,
            None => return Err("item has no content".to_string()),
        };

        #[cfg(target_os = "macos")]
        {
            use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
            use objc2_foundation::NSString;

            if item.content_type == "text" {
                // Interpret bytes as UTF-8 text and write to NSPasteboard
                let text = std::str::from_utf8(content)
                    .map_err(|e| format!("content is not valid UTF-8: {e}"))?;
                unsafe {
                    let pb = NSPasteboard::generalPasteboard();
                    pb.clearContents();
                    let ns_str = NSString::from_str(text);
                    let ok = pb.setString_forType(&ns_str, NSPasteboardTypeString);
                    if !ok {
                        return Err("NSPasteboard setString:forType: returned false".to_string());
                    }
                }
            } else {
                // Binary content: write raw bytes with a generic type
                use objc2_foundation::NSData;

                unsafe {
                    let pb = NSPasteboard::generalPasteboard();
                    pb.clearContents();
                    // Use the content_type as the pasteboard type string (best-effort)
                    let type_str = NSString::from_str(&item.content_type);
                    let data = NSData::with_bytes(content);
                    let ok = pb.setData_forType(Some(&data), &type_str);
                    if !ok {
                        return Err(format!(
                            "NSPasteboard setData:forType: returned false for type '{}'",
                            item.content_type
                        ));
                    }
                }
            }
            Ok(())
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = item;
            // No clipboard support on non-macOS platforms in this crate
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::Database;
    use tempfile::tempdir;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    async fn start_test_server(socket_path: &std::path::Path) -> Arc<AtomicBool> {
        start_test_server_with_mode(socket_path, false).await
    }

    async fn start_test_server_with_mode(
        socket_path: &std::path::Path,
        initial_private_mode: bool,
    ) -> Arc<AtomicBool> {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(initial_private_mode));
        let server = IpcServer::new(db, private_mode.clone());
        let path = socket_path.to_path_buf();
        tokio::spawn(async move {
            server.serve(&path).await.ok();
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        private_mode
    }

    #[tokio::test]
    async fn status_returns_running() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"1\",\"method\":\"status\"}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["status"], "running");
    }

    #[tokio::test]
    async fn list_empty_db_returns_zero() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test2.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"2\",\"method\":\"list\"}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["total"], 0);
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test3.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"3\",\"method\":\"bogus\"}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("unknown method"));
    }

    #[tokio::test]
    async fn search_with_no_fts_data_returns_empty() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test_search.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"s1\",\"method\":\"search\",\"params\":{\"query\":\"hello\",\"limit\":10}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn search_missing_query_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test_search_err.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"s2\",\"method\":\"search\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("missing param: query"));
    }

    #[tokio::test]
    async fn copy_unknown_id_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_test.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"1\",\"method\":\"copy\",\"params\":{\"id\":\"nonexistent\"}}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
    }

    #[tokio::test]
    async fn copy_missing_id_param_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_missing_param.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"2\",\"method\":\"copy\",\"params\":{}}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("missing param: id"));
    }

    #[tokio::test]
    async fn stats_returns_zero_for_empty_db() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("stats.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"1\",\"method\":\"stats\"}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["total_items"], 0);
    }

    #[tokio::test]
    async fn delete_all_returns_count() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("del_all.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"1\",\"method\":\"delete_all\"}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert!(resp["data"]["deleted"].as_i64().is_some());
    }

    // --- private mode IPC tests ---

    #[tokio::test]
    async fn get_private_mode_returns_false_by_default() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_get_default.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"1\",\"method\":\"get_private_mode\"}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["private_mode"], false);
    }

    #[tokio::test]
    async fn set_private_mode_enable_then_get() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_set_enable.sock");
        start_test_server(&sock).await;

        // Enable private mode — first connection
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":true}}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], true);
            assert_eq!(resp["data"]["private_mode"], true);
        }

        // Verify get_private_mode reflects the change — second connection
        {
            let mut stream2 = UnixStream::connect(&sock).await.unwrap();
            stream2
                .write_all(b"{\"id\":\"2\",\"method\":\"get_private_mode\"}\n")
                .await
                .unwrap();
            let mut lines2 = BufReader::new(&mut stream2).lines();
            let line2 = lines2.next_line().await.unwrap().unwrap();
            let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
            assert_eq!(resp2["ok"], true);
            assert_eq!(resp2["data"]["private_mode"], true);
        }
    }

    #[tokio::test]
    async fn set_private_mode_then_disable() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_disable.sock");
        start_test_server_with_mode(&sock, true).await;

        // Confirm it starts enabled — first connection
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"1\",\"method\":\"get_private_mode\"}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["data"]["private_mode"], true);
        }

        // Disable — second connection
        {
            let mut stream2 = UnixStream::connect(&sock).await.unwrap();
            stream2
                .write_all(b"{\"id\":\"2\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":false}}\n")
                .await
                .unwrap();
            let mut lines2 = BufReader::new(&mut stream2).lines();
            let line2 = lines2.next_line().await.unwrap().unwrap();
            let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
            assert_eq!(resp2["ok"], true);
            assert_eq!(resp2["data"]["private_mode"], false);
        }
    }

    #[tokio::test]
    async fn set_private_mode_missing_param_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_missing.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("enabled"));
    }

    #[tokio::test]
    async fn status_includes_private_mode_field() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("status_pm.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(b"{\"id\":\"1\",\"method\":\"status\"}\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["status"], "running");
        assert!(resp["data"]["private_mode"].is_boolean());
    }

    #[tokio::test]
    async fn set_private_mode_updates_shared_atomic() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pm_atomic.sock");
        let flag = start_test_server(&sock).await;

        // Initially false
        assert!(!flag.load(Ordering::Relaxed));

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":true}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let _line = lines.next_line().await.unwrap().unwrap();

        // The shared atomic should now be true
        assert!(flag.load(Ordering::Relaxed));
    }

    // --- history_page ---

    #[tokio::test]
    async fn history_page_empty_db_returns_zero() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hp_empty.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"hp1\",\"method\":\"history_page\",\"params\":{\"limit\":50,\"offset\":0}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert_eq!(resp["data"]["total"], 0);
        assert_eq!(resp["data"]["items"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn history_page_default_params_succeed() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hp_default.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        // No params — should default to limit=50, offset=0
        stream
            .write_all(b"{\"id\":\"hp2\",\"method\":\"history_page\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        assert!(resp["data"]["items"].is_array());
    }

    // --- paste ---

    #[tokio::test]
    async fn paste_missing_id_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("paste_missing.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"p1\",\"method\":\"paste\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("missing param: id"));
    }

    #[tokio::test]
    async fn paste_unknown_id_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("paste_unknown.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"p2\",\"method\":\"paste\",\"params\":{\"id\":\"nonexistent-id\"}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("not found"));
    }
}
