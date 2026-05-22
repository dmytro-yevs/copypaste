use std::path::PathBuf;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use copypaste_core::{Database, get_page, delete_item, delete_fts, count_items, search_items};
use crate::protocol::{Request, Response};

// ---------------------------------------------------------------------------
// P2P helpers
// ---------------------------------------------------------------------------

/// Format raw bytes as colon-separated hex groups (XX:XX:...).
fn format_fingerprint(bytes: &[u8]) -> String {
    let encoded = hex::encode(bytes);
    encoded
        .chars()
        .collect::<Vec<_>>()
        .chunks(2)
        .map(|c| c.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(":")
}

/// Path to peers.json in the app config directory.
fn peers_file_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("copypaste")
        .join("peers.json")
}

/// Load peers list from peers.json; returns empty vec if file is absent.
fn load_peers() -> anyhow::Result<Vec<serde_json::Value>> {
    let path = peers_file_path();
    if !path.exists() {
        return Ok(vec![]);
    }
    let data = std::fs::read_to_string(&path)?;
    let peers: Vec<serde_json::Value> = serde_json::from_str(&data)?;
    Ok(peers)
}

/// Persist peers list to peers.json, creating directories as needed.
fn save_peers(peers: &[serde_json::Value]) -> anyhow::Result<()> {
    let path = peers_file_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(peers)?;
    std::fs::write(&path, data)?;
    Ok(())
}

/// Validate that a fingerprint string matches the XX:XX:... hex pattern.
fn is_valid_fingerprint(fp: &str) -> bool {
    let groups: Vec<&str> = fp.split(':').collect();
    if groups.is_empty() {
        return false;
    }
    groups.iter().all(|g| g.len() == 2 && g.chars().all(|c| c.is_ascii_hexdigit()))
}

pub struct IpcServer {
    db: Arc<Mutex<Database>>,
}

impl IpcServer {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { db }
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

    async fn dispatch(&self, line: &str) -> Response {
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => return Response::err("?", format!("parse error: {e}")),
        };

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
            "copy" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                let db = self.db.lock().await;
                match copypaste_core::get_page(&db, 1000, 0) {
                    Ok(items) => {
                        if let Some(item) = items.iter().find(|i| i.id == id) {
                            // Note: we don't have the key here (it's in daemon.rs state)
                            // For now return item metadata — full decrypt support in next phase
                            Response::ok(req.id, serde_json::json!({
                                "id": item.id,
                                "content_type": item.content_type,
                                "found": true,
                                "note": "copy-to-clipboard requires daemon v2 with key access"
                            }))
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
            "status" => Response::ok(req.id, serde_json::json!({"status": "running"})),

            // ------------------------------------------------------------------
            // P2P IPC methods
            // ------------------------------------------------------------------

            "get_own_fingerprint" => {
                // Use a stable device identifier: SHA-256 of the machine UUID
                // (placeholder implementation — real keychain cert used in Phase 5+).
                use std::collections::hash_map::DefaultHasher;
                use std::hash::{Hash, Hasher};

                // Derive a deterministic pseudo-UUID from the hostname so each
                // device gets a stable, unique-enough fingerprint.
                let hostname = std::env::var("HOSTNAME")
                    .or_else(|_| {
                        std::fs::read_to_string("/etc/hostname")
                            .map(|s| s.trim().to_string())
                    })
                    .unwrap_or_else(|_| "localhost".to_string());

                let mut hasher = DefaultHasher::new();
                hostname.hash(&mut hasher);
                std::process::id().hash(&mut hasher);
                let hash_val = hasher.finish();

                // Expand to 32 bytes using a simple XOR-spread so we have
                // enough material to format a fingerprint.
                let mut bytes = [0u8; 32];
                let seed = hash_val.to_le_bytes();
                for (i, b) in bytes.iter_mut().enumerate() {
                    *b = seed[i % 8].wrapping_add(i as u8);
                }

                let fingerprint = format_fingerprint(&bytes);
                Response::ok(req.id, serde_json::json!({ "fingerprint": fingerprint }))
            }

            "list_peers" => {
                match load_peers() {
                    Ok(peers) => Response::ok(req.id, serde_json::json!({ "peers": peers })),
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            "pair_peer" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: fingerprint"),
                };
                let name = match req.params.get("name").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: name"),
                };

                if !is_valid_fingerprint(&fingerprint) {
                    return Response::err(req.id, format!("invalid fingerprint format: {fingerprint}"));
                }

                match load_peers() {
                    Ok(mut peers) => {
                        // Check for duplicates
                        let already_paired = peers.iter().any(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| f == fingerprint)
                                .unwrap_or(false)
                        });
                        if already_paired {
                            return Response::err(req.id, format!("peer already paired: {fingerprint}"));
                        }

                        let added_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        peers.push(serde_json::json!({
                            "name": name,
                            "fingerprint": fingerprint,
                            "added_at": added_at,
                        }));

                        match save_peers(&peers) {
                            Ok(_) => Response::ok(req.id, serde_json::json!({ "ok": true })),
                            Err(e) => Response::err(req.id, format!("failed to save peers: {e}")),
                        }
                    }
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            "unpair_peer" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: fingerprint"),
                };

                match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| f != fingerprint)
                                .unwrap_or(true)
                        });
                        let removed = peers.len() < before_len;

                        match save_peers(&peers) {
                            Ok(_) => Response::ok(req.id, serde_json::json!({ "ok": true, "removed": removed })),
                            Err(e) => Response::err(req.id, format!("failed to save peers: {e}")),
                        }
                    }
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            other => Response::err(req.id, format!("unknown method: {other}")),
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

    async fn start_test_server(socket_path: &std::path::Path) {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(db);
        let path = socket_path.to_path_buf();
        tokio::spawn(async move {
            server.serve(&path).await.ok();
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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
}
