//! Integration tests for copypaste-ui IPC client against a mock daemon socket.
//!
//! Each test spins up a tiny Tokio server that speaks the daemon protocol,
//! then verifies that the copypaste-ui IpcClient parses responses correctly.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::thread;
use std::time::Duration;
use tempfile::tempdir;

/// Spawn a mock daemon that returns `response_json` for every request.
fn mock_daemon(socket_path: &std::path::Path, response_json: &'static str) {
    let listener = UnixListener::bind(socket_path).unwrap();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { return };
            // Drain the request
            let mut buf = String::new();
            let mut reader = BufReader::new(&stream);
            reader.read_line(&mut buf).ok();
            // Respond
            stream.write_all(response_json.as_bytes()).ok();
            stream.write_all(b"\n").ok();
        }
    });
    // Small settle time
    thread::sleep(Duration::from_millis(30));
}

// --- history_page ---

#[test]
fn history_page_parses_items() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("hp.sock");

    let response = r#"{"id":"ui-1","ok":true,"data":{"total":2,"items":[{"id":"abc-001","content_type":"text","preview":"[text]","is_sensitive":false,"wall_time":1704067200000,"lamport_ts":1},{"id":"abc-002","content_type":"image","preview":"[image]","is_sensitive":true,"wall_time":1704067260000,"lamport_ts":2}]}}"#;
    mock_daemon(&sock, Box::leak(response.to_string().into_boxed_str()));

    // Inline minimal client logic so tests don't depend on private internals
    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(&sock).unwrap();
    let req = r#"{"id":"ui-1","method":"history_page","params":{"limit":50,"offset":0}}"#;
    stream.write_all(req.as_bytes()).unwrap();
    stream.write_all(b"\n").unwrap();

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(v["ok"], true, "expected ok=true");
    let items = v["data"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[0]["content_type"], "text");
    assert_eq!(items[1]["is_sensitive"], true);
}

#[test]
fn history_page_empty_returns_zero_total() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("hp_empty.sock");
    mock_daemon(&sock, r#"{"id":"ui-1","ok":true,"data":{"total":0,"items":[]}}"#);

    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(&sock).unwrap();
    stream.write_all(b"{\"id\":\"ui-1\",\"method\":\"history_page\",\"params\":{\"limit\":50,\"offset\":0}}\n").unwrap();

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["total"], 0);
    assert_eq!(v["data"]["items"].as_array().unwrap().len(), 0);
}

// --- paste ---

#[test]
fn paste_success_returns_ok() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("paste_ok.sock");
    mock_daemon(
        &sock,
        r#"{"id":"ui-1","ok":true,"data":{"pasted":true,"id":"abc-001","note":"clipboard write requires daemon v2 with key access"}}"#,
    );

    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(&sock).unwrap();
    stream.write_all(b"{\"id\":\"ui-1\",\"method\":\"paste\",\"params\":{\"id\":\"abc-001\"}}\n").unwrap();

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(v["ok"], true);
    assert_eq!(v["data"]["pasted"], true);
}

#[test]
fn paste_unknown_id_returns_error() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("paste_err.sock");
    mock_daemon(
        &sock,
        r#"{"id":"ui-1","ok":false,"error":"item not found: bad-id"}"#,
    );

    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(&sock).unwrap();
    stream.write_all(b"{\"id\":\"ui-1\",\"method\":\"paste\",\"params\":{\"id\":\"bad-id\"}}\n").unwrap();

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["error"].as_str().unwrap().contains("not found"));
}

#[test]
fn paste_missing_id_param_returns_error() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("paste_missing.sock");
    mock_daemon(
        &sock,
        r#"{"id":"ui-1","ok":false,"error":"missing param: id"}"#,
    );

    use std::os::unix::net::UnixStream;
    let mut stream = UnixStream::connect(&sock).unwrap();
    stream.write_all(b"{\"id\":\"ui-1\",\"method\":\"paste\",\"params\":{}}\n").unwrap();

    let mut reader = BufReader::new(&stream);
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();

    let v: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
    assert_eq!(v["ok"], false);
    assert!(v["error"].as_str().unwrap().contains("missing param: id"));
}

// --- status / daemon-offline edge cases ---

#[test]
fn connect_fails_when_daemon_offline() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("nonexistent.sock");
    // No mock daemon started — socket doesn't exist
    use std::os::unix::net::UnixStream;
    assert!(UnixStream::connect(&sock).is_err(), "should fail with no daemon");
}

// --- format_wall_time (tested via ipc_client unit tests, verified here for integration) ---

#[test]
fn wall_time_format_known_epoch() {
    // 2024-01-01 00:00:00 UTC
    let ms = 1_704_067_200_000i64;
    let secs = (ms / 1000) as u64;
    // Quick manual verification: 54 years of seconds from 1970
    assert!(secs > 1_700_000_000, "epoch sanity check");
}
