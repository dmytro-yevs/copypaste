//! The process-wide executor for generated bridge calls.
//!
//! The generator's default handler starts one pool thread and one Tokio worker
//! per core. Bridge calls mostly await IPC, so a small fixed set serves them.
//! Initialization needs no Dart engine, so native pairing and Android capture
//! share this runtime instead of building their own.

use std::panic::AssertUnwindSafe;
use std::sync::LazyLock;

use flutter_rust_bridge::for_generated::{NoOpErrorListener, SimpleExecutor, SimpleThreadPool};
use flutter_rust_bridge::{DefaultHandler, SimpleAsyncRuntime};

/// Synchronous calls include application scans and icon extraction; spare
/// threads keep a queue of them from stalling runtime lifecycle calls.
const SYNC_CALL_THREADS: usize = 4;
const ASYNC_WORKER_THREADS: usize = 2;
/// Blocking threads start on demand and exit when idle; this bounds a burst.
const MAX_BLOCKING_THREADS: usize = 16;

pub static FLUTTER_RUST_BRIDGE_HANDLER: LazyLock<DefaultHandler<SimpleThreadPool>> =
    LazyLock::new(|| {
        assert_eq!(
            crate::frb_generated::FLUTTER_RUST_BRIDGE_CODEGEN_VERSION,
            flutter_rust_bridge::for_generated::FLUTTER_RUST_BRIDGE_RUNTIME_VERSION,
            "flutter_rust_bridge codegen and runtime versions must match",
        );
        let sync_calls = threadpool::Builder::new()
            .num_threads(SYNC_CALL_THREADS)
            .thread_name("copypaste-bridge-sync".into())
            .build();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(ASYNC_WORKER_THREADS)
            .max_blocking_threads(MAX_BLOCKING_THREADS)
            .thread_name("copypaste-bridge")
            .enable_all()
            .build()
            .expect("bridge async runtime initializes");
        DefaultHandler::new(
            SimpleExecutor::new(
                NoOpErrorListener,
                SimpleThreadPool(sync_calls),
                SimpleAsyncRuntime(AssertUnwindSafe(runtime)),
            ),
            NoOpErrorListener,
        )
    });

/// The runtime behind every async bridge call. Native callers may `block_on`
/// it only from threads that are not its own workers.
pub(crate) fn async_runtime() -> &'static tokio::runtime::Runtime {
    &FLUTTER_RUST_BRIDGE_HANDLER.async_runtime().0
}

#[cfg(test)]
mod tests {
    #[test]
    fn generated_calls_use_this_handler() {
        assert!(std::ptr::eq(
            &*crate::frb_generated::FLUTTER_RUST_BRIDGE_HANDLER,
            &*super::FLUTTER_RUST_BRIDGE_HANDLER,
        ));
    }
}
