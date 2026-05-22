use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;

#[derive(Debug)]
pub struct Response {
    pub id: String,
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

pub struct IpcClient {
    stream: UnixStream,
}

impl IpcClient {
    pub fn connect(socket_path: &Path) -> Result<Self> {
        let stream = UnixStream::connect(socket_path)
            .with_context(|| format!("daemon not running (socket: {})", socket_path.display()))?;
        Ok(Self { stream })
    }

    pub fn call(&mut self, request: &Value) -> Result<Response> {
        let mut line = serde_json::to_string(request)?;
        line.push('\n');
        self.stream.write_all(line.as_bytes())
            .context("failed to write to daemon socket")?;

        let mut reader = BufReader::new(&self.stream);
        let mut resp_line = String::new();
        reader.read_line(&mut resp_line)
            .context("failed to read from daemon socket")?;

        if resp_line.is_empty() {
            return Err(anyhow!("daemon closed connection without response"));
        }

        let v: Value = serde_json::from_str(resp_line.trim())
            .context("invalid JSON from daemon")?;

        Ok(Response {
            id: v["id"].as_str().unwrap_or("").to_string(),
            ok: v["ok"].as_bool().unwrap_or(false),
            data: if v["data"].is_null() { None } else { Some(v["data"].clone()) },
            error: v["error"].as_str().map(|s| s.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;
    use std::thread;
    use tempfile::tempdir;

    fn mock_server(socket_path: &Path, response_json: &'static str) {
        let listener = UnixListener::bind(socket_path).unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = String::new();
                BufReader::new(&stream).read_line(&mut buf).unwrap();
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
        mock_server(&sock, r#"{"id":"1","ok":true,"data":{"total":2,"items":[]}}"#);
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id":"1","method":"list","params":{"limit":20,"offset":0}});
        let resp = client.call(&req).unwrap();
        assert!(resp.ok);
        assert_eq!(resp.id, "1");
    }

    #[test]
    fn call_parses_error_response() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("err.sock");
        mock_server(&sock, r#"{"id":"2","ok":false,"error":"not found"}"#);
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id":"2","method":"delete","params":{"id":"missing"}});
        let resp = client.call(&req).unwrap();
        assert!(!resp.ok);
        assert_eq!(resp.error.as_deref(), Some("not found"));
    }

    #[test]
    fn call_errors_on_closed_connection() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("closed.sock");
        // Server accepts but immediately closes without writing
        let listener = UnixListener::bind(&sock).unwrap();
        thread::spawn(move || {
            if let Ok((_stream, _)) = listener.accept() {
                // drop stream immediately
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id":"4","method":"status","params":{}});
        assert!(client.call(&req).is_err());
    }
}
