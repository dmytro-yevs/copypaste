//! Integration test: exercise the `vacuum` IPC verb over the live daemon socket.
//!
//! Tests spawn the real `copypaste-daemon` binary (build it first with
//! `cargo build -p copypaste-daemon`), issue `vacuum` requests over the Unix
//! socket, and assert the response shape.
//!
//! All tests are `#[ignore]` for the same reason as `integration_ipc.rs`:
//! they require the binary to be pre-built by CI or the developer.

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    process::{Child, Command},
    thread,
    time::{Duration, Instant},
};

// ---------------------------------------------------------------------------
// RAII guard — kills the child process when dropped.
// ---------------------------------------------------------------------------

struct DaemonGuard {
    child: Child,
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

// ---------------------------------------------------------------------------
// Helpers (mirrors integration_ipc.rs)
// ---------------------------------------------------------------------------

fn daemon_binary() -> PathBuf {
    if let Ok(p) = std::env::var("CARGO_BIN_EXE_copypaste-daemon") {
        return PathBuf::from(p);
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .parent() // crates/
        .and_then(|p| p.parent()) // workspace root
        .expect("unexpected directory layout");
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

fn spawn_daemon(socket_path: &Path, db_path: &Path) -> Child {
    Command::new(daemon_binary())
        .env("COPYPASTE_SOCKET", socket_path)
        .env("COPYPASTE_DB", db_path)
        // COPYPASTE_EPHEMERAL_KEY avoids interactive macOS Keychain prompts.
        .env("COPYPASTE_EPHEMERAL_KEY", "1")
        .env("RUST_LOG", "error")
        .spawn()
        .expect(
            "failed to spawn copypaste-daemon — run \
             `cargo build -p copypaste-daemon` first",
        )
}

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

fn ipc_roundtrip(socket_path: &Path, request: &str) -> serde_json::Value {
    let mut stream = UnixStream::connect(socket_path).expect("could not connect to daemon socket");
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .unwrap();
    let mut payload = request.to_string();
    payload.push('\n');
    stream
        .write_all(payload.as_bytes())
        .expect("IPC write failed");
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    reader.read_line(&mut line).expect("IPC read failed");
    serde_json::from_str(line.trim()).expect("response is not valid JSON")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// `vacuum` with default params (empty `{}`) must return `ok=true` and the
/// four numeric fields (`size_before`, `size_after`, `reclaimed`) on a fresh
/// daemon DB.
#[test]
#[ignore = "requires `cargo build -p copypaste-daemon` first; run with --ignored"]
fn vacuum_default_returns_ok_with_size_fields() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let socket_path = tmp.path().join("vac_test.sock");
    let db_path = tmp.path().join("vac_test.db");

    let child = spawn_daemon(&socket_path, &db_path);
    let _guard = DaemonGuard { child };

    assert!(
        wait_for_socket(&socket_path, Duration::from_secs(15)),
        "daemon did not start within 15 s"
    );

    // Issue vacuum with empty params — should run VACUUM + REINDEX.
    let resp = ipc_roundtrip(
        &socket_path,
        r#"{"id":"vac1","method":"vacuum","params":{}}"#,
    );

    assert_eq!(resp["ok"], true, "vacuum must return ok=true: {resp}");

    let data = &resp["data"];
    assert!(
        data["size_before"].is_number(),
        "size_before must be numeric: {resp}"
    );
    assert!(
        data["size_after"].is_number(),
        "size_after must be numeric: {resp}"
    );
    assert!(
        data["reclaimed"].is_number(),
        "reclaimed must be numeric: {resp}"
    );
    // On a fresh empty DB, reclaimed >= 0 (VACUUM can't shrink below a minimum
    // page count, but must not report a logic error).
    let size_before = data["size_before"].as_u64().unwrap();
    let size_after = data["size_after"].as_u64().unwrap();
    let reclaimed = data["reclaimed"].as_i64().unwrap();
    assert_eq!(
        reclaimed,
        size_before as i64 - size_after as i64,
        "reclaimed must equal size_before - size_after"
    );
}

/// `vacuum` with `dry_run=true` must report size but NOT mutate the database.
/// We verify by checking that a second `dry_run` call returns the same
/// `size_before` (immutable file).
#[test]
#[ignore = "requires `cargo build -p copypaste-daemon` first; run with --ignored"]
fn vacuum_dry_run_does_not_change_size() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let socket_path = tmp.path().join("vac_dry.sock");
    let db_path = tmp.path().join("vac_dry.db");

    let child = spawn_daemon(&socket_path, &db_path);
    let _guard = DaemonGuard { child };

    assert!(
        wait_for_socket(&socket_path, Duration::from_secs(15)),
        "daemon did not start within 15 s"
    );

    let r1 = ipc_roundtrip(
        &socket_path,
        r#"{"id":"dry1","method":"vacuum","params":{"dry_run":true}}"#,
    );
    let r2 = ipc_roundtrip(
        &socket_path,
        r#"{"id":"dry2","method":"vacuum","params":{"dry_run":true}}"#,
    );

    assert_eq!(r1["ok"], true, "first dry-run: {r1}");
    assert_eq!(r2["ok"], true, "second dry-run: {r2}");

    // `size_before` must be the same on both calls — dry-run must not write.
    assert_eq!(
        r1["data"]["size_before"], r2["data"]["size_before"],
        "dry-run must not change the file size between calls"
    );
    // `size_before == size_after` for a dry-run.
    assert_eq!(
        r1["data"]["size_before"], r1["data"]["size_after"],
        "dry-run must report size_after == size_before"
    );
}

/// `vacuum` with `reindex_only=true` must succeed and return the expected
/// shape. REINDEX does not shrink the file, so `size_after >= size_before`
/// (it can only stay flat or grow slightly).
#[test]
#[ignore = "requires `cargo build -p copypaste-daemon` first; run with --ignored"]
fn vacuum_reindex_only_succeeds() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let socket_path = tmp.path().join("vac_reindex.sock");
    let db_path = tmp.path().join("vac_reindex.db");

    let child = spawn_daemon(&socket_path, &db_path);
    let _guard = DaemonGuard { child };

    assert!(
        wait_for_socket(&socket_path, Duration::from_secs(15)),
        "daemon did not start within 15 s"
    );

    let resp = ipc_roundtrip(
        &socket_path,
        r#"{"id":"ri1","method":"vacuum","params":{"reindex_only":true}}"#,
    );

    assert_eq!(resp["ok"], true, "reindex_only must succeed: {resp}");
    assert!(
        resp["data"]["size_before"].is_number(),
        "size_before must be present: {resp}"
    );
    // The daemon must still accept further requests after REINDEX.
    let status = ipc_roundtrip(&socket_path, r#"{"id":"ri-after","method":"status"}"#);
    assert_eq!(
        status["ok"], true,
        "daemon must remain healthy after reindex: {status}"
    );
}
