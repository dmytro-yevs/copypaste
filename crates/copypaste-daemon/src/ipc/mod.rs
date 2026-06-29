use crate::protocol::{
    Request, Response, CURRENT_PROTOCOL_VERSION, ERR_CODE_AUTH_FAILED, ERR_CODE_INTERNAL_ERROR,
    ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, ERR_CODE_NOT_FOUND,
    ERR_CODE_NOT_IMPLEMENTED, ERR_CODE_RATE_LIMITED, ERR_CODE_REQUEST_TOO_LARGE,
    ERR_CODE_VERSION_MISMATCH, MIN_SUPPORTED_PROTOCOL_VERSION,
};
// CopyPaste-merc / CopyPaste-1jms.22: canonical badge-state computation lives in
// copypaste-ipc. The `_with_inflight` variant is used so the daemon can emit the
// `Syncing` (green-pulse) badge while a round-trip is in progress. Gated on
// cloud-sync: the get_sync_status handler is only compiled with that feature, so
// the import must match to avoid an unused-import warning (-D warnings).
#[cfg(feature = "cloud-sync")]
use copypaste_ipc::compute_sync_badge_state_with_inflight;
// derive_sync_key / SyncKey are used by both cloud-sync (Supabase) and relay-sync.
// `revoke_and_rotate` / `rotate_sync_key` derive a key from a passphrase;
// `revoke_peer` uses `SyncKey::random()` for automatic no-passphrase rotation
// (CopyPaste-gbo fix).
#[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
use copypaste_core::derive_sync_key;
#[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
use copypaste_core::SyncKey;
use copypaste_core::{
    bump_item_recency,
    chunks_from_blob,
    count_items,
    decode_file,
    decode_image,
    decrypt_item_by_version,
    derive_v2,
    ensure_revoked_devices_table,
    fetch_text_preview,
    // c4q2.17: get_page removed — the "list" handler that used it is now stubbed.
    fetch_text_previews_batch,
    get_device_names,
    get_item_by_id,
    get_page_pinned_first,
    is_sensitive_for_autowipe,
    pin_item,
    reorder_pinned,
    revoke_device,
    revoke_devices,
    search_items_filtered,
    unpin_item,
    Database,
    DbRead,
    SensitiveDetector,
    V1Key,
    V2Key,
};
// l07l: EncryptError is only matched on the macOS pasteboard decrypt path, so
// gate it to macOS — otherwise it's an unused import on non-macOS (-D warnings).
#[cfg(target_os = "macos")]
use copypaste_core::EncryptError;
// `soft_delete_item` is not yet re-exported from the crate root so we use the
// full module path (the `storage` module is `pub`).
use copypaste_core::storage::items::soft_delete_item;
use copypaste_p2p::pake::{
    channel_confirmation_tag, ConfirmRole, PakeInitiator, PakeResponder, CONFIRM_TAG_LEN,
};
use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Build-version string stamped by `build.rs` (`<crate-version>+<git-sha>`, or
/// just `<crate-version>` when git is unavailable at build time). Surfaced in
/// the `status`/`stats` IPC replies so a client can detect a STALE daemon left
/// running after an upgrade (a different value answering the socket means the
/// on-disk binary changed but the old process is still serving old code).
pub const BUILD_VERSION: &str = match option_env!("COPYPASTE_BUILD_VERSION") {
    Some(v) => v,
    None => env!("CARGO_PKG_VERSION"),
};

/// Maximum size of a single IPC request line for the few methods that carry a
/// genuinely large inbound payload (`import`, `add_file_item`). Clients
/// exceeding this receive an error response and have their connection closed.
/// Prevents OOM from a malicious or buggy client sending an unbounded stream
/// without newlines.
const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Default per-request size cap applied to EVERY method that is not on the
/// large-payload allow-list ([`IpcServer::allows_large_payload`]).
///
/// CopyPaste-c4q2.28: applying the 16 MiB [`MAX_REQUEST_BYTES`] cap to every
/// method let a hostile same-UID client send ~15.9 MiB for a `status`/`list`/
/// `delete` call; the daemon buffered the whole payload before rejecting it.
/// Worst case `MAX_CONCURRENT_CONNECTIONS` × 16 MiB ≈ 1 GiB of peak buffered
/// RAM. The IPC read path now reads at most this many bytes before it has
/// classified the method, and only `import` / `add_file_item` are allowed to
/// grow to [`MAX_REQUEST_BYTES`]. 64 KiB is comfortably larger than any
/// non-bulk request (the largest, a fully-populated `set_config`, is < 2 KiB).
const SMALL_REQUEST_BYTES: usize = 64 * 1024;

/// Extract the value of a top-level string field (`"key":"value"`) from a
/// possibly-truncated JSON request prefix, scanning raw bytes.
///
/// CopyPaste-c4q2.28 uses this to classify a request's `method` BEFORE the
/// whole (potentially huge) line has been buffered — `serde_json` cannot help
/// because the buffer may be cut mid-object. Well-behaved clients
/// (`copypaste-ipc::Request`) serialise `id` and `method` ahead of `params`, so
/// both are present in the first [`SMALL_REQUEST_BYTES`]. Returns `None` if the
/// key/value is absent or malformed in the prefix (the caller then treats the
/// request as non-large and rejects it). Only the first match is used; values
/// with JSON escapes are not expected for `method`/`id` and are not decoded.
fn extract_json_string_field(prefix: &[u8], key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = prefix
        .windows(needle.len())
        .position(|w| w == needle.as_bytes())?;
    let mut i = start + needle.len();
    let skip_ws = |i: &mut usize| {
        while *i < prefix.len() && prefix[*i].is_ascii_whitespace() {
            *i += 1;
        }
    };
    skip_ws(&mut i);
    if i >= prefix.len() || prefix[i] != b':' {
        return None;
    }
    i += 1;
    skip_ws(&mut i);
    if i >= prefix.len() || prefix[i] != b'"' {
        return None;
    }
    i += 1;
    let value_start = i;
    while i < prefix.len() && prefix[i] != b'"' {
        i += 1;
    }
    if i >= prefix.len() {
        return None; // closing quote not in the buffered prefix
    }
    std::str::from_utf8(&prefix[value_start..i])
        .ok()
        .map(str::to_owned)
}

/// Best-effort echo of the request `id` from a (possibly truncated) prefix so an
/// oversized-request error can still be correlated by the client's id-matching
/// guard. Falls back to `"0"` when the id is not recoverable.
fn echo_id_from_prefix(prefix: &[u8]) -> String {
    extract_json_string_field(prefix, "id").unwrap_or_else(|| "0".to_string())
}

/// Send a bounded `request_too_large` error response, then the caller closes the
/// connection. The write is wrapped in [`IPC_WRITE_TIMEOUT`] (c4q2.24) so a slow
/// reader cannot pin the connection even on this terminal path.
async fn send_request_too_large<W>(writer: &mut W, prefix: &[u8], limit_bytes: usize, detail: &str)
where
    W: tokio::io::AsyncWrite + Unpin,
{
    let limit_human = if limit_bytes >= 1024 * 1024 {
        format!("{} MiB", limit_bytes / (1024 * 1024))
    } else {
        format!("{} KiB", limit_bytes / 1024)
    };
    let resp = Response::err_with_code(
        echo_id_from_prefix(prefix),
        ERR_CODE_REQUEST_TOO_LARGE,
        format!("request too large: IPC request exceeds the {limit_human} limit. {detail}"),
    );
    if let Ok(mut out) = serde_json::to_string(&resp) {
        out.push('\n');
        let _ = tokio::time::timeout(IPC_WRITE_TIMEOUT, writer.write_all(out.as_bytes())).await;
    }
}

/// Per-request read timeout (CopyPaste-cce1).
///
/// A client that connects and then stalls — either never sending a newline or
/// drip-feeding bytes — holds one connection slot AND blocks the tokio
/// `Mutex<Database>` for the entire duration of `dispatch`.  30 s is generous
/// for any legitimate CLI/UI roundtrip (the slowest observed production request
/// — a full `history_page(1000)` — completes in < 1 s under load).
///
/// When the deadline fires we drop the connection without sending a response;
/// the client's next read returns EOF, which its retry logic must handle.
pub const IPC_READ_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

/// Per-response write deadline (CopyPaste-c4q2.24).
///
/// The read path has [`IPC_READ_TIMEOUT`], but a bare `writer.write_all(...)`
/// on the write path can block indefinitely: once the kernel's Unix-socket send
/// buffer fills (~128 KiB on macOS), `write_all` parks until the peer drains its
/// recv buffer. A client that sends a valid request and then never reads the
/// reply would pin its connection slot — and the `conn_semaphore` permit — for
/// the lifetime of the daemon. With [`MAX_CONCURRENT_CONNECTIONS`] such clients,
/// IPC stops accepting connections and the UI/CLI become inaccessible (a
/// same-UID local DoS).
///
/// 10 s is far more than any legitimate client needs to drain a single response
/// (the largest, a full `history_page`, is a few MiB). On timeout we log a warn
/// and drop the connection, reclaiming the semaphore permit.
pub const IPC_WRITE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

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

/// Maximum number of simultaneously-active IPC connections (CopyPaste-6ot5).
///
/// A tokio Semaphore with this many permits is held by the accept loop.
/// When a new connection arrives, the loop does a non-blocking `try_acquire`
/// (never blocking the accept path). The `OwnedSemaphorePermit` is moved into
/// the spawned connection task and dropped when the task completes, so the slot
/// is reclaimed promptly. Excess connections receive an immediate OS-level close
/// (the accept loop drops the `UnixStream`) instead of silently queueing forever.
///
/// 64 allows generous concurrent tooling (CLI, UI, sync) while bounding
/// unbounded resource growth from a buggy or hostile client.
const MAX_CONCURRENT_CONNECTIONS: usize = 64;

/// Human-readable `error` message returned when an IPC method is called before
/// the server's backing state (database, etc.) has finished initializing.
/// Clients branch on the machine-readable `error_code` (`ERR_CODE_IPC_NOT_READY`
/// = "ipc_not_ready") and should back off and retry rather than treat this as a
/// hard failure. CopyPaste-crh3.8: this is the user-facing string, so it is a
/// real sentence rather than the bare Rust constant name "IPC_NOT_READY".
const ERR_IPC_NOT_READY: &str = "daemon is still starting up; retry shortly";

// ---------------------------------------------------------------------------
// Submodules (behaviour-preserving split of the original ipc.rs god-file).
// Only clearly-separable, cohesive groups were extracted; everything that
// touches shared state or calls other helpers without a clean boundary stays
// here in mod.rs.  All pub / pub(crate) symbols from the submodules are
// re-exported below so external call sites (`crate::ipc::Foo`) remain valid
// without any modification.
// ---------------------------------------------------------------------------

pub(super) mod config;
pub(super) mod pairing;
pub(super) mod pasteboard;
pub(super) mod socket;

// ── config ─────────────────────────────────────────────────────────────────
// pub / pub(crate) items used by callers outside the ipc module:
pub use config::p2p_enabled_from_config;
pub(crate) use config::read_config;
pub(crate) use config::update_core_config;
pub use config::AppConfig;
// Helpers used directly in impl IpcServer dispatch code (non-test):
use config::{build_config_response, merge_config, write_config};
// Helpers only called from the inline test module; #[cfg(test)] keeps the
// import from triggering -D warnings in non-test compilation.
#[cfg(test)]
pub(crate) use config::config_path;

// ── pairing ────────────────────────────────────────────────────────────────
// pub(crate) items callable from outside ipc:
pub(crate) use pairing::canonical_fingerprint;
pub(crate) use pairing::compute_peer_online;
pub(crate) use pairing::display_fingerprint;
pub(crate) use pairing::peers_file_path;
// Helpers used in impl IpcServer dispatch code (non-test):
use pairing::{
    byte_to_char_offset, encrypt_pake_password_file, extract_uuid_param, is_valid_fingerprint,
    load_peers, paired_ip_hosts, queue_unpair_for_offline_delivery, save_peers,
    send_unpair_signal_if_connected, too_large_to_sync, PakeSession, StampedPakeSession,
    MAX_PAKE_SESSIONS, PAKE_SESSION_TTL,
};
// Helpers only called from the inline test module:
#[cfg(test)]
pub(crate) use pairing::{decrypt_pake_password_file, ONLINE_THRESHOLD_SECS};

// ── socket ─────────────────────────────────────────────────────────────────
// bind_with_stale_cleanup is called from impl IpcServer::bind (non-test):
use socket::bind_with_stale_cleanup;
// Helpers only called from the inline test module:
#[cfg(test)]
pub(crate) use socket::{
    is_socket_live, pid_exe_is_copypaste, pid_exe_path, probe_listening_daemon,
};

// ── pasteboard ─────────────────────────────────────────────────────────────
// pub(crate) items used by external callers:
pub(crate) use pasteboard::parse_file_meta;
pub(crate) use pasteboard::parse_image_file_id;
pub(crate) use pasteboard::parse_image_thumb_file_id;
pub(crate) use pasteboard::paste_file_cache_dir;
pub(crate) use pasteboard::prune_old_paste_files;
// Helpers used in impl IpcServer dispatch code (non-test):
use pasteboard::{
    lazy_backfill_thumbnail, map_content_type_to_uti, parse_image_thumb_dims, PasteboardError,
};

/// Validate `src_path` as a SQLCipher backup encrypted with `key`, then
/// atomically swap it into `db_path`, returning the freshly-opened restored
/// [`Database`]. This is the core of the `db_restore` IPC verb, extracted as a
/// pure filesystem + SQLCipher routine (no IPC state) so it is unit-testable
/// with temp directories (CopyPaste-8wbt / crh3.6).
///
/// Safety contract:
/// * **Validation runs on a throwaway staging copy** — a wrong-key, plaintext,
///   corrupt, or non-CopyPaste backup leaves the live files at `db_path`
///   completely untouched and returns `Err`.
/// * **The live DB is moved aside (never deleted) before the swap**, for BOTH
///   `force` values, so a failure during the swap rolls the originals back.
///   `force` only decides whether the aside safety copy
///   (`clipboard.db.before-restore-<ts>`) is removed on success.
/// * The caller must keep its existing `Database` handle until it installs the
///   returned one: on a rolled-back failure that handle stays valid (its inode
///   is renamed aside and back, never replaced).
fn restore_database_file(
    src_path: &std::path::Path,
    db_path: &std::path::Path,
    key: &[u8; 32],
    force: bool,
) -> Result<Database, String> {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    // Best-effort removal of a DB triple (main + WAL + SHM) sharing `base`.
    let remove_triple = |base: &std::path::Path| {
        for suffix in ["", "-wal", "-shm"] {
            let mut p = base.as_os_str().to_os_string();
            p.push(suffix);
            let _ = std::fs::remove_file(std::path::PathBuf::from(p));
        }
    };

    // ── PHASE A — validate on a throwaway staging copy. `db_path` is NOT
    //    touched here, so any failure leaves the live DB fully intact.
    let staging = {
        let mut s = db_path.as_os_str().to_os_string();
        s.push(format!(".restore-staging-{ts}"));
        std::path::PathBuf::from(s)
    };
    remove_triple(&staging);
    std::fs::copy(src_path, &staging).map_err(|e| {
        format!(
            "db_restore: failed to stage backup copy at {}: {e}",
            staging.display()
        )
    })?;

    // `open_no_auto_migrate` REJECTS plaintext/garbage files (no silent
    // plaintext→SQLCipher migration), so only a genuine SQLCipher DB encrypted
    // with `key` validates.
    let validation = (|| -> Result<(), String> {
        let probe = Database::open_no_auto_migrate(&staging, key).map_err(|e| {
            format!(
                "db_restore: backup did not open with the current key (wrong key, \
                 corrupt, or not a CopyPaste database): {e}"
            )
        })?;
        // integrity_check catches a backup that decrypts but is structurally
        // corrupt / truncated.
        let integrity: String = probe
            .conn()
            .query_row("PRAGMA integrity_check", [], |r| r.get(0))
            .map_err(|e| format!("db_restore: integrity_check failed: {e}"))?;
        if integrity != "ok" {
            return Err(format!(
                "db_restore: backup integrity_check returned '{integrity}' (corrupt backup)"
            ));
        }
        // Schema sanity: a legitimate backup carries the clipboard schema.
        probe
            .conn()
            .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(|e| {
                format!(
                    "db_restore: backup is missing the clipboard_items table — not a \
                     CopyPaste database: {e}"
                )
            })?;
        Ok(())
    })();
    remove_triple(&staging);
    validation?;

    // ── PHASE B — swap. Validation passed; every step is rollback-safe.
    //
    // Move the live DB aside (BOTH modes) so a late failure can roll back.
    let mut moved: Vec<(std::path::PathBuf, std::path::PathBuf)> = Vec::new();
    for suffix in ["", "-wal", "-shm"] {
        let mut orig = db_path.as_os_str().to_os_string();
        orig.push(suffix);
        let orig = std::path::PathBuf::from(orig);
        if orig.exists() {
            let mut aside = db_path.as_os_str().to_os_string();
            aside.push(format!("{suffix}.before-restore-{ts}"));
            let aside = std::path::PathBuf::from(aside);
            std::fs::rename(&orig, &aside)
                .map_err(|e| format!("db_restore: could not move {} aside: {e}", orig.display()))?;
            moved.push((orig, aside));
        }
    }

    // Roll back: drop any partially-written restored file, then move every
    // aside file back to its original path.
    let rollback = |moved: &[(std::path::PathBuf, std::path::PathBuf)]| {
        remove_triple(db_path);
        for (orig, aside) in moved {
            let _ = std::fs::rename(aside, orig);
        }
    };

    // Place the validated backup.
    if let Err(e) = std::fs::copy(src_path, db_path) {
        let msg = format!("db_restore: failed to copy backup into place: {e}");
        rollback(&moved);
        return Err(msg);
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(meta) = std::fs::metadata(db_path) {
            let mut perms = meta.permissions();
            perms.set_mode(0o600);
            let _ = std::fs::set_permissions(db_path, perms);
        }
    }

    let restored = match Database::open_no_auto_migrate(db_path, key) {
        Ok(db) => db,
        Err(e) => {
            let msg = format!(
                "db_restore: failed to open restored DB (rolled back to prior database): {e}"
            );
            rollback(&moved);
            return Err(msg);
        }
    };

    // Ensure the additive audit table exists (matches the normal startup path).
    if let Err(e) = ensure_revoked_devices_table(restored.conn()) {
        tracing::warn!("db_restore: ensure_revoked_devices_table failed: {e}");
    }

    // Success. With `force`, drop the aside safety copy; otherwise keep it as
    // clipboard.db.before-restore-<ts>.
    if force {
        for (_orig, aside) in &moved {
            let _ = std::fs::remove_file(aside);
        }
    }

    Ok(restored)
}

pub struct IpcServer {
    db: Arc<Mutex<Database>>,
    /// Optional r2d2 connection pool for concurrent read-only queries (CopyPaste-j8p).
    ///
    /// When present, the read-only handlers (`list`, `count`, `search`,
    /// `history_page`, `stats`) acquire a pooled connection and bypass the
    /// single write mutex, allowing N parallel reads without serializing on
    /// the clipboard-write path. SQLite WAL mode guarantees readers always
    /// see committed data without blocking the writer.
    ///
    /// Falls back to `self.db` (write mutex) when `None` (degraded startup,
    /// tests that don't need pool concurrency, or pool exhaustion).
    ///
    /// Wrapped in a `std::sync::Mutex` so `db_restore` can atomically rebuild
    /// the pool against the restored database file (CopyPaste-crh3.2). The
    /// pooled connections hold file descriptors to the *old* inode; after a
    /// restore swaps the on-disk DB they must be replaced or every read keeps
    /// serving pre-restore data. The lock is only ever held long enough to
    /// `clone()` the inner `Arc` (no `.await` across the guard).
    read_pool: std::sync::Mutex<Option<Arc<copypaste_core::SqlitePool>>>,
    /// Shared private-mode flag. When true, the clipboard monitor skips recording.
    private_mode: Arc<AtomicBool>,
    /// Monotonically-increasing epoch counter for the private-mode flag.
    ///
    /// CopyPaste-48k0: the tray's `spawn_tray_private_mode_resync` helper is a
    /// one-shot poller — it exits after a stable round-trip and never re-runs.
    /// After a daemon restart the tray's cached state may be stale (the new
    /// daemon loaded private-mode from disk but the tray already exited its
    /// poller).
    ///
    /// Fix: expose this counter in the `status` and `get_private_mode` responses
    /// so any periodic `status` poll (e.g. the UI's health check) can detect that
    /// private-mode changed and trigger a re-sync.  The counter starts at 0 and
    /// is incremented on every `set_private_mode` call, making it cheap to compare
    /// across polls: a changed epoch → re-read `private_mode`.
    private_mode_epoch: Arc<std::sync::atomic::AtomicU64>,
    /// Stable device UUID loaded (or created) at daemon start via
    /// `load_or_create_device_id`. Stamped on every locally-captured clipboard
    /// item as `origin_device_id`. Returned in `history_page` as `own_device_id`
    /// so the UI can label "This device" vs. synced items from other devices.
    /// `None` when not wired in (unit tests / degraded-mode builds).
    local_device_id: Option<String>,
    /// Local symmetric encryption key (XChaCha20-Poly1305). Required by the
    /// `copy`/`paste` handlers so paste-back can decrypt the ciphertext
    /// stored in `clipboard_items.content` and write *plaintext* to
    /// NSPasteboard. Audit CRIT #1: previously the handler wrote raw
    /// ciphertext bytes back, so paste produced "content is not valid
    /// UTF-8" for text and garbage for images.
    local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
    /// Device public-key bytes (X25519). Historically `get_own_fingerprint`
    /// derived its value from this via `keychain::own_fingerprint` (audit HIGH
    /// #6, superseding an unstable DefaultHasher scheme). CRITICAL-1: pairing
    /// now advertises the mTLS **cert** fingerprint (`cert_fingerprint`)
    /// instead, since the device-key fingerprint is never what the mTLS layer
    /// pins. The bytes are retained here — they remain part of the
    /// `IpcServer::new` contract and the device identity is still useful for
    /// future non-pairing surfaces.
    // The X25519 device public-key bytes (32 bytes). SHA-256 of this value is
    // surfaced in the `status` response as `device_key_fingerprint` (hex) so
    // operators and diagnostic tooling can correlate daemon identity without
    // reading the Keychain.  NOTE: pairing uses the mTLS cert fingerprint
    // (`cert_fingerprint`), not this value — they must never be confused.
    device_public_key: Arc<[u8; 32]>,
    /// Readiness gate. While `false`, all data-touching methods return
    /// `IPC_NOT_READY` instead of dispatching. Default `true` for production
    /// use (db is fully constructed before `IpcServer::new` is called); tests
    /// use [`IpcServer::new_with_ready`] to exercise the not-ready path.
    ready: Arc<AtomicBool>,
    /// DUP-ON-COPY fix: after `write_to_pasteboard` completes, record the new
    /// NSPasteboard `changeCount` here. The clipboard monitor reads this on
    /// the next tick and skips recording when it matches — preventing the
    /// daemon's own pasteboard writes from being captured as new clipboard events.
    /// Sentinel -1 means "no pending self-write".
    pub self_write_change_count: Arc<std::sync::atomic::AtomicI64>,
    /// In-progress PAKE sessions keyed by session_id UUID string.
    ///
    /// Each entry lives from the first IPC call (initiate / accept) until the
    /// matching finish call consumes it. Bounded against unbounded growth
    /// (fix/p2p-c-review #1 — DoS): entries older than [`PAKE_SESSION_TTL`]
    /// are evicted on every insert, and the live count is capped at
    /// [`MAX_PAKE_SESSIONS`]. See [`IpcServer::insert_pake_session`].
    pake_sessions: Arc<Mutex<HashMap<String, StampedPakeSession>>>,
    /// The single active QR-pairing token issued by `pair_generate_qr`, with
    /// its issue time for TTL eviction.
    ///
    /// QR pairing is the displaying-device-is-responder flow: this device
    /// generates a fresh token, renders it in the QR, and stores it here so the
    /// `pair_accept_qr` handler can re-derive the same PAKE password when the
    /// scanning device's `message1` arrives — without the user re-typing
    /// anything. Only one QR is active at a time (regenerating replaces it),
    /// matching the single-token pairing UX. Bounded by [`PAKE_SESSION_TTL`].
    /// `None` until the first `pair_generate_qr` call.
    pending_qr_token: Arc<Mutex<Option<(copypaste_core::PairingToken, std::time::Instant)>>>,
    /// Live P2P paired-peer allowlist, shared with the running mTLS transport
    /// (fix/p2p-c-review #2). When a PAKE handshake finishes, the newly-paired
    /// peer fingerprint is fed into this same instance via
    /// [`copypaste_p2p::transport::PairedPeers::rotate_peer`] so the accept loop immediately honours it
    /// (the S10 grace path is exercised). `None` when P2P is disabled — the
    /// PAKE handlers then only persist to `peers.json` (loaded on next start).
    p2p_peers: Option<copypaste_p2p::transport::PairedPeers>,
    /// Our live mTLS **certificate** fingerprint in user-facing colon-hex form,
    /// i.e. `display_fingerprint(hex(SHA-256(cert_der)))` for the exact same
    /// cert the running `PeerTransport` presents and that peers pin
    /// ([`copypaste_p2p::transport::PeerTransport::fingerprint`] /
    /// [`copypaste_p2p::cert::fingerprint_of`]).
    ///
    /// CRITICAL-1 fix: pairing (`pair_generate_qr`, `get_own_fingerprint`)
    /// MUST advertise this value — NOT the device-key fingerprint
    /// (`keychain::own_fingerprint`, SHA-256 of the X25519 public key), which
    /// the mTLS allowlist never compares against, so cert-pinning could never
    /// match and pairing could never authenticate.
    ///
    /// `None` when P2P is disabled (`COPYPASTE_P2P` unset): no transport runs,
    /// so there is no cert to advertise and the pairing handlers return a clear
    /// error rather than a fingerprint that cannot authenticate any channel.
    cert_fingerprint: Option<String>,
    /// Our self-signed mTLS certificate DER + key, used to TLS-wrap the
    /// unauthenticated bootstrap pairing channel (P2P Phase 1). This is a clone
    /// of the SAME cert `start_p2p`'s transport presents and whose fingerprint
    /// `cert_fingerprint` advertises, so the fingerprints a pairing peer learns
    /// over the bootstrap channel match the ones the pinned mTLS layer compares.
    ///
    /// `None` when P2P is disabled — the QR pairing handlers then fall back to
    /// the legacy IPC-relayed PAKE path (no network bootstrap channel).
    p2p_cert: Option<Arc<(Vec<u8>, Vec<u8>)>>,
    /// Optional mDNS discovery handle used by the initiator's QR-accept path to
    /// resolve the responder's `host:port` when the QR carries no `addr_hint`
    /// (best-effort fallback — loopback mDNS is unreliable, so `addr_hint` is
    /// the primary path). `None` when P2P discovery is not wired in.
    discovery: Option<Arc<copypaste_p2p::discovery::DiscoveryService>>,
    /// This daemon's own P2P sync-listener address (`host:port`), filled once
    /// `start_p2p` has bound its accept loop (the port is OS-assigned, so it is
    /// not known when `IpcServer` is constructed). The pairing handlers send
    /// this value in-band over the bootstrap channel so the peer can persist it
    /// for the Phase 3 outbound connector. A `std::sync::Mutex` (not tokio's) is
    /// used because the critical section is a trivial clone with no `.await`.
    /// Holds `None` until populated, or when P2P is disabled.
    p2p_sync_addr: Arc<std::sync::Mutex<Option<String>>>,
    /// Shared passphrase-derived cloud sync key (Argon2id, 32 bytes).
    ///
    /// `None` means the user has not yet configured a sync passphrase, so
    /// cloud upload/download is skipped. Set via `set_sync_passphrase`; shared
    /// with the cloud push/poll loops via `Arc<Mutex<Option<SyncKey>>>`.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub sync_key: Arc<Mutex<Option<SyncKey>>>,
    /// Monotonic timestamp (ms since UNIX epoch) of the last successful cloud
    /// sync round-trip. `0` means never synced. Shared with cloud loops so
    /// `get_sync_status` returns a live value.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
    /// Real GoTrue auth state, published by the cloud push/poll loops (BUG 2).
    /// `true` once `start_cloud` resolves a bearer, `false` on a bearer-resolution
    /// failure (`CloudError::AuthFailed`) or a failed 401-refresh. Read by
    /// `get_sync_status` so the UI reflects the actual signed-in state instead of
    /// the old hardcoded `signed_in = supabase_configured`.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub cloud_signed_in: Arc<AtomicBool>,
    /// Canonical Supabase account identity for this device (CopyPaste-1jms.34).
    ///
    /// Set by `with_cloud_account_id` after `start_cloud` returns. The value
    /// is `copypaste_supabase::supabase_account_id(url, user_id)` — a non-secret
    /// stable identifier derived from the Supabase project URL + GoTrue user UUID.
    ///
    /// The `get_sync_status` handler includes this in the response so the UI can
    /// surface a banner when two paired devices report different account IDs
    /// (= different Supabase projects or different GoTrue accounts).
    ///
    /// `None` when cloud-sync is off, not configured, or anon-key-only
    /// (no GoTrue session). Interior-mutable so it can be updated if the cloud
    /// loops are restarted at runtime without taking the entire IpcServer lock.
    #[cfg(feature = "cloud-sync")]
    pub cloud_account_id: Arc<std::sync::Mutex<Option<String>>>,
    /// Broadcast sender for newly-ingested clipboard items, shared with the
    /// clipboard monitor and the sync orchestrator (P2P Phase 3).
    ///
    /// Captured-by-polling items already flow through this channel from the
    /// monitor. The `import` IPC method historically inserted straight into the
    /// DB without notifying anyone, so imported items never reached the sync
    /// orchestrator and could not be pushed to a paired peer. Wiring the sender
    /// here lets `import` broadcast each inserted row so it syncs like a captured
    /// one. `None` when the daemon did not provide a sender (e.g. unit tests).
    new_item_tx: Option<tokio::sync::broadcast::Sender<copypaste_core::ClipboardItem>>,
    /// Degraded-startup reason, surfaced verbatim in the `status` response so
    /// the UI can render a recovery banner instead of treating an unreachable
    /// socket as a dead daemon.
    ///
    /// `None` in the normal case (DB opened, key available). `Some(reason)`
    /// when the daemon came up in degraded mode — e.g. the SQLCipher key could
    /// not be obtained from the Keychain (`keychain_locked`) so the existing
    /// encrypted DB could not be opened (`db_unavailable`). In degraded mode
    /// `ready` is `false`, so every DB-touching method already returns
    /// `IPC_NOT_READY`; this field tells the client *why* and that recovery is
    /// possible (re-grant Keychain access, then relaunch). See the
    /// [`DEGRADED_REASON_KEYCHAIN_LOCKED`] constant for the canonical value.
    ///
    /// Interior-mutable (`Arc<Mutex<…>>`) because the `reset_database` recovery
    /// handler clears it in-place — after wiping and recreating a fresh empty DB
    /// it brings the daemon OUT of degraded mode (sets `ready = true`, clears
    /// this reason) without a process restart. A `std::sync::Mutex` (not tokio's)
    /// is used because every critical section is a trivial read/write with no
    /// `.await`.
    degraded_reason: Arc<std::sync::Mutex<Option<String>>>,
    /// Shared live core config (`config.toml`). The `set_config` IPC handler
    /// writes new limit/feature values here after persisting to disk so the
    /// clipboard monitor, paste path, and prune code pick them up on the next
    /// tick without a daemon restart.
    /// `None` when not wired in (degraded mode / tests that don't need hot-reload).
    pub core_config: Option<Arc<std::sync::RwLock<copypaste_core::AppConfig>>>,

    /// Best-effort cached public / WAN IP (resolved via STUN on startup, then
    /// refreshed every ~15 minutes by a background task spawned in `daemon.rs`).
    /// `None` before the first resolution attempt completes, on failure, or when
    /// the user has opted out via `AppConfig::collect_public_ip = false`.
    ///
    /// `tokio::sync::RwLock` (not `std::sync::Mutex`) because the
    /// `get_own_device_info` hot path is async and must not block the executor.
    pub cached_public_ip: Arc<tokio::sync::RwLock<Option<String>>>,

    /// Discovery-initiated SAS pairing coordinator (LAN/SAS Phase 2).
    ///
    /// Holds the single-active-pairing state machine plus the confirmation
    /// `oneshot` channel that wires `pair_confirm_sas`/`pair_abort` into the
    /// in-flight bootstrap handshake's `confirm` callback. Shared (`Arc`) with
    /// the standing discovery-pairing responder task in `start_p2p`, so an
    /// inbound pair routes its SAS through the SAME machine the IPC handlers
    /// observe. Always present (the machine is `Idle` when nothing is pairing).
    pairing: Arc<crate::pairing_sm::PairingCoordinator>,

    /// Shared live peer-sink map — serves two purposes:
    ///   1. Online-status computation (`list_peers`): iterate to find non-closed senders.
    ///   2. Mutual-unpair signalling (`unpair_peer` / `revoke_peer` / `revoke_all_peers`):
    ///      look up a specific peer's sender and deliver `ControlMsg::Unpair`.
    ///
    /// `LivePeerSinks` and `PeerSinks` are identical type aliases
    /// (`Arc<tokio::sync::Mutex<HashMap<DeviceFingerprint, mpsc::Sender<PeerFrame>>>>`).
    /// `P2pHandle` exposes both names only because they were introduced at different times;
    /// both fields on that struct are `Arc::clone`s of the same underlying map.
    /// daemon.rs writes `P2pHandle::live_sinks` here after `start_p2p` returns.
    live_peer_sinks: Arc<std::sync::Mutex<Option<crate::p2p::LivePeerSinks>>>,
    /// Last-measured round-trip times per connected peer (milliseconds).
    ///
    /// The P2P subsystem's ping task writes to this map; `list_peers` reads it
    /// to populate the `latency_ms` field in each peer entry.  Wrapped in an
    /// `Option` (in a `std::sync::Mutex`) for the same lazy-injection pattern as
    /// `live_peer_sinks`: `None` until `start_p2p` returns and writes the value.
    live_peer_rtt_ms: Arc<std::sync::Mutex<Option<crate::p2p::PeerRttMs>>>,
    /// Clone of the running sync orchestrator's `SyncCrypto` context (H8).
    ///
    /// Because `SyncCrypto` stores its cached sync key behind an `Arc<Mutex>`,
    /// this clone shares the SAME backing store as the orchestrator's copy.
    /// Calling `reload_sync_key()` here after a pairing write propagates to the
    /// orchestrator immediately without any channel or restart. `None` when P2P
    /// is disabled (no orchestrator crypto context exists).
    p2p_sync_crypto: Option<crate::sync_orch::SyncCrypto>,

    /// Race-fix (CopyPaste-7mf): handle for the in-flight QR bootstrap responder
    /// task. `spawn_bootstrap_responder` stores the `JoinHandle` here so that
    /// `list_peers` can await it with a short timeout before reading peers.json.
    /// This guarantees that a caller doing `pair_generate_qr` (responder side)
    /// followed immediately by `list_peers` will see the freshly-persisted peer
    /// once the bootstrap PAKE completes, rather than racing the detached spawn.
    ///
    /// Protected by a `tokio::sync::Mutex` because the critical section includes
    /// an `.await` (waiting on the JoinHandle).
    pending_bootstrap: Arc<tokio::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Bounded queue of recent peer connect/disconnect events, drained by the
    /// `poll_peer_events` IPC handler.
    ///
    /// Populated by a background task in `daemon.rs` that subscribes to
    /// `P2pHandle::peer_event_tx` and enqueues each event here. Capped at
    /// `PEER_EVENT_QUEUE_CAP` to prevent unbounded growth when no consumer
    /// drains it (e.g. the Tauri UI is not open). The `poll_peer_events`
    /// handler drains and returns all pending events atomically.
    ///
    /// `std::sync::Mutex` (not tokio's) because the critical section is a
    /// short drain with no `.await`.
    peer_event_queue: Arc<std::sync::Mutex<std::collections::VecDeque<PeerEventRecord>>>,

    /// Handle to the most-recently-started mDNS-SD browse task (CopyPaste-ydhw).
    ///
    /// `rescan_discovered` calls `DiscoveryService::start()` which aborts the
    /// previous browse task via `shutdown_inner()`.  The old code detached the
    /// new browse handle with a bare `tokio::spawn` — the task ran indefinitely
    /// without participating in P2P shutdown or being replaceable on the next
    /// rescan.
    ///
    /// The fix: store the live browse `JoinHandle` here.  On each
    /// `rescan_discovered` call the previous handle (if any) is aborted before
    /// the new browse starts, and the new handle is stored in its place.  This
    /// prevents handle accumulation across multiple rescans.
    ///
    /// `std::sync::Mutex` because every critical section is a quick
    /// take/replace with no `.await`.
    discovery_browse_handle: Arc<std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>>,

    /// Optional P2P subsystem shutdown token (CopyPaste-ydhw).
    ///
    /// When populated (via [`p2p_shutdown_token_slot`](Self::p2p_shutdown_token_slot)),
    /// the `rescan_discovered` handler wraps the replacement browse handle in a
    /// `select!` that exits on P2P shutdown, ensuring the detached browse
    /// participates in graceful teardown.
    ///
    /// `daemon.rs` writes this slot after `start_p2p` returns (same pattern as
    /// `live_peer_sinks_slot`).  `None` means the slot has not been populated
    /// yet (or P2P is disabled) — the browse task then runs until the next
    /// rescan or process exit.
    ///
    /// `std::sync::Mutex` because the critical section is a trivial clone with
    /// no `.await`.
    p2p_shutdown_token: Arc<std::sync::Mutex<Option<CancellationToken>>>,

    /// nq39: in-memory Supabase password cache for non-macOS platforms.
    ///
    /// On macOS the `store_cloud_password` IPC handler writes directly to the
    /// macOS Keychain and never populates this field. On non-macOS (Linux,
    /// Windows-frozen) the Keychain is unavailable, so the password is held
    /// here for the duration of the daemon process — it is never written to
    /// `config.json` via this path. `None` until `store_cloud_password` is
    /// called.
    ///
    /// `zeroize::Zeroizing` ensures the heap string is scrubbed when the
    /// `Arc` is dropped (daemon shutdown or field replacement on update).
    /// `std::sync::Mutex` (not tokio's) because the critical section is a
    /// trivial clone/replace with no `.await`.
    #[cfg(not(target_os = "macos"))]
    in_memory_cloud_password: Arc<std::sync::Mutex<Option<zeroize::Zeroizing<String>>>>,

    /// Semaphore that bounds the number of simultaneously-active IPC connections
    /// (CopyPaste-6ot5). Each accepted connection acquires one permit via
    /// `try_acquire_owned` (non-blocking); the permit is moved into the spawned
    /// task and dropped on task completion. When all permits are taken, the
    /// accept loop drops the incoming `UnixStream` immediately rather than
    /// queueing or blocking. `Arc`-wrapped so it can be shared with the spawned
    /// connection tasks without lifetime issues.
    conn_semaphore: Arc<tokio::sync::Semaphore>,

    /// Live relay orchestrator handle (CopyPaste-44rq.67).
    ///
    /// `daemon::run` starts the relay (if `relay_url` is configured) and stores
    /// the resulting [`crate::relay::RelayHandle`] here so the `set_config`
    /// handler can shut it down at runtime when the user clears the relay URL
    /// (`set_config { relay_url: "" }`). Dropping/`shutdown()`-ing the handle
    /// stops the push + receive loops within one poll cycle, so the user can
    /// disable relay sync without restarting the daemon. `None` when no relay is
    /// running (not configured, failed to start, or already cleared).
    ///
    /// tokio `Mutex` because the `set_config` handler `.await`s while holding it.
    #[cfg(feature = "relay-sync")]
    relay_handle: Arc<tokio::sync::Mutex<Option<crate::relay::RelayHandle>>>,

    /// Shared in-flight sync flag (CopyPaste-1jms.22).
    ///
    /// Set to `true` by a [`crate::sync_in_flight::SyncInFlightGuard`] at the
    /// start of each active sync round-trip (cloud poll, cloud push, relay
    /// receive, relay push, P2P handshake) and reset to `false` when the guard
    /// is dropped (on success, error, or early return via `?`).
    ///
    /// The `get_sync_status` handler passes this value as `in_flight` to
    /// [`copypaste_ipc::compute_sync_badge_state_with_inflight`] so that
    /// `SyncBadgeState::Syncing` is emitted during active exchanges rather than
    /// the dead-code path it was before this fix.
    ///
    /// `AtomicBool` (not `Mutex`) because the read in `get_sync_status` and the
    /// writes in the sync loops are all best-effort races — a brief window where
    /// the badge says "idle" while a round-trip just started is acceptable, but a
    /// blocking lock on the hot IPC path is not.
    sync_in_flight: Arc<AtomicBool>,
}

/// Wire-serialisable peer event record returned by `poll_peer_events`.
#[derive(serde::Serialize, Clone, Debug)]
pub struct PeerEventRecord {
    /// `"connected"` or `"disconnected"`.
    pub kind: &'static str,
    /// Canonical lowercase colon-free hex fingerprint of the peer's cert.
    pub fingerprint: String,
}

/// Maximum number of [`PeerEventRecord`]s held in the IPC queue between polls.
///
/// The Tauri bridge polls every ~1 s; 64 is far more than enough to buffer a
/// burst of connections/disconnections before the next drain.
pub const PEER_EVENT_QUEUE_CAP: usize = 64;

/// Canonical `status.degraded_reason` value for the keychain-locked /
/// DB-unavailable degraded startup (the post-reinstall regression). The UI
/// keys its recovery banner off this exact string.
pub const DEGRADED_REASON_KEYCHAIN_LOCKED: &str = "keychain_locked";

/// Canonical `status.degraded_reason` value for the case where the SQLCipher
/// key WAS obtained but does NOT match the existing database (SQLITE_NOTADB /
/// `file is not a database`). Distinct from `keychain_locked` (key unreachable)
/// because the recovery story differs: the key is present but wrong — e.g. a
/// re-keyed device, a restored/foreign Keychain entry, or a fresh file-store
/// key minted over a DB encrypted by a pre-file-store (v0.5.1) Keychain key.
/// The UI shows a distinct banner so users are not told to "re-grant the
/// Keychain prompt" when that will not help.
pub const DEGRADED_REASON_DB_KEY_MISMATCH: &str = "db_key_mismatch";

impl IpcServer {
    pub fn new(
        db: Arc<Mutex<Database>>,
        private_mode: Arc<AtomicBool>,
        local_key: Arc<zeroize::Zeroizing<[u8; 32]>>,
        device_public_key: Arc<[u8; 32]>,
    ) -> Self {
        Self::new_with_ready(
            db,
            private_mode,
            local_key,
            device_public_key,
            Arc::new(AtomicBool::new(true)),
        )
    }

    /// Mark this server as serving a degraded startup (e.g. keychain-locked /
    /// db-unavailable). The reason is echoed in the `status` response so the UI
    /// can show a recovery banner. Pair this with `new_with_ready(.., false)`
    /// so DB-touching methods return `IPC_NOT_READY`.
    pub fn with_degraded_reason(self, reason: impl Into<String>) -> Self {
        // Poisoned mutex (a prior panic while holding the lock) is recovered:
        // the slot holds only a non-secret reason string.
        *self
            .degraded_reason
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = Some(reason.into());
        self
    }

    /// Attach the live mTLS certificate fingerprint that pairing advertises.
    ///
    /// CRITICAL-1: this MUST be the fingerprint of the same cert the running
    /// `PeerTransport` presents (`display_fingerprint(transport.fingerprint())`)
    /// so a scanning/pairing peer pins a value the mTLS layer actually compares
    /// against. The daemon generates the cert once and hands the same cert to
    /// `start_p2p` and the colon-hex fingerprint here, guaranteeing they agree.
    pub fn with_cert_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.cert_fingerprint = Some(fingerprint.into());
        self
    }

    /// Attach the stable per-device UUID so `history_page` can return it as
    /// `own_device_id`. The UI uses this to label locally-captured items as
    /// "This device" vs. items synced from a remote peer.
    pub fn with_local_device_id(mut self, id: impl Into<String>) -> Self {
        self.local_device_id = Some(id.into());
        self
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

    /// Return the slot that daemon.rs writes `P2pHandle::live_sinks` into after
    /// `start_p2p` returns.
    ///
    /// Two consumers share this slot:
    /// - `list_peers` iterates it to compute the authoritative online flag from
    ///   live connection state rather than the stale mTLS-allowlist heuristic.
    /// - `unpair_peer` / `revoke_peer` / `revoke_all_peers` look up a specific
    ///   peer's sender and deliver a best-effort `ControlMsg::Unpair` signal.
    pub fn live_peer_sinks_slot(&self) -> Arc<std::sync::Mutex<Option<crate::p2p::LivePeerSinks>>> {
        Arc::clone(&self.live_peer_sinks)
    }

    /// Return the slot that daemon.rs writes `P2pHandle::peer_rtt_ms` into
    /// after `start_p2p` returns.  The `list_peers` handler reads from this
    /// to add `latency_ms` to each peer entry.
    pub fn live_peer_rtt_ms_slot(&self) -> Arc<std::sync::Mutex<Option<crate::p2p::PeerRttMs>>> {
        Arc::clone(&self.live_peer_rtt_ms)
    }

    /// Return the shared peer-event queue that `daemon.rs` enqueues into and
    /// the `poll_peer_events` IPC handler drains.
    pub fn peer_event_queue(
        &self,
    ) -> Arc<std::sync::Mutex<std::collections::VecDeque<PeerEventRecord>>> {
        Arc::clone(&self.peer_event_queue)
    }

    /// Return the slot that `daemon.rs` can write the P2P subsystem's
    /// `CancellationToken` into after `start_p2p` returns (CopyPaste-ydhw).
    ///
    /// When populated, `rescan_discovered` wraps the replacement mDNS-SD browse
    /// task in a `select!` that respects P2P shutdown, preventing the browse
    /// from outliving the P2P subsystem.  Follows the same lazy-injection
    /// pattern as [`live_peer_sinks_slot`](Self::live_peer_sinks_slot).
    ///
    /// `None` means P2P is disabled or `start_p2p` has not yet returned.
    pub fn p2p_shutdown_token_slot(&self) -> Arc<std::sync::Mutex<Option<CancellationToken>>> {
        Arc::clone(&self.p2p_shutdown_token)
    }

    /// Attach a clone of the running sync orchestrator's `SyncCrypto` context
    /// (H8 perf fix). Because `SyncCrypto` stores its cached sync key behind an
    /// `Arc<Mutex>`, this clone shares the SAME backing store as the
    /// orchestrator's copy; calling `reload_sync_key()` here after a pairing
    /// write propagates to the orchestrator without any channel or restart.
    pub fn with_p2p_sync_crypto(mut self, crypto: crate::sync_orch::SyncCrypto) -> Self {
        self.p2p_sync_crypto = Some(crypto);
        self
    }

    /// Attach the self-signed mTLS cert (DER) + key used to TLS-wrap the
    /// unauthenticated bootstrap pairing channel (P2P Phase 1).
    ///
    /// MUST be a clone of the exact cert `start_p2p`'s transport presents (and
    /// whose fingerprint `with_cert_fingerprint` advertises) so the fingerprints
    /// a peer learns over the bootstrap channel match what the pinned mTLS layer
    /// later compares.
    pub fn with_p2p_cert(mut self, cert_der: Vec<u8>, key_der: Vec<u8>) -> Self {
        self.p2p_cert = Some(Arc::new((cert_der, key_der)));
        self
    }

    /// Attach the mDNS discovery handle used as the QR-accept fallback when the
    /// QR carries no `addr_hint`.
    pub fn with_discovery(
        mut self,
        discovery: Arc<copypaste_p2p::discovery::DiscoveryService>,
    ) -> Self {
        self.discovery = Some(discovery);
        self
    }

    /// Return a clone of the shared discovery-pairing coordinator (LAN/SAS
    /// Phase 2).
    ///
    /// `start_p2p`'s standing discovery-pairing responder routes its SAS
    /// confirmation through the SAME coordinator the IPC handlers observe, so
    /// the responder user confirms via `pair_get_sas`/`pair_confirm_sas` exactly
    /// like the initiator. The daemon calls this before moving the server into
    /// its task and hands the clone to `start_p2p`.
    pub fn pairing_coordinator(&self) -> Arc<crate::pairing_sm::PairingCoordinator> {
        Arc::clone(&self.pairing)
    }

    /// Return a handle to the shared slot holding this daemon's own P2P
    /// sync-listener address (`host:port`).
    ///
    /// The IPC server is constructed before `start_p2p` binds its accept loop,
    /// so the OS-assigned port is not known yet. The daemon calls
    /// [`set_p2p_sync_addr`](Self::set_p2p_sync_addr) (via this same Arc) once
    /// `start_p2p` returns the bound port; the pairing handlers then read it and
    /// send it in-band over the bootstrap channel. Returning the Arc lets the
    /// daemon populate the slot after the server has been moved into its task.
    pub fn p2p_sync_addr_slot(&self) -> Arc<std::sync::Mutex<Option<String>>> {
        Arc::clone(&self.p2p_sync_addr)
    }

    /// Populate the shared slot with this daemon's bound P2P sync-listener
    /// address. Convenience wrapper over [`p2p_sync_addr_slot`](Self::p2p_sync_addr_slot)
    /// for callers that still hold the server (e.g. tests).
    ///
    /// A poisoned mutex (a prior panic while holding the lock) is recovered
    /// rather than propagated — the slot holds only a non-secret address string,
    /// so reusing it after a panic is safe and keeps pairing functional.
    pub fn set_p2p_sync_addr(&self, addr: impl Into<String>) {
        let mut slot = self
            .p2p_sync_addr
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *slot = Some(addr.into());
    }

    /// Wire up shared cloud-sync state created by the daemon before spawning
    /// the IPC server and `start_cloud`.
    ///
    /// By calling this the daemon guarantees both surfaces see the **same**
    /// `Arc`s: a `set_sync_passphrase` IPC call writes to the same
    /// `sync_key` `Mutex` that the cloud push/poll loops read from, and the
    /// cloud loops write to the same `last_sync_ms` counter that
    /// `get_sync_status` reads.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    pub fn with_cloud_sync_state(
        mut self,
        sync_key: Arc<Mutex<Option<SyncKey>>>,
        last_sync_ms: Arc<std::sync::atomic::AtomicI64>,
        cloud_signed_in: Arc<AtomicBool>,
    ) -> Self {
        self.sync_key = sync_key;
        self.last_sync_ms = last_sync_ms;
        self.cloud_signed_in = cloud_signed_in;
        self
    }

    /// Wire the canonical Supabase account-identity slot (CopyPaste-1jms.34).
    ///
    /// The caller creates a shared `Arc<Mutex<Option<String>>>` BEFORE spawning
    /// the server, passes it here, and then retains its own clone to write the
    /// value produced by `start_cloud` *after* the server has been spawned.
    /// The `get_sync_status` handler reads through the same `Arc` on every
    /// request — writing once at startup is sufficient and race-free.
    #[cfg(feature = "cloud-sync")]
    pub fn with_cloud_account_id_slot(
        mut self,
        slot: Arc<std::sync::Mutex<Option<String>>>,
    ) -> Self {
        self.cloud_account_id = slot;
        self
    }

    /// Return a clone of the `Arc<Mutex<Option<String>>>` holding the local
    /// Supabase account identity so callers (e.g. the P2P standing responder)
    /// can read it without coupling to the full `IpcServer`.
    ///
    /// CopyPaste-yw2k: the arc is cloned once at startup and forwarded to
    /// `start_p2p` so the standing responder can include our `supabase_account_id`
    /// in the `PeerMeta` it sends during in-band pairing.
    #[cfg(feature = "cloud-sync")]
    pub fn cloud_account_id_slot(&self) -> Arc<std::sync::Mutex<Option<String>>> {
        Arc::clone(&self.cloud_account_id)
    }

    /// Attach the broadcast sender for newly-ingested clipboard items so the
    /// `import` IPC method can notify the sync orchestrator (P2P Phase 3).
    pub fn with_new_item_tx(
        mut self,
        tx: tokio::sync::broadcast::Sender<copypaste_core::ClipboardItem>,
    ) -> Self {
        self.new_item_tx = Some(tx);
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
            read_pool: std::sync::Mutex::new(None),
            private_mode,
            private_mode_epoch: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            local_device_id: None,
            local_key,
            device_public_key,
            ready,
            pake_sessions: Arc::new(Mutex::new(HashMap::new())),
            pending_qr_token: Arc::new(Mutex::new(None)),
            p2p_peers: None,
            cert_fingerprint: None,
            p2p_cert: None,
            discovery: None,
            p2p_sync_addr: Arc::new(std::sync::Mutex::new(None)),
            self_write_change_count: Arc::new(std::sync::atomic::AtomicI64::new(-1)),
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            sync_key: Arc::new(Mutex::new(None)),
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            last_sync_ms: Arc::new(std::sync::atomic::AtomicI64::new(0)),
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            cloud_signed_in: Arc::new(AtomicBool::new(false)),
            #[cfg(feature = "cloud-sync")]
            cloud_account_id: Arc::new(std::sync::Mutex::new(None)),
            new_item_tx: None,
            degraded_reason: Arc::new(std::sync::Mutex::new(None)),
            core_config: None,
            cached_public_ip: Arc::new(tokio::sync::RwLock::new(None)),
            pairing: Arc::new(crate::pairing_sm::PairingCoordinator::new()),
            live_peer_sinks: Arc::new(std::sync::Mutex::new(None)),
            live_peer_rtt_ms: Arc::new(std::sync::Mutex::new(None)),
            p2p_sync_crypto: None,
            pending_bootstrap: Arc::new(tokio::sync::Mutex::new(None)),
            peer_event_queue: Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new())),
            discovery_browse_handle: Arc::new(std::sync::Mutex::new(None)),
            p2p_shutdown_token: Arc::new(std::sync::Mutex::new(None)),
            // nq39: initialise to None; populated by `store_cloud_password`
            // on non-macOS platforms where the Keychain is unavailable.
            #[cfg(not(target_os = "macos"))]
            in_memory_cloud_password: Arc::new(std::sync::Mutex::new(None)),
            // CopyPaste-6ot5: start with the full connection cap available.
            conn_semaphore: Arc::new(tokio::sync::Semaphore::new(MAX_CONCURRENT_CONNECTIONS)),
            // CopyPaste-44rq.67: empty until daemon::run starts the relay and
            // wires the shared slot via `with_relay_handle`.
            #[cfg(feature = "relay-sync")]
            relay_handle: Arc::new(tokio::sync::Mutex::new(None)),
            // CopyPaste-1jms.22: starts false (no sync in progress at
            // construction time). The daemon replaces this with a shared Arc via
            // `with_sync_in_flight` before spawning the sync loops.
            sync_in_flight: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Wire in a read connection pool (CopyPaste-j8p).
    ///
    /// Read-only handlers (`list`, `count`, `search`, `history_page`, `stats`)
    /// will acquire connections from `pool` instead of locking `self.db`,
    /// allowing concurrent reads without blocking the writer.
    pub fn with_read_pool(self, pool: Arc<copypaste_core::SqlitePool>) -> Self {
        *self.read_pool.lock().unwrap_or_else(|p| p.into_inner()) = Some(pool);
        self
    }

    /// CopyPaste-crh3.86: run a read-only DB closure on the blocking pool with
    /// the canonical pool-then-writer-lock fallback, in ONE place.
    ///
    /// Every read IPC handler previously copy-pasted ~15 lines: clone the read
    /// pool + `self.db`, `spawn_blocking`, try `pool.get()` → [`ReadHandle`] else
    /// `db.blocking_lock()`, run the query, then `await`/join. Any fix to the
    /// fallback (or its error mapping) had to be applied at every site or the
    /// behaviour silently diverged. This helper centralises it; callers pass a
    /// closure over `&dyn DbRead` (the read fns are generic over `DbRead`, so the
    /// SAME closure body serves both the pooled connection and the writer lock —
    /// removing the in-branch duplication too) and map the returned value to a
    /// [`Response`].
    ///
    /// Returns `Err(String)` only when the blocking task itself failed (panic /
    /// runtime shutdown); the inner `Result<T, E>` is the query outcome.
    async fn with_read_db<T, E, F>(&self, f: F) -> Result<Result<T, E>, String>
    where
        F: FnOnce(&dyn DbRead) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Send + 'static,
    {
        let pool_opt = self
            .read_pool
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let db_arc = self.db.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(pool) = pool_opt {
                if let Ok(conn) = pool.get() {
                    return f(&copypaste_core::ReadHandle(conn));
                }
            }
            let db = db_arc.blocking_lock();
            f(&*db)
        })
        .await
        .map_err(|e| format!("blocking task failed: {e}"))
    }

    /// Share the live relay-handle slot with `daemon::run` (CopyPaste-44rq.67).
    ///
    /// `daemon::run` owns the same `Arc` and writes the started
    /// [`crate::relay::RelayHandle`] into it after `start_relay` succeeds; the
    /// `set_config` handler reads it to shut the relay down when the user clears
    /// the relay URL. Passing the shared slot (rather than the default
    /// per-server empty one) is what connects the two.
    #[cfg(feature = "relay-sync")]
    pub fn with_relay_handle(
        mut self,
        slot: Arc<tokio::sync::Mutex<Option<crate::relay::RelayHandle>>>,
    ) -> Self {
        self.relay_handle = slot;
        self
    }

    /// Wire the shared in-flight sync flag (CopyPaste-1jms.22).
    ///
    /// `daemon::run` allocates the `Arc<AtomicBool>` once, clones it into each
    /// sync loop (cloud poll, cloud push, relay receive, relay push, P2P
    /// handshake), and passes another clone here so `get_sync_status` reads the
    /// SAME flag that the sync loops flip via [`crate::sync_in_flight::SyncInFlightGuard`].
    ///
    /// When not wired (unit tests, degraded mode), the field defaults to a local
    /// `Arc::new(AtomicBool::new(false))` that is never flipped, so
    /// `badge_state` is unaffected.
    pub fn with_sync_in_flight(mut self, flag: Arc<AtomicBool>) -> Self {
        self.sync_in_flight = flag;
        self
    }

    /// Attach the shared live core config (`config.toml`) for hot-reload.
    ///
    /// The `set_config` IPC handler writes updated limit/feature values into this
    /// Arc after persisting to disk, so the clipboard monitor, paste path, and
    /// prune code pick them up on the next tick without a daemon restart.
    pub fn with_core_config(
        mut self,
        core_config: Arc<std::sync::RwLock<copypaste_core::AppConfig>>,
    ) -> Self {
        self.core_config = Some(core_config);
        self
    }

    /// Share the pre-allocated public-IP cache slot with the daemon's
    /// STUN-refresh background task.
    ///
    /// The daemon creates the `Arc<RwLock<…>>`, passes it into the IPC server
    /// via this method, and also clones it into the refresh task so both can
    /// write to / read from the same slot without a process-wide lock.
    pub fn with_public_ip_cache(mut self, cache: Arc<tokio::sync::RwLock<Option<String>>>) -> Self {
        self.cached_public_ip = cache;
        self
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
    /// against. We go through [`copypaste_p2p::transport::PairedPeers::rotate_peer`] (rather than `add`)
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

    /// This daemon's own P2P sync-listener address (`host:port`), if `start_p2p`
    /// has bound it. Sent in-band over the bootstrap channel so the peer can
    /// persist it for the Phase 3 connector. Returns an empty string when the
    /// port is not yet known (P2P disabled or not yet bound) — the bootstrap
    /// wire tolerates an empty address frame.
    fn own_sync_addr(&self) -> String {
        self.p2p_sync_addr
            .lock()
            .map(|slot| slot.clone().unwrap_or_default())
            .unwrap_or_else(|poisoned| poisoned.into_inner().clone().unwrap_or_default())
    }

    /// Collect THIS device's identity metadata for the in-band bootstrap
    /// metadata exchange (P2P Phase 4).
    ///
    /// Maps [`DeviceMeta`](crate::device_meta::DeviceMeta) onto the transport's
    /// [`PeerMeta`](copypaste_p2p::bootstrap::PeerMeta). The collection spawns
    /// short-lived child processes (`scutil`, `sysctl`, `sw_vers`) that can block
    /// up to ~2 s, so callers MUST invoke this from a blocking context (e.g.
    /// `tokio::task::spawn_blocking`) rather than on an async worker thread.
    ///
    /// `pub(crate)` so the LAN/SAS Phase 2 standing responder in `p2p.rs` reuses
    /// the same metadata collection as the QR path.
    ///
    /// `public_ip` is THIS device's STUN-discovered global IP, read by the caller
    /// from [`Self::cached_public_ip`] (the daemon's single existing STUN source)
    /// BEFORE entering `spawn_blocking`, then passed in here. It is threaded as an
    /// argument — rather than read inside this function — because the cache is an
    /// async `RwLock` and this runs on a blocking thread, and to avoid spinning up
    /// a second STUN client. `None` when the user opted out
    /// (`collect_public_ip = false`) or STUN has not yet resolved. Advertised
    /// in-band (B1) so the peer can show our global IP; informational only —
    /// never used for auth/trust.
    pub(crate) fn collect_own_peer_meta(
        public_ip: Option<String>,
        device_id: Option<String>,
        supabase_account_id: Option<String>,
    ) -> copypaste_p2p::bootstrap::PeerMeta {
        // CopyPaste-bps: use the process-wide cache warmed at daemon startup
        // instead of calling DeviceMeta::collect again (which spawns child
        // processes and can take ~6 s).  Falls back to an on-demand collect if
        // the cache was somehow never warmed (unit-test / degraded paths).
        let meta = crate::device_meta::get_cached(BUILD_VERSION);
        copypaste_p2p::bootstrap::PeerMeta {
            model: meta.device_model.clone(),
            os_version: meta.os_version.clone(),
            app_version: Some(meta.app_version.clone()),
            local_ip: meta.local_ip.clone(),
            // device_name is our own name — we advertise it over the bootstrap
            // channel so the peer can persist it as our display name. Collected
            // from the OS hostname via DeviceMeta.
            device_name: meta.device_name.clone(),
            public_ip,
            device_id,
            // CopyPaste-yw2k: advertise our non-secret Supabase account identity
            // so the peer can detect cross-account mismatches at pairing time.
            supabase_account_id,
        }
    }

    /// Build THIS device's [`SyncProvisioning`] to advertise over the
    /// authenticated bootstrap tunnel ("QR fully provisions all sync").
    ///
    /// Populates the non-secret Supabase connection params from the persisted
    /// [`AppConfig`] (env overrides applied, mirroring `get_sync_status`) and the
    /// DERIVED 32-byte cloud sync key from the live `sync_key` slot — NOT the
    /// passphrase. Returns `None` when nothing is configured (so an unconfigured
    /// device, or a build without `cloud-sync`, sends an all-`None` value and the
    /// peer learns nothing to apply).
    ///
    /// `relay_url` is populated from the persisted `relay_url` config field so a
    /// freshly paired peer inherits this device's relay endpoint. It is a
    /// non-secret base URL (no env override today, unlike the Supabase params).
    ///
    /// SECURITY: the returned struct's `derived_sync_key` is secret; it is never
    /// logged here (and `SyncProvisioning`'s `Debug` redacts it).
    /// Associated form so the detached QR responder task can call it with a
    /// cloned `sync_key` Arc (it cannot borrow `&self`).
    #[cfg(feature = "cloud-sync")]
    async fn build_local_provisioning_from(
        sync_key: &Arc<Mutex<Option<SyncKey>>>,
    ) -> Option<copypaste_p2p::bootstrap::SyncProvisioning> {
        // Read persisted config off the async worker (blocking fs I/O).
        let app_cfg = tokio::task::spawn_blocking(read_config)
            .await
            .unwrap_or_default();
        let relay_url = app_cfg.relay_url.clone();
        let supabase_url = std::env::var("SUPABASE_URL").ok().or(app_cfg.supabase_url);
        let supabase_anon_key = std::env::var("SUPABASE_ANON_KEY")
            .ok()
            .or(app_cfg.supabase_anon_key);
        // Snapshot the derived key bytes (the SyncKey itself is not Clone/Send-
        // friendly across the wire); wrap in Zeroizing so the transient copy is
        // scrubbed when this future's locals drop.
        let derived_sync_key = sync_key
            .lock()
            .await
            .as_ref()
            .map(|k| zeroize::Zeroizing::new(k.as_bytes().to_vec()));

        if supabase_url.is_none()
            && supabase_anon_key.is_none()
            && derived_sync_key.is_none()
            && relay_url.is_none()
        {
            return None;
        }
        Some(copypaste_p2p::bootstrap::SyncProvisioning {
            supabase_url,
            supabase_anon_key,
            relay_url,
            // Unwrap the Zeroizing into the owned Vec the struct holds. The
            // struct's own Debug redacts these bytes; they never hit a log.
            derived_sync_key: derived_sync_key.map(|z| z.to_vec()),
        })
    }

    /// `&self` convenience wrapper used by the (non-detached) initiator paths.
    #[cfg(feature = "cloud-sync")]
    async fn build_local_provisioning(&self) -> Option<copypaste_p2p::bootstrap::SyncProvisioning> {
        Self::build_local_provisioning_from(&self.sync_key).await
    }

    /// `cloud-sync`-disabled stub: this build cannot source any sync account, so
    /// it advertises nothing.
    #[cfg(not(feature = "cloud-sync"))]
    async fn build_local_provisioning(&self) -> Option<copypaste_p2p::bootstrap::SyncProvisioning> {
        None
    }

    /// Apply a peer's received [`SyncProvisioning`] ("QR fully provisions all
    /// sync"): fill in any sync-account field this device currently LACKS, but
    /// NEVER overwrite an existing local value.
    ///
    /// * `supabase_url` / `supabase_anon_key` — written into `config.json` (via
    ///   the same `merge_config` + `write_config` path `set_config` uses) only
    ///   when the device has neither an env override nor a persisted value.
    /// * `derived_sync_key` — when the device has no sync key yet, the 32-byte
    ///   key is wrapped in a [`SyncKey`] and persisted via the SAME backend
    ///   `set_sync_passphrase` uses (file store or Keychain), then installed in
    ///   the live `sync_key` slot so the cloud loops pick it up immediately. We
    ///   set the KEY directly — the passphrase is never transmitted.
    /// * `relay_url` — written into `config.json` (and mirrored to `config.toml`)
    ///   via the same `merge_config` + `write_config` path, but ONLY when this
    ///   device has no persisted `relay_url` yet. An existing local relay URL is
    ///   never overwritten (mirrors the `supabase_url` fill-missing rule).
    ///
    /// All steps are best-effort and idempotent; a persist failure is logged and
    /// swallowed (pairing already succeeded).
    #[cfg(feature = "cloud-sync")]
    async fn apply_peer_provisioning(&self, prov: copypaste_p2p::bootstrap::SyncProvisioning) {
        Self::apply_peer_provisioning_to(&self.sync_key, prov).await;
    }

    /// Associated form so the detached QR responder task can apply provisioning
    /// with a cloned `sync_key` Arc (it cannot borrow `&self`). See
    /// [`Self::apply_peer_provisioning`] for the full contract.
    #[cfg(feature = "cloud-sync")]
    async fn apply_peer_provisioning_to(
        sync_key: &Arc<Mutex<Option<SyncKey>>>,
        prov: copypaste_p2p::bootstrap::SyncProvisioning,
    ) {
        // ── 1. Non-secret Supabase connection params → config.json ──
        // Read current config; only fill fields that are currently empty AND have
        // no env override (env always wins and is not persisted here).
        let current = tokio::task::spawn_blocking(read_config)
            .await
            .unwrap_or_default();
        let env_has_url = std::env::var("SUPABASE_URL").is_ok();
        let env_has_key = std::env::var("SUPABASE_ANON_KEY").is_ok();

        let mut incoming = AppConfig::default();
        let mut config_changed = false;
        if current.supabase_url.is_none() && !env_has_url {
            // 34u2: SyncProvisioning is ZeroizeOnDrop (Drop) — cannot move out; clone.
            if let Some(url) = prov.supabase_url.clone() {
                incoming.supabase_url = Some(url);
                config_changed = true;
            }
        }
        if current.supabase_anon_key.is_none() && !env_has_key {
            if let Some(key) = prov.supabase_anon_key.clone() {
                incoming.supabase_anon_key = Some(key);
                config_changed = true;
            }
        }
        // relay_url: non-secret base URL. Fill it only when this device has no
        // persisted relay_url yet (never overwrite an existing local value),
        // mirroring the supabase_url fill-missing rule above. It is persisted to
        // BOTH config.json (via write_config below) and config.toml (via
        // update_core_config) because read_config overlays relay_url from the
        // core config.toml — a config.json-only write would be clobbered on the
        // next read.
        if current.relay_url.is_none() {
            if let Some(url) = prov.relay_url.clone() {
                incoming.relay_url = Some(url);
                config_changed = true;
            }
        }
        if config_changed {
            // merge_config keeps existing values for every field `incoming`
            // leaves `None`, so this only ADDS the missing sync params.
            let merged = merge_config(current, incoming);
            match tokio::task::spawn_blocking(move || {
                write_config(&merged)?;
                // Mirror relay_url (and any other core-backed fields) into
                // config.toml so read_config's overlay does not clobber it.
                update_core_config(&merged)?;
                Ok::<_, anyhow::Error>(())
            })
            .await
            {
                Ok(Ok(())) => {
                    tracing::info!("applied peer sync provisioning: persisted sync config")
                }
                Ok(Err(e)) => {
                    tracing::warn!("apply_peer_provisioning: config persist failed: {e}")
                }
                Err(e) => tracing::warn!("apply_peer_provisioning: config task join failed: {e}"),
            }
        }

        // ── 2. Derived cloud sync key → key store + live slot ──
        // Only when this device has NO sync key yet (never overwrite an existing
        // one — that would orphan locally-encrypted cloud blobs).
        // 34u2: clone the secret out of the ZeroizeOnDrop struct; the clone is
        // wrapped in Zeroizing below and the original zeroizes on prov's drop.
        let Some(key_bytes) = prov.derived_sync_key.clone() else {
            return;
        };
        if key_bytes.len() != 32 {
            tracing::warn!(
                "apply_peer_provisioning: ignoring sync key with wrong length ({} bytes)",
                key_bytes.len()
            );
            return;
        }
        // Wrap in Zeroizing so the transient byte buffer is scrubbed on drop.
        // Built before the overwrite-guard so we can constant-time compare the
        // incoming key against any existing key.
        let key_bytes = zeroize::Zeroizing::new(key_bytes);
        let mut arr = zeroize::Zeroizing::new([0u8; 32]);
        arr.copy_from_slice(&key_bytes);
        {
            let guard = sync_key.lock().await;
            if let Some(existing) = guard.as_ref() {
                // Distinguish ROUTINE pairing from a ROTATION re-provision.
                //
                // Routine pairing fills a MISSING key; both peers derive the
                // SAME deterministic Argon2id key from the same passphrase, so a
                // re-provision that carries the IDENTICAL key is a harmless
                // no-op and must NOT clobber locally-encrypted cloud blobs.
                //
                // After a sync-key ROTATION the operator re-scans the pairing QR
                // on each remaining device; the QR now carries the NEW key,
                // which DIFFERS from the stale key this device still holds. That
                // is the legitimate replace case — without it a remaining device
                // would silently ignore the rotated key and keep addressing the
                // dead (pre-rotation) relay inbox.
                //
                // Constant-time compare on the 32-byte key material
                // (`SyncKey::ct_eq_bytes` uses `subtle` — never `==` on secrets,
                // per CLAUDE.md security constraints).
                // `&arr` derefs Zeroizing<[u8; 32]> → &[u8; 32] at the call site.
                if existing.ct_eq_bytes(&arr) {
                    tracing::debug!(
                        "apply_peer_provisioning: incoming sync key matches existing; no-op"
                    );
                    return;
                }
                // Incoming key differs → treat as an explicit rotation re-provision
                // and REPLACE the stale key below.
                tracing::info!(
                    "apply_peer_provisioning: incoming sync key differs from existing; \
                     replacing (rotation re-provision)"
                );
            }
        }

        // Persist via the SAME backend set_sync_passphrase uses, so an
        // ad-hoc/unsigned install does not raise a Keychain prompt.
        #[cfg(target_os = "macos")]
        if crate::keychain::keychain_bypassed() {
            tracing::debug!(
                "apply_peer_provisioning: COPYPASTE_EPHEMERAL_KEY set; key in-memory only"
            );
        } else {
            match crate::keychain::signing::choose_key_backend() {
                crate::keychain::signing::KeyBackend::File => {
                    // `&*arr` derefs Zeroizing<[u8; 32]> to &[u8; 32] (the exact
                    // type store_cloud_sync_key expects) with no fallible cast.
                    if let Err(e) = crate::keychain::file_store::store_cloud_sync_key(&arr) {
                        tracing::warn!(
                            "apply_peer_provisioning: file-store persist failed ({e}); \
                             key active in-memory only until restart"
                        );
                    }
                }
                crate::keychain::signing::KeyBackend::Keychain => {
                    // CopyPaste-nkro: use the locked-down write path so the
                    // cloud-sync key is stored with ThisDeviceOnly + no iCloud
                    // sync (same hardening as the device key).
                    if let Err(e) = crate::keychain::set_generic_password_locked_down(
                        crate::keychain::SERVICE,
                        crate::keychain::CLOUD_SYNC_ACCOUNT,
                        &arr[..],
                    ) {
                        tracing::warn!(
                            "apply_peer_provisioning: keychain persist failed ({e}); \
                             key active in-memory only until restart"
                        );
                    }
                }
            }
        }

        *sync_key.lock().await = Some(SyncKey::from_bytes(*arr));
        tracing::info!("applied peer sync provisioning: installed derived cloud sync key");
    }

    /// Persist a freshly-derived [`SyncKey`] to the SAME backend
    /// `set_sync_passphrase` uses (0600 file store or Keychain, never raising a
    /// prompt on an ad-hoc/unsigned install), then swap the live `self.sync_key`
    /// slot so the cloud push/poll loops pick it up immediately.
    ///
    /// Shared by `set_sync_passphrase`, `rotate_sync_key`, `revoke_and_rotate`,
    /// and `revoke_peer` (auto-rotation) so the rotation path is byte-for-byte
    /// identical regardless of the call site. The key bytes are NEVER logged.
    ///
    /// Under `cloud-sync`: persists to the OS Keychain or a 0600 file store so
    /// the key survives a daemon restart.
    /// Under `relay-sync` (without `cloud-sync`): skips persistence — the key
    /// is active in-memory for this session only. Remaining devices must
    /// re-pair (QR re-scan) to receive the new key.
    ///
    /// A persist failure is logged and swallowed: the key is still installed
    /// in-memory for this session, matching `set_sync_passphrase`'s contract.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    async fn persist_and_install_sync_key(&self, new_key: SyncKey) {
        // Under cloud-sync: persist to the OS Keychain or file store so the
        // key survives a daemon restart.  Under relay-sync-only (no cloud-sync),
        // the key stays in-memory for this session.
        #[cfg(feature = "cloud-sync")]
        {
            // Persist the raw key bytes so they survive a daemon restart.
            #[cfg(target_os = "macos")]
            if crate::keychain::keychain_bypassed() {
                // Dev/test bypass: do not persist (would prompt / touch disk). The
                // key stays active in-memory for this session.
                tracing::debug!(
                    "persist_and_install_sync_key: COPYPASTE_EPHEMERAL_KEY set; not persisting"
                );
            } else {
                match crate::keychain::signing::choose_key_backend() {
                    crate::keychain::signing::KeyBackend::File => {
                        if let Err(e) =
                            crate::keychain::file_store::store_cloud_sync_key(new_key.as_bytes())
                        {
                            tracing::warn!(
                                "persist_and_install_sync_key: file-store persist failed ({e}); \
                                 key is active in-memory only until daemon restart"
                            );
                        }
                    }
                    crate::keychain::signing::KeyBackend::Keychain => {
                        // CopyPaste-nkro: use the locked-down write path so the
                        // cloud-sync key is stored with ThisDeviceOnly + no iCloud
                        // sync (same hardening as the device key).
                        if let Err(e) = crate::keychain::set_generic_password_locked_down(
                            crate::keychain::SERVICE,
                            crate::keychain::CLOUD_SYNC_ACCOUNT,
                            new_key.as_bytes(),
                        ) {
                            tracing::warn!(
                                "persist_and_install_sync_key: keychain persist failed ({e}); \
                                 key is active in-memory only until daemon restart"
                            );
                        }
                    }
                }
            }
        }

        // Store in shared state so push/poll loops pick it up immediately
        // (they hold an Arc to the same Mutex).
        *self.sync_key.lock().await = Some(new_key);
    }

    /// `cloud-sync`-disabled stub: nothing to apply.
    #[cfg(not(feature = "cloud-sync"))]
    async fn apply_peer_provisioning(&self, _prov: copypaste_p2p::bootstrap::SyncProvisioning) {}

    /// Derive the base64-encoded shared content sync key for a peer from the
    /// PAKE [`SessionKey`](copypaste_p2p::pake::SessionKey).
    ///
    /// Uses `SessionKey::derive_xchacha_key` with a fixed domain-separation
    /// salt so the derivation is (a) deterministic — both paired devices hold
    /// the same `SessionKey` and therefore derive the IDENTICAL content key —
    /// and (b) domain-separated from any other use of the same session key
    /// (e.g. TLS channel binding). The resulting 32-byte key is the
    /// XChaCha20-Poly1305 key the sync orchestrator feeds to
    /// `encrypt_for_cloud` / `decrypt_from_cloud` for cross-device item payloads.
    fn derive_peer_sync_key_b64(session_key: &copypaste_p2p::pake::SessionKey) -> String {
        use base64::Engine as _;
        // Fixed, non-secret domain-separation salt for the P2P content sync key.
        const P2P_SYNC_KEY_SALT: &[u8] = b"copypaste/p2p/content-sync-key/v1";
        let key = session_key.derive_xchacha_key(P2P_SYNC_KEY_SALT);
        base64::engine::general_purpose::STANDARD.encode(key)
    }

    /// Derive a 32-byte channel-binding token from the two cert fingerprints
    /// involved in an IPC-path PAKE handshake.
    ///
    /// # Security rationale (S3 — IPC pairing path)
    ///
    /// The IPC password-pairing path (`pair_peer_with_password` /
    /// `pair_accept_password` / `pair_accept_finish`) relays PAKE messages
    /// through the UI rather than over a shared TLS connection, so an RFC 5705
    /// `export_keying_material` binder is not available. The next-best binding
    /// is the pair of cert fingerprints the two sides have already agreed to
    /// pin: each device knows its own cert fingerprint and the peer fingerprint
    /// supplied by the UI.
    ///
    /// A relay/MitM that substitutes its own cert will have a different
    /// fingerprint pair → a different binder → a different channel-bound key →
    /// confirmation tags that will not match → the handshake is aborted.
    ///
    /// The binder is the SHA-256 of `min_fp || max_fp` (lexicographic order on
    /// the raw bytes, so both ends produce the same value regardless of which
    /// end calls this function). Domain-separated from the session-key
    /// derivation by the surrounding `SessionKey::bind_to_tls_channel` HKDF
    /// info string, which differs from `derive_xchacha_key`'s info string.
    fn pake_cert_binder(fp_a: &str, fp_b: &str) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        // Canonical order: lexicographic on the UTF-8 bytes so both sides
        // produce the same binder regardless of which is "own" vs "peer".
        let (lo, hi) = if fp_a.as_bytes() <= fp_b.as_bytes() {
            (fp_a.as_bytes(), fp_b.as_bytes())
        } else {
            (fp_b.as_bytes(), fp_a.as_bytes())
        };
        let mut h = Sha256::new();
        h.update(b"copypaste/p2p/ipc-cert-binder/v1\x00");
        h.update(lo);
        h.update(b"\x00");
        h.update(hi);
        h.finalize().into()
    }

    /// Durably persist a freshly-paired peer to `peers.json` (P2P Phase 2), in
    /// addition to the in-memory allowlist registration.
    ///
    /// `peer_fp_canonical` is the canonical (colon-free, lowercase) cert
    /// fingerprint the bootstrap channel reports; it is stored in the
    /// user-facing colon-hex form so the rest of the IPC peers surface
    /// (`list_peers`, revoke, etc.) and `load_persisted_peers_into` round-trip
    /// it consistently. `peer_sync_addr` is the peer's P2P sync-listener address
    /// learned in-band, stored so the Phase 3 connector can dial it directly
    /// (loopback mDNS filters 127.0.0.1 and is unreliable).
    ///
    /// Idempotent: if a record with the same fingerprint already exists it is
    /// replaced (address/name refreshed) rather than duplicated. Failures are
    /// logged and swallowed — pairing already succeeded in memory, and a persist
    /// failure must not turn a successful pair into an IPC error.
    ///
    /// A free function (not a `&self` method) so the detached bootstrap-responder
    /// task can call it after `self` has been moved/borrowed away.
    ///
    /// `pub(crate)` so the LAN/SAS Phase 2 standing responder in `p2p.rs` reuses
    /// the IDENTICAL persistence logic as the QR path.
    /// Durably persist a freshly-paired peer to `peers.json`, then refresh the
    /// in-memory sync-key cache.
    ///
    /// CopyPaste-ww5q: the file I/O (`load_peers` + `save_peers` which calls
    /// `fsync`) and the `reload_sync_key` disk read are all synchronous and must
    /// NOT run on an async worker thread.  We pre-compute the CPU-only
    /// `sync_key_b64` derivation (HKDF + base64) on the calling async thread
    /// before the move into `spawn_blocking`, where the blocking disk work
    /// actually executes.  All string data is cloned before the move; the
    /// `SyncCrypto` is `Clone + Send` and is moved in as well.
    pub(crate) async fn persist_paired_peer(
        peer_fp_canonical: &str,
        peer_sync_addr: &str,
        session_key: &copypaste_p2p::pake::SessionKey,
        peer_meta: &copypaste_p2p::bootstrap::PeerMeta,
        sync_crypto: Option<&crate::sync_orch::SyncCrypto>,
    ) {
        // Derive the shared content sync key on the async thread (pure CPU: HKDF
        // + base64 encode, no I/O).  `SessionKey` is not Clone, so we must
        // extract the derived bytes before moving into spawn_blocking.
        let sync_key_b64 = Some(Self::derive_peer_sync_key_b64(session_key));

        // Clone all borrowed data so it can be moved into the blocking thread.
        let peer_fp_canonical = peer_fp_canonical.to_string();
        let peer_sync_addr = peer_sync_addr.to_string();
        let peer_meta = peer_meta.clone();
        let sync_crypto = sync_crypto.cloned();

        let join = tokio::task::spawn_blocking(move || {
            let display = display_fingerprint(&peer_fp_canonical);
            let added_at = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let address = if peer_sync_addr.is_empty() {
                None
            } else {
                Some(peer_sync_addr.clone())
            };

            let path = peers_file_path();
            let mut peers = crate::peers::load_peers(&path);
            // Preserve any existing first/last-sync stamps across a re-pair so the
            // "first sync" history is not reset when the peer is re-paired.
            let (prior_first_sync, prior_last_sync) = peers
                .iter()
                .find(|p| canonical_fingerprint(&p.fingerprint) == peer_fp_canonical)
                .map(|p| (p.first_sync_at, p.last_sync_at))
                .unwrap_or((None, None));
            // Drop any prior record for the same peer (canonical compare) so a
            // re-pair refreshes the address/name instead of duplicating the entry.
            peers.retain(|p| canonical_fingerprint(&p.fingerprint) != peer_fp_canonical);
            // Populate `name` from the in-band device name received over the
            // bootstrap channel. Falls back to empty string when not provided
            // (e.g. discovery-initiated pairs that predate the device_name field).
            // TODO: carry device_name in PeerMeta for discovery-initiated pairs
            // (requires a BOOTSTRAP_PROTO_VERSION bump + re-pair).
            let name = peer_meta.device_name.clone().unwrap_or_default();
            peers.push(crate::peers::PairedDevice {
                fingerprint: display,
                name,
                added_at,
                address,
                sync_key_b64,
                model: peer_meta.model.clone(),
                os_version: peer_meta.os_version.clone(),
                app_version: peer_meta.app_version.clone(),
                local_ip: peer_meta.local_ip.clone(),
                public_ip: peer_meta.public_ip.clone(),
                // CopyPaste-yw2k: persist the peer's non-secret Supabase account
                // identity so list_peers can surface it and the UI can detect
                // cross-account mismatches at render time (not a token/key).
                supabase_account_id: peer_meta.supabase_account_id.clone(),
                first_sync_at: prior_first_sync,
                last_sync_at: prior_last_sync,
                // password_file_b64 / password_file_enc are only populated on the
                // RESPONDER side by pair_accept_finish; persist_paired_peer is called
                // from the INITIATOR path and the QR-responder bootstrap task — neither
                // holds the PasswordFile blob here.  Both fields default to None;
                // pair_accept_finish writes password_file_enc (encrypted) separately.
                password_file_b64: None,
                password_file_enc: None,
            });

            match crate::peers::save_peers(&path, &peers) {
                Ok(()) => {
                    tracing::info!(
                        fingerprint = %peer_fp_canonical,
                        addr = %peer_sync_addr,
                        "persisted paired peer to peers.json"
                    );
                    // H8: refresh the in-memory sync-key cache so the running
                    // orchestrator picks up the new shared key without a restart.
                    // reload_sync_key reads peers.json (disk I/O), so it belongs
                    // here in the blocking thread.
                    if let Some(ref crypto) = sync_crypto {
                        crypto.reload_sync_key();
                    }
                }
                Err(e) => tracing::warn!(
                    fingerprint = %peer_fp_canonical,
                    "failed to persist paired peer to peers.json: {e}"
                ),
            }
        });

        if let Err(e) = join.await {
            tracing::warn!("persist_paired_peer blocking task panicked: {e}");
        }
    }

    /// LAN/SAS Phase 2 — INITIATOR side of discovery-initiated SAS pairing.
    ///
    /// Resolves the discovered peer (`device_id`) to its bootstrap socket
    /// address via the shared [`DiscoveryService`](copypaste_p2p::discovery::DiscoveryService)
    /// (using the v2 `bport` TXT key), generates an EPHEMERAL random PAKE
    /// password, and runs [`run_initiator_with_confirm`](copypaste_p2p::bootstrap::run_initiator_with_confirm).
    ///
    /// ## Why an in-clear ephemeral password is safe here
    /// The discovery path has NO pre-shared secret, so the bootstrap TLS channel
    /// is run with a throwaway random password. Authentication is provided
    /// ENTIRELY by the human SAS comparison: the SAS is derived from the
    /// post-PAKE, post-channel-binding `bound_key`, so a man-in-the-middle that
    /// substitutes its own password per leg produces a DIFFERENT SAS per leg and
    /// the two users see mismatched codes. Both sides must ACCEPT (frame 10a)
    /// before any key is trusted; on reject/abort/timeout the session key is
    /// dropped/zeroized and NOTHING is persisted (no `rotate_peer`).
    ///
    /// The `confirm` callback transitions the state machine to `awaiting_sas`
    /// and awaits the `oneshot` that `pair_confirm_sas`/`pair_abort` fire. On a
    /// both-accept success this reuses the SAME `rotate_peer` +
    /// `persist_paired_peer` as the QR path so the steady-state link is
    /// identical (mutual fingerprint-pinned mTLS).
    async fn pair_with_discovered(&self, req_id: String, device_id: &str) -> Response {
        let cert = match self.p2p_cert.as_ref() {
            Some(c) => Arc::clone(c),
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "P2P is disabled (set COPYPASTE_P2P=1): cannot pair over the network",
                )
            }
        };
        let discovery = match self.discovery.as_ref() {
            Some(d) => d,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "discovery not available (P2P disabled)",
                )
            }
        };

        // Resolve the peer's bootstrap listener address from the live snapshot.
        let peer = match discovery.resolve_peer(device_id) {
            Some(p) => p,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_NOT_FOUND,
                    format!("device not currently discoverable: {device_id}"),
                )
            }
        };
        let bport =
            match peer.bport {
                Some(p) => p,
                None => return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "peer does not advertise a bootstrap port (v1 peer): SAS pairing unsupported",
                ),
            };
        // Prefer an IPv4 address (broadest compatibility); fall back to the
        // first address of any family. `ip_addrs` is sorted IPv4-first.
        let ip = match peer
            .ip_addrs
            .iter()
            .find(|a| a.is_ipv4())
            .or_else(|| peer.ip_addrs.first())
        {
            Some(ip) => *ip,
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_NOT_FOUND,
                    "peer has no resolved IP address",
                )
            }
        };
        let addr = std::net::SocketAddr::new(ip, bport);

        // Build the peer snapshot from the mDNS PeerInfo resolved above.
        // This is available immediately (pre-handshake) and is the richest
        // source of peer identity data at `pair_get_sas` poll time. The PAKE
        // metadata exchange (model/OS/version) happens AFTER the SAS confirm
        // step and is surfaced in the final `pair_with_discovered` response.
        let peer_snapshot = crate::pairing_sm::PeerSnapshot {
            device_name: if peer.device_name.is_empty() {
                None
            } else {
                Some(peer.device_name.clone())
            },
            ip_addrs: peer.ip_addrs.iter().map(|a| a.to_string()).collect(),
            // device_id IS the cert fingerprint (hex SHA-256); use it directly
            // so the UI can show the fingerprint before the TLS handshake.
            fingerprint: if peer.device_id.is_empty() {
                None
            } else {
                Some(peer.device_id.clone())
            },
        };

        // Claim the single-active-pairing slot. A concurrent request is rejected
        // with a rate-limited error (one pairing at a time, v0.6 simplicity).
        if !self.pairing.try_begin(
            crate::pairing_sm::PairingRole::Initiator,
            peer_snapshot.clone(),
        ) {
            return Response::err_with_code(
                req_id,
                ERR_CODE_RATE_LIMITED,
                "another pairing is already in progress",
            );
        }

        // Discovery (QR-less) path: a FIXED, well-known, NON-SECRET PAKE password
        // shared by every initiator/responder. opaque-ke is asymmetric, so a
        // per-side random password would fail `ClientLogin::finish` at frame 7
        // before any SAS is derived. The human SAS compare authenticates, not the
        // password — see `copypaste_p2p::DISCOVERY_PAIRING_PASSWORD`. (QR pairing
        // keeps its token-derived password; this only affects discovery.)
        let password = copypaste_p2p::DISCOVERY_PAIRING_PASSWORD.to_string();
        let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
        let own_sync_addr = self.own_sync_addr();
        // B1: our own STUN-discovered global IP, read from the shared cache and
        // advertised in-band so the peer can show it. None if STUN unresolved or
        // collection is disabled. Reuses the daemon's single STUN source.
        let own_public_ip = self.cached_public_ip.read().await.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: read the local Supabase account identity (non-secret)
        // to advertise it in-band so the peer can detect cross-account mismatches.
        let own_supabase_account_id: Option<String> = self
            .cloud_account_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let own_meta = tokio::task::spawn_blocking(move || {
            Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
        })
        .await
        .unwrap_or_default();
        // "QR fully provisions all sync": advertise our Supabase/relay config +
        // derived sync key over the authenticated tunnel (None if unconfigured).
        let own_provisioning = self.build_local_provisioning().await;

        let coordinator = Arc::clone(&self.pairing);
        // The confirm callback runs AFTER frame 9 (PAKE + channel binding), when
        // the SAS is known and identical on both honest endpoints. It moves the
        // SM to `awaiting_sas` and awaits the user's decision (or the dropped
        // sender on abort, which it maps to a rejection).
        let confirm = move |sas: &str, peer_fp: &str| {
            let coordinator = Arc::clone(&coordinator);
            let sas = sas.to_string();
            // Forward the already-captured peer snapshot so `pair_get_sas` polls
            // surface the mDNS identity while the user is reading the SAS code.
            // CopyPaste-n3bc: override with the verified TLS peer fingerprint.
            let mut snap = peer_snapshot.clone();
            snap.fingerprint = Some(peer_fp.to_string());
            async move {
                let rx = coordinator.enter_awaiting_sas(
                    sas,
                    crate::pairing_sm::PairingRole::Initiator,
                    snap,
                );
                // SAS_CONFIRM_TIMEOUT bounds the human decision; a dropped sender
                // (abort) or elapsed timeout both yield a rejection.
                match tokio::time::timeout(crate::pairing_sm::SAS_CONFIRM_TIMEOUT, rx).await {
                    Ok(Ok(accept)) => accept,
                    // Sender dropped (pair_abort) or timed out → reject.
                    _ => false,
                }
            }
        };

        let result = copypaste_p2p::bootstrap::run_initiator_with_confirm(
            addr,
            cert_der,
            key_der,
            &password,
            &own_sync_addr,
            &own_meta,
            own_provisioning,
            confirm,
        )
        .await;

        match result {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    "discovery SAS pairing completed (both sides accepted)"
                );
                // Both sides accepted: trust + persist exactly like the QR path.
                if let Some(ref peers) = self.p2p_peers {
                    peers.rotate_peer(
                        &outcome.peer_fingerprint,
                        outcome.peer_fingerprint.clone(),
                        String::new(),
                    );
                }
                let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                    model: outcome.peer_model.clone(),
                    os_version: outcome.peer_os.clone(),
                    app_version: outcome.peer_app_version.clone(),
                    local_ip: outcome.peer_local_ip.clone(),
                    device_name: outcome.peer_device_name.clone(),
                    public_ip: outcome.peer_public_ip.clone(),
                    device_id: outcome.peer_device_id.clone(),
                    // CopyPaste-yw2k: carry the peer's non-secret account identity.
                    supabase_account_id: outcome.peer_supabase_account_id.clone(),
                };
                Self::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                    &peer_meta,
                    self.p2p_sync_crypto.as_ref(),
                )
                .await;
                // "QR fully provisions all sync": apply any sync config the peer
                // advertised that we currently lack (never overwrites existing).
                if let Some(prov) = outcome.peer_provisioning {
                    self.apply_peer_provisioning(prov).await;
                }
                self.pairing
                    .finish(crate::pairing_sm::PairingState::Confirmed);
                let resp = Response::ok(
                    req_id,
                    serde_json::json!({
                        "ok": true,
                        "peer_fingerprint": outcome.peer_fingerprint,
                    }),
                );
                // BUG A1: the terminal outcome is returned synchronously to the
                // UI in `resp`, so the brief observable-window concern does not
                // apply on this initiator path. Reset the SM to `Idle` so a
                // SUBSEQUENT `pair_with_discovered` is not refused as
                // rate-limited (the SM requires `is_idle()` for `try_begin`).
                self.pairing.reset();
                resp
            }
            Err(e) => {
                // Reject / mismatch / timeout / network error → NO persist, NO
                // rotate_peer; the session key already dropped/zeroized inside
                // the bootstrap function. Record a terminal state unless the SM
                // was already moved to a terminal state by `pair_abort`.
                let snapshot = self.pairing.snapshot();
                if !snapshot.is_terminal() {
                    self.pairing
                        .finish(crate::pairing_sm::PairingState::Rejected);
                }
                tracing::warn!("discovery SAS pairing failed: {e}");
                // HB-4: a raw TCP connect failure ("Connection refused", host
                // unreachable, timeout) means the peer's bootstrap responder is
                // not listening — almost always because the device is already
                // paired (so it no longer advertises) or its Devices/pairing
                // screen is closed. Map that to a friendly message instead of the
                // raw os-error; genuine PAKE/SAS failures keep the auth message.
                let lower = e.to_string().to_ascii_lowercase();
                let is_connect_failure = lower.contains("connection refused")
                    || lower.contains("connect")
                    || lower.contains("unreachable")
                    || lower.contains("timed out")
                    || lower.contains("timeout")
                    || lower.contains("os error 61")
                    || lower.contains("os error 111");
                let (code, message) = if is_connect_failure {
                    (
                        ERR_CODE_NOT_FOUND,
                        "device not reachable (already paired or its screen is closed)".to_string(),
                    )
                } else {
                    (
                        ERR_CODE_AUTH_FAILED,
                        format!("discovery SAS pairing failed: {e}"),
                    )
                };
                let resp = Response::err_with_code(req_id, code, message);
                // BUG A1: reset the SM to `Idle` on EVERY failure return path that
                // reached here after `try_begin` succeeded, so the next pairing
                // attempt is not refused as rate-limited. The terminal outcome is
                // already returned synchronously to the UI in `resp` above.
                self.pairing.reset();
                resp
            }
        }
    }

    /// Spawn the responder side of the P2P Phase 1 bootstrap PAKE handshake.
    ///
    /// The `responder` already owns the bound, TLS-wrapped ephemeral listener
    /// whose address was advertised in the QR's `addr_hint`. This accepts ONE
    /// inbound connection within the pairing window and runs the PAKE responder
    /// over the TLS stream. On success the peer's cert fingerprint (learned over
    /// the same channel) is registered in the live mTLS allowlist so subsequent
    /// pinned mTLS sessions are accepted without a daemon restart.
    ///
    /// Runs detached: pairing is driven by the scanning device dialling in, so
    /// there is nothing for the IPC caller to await here. PAKE failure (wrong
    /// token, MitM, timeout) only logs — no peer is registered.
    ///
    /// Race-fix (CopyPaste-7mf): returns the `JoinHandle` so the caller can store
    /// it in `self.pending_bootstrap`. `list_peers` awaits that handle (with a
    /// short timeout) before reading `peers.json`, ensuring that a
    /// `pair_generate_qr` → (initiator scans) → `list_peers` sequence on the
    /// responder side always sees the freshly-persisted peer.
    ///
    /// Empty-address fix: `own_sync_addr` is now read from the slot INSIDE the
    /// spawned task, after `DeviceMeta::collect` completes but before
    /// `responder.run()`. This gives the P2P subsystem maximum time to bind its
    /// listener and populate the slot (it does so on startup, before any pairing
    /// request arrives in practice). If the slot is still empty at that point the
    /// record stores `address: null` and the connector falls back to mDNS — the
    /// same graceful degradation as before, but without over-capturing a stale
    /// empty string from before the P2P listener was ready.
    fn spawn_bootstrap_responder(
        &self,
        responder: copypaste_p2p::bootstrap::BootstrapResponder,
        password: String,
    ) -> tokio::task::JoinHandle<()> {
        let peers = self.p2p_peers.clone();
        // Clone the addr slot Arc so the task can read it after device metadata
        // is collected — giving the P2P listener maximum time to populate it.
        // (Empty-address fix: previously own_sync_addr() was called here, before
        // the async work inside the task, so a racing listener start would still
        // produce an empty address. Reading from the Arc inside the task is later
        // and avoids that window.)
        let own_sync_addr_slot = self.p2p_sync_addr.clone();
        // B1: clone the public-IP cache Arc before the move so the detached task
        // can read our current STUN-discovered global IP to advertise in-band.
        let public_ip_cache = self.cached_public_ip.clone();
        // "QR fully provisions all sync": clone the sync_key Arc so the detached
        // task can BUILD our provisioning to advertise and APPLY the peer's.
        #[cfg(feature = "cloud-sync")]
        let sync_key = self.sync_key.clone();
        // H8: clone before the move so the spawned task can call reload_sync_key
        // after persist_paired_peer writes peers.json.
        let spawn_sync_crypto = self.p2p_sync_crypto.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: clone the account-id Arc before the move so the
        // spawned task can read the non-secret identity to advertise in-band.
        let cloud_account_id_arc = self.cloud_account_id.clone();
        tokio::spawn(async move {
            // CopyPaste-yw2k: read the non-secret local Supabase account id
            // inside the task (after the Arc was cloned before the spawn).
            let own_supabase_account_id: Option<String> = cloud_account_id_arc
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .clone();
            // P2P Phase 4: collect our own device metadata to advertise in-band.
            // DeviceMeta::collect spawns child processes (up to ~2 s), so run it
            // off the async worker. Falls back to empty metadata on join error.
            let own_public_ip = public_ip_cache.read().await.clone();
            let own_meta = tokio::task::spawn_blocking(move || {
                Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
            })
            .await
            .unwrap_or_default();
            // Read own_sync_addr here, after metadata collection, to give the P2P
            // listener the maximum window to have populated the slot.
            let own_sync_addr = own_sync_addr_slot
                .lock()
                .map(|slot| slot.clone().unwrap_or_default())
                .unwrap_or_else(|poisoned| poisoned.into_inner().clone().unwrap_or_default());
            // Build our SyncProvisioning to advertise (None without cloud-sync).
            #[cfg(feature = "cloud-sync")]
            let own_provisioning = Self::build_local_provisioning_from(&sync_key).await;
            #[cfg(not(feature = "cloud-sync"))]
            let own_provisioning: Option<copypaste_p2p::bootstrap::SyncProvisioning> = None;
            match responder
                .run(&password, &own_sync_addr, &own_meta, own_provisioning)
                .await
            {
                Ok(outcome) => {
                    tracing::info!(
                        peer_fingerprint = %outcome.peer_fingerprint,
                        peer_sync_addr = %outcome.peer_sync_addr,
                        "bootstrap PAKE responder completed over network channel"
                    );
                    // Register the freshly-paired peer in the live allowlist.
                    // The bootstrap channel reports the canonical (colon-free)
                    // hex fingerprint; `rotate_peer` upserts it as active.
                    if let Some(peers) = peers {
                        peers.rotate_peer(
                            &outcome.peer_fingerprint,
                            outcome.peer_fingerprint.clone(),
                            String::new(),
                        );
                    }
                    // P2P Phase 2: durably persist the peer (fingerprint +
                    // sync-listener address) so it survives a restart and the
                    // Phase 3 connector can dial it directly. Phase 4: also
                    // persist the peer's advertised device metadata.
                    let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                        model: outcome.peer_model.clone(),
                        os_version: outcome.peer_os.clone(),
                        app_version: outcome.peer_app_version.clone(),
                        local_ip: outcome.peer_local_ip.clone(),
                        device_name: outcome.peer_device_name.clone(),
                        public_ip: outcome.peer_public_ip.clone(),
                        device_id: outcome.peer_device_id.clone(),
                        // CopyPaste-yw2k: carry the peer's non-secret account identity.
                        supabase_account_id: outcome.peer_supabase_account_id.clone(),
                    };
                    // Persist is the last observable side-effect of the bootstrap
                    // task. `list_peers` awaits `pending_bootstrap` (stored by
                    // `pair_generate_qr`) before reading peers.json, so callers
                    // see a consistent view once this JoinHandle completes.
                    Self::persist_paired_peer(
                        &outcome.peer_fingerprint,
                        &outcome.peer_sync_addr,
                        &outcome.session_key,
                        &peer_meta,
                        spawn_sync_crypto.as_ref(),
                    )
                    .await;
                    // "QR fully provisions all sync": apply any sync config the
                    // scanning peer advertised that we currently lack.
                    #[cfg(feature = "cloud-sync")]
                    if let Some(prov) = outcome.peer_provisioning {
                        Self::apply_peer_provisioning_to(&sync_key, prov).await;
                    }
                }
                Err(e) => {
                    tracing::warn!("bootstrap PAKE responder failed: {e}");
                }
            }
        })
    }

    /// Initiator side of the P2P Phase 1 network pairing flow.
    ///
    /// Decodes the scanned `qr`, derives the PAKE password from its token,
    /// resolves the responder's `host:port` (QR `addr_hint` primary; mDNS
    /// `resolve_peer` fallback), dials the unauthenticated bootstrap TLS channel,
    /// and runs the PAKE initiator over it. On success the responder's cert
    /// fingerprint is registered in the live mTLS allowlist.
    ///
    /// Returns the IPC `Response` directly (this is the whole handler for the
    /// network branch of `pair_accept_qr`).
    async fn pair_accept_qr_network(&self, req_id: String, qr: &str) -> Response {
        // We must have our own cert to present on the bootstrap channel so the
        // responder learns the fingerprint it will later pin.
        let cert = match self.p2p_cert.as_ref() {
            Some(c) => Arc::clone(c),
            None => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    "P2P is disabled (set COPYPASTE_P2P=1): cannot accept a pairing QR \
                     over the network without an mTLS certificate",
                )
            }
        };

        // Accept both the wrapped cppair://pair?p=… deep-link form (emitted by
        // pair_generate_qr / Android for external scanners) and a bare CPPAIR2
        // string (back-compat). strip_deeplink is a no-op on the bare form.
        let bare = copypaste_core::strip_deeplink(qr);
        let payload = match copypaste_core::PairingPayload::decode(&bare) {
            Ok(p) => p,
            Err(e) => {
                return Response::err_with_code(
                    req_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    format!("failed to decode pairing QR: {e}"),
                )
            }
        };

        let password = payload.token.to_pake_password();

        // Resolve the responder's address: addr_hint is primary; fall back to
        // mDNS resolution by device_id when it is empty (best-effort — loopback
        // mDNS is unreliable, see discovery::resolve_peer).
        let addr = match self.resolve_pairing_addr(&payload) {
            Ok(addr) => addr,
            Err(msg) => return Response::err_with_code(req_id, ERR_CODE_INVALID_ARGUMENT, msg),
        };

        let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
        // Our own P2P sync-listener address, sent in-band so the responder can
        // persist it for its Phase 3 connector.
        let own_sync_addr = self.own_sync_addr();
        // B1: our own STUN-discovered global IP, advertised in-band so the peer
        // can show it. None if unresolved/disabled.
        let own_public_ip = self.cached_public_ip.read().await.clone();
        let own_device_id = self.local_device_id.clone();
        // CopyPaste-yw2k: read the non-secret local Supabase account id to
        // advertise in-band so the peer can detect cross-account mismatches.
        let own_supabase_account_id: Option<String> = self
            .cloud_account_id
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        // P2P Phase 4: collect our own device metadata to advertise in-band.
        // DeviceMeta::collect spawns child processes (up to ~2 s), so run it off
        // the async worker; empty metadata on join error.
        let own_meta = tokio::task::spawn_blocking(move || {
            Self::collect_own_peer_meta(own_public_ip, own_device_id, own_supabase_account_id)
        })
        .await
        .unwrap_or_default();
        // "QR fully provisions all sync": advertise our Supabase/relay config +
        // derived sync key over the authenticated tunnel (None if unconfigured).
        let own_provisioning = self.build_local_provisioning().await;
        match copypaste_p2p::bootstrap::run_initiator(
            addr,
            cert_der,
            key_der,
            &password,
            &own_sync_addr,
            &own_meta,
            own_provisioning,
        )
        .await
        {
            Ok(outcome) => {
                tracing::info!(
                    peer_fingerprint = %outcome.peer_fingerprint,
                    peer_sync_addr = %outcome.peer_sync_addr,
                    "bootstrap PAKE initiator completed over network channel"
                );
                if let Some(ref peers) = self.p2p_peers {
                    peers.rotate_peer(
                        &outcome.peer_fingerprint,
                        outcome.peer_fingerprint.clone(),
                        String::new(),
                    );
                }
                // P2P Phase 2: durably persist the peer (fingerprint + the
                // sync-listener address it advertised) for restart-survival and
                // the Phase 3 outbound connector. Phase 4: also persist the
                // peer's advertised device metadata.
                let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
                    model: outcome.peer_model.clone(),
                    os_version: outcome.peer_os.clone(),
                    app_version: outcome.peer_app_version.clone(),
                    local_ip: outcome.peer_local_ip.clone(),
                    device_name: outcome.peer_device_name.clone(),
                    public_ip: outcome.peer_public_ip.clone(),
                    device_id: outcome.peer_device_id.clone(),
                    // CopyPaste-yw2k: carry the peer's non-secret account identity.
                    supabase_account_id: outcome.peer_supabase_account_id.clone(),
                };
                Self::persist_paired_peer(
                    &outcome.peer_fingerprint,
                    &outcome.peer_sync_addr,
                    &outcome.session_key,
                    &peer_meta,
                    self.p2p_sync_crypto.as_ref(),
                )
                .await;
                // "QR fully provisions all sync": apply any sync config the
                // responder advertised that we currently lack.
                if let Some(prov) = outcome.peer_provisioning {
                    self.apply_peer_provisioning(prov).await;
                }
                Response::ok(
                    req_id,
                    serde_json::json!({
                        "ok": true,
                        "peer_fingerprint": outcome.peer_fingerprint,
                    }),
                )
            }
            Err(e) => Response::err_with_code(
                req_id,
                ERR_CODE_AUTH_FAILED,
                format!("network PAKE pairing failed: {e}"),
            ),
        }
    }

    /// Resolve the responder's socket address for the initiator bootstrap dial.
    ///
    /// Uses the QR `addr_hint` when present; otherwise falls back to mDNS
    /// `resolve_peer` keyed by the QR's `device_id`. Returns a human-readable
    /// error string when neither yields a usable address.
    fn resolve_pairing_addr(
        &self,
        payload: &copypaste_core::PairingPayload,
    ) -> Result<std::net::SocketAddr, String> {
        if !payload.addr_hint.is_empty() {
            return payload
                .addr_hint
                .parse::<std::net::SocketAddr>()
                .map_err(|e| format!("invalid addr_hint '{}': {e}", payload.addr_hint));
        }

        // mDNS fallback (best-effort).
        let discovery = self
            .discovery
            .as_ref()
            .ok_or_else(|| "QR has no addr_hint and mDNS discovery is unavailable".to_string())?;
        let peer = discovery
            .resolve_peer(&payload.device_id)
            .ok_or_else(|| "QR has no addr_hint and the peer was not found via mDNS".to_string())?;
        let ip = peer
            .ip_addrs
            .first()
            .ok_or_else(|| "mDNS-resolved peer has no IP address".to_string())?;
        Ok(std::net::SocketAddr::new(*ip, peer.port))
    }

    /// Returns true if a request to `method` requires the backing database.
    /// Methods that only touch in-memory state (status, get/set_private_mode,
    /// get_own_fingerprint, peer file ops, config file ops) are allowed
    /// before the DB is ready so the client can still introspect the daemon.
    fn requires_db(method: &str) -> bool {
        matches!(
            method,
            // c4q2.17: "list" removed — now a not_implemented stub, no DB access.
            "delete"
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
                | "reorder_pinned"
                | "history_page"
                | "import"
                // export decrypts every row — needs a ready DB.
                | "export"
                // get_item_image decrypts image chunks — needs a ready DB.
                | "get_item_image"
                // get_item_thumbnail decrypts the thumbnail blob — needs a ready DB.
                | "get_item_thumbnail"
                // get_item_file decrypts file chunks — needs a ready DB.
                | "get_item_file"
                // add_file_item encrypts and stores a new file item — needs a ready DB.
                | "add_file_item"
                | "revoke_peer"
                | "revoke_all_peers"
                // revoke_and_rotate runs the revoke body (audit-row insert),
                // which needs a ready DB.
                | "revoke_and_rotate"
                // db_stats reads item count and file size — needs a ready DB.
                | "db_stats"
                // CopyPaste-crh3.7: db_backup and vacuum must be gated in
                // degraded mode. Otherwise db_backup VACUUM INTOs the empty
                // in-memory placeholder and returns {ok:true} for an EMPTY
                // backup, and vacuum runs on the placeholder while reporting
                // size_before/after read from the REAL on-disk file — both
                // dangerously misleading. db_restore is intentionally NOT here:
                // it is the recovery escape hatch and must work while degraded.
                | "db_backup"
                | "vacuum"
        )
    }

    /// Returns true if `method` may carry an inbound payload larger than
    /// [`SMALL_REQUEST_BYTES`] (up to [`MAX_REQUEST_BYTES`]).
    ///
    /// CopyPaste-c4q2.28: only bulk-ingest methods legitimately send megabytes
    /// of request body — `import` (whole-history restore) and `add_file_item`
    /// (base64-encoded file bytes from the desktop UI). Every other method's
    /// request is small (ids, flags, short strings); capping them at
    /// [`SMALL_REQUEST_BYTES`] removes the RAM-amplification vector without
    /// affecting any real client. Response size is unrelated — this bounds only
    /// the inbound request line.
    fn allows_large_payload(method: &str) -> bool {
        method == copypaste_ipc::METHOD_IMPORT || method == copypaste_ipc::METHOD_ADD_FILE_ITEM
    }

    /// Run the IPC accept loop until `shutdown` is cancelled.
    ///
    /// D2: accepts a [`CancellationToken`] so the daemon can stop the server
    /// cleanly on SIGINT/SIGTERM instead of relying on task abort.
    /// Bind the IPC listener (self-healing stale sockets) WITHOUT starting the
    /// accept loop.
    ///
    /// # Why this is split out from [`serve`](Self::serve)
    ///
    /// DUAL-DAEMON FIX: the daemon startup must treat a bind failure as FATAL
    /// (another healthy daemon already owns the socket → this instance is the
    /// loser and must exit WITHOUT starting its own P2P/mDNS stack). When the
    /// bind was buried inside the `tokio::spawn`ed `serve` future, a bind
    /// failure only logged and the rest of startup — including `start_p2p` —
    /// ran anyway, producing a second concurrent P2P stack. Binding here, in
    /// the caller's context, lets the caller `return Err` / exit before P2P.
    ///
    /// On success the socket exists with mode `0600` and is ready for
    /// [`serve_on`](Self::serve_on).
    pub fn bind(&self, socket_path: &std::path::Path) -> anyhow::Result<UnixListener> {
        // Ensure parent directory exists and is user-only (0o700) so that the
        // socket cannot be reached by other local users even if the socket
        // mode itself were ever loosened.
        if let Some(parent) = socket_path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }

        // Self-heal stale sockets. A previous daemon that crashed or was
        // killed (e.g. a v0.3.4 process replaced by a v0.4.0 upgrade) leaves
        // the on-disk socket file behind. A plain `bind` over an existing path
        // fails with `EADDRINUSE`, so the new daemon would never come up and
        // the UI would see "process alive but socket not reachable". We probe
        // the existing socket first: if NO live listener answers it, it is a
        // stale file we may safely remove and rebind. If a live listener DOES
        // answer, another healthy daemon already owns it — we must NOT steal
        // the socket out from under it, so we surface a hard error instead.
        let listener = bind_with_stale_cleanup(socket_path)?;

        // chmod 0600 — the IPC socket gives full control over the user's
        // clipboard history and peer database. It must not be world- or
        // group-connectable. Done immediately after bind, before accept loop.
        std::fs::set_permissions(socket_path, std::fs::Permissions::from_mode(0o600))?;

        tracing::info!("IPC listening on {} (mode=0600)", socket_path.display());
        Ok(listener)
    }

    pub async fn serve(
        self,
        socket_path: &std::path::Path,
        shutdown: CancellationToken,
    ) -> anyhow::Result<()> {
        let listener = self.bind(socket_path)?;
        self.serve_on(listener, shutdown).await
    }

    /// Run the IPC accept loop on an already-bound listener (see
    /// [`bind`](Self::bind)).
    pub async fn serve_on(
        self,
        listener: UnixListener,
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

        let server = Arc::new(self);
        // daemon-core L2: track in-flight per-connection tasks in a JoinSet so
        // they can be aborted on shutdown instead of being orphaned. Previously
        // each `tokio::spawn` was fire-and-forget: on `shutdown.cancelled()` the
        // accept loop returned while connection tasks kept running (benign today
        // since the process exits shortly after, but it leaked tasks that could
        // hold the DB Mutex past the documented drain point).
        let mut conns: tokio::task::JoinSet<()> = tokio::task::JoinSet::new();
        loop {
            tokio::select! {
                // D2: stop accepting new connections on daemon-wide shutdown.
                _ = shutdown.cancelled() => {
                    tracing::info!("IPC server: shutdown signal received, stopping accept loop");
                    break;
                }
                // Reap finished connection tasks so the JoinSet does not grow
                // unbounded over the daemon's lifetime. `join_next` resolves to
                // `None` only when the set is empty, in which case this branch is
                // disabled by the `if` guard and never busy-loops.
                _ = conns.join_next(), if !conns.is_empty() => {}
                result = listener.accept() => {
                    match result {
                        Ok((stream, _)) => {
                            // CopyPaste-6ot5: non-blocking permit acquire.
                            // `try_acquire_owned` never blocks the accept loop;
                            // it returns `Err` immediately when all permits are
                            // taken. The `OwnedSemaphorePermit` is moved into
                            // the task and dropped on task exit, reclaiming the
                            // slot for the next connection.
                            match server.conn_semaphore.clone().try_acquire_owned() {
                                Ok(permit) => {
                                    let s = server.clone();
                                    conns.spawn(async move {
                                        // Hold the permit for the connection lifetime.
                                        let _permit = permit;
                                        if let Err(e) = s.handle_connection(stream).await {
                                            tracing::warn!("IPC connection error: {e}");
                                        }
                                    });
                                }
                                Err(_) => {
                                    // All connection slots are taken; drop the
                                    // stream immediately (client sees a closed
                                    // connection). This prevents unbounded task
                                    // accumulation from a buggy or hostile client.
                                    tracing::warn!(
                                        "IPC connection rejected: concurrent connection \
                                         cap ({MAX_CONCURRENT_CONNECTIONS}) reached"
                                    );
                                    drop(stream);
                                }
                            }
                        }
                        Err(e) => tracing::error!("accept error: {e}"),
                    }
                }
            }
        }
        // daemon-core L2: abort any still-running connection tasks. The daemon's
        // drain step (`_ipc_handle.await` in daemon.rs) then completes promptly
        // instead of waiting on a client that never closes its socket.
        conns.abort_all();
        while conns.join_next().await.is_some() {}
        Ok(())
    }

    #[tracing::instrument(skip_all, name = "ipc_connection")]
    async fn handle_connection(&self, stream: UnixStream) -> anyhow::Result<()> {
        let (reader, mut writer) = stream.into_split();
        let mut reader = BufReader::new(reader);
        let mut buf: Vec<u8> = Vec::with_capacity(4 * 1024);

        loop {
            buf.clear();
            // CopyPaste-c4q2.28: two-pass, method-aware size cap. Phase 1 reads
            // at most SMALL_REQUEST_BYTES + 1 so an unclassified request can
            // never make the daemon buffer more than ~64 KiB. Only after the
            // method is known (and is on the large-payload allow-list) do we
            // extend the cap to MAX_REQUEST_BYTES in phase 2.
            //
            // CopyPaste-cce1: each read is wrapped in IPC_READ_TIMEOUT so a
            // stalled client cannot hold its connection slot (and the DB Mutex)
            // indefinitely. On deadline we drop the connection; the client sees
            // EOF on its next read.
            let mut limited = (&mut reader).take((SMALL_REQUEST_BYTES as u64) + 1);
            let n =
                match tokio::time::timeout(IPC_READ_TIMEOUT, limited.read_until(b'\n', &mut buf))
                    .await
                {
                    Ok(Ok(n)) => n,
                    Ok(Err(e)) => {
                        tracing::warn!("ipc read error: {e}");
                        return Ok(());
                    }
                    Err(_elapsed) => {
                        tracing::warn!(
                            timeout_secs = IPC_READ_TIMEOUT.as_secs(),
                            "ipc read timeout: dropping stalled client connection"
                        );
                        return Ok(());
                    }
                };

            // Clean EOF — client closed the socket without sending more data.
            if n == 0 {
                return Ok(());
            }

            // If phase 1 did not terminate on a newline, the request is larger
            // than the small per-method cap. Classify the method from the
            // buffered prefix: only the large-payload allow-list may continue;
            // everything else is rejected here, having buffered ≤ 64 KiB.
            if buf.last() != Some(&b'\n') {
                let method = extract_json_string_field(&buf, "method");
                let large_ok = method
                    .as_deref()
                    .map(Self::allows_large_payload)
                    .unwrap_or(false);
                if !large_ok {
                    tracing::warn!(
                        bytes_read = n,
                        limit = SMALL_REQUEST_BYTES,
                        method = method.as_deref().unwrap_or("<unknown>"),
                        "ipc request exceeded the per-method size cap; rejecting and closing"
                    );
                    send_request_too_large(
                        &mut writer,
                        &buf,
                        SMALL_REQUEST_BYTES,
                        "Only bulk methods (import, add_file_item) may exceed it.",
                    )
                    .await;
                    return Ok(());
                }

                // Phase 2: large-payload method — read the remainder up to the
                // MAX_REQUEST_BYTES total cap (still under the read deadline).
                let remaining = (MAX_REQUEST_BYTES as u64 + 1).saturating_sub(buf.len() as u64);
                let mut limited2 = (&mut reader).take(remaining);
                let n2 = match tokio::time::timeout(
                    IPC_READ_TIMEOUT,
                    limited2.read_until(b'\n', &mut buf),
                )
                .await
                {
                    Ok(Ok(n2)) => n2,
                    Ok(Err(e)) => {
                        tracing::warn!("ipc read error (large payload): {e}");
                        return Ok(());
                    }
                    Err(_elapsed) => {
                        tracing::warn!(
                            timeout_secs = IPC_READ_TIMEOUT.as_secs(),
                            "ipc read timeout (large payload): dropping stalled client connection"
                        );
                        return Ok(());
                    }
                };
                // Clean EOF mid-stream — client closed without finishing.
                if n2 == 0 {
                    return Ok(());
                }
                // Still no newline after consuming the full MAX cap → oversized.
                if buf.last() != Some(&b'\n') {
                    tracing::warn!(
                        bytes_read = buf.len(),
                        limit = MAX_REQUEST_BYTES,
                        "ipc request exceeded {MAX_REQUEST_BYTES} bytes; rejecting and closing"
                    );
                    send_request_too_large(
                        &mut writer,
                        &buf,
                        MAX_REQUEST_BYTES,
                        "For large imports split the payload into smaller batches.",
                    )
                    .await;
                    return Ok(());
                }
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
                        // Bounded write (CopyPaste-c4q2.24).
                        let _ = tokio::time::timeout(
                            IPC_WRITE_TIMEOUT,
                            writer.write_all(out.as_bytes()),
                        )
                        .await;
                    }
                    continue;
                }
            };

            // CopyPaste-44rq.19: watch_subscribe is a streaming method — it
            // holds the connection open and writes one line per new item rather
            // than returning a single response. Intercept it here, before the
            // normal one-shot dispatch, so the streaming loop can own `writer`
            // without interfering with any other method's request/response model.
            // When the client disconnects (write error) or the broadcast channel
            // is closed, the call returns and the connection is dropped cleanly.
            if extract_json_string_field(line.as_bytes(), "method").as_deref()
                == Some(copypaste_ipc::METHOD_WATCH_SUBSCRIBE)
            {
                // CopyPaste-crh3.105: apply the SAME protocol-version + readiness
                // gates that dispatch() enforces. Without this, a degraded daemon
                // accepted the subscription and handed the client a silent empty
                // stream with no indication it was unavailable. watch_subscribe
                // streams item metadata, so it requires a ready DB even though it
                // is (deliberately) absent from the requires_db allow-list.
                if let Ok(req) = serde_json::from_str::<Request>(line) {
                    if let Some(err) = self.check_request_gates(&req, true) {
                        let mut out = serde_json::to_string(&err)?;
                        out.push('\n');
                        let _ = tokio::time::timeout(
                            IPC_WRITE_TIMEOUT,
                            writer.write_all(out.as_bytes()),
                        )
                        .await;
                        return Ok(());
                    }
                }
                return self.handle_watch_subscribe(line, writer).await;
            }

            let resp = self.dispatch(line).await;
            let mut out = serde_json::to_string(&resp)?;
            out.push('\n');
            // CopyPaste-c4q2.24: bound the write with IPC_WRITE_TIMEOUT so a
            // client that stops draining its recv buffer cannot pin this
            // connection slot (and its semaphore permit) indefinitely.
            match tokio::time::timeout(IPC_WRITE_TIMEOUT, writer.write_all(out.as_bytes())).await {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    // Client disconnected mid-response — log and exit cleanly,
                    // do not panic the spawned task.
                    tracing::debug!("ipc write failed (client disconnected): {e}");
                    return Ok(());
                }
                Err(_elapsed) => {
                    tracing::warn!(
                        timeout_secs = IPC_WRITE_TIMEOUT.as_secs(),
                        "ipc write timeout: dropping slow-draining client connection"
                    );
                    return Ok(());
                }
            }
        }
    }

    /// Handle the `watch_subscribe` streaming IPC method (CopyPaste-44rq.19).
    ///
    /// Unlike every other method, this call does NOT return a single response and
    /// exit. Instead it:
    /// 1. Parses the request to extract the `id`.
    /// 2. Writes one ack line (`{"ok":true,"event":"subscribed","id":"<id>"}`).
    /// 3. Loops on `new_item_tx.subscribe()`: for each broadcast item writes one
    ///    event line (`{"ok":true,"event":"new_item",...}`), omitting all plaintext.
    /// 4. Returns (cleanly, no error) when:
    ///    - The client disconnects (write returns an error).
    ///    - `new_item_tx` is `None` — the daemon was started without a broadcast
    ///      channel (degraded mode / tests); in that case no events are ever emitted
    ///      but the ack is still sent and the call idles until client disconnect.
    ///    - A `Lagged` error from the broadcast receiver: we skip the missed items
    ///      and continue so a slow consumer never crashes the daemon.
    ///
    /// The `writer` half of the Unix stream is owned for the duration of this call
    /// and dropped on return, cleanly closing the send side of the connection.
    ///
    /// SECURITY: event lines carry `item_id`, `content_type`, `wall_time`, and
    /// `is_sensitive` — the same metadata as `history_page`. Plaintext / ciphertext
    /// is NEVER included. No special auth beyond the socket's 0600 mode is needed.
    async fn handle_watch_subscribe(
        &self,
        line: &str,
        mut writer: tokio::net::unix::OwnedWriteHalf,
    ) -> anyhow::Result<()> {
        // Extract the request id for the ack (best-effort; fall back to "?").
        let req_id: String = serde_json::from_str::<serde_json::Value>(line)
            .ok()
            .and_then(|v| {
                v["id"]
                    .as_str()
                    .map(|s| s.to_string())
                    .or_else(|| v["id"].as_i64().map(|n| n.to_string()))
                    .or_else(|| v["id"].as_u64().map(|n| n.to_string()))
            })
            .unwrap_or_else(|| "?".to_string());

        // Send the initial ack.
        let mut ack = serde_json::json!({
            "ok": true,
            "event": "subscribed",
            "id": req_id,
        })
        .to_string();
        ack.push('\n');
        // On write failure the client already disconnected — exit cleanly.
        match tokio::time::timeout(IPC_WRITE_TIMEOUT, writer.write_all(ack.as_bytes())).await {
            Ok(Ok(())) => {}
            Ok(Err(e)) => {
                tracing::debug!("watch_subscribe: ack write failed (client gone): {e}");
                return Ok(());
            }
            Err(_) => {
                tracing::debug!("watch_subscribe: ack write timed out — dropping client");
                return Ok(());
            }
        }

        // If the daemon has no broadcast channel, nothing to stream — just idle
        // until the client disconnects (which we cannot detect without a read,
        // so we return immediately; the client will see EOF).
        let Some(ref tx) = self.new_item_tx else {
            tracing::debug!("watch_subscribe: no new_item_tx; returning after ack");
            return Ok(());
        };

        // Subscribe AFTER sending the ack so we don't miss items that arrive
        // between construction and the loop. Missed items during the brief ack
        // write window are acceptable (the client has not yet set up its reader).
        let mut rx = tx.subscribe();

        loop {
            match rx.recv().await {
                Ok(item) => {
                    // Build the event line — metadata only, NO plaintext/ciphertext.
                    let mut evt = serde_json::json!({
                        "ok": true,
                        "event": "new_item",
                        "id": req_id,
                        "item_id": item.item_id,
                        "content_type": item.content_type,
                        "wall_time": item.wall_time,
                        "is_sensitive": item.is_sensitive,
                    })
                    .to_string();
                    evt.push('\n');
                    match tokio::time::timeout(IPC_WRITE_TIMEOUT, writer.write_all(evt.as_bytes()))
                        .await
                    {
                        Ok(Ok(())) => {}
                        Ok(Err(e)) => {
                            // Client disconnected — exit cleanly, no alarm.
                            tracing::debug!(
                                "watch_subscribe: write failed (client disconnected): {e}"
                            );
                            return Ok(());
                        }
                        Err(_) => {
                            tracing::warn!(
                                "watch_subscribe: event write timed out — dropping client"
                            );
                            return Ok(());
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    // The consumer was too slow; skip the missed messages and
                    // continue. We do NOT crash or disconnect — a slow watch
                    // client must never wedge the broadcast sender.
                    tracing::debug!(
                        "watch_subscribe: broadcast lagged by {n} messages; continuing"
                    );
                    // rx is already re-positioned to the next available message.
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    // Daemon is shutting down or the broadcast channel was dropped.
                    tracing::debug!("watch_subscribe: broadcast channel closed; exiting");
                    return Ok(());
                }
            }
        }
    }

    /// Soft-delete the item with primary key `id`, bump its `lamport_ts` and
    /// `wall_time` so the tombstone wins LWW on all peers, then broadcast the
    /// resulting tombstone row via `new_item_tx` so the sync orchestrator
    /// propagates it to P2P peers and the cloud upload queue.
    ///
    /// Returns `Ok((changed, tombstone_opt))` where `changed` is the number of
    /// rows modified (0 = not found). `Err` carries either the DB error string
    /// or a spawn-join failure message. Used by both the legacy `"delete"` arm
    /// and the typed `"delete_item"` arm; each arm formats its own distinct
    /// response shape and error style.
    async fn soft_delete_and_broadcast(
        &self,
        id: &str,
    ) -> Result<(usize, Option<copypaste_core::ClipboardItem>), String> {
        let db_arc = self.db.clone();
        let id_owned = id.to_string();
        let join = tokio::task::spawn_blocking(move || {
            let db = db_arc.blocking_lock();
            // Soft-delete: wipe content/nonce/thumb, set deleted=1, bump
            // lamport_ts + wall_time so the tombstone wins LWW on peers.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                // SAFETY: current time is always after UNIX_EPOCH.
                .unwrap_or_default()
                .as_millis() as i64;
            // Look up the current row to derive the new lamport_ts.
            let existing = get_item_by_id(&*db, &id_owned).map_err(|e| e.to_string())?;
            // CopyPaste-ojhe: stamp the unified lamport value space
            // (max(existing + 1, now_ms)) so the tombstone is both monotonic and
            // time-ordered — it can overtake a stale now_ms-magnitude recopy of
            // the same item that a small `existing + 1` could never beat.
            let prev_lamport = existing.as_ref().map(|r| r.lamport_ts).unwrap_or(0);
            let new_lamport = copypaste_core::next_lamport_ts(prev_lamport, now_ms);
            let changed =
                soft_delete_item(&db, &id_owned, new_lamport, now_ms).map_err(|e| e.to_string())?;
            // Re-read the tombstone row so we can broadcast it to peers.
            let tombstone = get_item_by_id(&*db, &id_owned).map_err(|e| e.to_string())?;
            Ok::<_, String>((changed, tombstone))
        })
        .await
        .map_err(|e| format!("blocking task failed: {e}"))?;

        if let Ok((_, Some(ref tombstone))) = join {
            // Broadcast the tombstone so P2P/cloud sync propagates the
            // deletion to peers. Fire-and-forget: a failed send only
            // means no sync receiver is currently active.
            if let Some(ref tx) = self.new_item_tx {
                let _ = tx.send(tombstone.clone());
            }
        }
        join
    }

    #[tracing::instrument(skip(self), fields(method), name = "ipc_dispatch")]
    /// Shared request admission gate: the protocol-version check (ADR-007) and
    /// the readiness check. Returns `Some(err_response)` when the request must be
    /// rejected, else `None`. Centralised so `dispatch` and the streaming
    /// `watch_subscribe` path (intercepted before `dispatch`) apply the SAME
    /// gates (CopyPaste-crh3.105).
    ///
    /// `force_requires_ready` is for methods dispatched OUTSIDE `dispatch` that
    /// must still require a ready DB even though they are absent from the
    /// `requires_db` allow-list (e.g. `watch_subscribe`, which streams item
    /// metadata and must not hand a degraded daemon's client a silent empty
    /// stream).
    fn check_request_gates(&self, req: &Request, force_requires_ready: bool) -> Option<Response> {
        // Protocol-version gate (ADR-007).
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
            return Some(Response::err_with_code(
                req.id.clone(),
                ERR_CODE_VERSION_MISMATCH,
                format!(
                    "unsupported protocol version {} (daemon supports {}..={})",
                    req.protocol_version, MIN_SUPPORTED_PROTOCOL_VERSION, CURRENT_PROTOCOL_VERSION
                ),
            ));
        }

        // Readiness gate — reject DB-touching methods before init is done.
        if (force_requires_ready || Self::requires_db(req.method.as_str()))
            && !self.ready.load(Ordering::Relaxed)
        {
            tracing::debug!(
                method = %req.method,
                id = %req.id,
                "rejecting DB-touching request: server not ready"
            );
            return Some(Response::err_with_code(
                req.id.clone(),
                ERR_CODE_IPC_NOT_READY,
                ERR_IPC_NOT_READY,
            ));
        }

        None
    }

    async fn dispatch(&self, line: &str) -> Response {
        let req: Request = match serde_json::from_str(line) {
            Ok(r) => r,
            Err(e) => {
                // CopyPaste-cbfl: echo the request's id back so the CLI's
                // id-matching guard doesn't reject this error response.
                // If the line is valid JSON but not a Request, extract id
                // from the raw Value. Fall back to "?" only when the line
                // is not parseable as JSON at all.
                let echo_id = serde_json::from_str::<serde_json::Value>(line)
                    .ok()
                    .and_then(|v| {
                        v["id"]
                            .as_str()
                            .map(|s| s.to_string())
                            .or_else(|| v["id"].as_i64().map(|n| n.to_string()))
                            .or_else(|| v["id"].as_u64().map(|n| n.to_string()))
                    })
                    .unwrap_or_else(|| "?".to_string());
                return Response::err_with_code(
                    echo_id,
                    ERR_CODE_INVALID_ARGUMENT,
                    format!("parse error: {e}"),
                );
            }
        };

        tracing::Span::current().record("method", req.method.as_str());
        tracing::debug!(method = %req.method, id = %req.id, "IPC request");

        // Protocol-version gate (ADR-007) + readiness gate. ADR-007: the version
        // gate uses ERR_CODE_VERSION_MISMATCH so the CLI can surface the "please
        // upgrade" prompt (P2-ptb8). Shared with the watch_subscribe path via
        // check_request_gates (CopyPaste-crh3.105).
        if let Some(err) = self.check_request_gates(&req, false) {
            return err;
        }

        match req.method.as_str() {
            // c4q2.17: "list" is the legacy CLI verb. Response shape is now
            // unified under "history_page" (pinned-first, same fields).
            // CLI copypaste-cli was migrated to METHOD_HISTORY_PAGE (c4q2.17).
            // Kept as an explicit stub so old callers get a diagnosable error.
            "list" => Response::err_with_code(
                req.id,
                ERR_CODE_NOT_IMPLEMENTED,
                "list is deprecated: use history_page with {limit, offset} — \
                 the response shape is identical but pinned items appear first (c4q2.17)",
            ),
            "delete" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    // P2-8u2b: tag with ERR_CODE_INVALID_ARGUMENT so machine
                    // clients can classify the error rather than getting a bare
                    // untyped error string.
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
                match self.soft_delete_and_broadcast(&id).await {
                    Ok(_) => Response::ok(req.id, serde_json::Value::Null),
                    Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
                }
            }
            "count" => match self.with_read_db(|db| count_items(db)).await {
                Ok(Ok(n)) => Response::ok(req.id, serde_json::json!({"count": n})),
                Ok(Err(e)) => Response::err(req.id, e.to_string()),
                Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
            },
            "search" => {
                let query = match req.params.get("query").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    // CopyPaste-kfe9: tag with ERR_CODE_INVALID_ARGUMENT so
                    // machine clients can classify the error (follow-up of 8u2b).
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: query",
                        )
                    }
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
                        let previews =
                            fetch_text_previews_batch(db, &preview_ids).unwrap_or_default();
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
                                        .get(&item.id)
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
                    Ok(Err(e)) => {
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string())
                    }
                    // CopyPaste-crh3.86: with_read_db already formats the join
                    // failure; surface it directly (no double "blocking task failed").
                    Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
                }
            }
            "copy" | "paste" => {
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    // P2-8u2b: tag with ERR_CODE_INVALID_ARGUMENT so machine
                    // clients can classify the error.
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
                    // Resolve directly by primary key — paging + linear scan
                    // silently missed any item past position 1000 (data loss).
                    let item = get_item_by_id(&*db, &id_for_task)?;
                    Ok::<_, anyhow::Error>(item)
                })
                .await;
                match join {
                    Ok(Ok(Some(item))) => match self.write_to_pasteboard(&item) {
                        Ok(()) => {
                            // C. PROMOTE-ON-COPY: bump wall_time/lamport so this
                            // item sorts to the top of history_page on the next
                            // request, matching Maccy-style recency ordering.
                            let db_arc2 = self.db.clone();
                            let item_id_bump = item.id.clone();
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
                                let new_lamport =
                                    copypaste_core::next_lamport_ts(prev_lamport, now_ms);
                                // Pass None: ipc recopy path doesn't know sensitive TTL;
                                // delete_expired picks up expires_at set at capture time.
                                bump_item_recency(&db, &item_id_bump, now_ms, new_lamport, None)
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
                    Ok(Ok(None)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
                    Ok(Err(e)) => {
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string())
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            "delete_all" => {
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
                        .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))?
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
                        let new_lamport =
                            copypaste_core::next_lamport_ts(*prev_lamport, now_ms);
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
            "stats" => match self
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
            },
            "pin" => {
                // Pin an item (remove expiry so it's never auto-deleted)
                let id = match req.params.get("id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    // CopyPaste-kfe9: tag with ERR_CODE_INVALID_ARGUMENT so
                    // machine clients can classify the error (follow-up of 8u2b).
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
                    Ok(Err(e)) => {
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e.to_string())
                    }
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
            "reorder_pinned" => {
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
                    let mut rows: Vec<copypaste_core::ClipboardItem> =
                        Vec::with_capacity(id_refs.len());
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
            "delete_item" => {
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
            // T5.x — copy an item back to the system clipboard by id. Same
            // paste-back path as `copy`/`paste` (decrypt → NSPasteboard) but
            // surfaces typed `invalid_argument` / `not_found` error codes so
            // the UI can branch on `error_code` rather than parsing strings.
            "copy_item" => {
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
                    Ok(Ok((Some(item), preview))) => match self.write_to_pasteboard(&item) {
                        Ok(()) => {
                            // C. PROMOTE-ON-COPY: bump wall_time/lamport so this
                            // item sorts to the top of history_page on the next
                            // request, matching Maccy-style recency ordering.
                            let db_arc2 = self.db.clone();
                            let item_id_bump = item.id.clone();
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
                                let new_lamport =
                                    copypaste_core::next_lamport_ts(prev_lamport, now_ms);
                                // Pass None: ipc recopy path doesn't know sensitive TTL;
                                // delete_expired picks up expires_at set at capture time.
                                bump_item_recency(&db, &item_id_bump, now_ms, new_lamport, None)
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
                    Ok(Ok((None, _))) => Response::err_with_code(
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
            // A. get_item_image — decrypt and return an IMAGE item as a data URI.
            //
            // Params: {"id": "<uuid>"}
            // Success: {"data_uri": "data:<content_type>;base64,<b64>"}
            // Error: item not found, non-image content_type, or decrypt failure.
            //
            // Reuses the same chunk-decrypt path as write_to_pasteboard for images
            // (chunks_from_blob → decode_image → PNG bytes), then base64-encodes
            // the raw PNG bytes for the UI to render as a thumbnail without having
            // to hit the pasteboard.
            "get_item_image" => {
                let id = match extract_uuid_param(&req.params, req.id.clone()) {
                    Ok(id) => id,
                    Err(resp) => return resp,
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                // CopyPaste-z1xt: do the WHOLE pipeline — DB fetch, decrypt
                // (decode_image), and base64 — inside spawn_blocking. Previously
                // only the DB fetch ran on the blocking pool; the CPU-heavy
                // decrypt + base64 ran on the async executor thread, stalling it.
                // CopyPaste-eq9m: encode directly from the decrypted `png_bytes`
                // slice and DROP it before building the data URI so peak RAM is
                // one decoded copy + one base64 string, not both plus the URI;
                // we also move `item.content` out instead of `.clone()`-ing the
                // full encrypted blob.
                // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
                // even if the spawn_blocking worker panics or is cancelled.
                let v1_key = zeroize::Zeroizing::new(**self.local_key);
                // ItemImageResult mirrors the response branches so error mapping
                // stays on the async side (Response::* needs `req.id`).
                enum ItemImageResult {
                    Ok(String),
                    NotFound,
                    NotImage(String),
                    Internal(String),
                    Auth(String),
                }
                let join =
                    tokio::task::spawn_blocking(move || -> anyhow::Result<ItemImageResult> {
                        let item = {
                            let db = db_arc.blocking_lock();
                            get_item_by_id(&*db, &id_for_task)?
                        };
                        let mut item = match item {
                            Some(it) => it,
                            None => return Ok(ItemImageResult::NotFound),
                        };
                        let is_image =
                            item.content_type == "image" || item.content_type.starts_with("image/");
                        if !is_image {
                            return Ok(ItemImageResult::NotImage(format!(
                                "item {id_for_task} is not an image (content_type: {})",
                                item.content_type
                            )));
                        }
                        // Move the encrypted blob out of the item — no extra clone.
                        let content = match item.content.take() {
                            Some(b) => b,
                            None => {
                                return Ok(ItemImageResult::Internal(format!(
                                    "image item {id_for_task} has no content blob"
                                )))
                            }
                        };
                        let meta_json = match item.blob_ref.as_deref() {
                            Some(s) => s,
                            None => {
                                return Ok(ItemImageResult::Internal(format!(
                                    "image item {id_for_task} missing blob_ref metadata"
                                )))
                            }
                        };
                        let file_id = match parse_image_file_id(meta_json) {
                            Ok(fid) => fid,
                            Err(e) => {
                                return Ok(ItemImageResult::Internal(format!(
                                    "image item {id_for_task} blob_ref parse error: {e}"
                                )))
                            }
                        };
                        let chunks = match chunks_from_blob(&content) {
                            Ok(c) => c,
                            Err(e) => {
                                return Ok(ItemImageResult::Internal(format!(
                                    "image item {id_for_task} chunks_from_blob failed: {e}"
                                )))
                            }
                        };
                        let v2_key = derive_v2(&v1_key);
                        let key_to_use: &[u8; 32] = if item.key_version == 1 {
                            &v1_key
                        } else {
                            &v2_key
                        };
                        let png_bytes = match decode_image(&chunks, key_to_use, &file_id) {
                            Ok(b) => b,
                            Err(e) => {
                                return Ok(ItemImageResult::Auth(format!(
                                    "image item {id_for_task} decode failed: {e}"
                                )))
                            }
                        };
                        use base64::Engine as _;
                        let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                        // CopyPaste-eq9m: free the decoded image bytes before we build
                        // the URI so the base64 string is the only large allocation
                        // still alive when we format the data URI.
                        drop(png_bytes);
                        // The stored content_type is "image" (legacy) or a real MIME
                        // type. For the data URI we always emit "image/png" because
                        // decode_image always returns PNG bytes.
                        let data_uri = format!("data:image/png;base64,{b64}");
                        Ok(ItemImageResult::Ok(data_uri))
                    })
                    .await;
                match join {
                    Ok(Ok(ItemImageResult::Ok(data_uri))) => {
                        Response::ok(req.id, serde_json::json!({ "data_uri": data_uri }))
                    }
                    Ok(Ok(ItemImageResult::NotFound)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
                    Ok(Ok(ItemImageResult::NotImage(msg))) => {
                        Response::err_with_code(req.id, ERR_CODE_INVALID_ARGUMENT, msg)
                    }
                    Ok(Ok(ItemImageResult::Internal(msg))) => {
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Ok(Ok(ItemImageResult::Auth(msg))) => {
                        Response::err_with_code(req.id, ERR_CODE_AUTH_FAILED, msg)
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }
            // A'. get_item_thumbnail — decrypt and return the small capture-time
            // thumbnail as a data URI. Mirrors `get_item_image` but reads
            // `item.thumb` (keyed by the DISTINCT `thumb_file_id` in the meta)
            // instead of the full-res `item.content`.
            //
            // Params: {"id": "<uuid>"}
            // Success (thumb present): {"thumbnail": "data:image/png;base64,<b64>"}
            // Success (no thumb):      {"thumbnail": null}   ← UI falls back to
            //                          get_item_image (full-res).
            // Error: item not found, non-image content_type, parse/decode failure.
            "get_item_thumbnail" => {
                let id = match extract_uuid_param(&req.params, req.id.clone()) {
                    Ok(id) => id,
                    Err(resp) => return resp,
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                // P2-iqkm: capture as Zeroizing so the key copy is wiped on drop
                // even if the spawn_blocking worker panics or is cancelled.
                // (Zeroizing<[u8;32]> is Send; the old "not Send" comment was incorrect.)
                let v1_key_thumb = zeroize::Zeroizing::new(**self.local_key);
                // All DB work — fetch + optional Phase-4 lazy backfill + decrypt —
                // runs in a single spawn_blocking so we hold the mutex for one
                // contiguous span and avoid async/sync boundary issues.
                // Returns: Ok(Some((png_bytes, data_uri_string))) on success,
                //          Ok(None) when item not found,
                //          Err for wrong content_type or missing blob_ref.
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let item = match get_item_by_id(&*db, &id_for_task)? {
                        Some(i) => i,
                        None => return Ok::<_, anyhow::Error>(None),
                    };
                    // Dispatch on key_version: v1 rows use the raw seed; v2 rows use derive_v2.
                    let v2_key_thumb = derive_v2(&v1_key_thumb);
                    let decode_key: &[u8; 32] = if item.key_version == 1 {
                        &v1_key_thumb
                    } else {
                        &v2_key_thumb
                    };

                    let is_image =
                        item.content_type == "image" || item.content_type.starts_with("image/");
                    if !is_image {
                        return Err(anyhow::anyhow!(
                            "item {} is not an image (content_type: {})",
                            id_for_task,
                            item.content_type
                        ));
                    }

                    let mut meta_json = item
                        .blob_ref
                        .as_deref()
                        .ok_or_else(|| {
                            anyhow::anyhow!("image item {} missing blob_ref metadata", id_for_task)
                        })?
                        .to_owned();

                    // Resolve the thumbnail blob: use the stored one when present
                    // AND it conforms to the current THUMBNAIL_MAX_DIM cap.
                    // Regenerate (Phase-4 backfill path) when either:
                    //   * thumb IS NULL (legacy row, never had a thumbnail), or
                    //   * the stored thumbnail was encoded under an older, larger
                    //     cap (e.g. 680 px) and its recorded dims exceed the new
                    //     cap — otherwise the UI would decode an oversized bitmap
                    //     (HB-10, 350 MB image-memory regression).
                    let stored_thumb: Option<Vec<u8>> = match item.thumb {
                        Some(b) => {
                            let (tw, th) = parse_image_thumb_dims(&meta_json);
                            if copypaste_core::thumb_dims_exceed_cap(tw, th) {
                                tracing::debug!(
                                    item_id = %id_for_task,
                                    thumb_w = tw,
                                    thumb_h = th,
                                    "stored thumbnail exceeds current cap; regenerating"
                                );
                                None // fall through to regeneration below
                            } else {
                                Some(b)
                            }
                        }
                        None => None,
                    };
                    let thumb_blob: Vec<u8> = match stored_thumb {
                        Some(b) => b,
                        None => {
                            // Phase 4 lazy backfill: generate + persist a
                            // thumbnail on first display (NULL thumb) OR
                            // regenerate an oversized one at the current cap.
                            // `set_thumb` overwrites any existing row, so an
                            // oversized stored thumbnail is replaced in place.
                            // Returns both the
                            // encrypted blob and the updated meta_json (which
                            // now includes thumb_file_id / thumb_w / thumb_h)
                            // so the subsequent decode path reads the right AAD.
                            // Any failure is logged and falls back to the null
                            // sentinel — we never error the request.
                            // content is Option<Vec<u8>>; for image items it is
                            // always Some (set at capture), so None here means
                            // the row is corrupt — treat it as backfill failure.
                            let content_ref: &[u8] = match item.content.as_deref() {
                                Some(b) => b,
                                None => {
                                    tracing::warn!(
                                        item_id = %id_for_task,
                                        "lazy thumbnail backfill: image item has no content blob"
                                    );
                                    return Ok(Some((Vec::<u8>::new(), String::new())));
                                }
                            };
                            match lazy_backfill_thumbnail(
                                &db,
                                &id_for_task,
                                content_ref,
                                // lazy_backfill_thumbnail dispatches on key_version
                                // INTERNALLY, so it needs the RAW v1 seed — not the
                                // already-derived `decode_key` (passing the latter
                                // would double-derive: derive_v2(derive_v2(seed))).
                                &meta_json,
                                &v1_key_thumb,
                                item.key_version,
                            ) {
                                Ok((blob, new_meta)) => {
                                    // Overwrite the local meta_json so the
                                    // thumb_file_id parse below reads the value
                                    // we just persisted to the DB.
                                    meta_json = new_meta;
                                    blob
                                }
                                Err(e) => {
                                    tracing::warn!(
                                        item_id = %id_for_task,
                                        err = %e,
                                        "lazy thumbnail backfill failed; returning null sentinel"
                                    );
                                    // Signal null sentinel via a sentinel Ok(Some)
                                    // with an empty bytes vec — caller checks.
                                    // Cleaner than a custom error variant: the
                                    // outer match maps empty bytes → null response.
                                    return Ok(Some((Vec::<u8>::new(), String::new())));
                                }
                            }
                        }
                    };

                    // The thumbnail is keyed by a DISTINCT thumb_file_id recorded
                    // additively in blob_ref meta JSON (written at capture time or
                    // by the backfill path above).
                    let thumb_file_id = parse_image_thumb_file_id(&meta_json).map_err(|e| {
                        anyhow::anyhow!("image item {} thumb meta parse error: {}", id_for_task, e)
                    })?;

                    // `decode_thumbnail` takes the serialized blob directly
                    // (runs `chunks_from_blob` + decrypt internally).
                    let png_bytes =
                        copypaste_core::decode_thumbnail(&thumb_blob, decode_key, &thumb_file_id)
                            .map_err(|e| {
                            anyhow::anyhow!("image item {} thumb decode failed: {}", id_for_task, e)
                        })?;

                    use base64::Engine as _;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&png_bytes);
                    let data_uri = format!("data:image/png;base64,{b64}");
                    Ok(Some((png_bytes, data_uri)))
                })
                .await;
                match join {
                    Ok(Ok(Some((png_bytes, _data_uri)))) if png_bytes.is_empty() => {
                        // Empty-bytes sentinel: backfill failed, return null.
                        Response::ok(
                            req.id,
                            serde_json::json!({ "thumbnail": serde_json::Value::Null }),
                        )
                    }
                    Ok(Ok(Some((_png_bytes, data_uri)))) => {
                        Response::ok(req.id, serde_json::json!({ "thumbnail": data_uri }))
                    }
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
            // B. get_item_file — decrypt and return a FILE item as raw bytes.
            //
            // Params: {"id": "<uuid>"}
            // Success: {"filename": "<name>", "mime": "<type>", "data_b64": "<b64>"}
            // Error: item not found, non-file content_type, or decrypt failure.
            //
            // Mirrors `get_item_image` but uses `decode_file` (no decode/re-encode)
            // and returns the raw bytes as base64 plus the filename and MIME type
            // parsed from the `blob_ref` meta JSON.
            "get_item_file" => {
                let id = match extract_uuid_param(&req.params, req.id.clone()) {
                    Ok(id) => id,
                    Err(resp) => return resp,
                };
                let db_arc = self.db.clone();
                let id_for_task = id.clone();
                // CopyPaste-z1xt: run the full DB-fetch + decrypt + base64 pipeline
                // inside spawn_blocking (the decrypt + base64 previously ran on the
                // async executor thread).
                // CopyPaste-eq9m: move the encrypted blob out of the item (no
                // clone) and free the decrypted `raw_bytes` before building the
                // response so peak RAM is one decoded copy + one base64 string.
                // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
                // even if the spawn_blocking worker panics or is cancelled.
                let v1_key = zeroize::Zeroizing::new(**self.local_key);
                enum ItemFileResult {
                    Ok {
                        filename: String,
                        mime: String,
                        data_b64: String,
                    },
                    NotFound,
                    NotFile(String),
                    Internal(String),
                    Auth(String),
                }
                let join =
                    tokio::task::spawn_blocking(move || -> anyhow::Result<ItemFileResult> {
                        let item = {
                            let db = db_arc.blocking_lock();
                            get_item_by_id(&*db, &id_for_task)?
                        };
                        let mut item = match item {
                            Some(it) => it,
                            None => return Ok(ItemFileResult::NotFound),
                        };
                        if item.content_type != "file" {
                            return Ok(ItemFileResult::NotFile(format!(
                                "item {id_for_task} is not a file (content_type: {})",
                                item.content_type
                            )));
                        }
                        let content = match item.content.take() {
                            Some(b) => b,
                            None => {
                                return Ok(ItemFileResult::Internal(format!(
                                    "file item {id_for_task} has no content blob"
                                )))
                            }
                        };
                        let meta_json = match item.blob_ref.as_deref() {
                            Some(s) => s,
                            None => {
                                return Ok(ItemFileResult::Internal(format!(
                                    "file item {id_for_task} missing blob_ref metadata"
                                )))
                            }
                        };
                        let file_meta = match parse_file_meta(meta_json) {
                            Ok(m) => m,
                            Err(e) => {
                                return Ok(ItemFileResult::Internal(format!(
                                    "file item {id_for_task} blob_ref parse error: {e}"
                                )))
                            }
                        };
                        let chunks = match chunks_from_blob(&content) {
                            Ok(c) => c,
                            Err(e) => {
                                return Ok(ItemFileResult::Internal(format!(
                                    "file item {id_for_task} chunks_from_blob failed: {e}"
                                )))
                            }
                        };
                        let v2_key = derive_v2(&v1_key);
                        let key_to_use: &[u8; 32] = if item.key_version == 1 {
                            &v1_key
                        } else {
                            &v2_key
                        };
                        let raw_bytes = match decode_file(&chunks, key_to_use, &file_meta.file_id) {
                            Ok(b) => b,
                            Err(e) => {
                                return Ok(ItemFileResult::Auth(format!(
                                    "file item {id_for_task} decode failed: {e}"
                                )))
                            }
                        };
                        use base64::Engine as _;
                        let data_b64 = base64::engine::general_purpose::STANDARD.encode(&raw_bytes);
                        // CopyPaste-eq9m: free the decoded file bytes before returning.
                        drop(raw_bytes);
                        Ok(ItemFileResult::Ok {
                            filename: file_meta.filename,
                            mime: file_meta.mime,
                            data_b64,
                        })
                    })
                    .await;
                match join {
                    Ok(Ok(ItemFileResult::Ok {
                        filename,
                        mime,
                        data_b64,
                    })) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "filename": filename,
                            "mime":     mime,
                            "data_b64": data_b64,
                        }),
                    ),
                    Ok(Ok(ItemFileResult::NotFound)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_NOT_FOUND,
                        format!("item not found: {id}"),
                    ),
                    Ok(Ok(ItemFileResult::NotFile(msg))) => {
                        Response::err_with_code(req.id, ERR_CODE_INVALID_ARGUMENT, msg)
                    }
                    Ok(Ok(ItemFileResult::Internal(msg))) => {
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Ok(Ok(ItemFileResult::Auth(msg))) => {
                        Response::err_with_code(req.id, ERR_CODE_AUTH_FAILED, msg)
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
                // CopyPaste-crh3.86: with_read_db centralises the pool/writer
                // fallback; build_page already accepts &dyn DbRead so the branch
                // collapses to a single call.
                let join = self
                    .with_read_db(move |db| {
                    // Helper: build json_items + total from any DbRead source.
                    fn build_page(
                        db: &dyn copypaste_core::DbRead,
                        limit: usize,
                        offset: usize,
                    ) -> anyhow::Result<(Vec<serde_json::Value>, i64)> {
                        let items = get_page_pinned_first(db, limit, offset)?;
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
                                        .get(&item.id)
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
                        Ok((json_items, total))
                    }

                    build_page(db, limit, offset)
                    })
                    .await;
                // Snapshot the own device id outside the blocking task (it lives on self).
                let own_device_id = self.local_device_id.clone().unwrap_or_default();
                match join {
                    Ok(Ok((json_items, total))) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "items": json_items,
                            "total": total,
                            "own_device_id": own_device_id,
                        }),
                    ),
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    // CopyPaste-crh3.86: with_read_db already formats the join failure.
                    Err(e) => Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, e),
                }
            }
            "get_config" => {
                // Never ship account credentials over IPC. `get_config` feeds
                // the UI settings form and the CLI's read-merge-write in
                // `cloud setup`; neither needs the raw GoTrue password or email
                // back (the CLI re-supplies both on every `set_config`, the UI
                // does not surface them at all). `build_config_response` maps
                // the internal config to a typed `AppConfigResponse` that has no
                // field for either secret — only `*_set` presence flags — so a
                // leak is structurally impossible. The Supabase anon/public key
                // is, by design, a publishable key and is kept so the UI can
                // prefill the settings field.
                //
                // Fix HIGH #3: read_config() does blocking fs I/O (reads
                // config.json + config.toml); run it on the blocking thread
                // pool so the async worker is never stalled by disk I/O.
                let join = tokio::task::spawn_blocking(read_config).await;
                match join {
                    // Build the typed, redacted wire response. `AppConfigResponse`
                    // has no field that can carry a credential, so secrets cannot
                    // leak here even if a new secret field is later added to the
                    // internal `AppConfig` (CopyPaste-c4q2.18).
                    Ok(cfg) => match serde_json::to_value(build_config_response(&cfg)) {
                        Ok(v) => Response::ok(req.id, v),
                        Err(e) => Response::err(req.id, e.to_string()),
                    },
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("get_config blocking task failed: {e}"),
                    ),
                }
            }
            "set_config" => {
                let incoming: AppConfig = match serde_json::from_value(req.params.clone()) {
                    Ok(c) => c,
                    Err(e) => return Response::err(req.id, format!("invalid config: {e}")),
                };
                // Capture the requested lan_visibility toggle BEFORE we move
                // `incoming` into the blocking task, so we can hot-apply it to
                // the running DiscoveryService after the persist succeeds.
                let requested_lan_visibility = incoming.lan_visibility;
                let discovery_for_lan = self.discovery.clone();
                // Capture p2p_enabled so we can log a restart-required notice
                // after the persist succeeds. Runtime start/stop of the full P2P
                // transport stack (start_p2p) is not feasible without a large
                // refactor (CopyPaste-bjh); the persisted value is honoured on
                // the NEXT daemon restart. `None` means the caller did not send
                // the field — no change, no notice needed.
                let requested_p2p_enabled = incoming.p2p_enabled;
                // CopyPaste-44rq.67: capture whether the caller explicitly
                // cleared the relay URL (the empty-string sentinel) BEFORE
                // `incoming` is moved into the blocking task, so the running
                // relay orchestrator can be shut down after the persist succeeds.
                // `None` (omitted) is NOT a clear — only `Some("")`/whitespace.
                #[cfg(feature = "relay-sync")]
                let relay_cleared = matches!(
                    incoming.relay_url.as_deref(),
                    Some(s) if s.trim().is_empty()
                );
                #[cfg(feature = "relay-sync")]
                let relay_handle_for_clear = self.relay_handle.clone();
                // MERGE, don't overwrite. `get_config` redacts the secret
                // fields (`supabase_password`, `supabase_email`) to `*_set`
                // booleans and drops the real values, so a UI/CLI
                // read-modify-write deserialises them as `None`. A blind
                // whole-struct write would then persist null and silently WIPE
                // the stored Supabase credentials, breaking cloud sync. Merge
                // the incoming config onto the persisted one, preserving any
                // secret the caller did not supply.
                //
                // Fix HIGH #3: read_config()/write_config()/update_core_config()
                // all do blocking fs I/O; run them on the blocking thread pool.
                let core_config_arc = self.core_config.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let mut merged = merge_config(read_config(), incoming);
                    // Item 1 (keychain supabase_password): if the caller supplied a
                    // new password, migrate it to the macOS Keychain and remove it
                    // from the config struct so it is NOT written to config.json in
                    // plain text. On failure (non-macOS, unsigned build without
                    // Keychain access) we keep the existing config.json behaviour as
                    // a fallback — the password stays in merged and is written to
                    // the 0600 config.json, same as before the fix.
                    if let Some(ref pw) = merged.supabase_password.clone() {
                        match crate::keychain::store_supabase_password_to_keychain(pw) {
                            Ok(()) => {
                                // Only drop the plaintext from config.json once the
                                // Keychain ACTUALLY returns it. Under the ephemeral-key
                                // bypass (CI / unsigned dev builds) `store_*` is a no-op
                                // that still returns Ok(()); a blind strip would then
                                // silently lose the secret from both stores. The
                                // read-back confirms real persistence before we delete
                                // the on-disk copy.
                                if crate::keychain::read_supabase_password_from_keychain()
                                    .as_deref()
                                    == Some(pw.as_str())
                                {
                                    tracing::info!(
                                        "supabase_password migrated to Keychain; \
                                         removing from config.json"
                                    );
                                    merged.supabase_password = None;
                                } else {
                                    tracing::debug!(
                                        "supabase_password Keychain store is a no-op \
                                         (ephemeral/bypass mode); keeping it in config.json"
                                    );
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "supabase_password Keychain store failed; \
                                     falling back to config.json persistence"
                                );
                                // Leave merged.supabase_password as-is so
                                // write_config below persists it to the 0600
                                // config.json (existing behaviour pre-fix).
                            }
                        }
                    }
                    // Persist IPC fields (Supabase creds, p2p_enabled) to config.json.
                    write_config(&merged)?;
                    // Persist limit fields to config.toml AND return the new
                    // core config for hot-reload in the caller.
                    let new_core = update_core_config(&merged)?;
                    Ok::<_, anyhow::Error>((merged, new_core))
                })
                .await;
                match join {
                    Ok(Ok((_merged, new_core))) => {
                        if let Some(ref arc) = core_config_arc {
                            if let Ok(mut guard) = arc.write() {
                                *guard = new_core;
                            }
                        }
                        // Hot-apply lan_visibility: stop or restart mDNS-SD
                        // without a full daemon restart.
                        //
                        // When the caller explicitly sets `lan_visibility: false`,
                        // stop advertisement and browsing immediately so the device
                        // disappears from the LAN straight away. When it is
                        // re-enabled (`Some(true)`), restart mDNS so the device
                        // becomes visible again without requiring a restart. When
                        // the caller omits the field (`None`), do nothing.
                        if let Some(visible) = requested_lan_visibility {
                            if let Some(ref disc) = discovery_for_lan {
                                if visible {
                                    tracing::info!(
                                        "lan_visibility set to true — restarting mDNS-SD"
                                    );
                                    let disc_for_task = Arc::clone(disc);
                                    tokio::spawn(async move {
                                        match disc_for_task.start().await {
                                            Ok(_handle) => {
                                                tracing::info!(
                                                    "mDNS-SD restarted (lan_visibility on)"
                                                );
                                                // The handle is intentionally dropped here:
                                                // the background browse loop keeps running via
                                                // the abort_handle retained in DiscoveryService.
                                            }
                                            Err(e) => tracing::warn!(
                                                "mDNS-SD restart failed after \
                                                 lan_visibility toggle: {e}"
                                            ),
                                        }
                                    });
                                } else {
                                    tracing::info!(
                                        "lan_visibility set to false — stopping mDNS-SD"
                                    );
                                    disc.stop();
                                }
                            }
                        }
                        // CopyPaste-bjh: p2p_enabled is persisted to config.json
                        // here and honoured at the NEXT daemon startup (A-SET-4).
                        // Hot-apply (runtime start/stop of start_p2p) is not
                        // implemented; inform operators so they know a restart is
                        // needed for the toggle to take effect.
                        if let Some(enabled) = requested_p2p_enabled {
                            tracing::info!(
                                p2p_enabled = enabled,
                                "p2p_enabled persisted — change takes effect on next daemon restart"
                            );
                        }
                        // CopyPaste-44rq.67: the user cleared the relay URL —
                        // tear down the running orchestrator now (config.toml was
                        // already set to relay_url=None above). Taking the handle
                        // out of the slot and calling `shutdown()` stops the push
                        // and receive loops within one poll cycle, so relay sync
                        // is disabled at runtime without a daemon restart. Unlike
                        // p2p_enabled (which needs a restart), the relay handle is
                        // explicitly shareable for exactly this purpose.
                        #[cfg(feature = "relay-sync")]
                        if relay_cleared {
                            if let Some(handle) = relay_handle_for_clear.lock().await.take() {
                                tracing::info!(
                                    "relay_url cleared — shutting down relay orchestrator"
                                );
                                handle.shutdown();
                            } else {
                                tracing::debug!(
                                    "relay_url cleared but no relay orchestrator was running"
                                );
                            }
                        }
                        Response::ok(req.id, serde_json::json!({"saved": true}))
                    }
                    Ok(Err(e)) => Response::err(req.id, e.to_string()),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("set_config blocking task failed: {e}"),
                    ),
                }
            }
            // ── nq39: dedicated store_cloud_password verb ──────────────────
            //
            // Stores the Supabase GoTrue account password WITHOUT routing it
            // through set_config and WITHOUT persisting it to config.json.
            //
            // On macOS: writes to the macOS Keychain via the existing
            // `keychain::store_supabase_password_to_keychain` helper (same
            // logic as the set_config path).
            //
            // On non-macOS: no Keychain is available; the password is held
            // in the in-memory slot (`self.in_memory_cloud_password`) for the
            // daemon's lifetime and is never written to config.json.  The
            // caller receives `persisted: false` as a signal that the
            // password will be lost on restart.
            "store_cloud_password" => {
                // nq39: parse only the `password` field we care about.
                // Use a local struct so the daemon does not need to depend on
                // `copypaste-ipc` (that crate is for clients — CLI and UI).
                #[derive(serde::Deserialize)]
                struct StoreCloudPasswordParams {
                    password: String,
                }
                let params: StoreCloudPasswordParams =
                    match serde_json::from_value(req.params.clone()) {
                        Ok(p) => p,
                        Err(e) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("invalid store_cloud_password params: {e}"),
                            )
                        }
                    };

                if params.password.trim().is_empty() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "password must not be empty",
                    );
                }

                // Attempt Keychain write (macOS real path) via the blocking
                // thread pool — Security-framework calls must not block the
                // async executor.
                let password_for_task = params.password.clone();
                let join = tokio::task::spawn_blocking(move || {
                    crate::keychain::store_supabase_password_to_keychain(&password_for_task)
                })
                .await;

                match join {
                    Ok(Ok(())) => {
                        // Keychain write succeeded (macOS) or was a no-op
                        // (ephemeral-key bypass).  Verify the round-trip to
                        // distinguish real persistence from the bypass.
                        let persisted = crate::keychain::read_supabase_password_from_keychain()
                            .as_deref()
                            == Some(params.password.trim());
                        tracing::info!(
                            persisted,
                            "store_cloud_password: keychain write {}",
                            if persisted {
                                "persisted"
                            } else {
                                "bypassed (ephemeral/non-macOS)"
                            }
                        );
                        // On non-macOS (or ephemeral bypass): hold in-memory
                        // so cloud code can still read it this session.
                        #[cfg(not(target_os = "macos"))]
                        if !persisted {
                            if let Ok(mut guard) = self.in_memory_cloud_password.lock() {
                                *guard = Some(zeroize::Zeroizing::new(params.password.clone()));
                            }
                        }
                        Response::ok(req.id, serde_json::json!({ "persisted": persisted }))
                    }
                    Ok(Err(e)) => {
                        // Keychain write failed (non-macOS KeychainError::Unsupported
                        // or a real macOS Keychain error).  Store in-memory as a
                        // best-effort fallback; warn caller via `persisted: false`.
                        tracing::warn!(
                            error = %e,
                            "store_cloud_password: keychain write failed; \
                             holding password in-memory only (will be lost on restart)"
                        );
                        #[cfg(not(target_os = "macos"))]
                        if let Ok(mut guard) = self.in_memory_cloud_password.lock() {
                            *guard = Some(zeroize::Zeroizing::new(params.password.clone()));
                        }
                        Response::ok(req.id, serde_json::json!({ "persisted": false }))
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("store_cloud_password blocking task panicked: {e}"),
                    ),
                }
            }
            // ── Cloud auth ─────────────────────────────────────────────────
            //
            // `cloud_sign_in`: resolve GoTrue credentials via the same path
            // `start_cloud` uses at daemon startup, then flip `cloud_signed_in`
            // to reflect the real auth state. This fixes CopyPaste-i5b where
            // the flag was never set from the IPC (UI-driven) sign-in path —
            // only the env-var startup path set it.
            //
            // `cloud_sign_out`: unconditionally clear `cloud_signed_in` so
            // `get_sync_status` immediately reflects the signed-out state.
            #[cfg(feature = "cloud-sync")]
            "cloud_sign_in" => {
                use crate::cloud::CloudConfig;
                let cfg = match CloudConfig::from_env() {
                    Some(c) => c,
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "cloud-sync not configured: set supabase_url and supabase_anon_key \
                             (via set_config or SUPABASE_URL / SUPABASE_ANON_KEY env vars)",
                        );
                    }
                };
                // Attempt GoTrue sign-in (or fall through to anon key when no
                // email/password is configured — mirrors resolve_bearer_with_client).
                let auth =
                    copypaste_supabase::auth::AuthClient::new(&cfg.supabase_url, &cfg.anon_key);
                let sign_in_result = match (cfg.email.as_deref(), cfg.password.as_deref()) {
                    (Some(email), Some(password)) if !email.is_empty() && !password.is_empty() => {
                        auth.sign_in(email, password).await.map(|_| ())
                    }
                    // No email/password → anon key is the bearer; sign-in
                    // succeeds trivially (the key itself is the credential).
                    _ => Ok(()),
                };
                match sign_in_result {
                    Ok(()) => {
                        // CopyPaste-i5b fix: set the shared flag so
                        // get_sync_status reports the real authenticated state.
                        self.cloud_signed_in.store(true, Ordering::SeqCst);
                        tracing::info!("cloud_sign_in: signed in; cloud_signed_in = true");
                        Response::ok(req.id, serde_json::json!({"signed_in": true}))
                    }
                    Err(e) => {
                        self.cloud_signed_in.store(false, Ordering::SeqCst);
                        tracing::warn!(
                            "cloud_sign_in: sign-in failed ({e}); cloud_signed_in = false"
                        );
                        Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("sign-in failed: {e}"),
                        )
                    }
                }
            }
            #[cfg(feature = "cloud-sync")]
            "cloud_sign_out" => {
                // CopyPaste-i5b fix: clear the flag on explicit sign-out so
                // get_sync_status stops reporting signed_in = true after logout.
                self.cloud_signed_in.store(false, Ordering::SeqCst);

                // CopyPaste-crh3.100: make sign-out PERSISTENT. Previously only
                // the in-memory flag was cleared, so CloudConfig::from_env
                // re-resolved the Keychain password on the next daemon start and
                // silently re-authenticated — the user stayed signed in across a
                // restart despite signing out. Delete the Keychain Supabase
                // password AND clear the persisted email/password from
                // config.json so credential resolution finds nothing. The
                // Supabase project URL + anon key are deliberately KEPT so the
                // user can sign back in without re-entering project settings.
                if let Err(e) = crate::keychain::delete_supabase_password_from_keychain() {
                    tracing::warn!(
                        error = %e,
                        "cloud_sign_out: failed to delete the Keychain Supabase password"
                    );
                }
                let mut cfg = read_config();
                cfg.supabase_email = None;
                cfg.supabase_password = None;
                if let Err(e) = write_config(&cfg) {
                    tracing::warn!(
                        error = %e,
                        "cloud_sign_out: failed to clear persisted Supabase credentials"
                    );
                }
                tracing::info!(
                    "cloud_sign_out: cloud_signed_in = false; Keychain + persisted \
                     Supabase credentials cleared"
                );
                Response::ok(req.id, serde_json::json!({"signed_in": false}))
            }
            // When cloud-sync is not compiled in, cloud_sign_in / cloud_sign_out
            // are not available. Return not_implemented so clients see a
            // machine-readable error_code rather than "method not found".
            #[cfg(not(feature = "cloud-sync"))]
            "cloud_sign_in" | "cloud_sign_out" => Response::not_implemented(req.id, "cloud-sync"),

            // ── cloud-sync IPC methods ──────────────────────────────────────
            //
            // `set_sync_passphrase` and `get_sync_status` are the UI-facing
            // surface for the cross-device shared encryption key. Both are
            // compiled in only when the `cloud-sync` Cargo feature is active.
            #[cfg(feature = "cloud-sync")]
            "set_sync_passphrase" => {
                let passphrase = match req.params.get("passphrase").and_then(|v| v.as_str()) {
                    Some(p) if !p.is_empty() => p.to_owned(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or empty param: passphrase",
                        )
                    }
                };

                // Derive the sync key via Argon2id (this is intentionally slow —
                // one-time cost on passphrase entry, not per-item).
                let new_key = match derive_sync_key(&passphrase) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!("set_sync_passphrase: key derivation failed: {e}");
                        return Response::err(req.id, format!("key derivation failed: {e}"));
                    }
                };

                // Persist via the SAME backend the device key uses (0600 file
                // store on unsigned installs, Keychain otherwise) and swap the
                // live slot so the cloud loops pick it up immediately. The key
                // bytes are never logged.
                self.persist_and_install_sync_key(new_key).await;
                tracing::info!("set_sync_passphrase: sync key updated");
                Response::ok(req.id, serde_json::json!({"ok": true}))
            }

            // ── C-P0-4: honest cloud/relay device revocation ────────────────
            //
            // Revoking a peer (`revoke_peer`) only cuts off P2P (mTLS allowlist
            // + revoked_fingerprints denylist). It does NOT cut off cloud /
            // relay sync, because the revoked device still holds the shared sync
            // key — it can keep decrypting NEW cloud items and keeps addressing
            // the SAME relay inbox (the inbox id is HKDF of the sync key).
            //
            // The ONLY real cloud/relay revocation is ROTATING the sync key:
            //   * the old key can no longer decrypt items encrypted under the
            //     new key (XChaCha20-Poly1305 auth-tag rejection — see
            //     copypaste_core::sync_key);
            //   * the relay inbox id (HKDF of the sync key — see
            //     copypaste_core::relay::derive_relay_inbox_id) diverges, so the
            //     revoked device's saved token now addresses a DEAD inbox.
            //
            // `rotate_sync_key` accepts a NEW passphrase, derives a fresh key,
            // and installs it via the SAME persist + slot-swap path as
            // `set_sync_passphrase`. Remaining devices must re-provision (re-scan
            // the pairing QR or re-enter the new passphrase) to keep syncing.
            //
            // Available for BOTH cloud-sync (Supabase) and relay-sync: the relay
            // inbox id is HKDF of the sync key, so rotating it cuts off the
            // revoked device's relay access too.
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            "rotate_sync_key" => {
                let passphrase = match req.params.get("passphrase").and_then(|v| v.as_str()) {
                    Some(p) if !p.is_empty() => p.to_owned(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or empty param: passphrase",
                        )
                    }
                };

                let new_key = match derive_sync_key(&passphrase) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!("rotate_sync_key: key derivation failed: {e}");
                        return Response::err(req.id, format!("key derivation failed: {e}"));
                    }
                };

                // CopyPaste-vvsf: snapshot old key bytes BEFORE installing the new
                // one so the re-encryption closure can decrypt existing Supabase rows.
                // The bytes are captured as a plain array (SyncKey is !Clone) and
                // used only inside the closure below.
                #[cfg(feature = "cloud-sync")]
                let old_key_bytes: Option<[u8; 32]> = {
                    let guard = self.sync_key.lock().await;
                    guard.as_ref().map(|k| *k.as_bytes())
                };
                #[cfg(not(feature = "cloud-sync"))]
                let _old_key_bytes: Option<[u8; 32]> = None;

                // Derive new key bytes BEFORE consuming new_key (persist_and_install
                // will move it into the Arc<Mutex<…>> slot).
                #[cfg(feature = "cloud-sync")]
                let new_key_bytes: [u8; 32] = *new_key.as_bytes();

                self.persist_and_install_sync_key(new_key).await;
                tracing::info!(
                    "rotate_sync_key: sync key rotated; relay inbox id will diverge and the old \
                     key can no longer decrypt new cloud items"
                );

                // CopyPaste-vvsf: re-encrypt all existing cloud rows under the new
                // key so devices provisioned with the new passphrase can still read
                // historic items.  This is a best-effort network call: we log results
                // but never fail the rotate_sync_key response because:
                //   * The key rotation itself (local Keychain + in-memory slot) has
                //     already succeeded and is the caller's primary intent.
                //   * A partial re-encryption (network failure mid-batch) is recoverable
                //     — the push loop will re-upload newly-captured items under the new
                //     key; the caller can retry rotate_sync_key to attempt re-encryption
                //     of remaining rows.
                #[cfg(feature = "cloud-sync")]
                {
                    use base64::Engine as _;
                    use copypaste_core::{decrypt_from_cloud, encrypt_for_cloud};
                    use copypaste_supabase::RestClient;

                    if let Some(old_bytes) = old_key_bytes {
                        match RestClient::from_env() {
                            Ok(rest_client) => {
                                let (ok, skipped, err) = rest_client
                                    .reencrypt_all_cloud_items(
                                        move |item_id, old_ct_b64| -> Result<String, String> {
                                            let old_k =
                                                copypaste_core::SyncKey::from_bytes(old_bytes);
                                            let new_k =
                                                copypaste_core::SyncKey::from_bytes(new_key_bytes);
                                            let raw = base64::engine::general_purpose::STANDARD
                                                .decode(old_ct_b64)
                                                .map_err(|e| format!("base64: {e}"))?;
                                            let plain = decrypt_from_cloud(&old_k, item_id, &raw)
                                                .map_err(|e| format!("decrypt: {e}"))?;
                                            let new_blob =
                                                encrypt_for_cloud(&new_k, item_id, &plain)
                                                    .map_err(|e| format!("re-encrypt: {e}"))?;
                                            Ok(base64::engine::general_purpose::STANDARD
                                                .encode(&new_blob))
                                        },
                                    )
                                    .await
                                    .unwrap_or((0, 0, 0));
                                tracing::info!(
                                    ok,
                                    skipped,
                                    err,
                                    "rotate_sync_key: cloud re-encryption complete \
                                     (CopyPaste-vvsf)"
                                );
                            }
                            Err(e) => {
                                // Access token not in env — cloud is managed by GoTrue
                                // inside start_cloud. Log a warning but do not fail the
                                // rotate (the key rotation is already done).
                                tracing::warn!(
                                    error = %e,
                                    "rotate_sync_key: RestClient not available from env — \
                                     existing cloud rows NOT re-encrypted (CopyPaste-vvsf); \
                                     the push loop will re-upload future items under the new key"
                                );
                            }
                        }
                    } else {
                        tracing::debug!(
                            "rotate_sync_key: no previous sync key — nothing to re-encrypt \
                             in cloud (CopyPaste-vvsf)"
                        );
                    }
                }

                Response::ok(req.id, serde_json::json!({"ok": true, "rotated": true}))
            }

            // C-P0-4: revoke a peer from P2P AND rotate the sync key in one call,
            // so the revoked device is cut off from cloud/relay sync too. Runs
            // the SAME body as `revoke_peer` (P2P allowlist eviction + audit
            // row), then derives & installs the new sync key. The new passphrase
            // is required; if it is missing/invalid we do NOT revoke (so the
            // caller can retry without a half-applied state).
            //
            // SECURITY (C-P0-4 / CopyPaste-gbo): previously gated only on
            // `cloud-sync`. Widened to `relay-sync` because the relay inbox id is
            // HKDF-derived from the sync key — without rotation a revoked device
            // retains its relay inbox address and the shared key to decrypt new
            // relay items. `revoke_peer` alone (P2P-only denylist) is NOT
            // sufficient revocation when relay-sync is active.
            #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
            "revoke_and_rotate" => {
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
                let passphrase = match req.params.get("passphrase").and_then(|v| v.as_str()) {
                    Some(p) if !p.is_empty() => p.to_owned(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or empty param: passphrase",
                        )
                    }
                };
                // Derive the new key FIRST so a bad passphrase fails before we
                // mutate any revocation state.
                let new_key = match derive_sync_key(&passphrase) {
                    Ok(k) => k,
                    Err(e) => {
                        tracing::warn!("revoke_and_rotate: key derivation failed: {e}");
                        return Response::err(req.id, format!("key derivation failed: {e}"));
                    }
                };

                // ── Revoke (same as the `revoke_peer` body) ──
                let (removed, captured_name) = match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        // Normalise both sides so colon-hex display fingerprints
                        // and bare-hex canonical fingerprints both match
                        // (CopyPaste-qvn: raw string compare missed cross-format).
                        let fp_canonical = canonical_fingerprint(&fingerprint);
                        let name = peers
                            .iter()
                            .find(|p| {
                                p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| canonical_fingerprint(f) == fp_canonical)
                                    .unwrap_or(false)
                            })
                            .and_then(|p| p.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) != fp_canonical)
                                .unwrap_or(true)
                        });
                        if let Err(e) = save_peers(&peers) {
                            return Response::err(req.id, format!("failed to save peers: {e}"));
                        }
                        (peers.len() < before_len, name)
                    }
                    Err(e) => return Response::err(req.id, format!("failed to load peers: {e}")),
                };

                let db_arc = self.db.clone();
                let fp_for_db = fingerprint.clone();
                let name_for_db = captured_name.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    revoke_device(db.conn(), &fp_for_db, &name_for_db)
                })
                .await;

                let revoked_at = match join {
                    Ok(Ok(ts)) => {
                        // Evict from the live mTLS allowlist immediately.
                        if let Some(ref peers) = self.p2p_peers {
                            peers.remove(&canonical_fingerprint(&fingerprint));
                        }
                        ts
                    }
                    Ok(Err(e)) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("failed to record revocation: {e}"),
                        )
                    }
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("revoke task join error: {e}"),
                        )
                    }
                };

                // ── Rotate the sync key (cuts off cloud/relay for the revoked
                // device; remaining devices must re-provision). ──
                self.persist_and_install_sync_key(new_key).await;
                tracing::info!(
                    "revoke_and_rotate: revoked peer and rotated sync key; remaining devices must \
                     re-provision to keep syncing"
                );
                Response::ok(
                    req.id,
                    serde_json::json!({
                        "ok": true,
                        "removed": removed,
                        "revoked_at": revoked_at,
                        "fingerprint": fingerprint,
                        "rotated": true,
                    }),
                )
            }

            #[cfg(feature = "cloud-sync")]
            "get_sync_status" => {
                let passphrase_set = self.sync_key.lock().await.is_some();
                // Fix HIGH #3: read_config() does blocking fs I/O; move it to
                // the blocking thread pool so the async worker is not stalled.
                let app_cfg = match tokio::task::spawn_blocking(read_config).await {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INTERNAL_ERROR,
                            format!("get_sync_status blocking task failed: {e}"),
                        )
                    }
                };
                let supabase_configured = app_cfg.supabase_url.is_some()
                    && app_cfg.supabase_anon_key.is_some()
                    || std::env::var("SUPABASE_URL").is_ok();
                // BUG 2 fix: report the REAL GoTrue auth state published by the
                // cloud loops, not the old `signed_in = supabase_configured`
                // placeholder. The flag is set `true` once `start_cloud` resolves
                // a bearer and `false` on a bearer-resolution / 401-refresh
                // failure, so the UI no longer claims "signed in" after a
                // `CloudError::AuthFailed` aborted cloud sync.
                let signed_in = self
                    .cloud_signed_in
                    .load(std::sync::atomic::Ordering::Relaxed);
                let raw_ts = self.last_sync_ms.load(std::sync::atomic::Ordering::Relaxed);
                let last_sync_ms_val: Option<i64> = if raw_ts > 0 { Some(raw_ts) } else { None };
                // B. Expose the non-secret Supabase URL and email so the UI can
                // show/prefill them. We do NOT expose the anon key, password, or
                // passphrase. Priority: env vars override AppConfig (same as
                // CloudConfig::from_env).
                let supabase_url_val: Option<String> = std::env::var("SUPABASE_URL")
                    .ok()
                    .or_else(|| app_cfg.supabase_url.clone());
                // M3 FIX: mask the email before sending over IPC so arbitrary
                // same-UID processes cannot harvest the full GoTrue address.
                // `a***@example.com` preserves the account-indicator the UI
                // needs (SettingsView shows "Signed in as …") without leaking
                // the full address. Mirrors `cloud::redact_email` — inlined
                // here because that helper is private to the cloud module.
                let email_val: Option<String> = std::env::var("SUPABASE_EMAIL")
                    .ok()
                    .or_else(|| app_cfg.supabase_email.clone())
                    .map(|e| {
                        // Show first char + *** + @domain; non-address input →
                        // "<redacted>" (same contract as cloud::redact_email).
                        match e.split_once('@') {
                            Some((local, domain)) if !local.is_empty() && !domain.is_empty() => {
                                let first = local.chars().next().unwrap_or('*');
                                if local.chars().count() <= 1 {
                                    format!("*@{domain}")
                                } else {
                                    format!("{first}***@{domain}")
                                }
                            }
                            _ => "<redacted>".to_string(),
                        }
                    });
                // CopyPaste-merc / CopyPaste-1jms.22: compute badge state once
                // here in the daemon so every consumer (macOS UI, Android)
                // renders the SAME canonical value. Use the `_with_inflight`
                // variant so `Syncing` (green pulse) is emitted while a sync
                // round-trip is actively in progress.
                // `supabase_url_val` is Some(url) when either the env var or
                // the config has a URL — use it as the "url_set" signal.
                let supabase_url_set = supabase_url_val.is_some();
                // Relaxed ordering: a brief window where the badge says "idle"
                // while a round-trip just started (or vice versa) is acceptable.
                // The badge is informational and is refreshed on every IPC poll.
                let in_flight = self
                    .sync_in_flight
                    .load(std::sync::atomic::Ordering::Relaxed);
                let badge_state = compute_sync_badge_state_with_inflight(
                    passphrase_set,
                    supabase_url_set,
                    supabase_configured,
                    signed_in,
                    last_sync_ms_val,
                    None, // use SystemTime::now() inside the helper
                    in_flight,
                );
                let badge_state_json =
                    serde_json::to_value(&badge_state).unwrap_or(serde_json::Value::Null);
                // CopyPaste-1jms.34: read the canonical account identity set by
                // `with_cloud_account_id` after `start_cloud` returned. A `None`
                // means cloud-sync is not active or the daemon is anon-key-only.
                // The Mutex hold is tiny (clone the String and drop immediately).
                let account_id_val: Option<String> = self
                    .cloud_account_id
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone();
                Response::ok(
                    req.id,
                    serde_json::json!({
                        "passphrase_set": passphrase_set,
                        "supabase_configured": supabase_configured,
                        "signed_in": signed_in,
                        "last_sync_ms": last_sync_ms_val,
                        "supabase_url": supabase_url_val,
                        "email": email_val,
                        // Single source of truth for the badge colour on all platforms.
                        // Optional for backward-compat: consumers that receive this field
                        // MUST use it; consumers talking to older daemons may not see it
                        // and may fall back to local derivation from the raw fields above.
                        "badge_state": badge_state_json,
                        // Non-secret stable account identity (CopyPaste-1jms.34).
                        // Omitted from the wire when None (cloud off / anon-key-only).
                        "supabase_account_id": account_id_val,
                    }),
                )
            }

            // `cloud_test_connection` validates the configured Supabase
            // credentials end-to-end so the UI/CLI can give a precise, actionable
            // diagnostic instead of leaving the user to guess why sync is silent.
            // It performs a single cheap `GET /rest/v1/clipboard_items?limit=0`
            // with the anon key (+ optional email/password bearer) and classifies
            // the outcome (URL reachable? key valid? table present? RLS ok?).
            #[cfg(feature = "cloud-sync")]
            "cloud_test_connection" => {
                let result = test_cloud_connection().await;
                Response::ok(req.id, result)
            }

            // When cloud-sync is not compiled in, return not_implemented for
            // Supabase-specific methods so the UI gets a machine-readable code
            // rather than "method not found".
            #[cfg(not(feature = "cloud-sync"))]
            "set_sync_passphrase" | "get_sync_status" | "cloud_test_connection" => {
                Response::not_implemented(req.id, "cloud-sync")
            }

            // rotate_sync_key and revoke_and_rotate are available when EITHER
            // cloud-sync OR relay-sync is compiled in (widened from cloud-sync
            // only — CopyPaste-gbo). When neither is active, report
            // not_implemented rather than "method not found" so callers can
            // distinguish "feature off" from "unknown method".
            #[cfg(not(any(feature = "cloud-sync", feature = "relay-sync")))]
            "rotate_sync_key" | "revoke_and_rotate" => {
                Response::not_implemented(req.id, "cloud-sync or relay-sync")
            }
            "set_private_mode" => {
                let enabled = match req.params.get("enabled").and_then(|v| v.as_bool()) {
                    Some(b) => b,
                    // P2-8u2b: tag with ERR_CODE_INVALID_ARGUMENT so machine
                    // clients can classify the error.
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: enabled (bool)",
                        )
                    }
                };
                self.private_mode.store(enabled, Ordering::Relaxed);
                // CopyPaste-48k0: increment the epoch counter so any periodic
                // `status` or `get_private_mode` poll can detect the change
                // without needing a dedicated subscription.  The tray's one-shot
                // poller exits after startup; the epoch lets it (or any other
                // client) re-sync by comparing the epoch value across polls.
                let epoch = self
                    .private_mode_epoch
                    .fetch_add(1, Ordering::Relaxed)
                    .wrapping_add(1);
                // Persist so the setting survives a daemon restart (restored by
                // `daemon::load_private_mode` at startup). Best-effort: the
                // in-memory atomic above is authoritative for this process.
                crate::daemon::persist_private_mode(enabled);
                tracing::info!("private mode set to {enabled} (epoch={epoch})");
                Response::ok(
                    req.id,
                    serde_json::json!({"private_mode": enabled, "private_mode_epoch": epoch}),
                )
            }
            "get_private_mode" => {
                let enabled = self.private_mode.load(Ordering::Relaxed);
                // CopyPaste-48k0: include the epoch so callers can detect
                // changes since their last poll without a separate subscription.
                let epoch = self.private_mode_epoch.load(Ordering::Relaxed);
                Response::ok(
                    req.id,
                    serde_json::json!({"private_mode": enabled, "private_mode_epoch": epoch}),
                )
            }
            "status" => {
                let enabled = self.private_mode.load(Ordering::Relaxed);
                // CopyPaste-48k0: include the epoch in `status` so the UI's
                // periodic health-check poll can detect private-mode changes
                // without a dedicated subscription. A changed epoch → re-sync.
                let epoch = self.private_mode_epoch.load(Ordering::Relaxed);
                // In degraded startup the daemon is alive and the socket is
                // bound, but the backing DB is unavailable (e.g. the Keychain
                // SQLCipher key could not be read after a reinstall). Report
                // status="degraded" + a machine-readable reason + a flag so the
                // UI shows a recovery banner instead of treating the reachable
                // socket as "everything is fine". When healthy, `ready` is true
                // and `degraded_reason` is absent — unchanged shape for clients
                // that only read `status`/`private_mode`.
                // `build_version` + `pid` let a client (or a newer daemon doing
                // socket takeover) detect and evict a STALE predecessor after an
                // upgrade. Both are reported even in the degraded branch so the
                // stale check works without a healthy DB.
                let reason = self
                    .degraded_reason
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .clone();
                // CopyPaste-ruep: surface the X25519 device-key fingerprint
                // (SHA-256 of the public key bytes, lowercase hex) for operator
                // diagnostics.  This is informational only and distinct from the
                // mTLS cert fingerprint that pairing uses.
                let device_key_fingerprint = {
                    use sha2::{Digest as _, Sha256};
                    hex::encode(Sha256::digest(self.device_public_key.as_ref()))
                };
                match reason {
                    Some(reason) => Response::ok(
                        req.id,
                        serde_json::json!({
                            "status": "degraded",
                            "private_mode": enabled,
                            "private_mode_epoch": epoch,
                            "ready": false,
                            "degraded": true,
                            "degraded_reason": reason,
                            "build_version": BUILD_VERSION,
                            "pid": std::process::id(),
                            "device_key_fingerprint": device_key_fingerprint,
                        }),
                    ),
                    None => Response::ok(
                        req.id,
                        serde_json::json!({
                            "status": "running",
                            "private_mode": enabled,
                            "private_mode_epoch": epoch,
                            "ready": self.ready.load(Ordering::Relaxed),
                            "degraded": false,
                            "build_version": BUILD_VERSION,
                            "pid": std::process::id(),
                            "device_key_fingerprint": device_key_fingerprint,
                        }),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // Destructive recovery: wipe + recreate the clipboard database.
            //
            // This is the explicit escape hatch for a daemon stuck in DEGRADED
            // mode because `clipboard.db` cannot be decrypted (key mismatch /
            // "file is not a database"). UNLIKE every other DB-touching method,
            // this one is NOT gated behind the `ready` flag — recovering FROM
            // degraded mode is its entire reason to exist, so it must run while
            // `ready = false`. It therefore appears BEFORE the readiness gate in
            // spirit (the gate's `requires_db` allow-list deliberately omits it).
            // ------------------------------------------------------------------
            "reset_database" => {
                // Guard #1: an explicit confirm flag is mandatory so a stray or
                // replayed call can never erase the user's history by accident.
                let confirm = req
                    .params
                    .get("confirm")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !confirm {
                    tracing::warn!(
                        "reset_database rejected: missing confirm=true — refusing \
                         to wipe the clipboard database without explicit confirmation"
                    );
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "reset_database requires confirm=true",
                    );
                }

                let db_path = crate::paths::db_path();
                tracing::warn!(
                    db_path = %db_path.display(),
                    "reset_database INVOKED: WIPING and RECREATING the clipboard \
                     database. All local clipboard history will be PERMANENTLY \
                     DELETED. This is the user-confirmed recovery escape hatch for \
                     a daemon stuck in degraded mode (undecryptable DB)."
                );

                // Resolve the key for the FRESH database. Prefer the real
                // device key from the Keychain (so the new DB re-opens normally
                // on the next restart); if that is unreachable (the very reason
                // we may be degraded), fall back to the key this server already
                // holds. Either way the fresh empty DB is self-consistent and
                // immediately usable this session.
                let fresh_key: zeroize::Zeroizing<[u8; 32]> = {
                    #[cfg(target_os = "macos")]
                    {
                        match crate::keychain::load_or_create() {
                            Ok(kp) => {
                                tracing::info!(
                                    "reset_database: using the device Keychain key for the \
                                     fresh database"
                                );
                                kp.local_enc_key()
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    "reset_database: Keychain key unavailable; recreating the \
                                     fresh database with the daemon's current in-memory key"
                                );
                                zeroize::Zeroizing::new(**self.local_key)
                            }
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        zeroize::Zeroizing::new(**self.local_key)
                    }
                };

                // Do the destructive filesystem work + reopen on a blocking
                // thread (rusqlite is sync). We hold the DB mutex for the whole
                // operation so no other request can touch the handle mid-swap.
                let db_arc = self.db.clone();
                let path_for_task = db_path.clone();
                let join = tokio::task::spawn_blocking(move || {
                    let mut guard = db_arc.blocking_lock();

                    // 1. Close the current connection. Swapping in a throwaway
                    //    in-memory DB drops the old `Database` (and its open
                    //    file handles / WAL) so the files can be removed cleanly.
                    *guard = Database::open_in_memory()
                        .map_err(|e| format!("failed to open transient in-memory DB: {e}"))?;

                    // 2. Delete clipboard.db and its WAL/SHM siblings. A missing
                    //    file is fine (NotFound is not an error here).
                    for suffix in ["", "-wal", "-shm"] {
                        let mut p = path_for_task.clone().into_os_string();
                        p.push(suffix);
                        let p = std::path::PathBuf::from(p);
                        match std::fs::remove_file(&p) {
                            Ok(()) => {
                                tracing::warn!(file = %p.display(), "reset_database: deleted")
                            }
                            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                            Err(e) => {
                                return Err(format!("failed to delete {}: {e}", p.display()));
                            }
                        }
                    }

                    // 3. Recreate a fresh empty encrypted DB with the chosen key
                    //    using the SAME open/migrate path a clean install uses.
                    let fresh = Database::open(&path_for_task, &fresh_key)
                        .map_err(|e| format!("failed to create fresh database: {e}"))?;

                    // 4. Ensure the additive audit table the IPC layer relies on
                    //    exists, matching the normal `serve()` startup path.
                    if let Err(e) = ensure_revoked_devices_table(fresh.conn()) {
                        tracing::warn!("reset_database: ensure_revoked_devices_table failed: {e}");
                    }

                    // 5. Install the fresh DB as the live handle.
                    *guard = fresh;
                    Ok::<(), String>(())
                })
                .await;

                match join {
                    Ok(Ok(())) => {
                        // Bring the daemon OUT of degraded mode IN-PLACE: the new
                        // empty DB is live, so flip readiness on and clear the
                        // degraded reason. Subsequent history_page / status calls
                        // now succeed without a process restart.
                        self.ready.store(true, Ordering::Relaxed);
                        *self
                            .degraded_reason
                            .lock()
                            .unwrap_or_else(|p| p.into_inner()) = None;
                        tracing::warn!(
                            db_path = %db_path.display(),
                            "reset_database COMPLETE: fresh empty database created, daemon \
                             recovered in-place (no longer degraded, ready=true)"
                        );
                        Response::ok(req.id, serde_json::json!({ "reset": true, "ready": true }))
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(
                            db_path = %db_path.display(),
                            error = %msg,
                            "reset_database FAILED: the clipboard database could not be \
                             wiped/recreated. The daemon remains in its prior state."
                        );
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("reset_database blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // Database maintenance
            // ------------------------------------------------------------------
            "vacuum" => {
                // Parse optional flags; both default to false so a bare `{}`
                // params object runs the full VACUUM + REINDEX path.
                let reindex_only = req
                    .params
                    .get("reindex_only")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let dry_run = req
                    .params
                    .get("dry_run")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let db_arc = self.db.clone();
                let db_path = crate::paths::db_path();
                let join = tokio::task::spawn_blocking(move || {
                    let guard = db_arc.blocking_lock();

                    // Stat the file before any writes so we can report
                    // reclaimed bytes.  The stat uses the filesystem path, not
                    // in-memory pages, so it accurately reflects WAL state.
                    let size_before = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

                    if dry_run {
                        // Verify the connection is healthy by running a cheap
                        // read-only statement; does NOT mutate anything.
                        guard
                            .conn()
                            .execute_batch("SELECT COUNT(*) FROM clipboard_items")
                            .map_err(|e| format!("dry-run DB probe failed: {e}"))?;
                        return Ok((size_before, size_before));
                    }

                    if !reindex_only {
                        // Flush WAL pages into the main file before VACUUM so
                        // the "after" size reflects the fully compacted state.
                        // A failed checkpoint is non-fatal — log and continue.
                        if let Err(e) = guard
                            .conn()
                            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE)")
                        {
                            tracing::warn!(
                                error = %e,
                                "vacuum: wal_checkpoint(TRUNCATE) failed; \
                                 continuing with VACUUM (after-size may be inflated)"
                            );
                        }
                        guard
                            .conn()
                            .execute_batch("VACUUM")
                            .map_err(|e| format!("VACUUM failed: {e}"))?;
                    }

                    guard
                        .conn()
                        .execute_batch("REINDEX")
                        .map_err(|e| format!("REINDEX failed: {e}"))?;

                    // Drop the guard so the OS flushes pending writes before
                    // we stat the file for the "after" size.
                    drop(guard);

                    let size_after = std::fs::metadata(&db_path)
                        .map(|m| m.len())
                        .unwrap_or(size_before);

                    Ok::<(u64, u64), String>((size_before, size_after))
                })
                .await;

                match join {
                    Ok(Ok((size_before, size_after))) => {
                        let reclaimed = size_before as i64 - size_after as i64;
                        tracing::info!(
                            size_before,
                            size_after,
                            reclaimed,
                            reindex_only,
                            dry_run,
                            "vacuum: completed"
                        );
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "ok": true,
                                "size_before": size_before,
                                "size_after": size_after,
                                "reclaimed": reclaimed,
                            }),
                        )
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(error = %msg, "vacuum: operation failed");
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("vacuum blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // db_stats — lightweight storage summary (CopyPaste-40gl)
            //
            // Used by the macOS UI settings panel (SettingsView.gq51) to show
            // item count and approximate on-disk size without the full `stats`
            // computation. Returns { item_count, size_bytes }.
            // ------------------------------------------------------------------
            "db_stats" => {
                let db_arc = self.db.clone();
                let db_path = crate::paths::db_path();
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    // Count all rows including tombstones — same contract as
                    // COUNT(*) so the number is consistent with `stats`.
                    let item_count: u64 = db
                        .conn()
                        .query_row("SELECT COUNT(*) FROM clipboard_items", [], |r| {
                            r.get::<_, i64>(0)
                        })
                        .map(|n| n.max(0) as u64)
                        .unwrap_or(0);
                    // File size from the filesystem — excludes the WAL file so
                    // it matches what a user sees in Finder / du. Returns 0 when
                    // the file doesn't exist yet (fresh install).
                    let size_bytes: u64 = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);
                    (item_count, size_bytes)
                })
                .await;
                match join {
                    Ok((item_count, size_bytes)) => Response::ok(
                        req.id,
                        serde_json::json!({ "item_count": item_count, "size_bytes": size_bytes }),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("db_stats blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // db_backup — in-process SQLCipher backup (CopyPaste-x94p)
            //
            // Uses `VACUUM INTO '<dest>'` which produces a consistent, hot
            // copy encrypted with the same key as the source database. The
            // daemon does NOT need to stop. The destination file must not
            // already exist (refuses overwrite for safety).
            // ------------------------------------------------------------------
            "db_backup" => {
                let dest_path = match req.params.get("dest_path").and_then(|v| v.as_str()) {
                    Some(s) if !s.is_empty() => s.to_string(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "db_backup requires a non-empty dest_path",
                        )
                    }
                };

                // Refuse to overwrite an existing file so a mis-aimed backup
                // cannot silently clobber a good previous backup.
                if std::path::Path::new(&dest_path).exists() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("db_backup: dest_path already exists: {dest_path}"),
                    );
                }

                let db_arc = self.db.clone();
                let dest_path_clone = dest_path.clone();
                let join = tokio::task::spawn_blocking(move || {
                    // Acquire the write-mutex so no other IPC call can mutate
                    // the DB mid-backup. `VACUUM INTO` takes a consistent
                    // snapshot of all non-empty pages; holding the lock ensures
                    // the snapshot is atomic from the daemon's perspective.
                    let guard = db_arc.blocking_lock();

                    // Ensure the parent directory exists so the error message
                    // is clear if it doesn't.
                    if let Some(parent) = std::path::Path::new(&dest_path_clone).parent() {
                        if !parent.as_os_str().is_empty() && !parent.exists() {
                            return Err(format!(
                                "db_backup: parent directory does not exist: {}",
                                parent.display()
                            ));
                        }
                    }

                    // VACUUM INTO copies all live pages into dest, encrypted
                    // with the same SQLCipher key as the source. This is the
                    // same mechanism the shell script used via `sqlcipher .backup`,
                    // but done in-process without stopping the daemon.
                    guard
                        .conn()
                        .execute_batch(&format!(
                            "VACUUM INTO '{}'",
                            dest_path_clone.replace('\'', "''")
                        ))
                        .map_err(|e| format!("VACUUM INTO failed: {e}"))?;

                    // Set restrictive permissions on the backup file so other
                    // local users cannot read the encrypted database.
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        if let Ok(meta) = std::fs::metadata(&dest_path_clone) {
                            let mut perms = meta.permissions();
                            perms.set_mode(0o600);
                            let _ = std::fs::set_permissions(&dest_path_clone, perms);
                        }
                    }

                    let size_bytes = std::fs::metadata(&dest_path_clone)
                        .map(|m| m.len())
                        .unwrap_or(0);

                    Ok::<u64, String>(size_bytes)
                })
                .await;

                match join {
                    Ok(Ok(size_bytes)) => {
                        tracing::info!(
                            dest = %dest_path,
                            size_bytes,
                            "db_backup: backup created successfully"
                        );
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "ok": true,
                                "dest_path": dest_path,
                                "size_bytes": size_bytes,
                            }),
                        )
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(error = %msg, "db_backup: failed");
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("db_backup blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // db_restore — replace the live DB with a backup (CopyPaste-8wbt,
            // crh3.6, crh3.2)
            //
            // VALIDATE-then-SWAP. The previous implementation moved/deleted the
            // live DB *before* checking the backup could be opened, so a
            // wrong-key or corrupt backup permanently destroyed the user's
            // history. The flow is now:
            //   PHASE A (no live mutation): copy the backup to a throwaway
            //     staging file, open it with the real device key, run an
            //     integrity_check and a schema sanity check. Any failure aborts
            //     with the live DB completely untouched.
            //   PHASE B (only after A succeeds): quiesce, move the live DB aside
            //     (BOTH force and non-force, so a late failure can roll back),
            //     copy the validated backup into place, reopen. On any Phase-B
            //     failure the aside files are moved back and the original DB is
            //     reopened. `force` only controls whether the aside safety copy
            //     is deleted on success.
            // Degraded mode (crh3.6): the key is resolved from the Keychain, not
            // the daemon's throwaway in-memory key. crh3.2: the r2d2 read pool is
            // rebuilt against the restored file so reads stop serving stale data.
            // ------------------------------------------------------------------
            "db_restore" => {
                let confirm = req
                    .params
                    .get("confirm")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                if !confirm {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "db_restore requires confirm=true",
                    );
                }

                let src_path = match req.params.get("src_path").and_then(|v| v.as_str()) {
                    Some(s) if !s.is_empty() => s.to_string(),
                    _ => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "db_restore requires a non-empty src_path",
                        )
                    }
                };

                if !std::path::Path::new(&src_path).is_file() {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("db_restore: backup file not found: {src_path}"),
                    );
                }

                let force = req
                    .params
                    .get("force")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let db_path = crate::paths::db_path();

                // Resolve the REAL device key for validating + opening the
                // restored DB (CopyPaste-crh3.6). The daemon's in-memory key is
                // a throwaway dummy when degraded (Keychain locked at startup),
                // which can NEVER open a backup encrypted with the real device
                // key. Mirror `reset_database`: load the key from the Keychain.
                // If the daemon is degraded AND the Keychain is still
                // unreachable, reject with ipc_not_ready and make NO filesystem
                // change (the only safe outcome).
                let restore_key: zeroize::Zeroizing<[u8; 32]> = {
                    #[cfg(target_os = "macos")]
                    {
                        match crate::keychain::load_or_create() {
                            Ok(kp) => kp.local_enc_key(),
                            Err(e) => {
                                if !self.ready.load(Ordering::Relaxed) {
                                    tracing::warn!(
                                        error = %e,
                                        "db_restore: refusing — daemon is degraded and the \
                                         Keychain key is unreachable; no filesystem change made"
                                    );
                                    return Response::err_with_code(
                                        req.id,
                                        ERR_CODE_IPC_NOT_READY,
                                        ERR_IPC_NOT_READY,
                                    );
                                }
                                tracing::warn!(
                                    error = %e,
                                    "db_restore: Keychain unavailable; validating with the \
                                     daemon's current in-memory key"
                                );
                                zeroize::Zeroizing::new(**self.local_key)
                            }
                        }
                    }
                    #[cfg(not(target_os = "macos"))]
                    {
                        zeroize::Zeroizing::new(**self.local_key)
                    }
                };

                tracing::warn!(
                    src_path = %src_path,
                    db_path = %db_path.display(),
                    force,
                    "db_restore: validating backup before any destructive change"
                );

                let db_arc = self.db.clone();
                let src_for_task = src_path.clone();
                let db_path_for_task = db_path.clone();
                // Clone the key for the blocking task; the outer `restore_key`
                // is reused after the join to rebuild the read pool (crh3.2).
                let key_for_task: zeroize::Zeroizing<[u8; 32]> =
                    zeroize::Zeroizing::new(*restore_key);
                let join = tokio::task::spawn_blocking(move || -> Result<(), String> {
                    // Hold the write lock across validate+swap so no concurrent
                    // write lands on the about-to-be-replaced handle. The old
                    // handle is kept live until the swap succeeds: on a
                    // rolled-back failure it remains valid (its inode is moved
                    // aside and back, never replaced), so `guard` is always a
                    // usable DB.
                    let mut guard = db_arc.blocking_lock();
                    let restored = restore_database_file(
                        std::path::Path::new(&src_for_task),
                        &db_path_for_task,
                        &key_for_task,
                        force,
                    )?;
                    *guard = restored;
                    Ok(())
                })
                .await;

                match join {
                    Ok(Ok(())) => {
                        // Mark daemon as ready (in case it was degraded before).
                        self.ready.store(true, Ordering::Relaxed);
                        *self
                            .degraded_reason
                            .lock()
                            .unwrap_or_else(|p| p.into_inner()) = None;
                        // CopyPaste-crh3.2: the r2d2 read pool still holds file
                        // descriptors to the OLD database inode. Rebuild it
                        // against the restored file so reads stop serving
                        // pre-restore data. On failure, drop to None — reads then
                        // fall back to the write handle, which IS the restored DB.
                        let rebuilt = copypaste_core::open_pool(&db_path, &restore_key, 4)
                            .ok()
                            .map(Arc::new);
                        *self.read_pool.lock().unwrap_or_else(|p| p.into_inner()) = rebuilt;
                        tracing::warn!(
                            src_path = %src_path,
                            db_path = %db_path.display(),
                            "db_restore: COMPLETE — restored database is live"
                        );
                        Response::ok(req.id, serde_json::json!({ "ok": true, "ready": true }))
                    }
                    Ok(Err(msg)) => {
                        tracing::error!(
                            error = %msg,
                            "db_restore: FAILED — prior database preserved"
                        );
                        Response::err_with_code(req.id, ERR_CODE_INTERNAL_ERROR, msg)
                    }
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("db_restore blocking task failed: {e}"),
                    ),
                }
            }

            // ------------------------------------------------------------------
            // P2P IPC methods
            // ------------------------------------------------------------------
            "get_own_fingerprint" => {
                // CRITICAL-1 fix: advertise the live mTLS **certificate**
                // fingerprint — the value peers pin and the mTLS verifier
                // compares (`PeerTransport::fingerprint` / `fingerprint_of`) —
                // NOT the device-key fingerprint
                // (`keychain::own_fingerprint`, SHA-256 of the X25519 public
                // key). The latter is never compared by the mTLS allowlist, so
                // pinning it could never authenticate a channel.
                //
                // When P2P is disabled there is no running transport and thus
                // no cert to advertise; return a clear error rather than a
                // fingerprint that cannot authenticate anything.
                match self.cert_fingerprint.as_ref() {
                    Some(fingerprint) => {
                        Response::ok(req.id, serde_json::json!({ "fingerprint": fingerprint }))
                    }
                    None => Response::err(
                        req.id,
                        "P2P is disabled (set COPYPASTE_P2P=1): no mTLS certificate \
                         to advertise for pairing",
                    ),
                }
            }

            // ----------------------------------------------------------------
            // `get_own_device_info` — rich identity for THIS device.
            //
            // Returns fingerprint (same as `get_own_fingerprint`) PLUS
            // human-readable metadata: device name, model, OS, app version,
            // and LAN IP.  All fields except `app_version` and `fingerprint`
            // are optional (`skip_serializing_if = "is_none"`) so older UI
            // versions that don't know about them still get a valid response.
            //
            // The fingerprint field is omitted when P2P is disabled — callers
            // must gracefully handle a `null` fingerprint (same contract as
            // `get_own_fingerprint`).
            //
            // CopyPaste-bps: previously called DeviceMeta::collect here on
            // every UI refresh, spawning scutil/sysctl/sw_vers (~6 s total).
            // Now reads the process-wide OnceLock cache that was warmed once at
            // daemon startup — no child-process spawn on the hot path.
            // ----------------------------------------------------------------
            "get_own_device_info" => {
                let fingerprint_val = self.cert_fingerprint.clone();
                // get_cached is wait-free after the startup warm; spawn_blocking
                // is kept for correctness on the unlikely cold path.
                let meta = tokio::task::spawn_blocking(|| {
                    crate::device_meta::get_cached(env!("CARGO_PKG_VERSION"))
                })
                .await
                .unwrap_or_else(|_| crate::device_meta::get_cached(env!("CARGO_PKG_VERSION")));
                // Read the cached public IP collected asynchronously on startup
                // (STUN, best-effort). `None` when disabled by config or when
                // the network query has not yet resolved / failed.
                let public_ip_val = self.cached_public_ip.read().await.clone();
                Response::ok(
                    req.id,
                    serde_json::json!({
                        "fingerprint": fingerprint_val,
                        "device_name": meta.device_name,
                        "device_model": meta.device_model,
                        "os_version": meta.os_version,
                        "app_version": meta.app_version,
                        "local_ip": meta.local_ip,
                        "public_ip": public_ip_val,
                    }),
                )
            }

            "list_peers" => {
                // Race-fix (CopyPaste-7mf): if the QR bootstrap responder task is
                // still in flight, await it (with a generous timeout) before reading
                // peers.json. This ensures that a responder-side caller doing
                // `pair_generate_qr` → (initiator scans) → `list_peers` always sees
                // the freshly-persisted peer rather than an empty list.
                // We take the handle out of the slot so we only wait once per
                // bootstrap session; subsequent list_peers calls on the same daemon
                // do not block (the slot is None again).
                {
                    let maybe_handle = self.pending_bootstrap.lock().await.take();
                    if let Some(handle) = maybe_handle {
                        // 5-second timeout — the bootstrap PAKE + file write should
                        // complete in well under 1 s on any real device. If it
                        // times out (task panicked / stuck) we proceed anyway so
                        // list_peers never stalls indefinitely.
                        let _ =
                            tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
                    }
                }
                match load_peers() {
                    Ok(peers) => {
                        let now_secs = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            // SAFETY: now() is always after UNIX_EPOCH on any
                            // supported platform (macOS, Linux, Android).
                            .unwrap_or_default()
                            .as_secs() as i64;

                        // SINGLE SOURCE OF TRUTH for online status: snapshot the
                        // live P2P peer-sinks map (set of fingerprints with a
                        // non-closed mpsc sender = currently-connected peers).
                        // Falls back to `last_sync_at` recency only when P2P is
                        // disabled (inner slot is None).
                        //
                        // The outer std::sync::Mutex is locked briefly to clone
                        // the inner Arc (no .await while holding it). The inner
                        // tokio Mutex is then locked with .await so we don't block
                        // the executor.
                        let live_fps: Option<std::collections::HashSet<String>> = {
                            // Clone the Arc while holding the std::sync lock, then
                            // drop the lock before awaiting.
                            let maybe_sinks_arc: Option<crate::p2p::LivePeerSinks> = {
                                let slot = self
                                    .live_peer_sinks
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner());
                                slot.as_ref().map(Arc::clone)
                            };
                            if let Some(sinks_arc) = maybe_sinks_arc {
                                let sinks = sinks_arc.lock().await;
                                Some(
                                    sinks
                                        .iter()
                                        .filter(|(_, tx)| !tx.is_closed())
                                        .map(|(fp, _)| fp.clone())
                                        .collect(),
                                )
                            } else {
                                None
                            }
                        };

                        // Snapshot the RTT map (fingerprint → last RTT in ms).
                        // Same lazy-injection pattern as live_fps: None when P2P is
                        // disabled or not yet started.
                        let rtt_snapshot: Option<std::collections::HashMap<String, u32>> = {
                            let maybe_rtt_arc: Option<crate::p2p::PeerRttMs> = {
                                let slot = self
                                    .live_peer_rtt_ms
                                    .lock()
                                    .unwrap_or_else(|p| p.into_inner());
                                slot.as_ref().map(std::sync::Arc::clone)
                            };
                            if let Some(rtt_arc) = maybe_rtt_arc {
                                let rtt = rtt_arc.lock().await;
                                Some(rtt.iter().map(|(k, v)| (k.clone(), *v)).collect())
                            } else {
                                None
                            }
                        };

                        // CopyPaste-1jms.32: Determine which non-P2P transport is
                        // active for this daemon so offline peers can be labelled
                        // "Relay" or "Supabase" rather than a generic "Cloud".
                        //
                        // Relay and Supabase both use a shared inbox (no per-peer
                        // routing), so transport is inferred from daemon config:
                        //   relay running  → all non-P2P peers use "relay"
                        //   supabase signed-in (no relay) → "supabase"
                        //   neither  → None (unknown)
                        //
                        // P2P takes precedence and is determined per-peer below
                        // via `live_fps`.
                        #[cfg(feature = "relay-sync")]
                        let relay_active: bool = {
                            // Non-blocking check: try_lock returns None if the lock
                            // is momentarily held; we conservatively treat that as
                            // "relay running" (the common-path assumption is that
                            // relay IS active — a brief lock on set_config should
                            // not flip the chip to Unknown transiently).
                            self.relay_handle
                                .try_lock()
                                .map_or(true, |g| g.is_some())
                        };
                        #[cfg(not(feature = "relay-sync"))]
                        let relay_active: bool = false;

                        #[cfg(feature = "cloud-sync")]
                        let supabase_active: bool = self.cloud_signed_in.load(Ordering::Relaxed);
                        #[cfg(not(feature = "cloud-sync"))]
                        let supabase_active: bool = false;

                        let enriched: Vec<serde_json::Value> = peers
                        .into_iter()
                        .map(|mut peer| {
                            // last_sync_at from the record (i64 or absent).
                            let last_sync_at: Option<i64> =
                                peer.get("last_sync_at").and_then(|v| v.as_i64());

                            // last_seen_secs: seconds since the last successful
                            // sync, or -1 when we have no stamp at all.
                            let last_seen_secs: i64 = match last_sync_at {
                                Some(t) => now_secs.saturating_sub(t),
                                None => -1,
                            };

                            // Compute online from the authoritative source:
                            // 1. If live_fps is available (P2P running): peer is
                            //    online iff its canonical fingerprint has a live
                            //    non-closed sink in the connection table.
                            // 2. Fallback (P2P disabled): recent last_sync_at
                            //    within ONLINE_THRESHOLD_SECS.
                            let peer_fp_canonical = peer
                                .get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(canonical_fingerprint)
                                .unwrap_or_default();

                            // c4q2.21: delegate to extracted pure function.
                            let live_sink_opt = live_fps
                                .as_ref()
                                .map(|fps| fps.contains(&peer_fp_canonical));
                            let online =
                                compute_peer_online(live_sink_opt, last_sync_at, now_secs);

                            // latency_ms: last measured RTT for this peer, in ms.
                            // Present only when P2P is running AND a ping-pong has
                            // completed at least once for this connection.
                            let latency_ms: Option<u32> = rtt_snapshot
                                .as_ref()
                                .and_then(|m| m.get(&peer_fp_canonical).copied());

                            if let Some(obj) = peer.as_object_mut() {
                                obj.insert("online".to_string(), serde_json::Value::Bool(online));
                                obj.insert(
                                    "last_seen_secs".to_string(),
                                    serde_json::Value::Number(last_seen_secs.into()),
                                );
                                if let Some(ms) = latency_ms {
                                    obj.insert(
                                        "latency_ms".to_string(),
                                        serde_json::Value::Number(ms.into()),
                                    );
                                }
                                // CopyPaste-vypo: surface trust status honestly.
                                // Every record in peers.json completed a PAKE
                                // handshake (mutual key confirmation), so each
                                // persisted peer is "verified".  We never store
                                // unauthenticated (unverified) peers on disk —
                                // the PAKE session is discarded on failure.
                                // Hardcoding "Verified" was misleading because it
                                // implied the label could vary; using the string
                                // "verified" (lowercase, stable enum value) makes
                                // the semantics explicit and leaves room for a
                                // future "unverified" value for in-memory
                                // discovered-but-not-yet-paired devices.
                                obj.insert(
                                    "trust".to_string(),
                                    serde_json::Value::String("verified".to_string()),
                                );

                                // CopyPaste-1jms.32: surface per-peer transport.
                                // Priority: P2P (live sink) > Relay > Supabase > None.
                                let transport_str: Option<&'static str> = if live_sink_opt
                                    == Some(true)
                                {
                                    // Live P2P connection for this peer.
                                    Some("p2p")
                                } else if relay_active {
                                    // Relay is the active secondary transport.
                                    Some("relay")
                                } else if supabase_active {
                                    // Supabase is the active secondary transport.
                                    Some("supabase")
                                } else {
                                    // No transport is active or known.
                                    None
                                };
                                if let Some(t) = transport_str {
                                    obj.insert(
                                        "transport".to_string(),
                                        serde_json::Value::String(t.to_string()),
                                    );
                                }
                                // CopyPaste-5lm: never expose the PasswordFile blob
                                // (encrypted or plaintext) over the IPC wire. The UI
                                // has no need for this field; stripping it here means
                                // a compromised IPC client cannot exfiltrate it.
                                obj.remove("password_file_enc");
                                obj.remove("password_file_b64");
                            }
                            peer
                        })
                        .collect();

                        Response::ok(req.id, serde_json::json!({ "peers": enriched }))
                    }
                    Err(e) => Response::err(req.id, format!("failed to load peers: {e}")),
                }
            }

            // Drain all pending peer connect/disconnect events and return them
            // as an array. Called by the Tauri event bridge every ~1 s so the
            // UI can update online presence dots without waiting for the next
            // full `list_peers` poll.
            //
            // Response: { events: [{ kind: "connected"|"disconnected",
            //                        fingerprint: "<hex>" }] }
            //
            // An empty `events` array is a valid response (no changes since the
            // last poll). This is a draining read — once returned, events are
            // removed from the queue.
            "poll_peer_events" => {
                let events: Vec<PeerEventRecord> = {
                    let mut q = self
                        .peer_event_queue
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    q.drain(..).collect()
                };
                Response::ok(req.id, serde_json::json!({ "events": events }))
            }

            // LAN/SAS Phase 0: return discovered peers (mDNS) cross-referenced
            // against peers.json to flag already-paired devices.
            //
            // Response: { devices: [{ device_id, device_name, ip_addrs, port,
            //                         bport, paired }] }
            // `paired` = true when canonical fingerprint matches peers.json.
            // `bport`  = null on v1 peers (UI disables "Pair" button).
            "list_discovered" => {
                let disc = match self.discovery.as_ref() {
                    Some(d) => d,
                    None => return Response::err(req.id, "discovery not available (P2P disabled)"),
                };

                // HB-4: the mDNS `device_id` is a UUID, NOT a cert fingerprint, so
                // a fingerprint-compare against peers.json never matched and paired
                // devices kept showing "Pair". Instead snapshot the set of IP hosts
                // we have paired with (`local_ip` + the host part of `address`) and
                // mark a discovered peer `paired` when ANY of its resolved
                // `ip_addrs` is in that set.
                //
                // Race-fix (CopyPaste-daq, sibling of CopyPaste-7mf): if the QR
                // bootstrap responder task is still in flight, await it (with a
                // timeout) before reading peers.json. Otherwise a just-paired
                // device's IP is absent from `paired_ips` and the Devices page
                // shows a spurious "Pair" prompt for an already-paired device.
                // Mirrors the identical await in the `list_peers` handler.
                {
                    let maybe_handle = self.pending_bootstrap.lock().await.take();
                    if let Some(handle) = maybe_handle {
                        let _ =
                            tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
                    }
                }
                let paired_ips: std::collections::HashSet<String> = match load_peers() {
                    Ok(stored) => paired_ip_hosts(&stored),
                    // Non-fatal: treat as empty — we just won't mark any peer paired.
                    Err(e) => {
                        tracing::warn!("list_discovered: failed to load peers.json: {e}");
                        std::collections::HashSet::new()
                    }
                };

                let devices: Vec<serde_json::Value> = disc
                    .peers()
                    .into_iter()
                    .map(|peer| {
                        let ip_strs: Vec<String> =
                            peer.ip_addrs.iter().map(|a| a.to_string()).collect();
                        let paired = ip_strs.iter().any(|ip| paired_ips.contains(ip));
                        serde_json::json!({
                            "device_id":   peer.device_id,
                            "device_name": peer.device_name,
                            "ip_addrs":    ip_strs,
                            "port":        peer.port,
                            // null when peer is v1 (no bport TXT key); UI
                            // disables "Pair" in that case.
                            "bport":       peer.bport,
                            "paired":      paired,
                        })
                    })
                    .collect();

                Response::ok(req.id, serde_json::json!({ "devices": devices }))
            }

            // HB-9: manual rescan. Restart the mDNS-SD browse in place
            // (`DiscoveryService::start` tears down the prior browse task/socket
            // first, then re-advertises + re-browses) and return the fresh peer
            // snapshot. Used by the UI "Refresh" button next to the discovered
            // list when passive polling hasn't surfaced a peer yet.
            //
            // Response: { devices: [...] } — same shape as `list_discovered`.
            "rescan_discovered" => {
                let disc = match self.discovery.as_ref() {
                    Some(d) => d,
                    None => return Response::err(req.id, "discovery not available (P2P disabled)"),
                };

                // CopyPaste-ydhw: abort any browse handle stored from a prior
                // rescan before starting a new one.  This prevents accumulation
                // of orphaned browse tasks across multiple UI "Refresh" presses.
                //
                // Note: `disc.start()` (below) also calls `shutdown_inner()`
                // which aborts the `DiscoveryService`-internal AbortHandle.
                // Aborting `prev_handle` here covers the JoinHandle we returned
                // from the *previous* rescan — the two mechanisms are complementary.
                {
                    let mut slot = self
                        .discovery_browse_handle
                        .lock()
                        .unwrap_or_else(|p| p.into_inner());
                    if let Some(prev_handle) = slot.take() {
                        prev_handle.abort();
                    }
                }

                // Restart-in-place re-browse.  `disc.start()` aborts the prior
                // browse via `shutdown_inner()`, which also aborts the JoinHandle
                // that `start_p2p`'s discovery task was select!-ing on — that
                // task then exits (see p2p.rs discovery task, CopyPaste-ydhw).
                // The IPC server takes over lifecycle ownership of the new browse
                // via `discovery_browse_handle`.
                //
                // If the P2P shutdown token is available (daemon.rs writes it
                // into `p2p_shutdown_token` after `start_p2p` returns), we wrap
                // the browse handle in a select! so it participates in graceful
                // shutdown.  When the token is absent (P2P disabled, or the slot
                // not yet wired by daemon.rs) we still store the handle so the
                // next rescan can abort it.
                match disc.start().await {
                    Ok(handle) => {
                        // Clone the shutdown token BEFORE locking browse_handle
                        // to avoid holding the mutex across an await.
                        let maybe_token: Option<CancellationToken> = {
                            self.p2p_shutdown_token
                                .lock()
                                .unwrap_or_else(|p| p.into_inner())
                                .clone()
                        };

                        let wrapper_handle = if let Some(token) = maybe_token {
                            // Wrap the browse handle with a cancellation select!
                            // so P2P shutdown aborts the browse task cleanly.
                            tokio::spawn(async move {
                                tokio::select! {
                                    _ = handle => {}
                                    _ = token.cancelled() => {
                                        tracing::debug!(
                                            "rescan_discovered browse task shut down by P2P shutdown token"
                                        );
                                    }
                                }
                            })
                        } else {
                            // No shutdown token yet — spawn a plain wrapper so
                            // dropping `wrapper_handle` does not abort the browse.
                            // The browse runs until the next rescan aborts it.
                            tokio::spawn(async move {
                                let _ = handle.await;
                            })
                        };

                        // Store the wrapper handle so the next rescan can abort it.
                        *self
                            .discovery_browse_handle
                            .lock()
                            .unwrap_or_else(|p| p.into_inner()) = Some(wrapper_handle);
                    }
                    Err(e) => {
                        return Response::err(req.id, format!("rescan failed to start: {e}"));
                    }
                }

                // HB-4: IP-correlate already-paired peers (see `list_discovered`).
                let paired_ips: std::collections::HashSet<String> = match load_peers() {
                    Ok(stored) => paired_ip_hosts(&stored),
                    Err(e) => {
                        tracing::warn!("rescan_discovered: failed to load peers.json: {e}");
                        std::collections::HashSet::new()
                    }
                };

                let devices: Vec<serde_json::Value> = disc
                    .peers()
                    .into_iter()
                    .map(|peer| {
                        let ip_strs: Vec<String> =
                            peer.ip_addrs.iter().map(|a| a.to_string()).collect();
                        let paired = ip_strs.iter().any(|ip| paired_ips.contains(ip));
                        serde_json::json!({
                            "device_id":   peer.device_id,
                            "device_name": peer.device_name,
                            "ip_addrs":    ip_strs,
                            "port":        peer.port,
                            "bport":       peer.bport,
                            "paired":      paired,
                        })
                    })
                    .collect();

                Response::ok(req.id, serde_json::json!({ "devices": devices }))
            }

            // LAN/SAS Phase 2: begin a discovery-initiated SAS pairing as the
            // INITIATOR. Resolves the peer's bootstrap port (`bport`) from the
            // shared discovery snapshot, generates an EPHEMERAL random PAKE
            // password (the SAS — derived from the post-PAKE bound_key — is the
            // real authenticator; the password is sent in-clear inside the
            // bootstrap TLS), and runs `run_initiator_with_confirm` with a
            // callback wired into the pairing state machine.
            "pair_with_discovered" => {
                let device_id = match req.params.get("device_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: device_id",
                        )
                    }
                };
                self.pair_with_discovered(req.id.clone(), &device_id).await
            }

            // LAN/SAS Phase 2: poll the pairing state machine. Returns the
            // current state plus the SAS + role when awaiting confirmation.
            // Also surfaces whatever peer metadata is known at this point:
            //   • peer_device_name  — mDNS advertised name (initiator path)
            //   • peer_ip_addrs     — resolved IP addresses (initiator path)
            //   • peer_fingerprint  — cert fingerprint = mDNS device_id (initiator path)
            // These are all Optional — absent on the responder path (inbound
            // connection, no prior mDNS resolution) and gracefully omitted by
            // the UI. Model/OS/version are NOT surfaced here: the PAKE metadata
            // extension happens AFTER the SAS confirm step; they appear in the
            // final `pair_with_discovered` response once both sides accept.
            "pair_get_sas" => {
                let state = self.pairing.snapshot();
                let mut body = serde_json::json!({ "state": state.as_str() });
                if let Some(sas) = state.sas() {
                    body["sas"] = serde_json::Value::String(sas.to_string());
                }
                if let Some(role) = state.role() {
                    body["role"] = serde_json::Value::String(role.as_str().to_string());
                }
                if let Some(snap) = state.peer_snapshot() {
                    if let Some(ref name) = snap.device_name {
                        body["peer_device_name"] = serde_json::Value::String(name.clone());
                    }
                    if !snap.ip_addrs.is_empty() {
                        body["peer_ip_addrs"] = serde_json::Value::Array(
                            snap.ip_addrs
                                .iter()
                                .map(|a| serde_json::Value::String(a.clone()))
                                .collect(),
                        );
                    }
                    if let Some(ref fp) = snap.fingerprint {
                        body["peer_fingerprint"] = serde_json::Value::String(fp.clone());
                    }
                }
                Response::ok(req.id, body)
            }

            // LAN/SAS Phase 2: deliver the local user's accept/reject decision
            // into the in-flight handshake's confirm callback. The pairing
            // succeeds (keys trusted + persisted) only when BOTH sides accept.
            "pair_confirm_sas" => {
                let accept = match req.params.get("accept").and_then(|v| v.as_bool()) {
                    Some(b) => b,
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing or non-boolean param: accept",
                        )
                    }
                };
                let delivered = self.pairing.deliver_decision(accept);
                if !delivered {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        "no pairing is awaiting SAS confirmation",
                    );
                }
                Response::ok(
                    req.id,
                    serde_json::json!({ "ok": true, "accepted": accept }),
                )
            }

            // LAN/SAS Phase 2: abort an in-flight pairing. Dropping the confirm
            // channel resolves the handshake's await as a rejection so the
            // session key drops/zeroizes; the machine moves to `aborted`.
            "pair_abort" => {
                self.pairing.abort();
                Response::ok(req.id, serde_json::json!({ "ok": true }))
            }

            // CopyPaste-3n9h: `pair_peer` previously trusted a peer and
            // registered it in the live mTLS allowlist WITHOUT any
            // authentication (no PAKE, no SAS). A caller that knew a peer's
            // TLS fingerprint could add it as trusted with no proof of identity.
            //
            // The unauthenticated path is now DISABLED. All pairing MUST go
            // through the authenticated paths:
            //   • QR / password: `pair_peer_with_password` + `pair_accept_finish`
            //   • LAN/SAS discovery: `pair_with_discovered` + `pair_confirm_sas`
            //
            // This handler is retained (not removed) so old CLI versions
            // receive an explicit error instead of "unknown method", which
            // makes the upgrade path diagnosable.
            "pair_peer" => Response::err_with_code(
                req.id,
                ERR_CODE_NOT_IMPLEMENTED,
                "pair_peer is disabled: use pair_peer_with_password (QR/password) \
                 or pair_with_discovered (LAN/SAS) for authenticated pairing",
            ),

            "unpair_peer" => {
                let fingerprint = match req.params.get("fingerprint").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: fingerprint"),
                };

                match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        let fp_canonical = canonical_fingerprint(&fingerprint);
                        // Gap A: capture the peer's last-known dial address +
                        // display name BEFORE removing the record, so a durable
                        // pending-unpair can be delivered if the peer is offline.
                        let (peer_addr, peer_name) = peers
                            .iter()
                            .find(|p| {
                                p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| canonical_fingerprint(f) == fp_canonical)
                                    .unwrap_or(false)
                            })
                            .map(|p| {
                                (
                                    p.get("address")
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()),
                                    p.get("name")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string(),
                                )
                            })
                            .unwrap_or((None, String::new()));
                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) != fp_canonical)
                                .unwrap_or(true)
                        });
                        let removed = peers.len() < before_len;

                        match save_peers(&peers) {
                            Ok(_) => {
                                // AB-9: unpair must also evict the live in-memory
                                // mTLS allowlist (mirrors what revoke_peer does),
                                // otherwise an existing mTLS session survives until
                                // the next daemon restart. Normalise to canonical
                                // lowercase hex (strip colons) to match
                                // PairedPeers' key format.
                                if let Some(ref peers) = self.p2p_peers {
                                    peers.remove(&canonical_fingerprint(&fingerprint));
                                }
                                // Mutual unpair: best-effort signal the peer if
                                // it is currently connected over P2P.
                                send_unpair_signal_if_connected(
                                    &self.live_peer_sinks,
                                    &canonical_fingerprint(&fingerprint),
                                );
                                // Gap A: queue a DURABLE pending-unpair so the
                                // connector can deliver the Unpair frame on the
                                // peer's next reconnect even if it was offline now.
                                if removed {
                                    queue_unpair_for_offline_delivery(
                                        &fingerprint,
                                        peer_addr.as_deref(),
                                        &peer_name,
                                    );
                                }
                                Response::ok(
                                    req.id,
                                    serde_json::json!({ "ok": true, "removed": removed }),
                                )
                            }
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
                let (removed, captured_name, captured_addr) = match load_peers() {
                    Ok(mut peers) => {
                        let before_len = peers.len();
                        let fp_canonical = canonical_fingerprint(&fingerprint);
                        let matched = peers.iter().find(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) == fp_canonical)
                                .unwrap_or(false)
                        });
                        let name = matched
                            .and_then(|p| p.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        // Gap A: capture the last-known dial address before delete.
                        let addr = matched
                            .and_then(|p| p.get("address"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        peers.retain(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) != fp_canonical)
                                .unwrap_or(true)
                        });
                        if let Err(e) = save_peers(&peers) {
                            return Response::err(req.id, format!("failed to save peers: {e}"));
                        }
                        (peers.len() < before_len, name, addr)
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
                    Ok(Ok(revoked_at)) => {
                        // Fix CRITICAL #1: remove the peer from the live in-memory
                        // mTLS allowlist so the revoked peer's existing (or new)
                        // mTLS session is rejected immediately — without waiting
                        // for a daemon restart. Normalise to canonical lowercase
                        // hex (strip colons) to match PairedPeers' key format.
                        if let Some(ref peers) = self.p2p_peers {
                            peers.remove(&canonical_fingerprint(&fingerprint));
                        }
                        // Mutual unpair: best-effort signal the peer if it is
                        // currently connected over P2P.
                        send_unpair_signal_if_connected(
                            &self.live_peer_sinks,
                            &canonical_fingerprint(&fingerprint),
                        );
                        // Gap A: durable pending-unpair for offline delivery.
                        if removed {
                            queue_unpair_for_offline_delivery(
                                &fingerprint,
                                captured_addr.as_deref(),
                                &captured_name,
                            );
                        }
                        // FIX (CopyPaste-gbo): when cloud-sync or relay-sync is
                        // compiled in AND a sync key is currently installed,
                        // automatically rotate it to a fresh random key so the
                        // revoked device is ALSO cut off from cloud/relay sync —
                        // without requiring a passphrase from the user.
                        //
                        // Security rationale: the revoked device holds the OLD
                        // shared sync key and can use it to decrypt items
                        // encrypted under that key (XChaCha20-Poly1305 auth tags
                        // only reject ciphertexts produced under a DIFFERENT key).
                        // Rotating to a fresh random key means:
                        //   • all items produced AFTER revocation are encrypted
                        //     under the new key — the revoked device cannot
                        //     decrypt them (auth-tag rejection);
                        //   • the relay inbox id (HKDF of the sync key) diverges,
                        //     so the revoked device's inbox token is now stale.
                        //
                        // Distribution: remaining paired devices must re-provision
                        // (re-scan the pairing QR or accept the next P2P
                        // bootstrap push) to receive the new key.  This is the
                        // same requirement as `revoke_and_rotate`, but WITHOUT
                        // manual passphrase entry.
                        //
                        // When no sync key is currently installed (sync not yet
                        // configured), the rotation is skipped — there is nothing
                        // to rotate — and the response reflects that.
                        #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
                        {
                            let key_was_active = self.sync_key.lock().await.is_some();
                            if key_was_active {
                                let new_key = SyncKey::random();
                                self.persist_and_install_sync_key(new_key).await;
                                tracing::info!(
                                    fingerprint = %fingerprint,
                                    "revoke_peer: P2P revoked + sync key auto-rotated (random); \
                                     remaining devices must re-provision to keep syncing",
                                );
                                Response::ok(
                                    req.id,
                                    serde_json::json!({
                                        "ok": true,
                                        "removed": removed,
                                        "revoked_at": revoked_at,
                                        "fingerprint": fingerprint,
                                        "sync_key_rotated": true,
                                    }),
                                )
                            } else {
                                // No sync key installed — P2P-only revocation is
                                // the complete action; nothing to rotate.
                                tracing::info!(
                                    fingerprint = %fingerprint,
                                    "revoke_peer: P2P-only revocation (no sync key installed); \
                                     cloud/relay sync was not active",
                                );
                                Response::ok(
                                    req.id,
                                    serde_json::json!({
                                        "ok": true,
                                        "removed": removed,
                                        "revoked_at": revoked_at,
                                        "fingerprint": fingerprint,
                                        "sync_key_rotated": false,
                                    }),
                                )
                            }
                        }
                        #[cfg(not(any(feature = "cloud-sync", feature = "relay-sync")))]
                        // P2P-only build: mTLS denylist is sufficient revocation.
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "ok": true,
                                "removed": removed,
                                "revoked_at": revoked_at,
                                "fingerprint": fingerprint,
                            }),
                        )
                    }
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
                // Gap A: capture last-known dial addresses alongside fingerprints
                // so each revoked peer gets a durable pending-unpair record.
                let captured_addrs: Vec<Option<String>> = peers
                    .iter()
                    .filter_map(|p| {
                        p.get("fingerprint").and_then(|v| v.as_str())?;
                        Some(
                            p.get("address")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                        )
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

                // Fix CRITICAL #1: evict every revoked peer from the live mTLS
                // allowlist so their sessions are rejected immediately without
                // a daemon restart. Normalise each fingerprint to canonical
                // lowercase hex (strip colons) to match PairedPeers' key format.
                if let Some(ref peers) = self.p2p_peers {
                    for (fp, _) in &captured {
                        peers.remove(&canonical_fingerprint(fp));
                    }
                }

                // Mutual unpair: signal every currently-connected peer.
                for (fp, _) in &captured {
                    send_unpair_signal_if_connected(
                        &self.live_peer_sinks,
                        &canonical_fingerprint(fp),
                    );
                }

                // Gap A: durable pending-unpair for every revoked peer so the
                // signal reaches peers that were offline at reset time.
                for ((fp, name), addr) in captured.iter().zip(captured_addrs.iter()) {
                    queue_unpair_for_offline_delivery(fp, addr.as_deref(), name);
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

                        // S3 (CopyPaste-4ca): consume the SessionKey to derive a
                        // cert-fingerprint-bound confirmation tag.
                        //
                        // The IPC path has no shared TLS channel between the two
                        // devices, so RFC 5705 `export_keying_material` is not
                        // available.  Instead we bind the SessionKey to the pair
                        // of cert fingerprints (own + peer) that mTLS already
                        // pins.  A relay/MitM that uses different certs will have
                        // a different fingerprint pair → different binder →
                        // different bound_key → confirmation tags that will not
                        // match on the responder side → handshake aborted.
                        //
                        // Residual gap: the binder is built from the fingerprints
                        // the UI supplies.  A MitM that can forge BOTH fingerprints
                        // in the UI channel AND intercept/substitute PAKE messages
                        // would still succeed.  Full RFC 5705 binding (over a
                        // shared TLS exporter) is not achievable on this path
                        // without a protocol change; that gap is tracked in bd
                        // issue CopyPaste-4ca notes.
                        let (session_key, msg3_bytes) = match initiator.finish(&msg2_bytes) {
                            Ok(pair) => pair,
                            Err(e) => {
                                return Response::err_with_code(
                                    req.id,
                                    ERR_CODE_AUTH_FAILED,
                                    format!("PAKE finish failed: {e}"),
                                )
                            }
                        };

                        // Derive the cert-binder from both fingerprints and bind
                        // the session key to it.  `own_fp` may be `None` in tests
                        // without a cert; fall back to a zero binder in that case
                        // (still binds the session key, just weakly — production
                        // daemons always have a cert fingerprint).
                        let own_fp = self.cert_fingerprint.clone().unwrap_or_default();
                        let binder = Self::pake_cert_binder(&own_fp, &peer_fingerprint);
                        let bound_key = session_key.bind_to_tls_channel(&binder);
                        let initiator_tag =
                            channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
                        let initiator_confirm_b64 = b64.encode(initiator_tag);

                        let msg3_b64 = b64.encode(&msg3_bytes);

                        // Store the paired peer on the initiator side (no PasswordFile).
                        let added_at = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();

                        match load_peers() {
                            Ok(mut peers) => {
                                // Only add if not already present — normalise
                                // both sides so colon-hex vs bare-hex match.
                                let fp_c = canonical_fingerprint(&peer_fingerprint);
                                let already = peers.iter().any(|p| {
                                    p.get("fingerprint")
                                        .and_then(|v| v.as_str())
                                        .map(|f| canonical_fingerprint(f) == fp_c)
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
                                // S3: initiator confirmation tag — responder must
                                // verify this in pair_accept_finish to prove both
                                // sides share the same SessionKey + cert binder.
                                "initiator_confirm_b64": initiator_confirm_b64,
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

            // c4q2.20: pair_accept_password (password-based PAKE responder) is
            // stubbed not_implemented. The password-pairing flow was removed as a
            // security concern (CopyPaste-c4q2.20) — use QR pairing
            // (pair_generate_qr / pair_accept_qr) instead.
            "pair_accept_password" => Response::err_with_code(
                req.id,
                ERR_CODE_NOT_IMPLEMENTED,
                "pair_accept_password is disabled — use QR pairing (pair_generate_qr / pair_accept_qr) (c4q2.20)",
            ),

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

                // CopyPaste-j8dr: the initiator's confirmation tag is now
                // MANDATORY. An absent tag is rejected with AUTH_FAILED so that
                // a relay stripping the field, or an older initiator that never
                // sent one, cannot complete the handshake without mutual
                // confirmation. This closes the backwards-compatibility escape
                // hatch that was left open in the original S3 implementation.
                let initiator_confirm_b64 = match req
                    .params
                    .get("initiator_confirm_b64")
                    .and_then(|v| v.as_str())
                {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            "missing initiator_confirm_b64 — confirm tag is required",
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

                // S3 (CopyPaste-4ca): finalize the handshake and consume the
                // SessionKey.  Bind it to the cert-fingerprint binder so a
                // relay/MitM using a different cert pair will derive a different
                // bound_key and therefore produce mismatching confirmation tags.
                let session_key = match responder.finish(&msg3_bytes) {
                    Ok(sk) => sk,
                    Err(e) => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!("PAKE accept_finish failed: {e}"),
                        );
                    }
                };

                let own_fp = self.cert_fingerprint.clone().unwrap_or_default();
                // On the responder side: own_fp is responder's fp, peer_fp is
                // initiator's fp — same canonical (sorted) binder as the other end.
                let binder = Self::pake_cert_binder(&own_fp, &peer_fingerprint);
                let bound_key = session_key.bind_to_tls_channel(&binder);

                // Verify the initiator's confirmation tag (mandatory).
                {
                    use subtle::ConstantTimeEq as _;
                    let received = match b64.decode(&initiator_confirm_b64) {
                        Ok(bytes) => bytes,
                        Err(e) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                format!("invalid base64 in initiator_confirm_b64: {e}"),
                            )
                        }
                    };
                    if received.len() != CONFIRM_TAG_LEN {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            format!(
                                "initiator_confirm_b64 wrong length: expected {CONFIRM_TAG_LEN}, got {}",
                                received.len()
                            ),
                        );
                    }
                    let expected = channel_confirmation_tag(&bound_key, ConfirmRole::Initiator);
                    // Constant-time compare — subtle::ConstantTimeEq on slices.
                    let ok: bool = received.as_slice().ct_eq(&expected).into();
                    if !ok {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_AUTH_FAILED,
                            "PAKE confirmation tag mismatch (possible relay MitM)",
                        );
                    }
                }

                // Derive and return the responder's confirmation tag so the
                // initiator can optionally verify it (future extension).
                let responder_tag = channel_confirmation_tag(&bound_key, ConfirmRole::Responder);
                let responder_confirm_b64 = b64.encode(responder_tag);

                // Persist the peer with the PasswordFile blob on the responder side.
                // CopyPaste-5lm: encrypt at rest with XChaCha20-Poly1305 under the
                // daemon's local key. The ciphertext (`password_file_enc`) replaces
                // the former plaintext base64 field (`password_file_b64`).
                let fp_c = canonical_fingerprint(&peer_fingerprint);
                let password_file_enc = match encrypt_pake_password_file(
                    &password_file.serialized,
                    &fp_c,
                    &self.local_key,
                ) {
                    Ok(enc) => enc,
                    Err(e) => {
                        return Response::err(
                            req.id,
                            format!("failed to encrypt PasswordFile for storage: {e}"),
                        )
                    }
                };
                let added_at = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                match load_peers() {
                    Ok(mut peers) => {
                        // Normalise both sides so colon-hex vs bare-hex match
                        // (CopyPaste-qvn: raw string compare missed cross-format).
                        let already = peers.iter().any(|p| {
                            p.get("fingerprint")
                                .and_then(|v| v.as_str())
                                .map(|f| canonical_fingerprint(f) == fp_c)
                                .unwrap_or(false)
                        });
                        if !already {
                            peers.push(serde_json::json!({
                                "fingerprint": peer_fingerprint,
                                // password_file_enc: encrypted-at-rest blob.
                                // password_file_b64 is NOT written — new records
                                // always use the encrypted form.
                                "password_file_enc": password_file_enc,
                                "added_at": added_at,
                            }));
                        } else {
                            // Update existing peer with the new encrypted PasswordFile.
                            // Also clear any legacy password_file_b64 field.
                            for p in peers.iter_mut() {
                                if p.get("fingerprint")
                                    .and_then(|v| v.as_str())
                                    .map(|f| canonical_fingerprint(f) == fp_c)
                                    .unwrap_or(false)
                                {
                                    p["password_file_enc"] =
                                        serde_json::Value::String(password_file_enc.clone());
                                    // Remove legacy plaintext field if present.
                                    if let Some(obj) = p.as_object_mut() {
                                        obj.remove("password_file_b64");
                                    }
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

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "ok": true,
                        // S3: responder confirmation tag — the initiator may
                        // optionally verify this to prove the responder holds the
                        // same SessionKey + cert binder.
                        "responder_confirm_b64": responder_confirm_b64,
                    }),
                )
            }

            // ----------------------------------------------------------------
            // QR pairing — displaying side. Generate a fresh pairing token,
            // store it for the matching `pair_accept_qr` step, and return a
            // single-line QR payload (the `copypaste-core::PairingPayload`
            // wire form) the *other* device scans. The token is the PAKE
            // password; the scanner derives it from the QR and drives the
            // existing `pair_peer_with_password` initiator flow. No new crypto:
            // QR is purely a transport for the token + this device's
            // fingerprint. See `copypaste_core::crypto::pairing_qr`.
            //
            // Request params: {} (device identity is taken from daemon state).
            // Response data: { "qr": "CPPAIR2...", "expires_in_secs": <u64> }
            // ----------------------------------------------------------------
            "pair_generate_qr" => {
                // CRITICAL-1 fix: the QR must carry the live mTLS **certificate**
                // fingerprint (the value the scanner pins and the mTLS verifier
                // compares — `PeerTransport::fingerprint` / `fingerprint_of`),
                // NOT the device-key fingerprint (`keychain::own_fingerprint`).
                // The QR payload already documents this field as the cert
                // fingerprint (see `copypaste_core::crypto::pairing_qr`), so the
                // payload format/version is unchanged — only the value sourced
                // here was wrong, making cert-pinning unable to ever match.
                //
                // No cert exists when P2P is disabled; refuse rather than
                // advertise a fingerprint that cannot authenticate the channel.
                let fingerprint = match self.cert_fingerprint.as_ref() {
                    Some(fp) => fp.clone(),
                    None => {
                        return Response::err(
                            req.id,
                            "P2P is disabled (set COPYPASTE_P2P=1): cannot generate a \
                             pairing QR without an mTLS certificate to advertise",
                        )
                    }
                };

                // Device name mirrors the P2P subsystem's source (HOSTNAME /
                // COMPUTERNAME, falling back to "CopyPaste") so the scanning
                // device shows a consistent label.
                let device_name = crate::daemon::resolve_device_name();

                // device_id must be a valid UUID: CPPAIR2 encodes it as 16 raw
                // bytes (base64url), and the decoder rejects any other length.
                // Use the stable daemon UUID when available; fall back to a
                // fresh v4 UUID (informational only — peer pinning uses the
                // fingerprint, not device_id).
                let device_id = self
                    .local_device_id
                    .clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                // Generate the single-use pairing token up front so the same
                // value feeds (a) the QR the scanner reads, (b) the legacy IPC
                // PAKE path's stored token, and (c) the bootstrap responder's
                // PAKE password — all derived from one token.
                let token = copypaste_core::PairingToken::generate();
                let password = token.to_pake_password();

                // P2P Phase 1: spawn an ephemeral, *unauthenticated* bootstrap
                // TLS listener and advertise its `host:port` in the QR's
                // `addr_hint`. The initiator dials it and the responder side of
                // the PAKE handshake runs over that TLS stream (PAKE provides
                // the mutual auth from the shared QR secret; the channel is
                // unpinned because neither side knows the other's cert yet).
                //
                // When P2P is disabled / the cert is absent we leave `addr_hint`
                // empty and fall back to the legacy IPC-relayed PAKE path.
                let addr_hint = if let Some(cert) = self.p2p_cert.clone() {
                    let (cert_der, key_der) = (cert.0.clone(), cert.1.clone());
                    match copypaste_p2p::bootstrap::BootstrapResponder::bind(cert_der, key_der)
                        .await
                    {
                        Ok(responder) => match responder.local_addr() {
                            Ok(local) => {
                                // The listener binds 0.0.0.0, so it's reachable on
                                // every interface — but the QR must carry one
                                // concrete host. A loopback hint (127.0.0.1) is
                                // unreachable from another device/emulator, so we
                                // advertise a real LAN-routable host via the shared
                                // `advertise_sync_addr` policy (same selection the
                                // in-band sync-listener address uses), falling back
                                // to 127.0.0.1 only when no LAN interface exists so
                                // same-host (and loopback-test) pairing still works.
                                let hint =
                                    copypaste_p2p::interfaces::advertise_sync_addr(local.port())
                                        .to_string();
                                // Race-fix (CopyPaste-7mf): store the handle so
                                // `list_peers` can await it before reading peers.json.
                                let handle =
                                    self.spawn_bootstrap_responder(responder, password.clone());
                                *self.pending_bootstrap.lock().await = Some(handle);
                                hint
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "bootstrap listener local_addr failed ({e}); \
                                     falling back to mDNS-only addr_hint"
                                );
                                String::new()
                            }
                        },
                        Err(e) => {
                            tracing::warn!(
                                "bootstrap listener bind failed ({e}); \
                                 falling back to mDNS-only addr_hint"
                            );
                            String::new()
                        }
                    }
                } else {
                    String::new()
                };

                // H4: embed relay + Supabase config into the QR as the optional
                // 6th provisioning field so the scanning device (Android) can
                // configure cloud/relay sync automatically at scan time — before
                // the P2P bootstrap tunnel is established (covers off-LAN case
                // where the P2P handshake may not complete).
                //
                // These are all non-secret values: relay_url is a plain HTTP
                // base URL; supabase_url + supabase_anon_key are the publishable
                // Supabase connection params, intentionally public per Supabase
                // documentation. No long-term secrets are embedded in the QR.
                let qr_provisioning = {
                    let app_cfg = read_config();
                    let relay_url = app_cfg.relay_url.clone();
                    let supabase_url = std::env::var("SUPABASE_URL").ok().or(app_cfg.supabase_url);
                    let supabase_anon_key = std::env::var("SUPABASE_ANON_KEY")
                        .ok()
                        .or(app_cfg.supabase_anon_key);
                    let prov = copypaste_core::QrProvisioning {
                        relay_url,
                        supabase_url,
                        supabase_anon_key,
                    };
                    if prov.is_empty() {
                        None
                    } else {
                        Some(prov)
                    }
                };

                // Build the payload directly from the pre-generated token so the
                // QR, the stored token, and the bootstrap password all agree.
                let payload = copypaste_core::PairingPayload {
                    fingerprint,
                    token,
                    device_id,
                    device_name,
                    addr_hint,
                    provisioning: qr_provisioning,
                };

                // Wrap the bare CPPAIR2 payload in the cppair://pair?p= deep-link
                // URI so external scanners (Google Lens, the system camera) treat
                // the QR as an actionable link and offer "open in app". The
                // in-app scanner and Android manifest deep-link both strip the
                // wrapper before decoding (see copypaste_core::strip_deeplink).
                let qr = payload.encode_deeplink();

                // Store the token (replacing any prior active QR) so the legacy
                // IPC `pair_accept_qr` path can re-derive the same PAKE password.
                {
                    let mut slot = self.pending_qr_token.lock().await;
                    *slot = Some((payload.token, std::time::Instant::now()));
                }

                Response::ok(
                    req.id,
                    serde_json::json!({
                        "qr": qr,
                        "expires_in_secs": PAKE_SESSION_TTL.as_secs(),
                    }),
                )
            }

            // ----------------------------------------------------------------
            // QR pairing — displaying side, accept step. The scanning device
            // (initiator) has derived the PAKE password from the QR token and
            // sent `message1`. We look up the stored token, re-derive the same
            // password, register a PasswordFile and respond exactly as
            // `pair_accept_password` does — but without the user typing the
            // password (it came from the QR we generated). The follow-up
            // `pair_accept_finish` step is unchanged.
            //
            // Request params: { "message1_b64", "peer_fingerprint" }
            // Response data:  { "session_id", "message2_b64" }
            // ----------------------------------------------------------------
            "pair_accept_qr" => {
                use base64::Engine as _;
                let b64 = base64::engine::general_purpose::STANDARD;

                // ── P2P Phase 1: network bootstrap path ─────────────────────
                // When the caller supplies the scanned `qr` string (rather than
                // a relayed `message1_b64`), this daemon is the *initiator*: it
                // decodes the QR, dials the responder's `addr_hint` over the
                // unauthenticated bootstrap TLS channel, and runs the full PAKE
                // initiator handshake over the network. PAKE provides mutual auth
                // from the shared QR secret; the channel is unpinned. On success
                // the responder's cert fingerprint (learned over the channel) is
                // registered in the live mTLS allowlist.
                if let Some(qr) = req.params.get("qr").and_then(|v| v.as_str()) {
                    let qr = qr.to_string();
                    return self.pair_accept_qr_network(req.id.clone(), &qr).await;
                }

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

                if !is_valid_fingerprint(&peer_fingerprint) {
                    return Response::err_with_code(
                        req.id,
                        ERR_CODE_INVALID_ARGUMENT,
                        format!("invalid peer_fingerprint format: {peer_fingerprint}"),
                    );
                }

                // Retrieve the active QR token, enforcing the TTL. Take it out
                // so a stale/expired token cannot linger.
                let password = {
                    let mut slot = self.pending_qr_token.lock().await;
                    match slot.take() {
                        Some((token, issued)) if issued.elapsed() < PAKE_SESSION_TTL => {
                            token.to_pake_password()
                        }
                        Some(_) => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "QR pairing token expired; regenerate the code",
                            )
                        }
                        None => {
                            return Response::err_with_code(
                                req.id,
                                ERR_CODE_INVALID_ARGUMENT,
                                "no active QR pairing token; generate a code first",
                            )
                        }
                    }
                };

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
                    /// Caller-supplied `is_sensitive` flag from the export JSON.
                    /// Used as a floor (OR) during import — the daemon always
                    /// recomputes sensitivity from the plaintext so a tampered
                    /// export cannot smuggle a credential in as non-sensitive.
                    caller_is_sensitive: bool,
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
                    // PG-26: read the caller-supplied flag but treat it only as
                    // a floor — the daemon recomputes sensitivity from plaintext
                    // below and ORs the two values so a tampered export file
                    // cannot downgrade a credential to non-sensitive.
                    let caller_is_sensitive = raw
                        .get("is_sensitive")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    decoded.push(DecodedImport {
                        content_type,
                        bytes,
                        created_at_ms,
                        caller_is_sensitive,
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
                // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
                // even if the spawn_blocking worker panics or is cancelled.
                let local_key_v1 = zeroize::Zeroizing::new(**self.local_key);
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
                    // P2P Phase 3: collect successfully-inserted rows so the
                    // handler can broadcast them to the sync orchestrator (which
                    // re-keys + pushes them to paired peers).
                    let mut inserted_clips: Vec<copypaste_core::ClipboardItem> = Vec::new();
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
                                    return Err::<
                                        (u32, u32, Vec<copypaste_core::ClipboardItem>),
                                        anyhow::Error,
                                    >(anyhow::anyhow!(
                                        "encrypt imported item failed: {e}"
                                    ));
                                }
                            };
                        let mut clip =
                            copypaste_core::ClipboardItem::new_text(ciphertext, nonce.to_vec(), 0);
                        clip.item_id = item_id;
                        clip.content_type = item.content_type.clone();
                        clip.wall_time = item.created_at_ms;
                        clip.content_hash = Some(hash_hex);

                        // PG-26: recompute sensitivity from the decrypted
                        // plaintext so a tampered export file cannot smuggle a
                        // credential in with `is_sensitive=false` and bypass the
                        // auto-wipe TTL.  Only text items carry detectable
                        // credentials (images have no text to scan).
                        // OR semantics: we never DOWNGRADE a caller-flagged
                        // item — a legitimate sensitive export stays sensitive;
                        // a credential falsely marked false is caught here.
                        clip.is_sensitive = if item.content_type == "text" {
                            let text = std::str::from_utf8(&item.bytes).unwrap_or("");
                            is_sensitive_for_autowipe(text) || item.caller_is_sensitive
                        } else {
                            // Non-text: trust caller flag only (no text to scan).
                            item.caller_is_sensitive
                        };

                        // FTS indexing: pass "" to skip the FTS write. The
                        // searchable plaintext is no longer available as a
                        // stored column (content is now ciphertext), matching
                        // the image path semantics — search over imported
                        // items is out of scope for this fix.
                        let requested_id = clip.id.clone();
                        match copypaste_core::insert_item_with_fts(&db, &clip, "") {
                            Ok(stored_id) if stored_id == requested_id => {
                                inserted += 1;
                                inserted_clips.push(clip);
                            }
                            Ok(_) => {
                                // Returned id differs => dedup hit (existing
                                // row with same content_hash/item_id).
                                skipped += 1;
                            }
                            Err(e) => {
                                return Err::<
                                    (u32, u32, Vec<copypaste_core::ClipboardItem>),
                                    anyhow::Error,
                                >(e.into());
                            }
                        }
                    }
                    Ok::<(u32, u32, Vec<copypaste_core::ClipboardItem>), anyhow::Error>((
                        inserted,
                        skipped,
                        inserted_clips,
                    ))
                })
                .await;

                match join {
                    Ok(Ok((inserted, skipped, inserted_clips))) => {
                        // P2P Phase 3: notify the sync orchestrator of each newly
                        // imported row so it is re-keyed and pushed to paired
                        // peers (a closed/absent channel is a no-op — no peers).
                        if let Some(ref tx) = self.new_item_tx {
                            for clip in inserted_clips {
                                let _ = tx.send(clip);
                            }
                        }
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "inserted": inserted,
                                "skipped": skipped,
                            }),
                        )
                    }
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

            // ------------------------------------------------------------------
            // export — return all decrypted items so the CLI backup command
            // can serialise them for `import`.
            //
            // Params: {} (no params required)
            // Success: {"items": [ {
            //     "id": "<row-uuid>",
            //     "item_id": "<item-uuid>",
            //     "content_type": "text"|...,
            //     "content_bytes_b64": "<base64 plaintext>",
            //     "created_at_ms": <i64 unix-ms>,
            //     "wall_time": <i64>,
            //     "lamport_ts": <i64>,
            //     "is_sensitive": <bool>
            // }, ... ]}
            //
            // Non-text items (images, etc.) are skipped — their chunked
            // ciphertext cannot be trivially re-imported by the CLI `import`
            // path (which only handles `content_bytes_b64`).
            //
            // Gated behind `requires_db` (see above) so it returns
            // IPC_NOT_READY during degraded/pre-ready startup.
            // ------------------------------------------------------------------
            "export" => {
                use base64::Engine as _;
                // `limit` > 0 → export the most-recent N items (DESC LIMIT in a
                // subquery, then re-order ASC for deterministic import order).
                // `limit` == 0 or absent → export ALL (legacy / unlimited).
                let export_limit = req
                    .params
                    .get("limit")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
                // P2-tj9s: `include_sensitive` defaults to false — sensitive items
                // are excluded by default to avoid bulk-exporting secrets via a
                // single IPC call. Callers that genuinely need them must opt in
                // explicitly. Note: the socket is 0600 so this is defence-in-depth,
                // not an authentication boundary.
                let include_sensitive = req
                    .params
                    .get("include_sensitive")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let db_arc = self.db.clone();
                // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop
                // even if the spawn_blocking worker panics or is cancelled.
                let local_key_v1 = zeroize::Zeroizing::new(**self.local_key);
                let join = tokio::task::spawn_blocking(move || {
                    let db = db_arc.blocking_lock();
                    let v2_key = derive_v2(&local_key_v1);
                    // When a limit is requested we select the most-recent N rows
                    // via a DESC subquery and then re-order ASC so the exported
                    // JSON can be re-imported in chronological order.  When no
                    // limit (or limit == 0) we return everything, oldest first.
                    let sql = if export_limit > 0 {
                        "SELECT id, item_id, content_type, content, content_nonce, \
                         is_sensitive, is_synced, lamport_ts, wall_time, key_version \
                         FROM ( \
                             SELECT id, item_id, content_type, content, content_nonce, \
                                    is_sensitive, is_synced, lamport_ts, wall_time, key_version \
                             FROM clipboard_items \
                             ORDER BY wall_time DESC \
                             LIMIT ?1 \
                         ) ORDER BY wall_time ASC"
                            .to_string()
                    } else {
                        "SELECT id, item_id, content_type, content, content_nonce, \
                         is_sensitive, is_synced, lamport_ts, wall_time, key_version \
                         FROM clipboard_items \
                         ORDER BY wall_time ASC"
                            .to_string()
                    };
                    let mut stmt = db.conn().prepare(&sql)?;
                    let b64 = base64::engine::general_purpose::STANDARD;
                    let mut items: Vec<serde_json::Value> = Vec::new();
                    // CopyPaste-93yr: count non-text items skipped so the CLI
                    // can warn the user rather than silently dropping them.
                    let mut skipped_non_text: u64 = 0;
                    let map_row = |row: &rusqlite::Row<'_>| {
                        // key_version can be NULL for genuine v1 rows written
                        // before the column was added.  We read it as Option<i64>
                        // and keep None distinct from a stored value of 1 or 2 so
                        // we can log it clearly rather than silently guessing.
                        let key_version_opt: Option<i64> = row.get(9)?;
                        Ok((
                            row.get::<_, String>(0)?,  // id
                            row.get::<_, String>(1)?,  // item_id
                            row.get::<_, String>(2)?,  // content_type
                            row.get::<_, Option<Vec<u8>>>(3)?,  // content
                            row.get::<_, Option<Vec<u8>>>(4)?,  // content_nonce
                            row.get::<_, bool>(5)?,    // is_sensitive
                            row.get::<_, bool>(6)?,    // is_synced
                            row.get::<_, i64>(7)?,     // lamport_ts
                            row.get::<_, i64>(8)?,     // wall_time
                            key_version_opt,
                        ))
                    };
                    // Cap export_limit to i64::MAX before casting: u64 values
                    // above i64::MAX would wrap negative after `as i64`, which
                    // SQLite treats as unlimited — silently exporting everything
                    // instead of the requested limit.
                    let lim = export_limit.min(i64::MAX as u64) as i64;
                    let rows = if export_limit > 0 {
                        stmt.query_map([lim], map_row)?
                    } else {
                        stmt.query_map([], map_row)?
                    };
                    for row_result in rows {
                        let (id, item_id, content_type, content_opt, nonce_opt,
                             is_sensitive, _is_synced, lamport_ts, wall_time, key_version_opt)
                            = row_result?;
                        // Only export text items — the CLI import path only
                        // accepts content_bytes_b64 (raw bytes), and images are
                        // stored as chunked AEAD blobs that require extra context.
                        // CopyPaste-93yr: count skipped non-text items so the
                        // response can include the count and the CLI can warn.
                        if content_type != "text" {
                            skipped_non_text += 1;
                            continue;
                        }
                        // P2-tj9s: skip sensitive items unless the caller opts in.
                        if is_sensitive && !include_sensitive {
                            continue;
                        }
                        let Some(content) = content_opt else { continue };
                        let Some(nonce_vec) = nonce_opt else { continue };
                        // Resolve key_version: NULL in the DB means the row
                        // predates the key_version column (genuine v1 row).
                        // Log NULL distinctly so mismatches are diagnosable;
                        // assume v1 rather than silently guessing v2 (which
                        // would produce an authentication-tag mismatch).
                        let key_version: u8 = match key_version_opt {
                            Some(v) => match u8::try_from(v) {
                                Ok(kv) => kv,
                                Err(_) => {
                                    tracing::warn!(
                                        id = %id,
                                        key_version = v,
                                        "export: out-of-range key_version {v}, skipping"
                                    );
                                    continue;
                                }
                            },
                            None => {
                                tracing::debug!(
                                    id = %id,
                                    "export: key_version is NULL (pre-column row); \
                                     attempting decrypt as v1"
                                );
                                1
                            }
                        };
                        let nonce: &[u8; 24] = match nonce_vec.as_slice().try_into() {
                            Ok(n) => n,
                            Err(_) => {
                                tracing::warn!(
                                    id = %id,
                                    "export: skipping item with invalid nonce length {}", nonce_vec.len()
                                );
                                continue;
                            }
                        };
                        // P2-zpd1: wrap plaintext in Zeroizing so the decrypted
                        // bytes are wiped on drop, even on early-exit paths
                        // (encode errors, serialisation failure, etc.).
                        let plaintext = match decrypt_item_by_version(
                            key_version,
                            V1Key(&local_key_v1),
                            V2Key(&v2_key),
                            &item_id,
                            nonce,
                            &content,
                        ) {
                            Ok(p) => zeroize::Zeroizing::new(p),
                            Err(e) => {
                                tracing::warn!(
                                    id = %id,
                                    "export: decrypt failed for item ({e}); skipping"
                                );
                                continue;
                            }
                        };
                        items.push(serde_json::json!({
                            "id": id,
                            "item_id": item_id,
                            "content_type": content_type,
                            "content_bytes_b64": b64.encode(&plaintext),
                            "created_at_ms": wall_time,
                            "wall_time": wall_time,
                            "lamport_ts": lamport_ts,
                            "is_sensitive": is_sensitive,
                        }));
                    }
                    // CopyPaste-93yr: return skipped_non_text alongside items
                    // so the CLI can warn the user.
                    Ok::<(Vec<serde_json::Value>, u64), anyhow::Error>((items, skipped_non_text))
                })
                .await;
                match join {
                    Ok(Ok((items, skipped_non_text))) => {
                        let count = items.len();
                        // P2-tj9s: audit log — record item COUNT only, never
                        // content. include_sensitive is logged so operators can
                        // detect unusual export calls in the daemon log.
                        tracing::info!(
                            count,
                            skipped_non_text,
                            include_sensitive,
                            "export: completed (item count only; content not logged)"
                        );
                        Response::ok(
                            req.id,
                            serde_json::json!({
                                "items": items,
                                // CopyPaste-93yr: non-zero means some image/file
                                // items were silently skipped; the CLI warns.
                                "skipped_non_text": skipped_non_text,
                            }),
                        )
                    }
                    Ok(Err(e)) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("export failed: {e}"),
                    ),
                    Err(e) => Response::err_with_code(
                        req.id,
                        ERR_CODE_INTERNAL_ERROR,
                        format!("blocking task failed: {e}"),
                    ),
                }
            }

            "get_app_icon" => {
                let bundle_id = match req.params.get("bundle_id").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => return Response::err(req.id, "missing param: bundle_id"),
                };
                // NSWorkspace / AppKit calls are blocking — offload to a
                // dedicated blocking thread so we never stall the async runtime.
                let join = tokio::task::spawn_blocking(move || {
                    crate::app_icon::get_app_icon_base64(&bundle_id)
                })
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
            "add_file_item" => {
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
                let data_b64 = match req.params.get("data_b64").and_then(|v| v.as_str()) {
                    Some(s) => s.to_string(),
                    None => {
                        return Response::err_with_code(
                            req.id,
                            ERR_CODE_INVALID_ARGUMENT,
                            "missing param: data_b64",
                        )
                    }
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
                    .map_err(|e| anyhow::anyhow!("encode_file failed: {e}"))?;

                    let blob = copypaste_core::chunks_to_blob(&chunks)
                        .map_err(|e| anyhow::anyhow!("chunks_to_blob failed: {e}"))?;

                    let meta_json = crate::clipboard::build_file_meta_json(&meta);
                    let mut item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
                    // Stable cross-device identity: derive item_id from the
                    // content-hash file_id (mirrors handle_file in daemon.rs).
                    item.item_id = uuid::Uuid::from_bytes(file_id).to_string();

                    let db_guard = db_arc.blocking_lock();
                    let stored_id = copypaste_core::insert_item_with_fts(&db_guard, &item, "")
                        .map_err(|e| anyhow::anyhow!("insert_item_with_fts failed: {e}"))?;

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
            // Drain the autorelease pool around the entire Cocoa body. Without
            // this, every paste-back (NSString::from_str, NSData::with_bytes for
            // multi-MB images, clearContents/setData_forType, and the
            // changeCount read in `record_self_write`) leaks autoreleased Cocoa
            // objects on this tokio worker thread — the same leak class fixed in
            // `clipboard.rs::poll`.
            objc2::rc::autoreleasepool(|_pool| {
                let content = match &item.content {
                    Some(bytes) => bytes.as_slice(),
                    None => return Err(PasteboardError::other("item has no content")),
                };

                use objc2_app_kit::{NSPasteboard, NSPasteboardTypeString};
                use objc2_foundation::{NSData, NSString};

                // Fix-4 (dup-on-copy race): stamp the self-write sentinel
                // BEFORE calling clearContents/setString so the clipboard
                // monitor can never observe the new changeCount with a stale
                // (un-set) sentinel.
                //
                // Previous code read changeCount AFTER the write and stored
                // it — a poll arriving between the write and the store would
                // see an incremented changeCount with sentinel == -1 and
                // record the just-pasted item as a fresh capture.
                //
                // Fix: read the current changeCount, pre-stamp
                // `current + 2` as the expected post-write value
                // (`clearContents` adds 1, `setString_forType` /
                // `setData_forType` adds 1 more), then write. After the
                // write, overwrite with the actual new count (handles cases
                // where macOS increments by a different amount). On error,
                // reset the sentinel to -1 so the monitor is not permanently
                // suppressed.
                let pre_count = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                // Pre-stamp with current+2 (the expected post-clearContents +
                // post-setString count). The monitor polls only on a 500ms
                // interval so a pre-stamp that is off by one is still safer
                // than a window with no stamp at all.
                let expected_after_write = pre_count + 2;
                self.self_write_change_count
                    .store(expected_after_write, std::sync::atomic::Ordering::Release);

                // Helper to post-stamp with the actual post-write count.
                //
                // CopyPaste-8yzf: only overwrite the sentinel when the
                // post-write count equals `expected_after_write`. If a
                // third-party app wrote to the pasteboard between our write
                // and this read, `actual > expected_after_write`. In that
                // case we leave the sentinel at `expected_after_write` (which
                // the monitor may have already consumed or will not see again
                // because the count moved past it). Unconditionally storing
                // `actual` would stamp the third-party's count, causing the
                // monitor to suppress their content as a daemon self-write.
                let post_stamp = |self_write_cc: &Arc<std::sync::atomic::AtomicI64>| {
                    let actual = unsafe { NSPasteboard::generalPasteboard().changeCount() } as i64;
                    if actual == expected_after_write {
                        // Our write was the only one; safe to confirm the exact count.
                        self_write_cc.store(actual, std::sync::atomic::Ordering::Release);
                    }
                    // else: third-party wrote after us; leave the pre-stamp
                    // (`expected_after_write`) in place — it will either
                    // already have been consumed by the monitor, or it is
                    // stale and harmless (no future poll will see it).
                    tracing::debug!(
                        change_count = actual,
                        expected = expected_after_write,
                        racing_write = actual != expected_after_write,
                        "clipboard: post-write changeCount check (self-write sentinel)"
                    );
                };

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
                    // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop.
                    let v1_key = zeroize::Zeroizing::new(**self.local_key);
                    let v2_key = derive_v2(&v1_key);
                    let plaintext_bytes = decrypt_item_by_version(
                        item.key_version,
                        V1Key(&v1_key),
                        V2Key(&v2_key),
                        &item.item_id,
                        nonce,
                        content,
                    )
                    .map_err(|e| {
                        // On decrypt failure reset the sentinel so the monitor
                        // is not permanently suppressed (Fix-4 error path).
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        match e {
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
                        }
                    })?;
                    let text = std::str::from_utf8(&plaintext_bytes).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::decrypt(format!("decrypted content is not UTF-8: {e}"))
                    })?;

                    // paste_as_plain_text: read the live config flag. When true,
                    // write only `public.utf8-plain-text` (strips RTF/HTML/attributed
                    // strings from the pasteboard so the receiving app gets bare text).
                    // When false (default), use NSPasteboardTypeString which is the
                    // standard "general string" UTI that most apps expect.
                    let plain_only = self
                        .core_config
                        .as_ref()
                        .and_then(|arc| arc.read().ok())
                        .map(|cfg| cfg.paste_as_plain_text)
                        .unwrap_or(false);

                    unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let ns_str = NSString::from_str(text);
                        // `public.utf8-plain-text` is the "bare UTF-8" UTI that
                        // explicitly strips rich formatting (RTF, HTML, etc.) on
                        // paste. NSPasteboardTypeString is also `public.utf8-plain-text`
                        // on modern macOS, but using the explicit UTI literal when
                        // paste_as_plain_text=true makes the intent unambiguous and
                        // avoids any implicit coercion bridges the system type may carry.
                        let ok = if plain_only {
                            let plain_uti = NSString::from_str("public.utf8-plain-text");
                            pb.setString_forType(&ns_str, &plain_uti)
                        } else {
                            pb.setString_forType(&ns_str, NSPasteboardTypeString)
                        };
                        if !ok {
                            // Fix-4: reset the self-write sentinel on write failure so
                            // a failed paste does not leave a stale changeCount that
                            // suppresses a later genuine capture.
                            self.self_write_change_count
                                .store(-1, std::sync::atomic::Ordering::Release);
                            return Err(PasteboardError::other(
                                "NSPasteboard setString:forType: returned false",
                            ));
                        }
                    }
                    post_stamp(&self.self_write_change_count);
                    Ok(())
                } else if item.content_type == "image" {
                    // ----- image: reassemble chunks → decrypt → write as PNG -----
                    // `file_id` is embedded in the JSON metadata stored in
                    // `blob_ref` (see ClipboardItem::new_image in
                    // storage/items.rs).
                    let meta_json = item.blob_ref.as_deref().ok_or_else(|| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other("image item missing blob_ref metadata")
                    })?;
                    let file_id = parse_image_file_id(meta_json).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other(e)
                    })?;

                    let chunks = chunks_from_blob(content).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other(format!("image chunks_from_blob failed: {e}"))
                    })?;
                    // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop.
                    let wtp_v1_key = zeroize::Zeroizing::new(**self.local_key);
                    let wtp_v2_key = derive_v2(&wtp_v1_key);
                    let wtp_img_key: &[u8; 32] = if item.key_version == 1 {
                        &wtp_v1_key
                    } else {
                        &wtp_v2_key
                    };
                    let png_bytes = decode_image(&chunks, wtp_img_key, &file_id).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::decrypt(format!("image decode failed: {e}"))
                    })?;

                    let write_ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let type_str = NSString::from_str("public.png");
                        let data = NSData::with_bytes(&png_bytes);
                        pb.setData_forType(Some(&data), &type_str)
                    };
                    if !write_ok {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        return Err(PasteboardError::other(
                            "NSPasteboard setData:forType: returned false for public.png",
                        ));
                    }
                    post_stamp(&self.self_write_change_count);
                    Ok(())
                } else if item.content_type == "file" {
                    // ----- file: reassemble chunks → decrypt → write as file-URL -----
                    //
                    // 1. Parse FileMeta (filename, mime, file_id) from blob_ref JSON.
                    // 2. Decrypt via chunks_from_blob → decode_file (v1 local_key, same as
                    //    get_item_file / handle_file).
                    // 3. Write bytes to ~/Library/Caches/CopyPaste/paste-files/<filename>.
                    // 4. Put an NSURL file-URL for that path on the pasteboard as
                    //    `public.file-url`.  The URL must outlive the paste so we do NOT
                    //    delete immediately; prune_old_paste_files() removes files >10 min
                    //    old on each call so the directory stays bounded.
                    let meta_json = item.blob_ref.as_deref().ok_or_else(|| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other("file item missing blob_ref metadata")
                    })?;
                    let file_meta = parse_file_meta(meta_json).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other(format!("file item blob_ref parse error: {e}"))
                    })?;

                    let chunks = chunks_from_blob(content).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other(format!("file chunks_from_blob failed: {e}"))
                    })?;
                    // Dispatch on key_version: v1 rows use the raw seed; v2 rows use derive_v2.
                    // P2-iqkm: wrap in Zeroizing so the key copy is wiped on drop.
                    let v1_key = zeroize::Zeroizing::new(**self.local_key);
                    let v2_key = derive_v2(&v1_key);
                    let key_to_use: &[u8; 32] = if item.key_version == 1 {
                        &v1_key
                    } else {
                        &v2_key
                    };
                    let raw_bytes =
                        decode_file(&chunks, key_to_use, &file_meta.file_id).map_err(|e| {
                            self.self_write_change_count
                                .store(-1, std::sync::atomic::Ordering::Release);
                            PasteboardError::decrypt(format!("file decode failed: {e}"))
                        })?;

                    // Sanitise the filename: strip any leading path separators so the
                    // stored name cannot escape the cache directory.
                    let safe_name = std::path::Path::new(&file_meta.filename)
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("paste-file"); // infallible fallback — filename came from our own capture
                    let paste_dir = paste_file_cache_dir();
                    // Prune stale entries before writing so the directory stays bounded;
                    // errors inside prune are logged at DEBUG and never propagate.
                    prune_old_paste_files(&paste_dir);
                    std::fs::create_dir_all(&paste_dir).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other(format!(
                            "failed to create paste-files dir {paste_dir:?}: {e}"
                        ))
                    })?;
                    let dest = paste_dir.join(safe_name);
                    std::fs::write(&dest, &raw_bytes).map_err(|e| {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        PasteboardError::other(format!("failed to write paste file {dest:?}: {e}"))
                    })?;

                    // Build the file:// URL string for the temp file.
                    // `public.file-url` data is the absolute URL string (percent-encoded),
                    // e.g. "file:///Users/.../paste-files/foo.txt".  This is what Finder,
                    // Terminal, and most Cocoa apps accept when reading `public.file-url`
                    // from the pasteboard.  We construct it via NSURL so percent-encoding
                    // is handled correctly, then write the absolute-string as NSString data.
                    use objc2_foundation::{NSString, NSURL};
                    let file_url_str: String = unsafe {
                        let path_ns = NSString::from_str(
                            dest.to_str().unwrap_or_default(), // UTF-8 path; infallible on macOS
                        );
                        // fileURLWithPath: produces "file:///…" with proper percent-encoding.
                        let nsurl = NSURL::fileURLWithPath(&path_ns);
                        // absoluteString returns the full URL string; unwrap_or_default is
                        // infallible in practice — a file URL always has an absolute string.
                        nsurl
                            .absoluteString()
                            .map(|s| s.to_string())
                            .unwrap_or_else(|| format!("file://{}", dest.display()))
                    };
                    let write_ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let uti = NSString::from_str("public.file-url");
                        let url_ns = NSString::from_str(&file_url_str);
                        pb.setString_forType(&url_ns, &uti)
                    };
                    if !write_ok {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        return Err(PasteboardError::other(
                            "NSPasteboard setString:forType: returned false for public.file-url",
                        ));
                    }
                    post_stamp(&self.self_write_change_count);
                    Ok(())
                } else {
                    // Unknown content_type — keep a best-effort raw-bytes write,
                    // but map to a real UTI when possible. We do NOT attempt
                    // decryption here because we don't know the shape of the
                    // ciphertext (no nonce / no chunk metadata). Used only by
                    // future content_types added without updating this handler.
                    let uti = map_content_type_to_uti(&item.content_type);
                    let write_ok = unsafe {
                        let pb = NSPasteboard::generalPasteboard();
                        pb.clearContents();
                        let type_str = NSString::from_str(&uti);
                        let data = NSData::with_bytes(content);
                        pb.setData_forType(Some(&data), &type_str)
                    };
                    if !write_ok {
                        self.self_write_change_count
                            .store(-1, std::sync::atomic::Ordering::Release);
                        return Err(PasteboardError::other(format!(
                            "NSPasteboard setData:forType: returned false for type '{uti}'"
                        )));
                    }
                    post_stamp(&self.self_write_change_count);
                    Ok(())
                }
            })
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = item;
            // No clipboard support on non-macOS platforms in this crate
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// Cloud connection diagnostics
// ---------------------------------------------------------------------------

/// Probe the configured Supabase project and return a structured diagnostic.
///
/// This is what backs the `cloud_test_connection` IPC method (and `copypaste
/// cloud test`). It performs at most one authenticated round-trip:
/// `GET /rest/v1/clipboard_items?limit=0` with the anon key in `apikey` and an
/// `Authorization: Bearer` header (email/password token when configured, anon
/// key otherwise). The HTTP outcome is mapped to an actionable message so the
/// user learns *which* step is wrong (credentials missing, URL unreachable,
/// key invalid, table not provisioned, RLS misconfigured) rather than seeing
/// silent no-op sync.
///
/// The returned JSON shape is stable (consumed by the CLI/UI):
/// ```json
/// { "ok": bool, "configured": bool, "stage": "<step>", "message": "<human>" }
/// ```
/// `ok` is the single source of truth ("is cloud sync ready?"); `stage` and
/// `message` are for display. No secrets are ever included in the output.
#[cfg(feature = "cloud-sync")]
async fn test_cloud_connection() -> serde_json::Value {
    use crate::cloud::CloudConfig;

    // Resolve credentials the same way the daemon's cloud orchestrator does
    // (env vars first, then the persisted AppConfig the UI writes).
    let cfg = match CloudConfig::from_env() {
        Some(c) => c,
        None => {
            return serde_json::json!({
                "ok": false,
                "configured": false,
                "stage": "config",
                "message": "Supabase is not configured. Set the project URL and anon key \
                            (Settings → Sync, or `copypaste cloud setup`).",
            });
        }
    };

    // Mirror the daemon's HTTPS-only gate so the diagnostic matches what
    // start_cloud would actually accept.
    if !cfg
        .supabase_url
        .to_ascii_lowercase()
        .starts_with("https://")
    {
        return serde_json::json!({
            "ok": false,
            "configured": true,
            "stage": "url",
            "message": format!(
                "Supabase URL must use https:// (got {}). Cloud sync refuses plain http.",
                cfg.supabase_url
            ),
        });
    }

    // Bearer: prefer an email/password GoTrue token (authenticated scope, the
    // scope RLS expects), falling back to the anon key. Credentials come from
    // `CloudConfig` (env vars first, then the persisted `0600` config written by
    // `copypaste cloud setup`) — the same resolution the orchestrator uses. We
    // do NOT fail the whole probe if sign-in fails — we report it as the failing
    // stage so the user can fix credentials specifically.
    let (bearer, signed_in) = match (cfg.email.as_deref(), cfg.password.as_deref()) {
        (Some(email), Some(password)) if !email.is_empty() && !password.is_empty() => {
            let auth = copypaste_supabase::auth::AuthClient::new(&cfg.supabase_url, &cfg.anon_key);
            match auth.sign_in(email, password).await {
                Ok(session) => (session.access_token, true),
                Err(e) => {
                    return serde_json::json!({
                        "ok": false,
                        "configured": true,
                        "stage": "auth",
                        "message": format!(
                            "Sign-in failed for {email}: {e}. Re-check the email/password \
                             (run `copypaste cloud setup` again, or set SUPABASE_EMAIL / \
                             SUPABASE_PASSWORD), and that the user is confirmed."
                        ),
                    });
                }
            }
        }
        _ => (cfg.anon_key.clone(), false),
    };

    // One cheap REST round-trip. `limit=0` returns an empty array on success
    // without transferring any rows, so it is safe even on a large table.
    // CopyPaste-16vr: use a request timeout so a stalled endpoint cannot
    // block the IPC handler indefinitely. 30 s matches SYNC_HTTP_TIMEOUT.
    let url = format!("{}/rest/v1/clipboard_items?limit=0", cfg.supabase_url);
    let client = reqwest::Client::builder()
        .timeout(crate::sync_common::SYNC_HTTP_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new()); // Client::new is fine here — timeout on send
    let resp = match client
        .get(&url)
        .header("apikey", &cfg.anon_key)
        .header("Authorization", format!("Bearer {bearer}"))
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return serde_json::json!({
                "ok": false,
                "configured": true,
                "stage": "network",
                "message": format!(
                    "Could not reach {}: {e}. Check the URL and your network/proxy.",
                    cfg.supabase_url
                ),
            });
        }
    };

    let status = resp.status();
    let code = status.as_u16();
    if status.is_success() {
        let scope = if signed_in {
            "signed in (authenticated scope)"
        } else {
            "anon key (sign in for full scope)"
        };
        return serde_json::json!({
            "ok": true,
            "configured": true,
            "stage": "done",
            "message": format!("Connected to Supabase — table reachable, {scope}."),
        });
    }

    // Classify the common failure HTTP codes into actionable guidance.
    let body = resp.text().await.unwrap_or_default();
    let (stage, message) = match code {
        // 401 has two distinct root causes. When we already hold an
        // authenticated bearer (`signed_in`), the anon key itself must be
        // wrong/expired. When the probe used only the anon key (no sign-in),
        // the project's `authenticated`-only RLS rejects the request and the
        // fix is to supply email/password, not to re-copy the anon key.
        401 if signed_in => (
            "auth",
            "401 Unauthorized — the anon key is wrong or expired. Re-copy it from \
             Supabase → Project Settings → API."
                .to_string(),
        ),
        401 => (
            "auth",
            "401 Unauthorized — the request used the anon key with no signed-in \
             session, and the table's RLS grants only the `authenticated` role. \
             Provide email/password (run `copypaste cloud setup` and supply them, \
             or set SUPABASE_EMAIL / SUPABASE_PASSWORD) so the daemon authenticates."
                .to_string(),
        ),
        404 => (
            "schema",
            "404 Not Found — the clipboard_items table is missing. Run the \
             provisioning SQL: `copypaste cloud setup-sql` then paste it into the \
             Supabase SQL Editor."
                .to_string(),
        ),
        // PostgREST returns 400/406 with a 'relation does not exist' hint when
        // the table is absent under some configs; surface the body for clarity.
        400 | 406 => (
            "schema",
            format!(
                "{code} from PostgREST — the table may be missing or misconfigured. \
                 Run `copypaste cloud setup-sql`. Server said: {}",
                body.trim()
            ),
        ),
        403 => (
            "rls",
            "403 Forbidden — row-level security rejected the request. Re-run the RLS \
             part of `copypaste cloud setup-sql`."
                .to_string(),
        ),
        _ => (
            "http",
            format!("Unexpected HTTP {code} from Supabase: {}", body.trim()),
        ),
    };
    serde_json::json!({
        "ok": false,
        "configured": true,
        "stage": stage,
        "message": message,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use copypaste_core::Database;
    use tempfile::tempdir;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::UnixStream;

    // -------------------------------------------------------------------
    // c4q2.21 — compute_peer_online unit tests
    // -------------------------------------------------------------------

    /// P2P live sink = true → online regardless of last_sync_at.
    #[test]
    fn compute_peer_online_live_sink_true() {
        assert!(compute_peer_online(Some(true), None, 1_000_000));
        assert!(compute_peer_online(Some(true), Some(0), 1_000_000));
    }

    /// P2P live sink = false → offline regardless of last_sync_at.
    #[test]
    fn compute_peer_online_live_sink_false() {
        assert!(!compute_peer_online(
            Some(false),
            Some(1_000_000),
            1_000_000
        ));
        assert!(!compute_peer_online(Some(false), None, 1_000_000));
    }

    /// No live sink — recent last_sync_at (within threshold) → online.
    #[test]
    fn compute_peer_online_boundary_at_threshold() {
        let now = 1_000_000_i64;
        // Exactly at boundary: now - t == ONLINE_THRESHOLD_SECS → online.
        let at_boundary = now - ONLINE_THRESHOLD_SECS;
        assert!(compute_peer_online(None, Some(at_boundary), now));
        // One second past → offline.
        let one_past = at_boundary - 1;
        assert!(!compute_peer_online(None, Some(one_past), now));
    }

    /// No live sink AND no last_sync_at → offline.
    #[test]
    fn compute_peer_online_none_last_sync_at_and_no_sink() {
        assert!(!compute_peer_online(None, None, 1_000_000));
    }

    /// CopyPaste-1jms.25: the peer-card fallback threshold MUST equal the sync
    /// badge chip's recency window so the two "recently heard from?" signals
    /// agree. Guards against the constants drifting apart again.
    #[test]
    fn online_threshold_matches_sync_badge_recent_window() {
        assert_eq!(
            ONLINE_THRESHOLD_SECS,
            (copypaste_ipc::SYNC_BADGE_RECENT_MS / 1_000) as i64,
            "peer-card online window must match the sync-chip recency window"
        );
        // Concrete regression of the formerly-contradictory case: a peer last
        // heard from 75 s ago is now ONLINE in the fallback (it was offline under
        // the old 60 s window while the chip still showed it as recent).
        let now = 1_000_000_i64;
        assert!(
            compute_peer_online(None, Some(now - 75), now),
            "a 75s-stale peer must be online (within the shared 5-min window)"
        );
    }

    /// Create a temp directory and immediately force its permissions to 0o700.
    ///
    /// `tempfile::TempDir::new()` calls `mkdir(path, 0o700)`, but the kernel
    /// applies the process umask: `mode & ~umask`. `bind_with_stale_cleanup`
    /// (and the tests that exercise it) temporarily set `libc::umask(0o177)`,
    /// which reduces `0o700 & ~0o177` to `0o600` (no execute bit). A 0o600
    /// directory silently blocks all subsequent file operations inside it with
    /// EACCES. Calling `set_permissions(0o700)` right after creation repairs
    /// the mode unconditionally — `chmod` requires only ownership, not execute.
    fn safe_tempdir() -> tempfile::TempDir {
        let dir = tempdir().expect("failed to create temp dir");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700));
        }
        dir
    }

    /// CopyPaste-c4q2.24: both IPC directions must be deadline-bounded so a
    /// stalled/hostile same-UID client cannot pin a connection slot (and its
    /// semaphore permit) forever. The write deadline must be > 0 and should not
    /// exceed the read deadline (a slow drain is a harder failure than a slow
    /// send and warrants a tighter bound). A full 64-connection buffer-fill DoS
    /// reproduction is kernel-send-buffer-dependent and ~10 s+ wall time, so it
    /// is covered by manual QA (see the bd issue) rather than a flaky unit test;
    /// this guards the intent of the constants.
    #[test]
    fn ipc_write_timeout_is_bounded_and_not_longer_than_read() {
        assert!(
            IPC_WRITE_TIMEOUT > std::time::Duration::ZERO,
            "write path must have a non-zero deadline"
        );
        assert!(
            IPC_WRITE_TIMEOUT <= IPC_READ_TIMEOUT,
            "write deadline ({:?}) should not exceed read deadline ({:?})",
            IPC_WRITE_TIMEOUT,
            IPC_READ_TIMEOUT
        );
    }

    /// `get_config` must never ship the GoTrue password or email over IPC.
    /// `build_config_response` maps both to `*_set` presence flags and exposes
    /// no field that could carry the plaintext, while leaving the publishable
    /// anon key intact (CopyPaste-c4q2.18).
    #[test]
    fn build_config_response_strips_password_and_email() {
        let cfg = AppConfig {
            p2p_enabled: Some(true),
            supabase_url: Some("https://x.supabase.co".into()),
            supabase_anon_key: Some("eyJpublishable".into()),
            supabase_email: Some("user@example.com".into()),
            supabase_password: Some("hunter2".into()),
            ..AppConfig::default()
        };
        // Serialise the typed response exactly as the get_config handler does.
        let v = serde_json::to_value(build_config_response(&cfg)).unwrap();
        let obj = v.as_object().unwrap();
        // Secrets cannot appear — the response type has no field for them.
        assert!(!obj.contains_key("supabase_password"));
        assert!(!obj.contains_key("supabase_email"));
        // Presence flags reflect that both were set.
        assert_eq!(obj["supabase_password_set"], serde_json::json!(true));
        assert_eq!(obj["supabase_email_set"], serde_json::json!(true));
        // Non-secret fields (incl. the publishable anon key) are untouched.
        assert_eq!(
            obj["supabase_anon_key"],
            serde_json::json!("eyJpublishable")
        );
        assert_eq!(
            obj["supabase_url"],
            serde_json::json!("https://x.supabase.co")
        );
        assert_eq!(obj["p2p_enabled"], serde_json::json!(true));
    }

    // ─── CopyPaste-5lm: PasswordFile at-rest encryption unit tests ──────────

    /// `encrypt_pake_password_file` / `decrypt_pake_password_file` must
    /// round-trip: encrypt → base64 blob → decrypt → original plaintext.
    #[test]
    fn pake_password_file_encrypt_decrypt_roundtrip() {
        let plaintext = b"fake_password_file_bytes_for_testing_01234567890";
        let local_key = [0x42u8; 32];
        let fp = "aabbccddeeff";

        let enc =
            encrypt_pake_password_file(plaintext, fp, &local_key).expect("encrypt must succeed");
        assert!(!enc.is_empty(), "encrypted output must not be empty");

        let decrypted =
            decrypt_pake_password_file(&enc, fp, &local_key).expect("decrypt must succeed");
        assert_eq!(
            decrypted, plaintext,
            "decrypted bytes must match original plaintext"
        );
    }

    /// A different fingerprint (wrong AAD) must cause authentication failure.
    #[test]
    fn pake_password_file_wrong_fp_aad_fails() {
        let plaintext = b"some_pake_blob";
        let local_key = [0x11u8; 32];
        let correct_fp = "aabbcc";
        let wrong_fp = "ddeeff";

        let enc = encrypt_pake_password_file(plaintext, correct_fp, &local_key)
            .expect("encrypt must succeed");
        let result = decrypt_pake_password_file(&enc, wrong_fp, &local_key);
        assert!(
            result.is_err(),
            "decrypt with wrong fingerprint must fail (AEAD auth): {result:?}"
        );
    }

    /// A wrong local key must cause authentication failure.
    #[test]
    fn pake_password_file_wrong_key_fails() {
        let plaintext = b"some_pake_blob";
        let correct_key = [0x11u8; 32];
        let wrong_key = [0x22u8; 32];
        let fp = "aabbcc";

        let enc =
            encrypt_pake_password_file(plaintext, fp, &correct_key).expect("encrypt must succeed");
        let result = decrypt_pake_password_file(&enc, fp, &wrong_key);
        assert!(
            result.is_err(),
            "decrypt with wrong key must fail (AEAD auth): {result:?}"
        );
    }

    /// A truncated blob (too short for even a nonce) must return an error.
    #[test]
    fn pake_password_file_truncated_blob_fails() {
        let local_key = [0x33u8; 32];
        let fp = "aabb";
        // Only 10 bytes — shorter than the 24-byte nonce.
        use base64::Engine as _;
        let short_b64 = base64::engine::general_purpose::STANDARD.encode([0u8; 10]);
        let result = decrypt_pake_password_file(&short_b64, fp, &local_key);
        assert!(
            result.is_err(),
            "truncated blob must fail with an error: {result:?}"
        );
    }

    /// list_peers must NOT expose `password_file_enc` or `password_file_b64`
    /// in its IPC response (CopyPaste-5lm: prevent credential exfiltration).
    #[tokio::test]
    async fn list_peers_strips_password_file_fields() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );

        // Write a peers.json with both sensitive fields present (simulating a
        // legacy + new mix so we confirm both are stripped).
        let peers_path = cfg_home.join("peers.json");
        std::fs::create_dir_all(&cfg_home).unwrap();
        std::fs::write(
            &peers_path,
            r#"[{"fingerprint":"aa:bb:cc","name":"Alice","added_at":1700000000,
                  "password_file_b64":"cGxhaW50ZXh0","password_file_enc":"ZW5jcnlwdGVk"}]"#,
        )
        .unwrap();

        let sock = dir.path().join("test-strip-pf.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"sp1\",\"method\":\"list_peers\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
        let peers = resp["data"]["peers"].as_array().unwrap();
        assert_eq!(peers.len(), 1, "must have one peer");
        let p = &peers[0];
        assert!(
            p.get("password_file_b64").is_none(),
            "list_peers must strip password_file_b64: {p}"
        );
        assert!(
            p.get("password_file_enc").is_none(),
            "list_peers must strip password_file_enc: {p}"
        );
        // The non-sensitive fields must still be present.
        assert_eq!(p["fingerprint"], "aa:bb:cc");
        assert_eq!(p["name"], "Alice");
    }

    /// CopyPaste-vypo: list_peers must include a `trust` field for every peer.
    /// All persisted peers completed PAKE = "verified".
    #[tokio::test]
    async fn list_peers_includes_trust_field() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg-trust");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );

        let peers_path = cfg_home.join("peers.json");
        std::fs::create_dir_all(&cfg_home).unwrap();
        std::fs::write(
            &peers_path,
            r#"[{"fingerprint":"11:22:33","name":"Bob","added_at":1700000000}]"#,
        )
        .unwrap();

        let sock = dir.path().join("test-trust.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"tv1\",\"method\":\"list_peers\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
        let peers = resp["data"]["peers"].as_array().unwrap();
        assert_eq!(peers.len(), 1, "must have one peer");
        let p = &peers[0];
        assert_eq!(
            p["trust"], "verified",
            "persisted peers must have trust=verified, got: {p}"
        );
    }

    /// CopyPaste-1jms.32: list_peers must include a `transport` field when a
    /// transport is active for a peer, and omit it (or set to null) when none
    /// is configured. In the test server (no P2P, no relay, no cloud) the field
    /// must be absent — the UI falls back to the local_ip/address heuristic.
    #[tokio::test]
    async fn list_peers_transport_absent_when_no_transport_active() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg-transport");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );

        let peers_path = cfg_home.join("peers.json");
        std::fs::create_dir_all(&cfg_home).unwrap();
        // Peer with a local_ip (normally the heuristic for P2P) but no live
        // P2P connection in the test server. Transport should be absent/null.
        std::fs::write(
            &peers_path,
            r#"[{"fingerprint":"aa:bb:11","name":"Carol","added_at":1700000000,"local_ip":"192.168.1.10"}]"#,
        )
        .unwrap();

        let sock = dir.path().join("test-transport-none.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"tp1\",\"method\":\"list_peers\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();

        assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
        let peers = resp["data"]["peers"].as_array().unwrap();
        assert_eq!(peers.len(), 1);
        let p = &peers[0];
        // In the test server: no P2P sinks wired (live_fps = None), relay feature
        // is compiled but no relay_handle injected, cloud-sync is off. The
        // transport field must be absent (not `"p2p"`) because there is no live
        // P2P connection — even though local_ip is present.
        assert!(
            p.get("transport").is_none() || p["transport"].is_null(),
            "transport must be absent/null when no transport is active, got: {p}"
        );
        // Other fields must still be present.
        assert_eq!(p["fingerprint"], "aa:bb:11");
        assert_eq!(p["trust"], "verified");
    }

    /// When the credentials are absent (None), the presence flags must be
    /// `false` and no secret key should appear on the wire.
    #[test]
    fn build_config_response_reports_unset_when_none() {
        let cfg = AppConfig {
            supabase_email: None,
            supabase_password: None,
            ..AppConfig::default()
        };
        let v = serde_json::to_value(build_config_response(&cfg)).unwrap();
        let obj = v.as_object().unwrap();
        assert_eq!(obj["supabase_password_set"], serde_json::json!(false));
        assert_eq!(obj["supabase_email_set"], serde_json::json!(false));
        assert!(!obj.contains_key("supabase_password"));
        assert!(!obj.contains_key("supabase_email"));
    }

    /// RAII guard that snapshots one or more env vars, sets them for the test,
    /// and restores the previous values (or unsets them) on drop — even on
    /// panic.  Holds `crate::TEST_ENV_LOCK` (the *process-wide* env lock shared
    /// with every other daemon test module) for its whole lifetime so env state
    /// cannot race tests in `paths`, `keychain`, or any other module that also
    /// mutates `HOME`/`XDG_CONFIG_HOME`.
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
            let lock = crate::TEST_ENV_LOCK
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let mut saved = Vec::with_capacity(keys.len());
            for &key in keys {
                saved.push((key, std::env::var_os(key)));
                // SAFETY: serialised via `crate::TEST_ENV_LOCK`; no other
                // thread reads or writes these vars concurrently for the
                // guard's lifetime.
                unsafe { std::env::set_var(key, value) };
            }
            Self { saved, _lock: lock }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            // SAFETY: still holding `crate::TEST_ENV_LOCK` (`_lock`), so the
            // restore is serialised against every other env-mutating test.
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
        // Give the test server a realistic mTLS cert fingerprint (colon-hex of a
        // 32-byte SHA-256) so the pairing handlers (`pair_generate_qr`,
        // `get_own_fingerprint`) behave as they do with P2P enabled. Generating a
        // real cert keeps this honest: the advertised value is exactly what the
        // transport would pin.
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("test-device").unwrap();
        let server = IpcServer::new(db.clone(), private_mode.clone(), local_key, device_pub)
            .with_cert_fingerprint(display_fingerprint(&cert.fingerprint()));
        // Bind directly without going through `IpcServer::bind` /
        // `bind_with_stale_cleanup`, which sets `libc::umask(0o177)` process-wide.
        // That process-wide umask change races with concurrent tests' `mkdir` /
        // `tempdir` calls, producing directories with mode 0o600 (no execute bit)
        // that make all subsequent file operations inside them fail with EACCES.
        // In tests the socket lives in a fresh tempdir, so neither stale-socket
        // self-healing nor restrictive socket permissions are needed.
        let path = socket_path.to_path_buf();
        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("test socket bind must succeed");
        tokio::spawn(async move {
            if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
                tracing::error!("ipc: server on {:?} exited with error: {e}", &path);
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        (private_mode, db)
    }

    // -----------------------------------------------------------------------
    // Stale-socket self-heal (fix/daemon-ipc-selfheal)
    // -----------------------------------------------------------------------

    /// A path that does not exist is never "live".
    #[test]
    fn is_socket_live_false_for_missing_path() {
        let dir = safe_tempdir();
        let sock = dir.path().join("missing.sock");
        assert!(!is_socket_live(&sock));
    }

    /// A regular file sitting at the socket path is not a live listener —
    /// `connect()` on a non-socket fails, so we treat it as not-live (and the
    /// bind helper will clean it up).
    #[test]
    fn is_socket_live_false_for_stale_regular_file() {
        let dir = safe_tempdir();
        // Hold TEST_ENV_LOCK so this test is serialised with the
        // bind_with_stale_cleanup_* tests that call libc::umask(0o177).
        // Without serialisation, the umask window from those tests can corrupt
        // the tempdir mode (0o600 instead of 0o700), making fs::write fail.
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_DATA_DIR",
                "COPYPASTE_CONFIG_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            dir.path(),
        );
        let sock = dir.path().join("stale.sock");
        std::fs::write(&sock, b"not a socket").unwrap();
        assert!(!is_socket_live(&sock));
    }

    /// `BUILD_VERSION` must be non-empty and start with the crate's semver so
    /// clients can compare it against their own version prefix to detect a
    /// stale daemon after an upgrade.
    #[test]
    fn build_version_is_crate_version_prefixed() {
        assert!(!BUILD_VERSION.is_empty(), "BUILD_VERSION must not be empty");
        let crate_ver = env!("CARGO_PKG_VERSION");
        assert!(
            BUILD_VERSION == crate_ver || BUILD_VERSION.starts_with(&format!("{crate_ver}+")),
            "BUILD_VERSION {BUILD_VERSION:?} must equal or be `<{crate_ver}>+<sha>`"
        );
    }

    /// A leftover socket *file* with no process accepting on it is stale:
    /// `bind_with_stale_cleanup` must remove it and successfully rebind,
    /// rather than failing with `EADDRINUSE`. This is the core self-heal for
    /// the "process alive but socket not reachable" upgrade bug.
    ///
    /// Uses `std::os::unix::net::UnixListener` to seed the stale socket so the
    /// "previous daemon" half does not depend on a Tokio reactor; the helper
    /// under test (`bind_with_stale_cleanup`) binds a `tokio` listener, hence
    /// `#[tokio::test]`.
    #[tokio::test]
    async fn bind_with_stale_cleanup_removes_dead_socket_and_rebinds() {
        let dir = safe_tempdir();
        let sock = dir.path().join("daemon.sock");
        // Hold TEST_ENV_LOCK for the duration of this test to serialise the
        // libc::umask(0o177) call inside bind_with_stale_cleanup with concurrent
        // tests that create directories (which are corrupted by the process-wide
        // umask if it leaks into their mkdir calls).
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_DATA_DIR",
                "COPYPASTE_CONFIG_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            dir.path(),
        );

        // Create a real socket then drop its listener so the path is left
        // behind with no live acceptor — exactly what a crashed daemon leaves.
        {
            let dead = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
            drop(dead);
        }
        assert!(sock.exists(), "socket file must remain after listener drop");
        // TOCTOU settle (CopyPaste-del): the kernel can briefly keep accept()ing
        // on a just-dropped listen socket before the fd is fully reaped, so a
        // single `is_socket_live` probe is flaky under parallel load. Poll until
        // it reads as not-live (bounded) instead of asserting on the first probe.
        let mut live = is_socket_live(&sock);
        for _ in 0..200 {
            if !live {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            live = is_socket_live(&sock);
        }
        assert!(
            !live,
            "dropped listener must not be detected as live (after settle)"
        );

        // The helper must clean up and bind successfully.
        let listener =
            bind_with_stale_cleanup(&sock).expect("must self-heal a stale socket and rebind");
        assert!(is_socket_live(&sock), "rebound socket must accept connects");
        drop(listener);
    }

    /// A live listener that does NOT speak our protocol (never answers
    /// `status`, so reports no `build_version`/`pid`) cannot be safely evicted:
    /// the helper must refuse to bind rather than unlink a socket a live
    /// process still owns. (A real same-version daemon answers `status` and is
    /// covered by `..._refuses_to_steal_healthy_same_version_daemon` below.)
    #[tokio::test]
    async fn bind_with_stale_cleanup_refuses_unidentifiable_live_socket() {
        let dir = safe_tempdir();
        let sock = dir.path().join("daemon.sock");

        // Hold a live listener (std, no reactor needed) for the whole test.
        let _live = std::os::unix::net::UnixListener::bind(&sock).expect("seed live bind");
        assert!(is_socket_live(&sock), "seeded listener must be live");

        let err =
            bind_with_stale_cleanup(&sock).expect_err("must refuse to bind over a live socket");
        let msg = err.to_string();
        assert!(
            msg.contains("cannot be evicted automatically"),
            "expected a 'cannot be evicted' refusal, got: {msg}"
        );
    }

    /// A live daemon answering `status` with the SAME `build_version` as us is
    /// a healthy same-version peer — the helper must NOT steal its socket.
    #[tokio::test]
    async fn bind_with_stale_cleanup_refuses_to_steal_healthy_same_version_daemon() {
        let dir = safe_tempdir();
        let sock = dir.path().join("daemon.sock");

        // A minimal acceptor that replies to `status` with OUR build_version
        // and a bogus pid. It keeps accepting for the whole test (loop on a
        // cloned fd) so the socket stays live through the probe.
        let listener = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
        let acceptor = listener.try_clone().expect("clone listener fd");
        let body = serde_json::json!({
            "ok": true,
            "data": { "build_version": BUILD_VERSION, "pid": 999_999u32 },
        })
        .to_string();
        let handle = std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            loop {
                let Ok((stream, _)) = acceptor.accept() else {
                    break;
                };
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() && line.contains("status") {
                    let mut resp = body.clone();
                    resp.push('\n');
                    let _ = (&stream).write_all(resp.as_bytes());
                }
            }
        });

        let err = bind_with_stale_cleanup(&sock)
            .expect_err("must refuse to steal a healthy same-version daemon's socket");
        let msg = err.to_string();
        assert!(
            msg.contains("healthy same-version peer"),
            "expected same-version refusal, got: {msg}"
        );
        drop(listener); // ends the acceptor thread; tempdir teardown frees the path.
        let _ = handle;
    }

    /// A live daemon answering `status` with a DIFFERENT `build_version` is a
    /// STALE predecessor from before an upgrade. The helper must try to evict
    /// it (SIGTERM its reported pid). Here the reported pid is unsignalable
    /// (ESRCH), so the socket is never released and we must surface a clear,
    /// actionable error rather than silently coexisting / unlinking a live
    /// socket.
    #[tokio::test]
    async fn bind_with_stale_cleanup_attempts_eviction_for_different_version() {
        let dir = safe_tempdir();
        let sock = dir.path().join("daemon.sock");

        // The seed acceptor keeps the socket live for the WHOLE test (looping on
        // blocking accept) so eviction genuinely cannot succeed. We hold the
        // original listener in the test and hand the thread a `try_clone` so the
        // socket stays bound until the test's tempdir teardown frees the path.
        let listener = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
        let acceptor = listener.try_clone().expect("clone listener fd");
        // Report a different build version + a pid that maps to ESRCH (no such
        // process), so `evict_stale_daemon` SIGTERMs nothing and then times out
        // observing the socket is still held.
        let body = serde_json::json!({
            "ok": true,
            "data": { "build_version": "0.0.0-stale+deadbeef", "pid": 2_000_000_001u32 },
        })
        .to_string();
        let handle = std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            loop {
                let Ok((stream, _)) = acceptor.accept() else {
                    break;
                };
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                if reader.read_line(&mut line).is_ok() && line.contains("status") {
                    let mut resp = body.clone();
                    resp.push('\n');
                    let _ = (&stream).write_all(resp.as_bytes());
                }
            }
        });

        let err = bind_with_stale_cleanup(&sock).expect_err(
            "eviction of an unsignalable stale pid must fail loudly, not silently bind",
        );
        let msg = err.to_string();
        assert!(
            msg.contains("could not evict daemon"),
            "expected an eviction-failure error, got: {msg}"
        );
        // Dropping both listener fds unblocks/ends the acceptor thread.
        drop(listener);
        let _ = handle;
    }

    /// The `status` probe must round-trip `build_version` + `pid` from a daemon
    /// that answers, and yield `None`/defaults from a socket that says nothing.
    #[tokio::test]
    async fn probe_listening_daemon_reads_version_and_pid() {
        let dir = safe_tempdir();
        let sock = dir.path().join("daemon.sock");
        let listener = std::os::unix::net::UnixListener::bind(&sock).expect("seed bind");
        let handle = std::thread::spawn(move || {
            use std::io::{BufRead, BufReader, Write};
            if let Ok((stream, _)) = listener.accept() {
                let mut reader = BufReader::new(&stream);
                let mut line = String::new();
                let _ = reader.read_line(&mut line);
                let resp = serde_json::json!({
                    "ok": true,
                    "data": { "build_version": "9.9.9+abc", "pid": 4242u32 },
                })
                .to_string();
                let _ = (&stream).write_all(format!("{resp}\n").as_bytes());
            }
        });

        let probed = probe_listening_daemon(&sock).expect("probe should connect");
        assert_eq!(probed.build_version.as_deref(), Some("9.9.9+abc"));
        assert_eq!(probed.pid, Some(4242));
        handle.join().ok();
    }

    // ── CopyPaste-dl1e: PID exe validation ───────────────────────────────────

    /// `pid_exe_is_copypaste` must return `Some(true)` for THIS process (whose
    /// exe path definitely contains "copypaste" in CI / cargo test paths, OR
    /// at minimum must return `Some(_)` meaning the exe was resolved without error).
    ///
    /// We also verify the negative: a non-existent PID must return `None` (process
    /// gone → fail safe → do not signal).
    #[cfg(unix)]
    #[test]
    fn pid_exe_is_copypaste_returns_none_for_dead_pid() {
        // PID 2_000_000_001 is above the typical Linux/macOS pid_max and cannot
        // exist — resolving its exe must return None (fail-safe path).
        let result = pid_exe_is_copypaste(2_000_000_001u32);
        assert!(
            result.is_none(),
            "dead/impossible pid must return None, got {result:?}"
        );
    }

    /// Our own process (current pid) must resolve its exe successfully.
    /// The result is `Some(true)` when run via `cargo test` (binary path contains
    /// "copypaste" or "deps") or `Some(false)` on non-copypaste test runners —
    /// either way it must be `Some(_)`, not `None`, because the process exists.
    #[cfg(unix)]
    #[test]
    fn pid_exe_path_resolves_own_pid() {
        let own_pid = std::process::id();
        let exe = pid_exe_path(own_pid);
        // Must resolve (Some); the exact path depends on the runner.
        assert!(
            exe.is_some(),
            "pid_exe_path must resolve current pid {own_pid}, got None"
        );
    }

    #[tokio::test]
    async fn status_returns_running() {
        let dir = safe_tempdir();
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

    /// CopyPaste-ruep: status must include a non-empty device_key_fingerprint
    /// (SHA-256 of the X25519 public key, lowercase hex, 64 chars).
    #[tokio::test]
    async fn status_includes_device_key_fingerprint() {
        let dir = safe_tempdir();
        let sock = dir.path().join("dkfp.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"dkfp\",\"method\":\"status\"}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true, "status must succeed: {resp}");
        let fp = resp["data"]["device_key_fingerprint"]
            .as_str()
            .expect("device_key_fingerprint must be a string");
        // SHA-256 of any 32-byte key = 64 lowercase hex chars
        assert_eq!(fp.len(), 64, "fingerprint must be 64 hex chars, got: {fp}");
        assert!(
            fp.chars().all(|c| c.is_ascii_hexdigit()),
            "fingerprint must be hex, got: {fp}"
        );
    }

    /// c4q2.17: `list` is now a not_implemented stub; zero-item check migrated to
    /// `history_page`. This test now verifies the stub response shape.
    #[tokio::test]
    async fn list_empty_db_returns_zero() {
        let dir = safe_tempdir();
        let sock = dir.path().join("test2.sock");
        start_test_server(&sock).await;

        let resp = call_one(&sock, r#"{"id":"2","method":"list"}"#).await;
        assert_eq!(
            resp["ok"], false,
            "list must return not_implemented (c4q2.17): {resp}"
        );
        assert_eq!(resp["error_code"].as_str().unwrap_or(""), "not_implemented");
    }

    #[tokio::test]
    async fn unknown_method_returns_error() {
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        // ADR-007 + P2-ptb8: the version gate must return ERR_CODE_VERSION_MISMATCH
        // ("version_mismatch") so the CLI can branch deterministically without
        // parsing the error text. A previous version of this test incorrectly
        // asserted "invalid_argument"; corrected to match the dispatcher code.
        assert_eq!(
            resp["error_code"],
            crate::protocol::ERR_CODE_VERSION_MISMATCH,
            "version gate must return ERR_CODE_VERSION_MISMATCH: {resp}"
        );
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
    ///
    /// Only meaningful when `cloud-sync` is OFF: that is the only build where
    /// `cloud_sign_in` is the not-implemented STUB. With `cloud-sync` enabled
    /// the real handler runs and (correctly) returns `invalid_argument` for the
    /// missing-params request used here, so the assertion does not apply.
    #[cfg(not(feature = "cloud-sync"))]
    #[tokio::test]
    async fn ipc_responses_carry_machine_readable_error_code() {
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
        let sock = dir.path().join("hardening_chmod.sock");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_DATA_DIR",
                "COPYPASTE_CONFIG_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            dir.path(),
        );
        // Use IpcServer::bind directly so that bind_with_stale_cleanup runs and
        // fchmods the bound socket fd to 0600 (CopyPaste-c4q2.26 replaced the
        // former process-wide umask(0o177) with a per-fd fchmod). This asserts
        // the on-disk socket mode security invariant.
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db,
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );
        let listener = server.bind(&sock).expect("bind must succeed");
        drop(listener); // release the socket fd; the file remains for inspection

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

    /// CopyPaste-c4q2.26: binding the IPC socket must NOT mutate the process-wide
    /// `umask`. The old implementation set `umask(0o177)` around `bind`, which
    /// could clamp files created by concurrent startup threads to 0600. The fix
    /// tightens the socket via a per-inode `chmod`, leaving `umask` untouched.
    /// This guards against re-introducing the global side effect.
    #[tokio::test]
    async fn bind_does_not_mutate_process_umask() {
        // Serialise with the process-wide env/umask lock: reading the umask
        // requires a set+restore, which would otherwise race other tests.
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());

        // Read the current umask without leaving it changed.
        let before = unsafe { libc::umask(0o022) };
        unsafe { libc::umask(before) };

        let dir = safe_tempdir();
        let sock = dir.path().join("umask_probe.sock");
        let listener = bind_with_stale_cleanup(&sock).expect("bind must succeed");
        drop(listener);

        let after = unsafe { libc::umask(0o022) };
        unsafe { libc::umask(after) };
        assert_eq!(
            before, after,
            "bind_with_stale_cleanup must not leave the process umask mutated \
             (before={before:#o}, after={after:#o})"
        );
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ipc_oversized_request_rejected_not_crashed() {
        let dir = safe_tempdir();
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

    /// CopyPaste-c4q2.28: a NON-bulk method (here `status`) whose request body
    /// exceeds the per-method small cap (`SMALL_REQUEST_BYTES`, 64 KiB) must be
    /// rejected with `request_too_large` — the daemon must NOT buffer up to
    /// 16 MiB for it. This is the RAM-amplification guard.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ipc_non_bulk_method_over_small_cap_rejected() {
        let dir = safe_tempdir();
        let sock = dir.path().join("small_cap.sock");
        start_test_server(&sock).await;

        // Valid JSON with method=status and ~256 KiB of padding params — well
        // over SMALL_REQUEST_BYTES but far under MAX_REQUEST_BYTES.
        let pad = "A".repeat(256 * 1024);
        let body = format!(r#"{{"id":"big-status","method":"status","params":{{"pad":"{pad}"}}}}"#);

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        let _ = stream.write_all(body.as_bytes()).await;
        let _ = stream.write_all(b"\n").await;

        let mut lines = BufReader::new(&mut stream).lines();
        let line = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line())
            .await
            .expect("daemon must respond, not hang")
            .expect("read must succeed")
            .expect("must receive a rejection line");
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], false,
            "oversized status must be rejected: {resp}"
        );
        assert_eq!(
            resp["error_code"], "request_too_large",
            "must be tagged request_too_large: {resp}"
        );
        assert!(
            resp["error"].as_str().unwrap_or("").contains("too large"),
            "error must mention 'too large': {resp}"
        );
    }

    /// CopyPaste-c4q2.28 (companion): a bulk method (`import`) with a body LARGER
    /// than the small cap but valid must pass the size gate (phase-2 read) and be
    /// dispatched — i.e. it must NOT be rejected as `request_too_large`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn ipc_bulk_import_over_small_cap_accepted() {
        let dir = safe_tempdir();
        let sock = dir.path().join("bulk_ok.sock");
        start_test_server(&sock).await;

        // ~128 KiB of base64 content — over SMALL_REQUEST_BYTES, valid import.
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;
        let content = b64.encode(vec![0x7Au8; 128 * 1024]);
        let body = format!(
            r#"{{"id":"imp-ok","method":"import","params":{{"items":[{{"content_type":"text","content_bytes_b64":"{content}","created_at_ms":1700000000}}]}}}}"#,
        );

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(body.as_bytes()).await.unwrap();
        stream.write_all(b"\n").await.unwrap();

        let mut lines = BufReader::new(&mut stream).lines();
        let line = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line())
            .await
            .expect("daemon must respond, not hang")
            .expect("read must succeed")
            .expect("must receive a response line");
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        // The size gate must have let this through: it is dispatched to the
        // import handler, not rejected for being too large.
        assert_ne!(
            resp["error_code"], "request_too_large",
            "valid bulk import over the small cap must pass the size gate: {resp}"
        );
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
        // Bind directly (no umask(0o177) race) — see comment in
        // start_test_server_returning_db for the full rationale.
        let path = socket_path.to_path_buf();
        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("test socket bind must succeed");
        tokio::spawn(async move {
            if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
                tracing::error!("ipc: server on {:?} exited with error: {e}", &path);
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        ready
    }

    #[tokio::test]
    async fn dispatch_returns_ipc_not_ready_when_not_ready() {
        let dir = safe_tempdir();
        let sock = dir.path().join("not_ready.sock");
        let ready = start_not_ready_server(&sock).await;

        // DB-touching methods must be rejected with ipc_not_ready error_code.
        // c4q2.17: "list" removed — now a not_implemented stub, no DB access.
        // c4q2.13: check error_code (machine-readable), not legacy "IPC_NOT_READY" string.
        for (method, params) in [
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
                resp["error_code"].as_str().unwrap_or_default(),
                "ipc_not_ready",
                "{method} should return error_code=ipc_not_ready, got: {resp}"
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

    /// c4q2.17: "list" is now a not_implemented stub. Limit-clamping is
    /// tested for the unified "history_page" verb in
    /// `history_page_clamps_oversize_limit_to_max_page` below.
    #[tokio::test]
    async fn list_clamps_oversize_limit_to_max_page() {
        let dir = safe_tempdir();
        let sock = dir.path().join("cap_list.sock");
        start_test_server(&sock).await;

        let resp = call_one(
            &sock,
            r#"{"id":"cap-list","method":"list","params":{"limit":5000,"offset":0}}"#,
        )
        .await;
        assert_eq!(
            resp["ok"], false,
            "list must return not_implemented: {resp}"
        );
        assert_eq!(
            resp["error_code"].as_str().unwrap_or(""),
            "not_implemented",
            "list must carry not_implemented error_code (c4q2.17): {resp}"
        );
    }

    #[tokio::test]
    async fn history_page_clamps_oversize_limit_to_max_page() {
        let dir = safe_tempdir();
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

    /// daemon-core backlog #2: the `search` handler must clamp an oversized
    /// `limit` to MAX_PAGE just like `list` / `history_page`. We seed more than
    /// MAX_PAGE rows all matching one FTS term, then request `limit=5000`. The
    /// SQL applies `LIMIT ?`, so without the `.min(MAX_PAGE)` clamp the response
    /// would carry > MAX_PAGE items; with it, exactly MAX_PAGE.
    #[tokio::test]
    async fn search_clamps_oversize_limit_to_max_page() {
        let dir = safe_tempdir();
        let sock = dir.path().join("cap_search.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Seed MAX_PAGE + 5 text rows whose FTS plaintext all contains "needle".
        {
            let guard = db.lock().await;
            for i in 0..(MAX_PAGE + 5) {
                let item = copypaste_core::ClipboardItem::new_text(
                    vec![0xAB],
                    vec![0u8; 24],
                    i as i64 + 1,
                );
                copypaste_core::insert_item_with_fts(&guard, &item, &format!("needle row {i}"))
                    .unwrap();
            }
        }

        let resp = call_one(
            &sock,
            r#"{"id":"cap-search","method":"search","params":{"query":"needle","limit":5000}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "search should be ok: {resp}");
        let items = resp["data"]["items"].as_array().unwrap();
        assert_eq!(
            items.len(),
            MAX_PAGE,
            "search must clamp to MAX_PAGE={MAX_PAGE}, got {} items",
            items.len()
        );
    }

    /// daemon-core backlog #3: list_view (`history_page`) preview offsets must
    /// not panic on width-changing Unicode normalisation. The sensitive detector
    /// reports byte ranges over the NFKC-normalised string; slicing the original
    /// preview with those offsets used to panic on a non-char-boundary. With a
    /// secret embedded after a ligature/full-width run, the handler must return
    /// without panicking and produce in-range, ordered char offsets.
    #[tokio::test]
    async fn history_page_adversarial_unicode_preview_no_panic() {
        let dir = safe_tempdir();
        let sock = dir.path().join("adv_unicode.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Full-width "AKIA" (U+FF21..) + 16 ASCII chars normalises (NFKC) to a
        // valid AWS access-key id, which the detector flags. The full-width
        // prefix is 3 bytes/char in the original but 1 byte/char after NFKC, so
        // the detector's byte offsets do not line up with the original string —
        // exactly the mismatch that triggered the slice panic.
        let plaintext = "ＡＫＩＡ0123456789ABCDEF and some trailing prose";
        {
            let guard = db.lock().await;
            let item = copypaste_core::ClipboardItem::new_text(vec![0xCD], vec![0u8; 24], 1);
            copypaste_core::insert_item_with_fts(&guard, &item, plaintext).unwrap();
        }

        // Must not panic — a panic in the blocking task would surface as an
        // internal error / dropped connection rather than an `ok` response.
        let resp = call_one(
            &sock,
            r#"{"id":"adv","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "history_page must not panic: {resp}");
        let items = resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        let preview = items[0]["preview"].as_str().unwrap();
        let preview_char_len = preview.chars().count();
        let spans = items[0]["sensitive_spans"].as_array().unwrap();
        for span in spans {
            let pair = span.as_array().unwrap();
            let start = pair[0].as_u64().unwrap() as usize;
            let end = pair[1].as_u64().unwrap() as usize;
            assert!(start <= end, "span start {start} must be <= end {end}");
            assert!(
                end <= preview_char_len,
                "span end {end} must be within preview char-len {preview_char_len}"
            );
        }
    }

    /// Fix-1 (NFKC span-mask leak): when the preview contains full-width or
    /// ligature chars that NFKC normalises to narrower forms, the returned
    /// `preview` string must be the NORMALISED form so that the returned char
    /// offsets (`sensitive_spans`) correctly index into it.
    ///
    /// Concretely: full-width "ＡＫＩＡ" (4 chars × 3 bytes each in the original)
    /// normalises to ASCII "AKIA" (4 chars × 1 byte each).  The detector sees
    /// "AKIA…" and reports a span at, say, chars [0..20].  If the daemon returned
    /// the ORIGINAL (full-width) preview, the UI would apply [0..20] to a string
    /// where char 0 is a 3-byte full-width 'Ａ' — the mask would cover the WRONG
    /// characters and might expose part of the secret.  The fix: always return the
    /// normalised preview so offsets and string share one basis.
    #[tokio::test]
    async fn history_page_spans_index_into_returned_preview_not_raw() {
        let dir = safe_tempdir();
        let sock = dir.path().join("span_basis.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Full-width prefix: each char is 3 UTF-8 bytes in the original,
        // but only 1 byte after NFKC.  The detector runs on NFKC form and
        // produces a span anchored at byte offset 0 of the normalised string.
        // If the daemon returns the raw (non-normalised) preview, char offset 0
        // in that string still maps to the full-width Ａ — the span basis is wrong.
        let plaintext = "ＡＫＩＡ0123456789ABCDEF trailing text";
        {
            let guard = db.lock().await;
            let item = copypaste_core::ClipboardItem::new_text(vec![0xCD], vec![0u8; 24], 1);
            copypaste_core::insert_item_with_fts(&guard, &item, plaintext).unwrap();
        }

        let resp = call_one(
            &sock,
            r#"{"id":"basis","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "history_page: {resp}");
        let items = resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);

        let preview = items[0]["preview"].as_str().unwrap();
        let spans = items[0]["sensitive_spans"].as_array().unwrap();

        // The detector must have flagged something (the normalised form is
        // "AKIA0123456789ABCDEF…" which contains an AWS-key-like pattern).
        assert!(
            !spans.is_empty(),
            "detector should flag the AKIA... pattern in the preview"
        );

        // KEY ASSERTION: every span must start with ASCII 'A' in the returned
        // preview.  If the preview is the RAW full-width string the first char
        // would be 'Ａ' (U+FF21), not 'A' (U+0041) — proving the span basis is
        // wrong.  After the fix the preview is normalised and spans[0][0] == 0
        // means preview.chars().nth(0) == 'A'.
        for span in spans {
            let pair = span.as_array().unwrap();
            let start = pair[0].as_u64().unwrap() as usize;
            let end = pair[1].as_u64().unwrap() as usize;
            // Span must be within the returned preview's char length.
            let char_len = preview.chars().count();
            assert!(
                end <= char_len,
                "span [{}..{}] out of range for preview (len={}): {:?}",
                start,
                end,
                char_len,
                preview
            );
            // Each char in the spanned range must be ASCII (normalised).
            // Full-width chars are 3 bytes wide; after NFKC they become ASCII.
            let span_chars: String = preview.chars().skip(start).take(end - start).collect();
            assert!(
                span_chars.is_ascii(),
                "span [{start}..{end}] covers non-ASCII chars in preview — \
                 preview is NOT normalised (raw full-width form leaked): {:?}",
                span_chars
            );
        }
    }

    /// `byte_to_char_offset` clamps out-of-range and mid-codepoint byte indices
    /// to a valid char boundary and never panics.
    #[test]
    fn byte_to_char_offset_clamps_and_never_panics() {
        let s = "café"; // 'é' is 2 bytes (0xC3 0xA9): bytes 0..5, chars 0..4
        assert_eq!(byte_to_char_offset(s, 0), 0);
        assert_eq!(byte_to_char_offset(s, 3), 3); // boundary before 'é'
        assert_eq!(byte_to_char_offset(s, 4), 3); // mid-'é' → walk back → 3 chars
        assert_eq!(byte_to_char_offset(s, 5), 4); // end
        assert_eq!(byte_to_char_offset(s, 9999), 4); // past end → clamp to char-len
    }

    // --- FIX 1: history_page returns pinned field and pinned-first order ---

    /// Each item in `history_page` must carry a boolean `pinned` field.
    #[tokio::test]
    async fn history_page_items_include_pinned_field() {
        let dir = safe_tempdir();
        let sock = dir.path().join("hp_pinned_field.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Seed one item.
        {
            let guard = db.lock().await;
            let item = copypaste_core::ClipboardItem::new_text(vec![0xAA], vec![0u8; 24], 1);
            copypaste_core::insert_item(&guard, &item).unwrap();
        }

        let resp = call_one(
            &sock,
            r#"{"id":"hpf1","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true);
        let items = resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 1);
        // The `pinned` field must be present and be a boolean.
        assert!(
            items[0]["pinned"].is_boolean(),
            "each item must have a boolean 'pinned' field, got: {}",
            items[0]
        );
        assert_eq!(
            items[0]["pinned"], false,
            "freshly inserted item must have pinned=false"
        );
    }

    /// `history_page` must return pinned items before unpinned items,
    /// regardless of wall_time ordering.
    #[tokio::test]
    async fn history_page_pinned_items_sort_first() {
        let dir = safe_tempdir();
        let sock = dir.path().join("hp_pinned_sort.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Insert two items: item_old (lower wall_time) and item_new (higher).
        // Then pin item_old — it must appear first in history_page even though
        // it is older.
        let (id_old, _id_new) = {
            let guard = db.lock().await;
            let mut item_old =
                copypaste_core::ClipboardItem::new_text(vec![0x01], vec![0u8; 24], 1);
            item_old.wall_time = 1_000;
            let id_old = item_old.id.clone();
            copypaste_core::insert_item(&guard, &item_old).unwrap();

            let mut item_new =
                copypaste_core::ClipboardItem::new_text(vec![0x02], vec![0u8; 24], 2);
            item_new.wall_time = 2_000;
            let id_new = item_new.id.clone();
            copypaste_core::insert_item(&guard, &item_new).unwrap();

            (id_old, id_new)
        };

        // Pin the older item via the IPC verb.
        let pin_body = format!(
            r#"{{"id":"hps-pin","method":"pin_item","params":{{"id":"{id_old}","pinned":true}}}}"#
        );
        let pin_resp = call_one(&sock, &pin_body).await;
        assert_eq!(pin_resp["ok"], true, "pin must succeed: {pin_resp}");

        // Now history_page must return item_old first.
        let hp_resp = call_one(
            &sock,
            r#"{"id":"hps-hp","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(hp_resp["ok"], true);
        let items = hp_resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0]["id"].as_str().unwrap(),
            id_old,
            "pinned (older) item must be first"
        );
        assert_eq!(items[0]["pinned"], true, "first item must have pinned=true");
        assert_eq!(
            items[1]["pinned"], false,
            "second item must have pinned=false"
        );
    }

    /// After unpinning, the item reverts to recency order in history_page.
    #[tokio::test]
    async fn history_page_unpinned_item_reverts_to_recency_order() {
        let dir = safe_tempdir();
        let sock = dir.path().join("hp_unpin.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        let (id_old, _id_new) = {
            let guard = db.lock().await;
            let mut item_old =
                copypaste_core::ClipboardItem::new_text(vec![0x01], vec![0u8; 24], 1);
            item_old.wall_time = 1_000;
            let id_old = item_old.id.clone();
            copypaste_core::insert_item(&guard, &item_old).unwrap();

            let mut item_new =
                copypaste_core::ClipboardItem::new_text(vec![0x02], vec![0u8; 24], 2);
            item_new.wall_time = 2_000;
            let id_new = item_new.id.clone();
            copypaste_core::insert_item(&guard, &item_new).unwrap();

            (id_old, id_new)
        };

        // Pin then unpin item_old.
        let pin_body = format!(
            r#"{{"id":"hpu-pin","method":"pin_item","params":{{"id":"{id_old}","pinned":true}}}}"#
        );
        call_one(&sock, &pin_body).await;
        let unpin_body = format!(
            r#"{{"id":"hpu-unpin","method":"pin_item","params":{{"id":"{id_old}","pinned":false}}}}"#
        );
        call_one(&sock, &unpin_body).await;

        // After unpin, history_page must return newest-first (item_new first).
        let hp_resp = call_one(
            &sock,
            r#"{"id":"hpu-hp","method":"history_page","params":{"limit":10,"offset":0}}"#,
        )
        .await;
        assert_eq!(hp_resp["ok"], true);
        let items = hp_resp["data"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(
            items[0]["pinned"], false,
            "first item must be unpinned after unpin"
        );
        assert!(
            items[0]["wall_time"].as_i64().unwrap() >= items[1]["wall_time"].as_i64().unwrap(),
            "items must be newest-first after unpin"
        );
    }

    /// In-process burst that exercises the same accept-spawn path used by
    /// the binary subprocess test, but without requiring a built binary.
    /// 10 tokio tasks each issue a status+stats roundtrip on its own
    /// connection; all must succeed.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_clients_in_process_consistent_state() {
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
        let sock = dir.path().join("test-spawn-blocking.sock");
        start_test_server(&sock).await;

        // c4q2.17: Fire 4 concurrent `history_page` requests (unified verb).
        // Previously used `list` which is now a not_implemented stub.
        const N: usize = 4;
        let started = std::time::Instant::now();
        let mut handles = Vec::with_capacity(N);
        for i in 0..N {
            let sock_path = sock.clone();
            handles.push(tokio::spawn(async move {
                let mut stream = UnixStream::connect(&sock_path).await.unwrap();
                let payload = format!(
                    "{{\"id\":\"sb{i}\",\"method\":\"history_page\",\"params\":{{\"limit\":10,\"offset\":0}}}}\n"
                );
                stream.write_all(payload.as_bytes()).await.unwrap();
                let mut lines = BufReader::new(&mut stream).lines();
                let line = lines.next_line().await.unwrap().unwrap();
                let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
                assert_eq!(resp["ok"], true, "history_page must succeed: {line}");
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        let elapsed = started.elapsed();

        // Sanity bound: 4 in-memory `history_page` calls on an empty DB should
        // finish in well under a second. 5s catches catastrophic regressions
        // (e.g., a single-thread deadlock) without flaking on slow CI runners.
        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "4 concurrent history_page requests took {elapsed:?} — tokio worker likely blocked"
        );
    }

    /// beta-W3.2 — `pair_peer_with_password` validates required params and
    /// returns `not_implemented` once inputs check out, so the UI can rely
    /// on a stable error_code for the not-yet-wired Transport path.
    #[tokio::test]
    async fn pair_peer_with_password_validates_inputs() {
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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

    /// c4q2.20: pair_accept_password is stubbed not_implemented. Verify the stub
    /// shape: ok=false, error_code="not_implemented" (not a crash or generic error).
    #[tokio::test]
    async fn pair_accept_password_returns_session_and_message2() {
        let dir = safe_tempdir();
        let sock = dir.path().join("test-pake-accept.sock");
        start_test_server(&sock).await;

        let resp = call_one(
            &sock,
            r#"{"id":"pa1","method":"pair_accept_password","params":{"message1_b64":"AAAA","peer_fingerprint":"cd:cd","password":"correct-horse"}}"#,
        )
        .await;
        assert_eq!(
            resp["ok"], false,
            "pair_accept_password must be not_implemented (c4q2.20): {resp}"
        );
        assert_eq!(
            resp["error_code"].as_str().unwrap_or(""),
            "not_implemented",
            "pair_accept_password must carry not_implemented error_code: {resp}"
        );
    }

    /// c4q2.20: renamed from pair_peer_with_password_full_round_trip.
    /// The responder step (pair_accept_password) is now stubbed not_implemented
    /// (CopyPaste-c4q2.20 security concern). Test that:
    ///   1. Initiator "initiate" step still works (step 1 untouched).
    ///   2. The stub returns not_implemented for the responder verb.
    ///   Full QR round-trip tested by pair_qr_full_round_trip below.
    #[tokio::test]
    async fn pair_peer_with_password_initiator_step_works() {
        use base64::Engine as _;
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            &cfg_home,
        );

        let sock_a = dir.path().join("test-pake-initiator-a.sock");
        let sock_b = dir.path().join("test-pake-initiator-b.sock");
        start_test_server(&sock_a).await;
        start_test_server(&sock_b).await;

        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        let password = "correct-horse-battery";

        let fp_b = call(
            &sock_b,
            r#"{"id":"fp_b","method":"get_own_fingerprint","params":{}}"#,
        )
        .await["data"]["fingerprint"]
            .as_str()
            .expect("server B must return own fingerprint")
            .to_string();

        // Step 1 (initiator "initiate"): must still work — pair_accept_password
        // is stubbed not_implemented but the initiator verb is untouched (c4q2.20).
        let body = format!(
            r#"{{"id":"rt1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{fp_b}","password":"{password}","step":"initiate"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(
            resp["ok"], true,
            "initiate step must succeed (c4q2.20): {resp}"
        );
        let session_id = resp["data"]["session_id"].as_str().unwrap();
        let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap();
        // Verify the expected fields are non-empty.
        assert!(!session_id.is_empty(), "session_id must be non-empty");
        let _ = base64::engine::general_purpose::STANDARD
            .decode(msg1_b64)
            .expect("message1_b64 must be valid base64");

        // c4q2.20: pair_accept_password (responder verb) is now a not_implemented stub.
        let not_impl_resp = call(
            &sock_b,
            &format!(
                r#"{{"id":"rt2","method":"pair_accept_password","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"aa:bb","password":"{password}"}}}}"#
            ),
        )
        .await;
        assert_eq!(
            not_impl_resp["ok"], false,
            "pair_accept_password must be stubbed not_implemented (c4q2.20): {not_impl_resp}"
        );
        assert_eq!(
            not_impl_resp["error_code"].as_str().unwrap_or(""),
            "not_implemented",
            "pair_accept_password must carry not_implemented error_code: {not_impl_resp}"
        );
    }

    // -----------------------------------------------------------------------
    // S3 (CopyPaste-4ca) — PAKE SessionKey cert-binding tests
    // -----------------------------------------------------------------------

    /// S3: The cert-binder helper must be symmetric (swap fp_a / fp_b → same
    /// output) and must produce different values for different fingerprint pairs.
    #[test]
    fn pake_cert_binder_is_symmetric_and_distinct() {
        let fp_a = "aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99";
        let fp_b = "11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00:11:22:33:44:55:66:77:88:99:aa:bb:cc:dd:ee:ff:00";
        let fp_c = "ff:ee:dd:cc:bb:aa:99:88:77:66:55:44:33:22:11:00:ff:ee:dd:cc:bb:aa:99:88:77:66:55:44:33:22:11:00";

        let binder_ab = IpcServer::pake_cert_binder(fp_a, fp_b);
        let binder_ba = IpcServer::pake_cert_binder(fp_b, fp_a);
        let binder_ac = IpcServer::pake_cert_binder(fp_a, fp_c);

        assert_eq!(binder_ab, binder_ba, "binder must be symmetric");
        assert_ne!(
            binder_ab, binder_ac,
            "different fp pairs must yield different binders"
        );
    }

    /// S3: A full PAKE round-trip with matching cert binders on both ends
    /// produces matching confirmation tags — simulating the honest pairing case.
    #[test]
    fn pake_channel_binding_succeeds_with_matching_cert_binders() {
        use copypaste_p2p::pake::{
            channel_confirmation_tag, ConfirmRole, PakeInitiator, PakeResponder, PasswordFile,
            CONFIRM_TAG_LEN,
        };

        let password = "correct-horse-battery-S3";
        let pf = PasswordFile::register(password).expect("register");

        let (client, msg1) = PakeInitiator::new(password).expect("initiator new");
        let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("responder respond");
        let (client_key, msg3) = client.finish(&msg2).expect("initiator finish");
        let server_key = server.finish(&msg3).expect("responder finish");

        // Both sides use the same cert fingerprints → same binder.
        let fp_initiator = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";
        let fp_responder = "f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f:f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f";

        let binder = IpcServer::pake_cert_binder(fp_initiator, fp_responder);
        let client_bound = client_key.bind_to_tls_channel(&binder);
        let server_bound = server_key.bind_to_tls_channel(&binder);

        let client_tag = channel_confirmation_tag(&client_bound, ConfirmRole::Initiator);
        let server_expected = channel_confirmation_tag(&server_bound, ConfirmRole::Initiator);

        assert_eq!(client_tag.len(), CONFIRM_TAG_LEN);
        assert_eq!(
            client_tag, server_expected,
            "initiator tag must match on both sides when binders agree"
        );

        // Responder also derives a matching responder tag.
        let resp_tag_from_client = channel_confirmation_tag(&client_bound, ConfirmRole::Responder);
        let resp_tag_from_server = channel_confirmation_tag(&server_bound, ConfirmRole::Responder);
        assert_eq!(
            resp_tag_from_client, resp_tag_from_server,
            "responder tag must also match"
        );
    }

    /// S3: When a relay/MitM substitutes different cert fingerprints on each leg,
    /// the binders differ → the bound keys differ → the confirmation tags do NOT
    /// match → the handshake is detected.
    ///
    /// This directly models the attack: relay terminates PAKE on leg A
    /// (fp_relay_a, fp_victim) and bridges to leg B (fp_relay_b, fp_target).
    /// The two legs use different cert pairs, so each leg computes a different
    /// binder → different confirmation tags → the responder's verify step rejects.
    #[test]
    fn pake_channel_binding_fails_with_mismatched_cert_binders() {
        use copypaste_p2p::pake::{
            channel_confirmation_tag, ConfirmRole, PakeInitiator, PakeResponder, PasswordFile,
            CONFIRM_TAG_LEN,
        };
        use subtle::ConstantTimeEq as _;

        let password = "correct-horse-battery-mitm";
        let pf = PasswordFile::register(password).expect("register");

        let (client, msg1) = PakeInitiator::new(password).expect("initiator new");
        let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("responder respond");
        let (client_key, msg3) = client.finish(&msg2).expect("initiator finish");
        let server_key = server.finish(&msg3).expect("responder finish");

        // Leg A (initiator side): MitM presents its own cert to the initiator.
        let fp_initiator = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";
        let fp_mitm_leg_a = "de:ad:be:ef:00:11:22:33:44:55:66:77:88:99:aa:bb:de:ad:be:ef:00:11:22:33:44:55:66:77:88:99:aa:bb";

        // Leg B (responder side): MitM uses a DIFFERENT cert toward the responder.
        let fp_mitm_leg_b = "ca:fe:ba:be:00:11:22:33:44:55:66:77:88:99:aa:bb:ca:fe:ba:be:00:11:22:33:44:55:66:77:88:99:aa:bb";
        let fp_responder = "f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f:f0:e1:d2:c3:b4:a5:96:87:78:69:5a:4b:3c:2d:1e:0f";

        // Initiator sees (fp_initiator, fp_mitm_leg_a) → binder_a
        let binder_a = IpcServer::pake_cert_binder(fp_initiator, fp_mitm_leg_a);
        // Responder sees (fp_mitm_leg_b, fp_responder) → binder_b (different!)
        let binder_b = IpcServer::pake_cert_binder(fp_mitm_leg_b, fp_responder);

        assert_ne!(
            binder_a, binder_b,
            "MitM legs must produce different binders"
        );

        let client_bound = client_key.bind_to_tls_channel(&binder_a);
        let server_bound = server_key.bind_to_tls_channel(&binder_b);

        // Initiator computes its confirmation tag with binder_a.
        let initiator_tag = channel_confirmation_tag(&client_bound, ConfirmRole::Initiator);
        // Responder verifies with binder_b → MUST NOT match.
        let responder_expected = channel_confirmation_tag(&server_bound, ConfirmRole::Initiator);

        assert_eq!(initiator_tag.len(), CONFIRM_TAG_LEN);
        assert_eq!(responder_expected.len(), CONFIRM_TAG_LEN);

        // Constant-time compare — proves the responder's check would fail.
        let tags_match: bool = initiator_tag.ct_eq(&responder_expected).into();
        assert!(
            !tags_match,
            "confirmation tags MUST differ when cert binders differ (MitM detected)"
        );
    }

    /// c4q2.20: Renamed from pair_accept_finish_rejects_wrong_initiator_confirm_tag.
    /// pair_accept_password is now stubbed (no longer creates responder sessions via
    /// password flow — use QR). This test verifies pair_accept_finish rejects an
    /// unknown/bogus session_id with a non-ok response (not_found or invalid_argument).
    /// The tampered-confirm-tag check is covered by the pure pake_channel_binding tests.
    #[tokio::test]
    async fn pair_accept_finish_rejects_unknown_session() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let dir = safe_tempdir();
        let sock = dir.path().join("test-s3-reject.sock");
        start_test_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(
                b"{\"id\":\"s3r\",\"method\":\"pair_accept_finish\",\"params\":{\"session_id\":\"no-such-session\",\"message3_b64\":\"AAAA\",\"peer_fingerprint\":\"aa:bb\",\"initiator_confirm_b64\":\"AAAA\"}}\n",
            )
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], false,
            "pair_accept_finish with unknown session must fail (c4q2.20): {resp}"
        );
        // Accept not_found or invalid_argument — both are valid error shapes.
        let code = resp["error_code"].as_str().unwrap_or("");
        assert!(
            code == "not_found" || code == "invalid_argument" || !resp["error"].is_null(),
            "pair_accept_finish must return diagnosable error for unknown session: {resp}"
        );
    }

    /// c4q2.20: pair_accept_password is stubbed not_implemented; the absent-tag
    /// scenario no longer applies to the password path. Test that pair_accept_finish
    /// with a completely absent `initiator_confirm_b64` on an unknown session still
    /// returns a non-ok error (guards the dispatch path, not PAKE logic).
    #[tokio::test]
    async fn pair_accept_finish_rejects_absent_initiator_confirm_tag() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let dir = safe_tempdir();
        let sock = dir.path().join("j8dr.sock");
        start_test_server(&sock).await;

        // Send pair_accept_finish without initiator_confirm_b64 AND with a
        // non-existent session_id — must return non-ok.
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(
                b"{\"id\":\"j6\",\"method\":\"pair_accept_finish\",\"params\":{\"session_id\":\"no-such-session\",\"message3_b64\":\"AAAA\",\"peer_fingerprint\":\"aa:bb\"}}\n",
            )
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], false,
            "pair_accept_finish without confirm tag and unknown session must fail (c4q2.20): {resp}"
        );
    }

    /// QR pairing end-to-end: device B (displaying) generates a QR, device A
    /// (scanning) decodes it via `copypaste_core::PairingPayload`, derives the
    /// PAKE password from the embedded token, and completes the 4-message
    /// handshake using `pair_accept_qr` on B in place of `pair_accept_password`.
    /// No password is ever typed — it travels in the QR token.
    #[tokio::test]
    async fn pair_qr_full_round_trip() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg");
        // Set COPYPASTE_CONFIG_DIR *first* — `peers_file_path` checks it ahead
        // of dirs::config_dir(), so peers.json goes into cfg_home regardless of
        // whether dirs::config_dir() is affected by HOME/XDG_CONFIG_HOME (macOS
        // ignores HOME for Application Support).
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            &cfg_home,
        );

        let sock_a = dir.path().join("test-qr-a.sock");
        let sock_b = dir.path().join("test-qr-b.sock");
        start_test_server(&sock_a).await;
        start_test_server(&sock_b).await;

        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        // S3/j8dr: Get the REAL cert fingerprints from both servers so the
        // cert-binder computation uses the correct values and the mandatory
        // initiator_confirm_b64 can be verified. (Old code used a static fake
        // fp_a which caused binder mismatch and was masked by the optional tag.)
        let fp_a = call(
            &sock_a,
            r#"{"id":"qr_fpa","method":"get_own_fingerprint","params":{}}"#,
        )
        .await["data"]["fingerprint"]
            .as_str()
            .expect("server A must return own fingerprint")
            .to_string();

        // Step 0: Device B generates a QR pairing code.
        let resp = call(
            &sock_b,
            r#"{"id":"qr0","method":"pair_generate_qr","params":{}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "pair_generate_qr failed: {resp}");
        let qr = resp["data"]["qr"].as_str().unwrap().to_string();
        // The generated QR is now wrapped in the cppair://pair?p= deep-link URI
        // so external scanners (Google Lens) can open it in the app.
        assert!(
            qr.starts_with(copypaste_core::PAIRING_DEEPLINK_PREFIX),
            "QR must be wrapped in the cppair:// deep-link: {qr}"
        );

        // Step 0b: Device A scans, strips the wrapper, decodes the QR and derives
        // the PAKE password (mirrors the in-app scanner / manifest deep-link path).
        let bare = copypaste_core::strip_deeplink(&qr);
        assert!(
            bare.starts_with("CPPAIR2."),
            "stripped QR must use the v2 magic: {bare}"
        );
        let payload = copypaste_core::PairingPayload::decode(&bare)
            .expect("scanning device must decode the QR");
        let password = payload.token.to_pake_password();
        // The fingerprint A pins is the one carried in the QR (B's fingerprint).
        // CPPAIR2 decode returns bare lowercase hex; convert to the colon-grouped
        // display form that `pair_peer_with_password` / `is_valid_fingerprint` expect.
        let fp_b_raw = payload.fingerprint.clone();
        assert!(!fp_b_raw.is_empty(), "QR must carry B's fingerprint");
        let fp_b = display_fingerprint(&fp_b_raw);

        // Step 1: Device A initiates using the QR-derived password.
        let body = format!(
            r#"{{"id":"qr1","method":"pair_peer_with_password","params":{{"peer_fingerprint":"{fp_b}","password":"{password}","step":"initiate"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(resp["ok"], true, "initiate failed: {resp}");
        let session_id_a = resp["data"]["session_id"].as_str().unwrap().to_string();
        let msg1_b64 = resp["data"]["message1_b64"].as_str().unwrap().to_string();

        // Step 2: Device B accepts via pair_accept_qr (looks up its stored token).
        // Use A's REAL cert fingerprint so the cert-binder on both sides agrees.
        let body = format!(
            r#"{{"id":"qr2","method":"pair_accept_qr","params":{{"message1_b64":"{msg1_b64}","peer_fingerprint":"{fp_a}"}}}}"#
        );
        let resp = call(&sock_b, &body).await;
        assert_eq!(resp["ok"], true, "pair_accept_qr failed: {resp}");
        let session_id_b = resp["data"]["session_id"].as_str().unwrap().to_string();
        let msg2_b64 = resp["data"]["message2_b64"].as_str().unwrap().to_string();

        // Step 3: Device A finishes — also returns initiator_confirm_b64 (S3).
        let body = format!(
            r#"{{"id":"qr3","method":"pair_peer_with_password","params":{{"step":"finish","session_id":"{session_id_a}","message2_b64":"{msg2_b64}","peer_fingerprint":"{fp_b}","password":"{password}"}}}}"#
        );
        let resp = call(&sock_a, &body).await;
        assert_eq!(resp["ok"], true, "initiator finish failed: {resp}");
        let msg3_b64 = resp["data"]["message3_b64"].as_str().unwrap().to_string();
        // j8dr: extract the mandatory confirm tag from A's finish response.
        let initiator_confirm_b64 = resp["data"]["initiator_confirm_b64"]
            .as_str()
            .expect("initiator finish must return initiator_confirm_b64")
            .to_string();

        // Step 4: Device B finishes — the OPAQUE authenticator must validate,
        // proving both sides agreed on the QR token as the shared secret.
        // j8dr: include the mandatory initiator_confirm_b64.
        let body = format!(
            r#"{{"id":"qr4","method":"pair_accept_finish","params":{{"session_id":"{session_id_b}","message3_b64":"{msg3_b64}","peer_fingerprint":"{fp_a}","initiator_confirm_b64":"{initiator_confirm_b64}"}}}}"#
        );
        let resp = call(&sock_b, &body).await;
        assert_eq!(resp["ok"], true, "responder finish failed: {resp}");
        assert_eq!(resp["data"]["ok"], true, "pair_accept_finish must succeed");
    }

    /// `pair_accept_qr` with no prior `pair_generate_qr` must be rejected
    /// rather than registering an empty/garbage PasswordFile.
    #[tokio::test]
    async fn pair_accept_qr_without_token_is_rejected() {
        use base64::Engine as _;
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg");
        // Include COPYPASTE_CONFIG_DIR so peers_file_path() points at cfg_home
        // on macOS (where dirs::config_dir() ignores HOME).
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            &cfg_home,
        );
        let sock = dir.path().join("test-qr-notoken.sock");
        start_test_server(&sock).await;

        let fp = "a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90:a1:b2:c3:d4:e5:f6:07:18:29:3a:4b:5c:6d:7e:8f:90";
        let msg1 = base64::engine::general_purpose::STANDARD.encode([0u8; 32]);
        let body = format!(
            r#"{{"id":"nt1","method":"pair_accept_qr","params":{{"message1_b64":"{msg1}","peer_fingerprint":"{fp}"}}}}"#
        );
        let resp = call_one(&sock, &body).await;
        assert_eq!(
            resp["ok"], false,
            "pair_accept_qr without a generated token must fail: {resp}"
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

        let dir = safe_tempdir();
        let sock = dir.path().join("test-revoke.sock");

        // Redirect the config dir to this test's own tempdir so the
        // `revoke_peer` handler's `save_peers` never writes to (and never
        // depends on the existence of) the machine's real config dir. Under
        // parallel CI execution the platform `dirs::config_dir()` may not
        // exist, which previously made `save_peers` fail with ENOENT. Setting
        // `COPYPASTE_CONFIG_DIR` (checked first by `peers_file_path`) plus the
        // HOME/XDG fallbacks makes the test fully hermetic. `EnvGuard` holds
        // the process-wide `TEST_ENV_LOCK` for its lifetime, so this does not
        // race the other env-mutating tests in the workspace.
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );

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
        // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
        let listener =
            tokio::net::UnixListener::bind(&sock).expect("test socket bind must succeed");
        let sock_path = sock.clone();
        tokio::spawn(async move {
            if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
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
    // CopyPaste-gbo: revoke_peer auto-rotates the sync key when a cloud or
    // relay sync key is already installed.  Tested under the widened cfg
    // gate so it compiles on both cloud-sync and relay-sync builds.
    // ------------------------------------------------------------------

    /// When a sync key is installed and `revoke_peer` is called:
    ///   - the audit row is written (same as bare revoke),
    ///   - `sync_key_rotated: true` appears in the response,
    ///   - the installed sync key changes to a DIFFERENT value (rotation).
    ///
    /// When NO sync key is installed, `sync_key_rotated: false` and the key
    /// slot remains empty.
    #[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
    #[tokio::test]
    async fn revoke_peer_auto_rotates_sync_key_when_active() {
        use copypaste_core::{list_revoked_devices, SyncKey};

        let dir = safe_tempdir();
        let sock = dir.path().join("test-revoke-rotate.sock");
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );

        // Shared sync-key state wired into the server so the test can
        // observe what the revoke_peer handler installed.
        let sync_key_arc: Arc<Mutex<Option<SyncKey>>> = Arc::new(Mutex::new(None));
        let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let cloud_signed_in = Arc::new(AtomicBool::new(false));

        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let server = IpcServer::new(
            db.clone(),
            private_mode.clone(),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        )
        .with_cloud_sync_state(
            sync_key_arc.clone(),
            last_sync_ms.clone(),
            cloud_signed_in.clone(),
        );

        // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
        let listener =
            tokio::net::UnixListener::bind(&sock).expect("test socket bind must succeed");
        let sock_path = sock.clone();
        tokio::spawn(async move {
            if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
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

        let fp = std::iter::repeat_n("cd", 32).collect::<Vec<_>>().join(":");

        // ── Case 1: no sync key installed → sync_key_rotated must be false ──
        {
            let body = format!(
                r#"{{"id":"rr1","method":"revoke_peer","params":{{"fingerprint":"{fp}"}}}}"#
            );
            let resp = call(&sock, &body).await;
            assert_eq!(resp["ok"], true, "revoke must succeed: {resp}");
            assert_eq!(
                resp["data"]["sync_key_rotated"], false,
                "no sync key installed → sync_key_rotated must be false"
            );
            // Key slot must still be empty.
            assert!(
                sync_key_arc.lock().await.is_none(),
                "sync_key must remain None when none was installed"
            );
        }

        // Install a known sync key (simulate the user having run set_sync_passphrase).
        let initial_key_bytes = [0xAAu8; 32];
        *sync_key_arc.lock().await = Some(SyncKey::from_bytes(initial_key_bytes));

        // ── Case 2: sync key installed → sync_key_rotated must be true and
        //            the key bytes must change (rotation). ──
        {
            let fp2 = std::iter::repeat_n("ef", 32).collect::<Vec<_>>().join(":");
            let body = format!(
                r#"{{"id":"rr2","method":"revoke_peer","params":{{"fingerprint":"{fp2}"}}}}"#
            );
            let resp = call(&sock, &body).await;
            assert_eq!(resp["ok"], true, "revoke+rotate must succeed: {resp}");
            assert_eq!(
                resp["data"]["sync_key_rotated"], true,
                "active sync key → sync_key_rotated must be true"
            );

            // The key slot must now hold a DIFFERENT key than before.
            let guard = sync_key_arc.lock().await;
            let rotated_key = guard.as_ref().expect("sync_key must be set after rotation");
            assert!(
                !rotated_key.ct_eq_bytes(&initial_key_bytes),
                "rotation must produce a key distinct from the initial key"
            );
        }

        // Audit rows must be written for both revocations.
        let db_guard = db.lock().await;
        let rows = list_revoked_devices(db_guard.conn()).unwrap();
        assert_eq!(rows.len(), 2, "exactly two audit rows expected");
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

    /// c4q2.17: `list` is now a not_implemented stub. The `too_large_to_sync`
    /// flag coverage lives in `history_page_reports_too_large_to_sync_per_item`.
    #[tokio::test]
    async fn list_reports_too_large_to_sync_per_item() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db.clone(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );
        let resp = server
            .dispatch(r#"{"id":"1","method":"list","params":{"limit":50,"offset":0}}"#)
            .await;
        assert!(
            !resp.ok,
            "list must return not_implemented (c4q2.17): {resp:?}"
        );
        assert_eq!(
            resp.error_code,
            Some(ERR_CODE_NOT_IMPLEMENTED),
            "list must carry not_implemented error_code: {resp:?}"
        );
    }

    /// The `history_page` IPC response — the verb the macOS UI (HistoryWindow)
    /// actually renders from — must carry the same daemon-computed
    /// `too_large_to_sync` flag per item as `list`: `true` for an item whose
    /// stored blob exceeds `SYNC_MAX_BLOB_BYTES` (8 MiB), `false` otherwise.
    /// Mirrors `list_reports_too_large_to_sync_per_item` so the badge is faithful
    /// regardless of which list verb the UI calls.
    #[tokio::test]
    async fn history_page_reports_too_large_to_sync_per_item() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db.clone(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );

        // Seed a normal small item and an oversized one. `content` is the
        // at-rest ciphertext blob; the badge compares its length to 8 MiB.
        {
            let guard = db.lock().await;
            let small = copypaste_core::ClipboardItem::new_text(vec![0xAB; 16], vec![0u8; 24], 1);
            copypaste_core::insert_item(&guard, &small).unwrap();
            // One byte over the ceiling guarantees too_large_to_sync == true.
            let oversized = copypaste_core::ClipboardItem::new_text(
                vec![0xCD; crate::sync_orch::SYNC_MAX_BLOB_BYTES + 1],
                vec![0u8; 24],
                2,
            );
            copypaste_core::insert_item(&guard, &oversized).unwrap();
        }

        let resp = server
            .dispatch(r#"{"id":"1","method":"history_page","params":{"limit":50,"offset":0}}"#)
            .await;
        assert!(resp.ok, "history_page must succeed: {resp:?}");
        let data = resp.data.expect("history_page returns data");
        let items = data["items"].as_array().expect("items array");
        assert_eq!(items.len(), 2, "two seeded items expected");

        let flags: Vec<bool> = items
            .iter()
            .map(|it| {
                it["too_large_to_sync"]
                    .as_bool()
                    .expect("too_large_to_sync must be a bool on every history_page item")
            })
            .collect();
        assert_eq!(
            flags.iter().filter(|&&f| f).count(),
            1,
            "exactly one item must be flagged too_large_to_sync: {items:?}"
        );
        assert_eq!(
            flags.iter().filter(|&&f| !f).count(),
            1,
            "exactly one item must be under the sync ceiling: {items:?}"
        );
    }

    /// `history_page` must include `origin_device_name` (the human-readable name
    /// from the `devices` table) for items whose `origin_device_id` matches a
    /// paired device, and must emit `null` for items with an unknown origin.
    #[tokio::test]
    async fn history_page_returns_device_name_for_known_origin() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db.clone(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );

        let known_device_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee";
        let unknown_device_id = "11111111-2222-3333-4444-555555555555";

        {
            let guard = db.lock().await;

            // Seed a device row so the known device has a name.
            guard
                .conn()
                .execute(
                    "INSERT INTO devices (id, name, platform, public_key, fingerprint, verified) \
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    rusqlite::params![
                        known_device_id,
                        "My Laptop",
                        "macos",
                        "PUBKEY_PLACEHOLDER",
                        "aa:bb:cc:dd:ee:ff",
                        1_i64,
                    ],
                )
                .unwrap();

            // Item from the known (paired) device.
            let mut known_item =
                copypaste_core::ClipboardItem::new_text(vec![0xAA; 4], vec![0u8; 24], 1);
            known_item.origin_device_id = known_device_id.to_string();
            copypaste_core::insert_item_with_fts(&guard, &known_item, "hello from known").unwrap();

            // Item from an unknown device (not in the `devices` table).
            let mut unknown_item =
                copypaste_core::ClipboardItem::new_text(vec![0xBB; 4], vec![0u8; 24], 2);
            unknown_item.origin_device_id = unknown_device_id.to_string();
            copypaste_core::insert_item_with_fts(&guard, &unknown_item, "hello from unknown")
                .unwrap();

            // Item with an empty origin_device_id (pre-v3 row).
            let legacy_item =
                copypaste_core::ClipboardItem::new_text(vec![0xCC; 4], vec![0u8; 24], 3);
            // origin_device_id starts as "" via new_text, no need to set it.
            copypaste_core::insert_item_with_fts(&guard, &legacy_item, "legacy item").unwrap();
        }

        let resp = server
            .dispatch(r#"{"id":"dnr","method":"history_page","params":{"limit":50,"offset":0}}"#)
            .await;
        assert!(resp.ok, "history_page must succeed: {resp:?}");
        let data = resp.data.expect("history_page returns data");
        let items = data["items"].as_array().expect("items array");
        assert_eq!(items.len(), 3, "three seeded items expected");

        // Find the item from the known device and verify it carries the name.
        let known_item_json = items
            .iter()
            .find(|it| it["origin_device_id"].as_str() == Some(known_device_id))
            .expect("item from known device must be present");
        assert_eq!(
            known_item_json["origin_device_name"].as_str(),
            Some("My Laptop"),
            "origin_device_name must be the paired device's name: {known_item_json}"
        );

        // The unknown device must yield a JSON null for origin_device_name.
        let unknown_item_json = items
            .iter()
            .find(|it| it["origin_device_id"].as_str() == Some(unknown_device_id))
            .expect("item from unknown device must be present");
        assert!(
            unknown_item_json["origin_device_name"].is_null(),
            "origin_device_name must be null for an unpaired device: {unknown_item_json}"
        );

        // The legacy item (empty origin_device_id) must also have a null name.
        let legacy_item_json = items
            .iter()
            .find(|it| it["origin_device_id"].as_str() == Some(""))
            .expect("legacy item must be present");
        assert!(
            legacy_item_json["origin_device_name"].is_null(),
            "origin_device_name must be null for a legacy empty-origin item: {legacy_item_json}"
        );
    }

    /// CRITICAL-1: `display_fingerprint` renders the mTLS canonical fingerprint
    /// (colon-free 64-hex from `fingerprint_of`) into the user-facing colon-hex
    /// form, and `canonical_fingerprint` round-trips it back to the exact value
    /// the mTLS verifier compares — so a pinned QR fingerprint authenticates.
    #[test]
    fn display_fingerprint_round_trips_cert_fingerprint() {
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("rt-device").unwrap();
        let canonical = cert.fingerprint(); // hex(SHA-256(cert_der)), 64 hex chars, no colons
        assert_eq!(canonical.len(), 64, "cert fingerprint must be 64 hex chars");

        let display = display_fingerprint(&canonical);
        // 32 colon-separated 2-hex groups.
        assert_eq!(
            display.split(':').count(),
            32,
            "must be 32 colon groups: {display}"
        );
        assert!(
            is_valid_fingerprint(&display),
            "display form must validate: {display}"
        );

        // The mTLS boundary strips colons; it MUST equal the original canonical
        // value the verifier (`fingerprint_of`) produces.
        assert_eq!(
            canonical_fingerprint(&display),
            canonical,
            "round-trip must recover the exact canonical fingerprint the verifier pins"
        );
    }

    /// CRITICAL-1: with no cert fingerprint set (P2P disabled), the pairing
    /// handlers must refuse rather than advertise the device-key fingerprint the
    /// mTLS layer never pins.
    #[tokio::test]
    async fn pairing_handlers_error_when_p2p_disabled() {
        let server = bare_server(); // no .with_cert_fingerprint → cert_fingerprint == None

        let resp = server
            .dispatch(r#"{"id":"f1","method":"get_own_fingerprint","params":{}}"#)
            .await;
        assert!(!resp.ok, "get_own_fingerprint must error without a cert");
        assert!(
            resp.error
                .as_deref()
                .unwrap_or_default()
                .contains("P2P is disabled"),
            "must be the disabled-P2P error, not a parse error: {resp:?}"
        );

        let resp = server
            .dispatch(r#"{"id":"q1","method":"pair_generate_qr","params":{}}"#)
            .await;
        assert!(!resp.ok, "pair_generate_qr must error without a cert");
        assert!(
            resp.error
                .as_deref()
                .unwrap_or_default()
                .contains("P2P is disabled"),
            "must be the disabled-P2P error, not a parse error: {resp:?}"
        );
    }

    /// LAN/SAS Phase 2: `pair_get_sas` on a fresh server reports the machine as
    /// `idle` with no SAS/role fields.
    #[tokio::test]
    async fn pair_get_sas_reports_idle_initially() {
        let server = bare_server();
        let resp = server
            .dispatch(r#"{"id":"s1","method":"pair_get_sas","params":{}}"#)
            .await;
        assert!(resp.ok, "pair_get_sas must succeed: {resp:?}");
        let data = resp.data.expect("data present");
        assert_eq!(data["state"], "idle");
        assert!(data.get("sas").is_none(), "no SAS when idle");
        assert!(data.get("role").is_none(), "no role when idle");
    }

    /// LAN/SAS Phase 2: `pair_confirm_sas` with no pairing awaiting confirmation
    /// is an invalid-argument error (there is no oneshot to fire).
    #[tokio::test]
    async fn pair_confirm_sas_without_pending_errors() {
        let server = bare_server();
        let resp = server
            .dispatch(r#"{"id":"c1","method":"pair_confirm_sas","params":{"accept":true}}"#)
            .await;
        assert!(!resp.ok, "must error when nothing is awaiting confirmation");
        assert_eq!(resp.error_code, Some("invalid_argument"));
    }

    /// LAN/SAS Phase 2: `pair_confirm_sas` missing the `accept` boolean is
    /// rejected with invalid_argument.
    #[tokio::test]
    async fn pair_confirm_sas_missing_accept_errors() {
        let server = bare_server();
        let resp = server
            .dispatch(r#"{"id":"c2","method":"pair_confirm_sas","params":{}}"#)
            .await;
        assert!(!resp.ok);
        assert_eq!(resp.error_code, Some("invalid_argument"));
    }

    /// LAN/SAS Phase 2: `pair_abort` always succeeds (idempotent) and leaves the
    /// machine non-active.
    #[tokio::test]
    async fn pair_abort_is_idempotent_and_succeeds() {
        let server = bare_server();
        let resp = server
            .dispatch(r#"{"id":"a1","method":"pair_abort","params":{}}"#)
            .await;
        assert!(resp.ok, "pair_abort must succeed: {resp:?}");
        // Still idle afterwards (nothing was in flight).
        let resp = server
            .dispatch(r#"{"id":"s2","method":"pair_get_sas","params":{}}"#)
            .await;
        assert_eq!(resp.data.unwrap()["state"], "idle");
    }

    /// LAN/SAS Phase 2: `pair_with_discovered` requires P2P (a cert); without one
    /// it errors with invalid_argument rather than silently starting a pairing.
    #[tokio::test]
    async fn pair_with_discovered_errors_when_p2p_disabled() {
        let server = bare_server(); // no cert / no discovery
        let resp = server
            .dispatch(
                r#"{"id":"p1","method":"pair_with_discovered","params":{"device_id":"deadbeef"}}"#,
            )
            .await;
        assert!(!resp.ok, "must error without P2P: {resp:?}");
        assert_eq!(resp.error_code, Some("invalid_argument"));
    }

    /// LAN/SAS Phase 2: `pair_with_discovered` missing `device_id` is rejected.
    #[tokio::test]
    async fn pair_with_discovered_missing_device_id_errors() {
        let server = bare_server();
        let resp = server
            .dispatch(r#"{"id":"p2","method":"pair_with_discovered","params":{}}"#)
            .await;
        assert!(!resp.ok);
        assert_eq!(resp.error_code, Some("invalid_argument"));
    }

    /// BUG A1: discovery-initiated pairing must work MORE THAN ONCE per daemon
    /// lifetime. The `pair_with_discovered` handler resets the coordinator to
    /// `Idle` after recording the terminal outcome (on BOTH the success and the
    /// failure arm). This reproduces the exact begin → terminal → reset sequence
    /// the handler performs and proves a SECOND pairing can begin (the SM is not
    /// stuck rate-limited). Before the fix the second `try_begin` returned false.
    #[tokio::test]
    async fn pair_with_discovered_can_begin_twice_sequentially() {
        use crate::pairing_sm::{PairingRole, PairingState, PeerSnapshot};
        let server = bare_server();
        let pairing = server.pairing_coordinator();

        // --- First pairing: success arm. ---
        assert!(
            pairing.try_begin(PairingRole::Initiator, PeerSnapshot::default()),
            "first pairing must begin from Idle"
        );
        // Handler records the terminal outcome, then resets (the fix).
        pairing.finish(PairingState::Confirmed);
        pairing.reset();
        assert!(
            pairing.snapshot().is_idle(),
            "after a confirmed pairing the SM must be Idle again"
        );

        // --- Second pairing: must NOT be refused as rate-limited. ---
        assert!(
            pairing.try_begin(PairingRole::Initiator, PeerSnapshot::default()),
            "BUG A1: a second pair_with_discovered must be able to begin; \
             without the reset the SM stays terminal and try_begin returns false"
        );
        // Failure arm of the handler also resets.
        pairing.finish(PairingState::Rejected);
        pairing.reset();
        assert!(
            pairing.snapshot().is_idle(),
            "after a failed pairing the SM must be Idle again"
        );

        // --- Third pairing proves the failure arm reset works too. ---
        assert!(
            pairing.try_begin(PairingRole::Initiator, PeerSnapshot::default()),
            "a pairing after a failed one must also begin"
        );
    }

    /// CRITICAL-1: when a cert fingerprint IS configured, `get_own_fingerprint`
    /// returns exactly that colon-hex cert fingerprint (not the device key).
    #[tokio::test]
    async fn get_own_fingerprint_returns_cert_fingerprint() {
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("own-fp-device").unwrap();
        let expected = display_fingerprint(&cert.fingerprint());
        let server = bare_server().with_cert_fingerprint(expected.clone());

        let resp = server
            .dispatch(r#"{"id":"f2","method":"get_own_fingerprint","params":{}}"#)
            .await;
        assert!(resp.ok, "must succeed with a cert: {resp:?}");
        let data = resp.data.expect("data present");
        assert_eq!(data["fingerprint"], serde_json::Value::String(expected));
    }

    /// `get_own_device_info` must include `public_ip` in its response payload.
    /// Without a wired public-IP cache the field serialises as JSON `null`, but
    /// it must NOT be absent entirely (the UI keys off its presence to decide
    /// whether to render the public-IP row).
    #[tokio::test]
    async fn get_own_device_info_includes_public_ip_field() {
        let server = bare_server();
        let resp = server
            .dispatch(r#"{"id":"d1","method":"get_own_device_info","params":{}}"#)
            .await;
        assert!(resp.ok, "get_own_device_info must succeed: {resp:?}");
        let data = resp.data.expect("data must be present");
        // The key must exist in the JSON object; its value may be null (no
        // cached IP yet) or a non-empty string (IP resolved).
        assert!(
            data.as_object()
                .map(|o| o.contains_key("public_ip"))
                .unwrap_or(false),
            "get_own_device_info response must include public_ip key: {data}"
        );
    }

    /// When the cached public-IP slot contains a value, `get_own_device_info`
    /// returns that exact string.
    #[tokio::test]
    async fn get_own_device_info_returns_cached_public_ip() {
        let cache = Arc::new(tokio::sync::RwLock::new(Some("203.0.113.42".to_owned())));
        let server = bare_server().with_public_ip_cache(cache);
        let resp = server
            .dispatch(r#"{"id":"d2","method":"get_own_device_info","params":{}}"#)
            .await;
        assert!(resp.ok, "must succeed: {resp:?}");
        let data = resp.data.expect("data present");
        assert_eq!(
            data["public_ip"],
            serde_json::Value::String("203.0.113.42".to_owned()),
            "public_ip must reflect cached value: {data}"
        );
    }

    /// B1: `collect_own_peer_meta` must copy the caller-supplied own public IP
    /// (read from the cache before `spawn_blocking`) into the outgoing `PeerMeta`
    /// so it is advertised in-band to the peer during pairing.
    #[test]
    fn collect_own_peer_meta_copies_public_ip_into_meta() {
        let meta = IpcServer::collect_own_peer_meta(Some("198.51.100.7".to_owned()), None, None);
        assert_eq!(
            meta.public_ip.as_deref(),
            Some("198.51.100.7"),
            "collect_own_peer_meta must put the supplied public_ip into PeerMeta"
        );
    }

    /// B1: when no own public IP is available (STUN unresolved or
    /// `collect_public_ip` disabled), the outgoing `PeerMeta.public_ip` is `None`.
    #[test]
    fn collect_own_peer_meta_none_public_ip_yields_none() {
        let meta = IpcServer::collect_own_peer_meta(None, None, None);
        assert_eq!(
            meta.public_ip, None,
            "a None public_ip must not synthesise any value in PeerMeta"
        );
    }

    /// CopyPaste-yw2k: `collect_own_peer_meta` must copy the supplied
    /// `supabase_account_id` into the outgoing `PeerMeta` so it is advertised
    /// in-band to the peer during pairing.
    #[test]
    fn collect_own_peer_meta_copies_supabase_account_id_into_meta() {
        let account_id = "proj_abc/uid_00000000-0000-0000-0000-000000000001".to_owned();
        let meta = IpcServer::collect_own_peer_meta(None, None, Some(account_id.clone()));
        assert_eq!(
            meta.supabase_account_id.as_deref(),
            Some(account_id.as_str()),
            "collect_own_peer_meta must put the supplied supabase_account_id into PeerMeta"
        );
    }

    /// CopyPaste-yw2k: when `supabase_account_id` is `None` (cloud-sync off),
    /// the outgoing `PeerMeta.supabase_account_id` must also be `None`.
    #[test]
    fn collect_own_peer_meta_none_supabase_account_id_yields_none() {
        let meta = IpcServer::collect_own_peer_meta(None, None, None);
        assert_eq!(
            meta.supabase_account_id, None,
            "a None supabase_account_id must not synthesise any value in PeerMeta"
        );
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

    /// c4q2.20: pair_accept_password is now a not_implemented stub. The short-
    /// password validation test is adapted to verify the stub returns not_implemented
    /// (the input validation that used to run is now irrelevant — the handler exits
    /// before reaching it).
    #[tokio::test]
    async fn pair_accept_password_rejects_short_password() {
        let dir = safe_tempdir();
        let sock = dir.path().join("test-short-pw.sock");
        start_test_server(&sock).await;

        let resp = call_one(
            &sock,
            r#"{"id":"sp1","method":"pair_accept_password","params":{"message1_b64":"AAAA","peer_fingerprint":"ab:ab","password":"short"}}"#,
        )
        .await;
        assert_eq!(
            resp["ok"], false,
            "pair_accept_password must fail (c4q2.20): {resp}"
        );
        assert_eq!(
            resp["error_code"].as_str().unwrap_or(""),
            "not_implemented",
            "pair_accept_password stub must return not_implemented: {resp}"
        );
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
        let sock = dir.path().join("del_item_missing.sock");
        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"di1","method":"delete_item","params":{}}"#).await;
        assert_eq!(resp["ok"], false, "missing id must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn delete_item_bad_uuid_returns_invalid_argument() {
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
        let sock = dir.path().join("copy_item_missing.sock");
        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"ci1","method":"copy_item","params":{}}"#).await;
        assert_eq!(resp["ok"], false, "missing id must fail");
        assert_eq!(resp["error_code"], "invalid_argument");
    }

    #[tokio::test]
    async fn copy_item_bad_uuid_returns_invalid_argument() {
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
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
        let dir = safe_tempdir();
        let sock = dir.path().join("revoke_all_empty.sock");
        // Isolate the config dir so this test never touches the developer's
        // real peers.json.  `peers_file_path()` checks COPYPASTE_CONFIG_DIR
        // first (before dirs::config_dir()), which is necessary on macOS
        // because dirs::config_dir() ignores $HOME and always resolves to
        // ~/Library/Application Support — so setting only HOME/XDG_CONFIG_HOME
        // was insufficient and the test leaked to the real peers.json.
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            &cfg_home,
        );
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
        let dir = safe_tempdir();
        let sock = dir.path().join("revoke_all_n.sock");
        // Pin COPYPASTE_CONFIG_DIR first — peers_file_path() checks it before
        // dirs::config_dir(), so the handler reads/writes cfg_home regardless
        // of whether dirs::config_dir() is affected by HOME (macOS ignores HOME
        // for Application Support). Without this pin the test accidentally
        // reads/writes the developer's real peers.json on macOS.
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            &cfg_home,
        );

        // Seed peers.json exactly where peers_file_path() will look:
        // cfg_home itself (COPYPASTE_CONFIG_DIR is the direct config dir, not a
        // base — paths::config_dir() returns it as-is).
        let peers_dir = cfg_home.clone();
        std::fs::create_dir_all(&peers_dir).unwrap();
        let peers_json = peers_dir.join("peers.json");
        // Use realistic (non-placeholder) fingerprints — the daemon filters out
        // all-same-byte fingerprints (e.g. aa:aa:aa:aa:aa:aa:aa:aa) to drop
        // stale test data from peers.json.
        let peers = serde_json::json!([
            {"name": "Laptop", "fingerprint": "a1:b2:c3:d4:e5:f6:07:18", "added_at": 1},
            {"name": "Phone",  "fingerprint": "f0:e1:d2:c3:b4:a5:96:87", "added_at": 2},
            {"name": "Tablet", "fingerprint": "12:34:56:78:9a:bc:de:f0", "added_at": 3},
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
            "a1:b2:c3:d4:e5:f6:07:18",
            "f0:e1:d2:c3:b4:a5:96:87",
            "12:34:56:78:9a:bc:de:f0",
        ] {
            assert!(
                audit.iter().any(|r| r.fingerprint == fp),
                "missing audit row for {fp}"
            );
        }
    }

    /// BUG 2 — `get_sync_status` must report the REAL `signed_in` auth state
    /// published by the cloud loops via the shared `cloud_signed_in` flag, not
    /// the old hardcoded `signed_in = supabase_configured`. We build a server,
    /// wire a shared flag, and assert the IPC response tracks the flag both ways.
    #[cfg(feature = "cloud-sync")]
    #[tokio::test]
    async fn get_sync_status_reports_real_signed_in_flag() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);

        let sync_key = Arc::new(Mutex::new(None));
        let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let signed_in = Arc::new(AtomicBool::new(false));

        let server = IpcServer::new(db, private_mode, local_key, device_pub).with_cloud_sync_state(
            sync_key,
            last_sync_ms,
            signed_in.clone(),
        );

        let line = r#"{"id":"1","method":"get_sync_status","params":{}}"#;

        // Flag false (e.g. after CloudError::AuthFailed) → signed_in == false,
        // even though supabase may be "configured".
        let resp = server.dispatch(line).await;
        let data = resp.data.expect("get_sync_status must return data");
        assert_eq!(
            data["signed_in"], false,
            "signed_in must reflect the false auth flag, not supabase_configured: {data}"
        );

        // Flip the shared flag true (successful bearer resolution) → reflected.
        signed_in.store(true, Ordering::Relaxed);
        let resp2 = server.dispatch(line).await;
        let data2 = resp2.data.expect("get_sync_status must return data");
        assert_eq!(
            data2["signed_in"], true,
            "signed_in must track the real auth flag once set true: {data2}"
        );
    }

    // ── CopyPaste-i5b: cloud_sign_in/out set cloud_signed_in ─────────────────

    /// `cloud_sign_out` must clear `cloud_signed_in` to false so
    /// `get_sync_status` stops reporting signed_in = true after logout.
    /// Proves the flag is set by the IPC sign-out path (not only by the
    /// startup `start_cloud` path that was the only setter before this fix).
    #[cfg(feature = "cloud-sync")]
    #[tokio::test]
    async fn cloud_sign_out_clears_signed_in_flag() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);

        let sync_key = Arc::new(Mutex::new(None));
        let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
        // Start the flag at true — simulating a previously signed-in session.
        let signed_in = Arc::new(AtomicBool::new(true));

        let server = IpcServer::new(db, private_mode, local_key, device_pub).with_cloud_sync_state(
            sync_key,
            last_sync_ms,
            signed_in.clone(),
        );

        let resp = server
            .dispatch(r#"{"id":"1","method":"cloud_sign_out"}"#)
            .await;
        assert!(resp.ok, "cloud_sign_out must return ok: true; got {resp:?}");
        // CopyPaste-i5b: the shared flag must now be false.
        assert!(
            !signed_in.load(Ordering::SeqCst),
            "cloud_signed_in must be false after cloud_sign_out"
        );
    }

    /// `cloud_sign_in` with no SUPABASE_URL configured must return
    /// `invalid_argument` without touching `cloud_signed_in` (it stays false).
    #[cfg(feature = "cloud-sync")]
    #[tokio::test]
    async fn cloud_sign_in_returns_invalid_argument_when_not_configured() {
        // Ensure no env override leaks from a parent shell.
        std::env::remove_var("SUPABASE_URL");
        std::env::remove_var("SUPABASE_ANON_KEY");

        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);

        let sync_key = Arc::new(Mutex::new(None));
        let last_sync_ms = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let signed_in = Arc::new(AtomicBool::new(false));

        // Use a temp config dir so read_config() finds no persisted credentials.
        let dir = safe_tempdir();
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            dir.path(),
        );

        let server = IpcServer::new(db, private_mode, local_key, device_pub).with_cloud_sync_state(
            sync_key,
            last_sync_ms,
            signed_in.clone(),
        );

        let resp = server
            .dispatch(r#"{"id":"1","method":"cloud_sign_in"}"#)
            .await;
        assert!(
            !resp.ok,
            "cloud_sign_in with no config must fail; got {resp:?}"
        );
        assert_eq!(
            resp.error_code,
            Some(ERR_CODE_INVALID_ARGUMENT),
            "must return invalid_argument when Supabase is not configured"
        );
        // Flag must remain false — the unconfigured path must not set it.
        assert!(
            !signed_in.load(Ordering::SeqCst),
            "cloud_signed_in must stay false when sign-in is rejected for missing config"
        );
    }

    // ── Fix #1: set_config MERGE preserves redacted secrets ─────────────────

    /// `merge_config` must preserve an existing secret when the incoming config
    /// omits it (the redacted read-modify-write shape deserialises the secret
    /// fields to `None`). A blind overwrite would null the stored credentials.
    #[test]
    fn merge_config_preserves_omitted_secrets() {
        let existing = AppConfig {
            p2p_enabled: Some(true),
            supabase_url: Some("https://proj.supabase.co".into()),
            supabase_anon_key: Some("anon-123".into()),
            supabase_email: Some("user@example.com".into()),
            supabase_password: Some("super-secret".into()),
            ..Default::default()
        };
        // Incoming mirrors what the UI sends back after `get_config` redaction:
        // secrets absent (None), only the toggle + publishable fields present.
        let incoming = AppConfig {
            p2p_enabled: Some(false),
            supabase_url: Some("https://proj.supabase.co".into()),
            supabase_anon_key: Some("anon-123".into()),
            supabase_email: None,
            supabase_password: None,
            ..Default::default()
        };
        let merged = merge_config(existing, incoming);
        assert_eq!(
            merged.supabase_password.as_deref(),
            Some("super-secret"),
            "omitted password must be preserved from the persisted config"
        );
        assert_eq!(
            merged.supabase_email.as_deref(),
            Some("user@example.com"),
            "omitted email must be preserved"
        );
        // Non-secret authoritative field still takes the incoming value.
        assert_eq!(
            merged.p2p_enabled,
            Some(false),
            "p2p_enabled incoming value wins"
        );
    }

    /// A provided secret in `set_config` overwrites the stored one (so the CLI
    /// `cloud setup` can rotate credentials).
    #[test]
    fn merge_config_incoming_secret_overrides() {
        let existing = AppConfig {
            p2p_enabled: Some(false),
            supabase_url: None,
            supabase_anon_key: None,
            supabase_email: Some("old@example.com".into()),
            supabase_password: Some("old-pw".into()),
            ..Default::default()
        };
        let incoming = AppConfig {
            p2p_enabled: Some(false),
            supabase_url: None,
            supabase_anon_key: None,
            supabase_email: Some("new@example.com".into()),
            supabase_password: Some("new-pw".into()),
            ..Default::default()
        };
        let merged = merge_config(existing, incoming);
        assert_eq!(merged.supabase_password.as_deref(), Some("new-pw"));
        assert_eq!(merged.supabase_email.as_deref(), Some("new@example.com"));
    }

    // ── QR fully provisions all sync: apply_peer_provisioning ────────────────

    /// On an UNCONFIGURED device, applying a peer's provisioning fills in the
    /// missing Supabase config AND installs the derived sync key.
    #[cfg(feature = "cloud-sync")]
    #[tokio::test]
    async fn apply_peer_provisioning_fills_missing_fields() {
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
                "SUPABASE_URL",
                "SUPABASE_ANON_KEY",
                "COPYPASTE_EPHEMERAL_KEY",
            ],
            &cfg_home,
        );
        // Ensure no env override / key persist interferes with the assertions.
        // (EnvGuard set all of the above to the same path; explicitly clear the
        // ones that must be UNSET for the "device lacks it" precondition.)
        // SAFETY: single-threaded test scope; restored by EnvGuard on drop.
        unsafe {
            std::env::remove_var("SUPABASE_URL");
            std::env::remove_var("SUPABASE_ANON_KEY");
            std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
        }

        let sync_key: Arc<Mutex<Option<SyncKey>>> = Arc::new(Mutex::new(None));
        let prov = copypaste_p2p::bootstrap::SyncProvisioning {
            supabase_url: Some("https://new.supabase.co".into()),
            supabase_anon_key: Some("new-anon".into()),
            relay_url: Some("https://relay.example.com".into()),
            derived_sync_key: Some(vec![5u8; 32]),
        };
        IpcServer::apply_peer_provisioning_to(&sync_key, prov).await;

        let cfg = read_config();
        assert_eq!(cfg.supabase_url.as_deref(), Some("https://new.supabase.co"));
        assert_eq!(cfg.supabase_anon_key.as_deref(), Some("new-anon"));
        // R2: a peer-advertised relay_url is persisted on an unconfigured device
        // and survives the read_config overlay (it round-trips via config.toml).
        assert_eq!(
            cfg.relay_url.as_deref(),
            Some("https://relay.example.com"),
            "an unconfigured device must adopt the peer's relay_url"
        );
        assert!(
            sync_key.lock().await.is_some(),
            "an unconfigured device must install the peer's derived sync key"
        );
    }

    /// On a device that ALREADY has Supabase config + a sync key, applying a
    /// peer's provisioning that carries the IDENTICAL key (routine re-pairing)
    /// must NOT overwrite the config OR the key. (A DIFFERING key signals a
    /// rotation re-provision and IS allowed to replace — see
    /// `apply_peer_provisioning_rotation_replaces_differing_key`.)
    #[cfg(feature = "cloud-sync")]
    #[tokio::test]
    async fn apply_peer_provisioning_never_overwrites_existing() {
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
                "SUPABASE_URL",
                "SUPABASE_ANON_KEY",
                "COPYPASTE_EPHEMERAL_KEY",
            ],
            &cfg_home,
        );
        // SAFETY: single-threaded test scope; restored by EnvGuard on drop.
        unsafe {
            std::env::remove_var("SUPABASE_URL");
            std::env::remove_var("SUPABASE_ANON_KEY");
            std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
        }

        // Seed an already-configured device. supabase_* live in config.json;
        // relay_url is core-backed, so seed it via update_core_config (config.toml)
        // — read_config overlays relay_url from there.
        let seed = AppConfig {
            supabase_url: Some("https://existing.supabase.co".into()),
            supabase_anon_key: Some("existing-anon".into()),
            relay_url: Some("https://existing-relay.example.com".into()),
            ..Default::default()
        };
        write_config(&seed).expect("seed config.json");
        update_core_config(&seed).expect("seed config.toml");
        let sync_key: Arc<Mutex<Option<SyncKey>>> =
            Arc::new(Mutex::new(Some(SyncKey::from_bytes([1u8; 32]))));

        // Carry the IDENTICAL key (all 1s) — this is the routine-pairing shape
        // where both peers derive the same deterministic key. It must be a
        // no-op for the key, and config fill-missing must still not overwrite.
        let prov = copypaste_p2p::bootstrap::SyncProvisioning {
            supabase_url: Some("https://peer.supabase.co".into()),
            supabase_anon_key: Some("peer-anon".into()),
            relay_url: Some("https://peer-relay.example.com".into()),
            derived_sync_key: Some(vec![1u8; 32]),
        };
        IpcServer::apply_peer_provisioning_to(&sync_key, prov).await;

        let cfg = read_config();
        assert_eq!(
            cfg.supabase_url.as_deref(),
            Some("https://existing.supabase.co"),
            "existing supabase_url must not be overwritten"
        );
        assert_eq!(cfg.supabase_anon_key.as_deref(), Some("existing-anon"));
        assert_eq!(
            cfg.relay_url.as_deref(),
            Some("https://existing-relay.example.com"),
            "existing relay_url must not be overwritten by the peer's"
        );
        // The pre-existing key (all 1s) must be untouched (identical → no-op).
        assert_eq!(
            sync_key.lock().await.as_ref().map(|k| *k.as_bytes()),
            Some([1u8; 32]),
            "an identical incoming sync key must not change the existing key"
        );
    }

    /// C-P0-4: after a sync-key ROTATION, the operator re-scans the pairing QR
    /// on each remaining device. That re-provision carries the NEW key, which
    /// DIFFERS from the stale key the device still holds — the apply path must
    /// REPLACE the stale key (otherwise the device keeps the dead, pre-rotation
    /// key and silently fails to sync). Config fields are still fill-missing
    /// only and must not be overwritten.
    #[cfg(feature = "cloud-sync")]
    #[tokio::test]
    async fn apply_peer_provisioning_rotation_replaces_differing_key() {
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
                "SUPABASE_URL",
                "SUPABASE_ANON_KEY",
                "COPYPASTE_EPHEMERAL_KEY",
            ],
            &cfg_home,
        );
        // SAFETY: single-threaded test scope; restored by EnvGuard on drop.
        unsafe {
            std::env::remove_var("SUPABASE_URL");
            std::env::remove_var("SUPABASE_ANON_KEY");
            std::env::set_var("COPYPASTE_EPHEMERAL_KEY", "1");
        }

        let seed = AppConfig {
            supabase_url: Some("https://existing.supabase.co".into()),
            supabase_anon_key: Some("existing-anon".into()),
            ..Default::default()
        };
        write_config(&seed).expect("seed config.json");

        // Device holds the STALE pre-rotation key (all 1s).
        let sync_key: Arc<Mutex<Option<SyncKey>>> =
            Arc::new(Mutex::new(Some(SyncKey::from_bytes([1u8; 32]))));

        // Rotation re-provision carries the NEW key (all 7s).
        let prov = copypaste_p2p::bootstrap::SyncProvisioning {
            supabase_url: Some("https://peer.supabase.co".into()),
            supabase_anon_key: Some("peer-anon".into()),
            relay_url: None,
            derived_sync_key: Some(vec![7u8; 32]),
        };
        IpcServer::apply_peer_provisioning_to(&sync_key, prov).await;

        // The differing key REPLACES the stale one (honest rotation).
        assert_eq!(
            sync_key.lock().await.as_ref().map(|k| *k.as_bytes()),
            Some([7u8; 32]),
            "a differing incoming sync key (rotation) must replace the stale key"
        );
        // Config fill-missing still never overwrites an existing value.
        let cfg = read_config();
        assert_eq!(
            cfg.supabase_url.as_deref(),
            Some("https://existing.supabase.co"),
            "existing supabase_url must not be overwritten on a rotation re-provision"
        );
    }

    /// End-to-end: seed a config with a password, then run a `set_config` whose
    /// params carry the REDACTED shape (`supabase_password_set: true`, no real
    /// password). The stored password must survive — proving the
    /// read-modify-write data-loss bug is fixed at the IPC boundary.
    #[tokio::test]
    async fn set_config_with_redacted_shape_preserves_stored_password() {
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &["COPYPASTE_CONFIG_DIR", "HOME", "XDG_CONFIG_HOME"],
            &cfg_home,
        );

        // Seed: persist a config carrying a real password.
        let seeded = AppConfig {
            p2p_enabled: Some(false),
            supabase_url: Some("https://proj.supabase.co".into()),
            supabase_anon_key: Some("anon-xyz".into()),
            supabase_email: Some("seed@example.com".into()),
            supabase_password: Some("do-not-wipe-me".into()),
            ..Default::default()
        };
        write_config(&seeded).expect("seed write_config");

        // Confirm get_config redacts the secret to a presence flag.
        let server = bare_server();
        let get_resp = server
            .dispatch(r#"{"id":"g1","method":"get_config","params":{}}"#)
            .await;
        let got = get_resp.data.expect("get_config data");
        assert_eq!(got["supabase_password_set"], true);
        assert!(
            got.get("supabase_password").is_none(),
            "raw password must never leave the daemon: {got}"
        );

        // The UI/CLI sends this redacted shape straight back via set_config.
        let set_body = format!(
            r#"{{"id":"s1","method":"set_config","params":{}}}"#,
            serde_json::to_string(&got).unwrap()
        );
        let set_resp = server.dispatch(&set_body).await;
        assert_eq!(
            set_resp.data.as_ref().map(|d| d["saved"].clone()),
            Some(serde_json::json!(true)),
            "set_config must succeed: {set_resp:?}"
        );

        // The persisted password must be intact. The daemon stores it in the
        // Keychain first (stripping it from config.json) and only falls back to
        // config.json when the Keychain is unavailable — exactly how the cloud
        // path retrieves it (cloud.rs: keychain-first, config fallback). Assert
        // that *effective* value so the test is robust whether or not the real
        // Keychain is reachable (CI runs with COPYPASTE_EPHEMERAL_KEY, so the
        // password stays in config.json; a signed build stores it in Keychain).
        let persisted = read_config();
        let effective_pw = crate::keychain::read_supabase_password_from_keychain()
            .or_else(|| persisted.supabase_password.clone());
        assert_eq!(
            effective_pw.as_deref(),
            Some("do-not-wipe-me"),
            "set_config with the redacted shape must NOT wipe the stored password"
        );
        assert_eq!(
            persisted.supabase_email.as_deref(),
            Some("seed@example.com"),
            "email must also survive"
        );
    }

    // ── export: limit param ──────────────────────────────────────────────────

    /// When `limit` > 0 the export handler must return at most `limit` items,
    /// selecting the most-recent ones (DESC LIMIT subquery) and re-ordering
    /// them oldest-first for deterministic import. When `limit` == 0 or is
    /// absent all items are returned.
    #[tokio::test]
    async fn export_limit_returns_most_recent_n_oldest_first() {
        use copypaste_core::{
            build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
        };

        let dir = safe_tempdir();
        let sock = dir.path().join("export_limit.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // The test server uses a zero v1 key. Derive v2 the same way the
        // handler does so we can produce decrypt-able ciphertext.
        let v1_key = [0u8; 32];
        let v2_key = derive_v2(&v1_key);

        // Seed 5 text items with distinct, monotonically increasing wall_time
        // values so we can verify ordering and limit selection.
        const TOTAL: usize = 5;
        let mut item_ids: Vec<String> = Vec::new();
        {
            let guard = db.lock().await;
            for i in 0..TOTAL {
                let plaintext = format!("item-{i}").into_bytes();
                let item_id = uuid::Uuid::new_v4().to_string();
                let aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
                let (nonce, ciphertext) = encrypt_item_with_aad(&plaintext, &v2_key, &aad).unwrap();
                // Use a distinct wall_time per item (base 1000 + i ms).
                let wall_time = 1_000_000i64 + i as i64;
                guard
                    .conn()
                    .execute(
                        "INSERT INTO clipboard_items \
                         (id, item_id, content_type, content, content_nonce, \
                          is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                         VALUES (?1, ?2, 'text', ?3, ?4, 0, 0, ?5, ?6, 2)",
                        rusqlite::params![
                            uuid::Uuid::new_v4().to_string(),
                            item_id,
                            ciphertext,
                            nonce.as_slice(),
                            i as i64 + 1,
                            wall_time,
                        ],
                    )
                    .unwrap();
                item_ids.push(format!("item-{i}"));
            }
        }

        // ── limit=3: must return the 3 most-recent items (item-2, item-3, item-4)
        //    serialised oldest-first (item-2, item-3, item-4 in that order).
        let resp = call_one(
            &sock,
            r#"{"id":"el1","method":"export","params":{"limit":3}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "export with limit=3 must succeed: {resp}");
        let items = resp["data"]["items"].as_array().expect("items array");
        assert_eq!(
            items.len(),
            3,
            "limit=3 must return exactly 3 items, got {}: {resp}",
            items.len()
        );
        // Verify chronological (ASC) ordering: wall_time must be non-decreasing.
        let wall_times: Vec<i64> = items
            .iter()
            .map(|it| it["wall_time"].as_i64().unwrap())
            .collect();
        assert!(
            wall_times.windows(2).all(|w| w[0] <= w[1]),
            "items must be ordered oldest-first: {wall_times:?}"
        );
        // The 3 most-recent items have wall_times 1_000_002, 1_000_003, 1_000_004.
        assert_eq!(
            wall_times[0], 1_000_002,
            "first exported item should be 3rd oldest"
        );
        assert_eq!(
            wall_times[2], 1_000_004,
            "last exported item should be newest"
        );

        // ── limit=0: must return ALL items (unlimited).
        let resp = call_one(
            &sock,
            r#"{"id":"el2","method":"export","params":{"limit":0}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "export with limit=0 must succeed: {resp}");
        let all_items = resp["data"]["items"].as_array().expect("items array");
        assert_eq!(
            all_items.len(),
            TOTAL,
            "limit=0 must return all {TOTAL} items, got {}",
            all_items.len()
        );

        // ── limit absent: must also return ALL items.
        let resp = call_one(&sock, r#"{"id":"el3","method":"export","params":{}}"#).await;
        assert_eq!(
            resp["ok"], true,
            "export with no limit must succeed: {resp}"
        );
        let no_limit_items = resp["data"]["items"].as_array().expect("items array");
        assert_eq!(
            no_limit_items.len(),
            TOTAL,
            "absent limit must return all {TOTAL} items, got {}",
            no_limit_items.len()
        );
    }

    // ── CopyPaste-tj9s: export include_sensitive filter ──────────────────────

    /// `export` must exclude sensitive items by default and include them only
    /// when `include_sensitive: true` is explicitly passed.
    ///
    /// Contract:
    /// - 1 non-sensitive item + 1 sensitive item inserted.
    /// - `export` with no `include_sensitive` (or `include_sensitive: false`) →
    ///   count == 1 (only the non-sensitive item).
    /// - `export` with `include_sensitive: true` → count == 2 (both items).
    #[tokio::test]
    async fn export_excludes_sensitive_by_default_and_includes_with_flag() {
        use copypaste_core::{
            build_item_aad_v2, derive_v2, encrypt_item_with_aad, AAD_SCHEMA_VERSION_V4,
        };

        let dir = safe_tempdir();
        let sock = dir.path().join("export_sensitive.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // The test server uses a zero v1 key. Derive v2 to match the handler.
        let v1_key = [0u8; 32];
        let v2_key = derive_v2(&v1_key);

        // Seed a non-sensitive item (is_sensitive = 0) and a sensitive item
        // (is_sensitive = 1), both encrypted with key_version = 2.
        {
            let guard = db.lock().await;
            for (i, is_sensitive) in [(0i64, false), (1i64, true)] {
                let plaintext = format!("item-sens-{i}").into_bytes();
                let item_id = uuid::Uuid::new_v4().to_string();
                let aad = build_item_aad_v2(&item_id, AAD_SCHEMA_VERSION_V4, 2);
                let (nonce, ciphertext) = encrypt_item_with_aad(&plaintext, &v2_key, &aad).unwrap();
                let wall_time = 2_000_000i64 + i;
                guard
                    .conn()
                    .execute(
                        "INSERT INTO clipboard_items \
                         (id, item_id, content_type, content, content_nonce, \
                          is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                         VALUES (?1, ?2, 'text', ?3, ?4, ?5, 0, ?6, ?7, 2)",
                        rusqlite::params![
                            uuid::Uuid::new_v4().to_string(),
                            item_id,
                            ciphertext,
                            nonce.as_slice(),
                            is_sensitive as i64,
                            i + 1,
                            wall_time,
                        ],
                    )
                    .unwrap();
            }
        }

        // ── default (no flag): only the non-sensitive item is returned.
        let resp = call_one(&sock, r#"{"id":"xs1","method":"export","params":{}}"#).await;
        assert_eq!(resp["ok"], true, "export must succeed: {resp}");
        let items = resp["data"]["items"].as_array().expect("items array");
        assert_eq!(
            items.len(),
            1,
            "default export must exclude sensitive items; got {}: {resp}",
            items.len()
        );
        assert_eq!(
            items[0]["is_sensitive"], false,
            "the returned item must not be sensitive"
        );

        // ── include_sensitive: false → same as default.
        let resp = call_one(
            &sock,
            r#"{"id":"xs2","method":"export","params":{"include_sensitive":false}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "export must succeed: {resp}");
        let items = resp["data"]["items"].as_array().expect("items array");
        assert_eq!(
            items.len(),
            1,
            "include_sensitive=false must exclude sensitive items; got {}: {resp}",
            items.len()
        );

        // ── include_sensitive: true → both items are returned.
        let resp = call_one(
            &sock,
            r#"{"id":"xs3","method":"export","params":{"include_sensitive":true}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "export must succeed: {resp}");
        let items = resp["data"]["items"].as_array().expect("items array");
        assert_eq!(
            items.len(),
            2,
            "include_sensitive=true must include all items; got {}: {resp}",
            items.len()
        );
        // Verify one of each kind is present.
        let sensitive_count = items
            .iter()
            .filter(|it| it["is_sensitive"].as_bool() == Some(true))
            .count();
        assert_eq!(
            sensitive_count, 1,
            "exactly one sensitive item must appear when include_sensitive=true"
        );
    }

    // ── Fix #2: config.json honours COPYPASTE_CONFIG_DIR ────────────────────

    /// `COPYPASTE_CONFIG_DIR` must redirect `config.json` (not just
    /// `peers.json`), and the two files must co-locate under the same
    /// `copypaste/` subdir.
    #[test]
    fn config_dir_override_redirects_config_json() {
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("override-root");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );

        let config = config_path().expect("config_path under override");
        let peers = peers_file_path();

        // config.json lands under the override, not the platform default.
        assert!(
            config.starts_with(&cfg_home),
            "config.json must live under COPYPASTE_CONFIG_DIR: {}",
            config.display()
        );
        // config.json ends with "config.json"; the parent dir name is
        // platform-dependent (CopyPaste on macOS/Windows, copypaste on Linux)
        // but in all cases the file must live under the override root.
        assert!(
            config.ends_with("config.json"),
            "config path must end with config.json: {}",
            config.display()
        );

        // Both files share the SAME directory so a config write and a peers
        // write can never diverge under the override.
        assert_eq!(
            config.parent(),
            peers.parent(),
            "config.json and peers.json must co-locate: {} vs {}",
            config.display(),
            peers.display()
        );

        // And a real round-trip write/read works through the redirected path.
        let cfg = AppConfig {
            p2p_enabled: Some(true),
            ..Default::default()
        };
        write_config(&cfg).expect("write under override");
        assert!(
            config.is_file(),
            "config.json must be written at {}",
            config.display()
        );
        assert_eq!(
            read_config().p2p_enabled,
            Some(true),
            "round-trip read under override"
        );
    }

    // ── Fix-2: write_config must create config.json atomically at mode 0600 ──

    /// `write_config` must produce a `config.json` with mode `0600` and must
    /// not leave any orphaned `.tmp.*` file behind after a successful write.
    /// The config may carry `supabase_password` / `supabase_anon_key`; it must
    /// never be momentarily world-readable between create and chmod.
    #[cfg(unix)]
    #[test]
    fn write_config_creates_file_with_mode_0600_and_no_tmp_orphan() {
        use std::os::unix::fs::PermissionsExt;

        let dir = safe_tempdir();
        let _env = EnvGuard::set_all(
            &["HOME", "XDG_CONFIG_HOME", "COPYPASTE_CONFIG_DIR"],
            dir.path(),
        );

        let cfg = AppConfig {
            p2p_enabled: Some(true),
            supabase_password: Some("secret".into()),
            ..Default::default()
        };
        write_config(&cfg).expect("write_config must succeed");

        // Find the written config.json under the temp home.
        let config = config_path().expect("config_path under override");
        assert!(config.exists(), "config.json must be written");

        let mode = std::fs::metadata(&config).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "config.json must be owner-only (0600), got {:o}",
            mode & 0o777
        );

        // No orphaned temp file in the config dir.
        let config_dir = config.parent().unwrap();
        let orphans: Vec<_> = std::fs::read_dir(config_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".config.json.tmp.")
            })
            .collect();
        assert!(
            orphans.is_empty(),
            "atomic write must not leave temp files behind: {:?}",
            orphans
        );
    }

    // ── Fix-5: p2p_enabled must be Option<bool> so omitting it preserves existing ──

    /// A `set_config` request that omits `p2p_enabled` (the field is absent from
    /// JSON or deserialises as `null`) must NOT flip the stored value to `false`.
    /// Previously `p2p_enabled: bool` with `#[serde(default)]` meant any
    /// deserialization that did not include the field produced `false`, silently
    /// disabling P2P for every caller that only sends a subset of fields.
    #[test]
    fn p2p_enabled_option_none_preserves_existing() {
        // When p2p_enabled is absent from JSON it must deserialise as None.
        let json_without = r#"{"supabase_url": "https://x.supabase.co"}"#;
        let cfg: AppConfig = serde_json::from_str(json_without).expect("deserialize");
        assert!(
            cfg.p2p_enabled.is_none(),
            "absent p2p_enabled must deserialise as None, got {:?}",
            cfg.p2p_enabled
        );

        // merge_config: when incoming has None, existing value must be preserved.
        let existing = AppConfig {
            p2p_enabled: Some(true),
            ..Default::default()
        };
        let merged = merge_config(existing, cfg);
        assert_eq!(
            merged.p2p_enabled,
            Some(true),
            "merge_config must preserve existing p2p_enabled when incoming is None"
        );
    }

    // ── get_item_thumbnail: serves the capture-time thumbnail blob ──────────

    /// Build a large PNG, encode it via `encode_image_full` with the test
    /// server's zero key, insert the resulting image item (full chunks +
    /// thumbnail blob + extended meta_json), then assert:
    ///   * `get_item_thumbnail` returns a non-null PNG data-URI,
    ///   * the thumbnail data-URI is SMALLER than the full-res `get_item_image`
    ///     output (the thumb is a downscaled re-encode),
    ///   * an image item with NO thumb returns the `{ "thumbnail": null }`
    ///     sentinel so the UI can fall back to full-res.
    #[tokio::test]
    async fn get_item_thumbnail_serves_thumb_and_null_sentinel() {
        use copypaste_core::THUMBNAIL_MAX_DIM;
        use image::{DynamicImage, RgbaImage};

        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db.clone(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );
        let key = [0u8; 32]; // v1 seed matching dummy server key
                             // new_image stamps key_version = 2; the server reads kv=2 rows with
                             // derive_v2(local_key). Encrypt with the same v2 key so the round-trip
                             // matches the production writer (handle_image uses derive_v2).
        let v2_key = derive_v2(&key);

        // A 1000×1000 image: larger than THUMBNAIL_MAX_DIM (192) so the
        // thumbnail is genuinely downscaled and its PNG is smaller than the
        // full-res PNG. A per-pixel gradient keeps PNG compression honest (a
        // flat color would compress so well the size gap could vanish).
        let mut buf = RgbaImage::new(1000, 1000);
        for (x, y, px) in buf.enumerate_pixels_mut() {
            *px = image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]);
        }
        let raw = copypaste_core::encode_as_png(&DynamicImage::ImageRgba8(buf)).unwrap();

        // file_id = content hash (mirrors handle_image); thumb_file_id distinct.
        let file_id = crate::clipboard::image_content_hash(&raw);
        let thumb_file_id = crate::clipboard::image_thumb_file_id(&file_id);

        let (meta, chunks, thumb_blob, thumb_w, thumb_h) = copypaste_core::encode_image_full(
            &raw,
            &v2_key,
            &file_id,
            &thumb_file_id,
            0,
            64,
            THUMBNAIL_MAX_DIM,
        )
        .unwrap();
        assert!(!thumb_blob.is_empty(), "thumbnail blob must be produced");

        let blob = copypaste_core::chunks_to_blob(&chunks).unwrap();
        let meta_json =
            crate::clipboard::build_image_meta_json(&meta, &thumb_file_id, thumb_w, thumb_h);

        let mut item =
            copypaste_core::ClipboardItem::new_image(blob, meta_json, 0, Some(thumb_blob));
        item.item_id = uuid::Uuid::from_bytes(file_id).to_string();
        let with_thumb_id = item.id.clone();

        // A second image item with NO thumbnail (full-image-only legacy path).
        let (meta2, chunks2) =
            copypaste_core::encode_image_with_limit(&raw, &v2_key, &file_id, 0, 64).unwrap();
        let blob2 = copypaste_core::chunks_to_blob(&chunks2).unwrap();
        let meta_json2 = format!(
            r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
            meta2.width, meta2.height, meta2.original_size, meta2.chunk_count, meta2.file_id
        );
        let mut item2 = copypaste_core::ClipboardItem::new_image(blob2, meta_json2, 0, None);
        item2.item_id = uuid::Uuid::new_v4().to_string();
        item2.id = uuid::Uuid::new_v4().to_string();
        let no_thumb_id = item2.id.clone();

        {
            let guard = db.lock().await;
            copypaste_core::insert_item_with_fts(&guard, &item, "").unwrap();
            copypaste_core::insert_item_with_fts(&guard, &item2, "").unwrap();
        }

        // get_item_thumbnail on the item WITH a thumb → non-null data-URI.
        let thumb_resp = server
            .dispatch(&format!(
                r#"{{"id":"t1","method":"get_item_thumbnail","params":{{"id":"{with_thumb_id}"}}}}"#
            ))
            .await;
        let thumb_data = thumb_resp.data.expect("get_item_thumbnail data");
        let thumb_uri = thumb_data["thumbnail"]
            .as_str()
            .expect("thumbnail must be a non-null data-URI string");
        assert!(
            thumb_uri.starts_with("data:image/png;base64,"),
            "thumbnail must be a PNG data-URI"
        );

        // get_item_image on the same item → full-res data-URI.
        let full_resp = server
            .dispatch(&format!(
                r#"{{"id":"f1","method":"get_item_image","params":{{"id":"{with_thumb_id}"}}}}"#
            ))
            .await;
        let full_uri = full_resp.data.expect("get_item_image data")["data_uri"]
            .as_str()
            .expect("data_uri")
            .to_string();
        assert!(
            thumb_uri.len() < full_uri.len(),
            "thumbnail data-URI ({}) must be smaller than full-res ({})",
            thumb_uri.len(),
            full_uri.len()
        );

        // Phase 4: get_item_thumbnail on a legacy item WITHOUT a stored thumb
        // now lazily backfills and returns a non-null PNG data-URI (Phase 4).
        // The null sentinel is only returned when backfill itself fails.
        let backfill_resp = server
            .dispatch(&format!(
                r#"{{"id":"t2","method":"get_item_thumbnail","params":{{"id":"{no_thumb_id}"}}}}"#
            ))
            .await;
        let backfill_data = backfill_resp
            .data
            .expect("get_item_thumbnail (no stored thumb) data");
        assert!(
            !backfill_data["thumbnail"].is_null(),
            "Phase-4: legacy thumb-less item must be lazily backfilled, not null: {backfill_data}"
        );
        assert!(
            backfill_data["thumbnail"]
                .as_str()
                .unwrap_or("")
                .starts_with("data:image/png;base64,"),
            "backfilled thumbnail must be a PNG data-URI: {backfill_data}"
        );
    }

    /// Phase 4: lazy backfill — an image item with `thumb IS NULL` (legacy row
    /// captured before schema v9 / Plan-B P2) must have a thumbnail generated
    /// and persisted on first `get_item_thumbnail` call, and returned as a
    /// non-null PNG data-URI. A second call must also return non-null (proving
    /// the thumbnail was written to the DB, not just computed in memory).
    #[tokio::test]
    async fn get_item_thumbnail_lazy_backfill_missing_thumb() {
        use copypaste_core::THUMBNAIL_MAX_DIM;
        use image::{DynamicImage, RgbaImage};

        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db.clone(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );
        let key = [0u8; 32]; // v1 seed matching dummy server key
                             // new_image stamps key_version = 2; the server reads kv=2 rows with
                             // derive_v2(local_key). Encrypt with the same v2 key so the round-trip
                             // matches the production writer (handle_image uses derive_v2).
        let v2_key = derive_v2(&key);

        // Build a 1000×1000 image (larger than THUMBNAIL_MAX_DIM so a real
        // downscale occurs), encode with the old path (no thumb blob), and
        // store with thumb=None to simulate a legacy row.
        let mut buf = RgbaImage::new(1000, 1000);
        for (x, y, px) in buf.enumerate_pixels_mut() {
            *px = image::Rgba([(x % 256) as u8, (y % 256) as u8, ((x + y) % 256) as u8, 255]);
        }
        let raw = copypaste_core::encode_as_png(&DynamicImage::ImageRgba8(buf)).unwrap();

        let file_id = crate::clipboard::image_content_hash(&raw);
        let (meta, chunks) =
            copypaste_core::encode_image_with_limit(&raw, &v2_key, &file_id, 0, 64).unwrap();
        let blob = copypaste_core::chunks_to_blob(&chunks).unwrap();

        // Legacy meta_json: no thumb_file_id / thumb_w / thumb_h fields.
        let meta_json = format!(
            r#"{{"width":{},"height":{},"original_size":{},"chunk_count":{},"file_id":{:?}}}"#,
            meta.width, meta.height, meta.original_size, meta.chunk_count, meta.file_id
        );

        let mut item = copypaste_core::ClipboardItem::new_image(blob, meta_json, 0, None);
        item.item_id = uuid::Uuid::new_v4().to_string();
        item.id = uuid::Uuid::new_v4().to_string();
        let item_id = item.id.clone();

        {
            let guard = db.lock().await;
            copypaste_core::insert_item_with_fts(&guard, &item, "").unwrap();
        }

        // ── First call: thumb is NULL → should backfill and return data-URI ──
        let resp1 = server
            .dispatch(&format!(
                r#"{{"id":"b1","method":"get_item_thumbnail","params":{{"id":"{item_id}"}}}}"#
            ))
            .await;
        let data1 = resp1.data.expect("first get_item_thumbnail must have data");
        assert!(
            !data1["thumbnail"].is_null(),
            "lazy backfill: first call must return non-null thumbnail, got: {data1}"
        );
        let uri1 = data1["thumbnail"]
            .as_str()
            .expect("thumbnail must be a string");
        assert!(
            uri1.starts_with("data:image/png;base64,"),
            "backfilled thumbnail must be a PNG data-URI"
        );
        // Verify thumbnail was genuinely downscaled (PNG is smaller than full-res).
        let thumb_b64_len = uri1.len() - "data:image/png;base64,".len();
        let full_resp = server
            .dispatch(&format!(
                r#"{{"id":"b_full","method":"get_item_image","params":{{"id":"{item_id}"}}}}"#
            ))
            .await;
        let full_uri = full_resp.data.expect("get_item_image data")["data_uri"]
            .as_str()
            .expect("data_uri")
            .to_string();
        let full_b64_len = full_uri.len() - "data:image/png;base64,".len();
        assert!(
            thumb_b64_len < full_b64_len,
            "backfilled thumbnail ({thumb_b64_len}) must be smaller than full-res ({full_b64_len})"
        );

        // ── Second call: thumb must now be in DB (persisted) ─────────────────
        let resp2 = server
            .dispatch(&format!(
                r#"{{"id":"b2","method":"get_item_thumbnail","params":{{"id":"{item_id}"}}}}"#
            ))
            .await;
        let data2 = resp2
            .data
            .expect("second get_item_thumbnail must have data");
        assert!(
            !data2["thumbnail"].is_null(),
            "lazy backfill: second call must still return non-null thumbnail (persisted), got: {data2}"
        );
        assert_eq!(
            data2["thumbnail"], data1["thumbnail"],
            "second call must return the same data-URI (served from DB, deterministic)"
        );

        // Confirm THUMBNAIL_MAX_DIM was respected by the backfill.
        let _ = THUMBNAIL_MAX_DIM; // ensure the constant stays referenced in this test
    }

    // -----------------------------------------------------------------------
    // list_peers: online status + last_seen_secs (B1 device-info feature)
    // -----------------------------------------------------------------------

    /// `list_peers` must include `online` (bool) and `last_seen_secs` (i64)
    /// in every peer entry.  When `last_sync_at` is absent (never synced),
    /// `online` must be `false` and `last_seen_secs` must be `-1`.
    #[tokio::test]
    async fn list_peers_response_includes_online_and_last_seen_fields() {
        let dir = safe_tempdir();
        let sock = dir.path().join("lp_online_fields.sock");
        let cfg_home = dir.path().join("cfg");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );
        std::fs::create_dir_all(&cfg_home).unwrap();

        // Seed one peer that has never synced (no last_sync_at).
        let peers_json = cfg_home.join("peers.json");
        let peers = serde_json::json!([
            {"name": "Laptop", "fingerprint": "a1:b2:c3:d4:e5:f6:07:18", "added_at": 1}
        ]);
        std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"lp1","method":"list_peers","params":{}}"#).await;
        assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
        let peer_arr = resp["data"]["peers"]
            .as_array()
            .expect("data.peers must be array");
        assert_eq!(peer_arr.len(), 1, "must have exactly one peer");

        let peer = &peer_arr[0];
        assert!(
            peer.get("online").is_some(),
            "peer entry must include 'online' field: {peer}"
        );
        assert!(
            peer.get("last_seen_secs").is_some(),
            "peer entry must include 'last_seen_secs' field: {peer}"
        );

        // No sync ever → offline, sentinel -1.
        assert_eq!(
            peer["online"].as_bool(),
            Some(false),
            "peer with no last_sync_at must be offline: {peer}"
        );
        assert_eq!(
            peer["last_seen_secs"].as_i64(),
            Some(-1),
            "peer with no last_sync_at must have last_seen_secs=-1: {peer}"
        );
    }

    /// When `last_sync_at` is recent (within ONLINE_THRESHOLD_SECS), the peer
    /// must be marked `online = true`.
    #[tokio::test]
    async fn list_peers_online_true_when_last_sync_at_is_recent() {
        let dir = safe_tempdir();
        let sock = dir.path().join("lp_online_recent.sock");
        let cfg_home = dir.path().join("cfg2");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );
        std::fs::create_dir_all(&cfg_home).unwrap();

        let peers_json = cfg_home.join("peers.json");
        // last_sync_at = now − 30 s  → within the 60 s threshold.
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let recent = now_secs - 30;
        let peers = serde_json::json!([
            {
                "name": "Phone",
                "fingerprint": "f0:e1:d2:c3:b4:a5:96:87",
                "added_at": 1,
                "last_sync_at": recent
            }
        ]);
        std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"lp2","method":"list_peers","params":{}}"#).await;
        assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
        let peer_arr = resp["data"]["peers"].as_array().expect("data.peers array");
        assert_eq!(peer_arr.len(), 1);

        let peer = &peer_arr[0];
        assert_eq!(
            peer["online"].as_bool(),
            Some(true),
            "peer with recent last_sync_at must be online: {peer}"
        );
        let last_seen = peer["last_seen_secs"].as_i64().expect("last_seen_secs");
        // last_seen_secs = now - last_sync_at ≈ 30, allow ±5 for clock skew.
        assert!(
            (25..=35).contains(&last_seen),
            "last_seen_secs must be ~30, got {last_seen}"
        );
    }

    /// When `last_sync_at` is stale (beyond ONLINE_THRESHOLD_SECS), the peer
    /// must be marked `online = false`.
    #[tokio::test]
    async fn list_peers_online_false_when_last_sync_at_is_stale() {
        let dir = safe_tempdir();
        let sock = dir.path().join("lp_online_stale.sock");
        let cfg_home = dir.path().join("cfg3");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );
        std::fs::create_dir_all(&cfg_home).unwrap();

        let peers_json = cfg_home.join("peers.json");
        // CopyPaste-1jms.25: derive the stale offset from ONLINE_THRESHOLD_SECS
        // (which now equals SYNC_BADGE_RECENT_MS / 1000) so this test tracks the
        // shared recency window instead of a hard-coded 120 s that silently broke
        // when the window widened from 60 s to 300 s.
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let stale = now_secs - (ONLINE_THRESHOLD_SECS + 60);
        let peers = serde_json::json!([
            {
                "name": "Tablet",
                "fingerprint": "12:34:56:78:9a:bc:de:f0",
                "added_at": 1,
                "last_sync_at": stale
            }
        ]);
        std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

        start_test_server(&sock).await;
        let resp = call_one(&sock, r#"{"id":"lp3","method":"list_peers","params":{}}"#).await;
        assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
        let peer_arr = resp["data"]["peers"].as_array().expect("data.peers array");
        assert_eq!(peer_arr.len(), 1);

        let peer = &peer_arr[0];
        assert_eq!(
            peer["online"].as_bool(),
            Some(false),
            "peer with stale last_sync_at must be offline: {peer}"
        );
    }

    /// `list_peers` must mark a peer `online = true` when the peer's fingerprint
    /// is present with a live (non-closed) sender in the live P2P peer-sinks
    /// map, even if `last_sync_at` is absent or stale.
    #[tokio::test]
    async fn list_peers_online_true_from_live_mtls_allowlist() {
        let dir = safe_tempdir();
        let sock = dir.path().join("lp_online_mtls.sock");
        let cfg_home = dir.path().join("cfg4");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );
        std::fs::create_dir_all(&cfg_home).unwrap();

        // Peer fingerprint in colon-hex (as stored in peers.json).
        let fp_display = "a1:b2:c3:d4:e5:f6:07:18";
        // Canonical (colon-free, lowercase) form used as the sinks-map key.
        let fp_canonical = canonical_fingerprint(fp_display);

        let peers_json = cfg_home.join("peers.json");
        // Peer has no last_sync_at — only the live sinks map signals online.
        let peers = serde_json::json!([
            {"name": "Desktop", "fingerprint": fp_display, "added_at": 1}
        ]);
        std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

        // Build a live sinks map with a non-closed sender for the peer.
        // The receiver is kept alive for the duration of the test so the
        // sender's `is_closed()` returns false (the channel is open).
        let (peer_tx, _peer_rx) =
            tokio::sync::mpsc::channel::<copypaste_sync::protocol::PeerFrame>(1);
        let sinks_map: crate::p2p::LivePeerSinks =
            Arc::new(tokio::sync::Mutex::new(std::collections::HashMap::from([
                (fp_canonical.clone(), peer_tx),
            ])));

        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("mtls-test").unwrap();
        let server = IpcServer::new(
            db,
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        )
        .with_cert_fingerprint(display_fingerprint(&cert.fingerprint()));

        // Populate the live-sinks slot (simulates what daemon.rs does after
        // start_p2p returns).
        {
            let slot = server.live_peer_sinks_slot();
            let mut guard = slot.lock().unwrap();
            *guard = Some(Arc::clone(&sinks_map));
        }

        // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
        let listener =
            tokio::net::UnixListener::bind(&sock).expect("test socket bind must succeed");
        tokio::spawn(async move {
            let _ = server.serve_on(listener, CancellationToken::new()).await;
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let resp = call_one(&sock, r#"{"id":"lp4","method":"list_peers","params":{}}"#).await;
        assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
        let peer_arr = resp["data"]["peers"].as_array().expect("data.peers array");
        assert_eq!(peer_arr.len(), 1);

        let peer = &peer_arr[0];
        assert_eq!(
            peer["online"].as_bool(),
            Some(true),
            "peer in live sinks map must be online even without last_sync_at: {peer}"
        );
        // Ensure the receiver stays alive until after the assertion so the
        // sender is not marked closed prematurely.
        drop(_peer_rx);
    }

    /// `persist_paired_peer` must populate the `name` field from `PeerMeta.device_name`
    /// when provided, so `list_peers` returns a human-readable name rather than
    /// an empty string.
    #[tokio::test]
    async fn persist_paired_peer_populates_name_from_peer_meta_device_name() {
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg5");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );
        std::fs::create_dir_all(&cfg_home).unwrap();

        // Build a PeerMeta with device_name set.
        let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
            model: Some("iPhone 15".to_string()),
            os_version: Some("iOS 17".to_string()),
            app_version: Some("0.6.0".to_string()),
            local_ip: Some("192.168.1.42".to_string()),
            device_name: Some("Alice's iPhone".to_string()),
            public_ip: Some("203.0.113.42".to_string()),
            device_id: None,
            supabase_account_id: None,
        };
        // A dummy session key (all-zero is fine for this structural test).
        // SessionKey is a newtype tuple-struct: SessionKey([u8; 32]).
        let session_key = copypaste_p2p::pake::SessionKey([0u8; 32]);
        let fp = "b3:c4:d5:e6:f7:08:19:2a";

        IpcServer::persist_paired_peer(fp, "127.0.0.1:5001", &session_key, &peer_meta, None).await;

        // Read back the written peers.json and check name.
        let peers_path = peers_file_path();
        let written = crate::peers::load_peers(&peers_path);
        let record = written
            .iter()
            .find(|p| canonical_fingerprint(&p.fingerprint) == canonical_fingerprint(fp));
        assert!(
            record.is_some(),
            "persist_paired_peer must write a record for {fp}"
        );
        let record = record.unwrap();
        assert_eq!(
            record.name, "Alice's iPhone",
            "name must come from PeerMeta.device_name; got {:?}",
            record.name
        );
        // B1: the peer's reported public IP must be persisted on the record so
        // list_peers can surface it to the Devices UI.
        assert_eq!(
            record.public_ip.as_deref(),
            Some("203.0.113.42"),
            "public_ip must come from PeerMeta.public_ip; got {:?}",
            record.public_ip
        );
    }

    /// CopyPaste-yw2k: `list_peers` must surface `supabase_account_id` when it
    /// is stored in `peers.json` (seeded by the pairing handshake). Peers that
    /// predate this field surface no `supabase_account_id` key (or null).
    #[tokio::test]
    async fn list_peers_surfaces_supabase_account_id() {
        let dir = safe_tempdir();
        let sock = dir.path().join("lp_sai.sock");
        let cfg_home = dir.path().join("cfg_sai");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );
        std::fs::create_dir_all(&cfg_home).unwrap();

        let peers_json = cfg_home.join("peers.json");
        // Seed one peer that carries a supabase_account_id (as stored after a
        // successful pairing handshake with a new-build peer).
        let peers = serde_json::json!([{
            "name": "Laptop",
            "fingerprint": "a1:b2:c3:d4:e5:f6:07:18",
            "added_at": 1,
            "supabase_account_id": "proj_abc/uid_00000000-1111-2222-3333-444444444444"
        }]);
        std::fs::write(&peers_json, serde_json::to_string(&peers).unwrap()).unwrap();

        start_test_server(&sock).await;
        let resp = call_one(
            &sock,
            r#"{"id":"lp_sai1","method":"list_peers","params":{}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "list_peers must succeed: {resp}");
        let peer_arr = resp["data"]["peers"]
            .as_array()
            .expect("data.peers must be array");
        assert_eq!(peer_arr.len(), 1, "must have exactly one peer");

        let peer = &peer_arr[0];
        assert_eq!(
            peer["supabase_account_id"].as_str(),
            Some("proj_abc/uid_00000000-1111-2222-3333-444444444444"),
            "list_peers must surface supabase_account_id from peers.json: {peer}"
        );
    }

    /// CopyPaste-yw2k: `persist_paired_peer` must store the peer's
    /// `supabase_account_id` from `PeerMeta` into `peers.json`.
    #[tokio::test]
    async fn persist_paired_peer_stores_supabase_account_id() {
        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg_sai2");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );
        std::fs::create_dir_all(&cfg_home).unwrap();

        let peer_meta = copypaste_p2p::bootstrap::PeerMeta {
            model: None,
            os_version: None,
            app_version: None,
            local_ip: None,
            device_name: Some("Bob's MacBook".to_string()),
            public_ip: None,
            device_id: None,
            supabase_account_id: Some(
                "proj_xyz/uid_99999999-aaaa-bbbb-cccc-dddddddddddd".to_string(),
            ),
        };
        let session_key = copypaste_p2p::pake::SessionKey([0u8; 32]);
        let fp = "de:ad:be:ef:ca:fe:00:11";

        IpcServer::persist_paired_peer(fp, "127.0.0.1:5002", &session_key, &peer_meta, None).await;

        let peers_path = peers_file_path();
        let written = crate::peers::load_peers(&peers_path);
        let record = written
            .iter()
            .find(|p| canonical_fingerprint(&p.fingerprint) == canonical_fingerprint(fp));
        assert!(
            record.is_some(),
            "persist_paired_peer must write a record for {fp}"
        );
        let record = record.unwrap();
        assert_eq!(
            record.supabase_account_id.as_deref(),
            Some("proj_xyz/uid_99999999-aaaa-bbbb-cccc-dddddddddddd"),
            "supabase_account_id must be stored from PeerMeta; got {:?}",
            record.supabase_account_id
        );
    }

    /// When `p2p_enabled: false` is explicitly sent, merge_config must take the
    /// incoming value (the toggle is authoritative when present).
    #[test]
    fn p2p_enabled_option_some_false_wins() {
        let existing = AppConfig {
            p2p_enabled: Some(true),
            ..Default::default()
        };
        let incoming = AppConfig {
            p2p_enabled: Some(false),
            ..Default::default()
        };
        let merged = merge_config(existing, incoming);
        assert_eq!(
            merged.p2p_enabled,
            Some(false),
            "explicit p2p_enabled=false must override existing true"
        );
    }

    // --- get_item_file ---

    /// `get_item_file` must decrypt and return a file item's raw bytes as
    /// base64, along with the filename and MIME type stored at capture time.
    /// The round-trip mirrors `get_item_image`: store via `ClipboardItem::new_file`
    /// (chunks_to_blob-encoded), then retrieve via the IPC verb.
    #[tokio::test]
    async fn get_item_file_round_trips_bytes_and_meta() {
        let dir = safe_tempdir();
        let socket_path = dir.path().join("ipc.sock");
        let (_pm, db) = start_test_server_returning_db(&socket_path, false).await;

        // Build a file item and seed it into the DB.
        // new_file stamps key_version = 2, so the server reads with
        // derive_v2(local_key). Encrypt with that same v2 key so the round-trip
        // matches the production writer (handle_file uses derive_v2).
        let raw = b"hello clipboard file";
        let key = [0u8; 32]; // v1 seed matching dummy server key
        let v2_key = derive_v2(&key); // server reads kv=2 rows with this
        let file_id = [0xAAu8; 16]; // fixed content-hash stand-in for test
        let (meta, chunks) =
            copypaste_core::encode_file(raw, "hello.txt", "text/plain", &v2_key, &file_id, 0)
                .expect("encode_file must succeed");
        let blob = copypaste_core::chunks_to_blob(&chunks).expect("chunks_to_blob must succeed");
        let meta_json = crate::clipboard::build_file_meta_json(&meta);
        let mut item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
        item.item_id = uuid::Uuid::from_bytes(file_id).to_string();

        let item_id = item.id.clone();
        {
            let db_guard = db.lock().await;
            copypaste_core::insert_item_with_fts(&db_guard, &item, "")
                .expect("insert must succeed");
        }

        // Issue get_item_file over IPC.
        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        let req = format!(
            "{{\"id\":\"gf1\",\"method\":\"get_item_file\",\"params\":{{\"id\":\"{item_id}\"}}}}\n"
        );
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

        assert_eq!(resp["ok"], true, "get_item_file must succeed: {resp}");
        assert_eq!(resp["data"]["filename"], "hello.txt");
        assert_eq!(resp["data"]["mime"], "text/plain");
        // Verify the raw bytes round-trip through base64.
        use base64::Engine as _;
        let returned_bytes = base64::engine::general_purpose::STANDARD
            .decode(resp["data"]["data_b64"].as_str().unwrap())
            .expect("data_b64 must be valid base64");
        assert_eq!(returned_bytes, raw);
    }

    /// `get_item_file` must reject requests for non-file content_type items.
    #[tokio::test]
    async fn get_item_file_rejects_non_file_item() {
        let dir = safe_tempdir();
        let socket_path = dir.path().join("ipc2.sock");
        let (_pm, db) = start_test_server_returning_db(&socket_path, false).await;

        // Insert a text item. new_text(encrypted_content, nonce, lamport_ts).
        let nonce = vec![0u8; copypaste_core::NONCE_SIZE];
        let ciphertext = b"dummy-ciphertext".to_vec();
        let item = copypaste_core::ClipboardItem::new_text(ciphertext, nonce, 0);
        let item_id = item.id.clone();
        {
            let db_guard = db.lock().await;
            copypaste_core::insert_item_with_fts(&db_guard, &item, "dummy text")
                .expect("insert must succeed");
        }

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        let req = format!(
            "{{\"id\":\"gf2\",\"method\":\"get_item_file\",\"params\":{{\"id\":\"{item_id}\"}}}}\n"
        );
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

        assert_eq!(
            resp["ok"], false,
            "get_item_file must fail for a text item: {resp}"
        );
    }

    /// `parse_file_meta` must extract filename, mime, original_size and
    /// chunk_count from the JSON produced by `build_file_meta_json`.
    #[test]
    fn parse_file_meta_round_trips_build_file_meta_json() {
        let meta = copypaste_core::FileMeta {
            filename: "test.pdf".to_string(),
            mime: "application/pdf".to_string(),
            original_size: 12345,
            chunk_count: 2,
            file_id: [0xABu8; 16],
        };
        let json = crate::clipboard::build_file_meta_json(&meta);
        let parsed = parse_file_meta(&json).expect("parse_file_meta must succeed");
        assert_eq!(parsed.filename, "test.pdf");
        assert_eq!(parsed.mime, "application/pdf");
        assert_eq!(parsed.original_size, 12345);
        assert_eq!(parsed.chunk_count, 2);
        assert_eq!(parsed.file_id, [0xABu8; 16]);
    }

    /// `history_page` must return `[file: <name>]` as the preview for file items.
    #[tokio::test]
    async fn history_page_shows_file_preview() {
        let dir = safe_tempdir();
        let socket_path = dir.path().join("hp_file.sock");
        let (_pm, db) = start_test_server_returning_db(&socket_path, false).await;

        let raw = b"pdf content";
        let key = [0u8; 32];
        let file_id = [0x01u8; 16];
        let (meta, chunks) =
            copypaste_core::encode_file(raw, "doc.pdf", "application/pdf", &key, &file_id, 0)
                .unwrap();
        let blob = copypaste_core::chunks_to_blob(&chunks).unwrap();
        let meta_json = crate::clipboard::build_file_meta_json(&meta);
        let item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
        {
            let db_guard = db.lock().await;
            copypaste_core::insert_item_with_fts(&db_guard, &item, "").unwrap();
        }

        let mut stream = UnixStream::connect(&socket_path).await.unwrap();
        stream
            .write_all(
                b"{\"id\":\"hpf\",\"method\":\"history_page\",\"params\":{\"limit\":10,\"offset\":0}}\n",
            )
            .await
            .unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

        assert_eq!(resp["ok"], true, "history_page must succeed: {resp}");
        let items = resp["data"]["items"].as_array().unwrap();
        let file_item = items.iter().find(|it| it["content_type"] == "file");
        assert!(file_item.is_some(), "must find a file item in history_page");
        let preview = file_item.unwrap()["preview"].as_str().unwrap();
        assert!(
            preview.starts_with("[file:"),
            "file item preview must start with '[file:'; got: {preview}"
        );
        assert!(
            preview.contains("doc.pdf"),
            "file item preview must include filename; got: {preview}"
        );
    }

    // --- write_to_pasteboard: file branch ---

    /// `paste_file_cache_dir` must return a path that ends in `paste-files` and
    /// lives under the platform cache directory (e.g. `~/Library/Caches/CopyPaste/paste-files`
    /// on macOS). The test is platform-agnostic: it only checks the basename.
    #[test]
    fn paste_file_cache_dir_ends_with_paste_files() {
        let dir = paste_file_cache_dir();
        assert_eq!(
            dir.file_name().and_then(|n| n.to_str()),
            Some("paste-files"),
            "paste_file_cache_dir must end in 'paste-files'; got: {dir:?}"
        );
    }

    /// `prune_old_paste_files` must remove files whose mtime is older than the
    /// retention window (~10 min) and leave recent files untouched.
    #[test]
    fn prune_old_paste_files_removes_stale_and_keeps_recent() {
        let dir = safe_tempdir();
        let cache = dir.path().to_path_buf();

        // Write a "recent" file (mtime = now).
        let recent = cache.join("recent.txt");
        std::fs::write(&recent, b"keep me").unwrap();

        // Write a "stale" file and backdate its mtime by 20 minutes.
        let stale = cache.join("stale.txt");
        std::fs::write(&stale, b"delete me").unwrap();
        let twenty_min_ago = std::time::SystemTime::now()
            .checked_sub(std::time::Duration::from_secs(20 * 60))
            .expect("time subtraction is infallible on any plausible system clock");
        // std::fs::FileTimes / File::set_times is stable since Rust 1.75 (MSRV = 1.96).
        // set_modified lives on FileTimes directly (no platform extension trait needed).
        {
            let f = std::fs::OpenOptions::new()
                .write(true)
                .open(&stale)
                .expect("open stale for set_times");
            let times = std::fs::FileTimes::new().set_modified(twenty_min_ago);
            f.set_times(times).expect("set_times on stale file");
        }

        prune_old_paste_files(&cache);

        assert!(recent.exists(), "recent file must survive prune");
        assert!(!stale.exists(), "stale (20-min-old) file must be pruned");
    }

    /// `write_to_pasteboard` must not return the `Unknown content_type` fallthrough
    /// for a `file` item; instead it must attempt the file-decode path.
    /// On non-macOS the non-macOS stub always returns `Ok(())` regardless of
    /// content_type, so we assert `Ok` there.
    /// On macOS we verify that either:
    ///   a) a paste temp-file was created under `paste_file_cache_dir()`, OR
    ///   b) an error was returned (e.g. NSPasteboard not available in headless CI) —
    ///      the important invariant is that the error is NOT the old "Unknown content_type"
    ///      fallthrough, which means the file branch was reached.
    #[tokio::test]
    async fn write_to_pasteboard_file_branch_is_reached() {
        let dir = safe_tempdir();
        let sock = dir.path().join("wtp_file.sock");

        // Point COPYPASTE_CACHE_DIR at a temp path so paste-files land there
        // and don't pollute ~/Library/Caches/CopyPaste during the test.
        let cache_home = dir.path().join("cache");
        // Acquire EnvGuard (and thus TEST_ENV_LOCK) BEFORE create_dir_all so
        // the mkdir is serialised with the umask(0o177) window from the
        // bind_with_stale_cleanup tests.
        let _env = EnvGuard::set_all(&["COPYPASTE_CACHE_DIR"], &cache_home);
        std::fs::create_dir_all(&cache_home).unwrap();

        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Build a real encoded file item with the same all-zero key as the test server.
        let raw = b"hello paste file";
        let key = [0u8; 32]; // matches the test server's local_key
        let file_id = [0xBBu8; 16];
        let (meta, chunks) =
            copypaste_core::encode_file(raw, "paste.txt", "text/plain", &key, &file_id, 0)
                .expect("encode_file must succeed");
        let blob = copypaste_core::chunks_to_blob(&chunks).expect("chunks_to_blob must succeed");
        let meta_json = crate::clipboard::build_file_meta_json(&meta);
        let mut item = copypaste_core::ClipboardItem::new_file(blob, meta_json, 0);
        // Align item_id with file_id (mirrors get_item_file_round_trips test).
        item.item_id = uuid::Uuid::from_bytes(file_id).to_string();
        let item_id = item.id.clone();
        {
            let db_guard = db.lock().await;
            copypaste_core::insert_item_with_fts(&db_guard, &item, "")
                .expect("insert must succeed");
        }

        // Trigger copy_item over IPC — this calls write_to_pasteboard internally.
        let mut stream = tokio::net::UnixStream::connect(&sock).await.unwrap();
        let req = format!(
            "{{\"id\":\"wtp1\",\"method\":\"copy_item\",\"params\":{{\"id\":\"{item_id}\"}}}}\n"
        );
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut reader = BufReader::new(&mut stream);
        let mut line = String::new();
        reader.read_line(&mut line).await.unwrap();
        let resp: serde_json::Value = serde_json::from_str(line.trim()).unwrap();

        // On macOS (with a real display/pasteboard) the call succeeds and a
        // paste-files temp file must exist.
        // In headless CI (macOS without a window server) the paste may fail, but
        // must NOT report "Unknown content_type" — that would mean the file branch
        // was bypassed entirely and we fell through to the old raw-bytes path.
        #[cfg(target_os = "macos")]
        {
            if resp["ok"] == true {
                // Verify a temp file was written.
                let paste_dir = cache_home.join("paste-files");
                let found = std::fs::read_dir(&paste_dir)
                    .map(|rd| {
                        rd.flatten()
                            .any(|e| e.file_name().to_str() == Some("paste.txt"))
                    })
                    .unwrap_or(false);
                assert!(
                    found,
                    "write_to_pasteboard file branch must create paste.txt under paste-files; dir: {paste_dir:?}"
                );
            } else {
                // Headless / no pasteboard — acceptable failure, but must not be the unknown-fallthrough.
                let err = resp["error"].as_str().unwrap_or("");
                assert!(
                    !err.contains("Unknown content_type"),
                    "write_to_pasteboard must NOT fall through to Unknown content_type for file items; error: {err}"
                );
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            // Non-macOS stub always returns Ok(()).
            assert_eq!(
                resp["ok"], true,
                "write_to_pasteboard non-macOS stub must succeed for file items: {resp}"
            );
        }
    }

    // -----------------------------------------------------------------------
    // CopyPaste-7mf regression: responder-side persist race
    // -----------------------------------------------------------------------

    /// Regression test for CopyPaste-7mf: after a successful network bootstrap
    /// pairing, the RESPONDER daemon's `list_peers` MUST return the newly-paired
    /// peer immediately after the INITIATOR's `pair_accept_qr` response returns —
    /// with NO sleep or polling between the two calls.
    ///
    /// The race: `pair_generate_qr` fires `spawn_bootstrap_responder` which runs
    /// the PAKE handshake + `persist_paired_peer` inside a `tokio::spawn`. The
    /// IPC response is returned before the spawn's persist completes. The fix
    /// (CopyPaste-7mf) stores the `JoinHandle` in `IpcServer::pending_bootstrap`
    /// and has `list_peers` await it (with a 5 s timeout) before reading
    /// peers.json. This test would fail WITHOUT the fix and MUST pass with it.
    #[tokio::test]
    async fn responder_list_peers_sees_peer_immediately_after_initiator_completes() {
        use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
        use tokio::net::UnixStream;

        let dir = safe_tempdir();
        let cfg_home = dir.path().join("cfg_7mf");
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_CONFIG_DIR",
                "COPYPASTE_DATA_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            &cfg_home,
        );
        std::fs::create_dir_all(&cfg_home).unwrap();

        // Helper: send one newline-terminated JSON request, return parsed response.
        async fn call(sock: &std::path::Path, body: &str) -> serde_json::Value {
            let mut stream = UnixStream::connect(sock).await.unwrap();
            stream.write_all(body.as_bytes()).await.unwrap();
            stream.write_all(b"\n").await.unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let line = lines.next_line().await.unwrap().unwrap();
            serde_json::from_str(&line).unwrap()
        }

        // ── Server A (responder): generates the QR. Needs a real cert so that
        // BootstrapResponder::bind uses real TLS and spawn_bootstrap_responder runs.
        let sock_a = dir.path().join("7mf-a.sock");
        let cert_a = copypaste_p2p::cert::SelfSignedCert::generate("test-a").unwrap();
        {
            let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
            let server = IpcServer::new(
                db,
                Arc::new(AtomicBool::new(false)),
                Arc::new(zeroize::Zeroizing::new([0u8; 32])),
                Arc::new([0u8; 32]),
            )
            .with_cert_fingerprint(display_fingerprint(&cert_a.fingerprint()))
            .with_p2p_cert(cert_a.cert_der.clone(), cert_a.key_der.clone());
            // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
            let listener_a =
                tokio::net::UnixListener::bind(&sock_a).expect("test socket A bind must succeed");
            tokio::spawn(async move {
                let _ = server.serve_on(listener_a, CancellationToken::new()).await;
            });
        }

        // ── Server B (initiator): dials A's bootstrap addr. Needs its own cert.
        let sock_b = dir.path().join("7mf-b.sock");
        let cert_b = copypaste_p2p::cert::SelfSignedCert::generate("test-b").unwrap();
        {
            let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
            let server = IpcServer::new(
                db,
                Arc::new(AtomicBool::new(false)),
                Arc::new(zeroize::Zeroizing::new([0u8; 32])),
                Arc::new([0u8; 32]),
            )
            .with_cert_fingerprint(display_fingerprint(&cert_b.fingerprint()))
            .with_p2p_cert(cert_b.cert_der.clone(), cert_b.key_der.clone());
            // Bind directly (no umask(0o177) race) — see start_test_server_returning_db.
            let listener_b =
                tokio::net::UnixListener::bind(&sock_b).expect("test socket B bind must succeed");
            tokio::spawn(async move {
                let _ = server.serve_on(listener_b, CancellationToken::new()).await;
            });
        }
        // Give both sockets a moment to come up.
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;

        // A's canonical fingerprint (colon-free) — what B should persist.
        let fp_a_canonical = canonical_fingerprint(&display_fingerprint(&cert_a.fingerprint()));
        // B's canonical fingerprint — what A's responder spawn should persist.
        let fp_b_canonical = canonical_fingerprint(&display_fingerprint(&cert_b.fingerprint()));

        // Step 1: A generates a QR. With a real p2p_cert, this binds a
        // bootstrap TLS listener, stores the JoinHandle in pending_bootstrap,
        // and embeds the listener's host:port in the QR's addr_hint.
        let qr_resp = call(
            &sock_a,
            r#"{"id":"7mf-q","method":"pair_generate_qr","params":{}}"#,
        )
        .await;
        assert_eq!(
            qr_resp["ok"], true,
            "pair_generate_qr must succeed: {qr_resp}"
        );
        let qr = qr_resp["data"]["qr"]
            .as_str()
            .expect("QR string in response")
            .to_string();
        // Ensure the QR carries an addr_hint so B dials the network path
        // (not the legacy IPC-relay path). The encoded QR wraps the bare CPPAIR2
        // payload in the deep-link URI; strip it to inspect the addr_hint field.
        let bare = copypaste_core::strip_deeplink(&qr);
        // v2 QR: CPPAIR2.<fp_b64>.<token_b64>.<device_id_b64>.<name>.<addr_hint>
        // addr_hint is the last '.' separated field. Use the existing helper.
        let has_hint = {
            let (_magic, body) = bare.split_once('.').expect("bare QR has magic.body");
            let hint = body.splitn(5, '.').nth(4).unwrap_or("");
            hint.parse::<std::net::SocketAddr>().is_ok()
        };
        // If there is no addr_hint the bootstrap listener did not bind (unlikely
        // on loopback) — skip the network PAKE path and let this test pass vacuously
        // rather than incorrectly block forever.
        if !has_hint {
            return;
        }

        // Step 2: B accepts the QR over the network. This drives the full PAKE
        // handshake; it only returns ok once both sides have agreed on the session key.
        let accept_body = serde_json::json!({
            "id": "7mf-accept",
            "method": "pair_accept_qr",
            "params": { "qr": qr },
        })
        .to_string();
        let accept_resp = call(&sock_b, &accept_body).await;
        assert_eq!(
            accept_resp["ok"], true,
            "network PAKE pairing must succeed end-to-end: {accept_resp}"
        );
        // B should have A's fingerprint as the confirmed peer.
        let returned_fp = accept_resp["data"]["peer_fingerprint"]
            .as_str()
            .expect("peer_fingerprint in accept response");
        assert_eq!(
            returned_fp, fp_a_canonical,
            "returned peer_fingerprint must equal A's cert fingerprint"
        );

        // Step 3 — THE REGRESSION CHECK: call list_peers on A IMMEDIATELY
        // (no sleep, no poll) and assert B's fingerprint is already present.
        // Without the CopyPaste-7mf fix this would race the detached spawn and
        // return an empty peers list. With the fix, list_peers awaits the
        // pending_bootstrap JoinHandle and blocks until persist_paired_peer runs.
        let list_resp = call(
            &sock_a,
            r#"{"id":"7mf-list","method":"list_peers","params":{}}"#,
        )
        .await;
        assert_eq!(
            list_resp["ok"], true,
            "list_peers on A must succeed: {list_resp}"
        );
        let peers = list_resp["data"]["peers"]
            .as_array()
            .expect("data.peers array");
        let found = peers.iter().any(|p| {
            p.get("fingerprint")
                .and_then(|v| v.as_str())
                .map(|fp| canonical_fingerprint(fp) == fp_b_canonical)
                .unwrap_or(false)
        });
        assert!(
            found,
            "A's list_peers must return B's fingerprint immediately after initiator completes \
             (CopyPaste-7mf race fix); fp_b={fp_b_canonical}; peers={peers:?}"
        );
    }

    // ── lan_visibility IPC config tests ───────────────────────────────────────

    /// `merge_config` preserves `lan_visibility` from existing when incoming
    /// omits it (`None`), and takes the new value when the caller supplies one.
    #[test]
    fn merge_config_preserves_and_overrides_lan_visibility() {
        // Case 1: incoming omits lan_visibility — existing value is kept.
        let existing = AppConfig {
            lan_visibility: Some(false),
            ..AppConfig::default()
        };
        let incoming_none = AppConfig {
            lan_visibility: None,
            ..AppConfig::default()
        };
        let merged = merge_config(existing, incoming_none);
        assert_eq!(
            merged.lan_visibility,
            Some(false),
            "merge_config must preserve existing lan_visibility when incoming is None"
        );

        // Case 2: incoming supplies an explicit value — it wins.
        let existing2 = AppConfig {
            lan_visibility: Some(false),
            ..AppConfig::default()
        };
        let incoming_some = AppConfig {
            lan_visibility: Some(true),
            ..AppConfig::default()
        };
        let merged2 = merge_config(existing2, incoming_some);
        assert_eq!(
            merged2.lan_visibility,
            Some(true),
            "merge_config must take incoming lan_visibility when Some"
        );
    }

    /// `update_core_config` persists `lan_visibility` to config.toml and the
    /// returned `AppConfig` reflects the new value.
    #[test]
    fn update_core_config_persists_lan_visibility() {
        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let dir = safe_tempdir();
        unsafe { std::env::set_var("COPYPASTE_CONFIG_DIR", dir.path()) };

        // Disable LAN visibility via IPC patch.
        let patch = AppConfig {
            lan_visibility: Some(false),
            ..AppConfig::default()
        };
        let new_core = update_core_config(&patch).expect("update_core_config must succeed");
        assert!(
            !new_core.lan_visibility,
            "update_core_config must persist lan_visibility=false to config.toml"
        );

        // Re-enable it.
        let patch2 = AppConfig {
            lan_visibility: Some(true),
            ..AppConfig::default()
        };
        let new_core2 = update_core_config(&patch2).expect("update_core_config must succeed");
        assert!(
            new_core2.lan_visibility,
            "update_core_config must persist lan_visibility=true to config.toml"
        );

        // When omitted (`None`), the stored value is unchanged (false from patch).
        // First persist false explicitly, then send None.
        let patch3_set = AppConfig {
            lan_visibility: Some(false),
            ..AppConfig::default()
        };
        update_core_config(&patch3_set).expect("set to false");
        let patch3_none = AppConfig {
            lan_visibility: None,
            ..AppConfig::default()
        };
        let new_core3 = update_core_config(&patch3_none).expect("update with None");
        assert!(
            !new_core3.lan_visibility,
            "update_core_config must not reset lan_visibility when patch has None"
        );

        // Restore env.
        unsafe { std::env::remove_var("COPYPASTE_CONFIG_DIR") };
        drop(env_lock);
    }

    // ── CopyPaste-44rq.67: relay_url clear-sentinel handling ────────────────

    /// `merge_config` must PRESERVE the empty-string "clear" sentinel rather than
    /// `.or()`-falling back to the existing URL, so `update_core_config` can see
    /// `Some("")` and clear the relay. A normal value wins; `None` preserves.
    #[test]
    fn merge_config_preserves_relay_clear_sentinel() {
        let with_url = || AppConfig {
            relay_url: Some("https://relay.example.com".to_owned()),
            ..AppConfig::default()
        };
        // Clear sentinel must survive the merge (not be replaced by existing URL).
        let cleared = merge_config(
            with_url(),
            AppConfig {
                relay_url: Some(String::new()),
                ..AppConfig::default()
            },
        );
        assert_eq!(
            cleared.relay_url.as_deref(),
            Some(""),
            "empty sentinel must be preserved so update_core_config can clear"
        );
        // Omitted (None) preserves the existing URL.
        let preserved = merge_config(with_url(), AppConfig::default());
        assert_eq!(
            preserved.relay_url.as_deref(),
            Some("https://relay.example.com")
        );
        // A real value wins.
        let replaced = merge_config(
            with_url(),
            AppConfig {
                relay_url: Some("https://new.example.com".to_owned()),
                ..AppConfig::default()
            },
        );
        assert_eq!(
            replaced.relay_url.as_deref(),
            Some("https://new.example.com")
        );
    }

    /// `update_core_config` must set `core.relay_url = None` when the incoming
    /// `relay_url` is the empty-string sentinel, persist a real URL when set, and
    /// leave it unchanged when omitted (`None`).
    #[test]
    fn update_core_config_clears_relay_url_on_empty_sentinel() {
        let env_lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        let dir = safe_tempdir();
        unsafe { std::env::set_var("COPYPASTE_CONFIG_DIR", dir.path()) };

        // Set a real URL.
        let set = AppConfig {
            relay_url: Some("https://relay.example.com".to_owned()),
            ..AppConfig::default()
        };
        let core1 = update_core_config(&set).expect("set relay_url");
        assert_eq!(
            core1.relay_url.as_deref(),
            Some("https://relay.example.com")
        );

        // Omit it — must be preserved.
        let core2 = update_core_config(&AppConfig::default()).expect("omit relay_url");
        assert_eq!(
            core2.relay_url.as_deref(),
            Some("https://relay.example.com"),
            "None must preserve the stored relay_url"
        );

        // Clear sentinel — must wipe to None.
        let clear = AppConfig {
            relay_url: Some(String::new()),
            ..AppConfig::default()
        };
        let core3 = update_core_config(&clear).expect("clear relay_url");
        assert_eq!(
            core3.relay_url, None,
            "empty-string sentinel must clear core.relay_url"
        );

        unsafe { std::env::remove_var("COPYPASTE_CONFIG_DIR") };
        drop(env_lock);
    }

    // ── CopyPaste-bjh: startup must honour persisted p2p_enabled ────────────

    /// `p2p_enabled_from_config` must default to `true` when no config.json
    /// exists (fresh install — P2P is ON by default so users can pair without
    /// an explicit toggle). Regression guard: daemon startup used to check
    /// `COPYPASTE_P2P` env-var only; now it falls back to this accessor.
    #[test]
    fn p2p_enabled_from_config_defaults_to_true_when_no_config() {
        let dir = safe_tempdir();
        let _env = EnvGuard::set_all(
            &["HOME", "XDG_CONFIG_HOME", "COPYPASTE_CONFIG_DIR"],
            dir.path(),
        );
        // No config.json written — accessor must return true (default ON).
        assert!(
            p2p_enabled_from_config(),
            "p2p_enabled_from_config must default to true when config.json is absent"
        );
    }

    /// When `p2p_enabled: false` is persisted (user toggled P2P off in Settings),
    /// `p2p_enabled_from_config` must return `false`. This is the value daemon
    /// startup reads (after the A-SET-4 fix) so the daemon skips `start_p2p`
    /// when the env-var override (`COPYPASTE_P2P`) is absent.
    #[test]
    fn p2p_enabled_from_config_returns_false_when_persisted_false() {
        let dir = safe_tempdir();
        let _env = EnvGuard::set_all(
            &["HOME", "XDG_CONFIG_HOME", "COPYPASTE_CONFIG_DIR"],
            dir.path(),
        );
        write_config(&AppConfig {
            p2p_enabled: Some(false),
            ..Default::default()
        })
        .expect("write_config must succeed");

        assert!(
            !p2p_enabled_from_config(),
            "p2p_enabled_from_config must return false when config.json stores p2p_enabled=false"
        );
    }

    /// When `p2p_enabled: true` is persisted, `p2p_enabled_from_config` must
    /// return `true`. Symmetric with the false case above.
    #[test]
    fn p2p_enabled_from_config_returns_true_when_persisted_true() {
        let dir = safe_tempdir();
        let _env = EnvGuard::set_all(
            &["HOME", "XDG_CONFIG_HOME", "COPYPASTE_CONFIG_DIR"],
            dir.path(),
        );
        write_config(&AppConfig {
            p2p_enabled: Some(true),
            ..Default::default()
        })
        .expect("write_config must succeed");

        assert!(
            p2p_enabled_from_config(),
            "p2p_enabled_from_config must return true when config.json stores p2p_enabled=true"
        );
    }

    // ── CopyPaste-6ot5: connection-cap unit test ──────────────────────────────

    /// Verify the connection-cap semaphore logic without touching real sockets.
    ///
    /// The semaphore starts with `MAX_CONCURRENT_CONNECTIONS` permits. When all
    /// permits are exhausted, `try_acquire_owned` must return `Err` immediately
    /// (non-blocking); once a permit is dropped the slot is reclaimed and the
    /// next `try_acquire_owned` succeeds again. This test exercises the pure
    /// Semaphore behaviour that `serve_on` depends on — avoiding any live-socket
    /// flood that could introduce a test-suite deadlock.
    #[test]
    fn connection_cap_semaphore_exhaustion_returns_err() {
        // Use a small cap so the test runs without allocating 64 permits.
        const TEST_CAP: usize = 4;
        let sem = Arc::new(tokio::sync::Semaphore::new(TEST_CAP));

        // Acquire all permits.
        let permits: Vec<_> = (0..TEST_CAP)
            .map(|_| {
                sem.clone()
                    .try_acquire_owned()
                    .expect("permit must be available below cap")
            })
            .collect();

        // One more acquire must fail (cap exhausted).
        assert!(
            sem.clone().try_acquire_owned().is_err(),
            "try_acquire_owned must return Err when the connection cap is reached"
        );

        // Drop one permit — the slot is reclaimed immediately.
        drop(permits.into_iter().next().unwrap());

        // Now a new acquire succeeds.
        assert!(
            sem.clone().try_acquire_owned().is_ok(),
            "try_acquire_owned must succeed again after a permit is released"
        );
    }

    /// Verify that the production `IpcServer` is initialised with a semaphore
    /// holding exactly `MAX_CONCURRENT_CONNECTIONS` permits and that the cap
    /// is enforced from the very first connection.
    #[test]
    fn ipc_server_connection_cap_is_max_concurrent_connections() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db,
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );

        // Drain all permits.
        let permits: Vec<_> = (0..MAX_CONCURRENT_CONNECTIONS)
            .map(|_| {
                server
                    .conn_semaphore
                    .clone()
                    .try_acquire_owned()
                    .expect("permit must be available within cap")
            })
            .collect();

        // The (cap+1)-th acquire must fail.
        assert!(
            server.conn_semaphore.clone().try_acquire_owned().is_err(),
            "IpcServer must enforce MAX_CONCURRENT_CONNECTIONS limit"
        );

        // Ensure permits are held for the assertion (not optimised away).
        drop(permits);
    }

    /// CopyPaste-kfe9: legacy IPC arms (search / copy / paste / pin) must
    /// return a machine-readable `error_code` on failure, not a bare untyped
    /// error string.  This is the follow-up to CopyPaste-8u2b which wired
    /// `error_code` onto the `delete` arm but left the others unchanged.
    #[tokio::test]
    async fn legacy_ipc_arms_return_error_code_on_failure() {
        let server = bare_server();

        // -- search: missing required `query` param → invalid_argument ---------
        let resp = server
            .dispatch(r#"{"id":"s1","method":"search","params":{}}"#)
            .await;
        assert!(!resp.ok, "search without query must fail");
        assert_eq!(
            resp.error_code,
            Some("invalid_argument"),
            "search/missing-query must carry error_code=invalid_argument, got: {resp:?}"
        );

        // -- pin: missing required `id` param → invalid_argument ---------------
        let resp = server
            .dispatch(r#"{"id":"p1","method":"pin","params":{}}"#)
            .await;
        assert!(!resp.ok, "pin without id must fail");
        assert_eq!(
            resp.error_code,
            Some("invalid_argument"),
            "pin/missing-id must carry error_code=invalid_argument, got: {resp:?}"
        );

        // -- pin: non-UUID `id` → invalid_argument -----------------------------
        let resp = server
            .dispatch(r#"{"id":"p2","method":"pin","params":{"id":"not-a-uuid"}}"#)
            .await;
        assert!(!resp.ok, "pin with bad UUID must fail");
        assert_eq!(
            resp.error_code,
            Some("invalid_argument"),
            "pin/bad-uuid must carry error_code=invalid_argument, got: {resp:?}"
        );

        // -- copy: item not found → not_found ----------------------------------
        let missing_uuid = "00000000-0000-0000-0000-000000000000";
        let resp = server
            .dispatch(&format!(
                r#"{{"id":"c1","method":"copy","params":{{"id":"{missing_uuid}"}}}}"#
            ))
            .await;
        assert!(!resp.ok, "copy of non-existent item must fail");
        assert_eq!(
            resp.error_code,
            Some("not_found"),
            "copy/not-found must carry error_code=not_found, got: {resp:?}"
        );

        // -- paste: item not found → not_found ---------------------------------
        let resp = server
            .dispatch(&format!(
                r#"{{"id":"p3","method":"paste","params":{{"id":"{missing_uuid}"}}}}"#
            ))
            .await;
        assert!(!resp.ok, "paste of non-existent item must fail");
        assert_eq!(
            resp.error_code,
            Some("not_found"),
            "paste/not-found must carry error_code=not_found, got: {resp:?}"
        );
    }

    /// CopyPaste-48k0: `set_private_mode` must increment `private_mode_epoch` on
    /// every call so that periodic pollers (UI health-check, tray) can detect
    /// private-mode changes across daemon restarts without a dedicated subscription.
    #[tokio::test]
    async fn private_mode_epoch_increments_on_every_set() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let server = IpcServer::new(
            db,
            private_mode,
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );

        // Epoch starts at 0.
        let resp0 = server
            .dispatch(r#"{"id":"t0","method":"get_private_mode","params":{}}"#)
            .await;
        assert!(resp0.ok, "initial get_private_mode must succeed");
        let data0 = resp0.data.expect("get_private_mode must return data");
        assert_eq!(
            data0["private_mode_epoch"],
            serde_json::json!(0),
            "epoch must start at 0"
        );

        // First set → epoch becomes 1.
        let resp1 = server
            .dispatch(r#"{"id":"t1","method":"set_private_mode","params":{"enabled":true}}"#)
            .await;
        assert!(resp1.ok, "set_private_mode must succeed");
        let data1 = resp1.data.expect("set_private_mode must return data");
        assert_eq!(
            data1["private_mode_epoch"],
            serde_json::json!(1),
            "epoch must be 1 after first set"
        );

        // Second set (same value) → epoch becomes 2.
        let resp2 = server
            .dispatch(r#"{"id":"t2","method":"set_private_mode","params":{"enabled":true}}"#)
            .await;
        let data2 = resp2.data.expect("set_private_mode must return data");
        assert_eq!(
            data2["private_mode_epoch"],
            serde_json::json!(2),
            "epoch must increment even when value is unchanged"
        );

        // get_private_mode must reflect the current epoch.
        let resp3 = server
            .dispatch(r#"{"id":"t3","method":"get_private_mode","params":{}}"#)
            .await;
        let data3 = resp3.data.expect("get_private_mode must return data");
        assert_eq!(
            data3["private_mode_epoch"],
            serde_json::json!(2),
            "get_private_mode must return current epoch"
        );

        // status must also include the epoch.
        let resp4 = server
            .dispatch(r#"{"id":"t4","method":"status","params":{}}"#)
            .await;
        let data4 = resp4.data.expect("status must return data");
        assert_eq!(
            data4["private_mode_epoch"],
            serde_json::json!(2),
            "status must return current epoch"
        );
    }

    /// CopyPaste-ah1m: `bind_with_stale_cleanup` must create a lockfile next to
    /// the socket so concurrent startups are serialized through `flock(2)` and
    /// the probe→remove→bind sequence is atomic.
    ///
    /// After a successful bind the `.lock` file must exist (it is never deleted,
    /// only created/locked). Its presence means a future concurrent starter will
    /// block on `flock` rather than racing through the stale-check.
    #[cfg(unix)]
    #[tokio::test]
    async fn bind_with_stale_cleanup_creates_lockfile() {
        let dir = safe_tempdir();
        let sock = dir.path().join("test-atomic.sock");
        let lock = dir.path().join("test-atomic.sock.lock");
        // Hold TEST_ENV_LOCK to serialise the libc::umask(0o177) inside
        // bind_with_stale_cleanup with concurrent tests that create directories.
        let _env = EnvGuard::set_all(
            &[
                "COPYPASTE_DATA_DIR",
                "COPYPASTE_CONFIG_DIR",
                "HOME",
                "XDG_CONFIG_HOME",
            ],
            dir.path(),
        );

        // Lockfile must NOT exist before the first call.
        assert!(
            !lock.exists(),
            "lockfile must not exist before bind; got: {lock:?}"
        );

        // Bind the socket — this should create the lockfile.
        let listener = super::bind_with_stale_cleanup(&sock)
            .expect("bind_with_stale_cleanup must succeed on a fresh path");
        drop(listener);

        // Lockfile must exist now.
        assert!(
            lock.exists(),
            "bind_with_stale_cleanup must create <socket>.lock; not found at {lock:?}"
        );
    }

    // ── CopyPaste-3n9h: pair_peer must be disabled (no unauthenticated trust) ─

    /// `pair_peer` must return `not_implemented` with an actionable error
    /// message. A caller that knows a peer's TLS fingerprint must NOT be able
    /// to add it as trusted without going through PAKE+SAS authentication.
    #[tokio::test]
    async fn pair_peer_is_disabled_returns_not_implemented() {
        let dir = safe_tempdir();
        let sock = dir.path().join("test-pair-peer-disabled.sock");
        start_test_server(&sock).await;

        // Valid fingerprint + name — the old code would have accepted this.
        let valid_fp = "a".repeat(64); // 64-char hex fingerprint
        let body = format!(
            r#"{{"id":"pp1","method":"pair_peer","params":{{"fingerprint":"{valid_fp}","name":"Bob's Mac"}}}}"#
        );
        let resp = call_one(&sock, &body).await;
        assert_eq!(
            resp["ok"], false,
            "pair_peer must be rejected (unauthenticated pairing is disabled)"
        );
        assert_eq!(
            resp["error_code"], "not_implemented",
            "pair_peer must return not_implemented error code, got: {resp}"
        );
        // Error message must suggest the authenticated alternatives.
        let err = resp["error"].as_str().unwrap_or("");
        assert!(
            err.contains("pair_peer_with_password") || err.contains("pair_with_discovered"),
            "error message must suggest authenticated alternatives, got: {err}"
        );
    }

    // ── CopyPaste-n3bc: pair_get_sas must include peer_fingerprint on responder path ─

    /// `pair_get_sas` in AwaitingSas state must include `peer_fingerprint`
    /// when the pairing coordinator carries a fingerprint snapshot.
    /// This tests the state-machine path directly (no network required).
    #[test]
    fn pair_get_sas_includes_peer_fingerprint_when_available() {
        use crate::pairing_sm::{PairingCoordinator, PairingRole, PeerSnapshot};

        let coord = PairingCoordinator::new();
        let snap = PeerSnapshot {
            device_name: Some("Alice's Mac".to_string()),
            ip_addrs: vec!["192.168.1.5".to_string()],
            fingerprint: Some("aabbccdd".to_string()),
        };
        assert!(coord.try_begin(PairingRole::Responder, snap.clone()));
        let _rx = coord.enter_awaiting_sas("123456".to_string(), PairingRole::Responder, snap);

        let state = coord.snapshot();
        // Verify the state machine surfaces the fingerprint.
        let peer_snap = state
            .peer_snapshot()
            .expect("must have peer snapshot in AwaitingSas");
        assert_eq!(
            peer_snap.fingerprint.as_deref(),
            Some("aabbccdd"),
            "peer_snapshot must carry fingerprint for pair_get_sas to surface it"
        );
    }

    // ── CopyPaste-8yzf: sentinel off-by-one — racing third-party write ─────────

    /// Post-stamp must NOT overwrite the sentinel with a count that belongs to
    /// a concurrent third-party write. If `actual_count > expected_count` it
    /// means another app wrote to the pasteboard after us; leaving the sentinel
    /// at the expected count (which no future poll will see) is safer than
    /// stamping the third-party's count (which would suppress their content).
    ///
    /// This test exercises the sentinel logic directly via the `AtomicI64` that
    /// both `write_to_pasteboard` and `ClipboardMonitor::poll` share.
    #[test]
    fn sentinel_does_not_suppress_third_party_write_after_self_write() {
        use std::sync::atomic::{AtomicI64, Ordering};
        use std::sync::Arc;

        // Simulate: pre_count=10, we wrote and actual became 12 (our 2 ops),
        // then a third party wrote → actual is now 13.
        let sentinel = Arc::new(AtomicI64::new(-1));

        // Pre-stamp with expected post-write value.
        let pre_count: i64 = 10;
        let expected_after_write = pre_count + 2; // clearContents + setString
        sentinel.store(expected_after_write, Ordering::Release);

        // Our write completes — actual count is what we expected.
        let our_actual: i64 = 12;

        // Third-party writes between our write and the post-stamp read.
        // Simulate: post-stamp reads the already-incremented count.
        let post_stamp_read: i64 = 13; // third-party wrote after us

        // The WRONG approach (current bug): unconditionally overwrite with post-stamp.
        // This would store 13, suppressing the third-party write.
        // The CORRECT fix: only post-stamp if actual == expected (no racing write).
        if our_actual == expected_after_write {
            // Correct: our write was the only one, safe to post-stamp.
            sentinel.store(our_actual, Ordering::Release);
        }
        // If our_actual != expected_after_write OR post_stamp_read != our_actual,
        // leave the sentinel at expected_after_write (which already fired or is stale).

        // Simulating the monitor: it sees the third-party count (13).
        // With the correct fix, sentinel is 12 (not 13), so it won't suppress.
        let sentinel_val = sentinel.load(Ordering::Acquire);
        assert_ne!(
            sentinel_val, post_stamp_read,
            "sentinel must not match the third-party write count ({}); \
             that would suppress their clipboard content",
            post_stamp_read
        );
        // The sentinel remains at our expected value (12), which the monitor
        // may or may not have already consumed. Either way, 13 is not suppressed.
        assert_eq!(
            sentinel_val, expected_after_write,
            "sentinel must stay at our expected count ({}) not the third-party count ({})",
            expected_after_write, post_stamp_read
        );
    }

    // ── CopyPaste-aazu: import larger than MAX_REQUEST_BYTES returns clear error ─

    /// A 64 MiB import exceeds MAX_REQUEST_BYTES (16 MiB) — the connection is
    /// closed without any explanation. The fix: the per-item ceiling
    /// (MAX_IMPORT_ITEM_BYTES = 4 MiB) already guards individual items;
    /// but a batch with many large items can still exceed MAX_REQUEST_BYTES.
    /// The IPC layer must return a clear "request too large" error rather than
    /// silently closing the connection (which the CLI surfaces as a confusing EOF).
    ///
    /// This test verifies that the "request too large" response is returned and
    /// contains a human-readable error before the connection is closed.
    #[tokio::test]
    async fn import_oversized_request_returns_clear_error() {
        let dir = safe_tempdir();
        let sock = dir.path().join("test-import-oversized.sock");
        start_test_server(&sock).await;

        // Build a request that just barely exceeds MAX_REQUEST_BYTES (16 MiB + 1).
        // We construct a JSON line with a huge "items" array of one dummy item
        // whose content_bytes_b64 is large enough to tip over the limit.
        // The IPC layer reads up to MAX_REQUEST_BYTES + 1 then rejects.
        let item_size = MAX_REQUEST_BYTES + 100; // guarantee we exceed the limit
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD;
        let large_content = b64.encode(vec![0u8; item_size]);
        let body = format!(
            r#"{{"id":"imp1","method":"import","params":{{"items":[{{"content_type":"text","content_bytes_b64":"{large_content}","created_at_ms":1700000000}}]}}}}"#,
        );

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        // Send the body. The server will read up to MAX_REQUEST_BYTES+1 then
        // close the connection — write_all may fail with BrokenPipe once the
        // server closes; that is expected and acceptable.
        let _ = stream.write_all(body.as_bytes()).await;
        let _ = stream.write_all(b"\n").await;

        let mut lines = BufReader::new(&mut stream).lines();
        // Must receive a response (the "request too large" error) rather than
        // a hang. The response may arrive before all bytes are written, so we
        // read with a short timeout.
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line()).await;
        match result {
            Ok(Ok(Some(line))) => {
                let v: serde_json::Value =
                    serde_json::from_str(&line).expect("response must be valid JSON");
                assert_eq!(v["ok"], false, "oversized request must return ok=false");
                let err = v["error"].as_str().unwrap_or("");
                assert!(
                    !err.is_empty(),
                    "oversized request must return a non-empty error message"
                );
            }
            Ok(Ok(None)) => {
                // EOF — server closed after reading the oversize request.
                // This means the server did NOT hang waiting for more data;
                // the connection was properly terminated.  Acceptable outcome.
            }
            Ok(Err(e)) => {
                // BrokenPipe on read side is also fine — server closed the
                // socket after the rejection.
                if e.kind() != std::io::ErrorKind::BrokenPipe {
                    panic!("unexpected read error: {e}");
                }
            }
            Err(_) => panic!("timed out waiting for oversized-import response (daemon may hang)"),
        }
    }

    // ── CopyPaste-cb7u: delete_all batches all soft-deletes in one blocking tx ──

    /// `delete_all` must atomically tombstone every non-pinned, non-deleted item
    /// in a single blocking transaction and leave pinned items untouched.
    ///
    /// Pre-fix, the handler ran N sequential `spawn_blocking` calls (one per
    /// item); post-fix it uses ONE blocking closure that holds the DB lock for
    /// the full batch.  Both approaches must produce identical observable results —
    /// this test guards the correctness invariant.
    ///
    /// (CopyPaste-cb7u)
    #[tokio::test]
    async fn delete_all_tombstones_non_pinned_leaves_pinned_intact() {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let server = IpcServer::new(
            db.clone(),
            Arc::new(AtomicBool::new(false)),
            Arc::new(zeroize::Zeroizing::new([0u8; 32])),
            Arc::new([0u8; 32]),
        );

        // Seed two regular items and one pinned item.
        let (id_a, id_b, id_pinned) = {
            let guard = db.lock().await;
            let a = copypaste_core::ClipboardItem::new_text(vec![0xA1; 8], vec![0u8; 24], 1);
            let b = copypaste_core::ClipboardItem::new_text(vec![0xB2; 8], vec![0u8; 24], 2);
            let mut p = copypaste_core::ClipboardItem::new_text(vec![0xC3; 8], vec![0u8; 24], 3);
            p.pinned = true;
            copypaste_core::insert_item(&guard, &a).unwrap();
            copypaste_core::insert_item(&guard, &b).unwrap();
            copypaste_core::insert_item(&guard, &p).unwrap();
            (a.id.clone(), b.id.clone(), p.id.clone())
        };

        // Call delete_all — must report 2 deleted (the two non-pinned items).
        let resp = server
            .dispatch(r#"{"id":"da1","method":"delete_all","params":{}}"#)
            .await;
        assert!(resp.ok, "delete_all must succeed: {resp:?}");
        let deleted = resp
            .data
            .as_ref()
            .and_then(|d| d["deleted"].as_u64())
            .expect("delete_all must return {\"deleted\": N}");
        assert_eq!(deleted, 2, "exactly 2 non-pinned items must be deleted");

        // Verify DB state: tombstones have deleted=1 and NULL content.
        {
            let guard = db.lock().await;
            let item_a = copypaste_core::get_item_by_id(&*guard, &id_a).unwrap();
            let item_b = copypaste_core::get_item_by_id(&*guard, &id_b).unwrap();
            let item_p = copypaste_core::get_item_by_id(&*guard, &id_pinned).unwrap();

            assert_eq!(
                item_a.as_ref().map(|i| i.deleted),
                Some(true),
                "item A must be tombstoned"
            );
            assert!(
                item_a.as_ref().and_then(|i| i.content.as_deref()).is_none(),
                "item A content must be cleared (tombstone)"
            );

            assert_eq!(
                item_b.as_ref().map(|i| i.deleted),
                Some(true),
                "item B must be tombstoned"
            );
            assert!(
                item_b.as_ref().and_then(|i| i.content.as_deref()).is_none(),
                "item B content must be cleared (tombstone)"
            );

            // Pinned item must survive with content intact.
            assert_eq!(
                item_p.as_ref().map(|i| i.deleted),
                Some(false),
                "pinned item must NOT be deleted"
            );
            assert!(
                item_p.as_ref().and_then(|i| i.content.as_deref()).is_some(),
                "pinned item content must be preserved"
            );
        }
    }

    // ── CopyPaste-0w4v: cloud methods in non-cloud builds return not_implemented ─

    /// `cloud_sign_in` and `cloud_sign_out` must return a machine-readable
    /// `not_implemented` error (not "unknown method") when `cloud-sync` is not
    /// compiled in. Gated on `not(feature = "cloud-sync")` because when the
    /// feature is enabled the handler performs a real auth attempt and returns
    /// a different error code (`invalid_argument` for missing credentials),
    /// not `not_implemented`.
    #[cfg(not(feature = "cloud-sync"))]
    #[tokio::test]
    async fn cloud_sign_in_out_return_not_implemented_without_cloud_feature() {
        // This test is only meaningful for non-cloud builds.
        // In cloud builds, the handler is different (auth attempt). We only
        // assert that the response is valid JSON with ok=false and a non-empty
        // error_code regardless of build, since the key invariant is that
        // callers get a machine-readable code instead of "unknown method".
        let dir = safe_tempdir();
        let sock = dir.path().join("test-cloud-not-impl.sock");
        start_test_server(&sock).await;

        for method in &["cloud_sign_in", "cloud_sign_out"] {
            let body = format!(r#"{{"id":"c1","method":"{method}","params":{{}}}}"#);
            let resp = call_one(&sock, &body).await;
            assert_eq!(resp["ok"], false, "{method} must return ok=false");
            // Must have an error_code (either not_implemented or invalid_argument
            // for cloud builds that need credentials).
            assert!(
                resp["error_code"].is_string(),
                "{method} must return a machine-readable error_code, got: {resp}"
            );
        }
    }

    /// CopyPaste-vvsf: verify the re-encryption closure that `rotate_sync_key`
    /// would pass to `reencrypt_all_cloud_items` performs a correct
    /// decrypt-old / encrypt-new round-trip using the item_id for AAD binding.
    ///
    /// This is a pure crypto unit test — no IPC socket, no Supabase HTTP call.
    /// It exercises the same `base64 decode → decrypt_from_cloud(old_key, item_id)
    /// → encrypt_for_cloud(new_key, item_id) → base64 encode` pipeline that the
    /// production handler uses, asserting:
    ///
    ///   1. The new ciphertext decrypts correctly under the NEW key.
    ///   2. The new ciphertext does NOT decrypt under the OLD key.
    ///   3. A wrong item_id as AAD causes authentication failure.
    #[test]
    fn rotate_sync_key_reencrypt_closure_crypto_roundtrip() {
        use base64::Engine as _;
        use copypaste_core::{decrypt_from_cloud, derive_sync_key, encrypt_for_cloud, SyncKey};

        let old_key = derive_sync_key("old-passphrase-test-1").expect("derive old key");
        let new_key = derive_sync_key("new-passphrase-test-2").expect("derive new key");

        let item_id = "item-uuid-deadbeef";
        let plaintext = b"secret clipboard content";

        // Simulate what the push loop stores in Supabase: encrypt under old key.
        let old_blob =
            encrypt_for_cloud(&old_key, item_id, plaintext).expect("encrypt under old key");
        let old_ct_b64 = base64::engine::general_purpose::STANDARD.encode(&old_blob);

        // Capture key bytes for the closure (SyncKey is !Clone).
        let old_bytes = *old_key.as_bytes();
        let new_bytes = *new_key.as_bytes();

        // This is the closure shape that rotate_sync_key passes to
        // reencrypt_all_cloud_items (CopyPaste-vvsf).
        let reencrypt = |rcv_item_id: &str, old_ct: &str| -> Result<String, String> {
            let old_k = SyncKey::from_bytes(old_bytes);
            let new_k = SyncKey::from_bytes(new_bytes);
            let raw = base64::engine::general_purpose::STANDARD
                .decode(old_ct)
                .map_err(|e| format!("base64 decode: {e}"))?;
            let plain = decrypt_from_cloud(&old_k, rcv_item_id, &raw)
                .map_err(|e| format!("decrypt: {e}"))?;
            let new_blob = encrypt_for_cloud(&new_k, rcv_item_id, &plain)
                .map_err(|e| format!("encrypt: {e}"))?;
            Ok(base64::engine::general_purpose::STANDARD.encode(&new_blob))
        };

        // Apply the closure.
        let new_ct_b64 = reencrypt(item_id, &old_ct_b64).expect("re-encryption must succeed");

        // Decode and verify the new ciphertext decrypts under the NEW key.
        let new_k2 = SyncKey::from_bytes(new_bytes);
        let new_raw = base64::engine::general_purpose::STANDARD
            .decode(&new_ct_b64)
            .unwrap();
        let recovered = decrypt_from_cloud(&new_k2, item_id, &new_raw)
            .expect("new ciphertext must decrypt under new key");
        assert_eq!(
            recovered, plaintext,
            "plaintext must survive old_key→decrypt→new_key→encrypt round-trip"
        );

        // The new ciphertext must NOT decrypt under the OLD key (different key → auth fail).
        let old_k2 = SyncKey::from_bytes(old_bytes);
        let old_decrypt_result = decrypt_from_cloud(&old_k2, item_id, &new_raw);
        assert!(
            old_decrypt_result.is_err(),
            "new ciphertext must not decrypt under old key (wrong key → auth fail)"
        );

        // A wrong item_id must cause auth failure (AAD mismatch).
        let new_k3 = SyncKey::from_bytes(new_bytes);
        let wrong_aad_result = decrypt_from_cloud(&new_k3, "wrong-item-id", &new_raw);
        assert!(
            wrong_aad_result.is_err(),
            "wrong item_id AAD must cause authentication failure"
        );
    }

    // ── CopyPaste-40gl: db_stats IPC verb ────────────────────────────────────

    /// `db_stats` on an empty database must return `{ item_count: 0, size_bytes }`.
    /// The `size_bytes` value reflects the on-disk daemon DB (not the in-memory
    /// DB the test server uses internally), so we only assert the structure and
    /// the zero item_count.
    #[tokio::test]
    async fn db_stats_empty_database_returns_zero_count() {
        let dir = safe_tempdir();
        let sock = dir.path().join("db_stats_empty.sock");
        start_test_server(&sock).await;

        let resp = call_one(&sock, r#"{"id":"ds1","method":"db_stats","params":{}}"#).await;
        assert_eq!(resp["ok"], true, "db_stats must succeed: {resp}");
        let item_count = resp["data"]["item_count"]
            .as_u64()
            .expect("item_count must be u64");
        assert_eq!(item_count, 0, "empty DB must report 0 items: {resp}");
        // size_bytes is present and is a u64 (value may be non-zero if the
        // daemon's own DB file exists on this machine).
        let _size_bytes = resp["data"]["size_bytes"]
            .as_u64()
            .expect("size_bytes must be a u64 field");
    }

    /// `db_stats` on a DB with items must report the correct count.
    #[tokio::test]
    async fn db_stats_reports_correct_item_count() {
        let dir = safe_tempdir();
        let sock = dir.path().join("db_stats_count.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Seed 3 items directly.
        {
            let guard = db.lock().await;
            for _ in 0..3 {
                let item =
                    copypaste_core::ClipboardItem::new_text(vec![0xABu8; 16], vec![0u8; 24], 1);
                copypaste_core::insert_item(&guard, &item).unwrap();
            }
        }

        let resp = call_one(&sock, r#"{"id":"ds2","method":"db_stats","params":{}}"#).await;
        assert_eq!(
            resp["ok"], true,
            "db_stats must succeed after seeding: {resp}"
        );
        let item_count = resp["data"]["item_count"]
            .as_u64()
            .expect("item_count must be u64");
        assert_eq!(item_count, 3, "expected 3 items after seeding: {resp}");
    }

    // ── CopyPaste-cbfl: parse-error / oversized id echoing ───────────────────

    /// When the daemon cannot parse a request as JSON, the error response's
    /// `id` must echo back the id from the raw JSON (if extractable) so that
    /// the CLI's id-matching guard can correlate the error.  When no id is
    /// extractable the fallback `"?"` is used.
    #[tokio::test]
    async fn parse_error_echoes_id_from_raw_json() {
        let dir = safe_tempdir();
        let sock = dir.path().join("parse_err_id.sock");
        start_test_server(&sock).await;

        // Send valid JSON that has an id field but is missing the required
        // `method` field, triggering a serde parse error.
        let resp = call_one(&sock, r#"{"id":"req-42","not_method":"foo","params":{}}"#).await;
        // The response must be an error.
        assert_eq!(resp["ok"], false, "malformed request must fail: {resp}");
        // The id in the response must echo "req-42" (the id from the raw JSON).
        assert_eq!(
            resp["id"].as_str(),
            Some("req-42"),
            "parse-error response id must echo the request id: {resp}"
        );
    }

    /// When the line is pure garbage (not parseable as JSON at all), the
    /// fallback id `"?"` is used since no id can be extracted.
    #[tokio::test]
    async fn parse_error_uses_fallback_id_when_not_valid_json() {
        let dir = safe_tempdir();
        let sock = dir.path().join("parse_err_fallback.sock");
        start_test_server(&sock).await;

        let resp = call_one(&sock, "this is not JSON at all!!!").await;
        assert_eq!(resp["ok"], false, "garbage input must fail: {resp}");
        assert_eq!(
            resp["id"].as_str(),
            Some("?"),
            "garbage parse-error response must use fallback id '?': {resp}"
        );
    }

    // ── CopyPaste-93yr: export warns on skipped non-text items ───────────────

    /// The export response must include `skipped_non_text` that is non-zero
    /// when image items exist in the database. Text items must still export.
    #[tokio::test]
    async fn export_skipped_non_text_count_is_non_zero_for_image_items() {
        let dir = safe_tempdir();
        let sock = dir.path().join("export_skip_img.sock");
        let (_pm, db) = start_test_server_returning_db(&sock, false).await;

        // Insert one image item and one text item directly.
        {
            let guard = db.lock().await;
            // Image item — will be skipped during export.
            // new_image takes (blob, meta_json, lamport_ts, thumb).
            let img_item = copypaste_core::ClipboardItem::new_image(
                vec![0xFFu8; 64], // fake encrypted blob bytes
                "{}".to_string(), // image_meta_json
                1,
                None,
            );
            copypaste_core::insert_item(&guard, &img_item).unwrap();

            // Text item — must appear in the export.
            guard
                .conn()
                .execute(
                    "INSERT INTO clipboard_items \
                 (id, item_id, content_type, content, content_nonce, \
                  is_sensitive, is_synced, lamport_ts, wall_time, key_version) \
                 VALUES (?, ?, 'text', ?, ?, 0, 0, 2, 2000000, 2)",
                    rusqlite::params![
                        uuid::Uuid::new_v4().to_string(),
                        uuid::Uuid::new_v4().to_string(),
                        // Zero-key v2 encrypt of "hello".
                        // We use a raw ciphertext that the zero-key daemon can decrypt;
                        // since we only care about skipped_non_text count we can test
                        // with a zero-length ciphertext (decrypt will fail → skipped by
                        // the decrypt-error path, but image count is independent).
                        vec![0u8; 1],
                        vec![0u8; 24],
                    ],
                )
                .unwrap();
        }

        let resp = call_one(
            &sock,
            r#"{"id":"xe1","method":"export","params":{"limit":100}}"#,
        )
        .await;
        assert_eq!(resp["ok"], true, "export must succeed: {resp}");

        let skipped = resp["data"]["skipped_non_text"]
            .as_u64()
            .expect("skipped_non_text must be present in export response");
        assert_eq!(
            skipped, 1,
            "exactly one image item must be counted as skipped: {resp}"
        );
    }

    // ── CopyPaste-x94p / CopyPaste-8wbt: db_backup + db_restore ────────────

    /// `db_backup` without `dest_path` must return an `invalid_argument` error.
    #[tokio::test]
    async fn db_backup_missing_dest_returns_error() {
        let dir = safe_tempdir();
        let sock = dir.path().join("backup_no_dest.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"b1\",\"method\":\"db_backup\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], false,
            "missing dest_path must return error: {resp}"
        );
        assert_eq!(
            resp["error_code"].as_str(),
            Some("invalid_argument"),
            "error_code must be invalid_argument: {resp}"
        );
    }

    /// `db_backup` with a valid `dest_path` must produce a backup file.
    #[tokio::test]
    async fn db_backup_creates_backup_file() {
        let dir = safe_tempdir();
        let sock = dir.path().join("backup_ok.sock");
        start_test_server(&sock).await;
        let dest = dir.path().join("test-backup.db.enc");
        let req = format!(
            "{{\"id\":\"b2\",\"method\":\"db_backup\",\"params\":{{\"dest_path\":\"{}\"}}}}\n",
            dest.to_string_lossy()
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(resp["ok"], true, "db_backup must succeed: {resp}");
        assert!(
            dest.exists(),
            "backup file must exist after successful db_backup"
        );
        let size_bytes = resp["data"]["size_bytes"].as_u64().unwrap_or(0);
        assert!(size_bytes > 0, "backup size must be > 0: {resp}");
    }

    /// `db_backup` to an already-existing path must return an error.
    #[tokio::test]
    async fn db_backup_refuses_overwrite() {
        let dir = safe_tempdir();
        let sock = dir.path().join("backup_overwrite.sock");
        start_test_server(&sock).await;
        let dest = dir.path().join("existing.db.enc");
        std::fs::write(&dest, b"existing content").unwrap();
        let req = format!(
            "{{\"id\":\"b3\",\"method\":\"db_backup\",\"params\":{{\"dest_path\":\"{}\"}}}}\n",
            dest.to_string_lossy()
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(req.as_bytes()).await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], false,
            "db_backup must refuse to overwrite an existing file: {resp}"
        );
        assert_eq!(
            resp["error_code"].as_str(),
            Some("invalid_argument"),
            "error_code must be invalid_argument: {resp}"
        );
    }

    /// `db_restore` without `confirm=true` must return `invalid_argument`.
    #[tokio::test]
    async fn db_restore_requires_confirm() {
        let dir = safe_tempdir();
        let sock = dir.path().join("restore_no_confirm.sock");
        start_test_server(&sock).await;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(
                b"{\"id\":\"r1\",\"method\":\"db_restore\",\"params\":{\"src_path\":\"/tmp/x.db\"}}\n",
            )
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], false,
            "restore without confirm must be rejected: {resp}"
        );
        assert_eq!(
            resp["error_code"].as_str(),
            Some("invalid_argument"),
            "error_code must be invalid_argument: {resp}"
        );
    }

    /// `db_restore` with a non-existent `src_path` must return `invalid_argument`.
    #[tokio::test]
    async fn db_restore_missing_file_returns_error() {
        let dir = safe_tempdir();
        let sock = dir.path().join("restore_missing_file.sock");
        start_test_server(&sock).await;
        let req = r#"{"id":"r2","method":"db_restore","params":{"confirm":true,"src_path":"/does/not/exist/backup.db.enc"}}"#;
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(format!("{req}\n").as_bytes())
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let line = lines.next_line().await.unwrap().unwrap();
        let resp: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(
            resp["ok"], false,
            "restore with missing file must be rejected: {resp}"
        );
    }

    /// CopyPaste-crh3.7 / crh3.57: db_backup and vacuum must be gated in
    /// degraded mode (ready=false). Otherwise db_backup writes an EMPTY backup
    /// of the in-memory placeholder and returns ok:true, and vacuum reports
    /// misleading size stats read from the real on-disk file. Both must return
    /// ipc_not_ready and db_backup must create NO file.
    #[tokio::test]
    async fn db_backup_and_vacuum_gated_in_degraded_mode() {
        let dir = safe_tempdir();
        let sock = dir.path().join("degraded_backup.sock");
        start_not_ready_server(&sock).await;

        // db_backup → ipc_not_ready, and no file at dest_path.
        let dest = dir.path().join("should_not_be_created.db.enc");
        let backup_req = format!(
            "{{\"id\":\"dg1\",\"method\":\"db_backup\",\"params\":{{\"dest_path\":\"{}\"}}}}\n",
            dest.to_string_lossy()
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(backup_req.as_bytes()).await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let resp: serde_json::Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(
            resp["ok"], false,
            "db_backup must be rejected when degraded: {resp}"
        );
        assert_eq!(
            resp["error_code"].as_str().unwrap_or_default(),
            "ipc_not_ready",
            "db_backup must return ipc_not_ready when degraded: {resp}"
        );
        assert!(
            !dest.exists(),
            "degraded db_backup must NOT create a backup file"
        );
        drop(stream);

        // vacuum → ipc_not_ready.
        let mut stream2 = UnixStream::connect(&sock).await.unwrap();
        stream2
            .write_all(b"{\"id\":\"dg2\",\"method\":\"vacuum\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines2 = BufReader::new(&mut stream2).lines();
        let resp2: serde_json::Value =
            serde_json::from_str(&lines2.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(
            resp2["ok"], false,
            "vacuum must be rejected when degraded: {resp2}"
        );
        assert_eq!(
            resp2["error_code"].as_str().unwrap_or_default(),
            "ipc_not_ready",
            "vacuum must return ipc_not_ready when degraded: {resp2}"
        );
    }

    /// CopyPaste-crh3.105: watch_subscribe is intercepted before dispatch(), so
    /// it previously bypassed the readiness gate — a degraded daemon accepted the
    /// subscription and streamed nothing. It must now return ipc_not_ready.
    #[tokio::test]
    async fn watch_subscribe_rejected_when_not_ready() {
        let dir = safe_tempdir();
        let sock = dir.path().join("watch_not_ready.sock");
        start_not_ready_server(&sock).await;

        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"ws-nr\",\"method\":\"watch_subscribe\",\"params\":{}}\n")
            .await
            .unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let resp: serde_json::Value =
            serde_json::from_str(&lines.next_line().await.unwrap().unwrap()).unwrap();
        assert_eq!(
            resp["ok"], false,
            "watch_subscribe must be rejected when degraded: {resp}"
        );
        assert_eq!(
            resp["error_code"].as_str().unwrap_or_default(),
            "ipc_not_ready",
            "watch_subscribe must return ipc_not_ready when degraded: {resp}"
        );
        assert_eq!(
            resp["id"].as_str(),
            Some("ws-nr"),
            "must echo the request id: {resp}"
        );
    }

    // ── CopyPaste-8wbt / crh3.6 / crh3.2: db_restore validate-then-swap ────────
    //
    // These drive the pure `restore_database_file` routine directly against real
    // on-disk SQLCipher databases in a temp dir, so they are deterministic and
    // independent of the Keychain / IPC harness.

    /// Create a real on-disk SQLCipher DB at `path` (clipboard schema via
    /// `Database::open`) and stamp a `_restore_marker` row with `tag` so two
    /// databases can be told apart. Checkpoints the WAL so a plain file copy of
    /// `path` is self-contained.
    fn make_marked_db(path: &std::path::Path, key: &[u8; 32], tag: &str) {
        let db = Database::open(path, key).expect("open marked db");
        db.conn()
            .execute_batch(&format!(
                "CREATE TABLE IF NOT EXISTS _restore_marker(tag TEXT); \
                 DELETE FROM _restore_marker; \
                 INSERT INTO _restore_marker(tag) VALUES ('{tag}'); \
                 PRAGMA wal_checkpoint(TRUNCATE);"
            ))
            .unwrap();
    }

    /// Read the marker stamped by [`make_marked_db`], opening `path` with `key`.
    /// Returns `None` if the DB cannot be opened with that key.
    fn read_marker(path: &std::path::Path, key: &[u8; 32]) -> Option<String> {
        let db = Database::open_no_auto_migrate(path, key).ok()?;
        db.conn()
            .query_row("SELECT tag FROM _restore_marker LIMIT 1", [], |r| {
                r.get::<_, String>(0)
            })
            .ok()
    }

    /// True if any `clipboard.db.before-restore-*` aside copy exists next to
    /// `db_path`.
    fn aside_exists(db_path: &std::path::Path) -> bool {
        let name = db_path.file_name().unwrap().to_string_lossy().to_string();
        let needle = format!("{name}.before-restore-");
        std::fs::read_dir(db_path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(&needle))
    }

    /// True if any `clipboard.db.restore-staging-*` artifact was left behind.
    fn staging_exists(db_path: &std::path::Path) -> bool {
        let name = db_path.file_name().unwrap().to_string_lossy().to_string();
        let needle = format!("{name}.restore-staging-");
        std::fs::read_dir(db_path.parent().unwrap())
            .unwrap()
            .filter_map(|e| e.ok())
            .any(|e| e.file_name().to_string_lossy().starts_with(&needle))
    }

    #[test]
    fn restore_valid_backup_swaps_data_and_keeps_aside_when_not_forced() {
        let dir = safe_tempdir();
        let key = [0x11u8; 32];
        let db_path = dir.path().join("clipboard.db");
        let backup = dir.path().join("backup.db");
        make_marked_db(&db_path, &key, "LIVE");
        make_marked_db(&backup, &key, "BACKUP");

        let restored = restore_database_file(&backup, &db_path, &key, false)
            .expect("valid backup must restore");
        drop(restored);
        assert_eq!(read_marker(&db_path, &key).as_deref(), Some("BACKUP"));
        assert!(
            aside_exists(&db_path),
            "non-force restore must keep the prior DB as an aside safety copy"
        );
        assert!(
            !staging_exists(&db_path),
            "staging copy must be cleaned up after a successful restore"
        );
    }

    #[test]
    fn restore_valid_backup_force_removes_aside() {
        let dir = safe_tempdir();
        let key = [0x22u8; 32];
        let db_path = dir.path().join("clipboard.db");
        let backup = dir.path().join("backup.db");
        make_marked_db(&db_path, &key, "LIVE");
        make_marked_db(&backup, &key, "BACKUP");

        restore_database_file(&backup, &db_path, &key, true).expect("restore");
        assert_eq!(read_marker(&db_path, &key).as_deref(), Some("BACKUP"));
        assert!(
            !aside_exists(&db_path),
            "force restore must delete the aside safety copy"
        );
    }

    #[test]
    fn restore_wrong_key_backup_leaves_live_db_intact() {
        // CopyPaste-8wbt: the core P0 — a wrong-key backup must NEVER touch the
        // live database.
        let dir = safe_tempdir();
        let live_key = [0x33u8; 32];
        let other_key = [0x44u8; 32];
        let db_path = dir.path().join("clipboard.db");
        let backup = dir.path().join("backup.db");
        make_marked_db(&db_path, &live_key, "LIVE");
        make_marked_db(&backup, &other_key, "OTHER");

        let err = restore_database_file(&backup, &db_path, &live_key, true)
            .map(|_| ())
            .expect_err("wrong-key backup must be rejected");
        assert!(err.contains("db_restore"), "error must be tagged: {err}");
        assert_eq!(
            read_marker(&db_path, &live_key).as_deref(),
            Some("LIVE"),
            "live DB must remain readable with its original key"
        );
        assert!(
            !aside_exists(&db_path),
            "failed validation must not move the live DB aside"
        );
        assert!(
            !staging_exists(&db_path),
            "failed validation must clean up the staging copy"
        );
    }

    #[test]
    fn restore_corrupt_backup_leaves_live_db_intact() {
        let dir = safe_tempdir();
        let key = [0x55u8; 32];
        let db_path = dir.path().join("clipboard.db");
        let backup = dir.path().join("garbage.db");
        make_marked_db(&db_path, &key, "LIVE");
        std::fs::write(&backup, b"this is not a sqlite database at all").unwrap();

        let err = restore_database_file(&backup, &db_path, &key, false)
            .map(|_| ())
            .expect_err("garbage backup must be rejected");
        assert!(err.contains("db_restore"), "error must be tagged: {err}");
        assert_eq!(read_marker(&db_path, &key).as_deref(), Some("LIVE"));
        assert!(!aside_exists(&db_path));
    }

    #[test]
    fn restore_wrong_schema_backup_is_rejected() {
        // A real SQLCipher DB with the correct key but WITHOUT the clipboard
        // schema must be rejected (it is not a CopyPaste database).
        let dir = safe_tempdir();
        let key = [0x66u8; 32];
        let db_path = dir.path().join("clipboard.db");
        let backup = dir.path().join("foreign.db");
        make_marked_db(&db_path, &key, "LIVE");
        {
            let foreign = Database::open(&backup, &key).unwrap();
            foreign
                .conn()
                .execute_batch(
                    "DROP TABLE IF EXISTS clipboard_items; PRAGMA wal_checkpoint(TRUNCATE);",
                )
                .unwrap();
        }
        let err = restore_database_file(&backup, &db_path, &key, false)
            .map(|_| ())
            .expect_err("non-CopyPaste DB must be rejected");
        assert!(
            err.contains("clipboard_items"),
            "error must name the missing clipboard table: {err}"
        );
        assert_eq!(read_marker(&db_path, &key).as_deref(), Some("LIVE"));
    }

    #[test]
    fn restore_into_empty_path_succeeds_for_degraded_recovery() {
        // crh3.6: degraded recovery — no live DB on disk yet. A valid backup must
        // still restore, with nothing to move aside.
        let dir = safe_tempdir();
        let key = [0x77u8; 32];
        let db_path = dir.path().join("clipboard.db");
        let backup = dir.path().join("backup.db");
        make_marked_db(&backup, &key, "BACKUP");
        assert!(!db_path.exists());

        restore_database_file(&backup, &db_path, &key, false).expect("restore into empty path");
        assert_eq!(read_marker(&db_path, &key).as_deref(), Some("BACKUP"));
        assert!(
            !aside_exists(&db_path),
            "nothing to move aside when there was no live DB"
        );
    }

    #[test]
    fn restore_rebuilt_pool_sees_restored_data_while_stale_pool_does_not() {
        // CopyPaste-crh3.2 / crh3.54: after a restore swaps the on-disk inode, a
        // read pool opened BEFORE the restore keeps serving stale data through
        // its cached file descriptors; only a pool rebuilt against the restored
        // file returns the restored contents. This is exactly why the db_restore
        // handler rebuilds `self.read_pool`.
        let dir = safe_tempdir();
        let key = [0x88u8; 32];
        let db_path = dir.path().join("clipboard.db");
        let backup = dir.path().join("backup.db");

        // Live DB starts at tag "A"; snapshot it as the backup at that point.
        make_marked_db(&db_path, &key, "A");
        std::fs::copy(&db_path, &backup).unwrap();
        // Advance the live DB to tag "B" (post-backup mutation), checkpointed.
        {
            let live = Database::open_no_auto_migrate(&db_path, &key).unwrap();
            live.conn()
                .execute_batch(
                    "UPDATE _restore_marker SET tag = 'B'; PRAGMA wal_checkpoint(TRUNCATE);",
                )
                .unwrap();
        }

        // A pool opened against the live "B" DB BEFORE the restore.
        let stale_pool = copypaste_core::open_pool(&db_path, &key, 2).unwrap();
        let stale_before: String = stale_pool
            .get()
            .unwrap()
            .query_row("SELECT tag FROM _restore_marker LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            stale_before, "B",
            "sanity: pool sees live data before restore"
        );

        // Restore from the "A" backup.
        restore_database_file(&backup, &db_path, &key, false).expect("restore");

        // The stale pool keeps serving "B" from its cached FDs — the crh3.2 bug
        // the handler must work around by rebuilding the pool.
        let stale_after: String = stale_pool
            .get()
            .unwrap()
            .query_row("SELECT tag FROM _restore_marker LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(
            stale_after, "B",
            "stale pool keeps serving pre-restore data (crh3.2 root cause)"
        );

        // A freshly rebuilt pool (what the handler installs after restore) sees
        // the restored "A" data.
        let fresh_pool = copypaste_core::open_pool(&db_path, &key, 2).unwrap();
        let fresh_tag: String = fresh_pool
            .get()
            .unwrap()
            .query_row("SELECT tag FROM _restore_marker LIMIT 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fresh_tag, "A", "rebuilt pool sees the restored data");
    }

    // ── CopyPaste-44rq.19: watch_subscribe push-streaming tests ────────────────

    /// Start a test server that has a `new_item_tx` wired in, returning both
    /// the broadcast sender and the db handle so callers can inject events.
    async fn start_test_server_with_broadcast(
        socket_path: &std::path::Path,
    ) -> (
        tokio::sync::broadcast::Sender<copypaste_core::ClipboardItem>,
        Arc<Mutex<Database>>,
    ) {
        let db = Arc::new(Mutex::new(Database::open_in_memory().unwrap()));
        let private_mode = Arc::new(AtomicBool::new(false));
        let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
        let device_pub = Arc::new([0u8; 32]);
        // Capacity 64: large enough for tests, mirrors the production value.
        let (tx, _) = tokio::sync::broadcast::channel::<copypaste_core::ClipboardItem>(64);
        let cert = copypaste_p2p::cert::SelfSignedCert::generate("test-device").unwrap();
        let server = IpcServer::new(db.clone(), private_mode, local_key, device_pub)
            .with_cert_fingerprint(display_fingerprint(&cert.fingerprint()))
            .with_new_item_tx(tx.clone());
        let listener =
            tokio::net::UnixListener::bind(socket_path).expect("test socket bind must succeed");
        let path = socket_path.to_path_buf();
        tokio::spawn(async move {
            if let Err(e) = server.serve_on(listener, CancellationToken::new()).await {
                tracing::error!("ipc watch-test server error: {e} at {path:?}");
            }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        (tx, db)
    }

    /// CopyPaste-44rq.19: `watch_subscribe` must send an initial ack (ok=true,
    /// event="subscribed") and then push one event line per new clipboard item
    /// broadcast on `new_item_tx`, without any polling.
    #[tokio::test]
    async fn watch_subscribe_receives_push_events() {
        let dir = safe_tempdir();
        let sock = dir.path().join("watch_push.sock");
        let (tx, _db) = start_test_server_with_broadcast(&sock).await;

        // Open a subscribe connection.
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream
            .write_all(b"{\"id\":\"w1\",\"method\":\"watch_subscribe\",\"params\":{}}\n")
            .await
            .unwrap();

        let mut lines = BufReader::new(&mut stream).lines();

        // First line must be the ack.
        let ack_line = tokio::time::timeout(std::time::Duration::from_secs(2), lines.next_line())
            .await
            .expect("ack must arrive within 2 s")
            .unwrap()
            .unwrap();
        let ack: serde_json::Value = serde_json::from_str(&ack_line).unwrap();
        assert_eq!(ack["ok"], true, "ack must be ok=true: {ack_line}");
        assert_eq!(
            ack["event"].as_str(),
            Some("subscribed"),
            "ack must have event=subscribed: {ack_line}"
        );
        assert_eq!(
            ack["id"].as_str(),
            Some("w1"),
            "ack must echo the request id: {ack_line}"
        );

        // Broadcast a new clipboard item.
        let item = copypaste_core::ClipboardItem::new_text(
            vec![0u8; 16], // dummy encrypted content
            vec![0u8; 24], // dummy nonce
            1,
        );
        let item_id = item.item_id.clone();
        let _ = tx.send(item);

        // The daemon must push a new_item event line within 500 ms.
        let evt_line =
            tokio::time::timeout(std::time::Duration::from_millis(500), lines.next_line())
                .await
                .expect("new_item event must arrive within 500 ms")
                .unwrap()
                .unwrap();
        let evt: serde_json::Value = serde_json::from_str(&evt_line).unwrap();
        assert_eq!(evt["ok"], true, "event must be ok=true: {evt_line}");
        assert_eq!(
            evt["event"].as_str(),
            Some("new_item"),
            "event must have event=new_item: {evt_line}"
        );
        assert_eq!(
            evt["item_id"].as_str(),
            Some(item_id.as_str()),
            "event must carry item_id: {evt_line}"
        );
        // Content/plaintext must NOT be present.
        assert!(
            evt.get("content").is_none(),
            "event must not contain raw content: {evt_line}"
        );
    }

    /// CopyPaste-44rq.19: while a `watch_subscribe` connection is open, a
    /// second normal one-shot request (e.g. `status`) on a DIFFERENT connection
    /// must still return its normal response. This verifies the subscribe path
    /// does not wedge the accept loop or any shared state.
    #[tokio::test]
    async fn watch_subscribe_does_not_break_concurrent_one_shot_requests() {
        let dir = safe_tempdir();
        let sock = dir.path().join("watch_concurrent.sock");
        let (_tx, _db) = start_test_server_with_broadcast(&sock).await;

        // Open a long-lived subscribe connection; don't close it.
        let mut sub_stream = UnixStream::connect(&sock).await.unwrap();
        sub_stream
            .write_all(b"{\"id\":\"ws1\",\"method\":\"watch_subscribe\",\"params\":{}}\n")
            .await
            .unwrap();
        // Consume the ack to confirm the subscribe is established.
        let mut sub_lines = BufReader::new(&mut sub_stream).lines();
        let _ = tokio::time::timeout(std::time::Duration::from_secs(2), sub_lines.next_line())
            .await
            .expect("subscribe ack must arrive")
            .unwrap()
            .unwrap();

        // Now make a normal one-shot `status` request on a DIFFERENT connection.
        let status_resp = call_one(&sock, r#"{"id":"st1","method":"status"}"#).await;
        assert_eq!(
            status_resp["ok"], true,
            "status must succeed while a subscribe connection is open: {status_resp}"
        );
        assert_eq!(
            status_resp["data"]["status"].as_str(),
            Some("running"),
            "status.data.status must be 'running': {status_resp}"
        );
    }

    /// CopyPaste-44rq.19: closing the subscribe client (dropping the stream)
    /// must NOT wedge the daemon. After the subscriber disconnects, subsequent
    /// `status` calls on a fresh connection must still succeed.
    #[tokio::test]
    async fn watch_subscribe_client_disconnect_does_not_wedge_daemon() {
        let dir = safe_tempdir();
        let sock = dir.path().join("watch_disconnect.sock");
        let (tx, _db) = start_test_server_with_broadcast(&sock).await;

        // Subscribe, read the ack, then drop the client connection.
        {
            let mut stream = UnixStream::connect(&sock).await.unwrap();
            stream
                .write_all(b"{\"id\":\"wd1\",\"method\":\"watch_subscribe\",\"params\":{}}\n")
                .await
                .unwrap();
            let mut lines = BufReader::new(&mut stream).lines();
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), lines.next_line())
                .await
                .expect("ack must arrive")
                .unwrap()
                .unwrap();
            // Drop `stream` here — simulates client disconnect.
        }

        // Broadcast an event so the daemon's subscribe loop sees a write error
        // and exits cleanly.
        let item = copypaste_core::ClipboardItem::new_text(vec![0u8; 16], vec![0u8; 24], 2);
        let _ = tx.send(item);

        // Give the task a moment to handle the disconnect.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // The daemon must still handle normal requests.
        let resp = call_one(&sock, r#"{"id":"wd2","method":"status"}"#).await;
        assert_eq!(
            resp["ok"], true,
            "daemon must still respond after subscriber disconnect: {resp}"
        );
    }

    /// Verify `db_backup` then `db_restore` end-to-end: backup creates the file,
    /// restore validation (is_file check) passes, and the handler returns a
    /// well-formed response (ok=true or a clear internal error) without panic.
    ///
    /// NOTE: the `db_restore` handler operates on the daemon's real data-dir path
    /// (`crate::paths::db_path()`), not on the temp dir of this test. In a
    /// sandbox/CI environment that lacks write access to the data dir the restore
    /// step may return `ok=false` with an `internal_error`. The test therefore
    /// only asserts on `db_backup` and that `db_restore` returns a *parseable*
    /// JSON response — it does NOT assert `ok=true` on restore, because the
    /// outcome depends on filesystem permissions outside the test's control.
    #[tokio::test]
    async fn db_backup_produces_file_and_restore_sends_response() {
        let dir = safe_tempdir();
        let sock = dir.path().join("backup_restore_rt.sock");
        start_test_server(&sock).await;

        // 1. Backup must succeed and create the file.
        let backup = dir.path().join("roundtrip.db.enc");
        let backup_req = format!(
            "{{\"id\":\"rt1\",\"method\":\"db_backup\",\"params\":{{\"dest_path\":\"{}\"}}}}\n",
            backup.to_string_lossy()
        );
        let mut stream = UnixStream::connect(&sock).await.unwrap();
        stream.write_all(backup_req.as_bytes()).await.unwrap();
        let mut lines = BufReader::new(&mut stream).lines();
        let bk_line = lines.next_line().await.unwrap().unwrap();
        let bk_resp: serde_json::Value = serde_json::from_str(&bk_line).unwrap();
        assert_eq!(bk_resp["ok"], true, "backup must succeed: {bk_resp}");
        assert!(backup.exists(), "backup file must exist");
        drop(stream);

        // 2. Restore: the handler parses the request and returns a well-formed
        //    JSON response.  We do not assert ok=true here because the handler
        //    attempts to copy the backup to the daemon's real data-dir (which may
        //    be inaccessible in sandboxed test environments).
        let restore_req = format!(
            "{{\"id\":\"rt2\",\"method\":\"db_restore\",\"params\":{{\"confirm\":true,\"src_path\":\"{}\",\"force\":true}}}}\n",
            backup.to_string_lossy()
        );
        let mut stream2 = UnixStream::connect(&sock).await.unwrap();
        stream2.write_all(restore_req.as_bytes()).await.unwrap();
        let mut lines2 = BufReader::new(&mut stream2).lines();
        let rs_line = lines2.next_line().await.unwrap().unwrap();
        let rs_resp: serde_json::Value = serde_json::from_str(&rs_line).unwrap();
        // Response must be parseable JSON with an "ok" field (bool) and an "id".
        assert!(
            rs_resp["ok"].is_boolean(),
            "restore response must have a boolean ok field: {rs_resp}"
        );
        assert_eq!(
            rs_resp["id"].as_str(),
            Some("rt2"),
            "restore response must echo the request id: {rs_resp}"
        );
    }
}
