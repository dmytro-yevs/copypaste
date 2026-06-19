//! Windows named-pipe IPC server skeleton (beta-bonus).
//!
//! **FROZEN 2026-05-23.** Windows is out of scope for v0.3+ — see
//! `docs/adr/ADR-012-windows-frozen-homebrew-only.md`. This module is
//! retained verbatim as reference material so the eventual thaw does
//! not have to re-derive the protocol shim; it is not compiled by any
//! active configuration (the daemon's `mod` declaration is already
//! `#[cfg(windows)]` and no CI target builds it). Do not delete.
//!
//! This module mirrors the line-delimited JSON request/response protocol
//! implemented for Unix domain sockets in [`crate::ipc`], but uses Windows
//! named pipes via [`tokio::net::windows::named_pipe`].
//!
//! # Protocol
//!
//! - Framing: one JSON object per line, terminated by `\n` (LF). `\r` is
//!   tolerated and stripped.
//! - Max request size: [`MAX_REQUEST_BYTES`] (16 MiB). A request that exceeds
//!   this is answered with `{"id":"0","ok":false,"error":"request too large"}\n`
//!   and the pipe instance is closed.
//! - Empty lines are silently ignored (keep-alive / no-op).
//! - Invalid UTF-8 lines receive an error response but the connection stays
//!   open (matching the Unix server's behaviour).
//!
//! # Default pipe name
//!
//! [`DEFAULT_PIPE_NAME`] = `\\.\pipe\copypaste-daemon`.
//!
//! # Status
//!
//! This is a *skeleton*. The full daemon currently dispatches requests via
//! the Unix-only [`crate::ipc::IpcServer`]. The Windows server here accepts
//! a caller-supplied [`Handler`] closure that receives the raw JSON request
//! string and must produce the raw JSON response string (without the trailing
//! newline). Wiring this into the daemon's `dispatch` is intentionally
//! deferred so this beta-bonus PR adds **only** the new file.
//!
//! # Platform gating
//!
//! The Windows implementation is gated behind `#[cfg(windows)]`. On non-Windows
//! targets, the public API still exists as a no-op stub so callers can
//! `use crate::ipc_win::run_named_pipe_server` unconditionally and receive a
//! `cfg-not-windows` runtime error rather than a compile failure. Tests are
//! gated `#[cfg(all(test, windows))]` and will be skipped on the macOS host
//! runner — they are expected to execute only on Windows CI.

#![allow(dead_code)]

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// Maximum size of a single IPC request line. Mirrors the Unix server's
/// 16 MiB cap. Clients exceeding this receive an error response and have
/// their pipe instance closed. Prevents OOM from a malicious or buggy
/// client sending an unbounded stream without newlines.
pub const MAX_REQUEST_BYTES: usize = 16 * 1024 * 1024;

/// Default Windows named-pipe path used by the daemon.
///
/// Equivalent in role to the Unix socket path at
/// `$XDG_RUNTIME_DIR/copypaste/daemon.sock`.
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\copypaste-daemon";

/// Async handler signature: takes one request line (trimmed, no trailing
/// `\n`) and returns one response line (also without trailing `\n` — the
/// server appends the LF before writing).
///
/// The handler is invoked on the per-connection task; long-running work
/// should be offloaded to `tokio::task::spawn_blocking` inside the handler.
pub type Handler =
    Arc<dyn for<'a> Fn(&'a str) -> Pin<Box<dyn Future<Output = String> + Send + 'a>> + Send + Sync>;

// ---------------------------------------------------------------------------
// Windows implementation
// ---------------------------------------------------------------------------

#[cfg(windows)]
mod imp {
    use super::{Handler, MAX_REQUEST_BYTES};
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    /// Run a named-pipe IPC server bound to `pipe_name`.
    ///
    /// The server uses the standard accept-loop pattern documented for
    /// [`ServerOptions::create`]: at any given moment there is exactly one
    /// "next" server instance listening; once a client connects, a fresh
    /// instance is created before the connected instance is handed off to
    /// a per-connection task.
    ///
    /// Returns only on a fatal listener error (e.g. invalid pipe name or
    /// OS-level resource exhaustion). Per-connection errors are logged and
    /// do not terminate the loop.
    pub async fn run_named_pipe_server(pipe_name: &str, handler: Handler) -> anyhow::Result<()> {
        tracing::info!("IPC (named pipe) listening on {}", pipe_name);

        // First instance must be created with `first_pipe_instance(true)`
        // so a stale server from a previous crashed daemon cannot squat
        // the name unnoticed. After that, subsequent instances are plain.
        let mut server = ServerOptions::new()
            .first_pipe_instance(true)
            .create(pipe_name)?;

        loop {
            // Wait for a client to connect to the current "next" instance.
            if let Err(e) = server.connect().await {
                tracing::warn!("named-pipe connect error: {e}");
                // Rebuild the listener and try again.
                server = ServerOptions::new().create(pipe_name)?;
                continue;
            }

            // Immediately create the next listening instance before handing
            // the connected one off — otherwise there is a window where a
            // client could try to connect and get ERROR_PIPE_BUSY.
            let connected = server;
            server = ServerOptions::new().create(pipe_name)?;

            let h = handler.clone();
            tokio::spawn(async move {
                if let Err(e) = handle_connection(connected, h).await {
                    tracing::warn!("named-pipe connection error: {e}");
                }
            });
        }
    }

    async fn handle_connection(pipe: NamedPipeServer, handler: Handler) -> anyhow::Result<()> {
        // tokio's `NamedPipeServer` does not have `into_split`, so we keep
        // it whole and use `&mut` borrows for reads and writes serially.
        // This matches the request/response shape (no server push), so no
        // concurrency is lost.
        let mut reader = BufReader::new(pipe);
        let mut buf: Vec<u8> = Vec::with_capacity(4 * 1024);

        loop {
            buf.clear();

            // Bound the read: at most MAX_REQUEST_BYTES + 1 so we can
            // distinguish "exactly the limit" from "exceeded the limit".
            let mut limited = (&mut reader).take((MAX_REQUEST_BYTES as u64) + 1);
            let n = match limited.read_until(b'\n', &mut buf).await {
                Ok(n) => n,
                Err(e) => {
                    tracing::warn!("ipc read error: {e}");
                    return Ok(());
                }
            };

            // Clean EOF — client closed without sending more data.
            if n == 0 {
                return Ok(());
            }

            // Oversized request: read more than MAX_REQUEST_BYTES without
            // finding a newline. Mirror the Unix server's error payload
            // exactly so clients can rely on a single error contract.
            if n > MAX_REQUEST_BYTES {
                tracing::warn!(
                    "ipc request exceeded {MAX_REQUEST_BYTES} bytes (read {n}); \
                     rejecting and closing"
                );
                let resp = r#"{"id":"0","ok":false,"error":"request too large"}"#;
                let mut out = String::from(resp);
                out.push('\n');
                let _ = reader.get_mut().write_all(out.as_bytes()).await;
                return Ok(());
            }

            // Trim trailing \n (and stray \r) before dispatch.
            while matches!(buf.last(), Some(b'\n' | b'\r')) {
                buf.pop();
            }

            // Empty line — silently ignored (keep-alive).
            if buf.is_empty() {
                continue;
            }

            let line = match std::str::from_utf8(&buf) {
                Ok(s) => s,
                Err(e) => {
                    let resp = format!(r#"{{"id":"0","ok":false,"error":"invalid UTF-8: {e}"}}"#);
                    let mut out = resp;
                    out.push('\n');
                    let _ = reader.get_mut().write_all(out.as_bytes()).await;
                    continue;
                }
            };

            let resp = (handler)(line).await;
            let mut out = resp;
            out.push('\n');
            if let Err(e) = reader.get_mut().write_all(out.as_bytes()).await {
                tracing::debug!("ipc write failed (client disconnected): {e}");
                return Ok(());
            }
        }
    }
}

#[cfg(windows)]
pub use imp::run_named_pipe_server;

// ---------------------------------------------------------------------------
// Non-Windows stub
// ---------------------------------------------------------------------------

/// Non-Windows stub: returns a runtime error explaining that the named-pipe
/// IPC backend is Windows-only. Lets the rest of the crate compile-check
/// this module on macOS / Linux without conditional `use` statements at the
/// call site.
#[cfg(not(windows))]
pub async fn run_named_pipe_server(_pipe_name: &str, _handler: Handler) -> anyhow::Result<()> {
    Err(anyhow::anyhow!(
        "named-pipe IPC is only available on Windows targets"
    ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Tests are gated `#[cfg(all(test, windows))]`. On the macOS host CI runner
// the entire test module is excluded from compilation, so `cargo test` is a
// silent no-op for Windows IPC coverage. Run on a Windows runner (or via
// `cargo test --target x86_64-pc-windows-gnu` with the appropriate linker
// toolchain) to exercise the round-trip and the 16 MiB cap.

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::windows::named_pipe::ClientOptions;

    fn unique_pipe_name() -> String {
        let nonce: u128 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        format!(r"\\.\pipe\copypaste-test-{nonce}")
    }

    /// Spawn the server, connect a client, send a `ping` line, expect `pong`.
    #[tokio::test]
    async fn ping_pong_roundtrip() {
        let pipe = unique_pipe_name();
        let handler: Handler = Arc::new(|line: &str| {
            let line = line.to_string();
            Box::pin(async move {
                if line.contains("ping") {
                    r#"{"id":"1","ok":true,"result":"pong"}"#.to_string()
                } else {
                    r#"{"id":"1","ok":false,"error":"unknown"}"#.to_string()
                }
            })
        });

        let pipe_clone = pipe.clone();
        let server_task = tokio::spawn(async move {
            // Server is allowed to fail when the test ends — we abort it.
            let _ = run_named_pipe_server(&pipe_clone, handler).await;
        });

        // Give the server a moment to bind the first instance.
        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = ClientOptions::new()
            .open(&pipe)
            .expect("client connect to named pipe");
        let (read_half, mut write_half) = tokio::io::split(client);
        let mut reader = BufReader::new(read_half);

        write_half
            .write_all(b"{\"id\":\"1\",\"method\":\"ping\"}\n")
            .await
            .expect("client write");

        let mut line = String::new();
        reader.read_line(&mut line).await.expect("client read");
        assert!(line.contains("pong"), "expected pong, got {line}");

        server_task.abort();
    }

    /// Oversize request (> 16 MiB without newline) must be rejected with the
    /// canonical "request too large" error payload, then the pipe closes.
    #[tokio::test]
    async fn oversize_request_rejected() {
        let pipe = unique_pipe_name();
        let handler: Handler = Arc::new(|_line: &str| {
            Box::pin(async move {
                // If we ever reach the handler, the cap is broken.
                r#"{"id":"x","ok":true,"result":"should-not-happen"}"#.to_string()
            })
        });

        let pipe_clone = pipe.clone();
        let server_task = tokio::spawn(async move {
            let _ = run_named_pipe_server(&pipe_clone, handler).await;
        });

        tokio::time::sleep(Duration::from_millis(100)).await;

        let client = ClientOptions::new()
            .open(&pipe)
            .expect("client connect to named pipe");
        let (read_half, mut write_half) = tokio::io::split(client);
        let mut reader = BufReader::new(read_half);

        // Write MAX_REQUEST_BYTES + 1 bytes WITHOUT a newline. The server
        // should give up after reading the +1 byte and respond with the
        // "request too large" error.
        let payload = vec![b'a'; MAX_REQUEST_BYTES + 1];
        write_half
            .write_all(&payload)
            .await
            .expect("client write oversize");
        let _ = write_half.shutdown().await;

        let mut line = String::new();
        reader
            .read_line(&mut line)
            .await
            .expect("client read error response");
        assert!(
            line.contains("request too large"),
            "expected 'request too large', got {line}"
        );

        server_task.abort();
    }
}
