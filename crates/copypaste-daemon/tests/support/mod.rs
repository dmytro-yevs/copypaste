//! Reusable test-support for the e2e harness.
//!
//! Step A: launch ONE real `copypaste-daemon` subprocess in a fully isolated
//! environment (unique socket + db + every data/config/cache/log dir), wait
//! for its IPC socket to become ready, and send newline-delimited JSON-RPC
//! requests. The returned [`Daemon`] is an RAII guard: dropping it kills the
//! subprocess and removes its temp directory.
//!
//! Future test files reuse this via:
//!
//! ```ignore
//! #[path = "support/mod.rs"]
//! mod support;
//! use support::Daemon;
//! ```
//!
//! P2P is intentionally left OFF in this step (no `COPYPASTE_P2P` env var).

#![allow(dead_code)] // Helper methods are consumed by individual test files, not all at once.

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

/// How long to wait for the daemon's IPC socket to start accepting connections.
///
/// On the very first run against a login keychain, the daemon performs a
/// one-time Keychain ACL rotation that can add several seconds (and may prompt)
/// before the IPC listener binds, so this is generous on purpose.
const SOCKET_READY_TIMEOUT: Duration = Duration::from_secs(30);
/// Poll interval while waiting for the socket.
const SOCKET_POLL_INTERVAL: Duration = Duration::from_millis(50);
/// Per-request read timeout for IPC round-trips.
const IPC_READ_TIMEOUT: Duration = Duration::from_secs(5);

/// A running, isolated `copypaste-daemon` subprocess.
///
/// Owns a [`tempfile::TempDir`]; all daemon state lives inside it and is removed
/// on drop. The child process is killed and reaped on drop as well.
pub struct Daemon {
    child: Child,
    socket_path: PathBuf,
    // Kept alive for the lifetime of the daemon; removed on drop.
    _tmp_dir: tempfile::TempDir,
}

impl Daemon {
    /// Spawn a fresh daemon with a fully isolated environment and block until
    /// its IPC socket is ready to accept connections.
    ///
    /// Panics (failing the test) if the binary cannot be spawned or the socket
    /// does not come up within [`SOCKET_READY_TIMEOUT`].
    pub fn spawn() -> Self {
        let tmp_dir = tempfile::tempdir().expect("could not create temp dir for daemon");
        let root = tmp_dir.path();

        let socket_path = root.join("copypaste.sock");
        let db_path = root.join("clipboard.db");
        let data_dir = root.join("data");
        let config_dir = root.join("config");
        let cache_dir = root.join("cache");
        let log_dir = root.join("logs");
        let device_id_path = root.join("device_id");

        for dir in [&data_dir, &config_dir, &cache_dir, &log_dir] {
            std::fs::create_dir_all(dir).expect("could not create isolated daemon dir");
        }

        let child = Command::new(daemon_binary())
            .env("COPYPASTE_SOCKET", &socket_path)
            .env("COPYPASTE_DB", &db_path)
            .env("COPYPASTE_DATA_DIR", &data_dir)
            .env("COPYPASTE_CONFIG_DIR", &config_dir)
            .env("COPYPASTE_CACHE_DIR", &cache_dir)
            .env("COPYPASTE_LOG_DIR", &log_dir)
            .env("COPYPASTE_DEVICE_ID_PATH", &device_id_path)
            // Use an ephemeral in-memory encryption key so the daemon never
            // touches the macOS login Keychain: this avoids the password prompt
            // (ad-hoc-signed dev builds invalidate the Keychain ACL on every
            // rebuild) and the slow one-time ACL-rotation delay on cold start.
            .env("COPYPASTE_EPHEMERAL_KEY", "1")
            // Step A keeps P2P OFF: do not set COPYPASTE_P2P.
            .env("RUST_LOG", "error") // keep test output quiet
            .spawn()
            .expect(
                "failed to spawn copypaste-daemon — \
                 run `cargo build -p copypaste-daemon` first",
            );

        let daemon = Self {
            child,
            socket_path,
            _tmp_dir: tmp_dir,
        };

        assert!(
            wait_for_socket(&daemon.socket_path, SOCKET_READY_TIMEOUT),
            "daemon did not become ready within {SOCKET_READY_TIMEOUT:?} — \
             socket not found at {:?}",
            daemon.socket_path,
        );

        daemon
    }

    /// Path to this daemon's IPC Unix socket.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Send one newline-delimited JSON-RPC request and return the parsed
    /// response. Opens a fresh connection per call (mirrors how the production
    /// UI/CLI talk to the daemon).
    pub fn request(&self, request: &str) -> serde_json::Value {
        let mut stream =
            UnixStream::connect(&self.socket_path).expect("could not connect to daemon IPC socket");
        stream
            .set_read_timeout(Some(IPC_READ_TIMEOUT))
            .expect("could not set IPC read timeout");

        let mut payload = request.to_string();
        payload.push('\n');
        stream
            .write_all(payload.as_bytes())
            .expect("IPC write failed");

        let mut reader = BufReader::new(stream);
        let mut line = String::new();
        reader.read_line(&mut line).expect("IPC read failed");

        serde_json::from_str(line.trim()).expect("daemon response is not valid JSON")
    }
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        // `_tmp_dir` removes the entire isolated tree (socket, db, dirs) here.
    }
}

/// Locate the compiled `copypaste-daemon` binary.
///
/// Prefers Cargo's `CARGO_BIN_EXE_copypaste-daemon`; falls back to walking up
/// from the manifest dir to `target/{debug,release}/copypaste-daemon`.
fn daemon_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_copypaste-daemon") {
        return PathBuf::from(p);
    }

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("unexpected directory layout: cannot find workspace root");

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

/// Poll until a connection to `socket_path` succeeds or the timeout elapses.
fn wait_for_socket(socket_path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if UnixStream::connect(socket_path).is_ok() {
            return true;
        }
        thread::sleep(SOCKET_POLL_INTERVAL);
    }
    false
}
