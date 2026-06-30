use crate::protocol::{
    Request, Response, CURRENT_PROTOCOL_VERSION, ERR_CODE_AUTH_FAILED, ERR_CODE_INTERNAL_ERROR,
    ERR_CODE_INVALID_ARGUMENT, ERR_CODE_IPC_NOT_READY, ERR_CODE_NOT_FOUND,
    ERR_CODE_NOT_IMPLEMENTED, ERR_CODE_RATE_LIMITED, ERR_CODE_REQUEST_TOO_LARGE,
    ERR_CODE_VERSION_MISMATCH, MIN_SUPPORTED_PROTOCOL_VERSION,
};
use anyhow::Context as _; // CopyPaste-crh3.90
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
// ra15.1: the non-test callers of these two helpers live in the macOS-gated
// paste-back path, but the helpers themselves are cross-platform and exercised
// by inline tests on every platform. Allow them unused on the non-macOS lib
// build so -D warnings stays green without breaking the Linux test build.
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
pub(crate) use pasteboard::paste_file_cache_dir;
#[cfg_attr(not(target_os = "macos"), allow(unused_imports))]
pub(crate) use pasteboard::prune_old_paste_files;
// Helpers used in impl IpcServer dispatch code (non-test):
use pasteboard::{lazy_backfill_thumbnail, parse_image_thumb_dims, PasteboardError};
// ra15.1: `map_content_type_to_uti` is `#[cfg(target_os = "macos")]` (its only
// caller is the macOS paste-back path); gate the import to match so the
// non-macOS (Linux) build resolves (CI E0432).
#[cfg(target_os = "macos")]
use pasteboard::map_content_type_to_uti;

// ra15.1: dispatch handler groups + helper-method clusters extracted from the
// original ipc god-module. mod.rs keeps the core types, shared consts/helpers,
// and the thin dispatcher that routes to the per-domain handlers below.
mod builder;
mod connection;
mod handlers_config;
mod handlers_db;
mod handlers_items;
mod handlers_pairing;
mod handlers_peers;
mod handlers_status;
mod handlers_sync;
mod handlers_transfer;
mod pairing_ops;

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
    ///
    /// Always present (not cfg-gated): the in-band pairing path reads it
    /// unconditionally to advertise the account id to the peer (it is simply
    /// `None` without cloud-sync), so gating the field would break
    /// `--no-default-features`.
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

        // ra15.1: route to the per-domain handler chain. Each dispatch_*
        // returns a Response and falls through to the next domain via its
        // `_ =>` arm; the final dispatch_items_extra holds the unknown-method
        // fallback. Behaviour is identical to the original single match.
        self.dispatch_items(req).await
    }
}

#[cfg(test)]
mod tests;
