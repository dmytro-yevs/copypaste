use anyhow::{anyhow, Context, Result};
use copypaste_ipc::ErrorCode;
use serde_json::Value;
use std::io::{BufRead, BufReader, ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

/// Max time to wait for the daemon to accept a write or produce a response
/// line. Without this, a daemon that accepts the connection but never replies
/// (deadlocked DB, stuck `spawn_blocking`) would hang the CLI forever.
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Minimal wire-level response. Mirrors protocol.rs in the daemon.
///
/// W3.3: gained an optional [`ErrorCode`] parsed from the daemon's
/// `error_code` field. Existing fields are unchanged so prior call sites
/// keep working.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Response {
    #[allow(dead_code)]
    pub id: String,
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
    /// Typed machine-readable error code, when the daemon attached one.
    /// `None` on success and on legacy (untagged) error responses.
    pub error_code: Option<ErrorCode>,
}

pub struct IpcClient {
    stream: UnixStream,
}

impl IpcClient {
    /// Connect to daemon socket. Returns an error if the socket does not exist or
    /// the daemon is not listening.
    pub fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .with_context(|| format!("daemon not running (socket: {})", socket_path.display()))?;
        // Bound every read/write so a connected-but-unresponsive daemon can't
        // hang the CLI indefinitely.
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .context("failed to set read timeout on daemon socket")?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .context("failed to set write timeout on daemon socket")?;
        Ok(Self { stream })
    }

    /// Send a JSON request and read exactly one JSON response line.
    pub fn call(&mut self, request: &Value) -> Result<Response> {
        // Write request line
        let mut line = serde_json::to_string(request)?;
        line.push('\n');
        self.stream
            .write_all(line.as_bytes())
            .map_err(map_timeout)
            .context("failed to write to daemon socket")?;

        // Read response line
        let mut reader = BufReader::new(&self.stream);
        let mut resp_line = String::new();
        reader
            .read_line(&mut resp_line)
            .map_err(map_timeout)
            .context("failed to read from daemon socket")?;

        if resp_line.is_empty() {
            return Err(anyhow!("daemon closed connection without response"));
        }

        // Parse response
        let v: Value =
            serde_json::from_str(resp_line.trim()).context("invalid JSON from daemon")?;

        Ok(Response {
            id: v["id"].as_str().unwrap_or("").to_string(),
            ok: v["ok"].as_bool().unwrap_or(false),
            data: if v["data"].is_null() {
                None
            } else {
                Some(v["data"].clone())
            },
            error: v["error"].as_str().map(|s| s.to_string()),
            // W3.3: parse the machine-readable `error_code` if attached.
            // Unknown / missing codes collapse to `None` so older daemons
            // keep working unchanged.
            error_code: v["error_code"].as_str().and_then(ErrorCode::parse),
        })
    }
}

/// Convert a socket-timeout I/O error into a clear, actionable message.
///
/// A read/write timeout surfaces as [`ErrorKind::WouldBlock`] on most Unix
/// platforms and [`ErrorKind::TimedOut`] on others; both mean the daemon
/// accepted the connection but did not respond in time. Any other I/O error
/// is passed through unchanged.
fn map_timeout(err: std::io::Error) -> std::io::Error {
    match err.kind() {
        ErrorKind::WouldBlock | ErrorKind::TimedOut => std::io::Error::new(
            err.kind(),
            "daemon not responding (timed out after 5s) — it may be deadlocked; try restarting it",
        ),
        _ => err,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::net::UnixListener;
    use std::thread;
    use tempfile::tempdir;

    fn mock_server(socket_path: &Path, response_json: &'static str) {
        let listener = UnixListener::bind(socket_path).unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                // Drain the request line
                let mut buf = String::new();
                let mut reader = BufReader::new(&stream);
                reader.read_line(&mut buf).unwrap();
                // Send canned response
                stream.write_all(response_json.as_bytes()).unwrap();
                stream.write_all(b"\n").unwrap();
            }
        });
    }

    #[test]
    fn connect_fails_when_no_socket() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("nonexistent.sock");
        assert!(IpcClient::connect(&path).is_err());
    }

    #[test]
    fn call_returns_ok_response() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("test.sock");
        mock_server(&sock, r#"{"id":"1","ok":true,"data":{"status":"running"}}"#);
        // Give the thread a moment to bind
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id": "1", "method": "status", "params": {}});
        let resp = client.call(&req).unwrap();

        assert!(resp.ok);
        assert_eq!(resp.id, "1");
        assert!(resp.data.is_some());
    }

    #[test]
    fn call_returns_err_response() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("err.sock");
        mock_server(
            &sock,
            r#"{"id":"2","ok":false,"error":"unknown method: foo"}"#,
        );
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id": "2", "method": "foo", "params": {}});
        let resp = client.call(&req).unwrap();

        assert!(!resp.ok);
        assert_eq!(resp.error.as_deref(), Some("unknown method: foo"));
    }
}
