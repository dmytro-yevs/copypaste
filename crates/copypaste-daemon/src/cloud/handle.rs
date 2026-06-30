// ── CloudHandle ───────────────────────────────────────────────────────────────

/// Handle returned by [`crate::cloud::start_cloud`].  Drop it to abandon the background tasks
/// (they will exit when the shutdown channel is signalled).
///
/// Audit-concurrency HIGH #3 (cloud-side): the daemon used to expose
/// `shutdown_tx` as a public field that the caller had to explicitly send on,
/// and in practice the daemon shutdown path never did — letting the cloud
/// tasks run until process exit. Two safeguards make that impossible now:
///   1. `shutdown_tx` is wrapped in `Option<...>` so [`Self::shutdown`] can take it
///      out behind a `&mut self`-style API.
///   2. `Drop` calls [`Self::shutdown`] automatically so dropping the handle (e.g.
///      losing the binding on a panic, or daemon teardown forgetting to call
///      it explicitly) still signals both loops.
pub struct CloudHandle {
    pub(super) shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// JoinHandle for the GoTrue auto-refresh task (`spawn_auto_refresh`).
    ///
    /// Audit-concurrency MEDIUM: that task loops forever holding an
    /// `Arc<AuthClient>` (and its reqwest connection pool); it has no shutdown
    /// path of its own (no `Notify`/token to `select!` on). Previously the
    /// JoinHandle was dropped with `let _ =`, so every cloud (re)start leaked
    /// one immortal task + AuthClient. Retaining the handle here lets us
    /// `.abort()` it on cloud shutdown/restart so it cannot outlive the loops
    /// it serves.
    pub(super) auth_refresh_handle: Option<tokio::task::JoinHandle<()>>,
    /// Canonical account identity token for the signed-in GoTrue session.
    ///
    /// Computed at `start_cloud` time by [`copypaste_supabase::supabase_account_id`]
    /// from `(supabase_url, user.id)`.  Two paired devices must share the SAME
    /// token — different tokens mean different Supabase projects or different
    /// GoTrue accounts, and Supabase RLS will silently hide each device's rows
    /// from the other.
    ///
    /// `None` when no GoTrue session is available (anon-key-only mode, which
    /// the project's RLS rejects anyway).
    ///
    /// Exposed here for the IPC `get_sync_status` handler to include in the
    /// status response (CopyPaste-44rq.26 follow-up: add the field to ipc.rs
    /// and propagate it through `IpcState` once the IPC/UI lane owns that work).
    pub cloud_account_id: Option<String>,
}

impl CloudHandle {
    /// Signal both background tasks to stop and abort the auth-refresh task.
    /// Idempotent — calling twice is a no-op (the slots are emptied on the
    /// first call; the consumed `self` then drops, and `Drop` finds `None`).
    pub fn shutdown(mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            // Receiver dropped or send failure → loops already exited.
            let _ = tx.send(());
        }
        if let Some(handle) = self.auth_refresh_handle.take() {
            // The auto-refresh loop has no cooperative shutdown; abort it.
            handle.abort();
        }
    }
}

impl Drop for CloudHandle {
    /// Belt-and-braces: if the caller forgot to call [`CloudHandle::shutdown`] explicitly
    /// (or dropped the handle on a panic/early return), still signal the
    /// background tasks and abort the auth-refresh task so they don't outlive
    /// the daemon.
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.auth_refresh_handle.take() {
            handle.abort();
        }
    }
}
