use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use copypaste_core::{Database, get_page, delete_item, delete_fts, count_items, search_items};
use crate::protocol::{Request, Response};

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
            "status" => Response::ok(req.id, serde_json::json!({"status": "running"})),
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
}
