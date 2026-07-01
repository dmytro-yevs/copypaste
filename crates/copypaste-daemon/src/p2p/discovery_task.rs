//! mDNS-SD registration and the long-running discovery browse task.
//!
//! Extracted from `start_p2p` (ADR-017, CopyPaste-vp63.2) — moved verbatim, no
//! behavior change.

use std::sync::Arc;

use anyhow::Context as _;
use tokio_util::sync::CancellationToken;

use copypaste_p2p::discovery::DiscoveryService;

/// Register the P2P service via mDNS-SD, gated on `lan_visibility`.
///
/// Advertises the bootstrap port in `bport` when available (v2); else v1.
/// When `lan_visibility` is false the registration (and the later browse task)
/// is skipped entirely so the device is invisible on the LAN. The mTLS
/// listener is still bound so paired peers with a persisted address can
/// connect directly.
///
/// # Errors
/// Returns an error if the underlying mDNS-SD `register` / `register_with_bport`
/// call fails.
pub(super) fn register_mdns(
    discovery: &DiscoveryService,
    lan_visibility: bool,
    actual_port: u16,
    device_id_str: &str,
    device_name: &str,
    bootstrap_port: Option<u16>,
) -> anyhow::Result<()> {
    if lan_visibility {
        // Advertise the bootstrap port in `bport` when available (v2); else v1.
        let register_result = match bootstrap_port {
            Some(bport) => {
                discovery.register_with_bport(actual_port, device_id_str, device_name, bport)
            }
            None => discovery.register(actual_port, device_id_str, device_name),
        };
        register_result.context("mDNS register failed")?;
    } else {
        tracing::info!("lan_visibility=false: skipping mDNS-SD registration and browsing");
    }
    Ok(())
}

/// Spawn the long-running mDNS-SD browse+advertise task, gated on
/// `lan_visibility`.
///
/// Only starts the browse+advertise loop when `lan_visibility` is enabled.
/// When off the discovery service is still available (the IPC server holds a
/// reference for peer resolution) but does not advertise or browse, so the
/// device is invisible on the LAN.
pub(super) fn spawn_discovery_task(
    discovery: Arc<DiscoveryService>,
    device_name: String,
    actual_port: u16,
    lan_visibility: bool,
    shutdown: CancellationToken,
) {
    if !lan_visibility {
        return;
    }
    tokio::spawn(async move {
        match discovery.start().await {
            Ok(handle) => {
                tracing::info!(
                    port = actual_port,
                    device_name = %device_name,
                    "mDNS-SD discovery service running"
                );
                // CopyPaste-ydhw: race the mDNS handle against shutdown.
                //
                // The `rescan_discovered` IPC handler calls `disc.start()`
                // which triggers `shutdown_inner()` inside the
                // `DiscoveryService`, aborting the browse JoinHandle this
                // select! arm is waiting on.  When that happens the `_ =
                // handle` arm fires and this task exits — that is now
                // intentional.  `rescan_discovered` stores its replacement
                // browse handle in `IpcServer::discovery_browse_handle` and
                // owns the lifecycle from that point on; this task gracefully
                // hands off rather than leaking or double-running.
                //
                // BUG F1 (original): race the mDNS handle against
                // cancellation so the task exits promptly on daemon shutdown
                // instead of awaiting `handle` forever.
                tokio::select! {
                    result = handle => {
                        match result {
                            Ok(()) => {
                                // Browse loop exited normally (channel closed).
                                tracing::debug!("mDNS-SD browse loop exited");
                            }
                            Err(e) if e.is_cancelled() => {
                                // Handle was aborted — most likely by a
                                // `rescan_discovered` call which restarts the
                                // browse in-place.  The IPC server now owns
                                // the new handle; this task exits cleanly.
                                tracing::debug!(
                                    "mDNS-SD browse handle aborted (likely rescan) \
                                     — discovery task exiting, IPC server owns new handle"
                                );
                            }
                            Err(e) => {
                                tracing::warn!("mDNS-SD browse task panicked: {e}");
                            }
                        }
                    }
                    _ = shutdown.cancelled() => {
                        tracing::info!("mDNS-SD discovery task shutting down");
                    }
                }
            }
            Err(e) => {
                tracing::warn!("mDNS-SD discovery failed to start: {e}");
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BUG F1 (verification follow-up): the mDNS discovery task (spawned by
    /// `spawn_discovery_task`) awaits its `DiscoveryService::start()` handle
    /// raced against cancellation. The task body performs real mDNS
    /// registration, so it cannot be unit-tested without multicast. This test
    /// asserts the exact, narrowest cancellable unit instead: a `select!` of a
    /// never-completing future against `shutdown.cancelled()` resolves to the
    /// cancel arm — i.e. the same structure that guards the discovery handle
    /// exits promptly on cancel.
    #[tokio::test(flavor = "multi_thread")]
    async fn cancellation_token_stops_discovery_select() {
        let token = CancellationToken::new();
        let handle = {
            let token = token.clone();
            tokio::spawn(async move {
                // Mirror the discovery task's guard: a long-lived handle future
                // (here a never-resolving future) raced against cancellation.
                tokio::select! {
                    _ = std::future::pending::<()>() => unreachable!("handle never completes"),
                    _ = token.cancelled() => {}
                }
            })
        };
        token.cancel();
        let joined = tokio::time::timeout(std::time::Duration::from_secs(5), handle).await;
        assert!(
            joined.is_ok(),
            "BUG F1: discovery task select must exit promptly on token cancel"
        );
        joined.unwrap().unwrap();
    }
}
