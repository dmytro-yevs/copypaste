use std::future::Future;
use std::time::Duration;

use anyhow::Context;

/// Reactor work is light: store, crypto, module and pasteboard work takes a
/// `spawn_blocking` hop, so the worker count does not scale with cores.
const WORKER_THREADS: usize = 2;
/// Above the 64 IPC connections that each run one blocking request at a time,
/// plus background owners and `block_in_place`, so admitted work never queues.
const MAX_BLOCKING_THREADS: usize = 128;

/// Runs the daemon on an owned runtime and never spends shutdown budget twice.
///
/// Tokio's `Runtime::Drop` waits for started blocking work. W6a has already
/// used the daemon's bounded teardown budget, so release the runtime without
/// another wait; a blocking closure can continue only until this process exits.
pub fn run_with_bounded_shutdown<F>(future: F) -> anyhow::Result<()>
where
    F: Future<Output = anyhow::Result<()>>,
{
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(WORKER_THREADS)
        .max_blocking_threads(MAX_BLOCKING_THREADS)
        .enable_all()
        .build()
        .context("build the daemon runtime")?;
    let result = runtime.block_on(future);
    runtime.shutdown_timeout(Duration::ZERO);
    result
}
