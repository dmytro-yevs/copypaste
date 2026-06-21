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
    /// The daemon's `COPYPASTE_CONFIG_DIR`; `peers.json` lives directly under it
    /// (see `peers_file_path()` in the daemon, which honours this env override).
    config_dir: PathBuf,
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
        Self::spawn_inner(false)
    }

    /// Spawn a fresh isolated daemon with the P2P subsystem ENABLED
    /// (`COPYPASTE_P2P=1`), so the mTLS transport, cert, and the network
    /// bootstrap pairing channel are live. Used by the network-pairing tests.
    pub fn spawn_with_p2p() -> Self {
        Self::spawn_inner(true)
    }

    fn spawn_inner(p2p: bool) -> Self {
        let tmp_dir = tempfile::tempdir().expect("could not create temp dir for daemon");
        let root = tmp_dir.path();

        // Harden the root tempdir to 0o700 immediately after creation.
        //
        // Rationale: the daemon's `bind_with_stale_cleanup` temporarily sets
        // `umask(0o177)` (process-wide) so the Unix socket is created at 0o600
        // with no TOCTOU window. If a unit test in this test binary runs that
        // path concurrently with the `tempfile::tempdir()` / `create_dir_all`
        // calls here, those directories can be created as 0o600 (no exec bit),
        // causing the daemon subprocess — or the test's own `read_peers_json` —
        // to fail with EACCES. Explicitly chmoding to 0o700 after creation
        // removes the dependency on umask and makes the test hermetic on macOS
        // and Linux regardless of what other parallel tests are doing.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(root, std::fs::Permissions::from_mode(0o700));
        }

        let socket_path = root.join("copypaste.sock");
        let db_path = root.join("clipboard.db");
        let data_dir = root.join("data");
        let config_dir = root.join("config");
        let cache_dir = root.join("cache");
        let log_dir = root.join("logs");
        let device_id_path = root.join("device_id");

        for dir in [&data_dir, &config_dir, &cache_dir, &log_dir] {
            std::fs::create_dir_all(dir).expect("could not create isolated daemon dir");
            // Ensure each subdirectory is also 0o700 for the same reason.
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
            }
        }

        let mut cmd = Command::new(daemon_binary());
        cmd.env("COPYPASTE_SOCKET", &socket_path)
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
            .env("RUST_LOG", "error"); // keep test output quiet

        if p2p {
            cmd.env("COPYPASTE_P2P", "1");
        }

        let child = cmd.spawn().expect(
            "failed to spawn copypaste-daemon — \
                 run `cargo build -p copypaste-daemon` first",
        );

        let daemon = Self {
            child,
            socket_path,
            config_dir,
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

    /// Path to this daemon's `peers.json`. The daemon writes peers.json
    /// directly under `COPYPASTE_CONFIG_DIR` (when set), with no extra
    /// subdirectory — `peers_file_path()` delegates to `paths::config_dir()`
    /// which returns the override value as-is.
    pub fn peers_json_path(&self) -> PathBuf {
        self.config_dir.join("peers.json")
    }

    /// Read and parse this daemon's `peers.json`, returning an empty array if it
    /// does not exist yet.
    pub fn read_peers_json(&self) -> serde_json::Value {
        match std::fs::read_to_string(self.peers_json_path()) {
            Ok(s) => serde_json::from_str(&s).expect("peers.json must be valid JSON"),
            Err(_) => serde_json::json!([]),
        }
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
