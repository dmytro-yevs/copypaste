//! Background poller for incoming (responder-side) SAS pairing requests.
//!
//! The frontend SAS poll lives inside `DevicesView` which is only mounted while
//! the Devices tab is active.  An inbound discovery-pair from another device
//! therefore goes unnoticed when the user is on any other tab.
//!
//! This module closes that gap with a background thread that polls
//! `pair_get_sas` every ~1 s and, the first time it observes
//! `state == "awaiting_sas"` with `role == "responder"`:
//!
//! 1. Posts a system notification ("CopyPaste — Pairing request").
//! 2. Brings the main window to the foreground (`show_main`).
//! 3. Emits the `"incoming-pairing"` Tauri event carrying the full
//!    `pair_get_sas` JSON so `App.tsx` can switch to the Devices tab and open
//!    the SAS modal pre-seeded with the code.

use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tauri::Manager;

/// Stop flag for `spawn_incoming_pairing_poller`. Same pattern as
/// `TrayResyncStop` — set on `RunEvent::Exit` so the thread exits cleanly.
pub(crate) struct PairingPollStop(pub(crate) Arc<AtomicBool>);

/// Always-on background poller for incoming (responder-side) pairing requests.
///
/// ## De-duplication
///
/// We track the last SAS string for which we fired the notification.  As long
/// as the daemon stays in `awaiting_sas` with the **same** SAS the notification
/// is not repeated (avoids a banner every second while the user reads the
/// code).  When the state leaves `awaiting_sas` we clear the last-seen SAS so
/// a subsequent, distinct pairing episode triggers a fresh notification.
///
/// ## Error handling
///
/// Every IPC error is logged and the loop sleeps before retrying — the thread
/// never panics.
pub(crate) fn spawn_incoming_pairing_poller(handle: tauri::AppHandle) {
    use copypaste_ipc::METHOD_PAIR_GET_SAS;
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::Duration;

    let stop_flag: Arc<AtomicBool> = handle
        .try_state::<PairingPollStop>()
        .map(|s| Arc::clone(&s.0))
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));

    thread::spawn(move || {
        /// How often to poll `pair_get_sas`.
        const POLL_INTERVAL: Duration = Duration::from_millis(1_000);

        // The SAS code for which we already fired a notification this episode.
        // `None` means no active episode; set once we fire, cleared when the
        // daemon leaves `awaiting_sas`.
        let mut notified_sas: Option<String> = None;

        loop {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }

            match crate::ipc::call(METHOD_PAIR_GET_SAS, serde_json::json!({})) {
                Err(e) => {
                    // Socket errors (daemon offline, not yet started, etc.)
                    // are expected during startup and are not worth logging
                    // at warn level every second.  Debug-log and retry.
                    tracing::debug!("incoming-pairing poller: pair_get_sas error: {e}");
                    // Clear notified SAS so a recovered episode fires fresh.
                    notified_sas = None;
                }
                Ok(reply) if !reply.ok => {
                    // Daemon returned an error response — log and continue.
                    tracing::debug!(
                        "incoming-pairing poller: pair_get_sas not-ok: {:?}",
                        reply.error
                    );
                    notified_sas = None;
                }
                Ok(reply) => {
                    let data = reply.data.as_ref();
                    let state = data.and_then(|d| d["state"].as_str()).unwrap_or("idle");
                    let role = data.and_then(|d| d["role"].as_str()).unwrap_or("");

                    if state == "awaiting_sas" && role == "responder" {
                        let sas = data
                            .and_then(|d| d["sas"].as_str())
                            .unwrap_or("")
                            .to_owned();

                        // Only fire notification + focus once per SAS episode.
                        if notified_sas.as_deref() != Some(sas.as_str()) {
                            notified_sas = Some(sas.clone());

                            // 1. Bring the main window to the foreground.
                            crate::popup::show_main(&handle);

                            // 2. Post a system notification.
                            crate::notifications::show_copy_notification(
                                "CopyPaste — Pairing request".to_owned(),
                                "A device wants to pair. Confirm the matching code in the app."
                                    .to_owned(),
                            );

                            // 3. Emit the Tauri event so App.tsx can route to
                            //    the Devices tab and open the SAS modal.
                            if let Err(e) = tauri::Emitter::emit(
                                &handle,
                                "incoming-pairing",
                                reply.data.unwrap_or(serde_json::Value::Null),
                            ) {
                                tracing::warn!("incoming-pairing poller: emit failed: {e}");
                            }
                        }
                    } else {
                        // Any non-awaiting_sas+responder state clears the
                        // de-dup so the next episode fires a fresh notification.
                        if notified_sas.is_some() {
                            notified_sas = None;
                        }
                    }
                }
            }

            thread::sleep(POLL_INTERVAL);
        }
    });
}
