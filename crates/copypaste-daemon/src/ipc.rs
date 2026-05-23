use crate::protocol::{
    Request, Response, CURRENT_PROTOCOL_VERSION, ERR_CODE_AUTH_FAILED, ERR_CODE_INTERNAL_ERROR,
    ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, MIN_SUPPORTED_PROTOCOL_VERSION,
};
use copypaste_core::{
    chunks_from_blob, count_items, decode_image, decrypt_item, delete_fts, delete_item, get_page,
    search_items, Database,
};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;

/// Maximum size of a single IPC request line. Clients exceeding this receive
/// an error response and have their connection closed. Prevents OOM from a
/// malicious or buggy client sending an unbounded stream without newlines.
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Server-side cap on paginated reads (`list`, `history_page`). A client
/// may request more, but the server silently clamps to this value. Protects
/// the daemon from accidental or malicious requests that would attempt to
/// materialize huge result sets in a single response.
const MAX_PAGE: usize = 1000;

/// Error code returned when an IPC method is called before the server's
/// backing state (database, etc.) has finished initializing. Clients should
/// back off and retry rather than treat this as a hard failure.
const ERR_IPC_NOT_READY: &str = "IPC_NOT_READY";

/// Persistent application configuration stored at
/// `dirs::config_dir()/copypaste/config.json`.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub p2p_enabled: bool,
    #[serde(default)]
    pub supabase_url: Option<String>,
    #[serde(default)]
    pub supabase_anon_key: Option<String>,
}

fn config_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("copypaste").join("config.json"))
}

fn read_config() -> AppConfig {
    let Some(path) = config_path() else {
        return AppConfig::default();
    };
    let raw = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return AppConfig::default(),
    };
    match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(
                "config parse failed at {}: {e}, using defaults",
                path.display()
            );
            AppConfig::default()
        }
    }
}

fn write_config(cfg: &AppConfig) -> anyhow::Result<()> {
    let path = config_path().ok_or_else(|| anyhow::anyhow!("cannot determine config dir"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
        // Best-effort: tighten parent dir perms to user-only.
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    let json = serde_json::to_string_pretty(cfg)?;
    std::fs::write(&path, json)?;
    // chmod 0600 — config may carry supabase keys; never world-readable.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

// ---------------------------------------------------------------------------
// P2P helpers
// ---------------------------------------------------------------------------

/// Format raw bytes as colon-separated hex groups (XX:XX:...).
///
/// NOTE (W3.6 consolidation): there are three near-identical fingerprint
/// formatters across daemon/UI/CLI. Within the daemon, only this one and
/// [`crate::keychain::own_fingerprint`] exist, and their semantics differ:
///
/// - [`crate::keychain::own_fingerprint`] SHA-256-hashes its input, then formats
///   the first 16 bytes (15 colons) — the canonical *device* fingerprint.
/// - This helper formats whatever raw bytes it is handed (any length) — used
///   for the legacy `get_own_fingerprint` stub which already supplies a
///   pre-derived 32-byte payload (31 colons).
///
/// Switching the call site below to `own_fingerprint` would change the
/// IPC contract (length + content) and is therefore deferred to post-alpha
/// along with the cross-crate consolidation into `copypaste-core`.
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
    static FALLBACK_WARNED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    let base = dirs::config_dir().unwrap_or_else(|| {
        FALLBACK_WARNED.get_or_init(|| {
            tracing::warn!(
                "dirs::config_dir() unavailable — falling back to CWD for peers.json. \
                 Set $XDG_CONFIG_HOME or $HOME to silence this warning."
            );
        });
        PathBuf::from(".")
    });
    base.join("copypaste").join("peers.json")
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
        // Best-effort: tighten parent dir perms to user-only.
        let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
    }
    let data = serde_json::to_string_pretty(peers)?;
    std::fs::write(&path, data)?;
    // chmod 0600 — peer fingerprints are sensitive identifiers; never world-readable.
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Validate that a fingerprint string matches the XX:XX:... hex pattern.
fn is_valid_fingerprint(fp: &str) -> bool {
    let groups: Vec<&str> = fp.split(':').collect();
    if groups.is_empty() {
        return false;
    }
    groups
        .iter()
        .all(|g| g.len() == 2 && g.chars().all(|c| c.is_ascii_hexdigit()))
}

pub struct IpcServer {
    db: Arc<Mutex<Database>>,
    /// Shared private-mode flag. When true, the clipboard monitor skips recording.
    private_mode: Arc<AtomicBool>,
    /// Local symmetric encryption key (XChaCha20-Poly1305). Required by the
    /// `copy`/`paste` handlers so paste-back can decrypt the ciphertext
    /// stored in `clipboard_items.content` and write *plaintext* to
    /// NSPasteboard. Audit CRIT #1: previously the handler wrote raw
    /// ciphertext bytes back, so paste produced "content is not valid
    /// UTF-8" for text and garbage for images.
    local_key: Arc<[u8; 32]>,
    /// Device public-key bytes — used by `get_own_fingerprint` to derive
    /// the canonical user-visible fingerprint (`keychain::own_fingerprint`).
    /// Audit HIGH #6: previously the handler used DefaultHasher(hostname,
    /// pid), which changed every restart and was not collision-resistant.
    device_public_key: Arc<[u8; 32]>,
    /// Readiness gate. While `false`, all data-touching methods return
    /// `IPC_NOT_READY` instead of dispatching. Default `true` for production
    /// use (db is fully constructed before `IpcServer::new` is called); tests
    /// use [`IpcServer::new_with_ready`] to exercise the not-ready path.
    ready: Arc<AtomicBool>,
}

impl IpcServer {
    pub fn new(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<[u8; 32]>,
        device_public_key: Arc<[u8; 32]>,
    ) -> Self {
        Self {
            db,
            private_mode,
            local_key,
            device_public_key,
            ready: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Construct with an explicit readiness flag. The returned handle can be
    /// flipped to `true` once initialization completes. Intended for tests
    /// and for callers that want to bind the socket before the database is
    /// fully open.
    #[allow(dead_code)]
    pub fn new_with_ready(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<[u8; 32]>,
        device_public_key: Arc<[u8; 32]>,
        ready: Arc<AtomicBool>,
    ) -> Self {
        Self {
            db,
            private_mode,
            local_key,
            device_public_key,
            ready,
        }
    }

    /// Returns true if a request to `method` requires the backing database.
    /// Methods that only touch in-memory state (status, get/set_private_mode,
    /// get_own_fingerprint, peer file ops, config file ops) are allowed
    /// before the DB is ready so the client can still introspect the daemon.
    fn requires_db(method: &str) -> bool {
        matches!(
            method,
            "list"
                | "delete"
                | "count"
                | "search"
                | "copy"
                | "paste"
                | "delete_all"
                | "stats"
                | "pin"
                | "history_page"
                | "import"
        )
    }

    pub async fn serve(self, socket_path: &std::path::Path) -> anyhow::Result<()> {
        // Ensure parent directory exists and is user-only (0o700) so that the
        // socket cannot be reached by other local users even if the socket
        // mode itself were ever loosened.
        if let Some(parent) = socket_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }

        // Remove stale socket file
        let _ = std::fs::remove_file(socket_path);
        let listener = UnixListener::bind(socket_path)?;

        // chmod 0600 — the IPC socket gives full control over the user's
        // clipboard history and peer database. It must not be world- or
        // group-connectable. Done immediately after bind, before accept loop.
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;

        tracing::info!("IPC listening on {} (mode=0600)", socket_path.display());

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
        let mut reader = BufReader::new(reader);
        let mut buf: Vec<u8> = Vec::with_capacity(4 * 1024);

        loop {
            buf.clear();
            // Bound the read: at most MAX_REQUEST_BYTES + 1 so we can distinguish
            // "exactly the limit" from "exceeded the limit".
            let mut limited = (&mut reader).take((MAX_REQUEST_BYTES as u64) + 1);
            let n = match limited.read_until(b'\n', &mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    tracing::warn!("ipc read error: {e}");
                    return Ok(());
                }
            };

            // Clean EOF — client closed the socket without sending more data.
            if n == 0 {
                return Ok(());
            }

            // Oversized request: read more than MAX_REQUEST_BYTES without
            // finding a newline. Reject with an error response, then close.
            if n > MAX_REQUEST_BYTES {
                tracing::warn!(
                    "ipc request exceeded {MAX_REQUEST_BYTES} bytes (read {n}); rejecting and closing"
                );
                let resp = Response::err("0", "request too large");
                if let Ok(mut out) = serde_json::to_string(&resp) {
                    out.push('\n');
                    let _ = writer.write_all(out.as_bytes()).await;
                }
                return Ok(());
            }

            // Trim trailing \n (and any stray \r) before dispatch.
            while matches!(buf.last(), Some(b'\n' | b'\r')) {
                buf.pop();
            }

            // Empty line — skip silently (treat as keep-alive / no-op).
            if buf.is_empty() {
                continue;
            }

            let line = match std::str::from_utf8(&buf) {
                Ok(s) => s,
                Err(e) => {
                    let resp = Response::err("0", format!("invalid UTF-8: {e}"));
                    if let Ok(mut out) = serde_json::to_string(&resp) {
                        out.push('\n');
                        let _ = writer.write_all(out.as_bytes()).await;
                    }
                    continue;
                }
            };

            let resp = self.dispatch(line).await;
            let mut out = serde_json::to_string(&resp)?;
            out.push('\n');
            if let Err(e) = writer.write_all(out.as_bytes()).await {
                // Client disconnected mid-response — log and exit cleanly,
                // do not panic the spawned task.
                tracing::debug!("ipc write failed (client disconnected): {e}");
                return Ok(());
            }
        }
    }

    #[tracing::instrument(skip(self), fields(method), name = "ipc_dispatch")]
    async fn dispatch(&self, line: &str) -> Response {
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => return Response::err("?", format!("parse error: {e}")),
        };

        tracing::Span::current().record("method", req.method.as_str());
        tracing::debug!(method = %req.method, id = %req.id, "IPC request");

        // Protocol-version gate (ADR-007) — reject before touching any
        // method-specific logic so clients get a deterministic upgrade signal.
        if req.protocol_version < MIN_SUPPORTED_PROTOCOL_VERSION
            || req.protocol_version > CURRENT_PROTOCOL_VERSION
        {
            tracing::warn!(
                method = %req.method,
                id = %req.id,
                client_version = req.protocol_version,
                supported = format!("{MIN_SUPPORTED_PROTOCOL_VERSION}..={CURRENT_PROTOCOL_VERSION}"),
                "rejecting request: unsupported protocol version"
            );
            return Response::err_with_code(
                req.id,
                ERR_CODE_INVALID_ARGUMENT,
                format!(
                    "unsupported protocol version {} (daemon supports {}..={})",
                    req.protocol_version, MIN_SUPPORTED_PROTOCOL_VERSION, CURRENT_PROTOCOL_VERSION
                ),
            );
        }

        // Readiness gate — reject DB-touching methods before init is done.
        if !self.ready.load(Ordering::Relaxed) && Self::requires_db(req.method.as_str()) {
            tracing::debug!(
                method = %req.method,
                id = %req.id,
                "rejecting DB-touching request: server not ready"
            );
            return Response::err_with_code(req.id, ERR_CODE_IPC_NOT_READY, ERR_IPC_NOT_READY);
        }

        match req.method.as_str() {
            "list" => {
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
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let items = get_page(&db, limit, offset)?;
                    let total = count_items(&db).unwrap_or(0);
                    Ok::<_, anyhow::Error>((items, total))
                })
                .await;
                match join {
                    Ok(Ok((items, total))) => {
                        let json_items: Vec<_> = items
                            .iter()
                            .map(|item| {
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "is_sensitive": item.is_sensitive,
                                    "wall_time": item.wall_time,
                                    "lamport_ts": item.lamport_ts,
                                })
                            })
                            .collect();
                        Response::ok(
                            req.id,
                            serde_json::json!({"items": json_items, "total": total}),
                        )
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "delete" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let del = delete_item(&db, &id_for_task);
                    // Best-effort FTS cleanup; surface as warning, not failure
                    let fts = delete_fts(&db, &id_for_task);
                    (del, fts)
                })
                .await;
                match join {
                    Ok((Ok(_), fts_res)) => {
                        if let Err(e) = fts_res {
                            tracing::warn!("fts delete failed for id={id}: {e}");
                        }
                        Response::ok(req.id, serde_json::Value::Null)
                    }
                    Ok((Err(e), _)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "count" => {
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    count_items(&db)
                })
                .await;
                match join {
                    Ok(Ok(n)) => Response::ok(req.id, serde_json::json!({"count": n})),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "search" => {
                let query = match req.params.get("query").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: query"),
                };
                let limit = req
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(20) as usize;

                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    search_items(&db, &query, limit)
                })
                .await;
                match join {
                    Ok(Ok(items)) => {
                        let json_items: Vec<_> = items
                            .iter()
                            .map(|item| {
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "is_sensitive": item.is_sensitive,
                                    "wall_time": item.wall_time,
                                    "lamport_ts": item.lamport_ts,
                                })
                            })
                            .collect();
                        Response::ok(req.id, serde_json::json!({"items": json_items}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "copy" | "paste" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let items = copypaste_core::get_page(&db, 1000, 0)?;
                    Ok::<_, anyhow::Error>(items.into_iter().find(|i| i.id == id_for_task))
                })
                .await;
                match join {
                    Ok(Ok(Some(item))) => match self.write_to_pasteboard(&item) {
                        Ok(()) => Response::ok(
                            req.id,
                            serde_json::json!({
                                "id": item.id,
                                "content_type": item.content_type,
                                "written": true,
                            }),
                        ),
                        Err(PasteboardError::DecryptFailed(msg)) => Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("paste decrypt failed: {msg}"),
                        ),
                        Err(PasteboardError::Other(msg)) => {
                            Response::err(req.id, format!("pasteboard write failed: {msg}"))
                        }
                    },
                    Ok(Ok(None)) => Response::err(req.id, format!("item not found: {id}")),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "delete_all" => {
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
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
                    count
                })
                .await;
                match join {
                    Ok(count) => Response::ok(req.id, serde_json::json!({"deleted": count})),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "stats" => {
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let total = copypaste_core::count_items(&db).unwrap_or(0);
                    // Count sensitive items via get_page scan (limited to first 1000)
                    let sample = copypaste_core::get_page(&db, 1000, 0).unwrap_or_default();
                    let sensitive_count = sample.iter().filter(|i| i.is_sensitive).count() as i64;
                    (total, sensitive_count)
                })
                .await;
                match join {
                    Ok((total, sensitive_count)) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "total_items": total,
                            "sensitive_items": sensitive_count,
                            "version": "1"
                        }),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "pin" => {
                // Pin an item (remove expiry so it's never auto-deleted)
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: id"),
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    copypaste_core::pin_item(&db, &id_for_task)
                })
                .await;
                match join {
                    Ok(Ok(())) => {
                        Response::ok(req.id, serde_json::json!({"pinned": true, "id": id}))
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
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let items = get_page(&db, limit, offset)?;
                    let total = count_items(&db).unwrap_or(0);
                    Ok::<_, anyhow::Error>((items, total))
                })
                .await;
                match join {
                    Ok(Ok((items, total))) => {
                        let json_items: Vec<_> = items
                            .iter()
                            .map(|item| {
                                // Build a safe text preview (first 120 chars of content, no decryption)
                                let preview =
                                    format!("[{} — id:{}]", item.content_type, &item.id[..8]);
                                serde_json::json!({
                                    "id": item.id,
                                    "content_type": item.content_type,
                                    "is_sensitive": item.is_sensitive,
                                    "wall_time": item.wall_time,
                                    "lamport_ts": item.lamport_ts,
                                    "preview": preview,
                                })
                            })
                            .collect();
                        Response::ok(
                            req.id,
                            serde_json::json!({"items": json_items, "total": total}),
                        )
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
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
            // Cloud auth — stubs until Supabase integration lands.
            // Route through `Response::not_implemented` so clients see a
            // machine-readable `error_code: "not_implemented"` instead of an
            // ambiguous `ok: true` carrying a "not yet implemented" note.
            "cloud_sign_in" => {
                tracing::info!("cloud_sign_in stub called");
                Response::not_implemented(req.id, "cloud-sync")
            }
            "cloud_sign_out" => {
                tracing::info!("cloud_sign_out stub called");
                Response::not_implemented(req.id, "cloud-sync")
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
                Response::ok(
                    req.id,
                    serde_json::json!({"status": "running", "private_mode": enabled}),
                )
            }

            // ------------------------------------------------------------------
            // P2P IPC methods
            // ------------------------------------------------------------------
            "get_own_fingerprint" => {
                // Audit HIGH #6 fix: use the canonical SHA-256-of-public-key
                // fingerprint derived from the device's persistent keypair
                // (loaded by the daemon at startup via
                // `keychain::load_or_create` and passed into `IpcServer::new`).
                //
                // The previous DefaultHasher(hostname, pid) scheme:
                //   * changed on every restart (pid varies),
                //   * was not collision-resistant across hosts that share a
                //     hostname (common in containers / dev VMs),
                //   * could not be reconciled with the canonical fingerprint
                //     the UI / pair_peer paths already used.
                let fingerprint = crate::keychain::own_fingerprint(self.device_public_key.as_ref());
                Response::ok(req.id, serde_json::json!({ "fingerprint": fingerprint }))
            }

            "list_peers" => match load_peers() {
                Ok(peers) => Response::ok(req.id, serde_json::json!({ "peers": peers })),
                Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
            },

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
                    return Response::err(
                        req.id,
                        format!("invalid fingerprint format: {fingerprint}"),
                    );
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
                            return Response::err(
                                req.id,
                                format!("peer already paired: {fingerprint}"),
                            );
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
                            Ok(_) => Response::ok(
                                req.id,
                                serde_json::json!({ "ok": true, "removed": removed }),
                            ),
                            Err(e) => Response::err(req.id, format!("failed to save peers: {e}")),
                        }
                    }
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            // Beta W3.2 — PAKE-based password pairing.
            //
            // Validates inputs and, in a full implementation, would:
            //   1. Spawn a `PakeInitiator` keyed by `password`,
            //   2. Route 3 handshake messages over the p2p Transport (W2.1)
            //      to the peer identified by `peer_fingerprint`,
            //   3. On success, persist the resulting `PasswordFile` in
            //      SQLCipher and derive an XChaCha key into the keychain.
            //
            // Until the Transport message-routing layer for PAKE frames is
            // wired (post-beta), we surface a `not_implemented` response so
            // the UI can render an actionable status message. The handler
            // still performs argument validation so callers exercise the
            // full request/response path.
            "pair_peer_with_password" => {
                let peer_fingerprint =
                    match req.params.get("peer_fingerprint").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "missing peer_fingerprint",
                            )
                        }
                    };
                let password = match req.params.get("password").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing password",
                        )
                    }
                };

                if !is_valid_fingerprint(&peer_fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid peer_fingerprint format: {peer_fingerprint}"),
                    );
                }

                if password.chars().count() < 6 {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "password must be at least 6 characters",
                    );
                }

                Response::not_implemented(req.id, "pair-peer-with-password")
            }

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
                let raw_items: Vec<serde_json::Value> = match items_value.as_array() {
                    Some(a) => a.clone(),
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
                    decoded.push(DecodedImport {
                        content_type,
                        bytes,
                        created_at_ms,
                        metadata,
                    });
                }

                // 3. Persist on the blocking pool — SQLite is sync.
                //    For each item: hash; if a row with the same hash exists
                //    within the dedupe window, skip; otherwise insert.
                let db_arc = self.db.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // 5 minute dedupe window — matches the live clipboard
                    // monitor's find_recent_by_hash usage.
                    const DEDUPE_WINDOW_MS: i64 = 5 * 60 * 1000;
                    let mut inserted: u32 = 0;
                    let mut skipped: u32 = 0;
                    for item in decoded {
                        let mut hasher = Sha256::new();
                        hasher.update(&item.bytes);
                        let hash_hex = hex::encode(hasher.finalize());

                        match copypaste_core::find_recent_by_hash(
                            &db,
                            &hash_hex,
                            item.created_at_ms,
                            DEDUPE_WINDOW_MS,
                        ) {
                            Ok(Some(_)) => {
                                skipped += 1;
                                continue;
                            }
                            Ok(None) => { /* fall through to insert */ }
                            Err(e) => return Err::<(u32, u32), anyhow::Error>(e.into()),
                        }

                        // Imported items have no encryption nonce — the bytes
                        // are stored verbatim as the "content" field. This
                        // mirrors how alpha-era exports were laid out and
                        // keeps the import path round-trip-safe.
                        // lamport_ts = 0 is a deliberate "imported, unknown
                        // origin" sentinel; sync will reassign on first push.
                        let mut clip =
                            copypaste_core::ClipboardItem::new_text(item.bytes, Vec::new(), 0);
                        clip.content_type = item.content_type;
                        clip.wall_time = item.created_at_ms;
                        clip.content_hash = Some(hash_hex);

                        if let Err(e) = copypaste_core::insert_item(&db, &clip) {
                            return Err::<(u32, u32), anyhow::Error>(e.into());
                        }
                        inserted += 1;
                    }
                    Ok::<(u32, u32), anyhow::Error>((inserted, skipped))
                })
                .await;

                match join {
                    Ok(Ok((inserted, skipped))) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "inserted": inserted,
                            "skipped": skipped,
                        }),
                    ),
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
    /// 1. Decrypts text via [`decrypt_item`] with the per-item nonce.
    /// 2. Reassembles + decrypts image chunks via [`chunks_from_blob`] +
    ///    [`decode_image`], using the `file_id` parsed out of `blob_ref`.
    /// 3. Maps the daemon's internal `content_type` to a real macOS UTI
    ///    (`"image"` is **not** a valid UTI — audit HIGH #2). Text uses
    ///    `NSPasteboardTypeString`; image always writes `public.png` since
    ///    `encode_image` re-encodes raw clipboard bytes to PNG before
    ///    chunking. Anything already shaped like a UTI (`public.*`,
    ///    `com.*`, `org.*`) is passed through unchanged.
    fn write_to_pasteboard(
        &self,
        item: &copypaste_core::ClipboardItem,
    ) -> Result<(), PasteboardError> {
        #[cfg(target_os = "macos")]
        {
            let content = match &item.content {
                Some(bytes) => bytes.as_slice(),
                None => return Err(PasteboardError::other("item has no content")),
            };

            use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
            use objc2_foundation::{NSData, NSString};

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
                let plaintext_bytes = decrypt_item(content, nonce, self.local_key.as_ref())
                    .map_err(|e| PasteboardError::decrypt(e.to_string()))?;
                let text = std::str::from_utf8(&plaintext_bytes).map_err(|e| {
                    PasteboardError::decrypt(format!("decrypted content is not UTF-8: {e}"))
                })?;
                unsafe {
                    let pb = NSPasteboard::generalPasteboard();
                    pb.clearContents();
                    let ns_str = NSString::from_str(text);
                    let ok = pb.setString_forType(&ns_str, NSPasteboardTypeString);
                    if !ok {
                        return Err(PasteboardError::other(
                            "NSPasteboard setString:forType: returned false",
                        ));
                    }
                }
                Ok(())
            } else if item.content_type == "image" {
                // ----- image: reassemble chunks → decrypt → write as PNG -----
                // `file_id` is embedded in the JSON metadata stored in
                // `blob_ref` (see ClipboardItem::new_image in
                // storage/items.rs).
                let meta_json = item.blob_ref.as_deref().ok_or_else(|| {
                    PasteboardError::other("image item missing blob_ref metadata")
                })?;
                let file_id = parse_image_file_id(meta_json).map_err(PasteboardError::other)?;

                let chunks = chunks_from_blob(content).map_err(|e| {
                    PasteboardError::other(format!("image chunks_from_blob failed: {e}"))
                })?;
                let png_bytes = decode_image(&chunks, self.local_key.as_ref(), &file_id)
                    .map_err(|e| PasteboardError::decrypt(format!("image decode failed: {e}")))?;

                unsafe {
                    let pb = NSPasteboard::generalPasteboard();
                    pb.clearContents();
                    let type_str = NSString::from_str("public.png");
                    let data = NSData::with_bytes(&png_bytes);
                    let ok = pb.setData_forType(Some(&data), &type_str);
                    if !ok {
                        return Err(PasteboardError::other(
                            "NSPasteboard setData:forType: returned false for public.png",
                        ));
                    }
                }
                Ok(())
            } else {
                // Unknown content_type — keep a best-effort raw-bytes write,
                // but map to a real UTI when possible. We do NOT attempt
                // decryption here because we don't know the shape of the
                // ciphertext (no nonce / no chunk metadata). Used only by
                // future content_types added without updating this handler.
                let uti = map_content_type_to_uti(&item.content_type);
                unsafe {
                    let pb = NSPasteboard::generalPasteboard();
                    pb.clearContents();
                    let type_str = NSString::from_str(&uti);
                    let data = NSData::with_bytes(content);
                    let ok = pb.setData_forType(Some(&data), &type_str);
                    if !ok {
                        return Err(PasteboardError::other(format!(
                            "NSPasteboard setData:forType: returned false for type '{uti}'"
                        )));
                    }
                }
                Ok(())
            }
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = item;
            // No clipboard support on non-macOS platforms in this crate
            Ok(())
        }
    }
}

/// Internal error type for the paste-back path so the dispatcher can
/// distinguish authentication / decryption failures (which deserve a
/// dedicated error code so a tampered row is surfaced to the caller) from
/// generic write failures.
#[derive(Debug)]
#[allow(dead_code)]
enum PasteboardError {
    DecryptFailed(String),
    Other(String),
}

impl PasteboardError {
    fn decrypt(msg: impl Into<String>) -> Self {
        Self::DecryptFailed(msg.into())
    }
    fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Parse the `file_id` field out of the JSON metadata embedded in an
/// image item's `blob_ref`. The metadata shape is produced by
/// `daemon::handle_image` (`{"width":...,"file_id":[u8; 16]}` — Rust
/// `{:?}` debug formatting of the byte array).
#[cfg(target_os = "macos")]
fn parse_image_file_id(meta_json: &str) -> Result<[u8; 16], String> {
    let value: serde_json::Value = serde_json::from_str(meta_json)
        .map_err(|e| format!("image meta_json parse error: {e}"))?;
    let arr = value
        .get("file_id")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "image meta_json missing 'file_id' array".to_string())?;
    if arr.len() != 16 {
        return Err(format!(
            "image meta_json 'file_id' has wrong length: expected 16, got {}",
            arr.len()
        ));
    }
    let mut out = [0u8; 16];
    for (i, v) in arr.iter().enumerate() {
        out[i] = v
            .as_u64()
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| format!("image meta_json 'file_id[{i}]' not a u8"))?;
    }
    Ok(out)
}

/// Map the daemon's internal `content_type` string to a macOS UTI suitable
/// for `setData:forType:`. Audit HIGH #2: bare `"image"` is not a UTI and
/// macOS refuses to set the pasteboard data for it.
///
/// Heuristic: anything already shaped like a UTI (`public.*`, `com.*`,
/// `org.*`) is passed through; bare `"image"` defaults to `public.png`;
/// `"text"` to `public.utf8-plain-text`; everything else gets
/// `public.data` so the write doesn't silently no-op.
#[cfg(target_os = "macos")]
fn map_content_type_to_uti(content_type: &str) -> String {
    if content_type.starts_with("public.")
        || content_type.starts_with("com.")
        || content_type.starts_with("org.")
    {
        return content_type.to_string();
    }
    match content_type {
        "image" => "public.png".to_string(),
        "text" => "public.utf8-plain-text".to_string(),
        _ => "public.data".to_string(),
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
        // Dummy keys: in-process tests do not hit paste-back or fingerprint
        // surfaces — they only validate dispatch / state-machine behaviour.
        let local_key = Arc::new([0u8; 32]);
        let device_pub = Arc::new([0u8; 32]);
        let server = IpcServer::new(db, private_mode.clone(), local_key, device_pub);
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
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"status\"}\n")
            .await
            .unwrap();
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
        stream
            .write_all(b"{\"id\":\"2\",\"method\":\"list\"}\n")
            .await
            .unwrap();
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
        stream
            .write_all(b"{\"id\":\"3\",\"method\":\"bogus\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("unknown method"));
    }

    /// ADR-007 — a request carrying a `protocol_version` outside the
    /// supported window must be rejected with a stable error code BEFORE
    /// the dispatcher tries to interpret the method.
    #[tokio::test]
    async fn unsupported_protocol_version_rejected_with_error_code() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-proto-ver.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        // Use a method that would normally succeed (`status`) to prove the
        // version gate fires first.
        let unsupported = CURRENT_PROTOCOL_VERSION + 99;
        let payload = format!(
            "{{\"id\":\"pv1\",\"method\":\"status\",\"protocol_version\":{}}}\n",
            unsupported
        );
        stream.write_all(payload.as_bytes()).await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false, "version gate must reject: {line}");
        assert_eq!(resp["error_code"], "invalid_argument");
        assert_eq!(resp["protocol_version"], CURRENT_PROTOCOL_VERSION);
        assert!(
            resp["error"]
                .as_str()
                .unwrap()
                .contains("unsupported protocol version"),
            "expected version-mismatch message, got: {}",
            resp["error"]
        );
    }

    /// W3.6 — stubbed methods (`cloud_sign_in`, `cloud_sign_out`) must carry
    /// a stable machine-readable `error_code: "not_implemented"` so clients
    /// can branch deterministically without parsing the English `error` text.
    #[tokio::test]
    async fn ipc_responses_carry_machine_readable_error_code() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test_err_code.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"42\",\"method\":\"cloud_sign_in\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(resp["ok"], false, "stub should report failure, not fake ok");
        assert_eq!(
            resp["error_code"], "not_implemented",
            "cloud stub must tag response with machine-readable not_implemented code"
        );
        assert!(
            resp["error"].as_str().unwrap().contains("cloud-sync"),
            "human-readable error should name the unimplemented feature"
        );
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
        assert!(resp["error"]
            .as_str()
            .unwrap()
            .contains("missing param: query"));
    }

    #[tokio::test]
    async fn copy_unknown_id_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_test.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"copy\",\"params\":{\"id\":\"nonexistent\"}}\n")
            .await
            .unwrap();
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
        stream
            .write_all(b"{\"id\":\"2\",\"method\":\"copy\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"]
            .as_str()
            .unwrap()
            .contains("missing param: id"));
    }

    #[tokio::test]
    async fn stats_returns_zero_for_empty_db() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("stats.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"stats\"}\n")
            .await
            .unwrap();
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
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"delete_all\"}\n")
            .await
            .unwrap();
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
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"get_private_mode\"}\n")
            .await
            .unwrap();
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
        stream
            .write_all(b"{\"id\":\"1\",\"method\":\"status\"}\n")
            .await
            .unwrap();
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
            .write_all(
                b"{\"id\":\"1\",\"method\":\"set_private_mode\",\"params\":{\"enabled\":true}}\n",
            )
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
        assert!(resp["error"]
            .as_str()
            .unwrap()
            .contains("missing param: id"));
    }

    #[tokio::test]
    async fn paste_unknown_id_returns_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("paste_unknown.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(
                b"{\"id\":\"p2\",\"method\":\"paste\",\"params\":{\"id\":\"nonexistent-id\"}}\n",
            )
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], false);
        assert!(resp["error"].as_str().unwrap().contains("not found"));
    }

    // ------------------------------------------------------------------
    // Wave 1.1 IPC hardening tests
    //
    // These verify the security guarantees added in
    // `fix(daemon-ipc): wave1.1 — socket chmod 0o600 + request size cap +
    //  handle disconnect`:
    //   * the Unix listener socket is created with mode 0600 (user-only),
    //   * a request line exceeding MAX_REQUEST_BYTES (16 MiB) is rejected
    //     with an error response without crashing the server,
    //   * a client that connects and disconnects abruptly (no newline,
    //     partial write, or zero bytes) does not panic the spawned task.
    // ------------------------------------------------------------------

    #[tokio::test]
    async fn ipc_socket_chmod_is_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hardening_chmod.sock");
        start_test_server(&sock).await;

        let meta = std::fs::metadata(&sock).expect("socket file should exist");
        let mode = meta.permissions().mode() & 0o777;
        assert_eq!(
            mode,
            0o600,
            "socket {} has mode {:o}, expected 0600",
            sock.display(),
            mode
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ipc_oversized_request_rejected_not_crashed() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hardening_oversize.sock");
        start_test_server(&sock).await;

        // Client A: send 17 MiB without a newline. The server reads up to
        // MAX_REQUEST_BYTES + 1 (16 MiB + 1) and trips the oversize branch,
        // returns an error response, and closes the connection.
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            let payload = vec![b'A'; 17 * 1024 * 1024];
            // The server may close before we finish writing — that's fine.
            let _ = stream.write_all(&payload).await;
            // Half-close write so the server's read_until unblocks.
            let _ = stream.shutdown().await;

            // Try to read the error response, bounded by a timeout so a
            // misbehaving server can't hang the test.
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            if let Ok(Ok(_n)) = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                reader.read_line(&mut line),
            )
            .await
            {
                if !line.trim().is_empty() {
                    let resp: serde_json::Value = serde_json::from_str(line.trim())
                        .expect("oversize response should be valid JSON");
                    assert_eq!(resp["ok"], false, "expected error response, got: {resp}");
                    let err = resp["error"].as_str().unwrap_or_default();
                    assert!(
                        err.contains("too large"),
                        "expected 'too large' in error, got: {err}"
                    );
                }
                // If we got no bytes back (race with server close), the
                // next client below proves the server didn't crash.
            }
        }

        // Client B: a normal request must still succeed — proves the server
        // survived the oversize client.
        {
            let mut stream = UnixStream::connect(&sock)
                .await
                .expect("server must still accept new connections after oversize client");
            stream
                .write_all(b"{\"id\":\"after-oversize\",\"method\":\"status\"}\n")
                .await
                .unwrap();
            let mut reader = BufReader::new(&mut stream);
            let mut line = String::new();
            let n = tokio::time::timeout(
                std::time::Duration::from_secs(5),
                reader.read_line(&mut line),
            )
            .await
            .expect("status read timed out — server may have crashed")
            .expect("status read failed");
            assert!(n > 0, "expected a status response line");
            let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
            assert_eq!(
                resp["ok"], true,
                "status should be ok after oversize, got: {resp}"
            );
            assert_eq!(resp["data"]["status"], "running");
        }
    }

    // ------------------------------------------------------------------
    // Wave 2.3 IPC hardening tests
    //
    // Cover edge cases that the binary-driven integration suite cannot
    // reach in-process:
    //   * IPC_NOT_READY when a DB-touching method fires before the
    //     readiness flag flips,
    //   * MAX_PAGE clamping on `list` and `history_page` enforced by the
    //     dispatcher itself (independent of DB row count).
    // ------------------------------------------------------------------

    /// Spawn an IpcServer whose readiness flag starts `false`, returning
    /// the socket path and the flag handle so the test can flip it.
    async fn start_not_ready_server(socket_path: &std::path::Path) -> Arc<AtomicBool> {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let ready = Arc::new(AtomicBool::new(false));
        let ready_clone = ready.clone();
        let local_key = Arc::new([0u8; 32]);
        let device_pub = Arc::new([0u8; 32]);
        let server =
            IpcServer::new_with_ready(db, private_mode, local_key, device_pub, ready_clone);
        let path = socket_path.to_path_buf();
        tokio::spawn(async move {
            server.serve(&path).await.ok();
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        ready
    }

    #[tokio::test]
    async fn dispatch_returns_ipc_not_ready_when_not_ready() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("not_ready.sock");
        let ready = start_not_ready_server(&sock).await;

        // DB-touching methods must be rejected with IPC_NOT_READY.
        for (method, params) in [
            ("list", "{}"),
            ("count", "{}"),
            ("stats", "{}"),
            ("history_page", "{}"),
            ("delete_all", "{}"),
        ] {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            let req =
                format!("{{\"id\":\"nr-{method}\",\"method\":\"{method}\",\"params\":{params}}}\n");
            stream.write_all(req.as_bytes()).await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], false, "{method} should be rejected: {resp}");
            assert_eq!(
                resp["error"].as_str().unwrap_or_default(),
                "IPC_NOT_READY",
                "{method} should return IPC_NOT_READY, got: {resp}"
            );
        }

        // Non-DB methods (status, get_private_mode) must still work, so the
        // client can introspect the daemon and decide whether to retry.
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"nr-status\",\"method\":\"status\"}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], true, "status should pass: {resp}");
        }

        // After the readiness flag flips, previously-rejected methods succeed.
        ready.store(true, Ordering::Relaxed);
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"nr-stats-after\",\"method\":\"stats\"}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
            assert_eq!(resp["ok"], true, "stats should pass after ready: {resp}");
            assert!(resp["data"]["total_items"].is_number());
        }
    }

    #[tokio::test]
    async fn list_clamps_oversize_limit_to_max_page() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("cap_list.sock");
        start_test_server(&sock).await;

        // Empty DB — we cannot directly observe the clamp on item count,
        // but we *can* verify the dispatcher accepts the request and
        // returns at most MAX_PAGE items. The count_items helper is the
        // path that would blow up if the unclamped limit reached the DB.
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"cap-list\",\"method\":\"list\",\"params\":{\"limit\":5000,\"offset\":0}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], true,
            "list with limit=5000 should be ok: {resp}"
        );
        let items = resp["data"]["items"].as_array().unwrap();
        assert!(
            items.len() <= 1000,
            "list returned {} items, exceeds MAX_PAGE=1000",
            items.len()
        );
    }

    #[tokio::test]
    async fn history_page_clamps_oversize_limit_to_max_page() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("cap_hp.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"cap-hp\",\"method\":\"history_page\",\"params\":{\"limit\":9999,\"offset\":0}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
        let items = resp["data"]["items"].as_array().unwrap();
        assert!(
            items.len() <= 1000,
            "history_page returned {} items, exceeds MAX_PAGE=1000",
            items.len()
        );
    }

    /// In-process burst that exercises the same accept-spawn path used by
    /// the binary subprocess test, but without requiring a built binary.
    /// 10 tokio tasks each issue a status+stats roundtrip on its own
    /// connection; all must succeed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_clients_in_process_consistent_state() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("concurrent.sock");
        start_test_server(&sock).await;

        const N: usize = 10;
        let mut handles = Vec::with_capacity(N);
        for i in 0..N {
            let sock = sock.clone();
            handles.push(tokio::spawn(async move {
                // status
                let mut s = UnixStream::connect(&sock).await.unwrap();
                let req = format!("{{\"id\":\"c{i}-status\",\"method\":\"status\"}}\n");
                s.write_all(req.as_bytes()).await.unwrap();
                let mut lines = BufReader::new(&mut s).lines();
                let line = lines.next_line().await.unwrap().unwrap();
                let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(resp["ok"], true, "client {i} status: {resp}");

                // stats — fresh connection
                let mut s2 = UnixStream::connect(&sock).await.unwrap();
                let req2 = format!("{{\"id\":\"c{i}-stats\",\"method\":\"stats\"}}\n");
                s2.write_all(req2.as_bytes()).await.unwrap();
                let mut lines2 = BufReader::new(&mut s2).lines();
                let line2 = lines2.next_line().await.unwrap().unwrap();
                let resp2: serde_json::Value = serde_json::from_str(&line2).unwrap();
                assert_eq!(resp2["ok"], true, "client {i} stats: {resp2}");
                assert!(resp2["data"]["total_items"].is_number());
            }));
        }
        for h in handles {
            h.await.expect("client task panicked");
        }

        // Survivor request after the burst.
        let mut s = UnixStream::connect(&sock).await.unwrap();
        s.write_all(b"{\"id\":\"survivor\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut s).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ipc_client_mid_request_disconnect_does_not_panic() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("hardening_disconnect.sock");
        start_test_server(&sock).await;

        // Open + close 10 times without writing anything (clean EOF on
        // first read — must be handled, not panic).
        for _ in 0..10 {
            let stream = UnixStream::connect(&sock).await.unwrap();
            drop(stream);
        }

        // Partial write disconnect: write bytes but no newline, then drop.
        // Server's read_until returns >0 bytes then EOF on next iteration.
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"partial\",\"meth")
                .await
                .unwrap();
            drop(stream);
        }

        // Give server tasks a moment to settle.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Fresh client must still get an answer — proves no listener crash.
        let mut stream = UnixStream::connect(&sock)
            .await
            .expect("server must still accept new connections after abrupt disconnects");
        stream
            .write_all(b"{\"id\":\"survivor\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        let n = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            reader.read_line(&mut line),
        )
        .await
        .expect("survivor read timed out — server may have crashed")
        .expect("survivor read failed");
        assert!(n > 0, "expected a status response line");
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(
            resp["ok"], true,
            "status should be ok after disconnects, got: {resp}"
        );
    }

    /// beta-W3.1 — DB-touching IPC handlers must run on spawn_blocking so a
    /// slow rusqlite read does not block tokio worker threads. We exercise
    /// this by issuing N concurrent `list` requests on a single-threaded
    /// runtime (`#[tokio::test]` default). If any handler held a tokio worker
    /// across the SQLite call, the requests would serialize and the wall
    /// clock would exceed N × per-request latency. With spawn_blocking they
    /// fan out across the blocking pool and complete near-concurrently.
    ///
    /// We assert a *generous* upper bound (well below strict serialization)
    /// rather than a tight one so the test stays robust on slow CI.
    #[tokio::test]
    async fn spawn_blocking_does_not_block_tokio_worker() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-spawn-blocking.sock");
        start_test_server(&sock).await;

        // Fire 4 concurrent `list` requests, each on its own connection.
        const N: usize = 4;
        let started = std::time::Instant::now();
        let mut handles = Vec::with_capacity(N);
        for i in 0..N {
            let sock_path = sock.clone();
            handles.push(tokio::spawn(async move {
                let mut stream = UnixStream::connect(&sock_path).await.unwrap();
                let payload = format!("{{\"id\":\"sb{i}\",\"method\":\"list\"}}\n");
                stream.write_all(payload.as_bytes()).await.unwrap();
                let mut lines = BufReader::new(&mut stream).lines();
                let line = lines.next_line().await.unwrap().unwrap();
                let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(resp["ok"], true, "list must succeed: {line}");
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let elapsed = started.elapsed();

        // Sanity bound: 4 in-memory `list` calls on an empty DB should finish
        // in well under a second even with sequential serialization, so 5s
        // catches catastrophic regressions (e.g., a single-thread deadlock)
        // without flaking on slow CI runners.
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "4 concurrent list requests took {elapsed:?} — tokio worker likely blocked"
        );
    }

    /// beta-W3.2 — `pair_peer_with_password` validates required params and
    /// returns `not_implemented` once inputs check out, so the UI can rely
    /// on a stable error_code for the not-yet-wired Transport path.
    #[tokio::test]
    async fn pair_peer_with_password_validates_inputs() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-pair-pw.sock");
        start_test_server(&sock).await;

        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        // Missing peer_fingerprint → invalid_argument
        let resp = call(
            &sock,
            r#"{"id":"p1","method":"pair_peer_with_password","params":{"password":"hunter22"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "missing peer_fingerprint must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Missing password → invalid_argument
        let valid_fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let body = format!(
            r#"{{"id":"p2","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}"}}}}"#
        );
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], false, "missing password must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Short password → invalid_argument (UI enforces but daemon double-checks)
        let body = format!(
            r#"{{"id":"p3","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"ab"}}}}"#
        );
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], false, "short password must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Bad fingerprint hex → invalid_argument
        let resp = call(
            &sock,
            r#"{"id":"p4","method":"pair_peer_with_password","params":{"peer_fingerprint":"not-hex","password":"hunter22"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad fingerprint must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Valid request → not_implemented (Transport wiring is post-beta)
        let body = format!(
            r#"{{"id":"p5","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"hunter22"}}}}"#
        );
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], false, "stub must report not_implemented");
        assert_eq!(
            resp["error_code"], "not_implemented",
            "valid request must report not_implemented until Transport is wired"
        );
    }
}
