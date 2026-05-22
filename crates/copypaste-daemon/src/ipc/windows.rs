//! Windows named-pipe IPC server.
//!
//! Listens on `\\.\pipe\copypaste-daemon`.
//! The dispatch logic is shared with Unix via `super::handle_connection`.
//!
//! # TODO (Phase 3)
//!
//! - [ ] Implement `NamedPipeIpcServer::serve()` accept loop using
//!       `tokio::net::windows::named_pipe::ServerOptions`
//! - [ ] Each `accept` iteration: create a new `ServerOptions` instance,
//!       call `.connect().await` on it, then immediately create the next
//!       pending instance (Windows named pipes work one-instance-at-a-time)
//! - [ ] Restrict pipe DACL to current user:
//!       `ServerOptions::new().pipe_mode(...).security_attributes(sddl)...`
//!       SDDL: `"D:(A;;GRGW;;;AU)"` (Authenticated Users read/write)
//!       Tighten to `"D:(A;;GRGW;;;CU)"` (Current User) for production
//! - [ ] CLI client: `tokio::net::windows::named_pipe::ClientOptions::new()
//!           .open(PIPE_NAME)?`
//! - [ ] Add integration test mirroring `unix.rs` tests

#![cfg(windows)]

use std::sync::Arc;
use tokio::sync::Mutex;
use copypaste_core::Database;

pub const PIPE_NAME: &str = r"\\.\pipe\copypaste-daemon";

pub struct NamedPipeIpcServer {
    db: Arc<Mutex<Database>>,
}

impl NamedPipeIpcServer {
    pub fn new(db: Arc<Mutex<Database>>) -> Self {
        Self { db }
    }

    /// Accept connections on the named pipe and dispatch JSON-RPC requests.
    ///
    /// # TODO (Phase 3)
    ///
    /// Replace the `unimplemented!()` below with:
    /// ```rust,ignore
    /// use tokio::net::windows::named_pipe::ServerOptions;
    ///
    /// loop {
    ///     let server = ServerOptions::new()
    ///         .first_pipe_instance(true)  // only for the very first instance
    ///         .create(PIPE_NAME)?;
    ///
    ///     // Wait for a client to connect.
    ///     server.connect().await?;
    ///
    ///     let db = Arc::clone(&self.db);
    ///     tokio::spawn(async move {
    ///         if let Err(e) = super::handle_connection(db, server).await {
    ///             tracing::warn!("IPC connection error: {e}");
    ///         }
    ///     });
    ///
    ///     // Reset first_pipe_instance for subsequent accepts.
    /// }
    /// ```
    pub async fn serve(self) -> anyhow::Result<()> {
        // TODO(Phase 3): implement named-pipe accept loop (see doc comment above).
        tracing::warn!("Windows IPC server not yet implemented — Phase 3 stub");
        // Block forever so the daemon doesn't exit; real impl will loop on accept.
        std::future::pending::<anyhow::Result<()>>().await
    }
}

#[cfg(test)]
mod tests {
    // TODO(Phase 3): add integration tests mirroring ipc/unix.rs tests.
    // Use `tokio::net::windows::named_pipe::ClientOptions` to connect.
}
