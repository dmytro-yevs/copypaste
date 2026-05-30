//! Regression: keychain-locked / DB-unavailable degraded startup.
//!
//! LIVE-CONFIRMED bug — after the macOS app is reinstalled the daemon binary's
//! code signature changes, the Keychain ACL on the stored SQLCipher key no
//! longer trusts it, and the daemon either:
//!   * HUNG forever on a SecurityAgent GUI prompt (never bound the socket), or
//!   * (launchd) fell back to an ephemeral key, tried to open the EXISTING
//!     encrypted DB with the wrong key → "file is not a database" → EXITED.
//!
//! Both outcomes left no daemon and a blank UI. The fix makes startup ALWAYS
//! reach a defined state in bounded time and, when the key cannot open an
//! existing encrypted DB, come up DEGRADED: socket bound, `status` reports
//! `status="degraded"` + an accurate `degraded_reason`, DB-touching methods
//! return `IPC_NOT_READY`, and the existing DB file is left untouched.
//!
//! How this test forces the degraded path deterministically WITHOUT a real
//! Keychain: it runs the daemon with `COPYPASTE_EPHEMERAL_KEY=1` (so the key is
//! a fresh ephemeral key) AND pre-places a NON-EMPTY file at `COPYPASTE_DB`
//! that is not a valid SQLCipher database under that key. `Database::open` then
//! fails with SQLITE_NOTADB — exactly the "wrong key vs existing encrypted DB"
//! shape — and the daemon takes the degraded safety-net path instead of exiting.
//! Because the key WAS obtained (it is just the wrong one), the accurate reason
//! here is `db_key_mismatch`, NOT `keychain_locked` (which is reserved for the
//! key-UNREACHABLE case — a locked/denied Keychain read).

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const SOCKET_READY_TIMEOUT: Duration = Duration::from_secs(20);
const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(50);
const IPC_READ_TIMEOUT: Duration = Duration::from_secs(5);

fn daemon_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_copypaste-daemon") {
        return PathBuf::from(p);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root");
    let profile = if cfg!(debug_assertions) {
        "debug"
    } else {
        "release"
    };
    workspace_root
        .join("target")
        .join(profile)
        .join("copypaste-daemon")
}

fn wait_for_socket(socket_path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if UnixStream::connect(socket_path).is_ok() {
            return true;
        }
        std::thread::sleep(SOCKET_POLL_INTERVAL);
    }
    false
}

fn request(socket_path: &Path, payload: &str) -> serde_json::Value {
    let mut stream = UnixStream::connect(socket_path).expect("connect IPC socket");
    stream
        .set_read_timeout(Some(IPC_READ_TIMEOUT))
        .expect("set read timeout");
    let mut line = payload.to_string();
    line.push('\n');
    stream.write_all(line.as_bytes()).expect("write request");
    let mut reader = BufReader::new(stream);
    let mut resp = String::new();
    reader.read_line(&mut resp).expect("read response");
    serde_json::from_str(resp.trim()).expect("valid JSON response")
}

struct DegradedDaemon {
    child: Child,
    socket_path: PathBuf,
    db_path: PathBuf,
    _tmp: tempfile::TempDir,
}

impl Drop for DegradedDaemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// The corrupt-but-non-empty bytes we seed at `COPYPASTE_DB` to simulate an
/// existing encrypted DB the current key cannot open.
const PREEXISTING_DB_BYTES: &[u8] =
    b"this is NOT a valid sqlite database header - simulated wrong-key encrypted blob \x00\x01\x02";

fn spawn_degraded() -> DegradedDaemon {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();
    let socket_path = root.join("copypaste.sock");
    let db_path = root.join("clipboard.db");
    let data_dir = root.join("data");
    let config_dir = root.join("config");
    let cache_dir = root.join("cache");
    let log_dir = root.join("logs");
    let device_id_path = root.join("device_id");
    for dir in [&data_dir, &config_dir, &cache_dir, &log_dir] {
        std::fs::create_dir_all(dir).expect("create dir");
    }

    // Pre-place a non-empty file the daemon's key cannot open as a DB.
    std::fs::write(&db_path, PREEXISTING_DB_BYTES).expect("seed pre-existing db file");

    let mut cmd = Command::new(daemon_binary());
    cmd.env("COPYPASTE_SOCKET", &socket_path)
        .env("COPYPASTE_DB", &db_path)
        .env("COPYPASTE_DATA_DIR", &data_dir)
        .env("COPYPASTE_CONFIG_DIR", &config_dir)
        .env("COPYPASTE_CACHE_DIR", &cache_dir)
        .env("COPYPASTE_LOG_DIR", &log_dir)
        .env("COPYPASTE_DEVICE_ID_PATH", &device_id_path)
        .env("COPYPASTE_EPHEMERAL_KEY", "1")
        .env("RUST_LOG", "error");

    let child = cmd
        .spawn()
        .expect("spawn copypaste-daemon (run `cargo build -p copypaste-daemon` first)");

    DegradedDaemon {
        child,
        socket_path,
        db_path,
        _tmp: tmp,
    }
}

/// Acceptance criteria #1 + #2: the daemon must NOT hang and must NOT exit when
/// the key cannot open an existing encrypted DB — it binds the socket and
/// serves a degraded status in bounded time.
#[test]
fn degraded_startup_binds_socket_and_reports_db_key_mismatch() {
    let daemon = spawn_degraded();

    assert!(
        wait_for_socket(&daemon.socket_path, SOCKET_READY_TIMEOUT),
        "degraded daemon must bind its IPC socket (it must not hang or exit) — \
         socket never appeared at {:?}",
        daemon.socket_path
    );

    let resp = request(
        &daemon.socket_path,
        r#"{"id":"s1","method":"status","protocol_version":1}"#,
    );

    assert_eq!(
        resp["data"]["status"], "degraded",
        "status must be 'degraded', got: {resp}"
    );
    assert_eq!(
        resp["data"]["degraded_reason"], "db_key_mismatch",
        "degraded_reason must be the canonical 'db_key_mismatch' (key present but \
         wrong — NOT 'keychain_locked', which is for an unreachable key), got: {resp}"
    );
    assert_eq!(
        resp["data"]["degraded"], true,
        "degraded flag must be true, got: {resp}"
    );
}

/// A DB-touching method must be rejected with IPC_NOT_READY in degraded mode,
/// so the UI shows a recovery banner rather than getting stale/garbage data.
#[test]
fn degraded_startup_rejects_db_methods_with_not_ready() {
    let daemon = spawn_degraded();
    assert!(
        wait_for_socket(&daemon.socket_path, SOCKET_READY_TIMEOUT),
        "degraded daemon must bind its socket"
    );

    let resp = request(
        &daemon.socket_path,
        r#"{"id":"l1","method":"list","params":{"limit":10},"protocol_version":1}"#,
    );
    assert_eq!(
        resp["ok"], false,
        "DB-touching `list` must be rejected in degraded mode, got: {resp}"
    );
    // The error code constant is "IPC_NOT_READY"; assert it appears in the
    // error payload (code or message) without over-fitting the envelope shape.
    let blob = resp.to_string();
    assert!(
        blob.contains("IPC_NOT_READY"),
        "rejection must carry IPC_NOT_READY, got: {resp}"
    );
}

/// Acceptance criterion #3: the existing encrypted DB file must be left
/// byte-for-byte untouched in degraded mode — never overwritten/recreated — so
/// a later correct-key launch can still open it.
#[test]
fn degraded_startup_does_not_modify_existing_db_file() {
    let daemon = spawn_degraded();
    assert!(
        wait_for_socket(&daemon.socket_path, SOCKET_READY_TIMEOUT),
        "degraded daemon must bind its socket"
    );
    // Give the daemon a moment to (not) touch the file.
    std::thread::sleep(Duration::from_millis(300));

    let after = std::fs::read(&daemon.db_path).expect("db file still present");
    assert_eq!(
        after, PREEXISTING_DB_BYTES,
        "the pre-existing DB file must be left byte-for-byte untouched"
    );
}
