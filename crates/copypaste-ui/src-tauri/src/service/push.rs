//! Turning the daemon's change stream into WebView events.
//!
//! Parity finding 15: v1's clients subscribed, v2's app polled every three
//! seconds. Polling costs up to three seconds of lag on every copy and a
//! constant trickle of IPC on a machine nobody is touching.
//!
//! # Push accelerates, it does not replace
//!
//! The frontend keeps a poll running while push is live — a slow one. A
//! subscription can die in ways neither end notices promptly, and a UI that
//! stopped polling because it believed it was subscribed shows stale history
//! indefinitely. `copypaste_cloud::sync::cadence` reached the same conclusion
//! about Realtime, and manifest 05 §5.4 calls the backstop "the only item that
//! can silently reintroduce data loss". Same shape, same answer.
//!
//! So this emits two things: the changes, and whether it is delivering them.
//! The second is what lets the frontend choose its poll interval honestly
//! instead of guessing.

use std::time::Duration;

use backon::BackoffBuilder;
use copypaste_ipc::{EventData, EventKind};
use copypaste_retry::stream_reconnect_backoff;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};
use tokio_util::sync::CancellationToken;

use crate::backend::{Backend, BackendError, SelectedBackend};
use crate::events::TauriEventName;

/// Owns the app-wide push subscription and its shutdown signal.
pub struct PushMonitor {
    shutdown: CancellationToken,
}

impl PushMonitor {
    /// Cancel an active subscription or reconnect wait.
    pub fn stop(&self) {
        self.shutdown.cancel();
    }
}

impl Drop for PushMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct ChangePayload {
    pub topic: EventKind,
    pub item_count: u64,
    /// Detected secrets the auto-wipe sweep deleted in this change; zero on
    /// every other one.
    ///
    /// Forwarded rather than dropped because it is the only history change the
    /// user did not ask for, and a deletion nobody is told about is AGENTS.md
    /// rule 4's worst outcome arriving quietly. A count, never ids: the rows
    /// are gone and the event carries no content either way.
    pub swept: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
pub struct PushStatePayload {
    pub live: bool,
}

/// Run the subscription for the life of the app.
///
/// Spawned once from `crate::run`. It outlives any individual screen on
/// purpose: the tray and the menu-bar badge want to know about changes while
/// the window is hidden, and re-subscribing on every window show would burn a
/// connection per gesture.
pub fn spawn<R: Runtime>(app: AppHandle<R>) -> PushMonitor {
    let monitor = PushMonitor {
        shutdown: CancellationToken::new(),
    };
    let shutdown = monitor.shutdown.clone();
    tauri::async_runtime::spawn(run(app, shutdown));
    monitor
}

async fn run<R: Runtime>(app: AppHandle<R>, shutdown: CancellationToken) {
    let policy = stream_reconnect_backoff();
    let mut schedule = policy.build();
    loop {
        let result = tokio::select! {
            biased;
            () = shutdown.cancelled() => {
                set_live(&app, false);
                return;
            }
            result = subscribe(&app) => result,
        };

        match result {
            // A stream that was live resets the next reconnect to the floor.
            Ok(()) => schedule = policy.build(),
            Err(BackendError::Unsupported(_)) => {
                set_live(&app, false);
                return;
            }
            Err(error) => {
                tracing::debug!(%error, "the change stream is not available");
            }
        }
        set_live(&app, false);

        let Some(delay) = schedule.next() else {
            return;
        };
        if wait_to_reconnect(delay, &shutdown).await {
            return;
        }
    }
}

async fn wait_to_reconnect(delay: Duration, shutdown: &CancellationToken) -> bool {
    tokio::select! {
        biased;
        () = shutdown.cancelled() => true,
        () = tokio::time::sleep(delay) => false,
    }
}

/// One subscription, from connect to hang-up.
async fn subscribe<R: Runtime>(app: &AppHandle<R>) -> Result<(), BackendError> {
    let mut events = {
        let backend = app.state::<SelectedBackend>();
        backend.watch().await?
    };

    set_live(app, true);
    while let Some(event) = events.recv().await {
        emit_change(app, event);
    }
    Ok(())
}

fn emit_change<R: Runtime>(app: &AppHandle<R>, event: EventData) {
    let _ = app.emit(
        TauriEventName::Changed.as_str(),
        ChangePayload {
            topic: event.event,
            item_count: event.item_count,
            swept: event.swept,
        },
    );
    // Not forwarded to the WebView: the notification is posted natively, and a
    // hidden window's React tree is not a surface that could post one anyway.
    if event.captured {
        crate::shell::notify::on_capture(app);
    }
}

fn set_live<R: Runtime>(app: &AppHandle<R>, live: bool) {
    let _ = app.emit(
        TauriEventName::PushState.as_str(),
        PushStatePayload { live },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The payload the WebView receives must name the topic in the same words
    /// the wire does, so a client can tell an item change from a peer change
    /// without a lookup table.
    #[test]
    fn a_change_payload_names_its_topic_and_carries_no_content() {
        let json = serde_json::to_string(&ChangePayload {
            topic: EventKind::Items,
            item_count: 12,
            swept: 0,
        })
        .unwrap();
        assert_eq!(json, r#"{"topic":"items","item_count":12,"swept":0}"#);
    }

    /// The count is on the frame the frontend already listens to, so a sweep
    /// announces itself without a second subscription to keep alive.
    #[test]
    fn a_sweep_reaches_the_payload_with_its_count() {
        let payload = ChangePayload {
            topic: EventKind::Items,
            item_count: 3,
            swept: 2,
        };
        let json = serde_json::to_string(&payload).unwrap();
        assert!(json.contains(r#""swept":2"#), "{json}");
    }

    #[tokio::test]
    async fn cancellation_interrupts_the_longest_reconnect_wait() {
        let shutdown = CancellationToken::new();
        let waiting = tokio::spawn({
            let shutdown = shutdown.clone();
            async move { wait_to_reconnect(copypaste_retry::STREAM_RECONNECT_MAX, &shutdown).await }
        });
        tokio::task::yield_now().await;

        shutdown.cancel();
        let cancelled = tokio::time::timeout(Duration::from_secs(1), waiting)
            .await
            .expect("cancellation left the reconnect sleep running")
            .expect("wait task panicked");
        assert!(cancelled);
    }
}
