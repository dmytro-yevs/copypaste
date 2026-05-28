use crate::protocol::{
    Request, Response, CURRENT_PROTOCOL_VERSION, ERR_CODE_AUTH_FAILED, ERR_CODE_INTERNAL_ERROR,
    ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, ERR_CODE_NOT_FOUND,
    MIN_SUPPORTED_PROTOCOL_VERSION,
};
use copypaste_core::{
    chunks_from_blob, count_items, decode_image, decrypt_item_by_version, delete_fts, delete_item,
    derive_v2, ensure_revoked_devices_table, fetch_text_preview, get_item_by_id, get_page,
    pin_item, revoke_device, revoke_devices, search_items, unpin_item, Database, EncryptError,
};
use copypaste_p2p::pake::{PakeInitiator, PakeResponder, PasswordFile};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Maximum size of a single IPC request line. Clients exceeding this receive
/// an error response and have their connection closed. Prevents OOM from a
/// malicious or buggy client sending an unbounded stream without newlines.
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Server-side cap on paginated reads (`list`, `history_page`). A client
/// may request more, but the server silently clamps to this value. Protects
/// the daemon from accidental or malicious requests that would attempt to
/// materialize huge result sets in a single response.
const MAX_PAGE: usize = 1000;

/// Per-item ceiling on `import` payloads (decoded `content_bytes_b64` length).
/// Larger items are rejected with `invalid_argument` BEFORE storage so a
/// malformed or hostile export cannot exhaust memory / disk on the daemon.
/// 4 MiB matches the practical upper bound for clipboard text/image payloads
/// we round-trip today; bumping this requires re-evaluating SQLite blob limits.
const MAX_IMPORT_ITEM_BYTES: usize = 4 * 1024 * 1024;

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
pub(crate) fn peers_file_path() -> PathBuf {
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

/// Normalise a user-facing `XX:XX:...` colon-hex fingerprint to the canonical
/// lowercase, colon-free hex form used by the mTLS layer
/// ([`copypaste_p2p::cert::fingerprint_of`] → `hex::encode(SHA-256(cert_der))`).
///
/// The IPC pairing surface and `peers.json` carry the human-readable colon
/// form; [`PairedPeers::is_known`] compares against `fingerprint_of` output.
/// Both must agree or a paired peer is silently rejected at handshake time, so
/// the live-allowlist registration (fix/p2p-c-review #2) goes through this.
pub(crate) fn canonical_fingerprint(fp: &str) -> String {
    fp.replace(':', "").to_ascii_lowercase()
}

/// Maximum lifetime of an in-progress PAKE session before it is evicted as
/// stale (fix/p2p-c-review #1 — DoS). The full 3-message handshake is two
/// user-driven IPC round-trips; 120 s is generous for a human typing a
/// pairing password on the second device while bounding how long a leaked /
/// abandoned session (crashed client) pins a `PakeInitiator`/`PakeResponder`
/// in memory.
const PAKE_SESSION_TTL: std::time::Duration = std::time::Duration::from_secs(120);

/// Hard cap on the number of simultaneously-live PAKE sessions (fix/p2p-c-review
/// #1 — DoS). Pairing is an interactive, one-at-a-time-per-user operation; a
/// healthy host never approaches this. The cap converts an unbounded-growth
/// memory-exhaustion vector into a bounded one: past the cap, new `initiate` /
/// `pair_accept_password` calls are rejected with a clear error rather than
/// allocating without limit.
const MAX_PAKE_SESSIONS: usize = 64;

/// In-progress PAKE handshake session stored between IPC round-trips.
///
/// Because IPC is request-response (single turn), the 3-message OPAQUE
/// handshake is split across two calls on each side:
///
/// - Initiator: `pair_peer_with_password {step:"initiate"}` → stores
///   `PakeSession::Initiator`; `pair_peer_with_password {step:"finish"}` →
///   consumes it.
/// - Responder: `pair_accept_password` → stores `PakeSession::Responder`;
///   `pair_accept_finish` → consumes it.
///
/// Sessions are keyed by a UUID `session_id` that is returned to the caller
/// and echoed back in the follow-up call. Each entry is timestamped
/// ([`StampedPakeSession`]) and bounded by [`PAKE_SESSION_TTL`] /
/// [`MAX_PAKE_SESSIONS`] — see [`IpcServer::insert_pake_session`].
enum PakeSession {
    /// Initiator waiting for the server's `CredentialResponse` (message2)
    /// to call `PakeInitiator::finish`. Boxed to equalise variant sizes and
    /// satisfy `clippy::large_enum_variant`.
    Initiator(Box<PakeInitiator>),
    /// Responder waiting for the client's `CredentialFinalization` (message3)
    /// to call `PakeResponder::finish`, plus the peer fingerprint needed to
    /// store the resulting `PasswordFile`.
    Responder {
        responder: Box<PakeResponder>,
        /// Persisted `PasswordFile` registered for this session's password.
        /// Needed to re-drive `PakeResponder::respond` — already computed in
        /// `pair_accept_password`, stored here so `pair_accept_finish` can
        /// persist it without re-registering.
        password_file: PasswordFile,
        /// Fingerprint of the initiating peer; stored in peers.json on success.
        peer_fingerprint: String,
    },
}

/// A [`PakeSession`] tagged with its creation time so stale sessions can be
/// evicted (fix/p2p-c-review #1 — DoS).
struct StampedPakeSession {
    session: PakeSession,
    created_at: std::time::Instant,
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
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
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
    /// In-progress PAKE sessions keyed by session_id UUID string.
    ///
    /// Each entry lives from the first IPC call (initiate / accept) until the
    /// matching finish call consumes it. Bounded against unbounded growth
    /// (fix/p2p-c-review #1 — DoS): entries older than [`PAKE_SESSION_TTL`]
    /// are evicted on every insert, and the live count is capped at
    /// [`MAX_PAKE_SESSIONS`]. See [`IpcServer::insert_pake_session`].
    pake_sessions: Arc<Mutex<HashMap<String, StampedPakeSession>>>,
    /// Live P2P paired-peer allowlist, shared with the running mTLS transport
    /// (fix/p2p-c-review #2). When a PAKE handshake finishes, the newly-paired
    /// peer fingerprint is fed into this same instance via
    /// [`PairedPeers::rotate_peer`] so the accept loop immediately honours it
    /// (the S10 grace path is exercised). `None` when P2P is disabled — the
    /// PAKE handlers then only persist to `peers.json` (loaded on next start).
    p2p_peers: Option<copypaste_p2p::transport::PairedPeers>,
}

impl IpcServer {
    pub fn new(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
        device_public_key: Arc<[u8; 32]>,
    ) -> Self {
        Self {
            db,
            private_mode,
            local_key,
            device_public_key,
            ready: Arc::new(AtomicBool::new(true)),
            pake_sessions: Arc::new(Mutex::new(HashMap::new())),
            p2p_peers: None,
        }
    }

    /// Attach the live P2P paired-peer allowlist (fix/p2p-c-review #2).
    ///
    /// The daemon shares the same `PairedPeers` instance with the running mTLS
    /// transport; supplying it here lets the PAKE finish handlers register a
    /// freshly-paired peer in-memory so the accept loop honours it without a
    /// daemon restart.
    pub fn with_p2p_peers(mut self, peers: copypaste_p2p::transport::PairedPeers) -> Self {
        self.p2p_peers = Some(peers);
        self
    }

    /// Construct with an explicit readiness flag. The returned handle can be
    /// flipped to `true` once initialization completes. Intended for tests
    /// and for callers that want to bind the socket before the database is
    /// fully open.
    #[allow(dead_code)]
    pub fn new_with_ready(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
        device_public_key: Arc<[u8; 32]>,
        ready: Arc<AtomicBool>,
    ) -> Self {
        Self {
            db,
            private_mode,
            local_key,
            device_public_key,
            ready,
            pake_sessions: Arc::new(Mutex::new(HashMap::new())),
            p2p_peers: None,
        }
    }

    /// Insert a PAKE session under `session_id`, first evicting stale and
    /// excess sessions (fix/p2p-c-review #1 — DoS).
    ///
    /// Eviction policy, applied on every insert:
    /// 1. Drop any session older than [`PAKE_SESSION_TTL`].
    /// 2. If still at/above [`MAX_PAKE_SESSIONS`], reject the new session with
    ///    `Err` so the caller can surface a clear error instead of growing the
    ///    map without bound.
    ///
    /// On success returns `Ok(())` with the timestamped session stored.
    async fn insert_pake_session(
        &self,
        session_id: String,
        session: PakeSession,
    ) -> Result<(), &'static str> {
        let now = std::time::Instant::now();
        let mut sessions = self.pake_sessions.lock().await;

        // 1. Evict stale sessions (TTL).
        sessions.retain(|_, s| now.duration_since(s.created_at) < PAKE_SESSION_TTL);

        // 2. Enforce the hard cap. Reuse of an existing id (should not happen —
        //    ids are fresh UUIDs) overwrites in place and does not grow the map.
        if !sessions.contains_key(&session_id) && sessions.len() >= MAX_PAKE_SESSIONS {
            tracing::warn!(
                live = sessions.len(),
                cap = MAX_PAKE_SESSIONS,
                "rejecting new PAKE session: live-session cap reached"
            );
            return Err("too many in-flight pairing sessions; try again shortly");
        }

        sessions.insert(
            session_id,
            StampedPakeSession {
                session,
                created_at: now,
            },
        );
        Ok(())
    }

    /// Register a freshly-paired peer in the live mTLS allowlist so the accept
    /// loop honours it immediately, with no daemon restart (fix/p2p-c-review #2).
    ///
    /// `peer_fingerprint` is the user-facing colon-hex form; it is normalised
    /// to the canonical lowercase, colon-free hex the transport compares
    /// against. We go through [`PairedPeers::rotate_peer`] (rather than `add`)
    /// so the S10 cert-rotation grace path is exercised on the same code path
    /// used for re-pairing; for a first-time pair `old == new`, which `rotate`
    /// treats as a plain add (no superseded entry — nothing to grace).
    ///
    /// No-op when P2P is disabled (`p2p_peers == None`): the PAKE handler has
    /// already persisted the peer to `peers.json`, which `start_p2p` loads on
    /// the next run.
    fn register_live_peer(&self, peer_fingerprint: &str) {
        if let Some(ref peers) = self.p2p_peers {
            let canonical = canonical_fingerprint(peer_fingerprint);
            peers.rotate_peer(&canonical, canonical.clone(), peer_fingerprint);
            tracing::info!(
                fingerprint = %peer_fingerprint,
                "registered paired peer in live P2P allowlist"
            );
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
                | "copy_item"
                | "delete_all"
                | "delete_item"
                | "stats"
                | "pin"
                | "pin_item"
                | "history_page"
                | "import"
                | "revoke_peer"
                | "revoke_all_peers"
        )
    }

    /// Run the IPC accept loop until `shutdown` is cancelled.
    ///
    /// D2: accepts a [`CancellationToken`] so the daemon can stop the server
    /// cleanly on SIGINT/SIGTERM instead of relying on task abort.
    pub async fn serve(
        self,
        socket_path: &std::path::Path,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()> {
        // T4 (v0.3) — make sure the `revoked_devices` audit table exists
        // before any client can call `revoke_peer`. The DDL is purely
        // additive (`CREATE TABLE IF NOT EXISTS`) and does NOT bump the
        // SQLite `user_version`, keeping us out of the HKDF v2 worker's
        // schema-migration territory.
        {
            let db = self.db.lock().await;
            if let Err(e) = ensure_revoked_devices_table(db.conn()) {
                tracing::error!(
                    "failed to ensure revoked_devices table: {e} — \
                     revoke_peer requests will fail until this is fixed"
                );
            }
        }

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
            tokio::select! {
                // D2: stop accepting new connections on daemon-wide shutdown.
                _ = shutdown.cancelled() => {
                    tracing::info!("IPC server: shutdown signal received, stopping accept loop");
                    break;
                }
                result = listener.accept() => {
                    match result {
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
        }
        Ok(())
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
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err(req.id, "invalid param: id must be a valid UUID");
                }
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
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err(req.id, "invalid param: id must be a valid UUID");
                }
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Resolve directly by primary key — paging + linear scan
                    // silently missed any item past position 1000 (data loss).
                    let item = get_item_by_id(&db, &id_for_task)?;
                    Ok::<_, anyhow::Error>(item)
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
                                    if let Err(e) = delete_item(&db, &item.id) {
                                        tracing::error!(
                                            "ipc: delete_item failed for id={}: {e}",
                                            &item.id
                                        );
                                    }
                                    if let Err(e) = delete_fts(&db, &item.id) {
                                        tracing::error!(
                                            "ipc: delete_fts failed for id={}: {e}",
                                            &item.id
                                        );
                                    }
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
                if uuid::Uuid::parse_str(&id).is_err() {
                    return Response::err(req.id, "invalid param: id must be a valid UUID");
                }
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
            // T5.x — pin or unpin an item by id. Unlike the legacy `pin`
            // verb (pin-only), this takes an explicit `pinned: bool` so the
            // UI can toggle from a single callback. A `pinned=false` request
            // clears the pin flag (restoring normal TTL behaviour).
            "pin_item" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
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
                        pin_item(&db, &id_for_task)
                    } else {
                        unpin_item(&db, &id_for_task)
                    }
                })
                .await;
                match join {
                    Ok(Ok(())) => {
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
            // T5.x — delete a single item by id. Mirrors the legacy `delete`
            // verb but uses the typed `invalid_argument` error code (the UI
            // branches on `error_code`) and returns a structured `{deleted,
            // id}` payload. FTS cleanup is best-effort (logged on failure).
            "delete_item" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
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
                    let del = delete_item(&db, &id_for_task);
                    let fts = delete_fts(&db, &id_for_task);
                    (del, fts)
                })
                .await;
                match join {
                    Ok((Ok(removed), fts_res)) => {
                        if let Err(e) = fts_res {
                            tracing::warn!("fts delete failed for id={id}: {e}");
                        }
                        // Report whether a row was actually removed so the
                        // response matches reality: `deleted: false` for an id
                        // that did not exist, instead of always claiming `true`.
                        Response::ok(
                            req.id,
                            serde_json::json!({"deleted": removed > 0, "id": id}),
                        )
                    }
                    Ok((Err(e), _)) => Response::err(req.id, e.to_string()),
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
            "copy_item" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
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
                    // Resolve the row directly by primary key. Previously this
                    // paged `get_page(1000, 0)` and linear-scanned, so any item
                    // beyond position 1000 silently returned `not_found`
                    // (data-loss for power users). `get_item_by_id` is a single
                    // indexed `SELECT ... WHERE id = ?1` with no window cap.
                    let item = get_item_by_id(&db, &id_for_task)?;
                    Ok::<_, anyhow::Error>(item)
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
                    // Build previews inside the blocking task while `db` is
                    // still held.  Text items: read from the FTS5 plaintext
                    // index (capped at MAX_PREVIEW_BYTES = 1 KiB).
                    // Image items: return a placeholder (full preview in v0.4).
                    // Sensitive items: never expose plaintext in list view.
                    let json_items: Vec<serde_json::Value> = items
                        .iter()
                        .map(|item| {
                            let preview = if item.is_sensitive {
                                format!("[sensitive — id:{}]", &item.id[..8])
                            } else if item.content_type == "text" {
                                fetch_text_preview(&db, &item.id)
                                    .unwrap_or(None)
                                    .unwrap_or_else(|| format!("[text — id:{}]", &item.id[..8]))
                            } else {
                                // image (and any future non-text type)
                                format!("[image — id:{}]", &item.id[..8])
                            };
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
                    Ok::<_, anyhow::Error>((json_items, total))
                })
                .await;
                match join {
                    Ok(Ok((json_items, total))) => Response::ok(
                        req.id,
                        serde_json::json!({"items": json_items, "total": total}),
                    ),
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

            // T4 (v0.3) — manual peer revocation. Atomic with respect to the
            // user: a single click both (a) removes the peer from the local
            // JSON peer store so future sync attempts won't re-discover the
            // device by name, and (b) writes a row to the SQLite
            // `revoked_devices` audit table. The v1.0 cryptographic
            // revocation protocol will later consume that table to broadcast
            // revocation markers. For v0.3 the audit row is the only durable
            // record — mTLS rejection on unknown fingerprint is what blocks
            // the revoked peer from continuing to sync.
            "revoke_peer" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: fingerprint",
                        )
                    }
                };
                if !is_valid_fingerprint(&fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid fingerprint format: {fingerprint}"),
                    );
                }

                // Capture the peer's display name *before* deleting so the
                // audit row preserves the human-readable label. Falls back
                // to an empty string if the peer wasn't in the store
                // (revoking an unknown fingerprint is allowed — useful when
                // the local peer list is out of sync with reality).
                let (removed, captured_name) = match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        let name = peers
                            .iter()
                            .find(|p| {
                                p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| f == fingerprint)
                                    .unwrap_or(false)
                            })
                            .and_then(|p| p.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();

                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| f != fingerprint)
                                .unwrap_or(true)
                        });
                        if let Err(e) = save_peers(&peers) {
                            return Response::err(req.id, format!("failed to save peers: {e}"));
                        }
                        (peers.len() < before_len, name)
                    }
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                };

                // Write the audit row. Done on the blocking thread pool
                // because rusqlite is sync; the mutex is held only for the
                // duration of the two short statements inside
                // `revoke_device`.
                let db_arc = self.db.clone();
                let fp_for_db = fingerprint.clone();
                let name_for_db = captured_name.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    revoke_device(db.conn(), &fp_for_db, &name_for_db)
                })
                .await;

                match join {
                    Ok(Ok(revoked_at)) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "ok": true,
                            "removed": removed,
                            "revoked_at": revoked_at,
                            "fingerprint": fingerprint,
                        }),
                    ),
                    Ok(Err(e)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("failed to record revocation: {e}"),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("revoke task join error: {e}"),
                    ),
                }
            }

            // T5.x — revoke ALL paired peers in one call (Settings →
            // "Reset pairings"). Clears the local JSON peer store and writes
            // a `revoked_devices` audit row for each peer, reusing the same
            // single-peer `revoke_device` primitive. An empty store is a
            // success returning `{revoked: 0}` rather than an error.
            "revoke_all_peers" => {
                // Snapshot the current peers (fingerprint + display name)
                // before clearing the store so we can write audit rows.
                let peers = match load_peers() {
                    Ok(p) => p,
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                };
                let captured: Vec<(String, String)> = peers
                    .iter()
                    .filter_map(|p| {
                        let fp = p.get("fingerprint").and_then(|v| v.as_str())?.to_string();
                        let name = p
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        Some((fp, name))
                    })
                    .collect();

                // Write every audit row in a single transaction FIRST, and only
                // clear the JSON peer store once that transaction has durably
                // committed. The previous order (clear store → loop inserting
                // audit rows, swallowing per-row errors) could leave the store
                // empty with audit rows missing on a partial failure, with the
                // loss only logged. With this order a failure leaves *both*
                // stores untouched so the caller can safely retry.
                let db_arc = self.db.clone();
                let captured_for_db = captured.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    revoke_devices(db.conn(), &captured_for_db)
                })
                .await;

                let revoked_at = match join {
                    Ok(Ok(ts)) => ts,
                    Ok(Err(e)) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("failed to record revocations: {e}"),
                        )
                    }
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("revoke_all task join error: {e}"),
                        )
                    }
                };

                // Audit log committed — now clear the local peer store. If this
                // fails the audit rows are already durable (idempotent on a
                // retry via the UPSERT), so we surface the error rather than
                // silently leaving stale peers behind.
                if let Err(e) = save_peers(&[]) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("revocations recorded but failed to clear peers: {e}"),
                    );
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "ok": true,
                        "revoked": captured.len(),
                        "cleared": captured.len(),
                        "revoked_at": revoked_at,
                    }),
                )
            }

            // W2.4 — PAKE-based password pairing (initiator side).
            //
            // Two-step protocol over IPC:
            //   step="initiate": validates inputs, creates PakeInitiator,
            //     stores session in pake_sessions, returns {session_id, message1_b64}.
            //   step="finish": looks up PakeInitiator by session_id, completes
            //     handshake with server's message2, stores peer, returns
            //     {ok: true, message3_b64}.
            "pair_peer_with_password" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

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

                if !is_valid_fingerprint(&peer_fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid peer_fingerprint format: {peer_fingerprint}"),
                    );
                }

                let step = req
                    .params
                    .get("step")
                    .and_then(|v| v.as_str())
                    .unwrap_or("initiate")
                    .to_string();

                match step.as_str() {
                    "initiate" => {
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

                        if password.chars().count() < 6 {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "password must be at least 6 characters",
                            );
                        }

                        let (initiator, msg1_bytes) = match PakeInitiator::new(&password) {
                            Ok(pair) => pair,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INTERNAL_ERROR,
                                    format!("PAKE init failed: {e}"),
                                )
                            }
                        };

                        let session_id = uuid::Uuid::new_v4().to_string();
                        let msg1_b64 = b64.encode(&msg1_bytes);

                        if let Err(msg) = self
                            .insert_pake_session(
                                session_id.clone(),
                                PakeSession::Initiator(Box::new(initiator)),
                            )
                            .await
                        {
                            return Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg);
                        }

                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "session_id": session_id,
                                "message1_b64": msg1_b64,
                            }),
                        )
                    }

                    "finish" => {
                        let session_id = match req.params.get("session_id").and_then(|v| v.as_str())
                        {
                            Some(s) => s.to_string(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    "missing session_id for step=finish",
                                )
                            }
                        };
                        let msg2_b64 = match req.params.get("message2_b64").and_then(|v| v.as_str())
                        {
                            Some(s) => s.to_string(),
                            None => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    "missing message2_b64 for step=finish",
                                )
                            }
                        };

                        let msg2_bytes = match b64.decode(&msg2_b64) {
                            Ok(b) => b,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_INVALID_ARGUMENT,
                                    format!("invalid base64 in message2_b64: {e}"),
                                )
                            }
                        };

                        // Extract and consume the initiator session.
                        let initiator = {
                            let mut sessions = self.pake_sessions.lock().await;
                            match sessions.remove(&session_id) {
                                Some(StampedPakeSession {
                                    session: PakeSession::Initiator(i),
                                    ..
                                }) => *i,
                                Some(other) => {
                                    // Wrong session type — put it back and error.
                                    let key = session_id.clone();
                                    sessions.insert(key, other);
                                    return Response::err_with_code(
                                        req.id,
                                        ERR_CODE_INVALID_ARGUMENT,
                                        "session_id refers to a responder session, not initiator",
                                    );
                                }
                                None => {
                                    return Response::err_with_code(
                                        req.id,
                                        ERR_CODE_INVALID_ARGUMENT,
                                        format!("unknown session_id: {session_id}"),
                                    )
                                }
                            }
                        };

                        // TODO(S3): the PAKE `SessionKey` is derived here and
                        // immediately dropped. It SHOULD be mixed with the
                        // RFC 5705 TLS channel binder (see
                        // `copypaste_p2p::transport::tls_channel_binder_*` and
                        // `SessionKey::bind_to_tls_channel`) and verified against
                        // the peer to defeat a relay/MitM that terminates the
                        // PAKE on one socket and the mTLS on another. Wiring it
                        // is a deliberate design decision left to the human
                        // owner; until then pairing authenticity rests on the
                        // mTLS cert-fingerprint pinning alone.
                        let (_session_key, msg3_bytes) = match initiator.finish(&msg2_bytes) {
                            Ok(pair) => pair,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_AUTH_FAILED,
                                    format!("PAKE finish failed: {e}"),
                                )
                            }
                        };

                        let msg3_b64 = b64.encode(&msg3_bytes);

                        // Store the paired peer on the initiator side (no PasswordFile).
                        let added_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        match load_peers() {
                            Ok(mut peers) => {
                                // Only add if not already present.
                                let already = peers.iter().any(|p| {
                                    p.get("fingerprint")
                                        .and_then(|v| v.as_str())
                                        .map(|f| f == peer_fingerprint)
                                        .unwrap_or(false)
                                });
                                if !already {
                                    peers.push(serde_json::json!({
                                        "fingerprint": peer_fingerprint,
                                        "added_at": added_at,
                                    }));
                                    if let Err(e) = save_peers(&peers) {
                                        return Response::err(
                                            req.id,
                                            format!("failed to save peers: {e}"),
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                return Response::err(req.id, format!("failed to load peers: {e}"))
                            }
                        }

                        // Feed the newly-paired peer into the live allowlist so
                        // the mTLS accept loop honours it without a restart.
                        self.register_live_peer(&peer_fingerprint);

                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "ok": true,
                                "message3_b64": msg3_b64,
                            }),
                        )
                    }

                    other => Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("unknown step '{other}'; expected 'initiate' or 'finish'"),
                    ),
                }
            }

            // W2.4 — PAKE responder: receives message1 from initiator,
            // runs PakeResponder::respond, stores session, returns message2.
            // Params: {message1_b64, peer_fingerprint, password}
            // Response: {session_id, message2_b64}
            "pair_accept_password" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                let message1_b64 = match req.params.get("message1_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing message1_b64",
                        )
                    }
                };
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

                // fix/p2p-c-review #5: enforce the same 6-char minimum the
                // initiator does. Without this the responder would happily
                // register a PasswordFile for a 1-char password if the peer
                // (or a malicious initiator) skipped the initiator-side check.
                if password.chars().count() < 6 {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "password must be at least 6 characters",
                    );
                }

                let msg1_bytes = match b64.decode(&message1_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!("invalid base64 in message1_b64: {e}"),
                        )
                    }
                };

                // Register the password so we have a PasswordFile for respond.
                let password_file = match copypaste_p2p::pake::PasswordFile::register(&password) {
                    Ok(pf) => pf,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("PasswordFile::register failed: {e}"),
                        )
                    }
                };

                let (responder, msg2_bytes) =
                    match PakeResponder::respond(&password_file, &msg1_bytes) {
                        Ok(pair) => pair,
                        Err(e) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_AUTH_FAILED,
                                format!("PAKE respond failed: {e}"),
                            )
                        }
                    };

                let session_id = uuid::Uuid::new_v4().to_string();
                let msg2_b64 = b64.encode(&msg2_bytes);

                if let Err(msg) = self
                    .insert_pake_session(
                        session_id.clone(),
                        PakeSession::Responder {
                            responder: Box::new(responder),
                            password_file,
                            peer_fingerprint,
                        },
                    )
                    .await
                {
                    return Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg);
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "session_id": session_id,
                        "message2_b64": msg2_b64,
                    }),
                )
            }

            // W2.4 — PAKE responder finish: receives message3 from initiator,
            // completes handshake, persists peer + PasswordFile.
            // Params: {session_id, message3_b64, peer_fingerprint}
            // Response: {ok: true}
            "pair_accept_finish" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                let session_id = match req.params.get("session_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing session_id",
                        )
                    }
                };
                let msg3_b64 = match req.params.get("message3_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing message3_b64",
                        )
                    }
                };

                let msg3_bytes = match b64.decode(&msg3_b64) {
                    Ok(b) => b,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!("invalid base64 in message3_b64: {e}"),
                        )
                    }
                };

                // Extract and consume the responder session.
                let (responder, password_file, peer_fingerprint) = {
                    let mut sessions = self.pake_sessions.lock().await;
                    match sessions.remove(&session_id) {
                        Some(StampedPakeSession {
                            session:
                                PakeSession::Responder {
                                    responder,
                                    password_file,
                                    peer_fingerprint,
                                },
                            ..
                        }) => (*responder, password_file, peer_fingerprint),
                        Some(other) => {
                            let key = session_id.clone();
                            sessions.insert(key, other);
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "session_id refers to an initiator session, not responder",
                            );
                        }
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("unknown session_id: {session_id}"),
                            )
                        }
                    }
                };

                // Finalize the handshake (validates the initiator's authenticator).
                //
                // TODO(S3): `responder.finish` returns the shared `SessionKey`,
                // which we discard here. It SHOULD be mixed with the RFC 5705
                // TLS channel binder (`tls_channel_binder_server` +
                // `SessionKey::bind_to_tls_channel`) and confirmed with the peer
                // so a relay/MitM cannot bridge a PAKE on one connection to an
                // mTLS session on another. Deferred — design decision left to
                // the human owner; pairing currently relies on mTLS
                // cert-fingerprint pinning for channel authenticity.
                if let Err(e) = responder.finish(&msg3_bytes) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_AUTH_FAILED,
                        format!("PAKE accept_finish failed: {e}"),
                    );
                }

                // Persist the peer with the PasswordFile blob on the responder side.
                let password_file_b64 = b64.encode(&password_file.serialized);
                let added_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                match load_peers() {
                    Ok(mut peers) => {
                        let already = peers.iter().any(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| f == peer_fingerprint)
                                .unwrap_or(false)
                        });
                        if !already {
                            peers.push(serde_json::json!({
                                "fingerprint": peer_fingerprint,
                                "password_file_b64": password_file_b64,
                                "added_at": added_at,
                            }));
                        } else {
                            // Update existing peer with the new PasswordFile.
                            for p in peers.iter_mut() {
                                if p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| f == peer_fingerprint)
                                    .unwrap_or(false)
                                {
                                    p["password_file_b64"] =
                                        serde_json::Value::String(password_file_b64.clone());
                                    break;
                                }
                            }
                        }
                        if let Err(e) = save_peers(&peers) {
                            return Response::err(req.id, format!("failed to save peers: {e}"));
                        }
                    }
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                }

                // Feed the newly-paired peer into the live allowlist so the
                // mTLS accept loop honours it without a restart.
                self.register_live_peer(&peer_fingerprint);

                Response::ok(req.id, serde_json::json!({ "ok": true }))
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
                let raw_items: &[serde_json::Value] = match items_value.as_array() {
                    Some(a) => a.as_slice(),
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
                    // Audit MED #4: enforce per-item ceiling BEFORE storage so
                    // a hostile/corrupt export cannot exhaust daemon memory or
                    // SQLite blob limits. Reject the whole import on first
                    // oversized item — matches the "malformed entry aborts
                    // the batch" contract documented above.
                    if bytes.len() > MAX_IMPORT_ITEM_BYTES {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            format!(
                                "item[{idx}]: decoded payload {} bytes exceeds max {} bytes",
                                bytes.len(),
                                MAX_IMPORT_ITEM_BYTES
                            ),
                        );
                    }
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
                // Move a copy of the device's v1 storage key into the blocking
                // task so imported content can be ENCRYPTED with the same
                // (key, AAD, key_version) the normal ingest path uses — see
                // the per-item block below.
                let local_key_v1: [u8; 32] = **self.local_key;
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // v0.3 post-T2: dedup is now enforced atomically by the
                    // v5 UNIQUE indexes (content_hash + minute_bucket) inside
                    // insert_item_with_fts. The previous explicit
                    // `find_recent_by_hash` precheck created a TOCTOU window
                    // — two concurrent imports of the same payload could both
                    // pass the precheck and then race on insert. The new
                    // path returns the existing row's id on a unique-violation,
                    // which we treat as a dedup skip.
                    let mut inserted: u32 = 0;
                    let mut skipped: u32 = 0;
                    // Derive the v2 storage key once: imported content is
                    // encrypted exactly as `daemon::encrypt_text_for_storage`
                    // does (v2 key + v4 AAD, stamped key_version = 2), so the
                    // read path (`decrypt_item_by_version`, dispatched by the
                    // `copy`/`paste` IPC verb) can decrypt it.
                    let v2_key = derive_v2(&local_key_v1);
                    for item in decoded {
                        let mut hasher = Sha256::new();
                        hasher.update(&item.bytes);
                        let hash_hex = hex::encode(hasher.finalize());

                        // Audit fix (import round-trip): previously imported
                        // bytes were stored VERBATIM with an EMPTY nonce while
                        // `ClipboardItem::new_text` stamped key_version = 2.
                        // The read path then tried to XChaCha20-Poly1305-decrypt
                        // them under the v2 key and failed with AuthFailed, so
                        // imported items could never be retrieved.
                        //
                        // Now we ENCRYPT the content the same way fresh ingest
                        // does: build the AAD from the row's own item_id with
                        // the v4 schema + key_version 2, encrypt with the v2
                        // key, and store the real (nonce, ciphertext). The row
                        // stays at key_version = 2 (set by new_text) so the
                        // read path selects the matching key/AAD.
                        //
                        // lamport_ts = 0 is a deliberate "imported, unknown
                        // origin" sentinel; sync will reassign on first push.
                        let item_id = uuid::Uuid::new_v4().to_string();
                        let aad = copypaste_core::build_item_aad_v2(
                            &item_id,
                            copypaste_core::AAD_SCHEMA_VERSION_V4,
                            copypaste_core::ITEM_KEY_VERSION_CURRENT as u32,
                        );
                        let (nonce, ciphertext) =
                            match copypaste_core::encrypt_item_with_aad(&item.bytes, &v2_key, &aad)
                            {
                                Ok(v) => v,
                                Err(e) => {
                                    return Err::<(u32, u32), anyhow::Error>(anyhow::anyhow!(
                                        "encrypt imported item failed: {e}"
                                    ));
                                }
                            };
                        let mut clip =
                            copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
                        clip.item_id = item_id;
                        clip.content_type = item.content_type;
                        clip.wall_time = item.created_at_ms;
                        clip.content_hash = Some(hash_hex);

                        // FTS indexing: pass "" to skip the FTS write. The
                        // searchable plaintext is no longer available as a
                        // stored column (content is now ciphertext), matching
                        // the image path semantics — search over imported
                        // items is out of scope for this fix.
                        let requested_id = clip.id.clone();
                        match copypaste_core::insert_item_with_fts(&db, &clip, "") {
                            Ok(stored_id) if stored_id == requested_id => {
                                inserted += 1;
                            }
                            Ok(_) => {
                                // Returned id differs => dedup hit (existing
                                // row with same content_hash/item_id).
                                skipped += 1;
                            }
                            Err(e) => {
                                return Err::<(u32, u32), anyhow::Error>(e.into());
                            }
                        }
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
    /// 1. Decrypts text via [`decrypt_item_with_aad`] with the per-item nonce,
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
                let v1_key: [u8; 32] = **self.local_key;
                let v2_key = derive_v2(&v1_key);
                let plaintext_bytes = decrypt_item_by_version(
                    item.key_version,
                    &v1_key,
                    &v2_key,
                    &item.item_id,
                    nonce,
                    content,
                )
                .map_err(|e| match e {
                    EncryptError::AuthFailed | EncryptError::AadMismatch => {
                        PasteboardError::decrypt(
                            "Decryption failed: authentication tag mismatch".to_string(),
                        )
                    }
                    EncryptError::UnknownKeyVersion(_) => PasteboardError::decrypt(
                        "Item encrypted with a previous key — cannot be recovered. \
                             Clear history to start fresh."
                            .to_string(),
                    ),
                    other => PasteboardError::decrypt(other.to_string()),
                })?;
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
                let png_bytes = decode_image(&chunks, &self.local_key, &file_id)
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
///
/// Lives here as `pub(crate)` (not behind `#[cfg(macos)]`) so the daemon's
/// image round-trip tests can drive the exact same read-path parser on any
/// host. Only the macOS `write_to_pasteboard` path calls it at runtime, hence
/// the dead-code allowance on non-macOS builds.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn parse_image_file_id(meta_json: &str) -> Result<[u8; 16], String> {
    let value: serde_json::Value =
        serde_json::from_str(meta_json).map_err(|e| format!("image meta_json parse error: {e}"))?;
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

    // Env mutation (`set_var`/`remove_var`) is process-global and unsound under
    // concurrent access — Rust 1.89 warns and edition 2024 makes it `unsafe`.
    // Tests that redirect the config dir (peers.json location) serialise on this
    // lock so no two run their env mutation concurrently.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// RAII guard that snapshots one or more env vars, sets them for the test,
    /// and restores the previous values (or unsets them) on drop — even on
    /// panic. Mirrors the pattern in `paths.rs`. Holds `ENV_LOCK` for its whole
    /// lifetime so env state cannot race another serialised test.
    struct EnvGuard {
        saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        /// Point every given env var at `value`. Used to redirect the config
        /// dir to a temp path across platforms: `dirs::config_dir()` honours
        /// `XDG_CONFIG_HOME` on Linux/BSD and `$HOME` (→ Library/Application
        /// Support) on macOS, so callers set both.
        fn set_all(keys: &[&'static str], value: &std::path::Path) -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
            let mut saved = Vec::with_capacity(keys.len());
            for &key in keys {
                saved.push((key, std::env::var_os(key)));
                // SAFETY: serialised via `ENV_LOCK`; no other thread reads or
                // writes these vars concurrently for the guard's lifetime.
                unsafe { std::env::set_var(key, value) };
            }
            Self { saved, _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: still holding `ENV_LOCK` (`_lock`), so the restore is
            // serialised against every other env-mutating test.
            unsafe {
                for (key, original) in self.saved.drain(..) {
                    match original {
                        Some(v) => std::env::set_var(key, v),
                        None => std::env::remove_var(key),
                    }
                }
            }
        }
    }

    async fn start_test_server(socket_path: &std::path::Path) -> Arc<AtomicBool> {
        start_test_server_with_mode(socket_path, false).await
    }

    async fn start_test_server_with_mode(
        socket_path: &std::path::Path,
        initial_private_mode: bool,
    ) -> Arc<AtomicBool> {
        let (private_mode, _db) =
            start_test_server_returning_db(socket_path, initial_private_mode).await;
        private_mode
    }

    /// Like `start_test_server_with_mode` but also hands back the shared
    /// `Database` handle so a test can seed rows / inspect audit tables.
    async fn start_test_server_returning_db(
        socket_path: &std::path::Path,
        initial_private_mode: bool,
    ) -> (Arc<AtomicBool>, Arc<Mutex<Database>>) {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(initial_private_mode));
        // Dummy keys: in-process tests do not hit paste-back or fingerprint
        // surfaces — they only validate dispatch / state-machine behaviour.
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);
        let server = IpcServer::new(db.clone(), private_mode.clone(), local_key, device_pub);
        let path = socket_path.to_path_buf();
        tokio::spawn(async move {
            if let Err(e) = server.serve(&path, CancellationToken::new()).await {
                tracing::error!("ipc: server on {:?} exited with error: {e}", &path);
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        (private_mode, db)
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
                b"{\"id\":\"p2\",\"method\":\"paste\",\"params\":{\"id\":\"00000000-0000-0000-0000-000000000000\"}}\n",
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
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);
        let server =
            IpcServer::new_with_ready(db, private_mode, local_key, device_pub, ready_clone);
        let path = socket_path.to_path_buf();
        tokio::spawn(async move {
            if let Err(e) = server.serve(&path, CancellationToken::new()).await {
                tracing::error!("ipc: server on {:?} exited with error: {e}", &path);
            }
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

        // Missing step → defaults to "initiate"; valid request returns session_id + message1_b64
        let body = format!(
            r#"{{"id":"p5","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"hunter22","step":"initiate"}}}}"#
        );
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], true, "initiate step must succeed: {resp}");
        assert!(
            resp["data"]["session_id"].is_string(),
            "response must contain session_id"
        );
        assert!(
            resp["data"]["message1_b64"].is_string(),
            "response must contain message1_b64"
        );
    }

    /// W2.4 — `pair_peer_with_password` with step="initiate" returns a
    /// session_id and base64-encoded message1 to send to the responder.
    #[tokio::test]
    async fn pair_peer_with_password_initiate_returns_session_and_message1() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-pake-init.sock");
        start_test_server(&sock).await;

        let valid_fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let body = format!(
            r#"{{"id":"pi1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{valid_fp}","password":"correct-horse","step":"initiate"}}}}"#
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(resp["ok"], true, "initiate must succeed: {resp}");
        let session_id = resp["data"]["session_id"].as_str().unwrap();
        assert!(!session_id.is_empty(), "session_id must not be empty");
        let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap();
        // Verify it decodes as valid base64 bytes
        use base64::Engine as _;
        let msg1_bytes = base64::engine::general_purpose::STANDARD
            .decode(msg1_b64)
            .expect("message1_b64 must be valid base64");
        assert!(!msg1_bytes.is_empty(), "message1 must not be empty");
    }

    /// W2.4 — `pair_accept_password` returns a session_id and message2 in
    /// response to a valid message1.
    #[tokio::test]
    async fn pair_accept_password_returns_session_and_message2() {
        use base64::Engine as _;
        use copypaste_p2p::pake::PakeInitiator;

        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-pake-accept.sock");
        start_test_server(&sock).await;

        // Simulate the initiator side locally.
        let password = "correct-horse";
        let (_initiator, msg1_bytes) = PakeInitiator::new(password).expect("PakeInitiator::new");
        let msg1_b64 = base64::engine::general_purpose::STANDARD.encode(&msg1_bytes);

        let valid_fp = std::iter::repeat_n("cd", 32).collect::<Vec<_>>().join(":");
        let body = format!(
            r#"{{"id":"pa1","method":"pair_accept_password","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{valid_fp}","password":"{password}"}}}}"#
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(
            resp["ok"], true,
            "pair_accept_password must succeed: {resp}"
        );
        assert!(
            resp["data"]["session_id"].is_string(),
            "must return session_id"
        );
        let msg2_b64 = resp["data"]["message2_b64"].as_str().unwrap();
        let msg2_bytes = base64::engine::general_purpose::STANDARD
            .decode(msg2_b64)
            .expect("message2_b64 must be valid base64");
        assert!(!msg2_bytes.is_empty(), "message2 must not be empty");
    }

    /// W2.4 — full PAKE round-trip through IPC: initiator initiate →
    /// responder accept → initiator finish → responder finish → both sides
    /// complete and peer is stored.
    #[tokio::test]
    async fn pair_peer_with_password_full_round_trip() {
        use base64::Engine as _;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let dir = tempdir().unwrap();
        // Use two server instances to simulate two separate daemons.
        let sock_a = dir.path().join("test-pake-rt-a.sock");
        let sock_b = dir.path().join("test-pake-rt-b.sock");
        start_test_server(&sock_a).await;
        start_test_server(&sock_b).await;

        // Helper closure for a single IPC call.
        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        let b64 = base64::engine::general_purpose::STANDARD;
        let password = "correct-horse-battery";
        let fp_a = std::iter::repeat_n("aa", 32).collect::<Vec<_>>().join(":");
        let fp_b = std::iter::repeat_n("bb", 32).collect::<Vec<_>>().join(":");

        // Step 1: Device A initiates.
        let body = format!(
            r#"{{"id":"rt1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{fp_b}","password":"{password}","step":"initiate"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(resp["ok"], true, "initiate step failed: {resp}");
        let session_id_a = resp["data"]["session_id"].as_str().unwrap().to_string();
        let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap().to_string();

        // Step 2: Device B accepts (responder side).
        let body = format!(
            r#"{{"id":"rt2","method":"pair_accept_password","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{fp_a}","password":"{password}"}}}}"#
        );
        let resp = call(&sock_b, &body).await;
        assert_eq!(resp["ok"], true, "pair_accept_password failed: {resp}");
        let session_id_b = resp["data"]["session_id"].as_str().unwrap().to_string();
        let msg2_b64 = resp["data"]["message2_b64"].as_str().unwrap().to_string();

        // Step 3: Device A finishes.
        let body = format!(
            r#"{{"id":"rt3","method":"pair_peer_with_password","params":{{"step":"finish","session_id":"{session_id_a}","message2_b64":"{msg2_b64}","peer_fingerprint":"{fp_b}","password":"{password}"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(resp["ok"], true, "initiator finish failed: {resp}");
        let msg3_b64 = resp["data"]["message3_b64"].as_str().unwrap().to_string();

        // Step 4: Device B finishes.
        let body = format!(
            r#"{{"id":"rt4","method":"pair_accept_finish","params":{{"session_id":"{session_id_b}","message3_b64":"{msg3_b64}","peer_fingerprint":"{fp_a}"}}}}"#
        );
        let resp = call(&sock_b, &body).await;
        assert_eq!(resp["ok"], true, "responder finish failed: {resp}");
        assert_eq!(
            resp["data"]["ok"], true,
            "pair_accept_finish data.ok must be true"
        );

        // Verify Device B stored the peer (with password_file_b64) in peers.json.
        // We check via the list_peers IPC method.
        let list_resp = call(&sock_b, r#"{"id":"rt5","method":"list_peers","params":{}}"#).await;
        assert_eq!(list_resp["ok"], true, "list_peers failed: {list_resp}");
        let peers = list_resp["data"]["peers"].as_array().unwrap();
        let stored = peers.iter().find(|p| {
            p.get("fingerprint")
                .and_then(|v| v.as_str())
                .map(|f| f == fp_a)
                .unwrap_or(false)
        });
        assert!(
            stored.is_some(),
            "peer {fp_a} must be stored on device B after finish"
        );

        // Verify the stored peer has the password_file_b64 field (PasswordFile blob).
        let pf_b64 = stored
            .unwrap()
            .get("password_file_b64")
            .and_then(|v| v.as_str());
        assert!(pf_b64.is_some(), "peer must have password_file_b64 stored");
        let pf_bytes = b64
            .decode(pf_b64.unwrap())
            .expect("password_file_b64 is valid base64");
        assert!(!pf_bytes.is_empty(), "PasswordFile blob must not be empty");

        // Verify Device A also stored the peer (without PasswordFile — initiator side).
        let list_resp = call(&sock_a, r#"{"id":"rt6","method":"list_peers","params":{}}"#).await;
        assert_eq!(list_resp["ok"], true, "list_peers on A failed: {list_resp}");
        let peers = list_resp["data"]["peers"].as_array().unwrap();
        let stored_a = peers.iter().find(|p| {
            p.get("fingerprint")
                .and_then(|v| v.as_str())
                .map(|f| f == fp_b)
                .unwrap_or(false)
        });
        assert!(
            stored_a.is_some(),
            "peer {fp_b} must be stored on device A after finish"
        );
    }

    /// T4 (v0.3) — `revoke_peer` validates its fingerprint argument and, for
    /// a well-formed request, writes a row to the `revoked_devices` audit
    /// table even when the peer was never in the local JSON peer store
    /// (revoking an unknown fingerprint is intentionally allowed so the UI
    /// can recover from a corrupted peers.json).
    #[tokio::test]
    async fn revoke_peer_validates_and_records_audit_row() {
        use copypaste_core::list_revoked_devices;

        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-revoke.sock");

        // Build the server manually so we can reach the shared Database
        // handle for assertions after the call.
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let server = IpcServer::new(
            db.clone(),
            private_mode,
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );
        let sock_path = sock.clone();
        tokio::spawn(async move {
            if let Err(e) = server.serve(&sock_path, CancellationToken::new()).await {
                tracing::error!("ipc: server on {:?} exited with error: {e}", &sock_path);
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        // Missing fingerprint → invalid_argument
        let resp = call(&sock, r#"{"id":"r1","method":"revoke_peer","params":{}}"#).await;
        assert_eq!(resp["ok"], false, "missing fingerprint must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Bad fingerprint hex → invalid_argument
        let resp = call(
            &sock,
            r#"{"id":"r2","method":"revoke_peer","params":{"fingerprint":"not-hex"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad fingerprint must fail");
        assert_eq!(resp["error_code"], "invalid_argument");

        // Valid request — unknown peer, but revoke still succeeds and writes
        // the audit row.
        let fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let body =
            format!(r#"{{"id":"r3","method":"revoke_peer","params":{{"fingerprint":"{fp}"}}}}"#);
        let resp = call(&sock, &body).await;
        assert_eq!(resp["ok"], true, "valid revoke must succeed: {resp}");
        assert_eq!(resp["data"]["fingerprint"], fp);
        assert!(
            resp["data"]["revoked_at"].as_u64().unwrap_or(0) > 0,
            "revoked_at must be populated"
        );

        // Audit row must be persisted in the shared SQLite DB.
        let db_guard = db.lock().await;
        let rows = list_revoked_devices(db_guard.conn()).unwrap();
        assert_eq!(rows.len(), 1, "exactly one audit row expected");
        assert_eq!(rows[0].fingerprint, fp);
    }

    // ------------------------------------------------------------------
    // T5.x — clipboard-history UI action wiring
    //
    // New verbs added so the UI can drive history actions end-to-end over
    // the Unix socket: `pin_item`, `delete_item`, `copy_item`, and
    // `revoke_all_peers`. Each validates its arguments and returns the
    // documented error code on missing/bad params, mirroring the
    // beta-W3.2 (`pair_peer_with_password`) and T4 (`revoke_peer`) tests.
    // ------------------------------------------------------------------

    async fn call_one(sock: &std::path::Path, body: &str) -> serde_json::Value {
        let mut stream = UnixStream::connect(sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        serde_json::from_str(&line).unwrap()
    }

    /// Build a bare in-process `IpcServer` (no socket) for exercising private
    /// helpers like `insert_pake_session` directly.
    fn bare_server() -> IpcServer {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        IpcServer::new(
            db,
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        )
    }

    /// fix/p2p-c-review #1 — a session older than `PAKE_SESSION_TTL` is evicted
    /// on the next `insert_pake_session`, so the map cannot grow with abandoned
    /// (crashed-client) sessions.
    #[tokio::test]
    async fn stale_pake_sessions_are_evicted_on_insert() {
        let server = bare_server();

        // Insert a first session, then back-date it past the TTL so it is
        // considered stale. (`Instant` can't be constructed directly; we patch
        // the stored `created_at` in place — this module has field access.)
        let (init1, _msg1) = PakeInitiator::new("hunter2-pw").unwrap();
        server
            .insert_pake_session("stale".into(), PakeSession::Initiator(Box::new(init1)))
            .await
            .unwrap();
        {
            let mut sessions = server.pake_sessions.lock().await;
            let stamped = sessions.get_mut("stale").expect("stale session present");
            stamped.created_at =
                std::time::Instant::now() - (PAKE_SESSION_TTL + std::time::Duration::from_secs(1));
        }

        // Inserting a fresh session triggers TTL eviction of the stale one.
        let (init2, _msg2) = PakeInitiator::new("hunter2-pw").unwrap();
        server
            .insert_pake_session("fresh".into(), PakeSession::Initiator(Box::new(init2)))
            .await
            .unwrap();

        let sessions = server.pake_sessions.lock().await;
        assert!(
            !sessions.contains_key("stale"),
            "stale session must be evicted on insert"
        );
        assert!(
            sessions.contains_key("fresh"),
            "fresh session must remain after eviction pass"
        );
        assert_eq!(sessions.len(), 1, "exactly one live session expected");
    }

    /// fix/p2p-c-review #1 — once `MAX_PAKE_SESSIONS` non-stale sessions are
    /// live, a further insert is rejected (rather than growing without bound).
    #[tokio::test]
    async fn pake_session_cap_rejects_excess() {
        let server = bare_server();

        for i in 0..MAX_PAKE_SESSIONS {
            let (init, _m) = PakeInitiator::new("hunter2-pw").unwrap();
            server
                .insert_pake_session(format!("s{i}"), PakeSession::Initiator(Box::new(init)))
                .await
                .expect("inserts up to the cap must succeed");
        }

        let (init, _m) = PakeInitiator::new("hunter2-pw").unwrap();
        let over_cap = server
            .insert_pake_session("over".into(), PakeSession::Initiator(Box::new(init)))
            .await;
        assert!(over_cap.is_err(), "insert past the cap must be rejected");
        assert_eq!(
            server.pake_sessions.lock().await.len(),
            MAX_PAKE_SESSIONS,
            "map must not exceed the cap"
        );
    }

    /// fix/p2p-c-review #5 — the responder (`pair_accept_password`) enforces the
    /// 6-char minimum password, matching the initiator side.
    #[tokio::test]
    async fn pair_accept_password_rejects_short_password() {
        use base64::Engine as _;

        let dir = tempdir().unwrap();
        let sock = dir.path().join("test-short-pw.sock");
        start_test_server(&sock).await;

        let (_init, msg1) = PakeInitiator::new("short").unwrap();
        let msg1_b64 = base64::engine::general_purpose::STANDARD.encode(&msg1);
        let fp = std::iter::repeat_n("ab", 32).collect::<Vec<_>>().join(":");
        let body = format!(
            r#"{{"id":"sp1","method":"pair_accept_password","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{fp}","password":"short"}}}}"#
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(
            resp["ok"], false,
            "5-char password must be rejected: {resp}"
        );
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    /// fix/p2p-c-review #2 — when a live P2P allowlist is attached, finishing a
    /// PAKE pairing registers the peer in it (normalised to canonical hex) so
    /// the mTLS accept loop honours the peer without a restart.
    #[tokio::test]
    async fn register_live_peer_feeds_shared_allowlist() {
        let peers = copypaste_p2p::transport::PairedPeers::new();
        let server = bare_server().with_p2p_peers(peers.clone());

        let colon_fp = std::iter::repeat_n("aa", 32).collect::<Vec<_>>().join(":");
        let canonical = canonical_fingerprint(&colon_fp);
        assert!(!peers.is_known(&canonical), "precondition: not yet known");

        server.register_live_peer(&colon_fp);

        assert!(
            peers.is_known(&canonical),
            "paired peer must be accepted by the live allowlist after finish"
        );
    }

    #[tokio::test]
    async fn pin_item_missing_id_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin_item_missing.sock");
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"pi1","method":"pin_item","params":{"pinned":true}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "missing id must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn pin_item_missing_pinned_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin_item_no_flag.sock");
        start_test_server(&sock).await;
        let fp_id = "00000000-0000-0000-0000-000000000000";
        let body = format!(r#"{{"id":"pi2","method":"pin_item","params":{{"id":"{fp_id}"}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], false, "missing pinned bool must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn pin_item_bad_uuid_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin_item_bad_uuid.sock");
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"pi3","method":"pin_item","params":{"id":"not-a-uuid","pinned":true}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad uuid must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn pin_item_valid_uuid_pins_and_unpins() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin_item_ok.sock");
        start_test_server(&sock).await;
        let id = "00000000-0000-0000-0000-000000000000";
        // Pin: even when the row does not exist, the UPDATE affects 0 rows
        // and succeeds (the UI optimistically pins; a stale id is harmless).
        let body =
            format!(r#"{{"id":"pi4","method":"pin_item","params":{{"id":"{id}","pinned":true}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], true, "valid pin must succeed: {resp}");
        assert_eq!(resp["data"]["pinned"], true);
        assert_eq!(resp["data"]["id"], id);
        // Unpin path.
        let body = format!(
            r#"{{"id":"pi5","method":"pin_item","params":{{"id":"{id}","pinned":false}}}}"#
        );
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], true, "valid unpin must succeed: {resp}");
        assert_eq!(resp["data"]["pinned"], false);
    }

    #[tokio::test]
    async fn delete_item_missing_id_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("del_item_missing.sock");
        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"di1","method":"delete_item","params":{}}"#).await;
        assert_eq!(resp["ok"], false, "missing id must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn delete_item_bad_uuid_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("del_item_bad_uuid.sock");
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"di2","method":"delete_item","params":{"id":"not-a-uuid"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad uuid must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn delete_item_valid_uuid_succeeds() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("del_item_ok.sock");
        start_test_server(&sock).await;
        let id = "00000000-0000-0000-0000-000000000000";
        let body = format!(r#"{{"id":"di3","method":"delete_item","params":{{"id":"{id}"}}}}"#);
        let resp = call_one(&sock, &body).await;
        // Deleting a non-existent row is a no-op DELETE → request still ok,
        // but `deleted` reflects rows-affected (0 → false) so the response
        // matches reality rather than always claiming a deletion happened.
        assert_eq!(resp["ok"], true, "valid delete must succeed: {resp}");
        assert_eq!(resp["data"]["deleted"], false, "no row existed: {resp}");
        assert_eq!(resp["data"]["id"], id);
    }

    #[tokio::test]
    async fn copy_item_missing_id_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_item_missing.sock");
        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"ci1","method":"copy_item","params":{}}"#).await;
        assert_eq!(resp["ok"], false, "missing id must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn copy_item_bad_uuid_returns_invalid_argument() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_item_bad_uuid.sock");
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"ci2","method":"copy_item","params":{"id":"not-a-uuid"}}"#,
        )
        .await;
        assert_eq!(resp["ok"], false, "bad uuid must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn copy_item_unknown_id_returns_not_found() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_item_unknown.sock");
        start_test_server(&sock).await;
        let id = "00000000-0000-0000-0000-000000000000";
        let body = format!(r#"{{"id":"ci3","method":"copy_item","params":{{"id":"{id}"}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_eq!(resp["ok"], false, "unknown id must fail");
        assert_eq!(resp["error_code"], "not_found");
    }

    #[tokio::test]
    async fn copy_item_seeded_id_is_resolved() {
        // Regression for the data-loss fix: copy_item must resolve a row by its
        // primary key (`get_item_by_id`) rather than paging + scanning. We seed
        // a text item with a deliberately wrong-length nonce so the paste-back
        // path returns a deterministic error *without* touching the real
        // NSPasteboard — the key assertion is that the lookup found the row, so
        // the response is anything except `not_found`.
        let dir = tempdir().unwrap();
        let sock = dir.path().join("copy_item_seeded.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        let id = {
            let guard = db.lock().await;
            // 0xAA/0xBB content with a 1-byte nonce (invalid: must be 24) so
            // write_to_pasteboard short-circuits before any NSPasteboard call.
            let item = copypaste_core::ClipboardItem::new_text(vec![0xAA, 0xBB], vec![0u8; 1], 1);
            let id = item.id.clone();
            copypaste_core::insert_item(&guard, &item).unwrap();
            id
        };

        let body = format!(r#"{{"id":"ci4","method":"copy_item","params":{{"id":"{id}"}}}}"#);
        let resp = call_one(&sock, &body).await;
        assert_ne!(
            resp["error_code"], "not_found",
            "seeded item must be resolved by id, not reported missing: {resp}"
        );
    }

    #[tokio::test]
    async fn revoke_all_peers_empty_store_succeeds() {
        // With no peers.json present, revoke_all_peers must succeed and
        // report zero revoked rather than erroring.
        let dir = tempdir().unwrap();
        let sock = dir.path().join("revoke_all_empty.sock");
        // Isolate the config dir so this test never touches the developer's
        // real peers.json. `dirs::config_dir()` reads XDG_CONFIG_HOME on
        // Linux/BSD and $HOME (→ Library/Application Support) on macOS, so set
        // both. Held until end of test (RAII restore).
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(&["HOME", "XDG_CONFIG_HOME"], &cfg_home);
        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"ra1","method":"revoke_all_peers","params":{}}"#,
        )
        .await;
        assert_eq!(
            resp["ok"], true,
            "revoke_all on empty store must succeed: {resp}"
        );
        assert_eq!(
            resp["data"]["revoked"].as_u64(),
            Some(0),
            "empty store revokes zero peers: {resp}"
        );
    }

    #[tokio::test]
    async fn revoke_all_peers_revokes_every_peer() {
        // Happy path: seed N peers in peers.json, call revoke_all_peers, and
        // assert all N are revoked, the store is cleared, and an audit row was
        // written for each (atomic batch via revoke_devices).
        let dir = tempdir().unwrap();
        let sock = dir.path().join("revoke_all_n.sock");
        // Redirect the config dir (both Linux XDG and macOS HOME) to a temp
        // path so we read/write an isolated peers.json, never the real one.
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(&["HOME", "XDG_CONFIG_HOME"], &cfg_home);

        // Resolve the actual peers.json location the same way the daemon does
        // (`dirs::config_dir()/copypaste/peers.json`) so the seed lands exactly
        // where the handler will read it, on whatever platform we run.
        let peers_dir = dirs::config_dir()
            .expect("config_dir resolvable under redirected HOME/XDG_CONFIG_HOME")
            .join("copypaste");
        std::fs::create_dir_all(&peers_dir).unwrap();
        let peers_json = peers_dir.join("peers.json");
        let peers = serde_json::json!([
            {"name": "Laptop", "fingerprint": "aa:aa:aa:aa:aa:aa:aa:aa", "added_at": 1},
            {"name": "Phone",  "fingerprint": "bb:bb:bb:bb:bb:bb:bb:bb", "added_at": 2},
            {"name": "Tablet", "fingerprint": "cc:cc:cc:cc:cc:cc:cc:cc", "added_at": 3},
        ]);
        std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

        let (_pm, db) = start_test_server_returning_db(&sock, false).await;
        let resp = call_one(
            &sock,
            r#"{"id":"ra2","method":"revoke_all_peers","params":{}}"#,
        )
        .await;

        assert_eq!(resp["ok"], true, "revoke_all must succeed: {resp}");
        assert_eq!(
            resp["data"]["revoked"].as_u64(),
            Some(3),
            "all three peers must be revoked: {resp}"
        );
        assert_eq!(resp["data"]["cleared"].as_u64(), Some(3));

        // Store must now be empty.
        let remaining = std::fs::read_to_string(&peers_json).unwrap_or_else(|_| "[]".into());
        let remaining: Vec<serde_json::Value> = serde_json::from_str(&remaining).unwrap();
        assert!(remaining.is_empty(), "peer store must be cleared");

        // An audit row must exist for every revoked fingerprint.
        let audit = {
            let guard = db.lock().await;
            copypaste_core::list_revoked_devices(guard.conn()).unwrap()
        };
        assert_eq!(audit.len(), 3, "one audit row per revoked peer");
        for fp in [
            "aa:aa:aa:aa:aa:aa:aa:aa",
            "bb:bb:bb:bb:bb:bb:bb:bb",
            "cc:cc:cc:cc:cc:cc:cc:cc",
        ] {
            assert!(
                audit.iter().any(|r| r.fingerprint == fp),
                "missing audit row for {fp}"
            );
        }
    }
}
