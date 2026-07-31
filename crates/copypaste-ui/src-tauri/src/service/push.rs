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

use copypaste_ipc::{EventData, EventKind};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::backend::{Backend, BackendError, SelectedBackend};

/// Something changed; re-read through the ordinary commands.
///
/// Carries no clipboard content — the wire event does not either. A subscriber
/// re-reads through `list`/`search`, so there is one set of rules about what
/// the WebView may see rather than two.
pub const EVENT_CHANGED: &str = "copypaste://changed";

/// Whether the change stream is currently delivering.
pub const EVENT_PUSH_STATE: &str = "copypaste://push-state";

/// How long to wait before reconnecting a stream that ended.
const RECONNECT_MIN: Duration = Duration::from_millis(500);
const RECONNECT_MAX: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ChangePayload {
    pub topic: EventKind,
    pub item_count: u64,
    /// Detected secrets the auto-wipe sweep deleted in this change; zero on
    /// every other one.
    ///
    /// Forwarded rather than dropped because it is the only history change the
    /// user did not ask for, and a deletion nobody is told about is CLAUDE.md
    /// rule 4's worst outcome arriving quietly. A count, never ids: the rows
    /// are gone and the event carries no content either way.
    pub swept: u32,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct PushStatePayload {
    pub live: bool,
}

/// Run the subscription for the life of the app.
///
/// Spawned once from `crate::run`. It outlives any individual screen on
/// purpose: the tray and the menu-bar badge want to know about changes while
/// the window is hidden, and re-subscribing on every window show would burn a
/// connection per gesture.
pub fn spawn<R: Runtime>(app: AppHandle<R>) {
    tauri::async_runtime::spawn(async move {
        let mut delay = RECONNECT_MIN;
        loop {
            match subscribe(&app).await {
                // The stream ended cleanly — the daemon stopped or was
                // restarted. Reconnect, backing off so a daemon that is down
                // is not hammered.
                Ok(()) => {}
                Err(BackendError::Unsupported(_)) => {
                    // This build has no daemon to subscribe to (Android).
                    // Nothing will change that, so stop rather than retry
                    // forever; the frontend polls.
                    set_live(&app, false);
                    return;
                }
                Err(error) => {
                    tracing::debug!(%error, "the change stream is not available");
                }
            }
            set_live(&app, false);
            tokio::time::sleep(delay).await;
            delay = (delay * 2).min(RECONNECT_MAX);
        }
    });
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
        EVENT_CHANGED,
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
    let _ = app.emit(EVENT_PUSH_STATE, PushStatePayload { live });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names are a contract with `src/hooks/usePush.ts`, which cannot
    /// import them. Spelling one of them differently on either side produces a
    /// UI that silently never updates, so they are asserted rather than left to
    /// a grep.
    #[test]
    fn the_event_names_are_the_ones_the_frontend_listens_for() {
        assert_eq!(EVENT_CHANGED, "copypaste://changed");
        assert_eq!(EVENT_PUSH_STATE, "copypaste://push-state");
    }

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

    #[test]
    fn the_reconnect_delay_is_bounded() {
        let mut delay = RECONNECT_MIN;
        for _ in 0..20 {
            delay = (delay * 2).min(RECONNECT_MAX);
        }
        assert_eq!(delay, RECONNECT_MAX);
    }
}
