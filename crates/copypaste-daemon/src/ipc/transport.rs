//! IPC transport abstraction — platform-agnostic connection handling.
//!
//! The `IpcTransport` trait is implemented by `UnixIpcServer` (macOS/Linux)
//! and `NamedPipeIpcServer` (Windows).  `daemon.rs` selects the right
//! implementation at compile time via the `IpcServer` enum.
//!
//! Wire format is unchanged: newline-delimited JSON over a byte stream.

use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use copypaste_core::Database;

/// A connected client stream — anything that supports async line-delimited I/O.
///
/// Both `tokio::net::UnixStream` and tokio's Windows named-pipe
/// `NamedPipeServer` implement `AsyncRead + AsyncWrite`, so we can share
/// the `handle_connection` logic across platforms.
pub trait ConnectedStream:
    tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static
{
}

// Blanket impl: anything with the right bounds is a ConnectedStream.
impl<T> ConnectedStream for T where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static
{
}

/// Platform-selected IPC server — wraps either Unix or Windows implementation.
///
/// Constructed via `IpcServer::platform_default(db)` and driven by
/// `IpcServer::serve(path)`.
pub enum IpcServer {
    #[cfg(unix)]
    Unix(super::unix::UnixIpcServer),

    #[cfg(windows)]
    Windows(super::windows::NamedPipeIpcServer),
}

impl IpcServer {
    /// Construct the platform-appropriate IPC server backed by `db`.
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        #[cfg(unix)]
        return IpcServer::Unix(super::unix::UnixIpcServer::new(db));

        #[cfg(windows)]
        return IpcServer::Windows(super::windows::NamedPipeIpcServer::new(db));
    }

    /// Start accepting connections.  `path` is the Unix socket path on
    /// Unix systems and ignored on Windows (pipe name is fixed as
    /// `\\.\pipe\copypaste-daemon`).
    pub async fn serve(self, path: &Path) -> anyhow::Result<()> {
        match self {
            #[cfg(unix)]
            IpcServer::Unix(s) => s.serve(path).await,

            #[cfg(windows)]
            IpcServer::Windows(s) => {
                let _ = path; // Windows uses a fixed named-pipe path
                s.serve().await
            }
        }
    }
}
