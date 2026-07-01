//! IPC tuning constants + degraded-startup reason strings (split from
//! ipc/mod.rs, ADR-017 daemon-ipc track, CopyPaste-vp63.19).
//!
//! Visibility is preserved EXACTLY as it was when these were declared
//! directly in mod.rs: `BUILD_VERSION`, `IPC_READ_TIMEOUT`,
//! `IPC_WRITE_TIMEOUT`, `PEER_EVENT_QUEUE_CAP`, `DEGRADED_REASON_*` are `pub`
//! (re-exported by mod.rs so `crate::ipc::X` keeps working); the rest were
//! bare (module-private) `const`s and are declared `pub(super)` here so
//! mod.rs's private re-import keeps them visible ONLY within the `ipc`
//! module and its descendants (unchanged reachability — do NOT widen).

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
pub(super) const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Default per-request size cap applied to EVERY method that is not on the
/// large-payload allow-list ([`crate::ipc::IpcServer::allows_large_payload`]).
///
/// CopyPaste-c4q2.28: applying the 16 MiB [`MAX_REQUEST_BYTES`] cap to every
/// method let a hostile same-UID client send ~15.9 MiB for a `status`/`list`/
/// `delete` call; the daemon buffered the whole payload before rejecting it.
/// Worst case `MAX_CONCURRENT_CONNECTIONS` × 16 MiB ≈ 1 GiB of peak buffered
/// RAM. The IPC read path now reads at most this many bytes before it has
/// classified the method, and only `import` / `add_file_item` are allowed to
/// grow to [`MAX_REQUEST_BYTES`]. 64 KiB is comfortably larger than any
/// non-bulk request (the largest, a fully-populated `set_config`, is < 2 KiB).
pub(super) const SMALL_REQUEST_BYTES: usize = 64 * 1024;

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
/// the lifetime of the daemon. With `MAX_CONCURRENT_CONNECTIONS` such clients,
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
pub(super) const MAX_PAGE: usize = 1000;

/// Per-item ceiling on `import` payloads (decoded `content_bytes_b64` length).
/// Larger items are rejected with `invalid_argument` BEFORE storage so a
/// malformed or hostile export cannot exhaust memory / disk on the daemon.
/// 4 MiB matches the practical upper bound for clipboard text/image payloads
/// we round-trip today; bumping this requires re-evaluating SQLite blob limits.
pub(super) const MAX_IMPORT_ITEM_BYTES: usize = 4 * 1024 * 1024;

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
pub(super) const MAX_CONCURRENT_CONNECTIONS: usize = 64;

/// Human-readable `error` message returned when an IPC method is called before
/// the server's backing state (database, etc.) has finished initializing.
/// Clients branch on the machine-readable `error_code` (`ERR_CODE_IPC_NOT_READY`
/// = "ipc_not_ready") and should back off and retry rather than treat this as a
/// hard failure. CopyPaste-crh3.8: this is the user-facing string, so it is a
/// real sentence rather than the bare Rust constant name "IPC_NOT_READY".
pub(super) const ERR_IPC_NOT_READY: &str = "daemon is still starting up; retry shortly";

/// Maximum number of [`crate::ipc::PeerEventRecord`]s held in the IPC queue between polls.
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
