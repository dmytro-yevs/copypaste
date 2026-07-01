//! Shared process-wide tokio runtime backing every blocking P2P FFI wrapper
//! (bootstrap-initiator, discovery, and SAS pairing).

use std::sync::OnceLock;

use crate::CopypasteError;

/// Process-wide tokio runtime backing the blocking P2P FFI wrappers.
///
/// A single multi-thread runtime is created lazily on first pairing call and
/// reused for the life of the process. Multi-thread is required: the bootstrap
/// handshake interleaves framed TLS reads and writes that would deadlock on a
/// current-thread runtime under `block_on`.
///
/// `OnceLock` only lets us store a fully-initialised value, so we store a
/// `Result` (via an `Option`) to propagate build failures to callers instead
/// of panicking across the FFI boundary. The `Option` is always `Some` after
/// the first call; `None` is unreachable in practice but handled for
/// soundness.
pub(crate) static RUNTIME: OnceLock<Result<tokio::runtime::Runtime, String>> = OnceLock::new();

/// Return a reference to the shared multi-thread runtime, or an error if it
/// could not be built. Never panics — callers surface the error as
/// `CopypasteError::P2pError` so the JVM is not killed.
pub(crate) fn runtime() -> Result<&'static tokio::runtime::Runtime, CopypasteError> {
    RUNTIME
        .get_or_init(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("failed to build tokio runtime for P2P FFI: {e}"))
        })
        .as_ref()
        .map_err(|e| CopypasteError::P2pError { reason: e.clone() })
}
