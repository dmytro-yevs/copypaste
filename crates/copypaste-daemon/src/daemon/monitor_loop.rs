//! Steady-state clipboard monitor loop: poll ticker, periodic sensitive/
//! general TTL cleanups, hot-reload of poll interval + size gates from the
//! live config, and signal/quit-flag handling.

use crate::clipboard::ClipboardMonitor;
use copypaste_core::{AppConfig, ClipboardItem, Database};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex};
use tokio::time::interval;
use tokio_util::sync::CancellationToken;

#[cfg(target_os = "macos")]
use super::FrontmostAppCache;
use super::{handle_tick, run_ttl_cleanup};

/// How often the sensitive-item TTL cleanup runs (milliseconds). Used in both
/// the macOS and non-macOS monitor loops to avoid magic literals.
const SENSITIVE_CLEANUP_INTERVAL_MS: u64 = 5_000;

/// How often the general expires_at TTL cleanup runs (milliseconds).
const GENERAL_CLEANUP_INTERVAL_MS: u64 = 60_000;

/// Run the clipboard monitor poll/cleanup loop until shutdown.
///
/// This owns the daemon's steady-state lifecycle: the poll ticker, the periodic
/// sensitive/general TTL cleanups, hot-reload of poll interval + size gates from
/// the live config, and signal/quit-flag handling. It returns once `Ctrl+C`,
/// `SIGTERM`, or the tray `quit_flag` triggers shutdown — after cancelling
/// `shutdown_token` so the caller can drain the remaining subsystem tasks.
///
/// EXACT-BEHAVIOUR NOTE: the macOS and non-macOS branches differ only in that
/// macOS also checks the tray `quit_flag` at the top of each loop iteration and
/// threads a `FrontmostAppCache` into `handle_tick`. Both are preserved verbatim.
// crh3.78: this is the daemon's steady-state lifecycle loop extracted verbatim
// from `run_with_quit_flag`. Each argument is a distinct, already-constructed
// shared handle the loop needs (db, key, monitor, channels, config, shutdown);
// bundling them into a context struct would add indirection without reducing the
// genuine fan-in, so an explicit allow is clearer than a wrapper type here.
#[allow(clippy::too_many_arguments)]
#[cfg_attr(not(target_os = "macos"), allow(unused_variables))]
pub(crate) async fn run_monitor_loop(
    mut monitor: ClipboardMonitor,
    db: Arc<Mutex<Database>>,
    local_key_arc: Arc<zeroize::Zeroizing<[u8; 32]>>,
    private_mode: Arc<AtomicBool>,
    new_item_tx: broadcast::Sender<ClipboardItem>,
    local_device_id: String,
    core_config_arc: Arc<RwLock<AppConfig>>,
    config: AppConfig,
    quit_flag: Arc<AtomicBool>,
    shutdown_token: CancellationToken,
) -> anyhow::Result<()> {
    // at2m: ticker is `mut` so we can recreate it when poll_interval_ms changes
    // at runtime via set_config.  The current interval value is tracked in
    // `current_poll_ms`; when live_config diverges we replace the interval.
    let mut current_poll_ms = config.poll_interval_ms;
    let mut ticker = interval(Duration::from_millis(current_poll_ms));
    let mut cleanup_ticks: u64 = 0;
    // Sensitive TTL cleanup runs every 5 seconds; track elapsed ticks separately.
    let mut sensitive_cleanup_ticks: u64 = 0;

    tracing::info!("clipboard monitor started");
    tracing::info!(
        "sensitive auto-wipe TTL: {}s, checked every 5s",
        config.sensitive_ttl_secs,
    );

    #[cfg(target_os = "macos")]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())?;
        // CopyPaste-44rq.33: one cache instance shared across all ticks so
        // lsappinfo is forked at most once per FRONTMOST_APP_CACHE_TTL_SECS.
        let mut frontmost_cache = FrontmostAppCache::new();
        loop {
            // Check tray quit flag before blocking on select
            if quit_flag.load(Ordering::Relaxed) {
                tracing::info!("quit flag set, shutting down daemon");
                // D3: ensure all tasks receive the cancellation signal even
                // when the tray host (not a signal) triggers shutdown.
                shutdown_token.cancel();
                break;
            }
            tokio::select! {
                _ = ticker.tick() => {
                    // Hot-reload: snapshot the current live config on every tick
                    // so limit/feature changes from set_config take effect without
                    // a daemon restart (excluded_app_bundle_ids, paste_as_plain_text,
                    // sensitive_ttl_secs, etc.).
                    let live_config = core_config_arc
                        .read()
                        .map(|g| g.clone())
                        .unwrap_or_else(|_| config.clone());
                    // at2m: hot-reload the poll interval when set_config changes it.
                    // Recreating the interval resets its internal deadline to "now",
                    // which is safe: at worst we poll once immediately on the next
                    // select! iteration.  Reset cleanup_ticks to avoid a spurious
                    // early TTL run after a potentially large interval change.
                    if live_config.poll_interval_ms != current_poll_ms {
                        tracing::info!(
                            old_ms = current_poll_ms,
                            new_ms = live_config.poll_interval_ms,
                            "clipboard: poll_interval_ms changed — recreating interval timer"
                        );
                        current_poll_ms = live_config.poll_interval_ms;
                        ticker = interval(Duration::from_millis(current_poll_ms));
                    }
                    // P2: guard sensitive_ttl_secs == 0 → "disabled". When the
                    // user sets ttl to 0 (no auto-wipe), sensitive_ttl_ms would be
                    // 0, making threshold = now_ms - 0 = now_ms which deletes ALL
                    // sensitive items on every tick. Skip the cleanup entirely when
                    // ttl is 0 to honour the "disabled" intent.
                    let sensitive_ttl_ms = if live_config.sensitive_ttl_secs == 0 {
                        None
                    } else {
                        Some(live_config.sensitive_ttl_secs as i64 * 1000)
                    };
                    // Hot-reload the monitor's READ gate from the live config so
                    // raising/lowering the text/image/file cap via set_config
                    // takes effect without a restart (cheap: three field writes per tick).
                    monitor.set_max_text_bytes(live_config.max_text_size_bytes);
                    monitor.set_max_image_bytes(
                        usize::try_from(live_config.max_image_size_bytes).unwrap_or(usize::MAX),
                    );
                    monitor.set_max_file_bytes(
                        usize::try_from(live_config.max_file_size_bytes).unwrap_or(usize::MAX),
                    );
                    handle_tick(&mut monitor, &db, &local_key_arc, &live_config, &private_mode, &new_item_tx, &local_device_id, &mut frontmost_cache).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every SENSITIVE_CLEANUP_INTERVAL_MS.
                    // Integer-divide gives 0 when poll_interval > interval; clamp
                    // to 1 so cleanup runs at most once per tick in that case.
                    let do_sensitive = sensitive_ttl_ms.is_some()
                        && sensitive_cleanup_ticks
                            >= (SENSITIVE_CLEANUP_INTERVAL_MS
                                / current_poll_ms.max(1))
                            .max(1);
                    if do_sensitive {
                        sensitive_cleanup_ticks = 0;
                    }
                    // General expires_at TTL: run every GENERAL_CLEANUP_INTERVAL_MS.
                    let do_general =
                        cleanup_ticks >= (GENERAL_CLEANUP_INTERVAL_MS / current_poll_ms.max(1)).max(1);
                    if do_general {
                        cleanup_ticks = 0;
                    }
                    // daemon-core L1: the deletes are synchronous rusqlite. Run
                    // them on a blocking thread (like the IPC path) so the async
                    // executor is never blocked while the DB lock is held.
                    run_ttl_cleanup(&db, sensitive_ttl_ms.unwrap_or(0), do_sensitive, do_general).await;
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down");
                    quit_flag.store(true, Ordering::Relaxed);
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
                _ = sigterm.recv() => {
                    tracing::info!("SIGTERM received, shutting down");
                    quit_flag.store(true, Ordering::Relaxed);
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        // SIGTERM handling on non-macOS — previously only SIGINT was wired,
        // so launchd/systemd sending SIGTERM would terminate the process
        // without running our cleanup branch (sock file removal, log flush).
        // SIGTERM future: a real terminate-signal stream on unix, a never-resolving
        // future elsewhere. Boxed + always-defined so the select! branch below needs
        // NO in-macro #[cfg] attribute — tokio 1.52's select! macro rejects attributes
        // on branches ("no rules expected `}`", CopyPaste-l07l).
        let mut sigterm_fut: std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> = {
            #[cfg(unix)]
            {
                use tokio::signal::unix::{signal, SignalKind};
                let mut sig = signal(SignalKind::terminate())?;
                Box::pin(async move {
                    sig.recv().await;
                })
            }
            #[cfg(not(unix))]
            {
                Box::pin(std::future::pending::<()>())
            }
        };
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let live_config = core_config_arc
                        .read()
                        .map(|g| g.clone())
                        .unwrap_or_else(|_| config.clone());
                    // at2m: hot-reload the poll interval when set_config changes it.
                    if live_config.poll_interval_ms != current_poll_ms {
                        tracing::info!(
                            old_ms = current_poll_ms,
                            new_ms = live_config.poll_interval_ms,
                            "clipboard: poll_interval_ms changed — recreating interval timer"
                        );
                        current_poll_ms = live_config.poll_interval_ms;
                        ticker = interval(Duration::from_millis(current_poll_ms));
                    }
                    let sensitive_ttl_ms = if live_config.sensitive_ttl_secs == 0 {
                        None
                    } else {
                        Some(live_config.sensitive_ttl_secs as i64 * 1000)
                    };
                    // Hot-reload the monitor's READ gate from the live config so
                    // raising/lowering the text/image/file cap via set_config
                    // takes effect without a restart (cheap: three field writes per tick).
                    monitor.set_max_text_bytes(live_config.max_text_size_bytes);
                    monitor.set_max_image_bytes(
                        usize::try_from(live_config.max_image_size_bytes).unwrap_or(usize::MAX),
                    );
                    monitor.set_max_file_bytes(
                        usize::try_from(live_config.max_file_size_bytes).unwrap_or(usize::MAX),
                    );
                    handle_tick(&mut monitor, &db, &local_key_arc, &live_config, &private_mode, &new_item_tx, &local_device_id).await;
                    cleanup_ticks += 1;
                    sensitive_cleanup_ticks += 1;

                    // Sensitive item TTL: run every SENSITIVE_CLEANUP_INTERVAL_MS.
                    let do_sensitive = sensitive_ttl_ms.is_some()
                        && sensitive_cleanup_ticks
                            >= (SENSITIVE_CLEANUP_INTERVAL_MS
                                / current_poll_ms.max(1))
                            .max(1);
                    if do_sensitive {
                        sensitive_cleanup_ticks = 0;
                    }
                    // General expires_at TTL: run every GENERAL_CLEANUP_INTERVAL_MS.
                    let do_general =
                        cleanup_ticks >= (GENERAL_CLEANUP_INTERVAL_MS / current_poll_ms.max(1)).max(1);
                    if do_general {
                        cleanup_ticks = 0;
                    }
                    // daemon-core L1: offload the synchronous rusqlite deletes.
                    run_ttl_cleanup(&db, sensitive_ttl_ms.unwrap_or(0), do_sensitive, do_general).await;
                }
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("SIGINT received, shutting down");
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
                _ = &mut sigterm_fut => {
                    tracing::info!("SIGTERM received, shutting down");
                    // D3: broadcast shutdown to all tasks.
                    shutdown_token.cancel();
                    break;
                }
            }
        }
    }
    Ok(())
}
