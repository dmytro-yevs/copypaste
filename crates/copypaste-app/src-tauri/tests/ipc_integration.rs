use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixListener;
use std::path::Path;
use std::thread;
use tempfile::tempdir;

use copypaste_app_lib::ipc::IpcClient;

fn mock_daemon(sock: &Path, response: &'static str) {
    let listener = UnixListener::bind(sock).unwrap();
    thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = String::new();
            BufReader::new(&stream).read_line(&mut buf).unwrap();
            stream.write_all(response.as_bytes()).unwrap();
            stream.write_all(b"\n").unwrap();
        }
    });
}

#[test]
fn list_roundtrip() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("list.sock");
    mock_daemon(
        &sock,
        r#"{"id":"1","ok":true,"data":{"total":1,"items":[{"id":"abc","content_type":"text/plain","wall_time":1716000000000,"snippet":"hello","is_sensitive":false}]}}"#,
    );
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut client = IpcClient::connect(&sock).unwrap();
    let req = serde_json::json!({"id":"1","method":"list","params":{"limit":20,"offset":0}});
    let resp = client.call(&req).unwrap();
    assert!(resp.ok);
    let items = resp.data.unwrap()["items"].as_array().unwrap().clone();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["snippet"].as_str().unwrap(), "hello");
}

#[test]
fn delete_roundtrip() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("delete.sock");
    mock_daemon(&sock, r#"{"id":"3","ok":true,"data":{}}"#);
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut client = IpcClient::connect(&sock).unwrap();
    let req = serde_json::json!({"id":"3","method":"delete","params":{"id":"abc"}});
    let resp = client.call(&req).unwrap();
    assert!(resp.ok);
}

#[test]
fn search_roundtrip() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("search.sock");
    mock_daemon(
        &sock,
        r#"{"id":"2","ok":true,"data":{"total":1,"items":[{"id":"xyz","content_type":"text/plain","wall_time":1716000000000,"snippet":"search result","is_sensitive":false}]}}"#,
    );
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut client = IpcClient::connect(&sock).unwrap();
    let req = serde_json::json!({"id":"2","method":"search","params":{"query":"result","limit":20}});
    let resp = client.call(&req).unwrap();
    assert!(resp.ok);
    let items = resp.data.unwrap()["items"].as_array().unwrap().clone();
    assert_eq!(items[0]["snippet"].as_str().unwrap(), "search result");
}

#[test]
fn error_response_propagates() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("errsearch.sock");
    mock_daemon(&sock, r#"{"id":"5","ok":false,"error":"method not found"}"#);
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut client = IpcClient::connect(&sock).unwrap();
    let req = serde_json::json!({"id":"5","method":"search","params":{"query":"x","limit":5}});
    let resp = client.call(&req).unwrap();
    assert!(!resp.ok);
    assert_eq!(resp.error.as_deref(), Some("method not found"));
}
