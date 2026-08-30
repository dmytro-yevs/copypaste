use std::future::Future;
use std::time::Duration;

use anyhow::Context;

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
        .enable_all()
        .build()
        .context("build the daemon runtime")?;
    let result = runtime.block_on(future);
    runtime.shutdown_timeout(Duration::ZERO);
    result
}
