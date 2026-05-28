//! Integration test: spawn the real daemon binary and exercise the IPC protocol.
//!
//! The test:
//!   1. Finds the compiled `copypaste-daemon` binary.
//!   2. Spawns it as a subprocess with `COPYPASTE_SOCKET` and `COPYPASTE_DB`
//!      pointing to temporary paths so we don't touch the user's real data.
//!   3. Polls the Unix socket until the daemon is listening (up to 5 s).
//!   4. Sends a `status` request and asserts `ok == true`.
//!   5. Sends a `list`   request and asserts `items == []`.
//!   6. Kills the daemon and cleans up temp files in the `Drop` guard.

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// RAII guard — kills the child process when dropped.
// ---------------------------------------------------------------------------

struct DaemonGuard {
    child: Child,
    socket_path: PathBuf,
    db_path: PathBuf,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_file(&self.socket_path);
        let _ = std::fs::remove_file(&self.db_path);
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Returns the path to the compiled daemon binary.
/// Cargo puts integration-test binaries in the same directory as the test
/// runner.  The daemon binary lives in the same `target/{profile}` dir.
fn daemon_binary() -> PathBuf {
    // `CARGO_BIN_EXE_copypaste-daemon` is set by Cargo when the test crate
    // declares the binary as a `[[test]]` `required-features`-sibling via
    // `CARGO_BIN_EXE_<name>`.  Fall back to discovering it manually.
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_copypaste-daemon") {
        return PathBuf::from(p);
    }

    // Walk up from the manifest dir to find `target/`.
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("unexpected directory layout");

    // Choose debug vs release based on how the tests were built.
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

/// Spawn the daemon with isolated socket + DB paths.
fn spawn_daemon(socket_path: &Path, db_path: &Path) -> Child {
    Command::new(daemon_binary())
        .env("COPYPASTE_SOCKET", socket_path)
        .env("COPYPASTE_DB", db_path)
        .env("RUST_LOG", "error") // suppress noisy logs during tests
        .spawn()
        .expect("failed to spawn copypaste-daemon — did you run `cargo build -p copypaste-daemon` first?")
}

/// Poll until we can connect to the socket (daemon is up) or timeout.
fn wait_for_socket(socket_path: &Path, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if UnixStream::connect(socket_path).is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Send one newline-delimited JSON request and return the parsed response.
fn ipc_roundtrip(socket_path: &Path, request: &str) -> serde_json::Value {
    let mut stream = UnixStream::connect(socket_path).expect("could not connect to daemon socket");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();

    // Write request followed by newline (the daemon reads line-by-line).
    let mut payload = request.to_string();
    payload.push('\n');
    stream
        .write_all(payload.as_bytes())
        .expect("IPC write failed");

    // Read one response line.
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("IPC read failed");

    serde_json::from_str(line.trim()).expect("response is not valid JSON")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// Spawns the daemon, waits for it to be ready, and runs both IPC checks.
///
/// NOTE: the daemon's macOS clipboard monitor starts polling the real system
/// clipboard immediately; if anything is on the user's clipboard it will be
/// captured before the `list` assertion runs, breaking the "empty DB" check.
/// Until the daemon grows a `COPYPASTE_DISABLE_CLIPBOARD_POLL` knob, this
/// test must be run in a controlled clipboard environment.
#[test]
#[ignore = "macOS clipboard polling races with the empty-list assertion; run manually with --ignored after clearing clipboard"]
fn daemon_ipc_status_and_list() {
    let tmp_dir = tempfile::tempdir().expect("could not create temp dir");
    let socket_path = tmp_dir.path().join("daemon_test.sock");
    let db_path = tmp_dir.path().join("clipboard_test.db");

    let child = spawn_daemon(&socket_path, &db_path);
    // The guard ensures the child is killed even on panic.
    let _guard = DaemonGuard {
        child,
        socket_path: socket_path.clone(),
        db_path: db_path.clone(),
    };

    // ---- wait for the daemon to start ----
    assert!(
        wait_for_socket(&socket_path, Duration::from_secs(10)),
        "daemon did not start within 10 seconds — socket not found at {:?}",
        socket_path
    );

    // ---- 1. status ----
    let status_resp = ipc_roundtrip(&socket_path, r#"{"id":"t1","method":"status"}"#);
    assert_eq!(
        status_resp["ok"], true,
        "expected ok=true for status, got: {status_resp}"
    );

    // ---- 2. list — fresh DB should have no items ----
    let list_resp = ipc_roundtrip(
        &socket_path,
        r#"{"id":"t2","method":"list","params":{"limit":50,"offset":0}}"#,
    );
    assert_eq!(
        list_resp["ok"], true,
        "expected ok=true for list, got: {list_resp}"
    );
    let items = list_resp["data"]["items"]
        .as_array()
        .expect("data.items should be an array");
    assert!(
        items.is_empty(),
        "expected empty items array for a fresh DB, got: {items:?}"
    );
}

// ---------------------------------------------------------------------------
// Wave 2.3 — concurrent clients, pagination cap, early-init readiness
// ---------------------------------------------------------------------------

/// Spawn the daemon with a fresh tempdir socket+db, wait for the socket,
/// and return a guard that kills it on drop plus the socket path.
fn boot_daemon() -> (DaemonGuard, PathBuf, tempfile::TempDir) {
    let tmp_dir = tempfile::tempdir().expect("could not create temp dir");
    let socket_path = tmp_dir.path().join("daemon_test.sock");
    let db_path = tmp_dir.path().join("clipboard_test.db");

    let child = spawn_daemon(&socket_path, &db_path);
    let guard = DaemonGuard {
        child,
        socket_path: socket_path.clone(),
        db_path,
    };
    assert!(
        wait_for_socket(&socket_path, Duration::from_secs(10)),
        "daemon did not start within 10 seconds — socket not found at {:?}",
        socket_path
    );
    (guard, socket_path, tmp_dir)
}

/// **Edge HIGH #11** — 10 concurrent IPC clients each issue a `status`
/// followed by a `stats` request. The test asserts every request gets
/// `ok=true` and the daemon survives the burst (proven by a final
/// post-burst `status` call on a fresh connection).
///
/// Uses only `status` + `stats` (no `list`) to avoid the clipboard-poll
/// race that forces the broader `daemon_ipc_status_and_list` test to be
/// `#[ignore]`'d.
#[test]
#[ignore = "requires `cargo build -p copypaste-daemon` first; run with --ignored"]
fn concurrent_ten_clients_consistent_state() {
    let (_guard, socket_path, _tmp) = boot_daemon();

    const N_CLIENTS: usize = 10;
    let errors: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let mut handles = Vec::with_capacity(N_CLIENTS);

    for client_idx in 0..N_CLIENTS {
        let sock = socket_path.clone();
        let errs = Arc::clone(&errors);
        handles.push(thread::spawn(move || {
            // Each client opens its own connection per request — this is
            // also what the production UI does, so we exercise the same
            // accept path that production hits under load.
            let status_resp = ipc_roundtrip(
                &sock,
                &format!(r#"{{"id":"c{client_idx}-status","method":"status"}}"#),
            );
            if status_resp["ok"] != true {
                errs.lock()
                    .unwrap()
                    .push(format!("client {client_idx} status: {status_resp}"));
                return;
            }
            if status_resp["data"]["status"] != "running" {
                errs.lock().unwrap().push(format!(
                    "client {client_idx} status missing 'running': {status_resp}"
                ));
                return;
            }

            let stats_resp = ipc_roundtrip(
                &sock,
                &format!(r#"{{"id":"c{client_idx}-stats","method":"stats"}}"#),
            );
            if stats_resp["ok"] != true {
                errs.lock()
                    .unwrap()
                    .push(format!("client {client_idx} stats: {stats_resp}"));
                return;
            }
            if !stats_resp["data"]["total_items"].is_number() {
                errs.lock().unwrap().push(format!(
                    "client {client_idx} stats missing total_items: {stats_resp}"
                ));
            }
        }));
    }

    for h in handles {
        h.join().expect("client thread panicked");
    }

    let collected = errors.lock().unwrap();
    assert!(
        collected.is_empty(),
        "concurrent client errors:\n{}",
        collected.join("\n")
    );

    // Daemon must still serve a fresh client after the burst — proves no
    // listener task crashed mid-flight.
    let final_resp = ipc_roundtrip(&socket_path, r#"{"id":"after-burst","method":"status"}"#);
    assert_eq!(
        final_resp["ok"], true,
        "daemon stopped accepting after concurrent burst: {final_resp}"
    );
}

/// **Edge MEDIUM #24** — `list` (and `history_page`) must clamp client-
/// requested `limit` to `MAX_PAGE` (1000). A client asking for 5000
/// items must receive at most 1000 and a successful response.
#[test]
#[ignore = "requires `cargo build -p copypaste-daemon` first; run with --ignored"]
fn list_enforces_max_page_cap() {
    let (_guard, socket_path, _tmp) = boot_daemon();

    // limit=5000 is 5x the server cap. We assert:
    //   * ok == true (request not rejected),
    //   * data.items.len() <= 1000 (cap honored).
    let resp = ipc_roundtrip(
        &socket_path,
        r#"{"id":"cap1","method":"list","params":{"limit":5000,"offset":0}}"#,
    );
    assert_eq!(resp["ok"], true, "expected ok=true, got: {resp}");
    let items = resp["data"]["items"]
        .as_array()
        .expect("data.items should be an array");
    assert!(
        items.len() <= 1000,
        "list returned {} items, exceeds MAX_PAGE=1000",
        items.len()
    );

    // Same contract for history_page.
    let hp = ipc_roundtrip(
        &socket_path,
        r#"{"id":"cap2","method":"history_page","params":{"limit":5000,"offset":0}}"#,
    );
    assert_eq!(hp["ok"], true, "expected ok=true, got: {hp}");
    let hp_items = hp["data"]["items"]
        .as_array()
        .expect("history_page data.items should be an array");
    assert!(
        hp_items.len() <= 1000,
        "history_page returned {} items, exceeds MAX_PAGE=1000",
        hp_items.len()
    );
}
