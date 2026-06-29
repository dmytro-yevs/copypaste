//! Integration test: settings set via `set_config` IPC must survive an
//! `IpcServer` restart — i.e. be durably persisted to disk, not just held in
//! the first server's in-memory state.
//!
//! ## What is tested
//!
//! Two fields with different persistence paths are covered:
//!
//! * **`sensitive_ttl_secs`** (`u64`, numeric) — written to `config.toml` by
//!   `update_core_config()` and read back from it on the next `get_config`.
//!   Default value is `30`; the test uses `9999` to ensure the assertion does
//!   not accidentally pass on a real user config.
//!
//! * **`p2p_enabled`** (`bool`) — written to `config.json` by `write_config()`
//!   and read back from it.  Default is absent (`None`) / effectively `true`;
//!   the test sets it to `false`.
//!
//! ## Isolation
//!
//! `HOME` and `COPYPASTE_CONFIG_DIR` are redirected to a `tempfile::TempDir`
//! for the duration of the test so no user files are touched.  The test holds
//! `copypaste_daemon::TEST_ENV_LOCK` for its entire duration via an RAII guard,
//! serialising it against any other env-mutating test in the binary (paths,
//! keychain, IPC inline tests, etc.).
//!
//! ## Harness
//!
//! Uses the in-process `IpcServer` (not a subprocess), matching the style of
//! `tests/health.rs`.  Two server instances are started sequentially against
//! the **same** temp dirs — server 1 writes, is cancelled, server 2 reads.

use std::sync::{atomic::AtomicBool, Arc};
use std::time::Duration;

use copypaste_core::Database;
use copypaste_daemon::ipc::IpcServer;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

// ── RAII env-var guard ────────────────────────────────────────────────────────

/// Process-wide env lock for this test binary.
///
/// `TEST_ENV_LOCK` in `copypaste_daemon::lib.rs` is gated on `#[cfg(test)]`,
/// which only activates when the daemon lib itself is under test — NOT when it
/// is compiled as a dependency of this integration-test binary. A local static
/// is sufficient here because each `[[test]]` entry is its own binary process,
/// so there is no cross-binary sharing of OS env state to guard against.
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Snapshot a set of env vars, redirect them all to `value`, and restore on
/// drop.  Holds [`ENV_LOCK`] for its whole lifetime so no other env-mutating
/// test in this binary can race it.
struct EnvGuard {
    saved: Vec<(&'static str, Option<std::ffi::OsString>)>,
    // Kept alive to hold the mutex for the struct's lifetime.
    #[allow(dead_code)]
    lock: std::sync::MutexGuard<'static, ()>,
}

impl EnvGuard {
    fn redirect_all(keys: &[&'static str], value: &std::path::Path) -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|p| p.into_inner());
        let mut saved = Vec::with_capacity(keys.len());
        for &key in keys {
            saved.push((key, std::env::var_os(key)));
            // SAFETY: serialised via ENV_LOCK; no other thread in this binary
            // reads or writes these vars concurrently while the guard is alive.
            unsafe { std::env::set_var(key, value) };
        }
        Self { saved, lock }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        // SAFETY: TEST_ENV_LOCK is still held by `self.lock`.
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

// ── IPC helpers ───────────────────────────────────────────────────────────────

/// Open a fresh connection to `sock`, send one newline-delimited JSON request,
/// and return the parsed JSON response.
async fn ipc_call(sock: &std::path::Path, payload: &str) -> serde_json::Value {
    let mut stream = UnixStream::connect(sock)
        .await
        .expect("connect to test IPC socket");
    let mut msg = payload.to_string();
    msg.push('\n');
    stream.write_all(msg.as_bytes()).await.expect("IPC write");

    let mut lines = BufReader::new(&mut stream).lines();
    let line = lines
        .next_line()
        .await
        .expect("IPC read")
        .expect("server closed connection without a response");
    serde_json::from_str(&line).expect("parse IPC response JSON")
}

/// Bind a minimal in-process `IpcServer` on `sock_path` and start it in a
/// background task.  Returns the `CancellationToken` so the caller can shut
/// the server down cleanly before starting the next one.
///
/// Uses `UnixListener::bind` directly (instead of `IpcServer::bind`) to avoid
/// the `libc::umask(0o177)` side-effect that corrupts concurrent tempdir
/// permissions — matches the pattern in `ipc/tests.rs`.
async fn boot_server(sock_path: &std::path::Path) -> CancellationToken {
    let db = Arc::new(Mutex::new(
        Database::open_in_memory().expect("in-memory DB"),
    ));
    let private_mode = Arc::new(AtomicBool::new(false));
    let local_key = Arc::new(zeroize::Zeroizing::new([0u8; 32]));
    let device_pub = Arc::new([0u8; 32]);
    let server = IpcServer::new(db, private_mode, local_key, device_pub);

    let listener = tokio::net::UnixListener::bind(sock_path).expect("test IpcServer socket bind");
    let cancel = CancellationToken::new();
    let cancel_for_task = cancel.clone();
    tokio::spawn(async move {
        let _ = server.serve_on(listener, cancel_for_task).await;
    });
    // Brief pause so the accept loop is listening before the first IPC call.
    tokio::time::sleep(Duration::from_millis(50)).await;
    cancel
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// A numeric field (`sensitive_ttl_secs`) and a bool field (`p2p_enabled`) set
/// via `set_config` must be readable via `get_config` on a **fresh** `IpcServer`
/// instance that was started against the same config directory.
///
/// This exercises the disk-persistence path end-to-end:
///
///   set_config → write_config (config.json) + update_core_config (config.toml)
///     → daemon restart (new IpcServer, no shared in-memory state)
///       → get_config → read_config (reads both files fresh from disk)
#[tokio::test]
async fn set_config_numeric_and_bool_persist_across_ipc_restart() {
    // ── 0. Redirect all config I/O to an isolated temp dir ───────────────────
    //
    // HOME   → controls config.toml path via app_support_dir() on macOS
    //          (~/Library/Application Support/CopyPaste/config.toml) and
    //          ~/.local/share/copypaste/config.toml on Linux.
    // COPYPASTE_CONFIG_DIR → controls config.json path directly.
    // XDG_CONFIG_HOME      → overrides dirs::config_dir() on Linux so the
    //                         legacy config path in read_config falls under
    //                         the temp dir too.
    // XDG_DATA_HOME        → overrides dirs::data_dir() on Linux, redirecting
    //                         app_support_dir() on non-macOS platforms.
    let dir = tempfile::tempdir().expect("tempdir for settings persistence test");
    let cfg_root = dir.path().to_path_buf();
    let _env = EnvGuard::redirect_all(
        &[
            "HOME",
            "COPYPASTE_CONFIG_DIR",
            "XDG_CONFIG_HOME",
            "XDG_DATA_HOME",
        ],
        &cfg_root,
    );

    // ── 1. Server 1: write non-default values and wait for the ack ───────────
    let sock1 = cfg_root.join("srv1.sock");
    let cancel1 = boot_server(&sock1).await;

    let set_resp = ipc_call(
        &sock1,
        // sensitive_ttl_secs = 9999 → goes to config.toml (non-default; default is 30)
        // p2p_enabled = false      → goes to config.json (non-default; default is absent/true)
        r#"{"id":"set1","method":"set_config","params":{"sensitive_ttl_secs":9999,"p2p_enabled":false}}"#,
    )
    .await;
    assert_eq!(
        set_resp["ok"],
        serde_json::json!(true),
        "set_config must succeed on server 1: {set_resp}"
    );

    // Sanity-check: same server round-trip should already reflect the values
    // (both in-memory hot-reload and fresh disk read).
    let get1 = ipc_call(&sock1, r#"{"id":"get1","method":"get_config"}"#).await;
    assert_eq!(
        get1["ok"],
        serde_json::json!(true),
        "get_config on server 1 must succeed: {get1}"
    );
    assert_eq!(
        get1["data"]["sensitive_ttl_secs"],
        serde_json::json!(9999),
        "server 1 in-session: sensitive_ttl_secs must be 9999 immediately after set: {get1}"
    );
    assert_eq!(
        get1["data"]["p2p_enabled"],
        serde_json::json!(false),
        "server 1 in-session: p2p_enabled must be false immediately after set: {get1}"
    );

    // Shut down server 1.  After cancel() the accept loop exits; any in-flight
    // blocking tasks (write_config / update_core_config) have already completed
    // because set_config awaited their result before sending "ok".
    cancel1.cancel();
    tokio::time::sleep(Duration::from_millis(50)).await;

    // ── 2. Server 2: fresh instance, same config dirs, must read from disk ───
    //
    // Server 2 shares NO in-memory state with server 1.  Its get_config will
    // call read_config() which reads config.toml + config.json cold from disk.
    let sock2 = cfg_root.join("srv2.sock");
    let _cancel2 = boot_server(&sock2).await;

    let get2 = ipc_call(&sock2, r#"{"id":"get2","method":"get_config"}"#).await;
    assert_eq!(
        get2["ok"],
        serde_json::json!(true),
        "get_config on server 2 (after restart) must succeed: {get2}"
    );

    // ── 3. Persistence assertions ─────────────────────────────────────────────
    //
    // These are the core invariants: values written by server 1 must survive
    // the restart and be read back by server 2 from disk.

    // Numeric field — persisted to config.toml via update_core_config().
    assert_eq!(
        get2["data"]["sensitive_ttl_secs"],
        serde_json::json!(9999),
        "sensitive_ttl_secs (u64, config.toml) must persist across IpcServer restart: {get2}"
    );

    // Bool field — persisted to config.json via write_config().
    assert_eq!(
        get2["data"]["p2p_enabled"],
        serde_json::json!(false),
        "p2p_enabled (bool, config.json) must persist across IpcServer restart: {get2}"
    );
}
