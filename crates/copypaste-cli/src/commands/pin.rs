//! `pin` / `unpin` commands.
//!
//! Beta-bonus: front-end for the `pin_item` storage function that landed in
//! alpha (commit 76c4636). Pinning an item removes its TTL so the daemon's
//! retention worker never auto-deletes it.
//!
//! Wire format mirrors the rest of the CLI: a single line of JSON over the
//! unix socket. Both `pin` and `unpin` map to the daemon's single
//! `pin_item` handler, which takes `{id, pinned: bool}`. There is no
//! separate `unpin` method on the daemon, so we must not send one.

use anyhow::{anyhow, Result};
use copypaste_ipc::METHOD_PIN_ITEM;
use std::path::Path;

use crate::commands::common::exit_on_err;
use crate::ipc::IpcClient;

/// Loose syntactic check for a UUID string. We deliberately avoid pulling in
/// the `uuid` crate just for parsing — the daemon is the source of truth and
/// will reject malformed IDs. This catches the common typo / wrong-arg case
/// up-front with a clear error before we open the socket.
fn validate_uuid(id: &str) -> Result<()> {
    // 8-4-4-4-12 hex = 36 chars with four hyphens.
    if id.len() != 36 {
        return Err(anyhow!(
            "invalid id: expected UUID (36 chars), got {} chars",
            id.len()
        ));
    }
    let bytes = id.as_bytes();
    for (i, b) in bytes.iter().enumerate() {
        let is_hyphen_pos = matches!(i, 8 | 13 | 18 | 23);
        if is_hyphen_pos {
            if *b != b'-' {
                return Err(anyhow!("invalid id: expected '-' at position {i}"));
            }
        } else if !b.is_ascii_hexdigit() {
            return Err(anyhow!("invalid id: non-hex char at position {i}"));
        }
    }
    Ok(())
}

pub fn run_pin(socket_path: &Path, id: &str) -> Result<()> {
    validate_uuid(id)?;
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_PIN_ITEM,
        serde_json::json!({"id": id, "pinned": true}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("pinned {id}");
    Ok(())
}

pub fn run_unpin(socket_path: &Path, id: &str) -> Result<()> {
    validate_uuid(id)?;
    let mut client = IpcClient::connect(socket_path)?;
    let req = IpcClient::build_request(
        &IpcClient::next_id(),
        METHOD_PIN_ITEM,
        serde_json::json!({"id": id, "pinned": false}),
    );
    let resp = client.call(&req)?;
    exit_on_err(&resp);

    println!("unpinned {id}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::sync::mpsc;
    use std::thread;
    use tempfile::tempdir;

    /// Spin up a one-shot mock daemon that records the request line it
    /// receives, then sends back `response_json`. Returns a channel the test
    /// can drain to assert on the wire format.
    fn mock_server(socket_path: &Path, response_json: &'static str) -> mpsc::Receiver<String> {
        let listener = UnixListener::bind(socket_path).unwrap();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = String::new();
                let mut reader = BufReader::new(&stream);
                reader.read_line(&mut buf).unwrap();
                tx.send(buf).unwrap();
                stream.write_all(response_json.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });
        rx
    }

    const VALID_UUID: &str = "550e8400-e29b-41d4-a716-446655440000";

    #[test]
    fn pin_command_sends_correct_ipc_method() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("pin.sock");
        let rx = mock_server(
            &sock,
            r#"{"id":"1","ok":true,"data":{"pinned":true,"id":"550e8400-e29b-41d4-a716-446655440000"}}"#,
        );
        thread::sleep(std::time::Duration::from_millis(20));

        run_pin(&sock, VALID_UUID).unwrap();

        let req_line = rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
        let v: serde_json::Value = serde_json::from_str(req_line.trim()).unwrap();
        assert_eq!(v["method"], "pin_item");
        assert_eq!(v["params"]["id"], VALID_UUID);
        assert_eq!(v["params"]["pinned"], true);
    }

    #[test]
    fn unpin_command_sends_correct_ipc_method() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("unpin.sock");
        let rx = mock_server(
            &sock,
            r#"{"id":"1","ok":true,"data":{"unpinned":true,"id":"550e8400-e29b-41d4-a716-446655440000"}}"#,
        );
        thread::sleep(std::time::Duration::from_millis(20));

        run_unpin(&sock, VALID_UUID).unwrap();

        let req_line = rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap();
        let v: serde_json::Value = serde_json::from_str(req_line.trim()).unwrap();
        assert_eq!(v["method"], "pin_item");
        assert_eq!(v["params"]["id"], VALID_UUID);
        assert_eq!(v["params"]["pinned"], false);
    }

    #[test]
    fn pin_with_invalid_uuid_returns_clear_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("invalid.sock");
        // Don't even bind — UUID validation must fail BEFORE we touch the socket.
        let err = run_pin(&sock, "not-a-uuid").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("invalid id"),
            "expected clear error, got: {msg}"
        );

        // Same for unpin.
        let err = run_unpin(&sock, "still-not-a-uuid").unwrap_err();
        let msg = format!("{err}");
        assert!(
            msg.contains("invalid id"),
            "expected clear error, got: {msg}"
        );

        // And a 36-char string with non-hex chars should also be caught.
        let bad = "ZZZe8400-e29b-41d4-a716-446655440000";
        let err = run_pin(&sock, bad).unwrap_err();
        assert!(format!("{err}").contains("non-hex"));
    }
}
