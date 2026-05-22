//! IPC layer — newline-delimited JSON over a platform-specific transport.
//!
//! `dispatch()` is shared across platforms; only the transport (Unix socket vs
//! Windows named pipe) differs.

use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex;
use copypaste_core::{Database, get_page, delete_item, delete_fts, count_items, search_items};
use crate::protocol::{Request, Response};

pub mod transport;

#[cfg(unix)]
pub mod unix;

#[cfg(windows)]
pub mod windows;

// Re-export the platform-selected server as `IpcServer` for daemon.rs.
pub use transport::IpcServer;

/// Shared dispatch logic — called by both Unix and Windows connection handlers.
pub(crate) async fn dispatch(db: &Arc<Mutex<Database>>, line: &str) -> Response {
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
            let db = db.lock().await;
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
            let db = db.lock().await;
            match delete_item(&db, &id) {
                Ok(_) => {
                    if let Err(e) = delete_fts(&db, &id) {
                        tracing::warn!("fts delete failed for id={id}: {e}");
                    }
                    Response::ok(req.id, serde_json::Value::Null)
                }
                Err(e) => Response::err(req.id, e.to_string()),
            }
        }
        "count" => {
            let db = db.lock().await;
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
            let db = db.lock().await;
            match search_items(&db, &query, limit) {
                Ok(items) => {
                    let json_items: Vec<_> = items.iter().map(|item| serde_json::json!({
                        "id": item.id,
                        "content_type": item.content_type,
                        "is_sensitive": item.is_sensitive,
                        "wall_time": item.wall_time,
                        "lamport_ts": item.lamport_ts,
                    })).collect();
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
            let db = db.lock().await;
            match copypaste_core::get_page(&db, 1000, 0) {
                Ok(items) => {
                    if let Some(item) = items.iter().find(|i| i.id == id) {
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
            let db = db.lock().await;
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
            let db = db.lock().await;
            let total = copypaste_core::count_items(&db).unwrap_or(0);
            let sample = copypaste_core::get_page(&db, 1000, 0).unwrap_or_default();
            let sensitive_count = sample.iter().filter(|i| i.is_sensitive).count() as i64;
            Response::ok(req.id, serde_json::json!({
                "total_items": total,
                "sensitive_items": sensitive_count,
                "version": "1"
            }))
        }
        "pin" => {
            let id = match req.params.get("id").and_then(|v| v.as_str()) {
                Some(s) => s.to_string(),
                None => return Response::err(req.id, "missing param: id"),
            };
            let db = db.lock().await;
            match copypaste_core::pin_item(&db, &id) {
                Ok(()) => Response::ok(req.id, serde_json::json!({"pinned": true, "id": id})),
                Err(e) => Response::err(req.id, e.to_string()),
            }
        }
        "status" => Response::ok(req.id, serde_json::json!({"status": "running"})),
        other => Response::err(req.id, format!("unknown method: {other}")),
    }
}

/// Shared connection handler — platform-agnostic.
///
/// Takes any `AsyncRead + AsyncWrite` stream (UnixStream or NamedPipeServer)
/// and processes newline-delimited JSON requests until EOF.
pub(crate) async fn handle_connection<S>(
    db: Arc<Mutex<Database>>,
    stream: S,
) -> anyhow::Result<()>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let resp = dispatch(&db, &line).await;
        let mut out = serde_json::to_string(&resp)?;
        out.push('\n');
        writer.write_all(out.as_bytes()).await?;
    }
    Ok(())
}
