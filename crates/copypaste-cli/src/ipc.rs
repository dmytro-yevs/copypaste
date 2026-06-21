use anyhow::{anyhow, Context, Result};
use copypaste_ipc::ErrorCode;
use serde_json::Value;
use std::io::{BufRead, BufReader, ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

// ro0r: backoff/retry parameters for `migration_in_progress`.
//
// When the daemon is running its v4 key-rotation sweep it temporarily rejects
// ingest writes with this code. 5 attempts with 250ms→2s exponential backoff
// covers the typical sweep window without hanging the CLI for more than ~8s.
// Only this one code is retried — everything else propagates immediately.

/// Maximum number of retry attempts for a `migration_in_progress` response.
const MIGRATION_MAX_RETRIES: u32 = 5;
/// Initial backoff delay in milliseconds (doubles each attempt, capped at
/// [`MIGRATION_BACKOFF_CAP_MS`]).
const MIGRATION_BACKOFF_INIT_MS: u64 = 250;
/// Upper bound on a single backoff delay (in milliseconds). Caps exponential
/// growth so the longest single wait is ~2 s even when many retries remain.
const MIGRATION_BACKOFF_CAP_MS: u64 = 2_000;

/// Opaque counter for generating unique request ids within a process run.
// Used by next_id(); suppress dead_code: this is intentional public API for
// callers that want monotonic ids rather than the hardcoded "1" used today.
#[allow(dead_code)]
static REQ_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// Max time to wait for the daemon to accept a write or produce a response
/// line. Without this, a daemon that accepts the connection but never replies
/// (deadlocked DB, stuck `spawn_blocking`) would hang the CLI forever.
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// Hard ceiling on a single response line read from the daemon socket.
///
/// Mirrors the daemon's own `MAX_REQUEST_BYTES` posture (16 MiB): without a
/// cap, a buggy or hostile peer on the socket could stream an unbounded line
/// and force the CLI to grow `resp_line` until it exhausts memory. 16 MiB is
/// comfortably larger than any legitimate response (the daemon clamps `list`
/// pages to 1000 rows) while still bounding worst-case allocation.
const MAX_RESPONSE_BYTES: u64 = 16 * 1024 * 1024;

/// Minimal wire-level response. Mirrors protocol.rs in the daemon.
///
/// W3.3: gained an optional [`ErrorCode`] parsed from the daemon's
/// `error_code` field. Existing fields are unchanged so prior call sites
/// keep working.
///
/// FEACLI-8: `raw_error_code` captures the wire string verbatim so that
/// codes unknown to this build of the CLI are still surfaced in error output
/// rather than silently dropped.  `error_code` remains the typed variant
/// (used internally for retry/version-mismatch branching); display code
/// should prefer `raw_error_code` so future daemon codes remain visible.
#[derive(Debug)]
#[allow(dead_code)]
pub struct Response {
    #[allow(dead_code)]
    pub id: String,
    pub ok: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
    /// Typed machine-readable error code, when the daemon attached one.
    /// `None` on success, on legacy (untagged) error responses, and for
    /// codes that are not yet known to this CLI build (use `raw_error_code`
    /// for display in those cases).
    pub error_code: Option<ErrorCode>,
    /// Raw `error_code` wire string, preserved verbatim even when the code
    /// is not recognised by [`ErrorCode::parse`].  `None` only when the
    /// daemon did not attach an `error_code` field at all.
    pub raw_error_code: Option<String>,
}

pub struct IpcClient {
    stream: UnixStream,
    /// Socket path retained for `call` reconnects on `migration_in_progress`
    /// retries. Stored as `Box<Path>` to avoid a lifetime parameter on the struct.
    socket_path: Box<Path>,
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
        Ok(Self {
            stream,
            socket_path: socket_path.into(),
        })
    }

    /// Open a fresh connection to the stored socket path and replace `self.stream`.
    ///
    /// Used by the `migration_in_progress` retry loop: each retry needs a new
    /// TCP-like connection because the old one is in an unknown state after an
    /// error response.
    fn reconnect(&mut self) -> Result<()> {
        let stream = UnixStream::connect(&self.socket_path).with_context(|| {
            format!(
                "daemon not running (socket: {})",
                self.socket_path.display()
            )
        })?;
        stream
            .set_read_timeout(Some(IO_TIMEOUT))
            .context("failed to set read timeout on daemon socket")?;
        stream
            .set_write_timeout(Some(IO_TIMEOUT))
            .context("failed to set write timeout on daemon socket")?;
        self.stream = stream;
        Ok(())
    }

    /// Build a request JSON object stamped with `protocol_version` and a
    /// unique string `id`. All command modules should use this helper instead
    /// of constructing raw `serde_json::json!` literals so every request
    /// automatically carries the protocol version.
    ///
    /// The id is a monotonically-increasing counter converted to a string so
    /// it remains compatible with the current string-id wire format used by
    /// the daemon. The counter is process-global and wraps at u64::MAX
    /// (effectively never).
    pub fn build_request(id: &str, method: &str, params: Value) -> Value {
        serde_json::json!({
            "id": id,
            "method": method,
            "protocol_version": copypaste_ipc::PROTOCOL_VERSION,
            "params": params,
        })
    }

    /// Allocate a fresh monotonic request id as a decimal string.
    // Intentional public API for future callers; suppress dead_code warning.
    #[allow(dead_code)]
    pub fn next_id() -> String {
        REQ_COUNTER
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .to_string()
    }

    /// Send a JSON request and read exactly one JSON response line (raw, no retry).
    ///
    /// Enforces two framing guards beyond the raw I/O:
    /// 1. The response `id` must echo the request `id`; a mismatch indicates
    ///    a framing desync (or a rogue peer) and is rejected immediately.
    /// 2. A `version_mismatch` error code in the response is surfaced as a
    ///    clear, actionable error so the user knows to upgrade CLI or daemon.
    ///
    /// This is the low-level primitive. Most callers should use [`Self::call`]
    /// which wraps this with `migration_in_progress` backoff/retry (ro0r).
    fn call_once(&mut self, request: &Value) -> Result<Response> {
        // Capture the id Value directly so we can compare it against the
        // response id Value without losing type information. Using
        // `.as_str().unwrap_or("")` would coerce a numeric id (e.g. `1`)
        // to `""`, making both sides `""` and the mismatch guard always pass.
        let req_id_value = request["id"].clone();

        // Write request line
        let mut line = serde_json::to_string(request)?;
        line.push('\n');
        self.stream
            .write_all(line.as_bytes())
            .map_err(map_timeout)
            .context("failed to write to daemon socket")?;

        // Read response line, bounded to MAX_RESPONSE_BYTES so a misbehaving
        // peer can't stream an unbounded line and OOM the CLI. We read at most
        // MAX_RESPONSE_BYTES + 1 so we can tell a legitimate (capped) line from
        // one the daemon truncated by hitting the ceiling.
        let mut reader = BufReader::new((&self.stream).take(MAX_RESPONSE_BYTES + 1));
        let mut resp_line = String::new();
        let n = reader
            .read_line(&mut resp_line)
            .map_err(map_timeout)
            .context("failed to read from daemon socket")?;

        if resp_line.is_empty() {
            return Err(anyhow!("daemon closed connection without response"));
        }

        // If we read past the cap, the line is oversized (or unterminated):
        // reject rather than parse a truncated, possibly-misleading payload.
        if n as u64 > MAX_RESPONSE_BYTES {
            return Err(anyhow!(
                "daemon response exceeded {MAX_RESPONSE_BYTES} bytes; refusing to parse"
            ));
        }

        // Parse response
        let v: Value =
            serde_json::from_str(resp_line.trim()).context("invalid JSON from daemon")?;

        // Guard: reject responses whose id doesn't echo the request id.
        // This catches framing desyncs where we read a stale response meant
        // for a previous request (or a rogue peer injecting traffic).
        //
        // We compare the serde_json::Value directly so both string ids
        // ("1") and numeric ids (1) round-trip correctly. Using `.as_str()`
        // on a numeric JSON value returns None, which `.unwrap_or("")`
        // silently collapses to "" on both sides — making the guard a no-op
        // for any non-string id.
        let resp_id_value = v["id"].clone();
        if resp_id_value != req_id_value {
            return Err(anyhow!(
                "response id mismatch: sent {}, got {}",
                req_id_value,
                resp_id_value
            ));
        }
        // For the Response struct we still store a String. Prefer the string
        // representation if the id is a JSON string; fall back to the JSON
        // serialisation for numeric ids so callers always get something useful.
        let resp_id_str = resp_id_value
            .as_str()
            .map(|s| s.to_string())
            .unwrap_or_else(|| resp_id_value.to_string());

        // FEACLI-8: capture the raw wire string first so unknown codes are
        // never silently dropped.  `error_code` is the typed variant; it is
        // `None` for unrecognised codes, but `raw_error_code` always carries
        // the original string when the daemon sent one.
        let raw_error_code: Option<String> = v["error_code"].as_str().map(|s| s.to_string());
        let error_code = raw_error_code.as_deref().and_then(ErrorCode::parse);

        // Surface version_mismatch immediately so users get a clear, actionable
        // message rather than a generic error string buried in resp.error.
        if matches!(error_code, Some(ErrorCode::VersionMismatch)) {
            let msg = v["error"]
                .as_str()
                .unwrap_or("daemon requires a different protocol version");
            return Err(anyhow!(
                "version mismatch: {} — upgrade CLI or restart daemon",
                msg
            ));
        }

        Ok(Response {
            id: resp_id_str,
            ok: v["ok"].as_bool().unwrap_or(false),
            data: if v["data"].is_null() {
                None
            } else {
                Some(v["data"].clone())
            },
            error: v["error"].as_str().map(|s| s.to_string()),
            // W3.3: typed code for retry/branching logic.
            // FEACLI-8: raw string for display — preserves unknown codes.
            error_code,
            raw_error_code,
        })
    }

    /// Send a JSON request and read exactly one JSON response line.
    ///
    /// ro0r: when the daemon replies with `migration_in_progress` (v4
    /// key-rotation sweep in flight), this method backs off and retries up to
    /// [`MIGRATION_MAX_RETRIES`] times before giving up. Every other error code
    /// and every transport-level error is propagated immediately without
    /// retrying. Backoff schedule: 250 ms → 500 ms → 1 s → 2 s → 2 s.
    ///
    /// On exhausted retries the method returns `Err` (rather than calling
    /// `process::exit`) so that callers holding `Zeroizing<…>` secret material
    /// have their destructors run before the process terminates.
    /// (CopyPaste-liaz: `process::exit` bypasses all Drop impls.)
    pub fn call(&mut self, request: &Value) -> Result<Response> {
        let mut delay_ms = MIGRATION_BACKOFF_INIT_MS;
        for attempt in 0..=MIGRATION_MAX_RETRIES {
            let resp = self.call_once(request)?;

            match resp.error_code {
                Some(ErrorCode::MigrationInProgress) if attempt < MIGRATION_MAX_RETRIES => {
                    // Back off and retry. Reconnect for each attempt because the
                    // daemon closes the connection after an error response.
                    eprintln!(
                        "daemon migration in progress — retrying in {delay_ms}ms \
                         (attempt {}/{MIGRATION_MAX_RETRIES})",
                        attempt + 1
                    );
                    std::thread::sleep(Duration::from_millis(delay_ms));
                    delay_ms = (delay_ms * 2).min(MIGRATION_BACKOFF_CAP_MS);
                    // Reconnect for next attempt.
                    if let Err(e) = self.reconnect() {
                        return Err(e.context("failed to reconnect for migration retry"));
                    }
                }
                Some(ErrorCode::MigrationInProgress) => {
                    // Retries exhausted. Return Err so callers can drop Zeroizing
                    // secrets before process termination (CopyPaste-liaz).
                    // The error message is the same that was previously printed
                    // before process::exit(1) — main.rs will print it via eprintln.
                    return Err(anyhow::anyhow!(
                        "error [migration_in_progress]: daemon key-rotation is still in \
                         progress after {MIGRATION_MAX_RETRIES} retries — \
                         please try again in a few seconds"
                    ));
                }
                _ => return Ok(resp),
            }
        }
        // Unreachable: the loop above always returns or exits before reaching
        // attempt == MIGRATION_MAX_RETRIES + 1. The compiler requires this.
        unreachable!("migration retry loop exited without returning")
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

    /// A response line larger than `MAX_RESPONSE_BYTES` must be rejected
    /// (not parsed, not allowed to grow `resp_line` unbounded). We stream a
    /// payload past the cap with no trailing newline so `read_line` stops only
    /// because `.take()` hit the ceiling.
    #[test]
    fn call_rejects_oversized_response() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("big.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buf = String::new();
                let mut reader = BufReader::new(&stream);
                reader.read_line(&mut buf).unwrap();
                // Write more than the cap, with no newline, in chunks so we
                // don't allocate one giant buffer. Ignore write errors: once
                // the client bails and drops its end, the pipe breaks.
                let chunk = vec![b'a'; 64 * 1024];
                let mut written: u64 = 0;
                while written <= MAX_RESPONSE_BYTES {
                    if stream.write_all(&chunk).is_err() {
                        break;
                    }
                    written += chunk.len() as u64;
                }
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id": "1", "method": "list", "params": {}});
        let err = client.call(&req).unwrap_err();
        assert!(
            err.to_string().contains("exceeded"),
            "expected oversize rejection, got: {err}"
        );
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

    /// build_request must stamp protocol_version = PROTOCOL_VERSION on every
    /// request JSON object.
    #[test]
    fn build_request_stamps_protocol_version() {
        let req = IpcClient::build_request("req-1", "list", serde_json::json!({"limit": 10}));
        assert_eq!(
            req["protocol_version"].as_u64(),
            Some(copypaste_ipc::PROTOCOL_VERSION as u64),
            "build_request must set protocol_version"
        );
        assert_eq!(req["id"].as_str(), Some("req-1"));
        assert_eq!(req["method"].as_str(), Some("list"));
    }

    /// build_request must include the params payload unchanged.
    #[test]
    fn build_request_preserves_params() {
        let params = serde_json::json!({"query": "hello", "limit": 5});
        let req = IpcClient::build_request("r", "search", params.clone());
        assert_eq!(req["params"], params);
    }

    /// call must return an error when the response id doesn't match the request id.
    #[test]
    fn call_rejects_mismatched_response_id() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("mismatch.sock");
        // Server echoes id "99" but the request will carry id "1"
        mock_server(&sock, r#"{"id":"99","ok":true,"data":{}}"#);
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id": "1", "method": "status", "params": {}});
        let err = client.call(&req).unwrap_err();
        assert!(
            err.to_string().contains("response id mismatch"),
            "expected id mismatch error, got: {err}"
        );
    }

    /// A response carrying error_code = "version_mismatch" must be surfaced as
    /// a clear, actionable error rather than a generic ok/err response.
    #[test]
    fn call_surfaces_version_mismatch_error() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("vermismatch.sock");
        mock_server(
            &sock,
            r#"{"id":"1","ok":false,"error":"protocol version mismatch","error_code":"version_mismatch"}"#,
        );
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id": "1", "method": "list", "params": {}});
        let err = client.call(&req).unwrap_err();
        assert!(
            err.to_string().contains("version mismatch"),
            "expected version mismatch error, got: {err}"
        );
    }

    /// ro0r: call() must retry when the first response carries
    /// error_code = "migration_in_progress" and succeed on the next attempt
    /// when the subsequent response is ok.
    ///
    /// The mock server handles two sequential connections: the first sends
    /// migration_in_progress, the second sends ok. The test verifies that
    /// call() retries transparently and returns the successful response.
    #[test]
    fn call_retries_on_migration_in_progress_and_succeeds() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc as SArc;

        let dir = tempdir().unwrap();
        let sock = dir.path().join("mig_retry.sock");
        let listener = UnixListener::bind(&sock).unwrap();
        let connection_count = SArc::new(AtomicUsize::new(0));
        let cc = SArc::clone(&connection_count);
        thread::spawn(move || {
            // Accept two sequential connections.
            for _ in 0..2 {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = String::new();
                    let mut reader = BufReader::new(&stream);
                    reader.read_line(&mut buf).unwrap();
                    let n = cc.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                    let reply = if n == 0 {
                        // First attempt: migration in progress.
                        r#"{"id":"1","ok":false,"error":"sweep in flight","error_code":"migration_in_progress"}"#
                    } else {
                        // Second attempt: success.
                        r#"{"id":"1","ok":true,"data":{"done":true}}"#
                    };
                    stream.write_all(reply.as_bytes()).unwrap();
                    stream.write_all(b"\n").unwrap();
                }
            }
        });
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id": "1", "method": "copy", "params": {}});
        // call() should retry transparently and return the success response.
        let resp = client.call(&req).unwrap();
        assert!(
            resp.ok,
            "expected ok=true after migration retry, got: {resp:?}"
        );
        assert_eq!(connection_count.load(Ordering::SeqCst), 2);
    }

    /// FEACLI-8: a response carrying an error_code that is NOT in the
    /// ErrorCode enum must still be preserved in raw_error_code so callers
    /// can surface it rather than silently dropping it.
    #[test]
    fn call_preserves_unknown_error_code_in_raw_field() {
        let dir = tempdir().unwrap();
        let sock = dir.path().join("unknown_code.sock");
        // "future_code" is not in the ErrorCode enum — raw_error_code must
        // carry it even though error_code will be None.
        mock_server(
            &sock,
            r#"{"id":"1","ok":false,"error":"daemon says nope","error_code":"future_code"}"#,
        );
        std::thread::sleep(std::time::Duration::from_millis(20));

        let mut client = IpcClient::connect(&sock).unwrap();
        let req = serde_json::json!({"id": "1", "method": "list", "params": {}});
        let resp = client.call(&req).unwrap();

        assert!(!resp.ok);
        assert_eq!(
            resp.raw_error_code.as_deref(),
            Some("future_code"),
            "raw_error_code must preserve the wire string verbatim"
        );
        assert!(
            resp.error_code.is_none(),
            "typed error_code must be None for unrecognised codes"
        );
    }
}
