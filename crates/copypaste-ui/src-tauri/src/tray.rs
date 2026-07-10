//! System tray icon, menu construction, and background resync threads for
//! Private Mode and the Recent submenu.

use std::sync::Mutex;
use tauri::Manager;

use copypaste_ipc::{
    METHOD_COPY_ITEM, METHOD_GET_PRIVATE_MODE, METHOD_HISTORY_PAGE, METHOD_SET_PRIVATE_MODE,
};

// ---------------------------------------------------------------------------
// Managed state
// ---------------------------------------------------------------------------

/// Handle to the "Private Mode" tray CheckMenuItem so the startup-race
/// re-sync handler (V-21-A) can update the checkmark after the daemon is
/// confirmed ready — without re-entering `setup_tray`.
///
/// `CheckMenuItem<Wry>` is internally `Arc`-backed, so cloning and storing
/// here is cheap.  The `Option` is `None` until `setup_tray` runs.
pub(crate) struct PrivateModeMenuItem(
    pub(crate) Mutex<Option<tauri::menu::CheckMenuItem<tauri::Wry>>>,
);

/// Handle to the "Recent" tray Submenu so the background poller can
/// rebuild it once the daemon is ready and periodically thereafter.
///
/// `Submenu<Wry>` is internally `Arc`-backed; cloning is cheap.
/// The `Option` is `None` until `setup_tray` runs.
pub(crate) struct RecentSubmenu(pub(crate) Mutex<Option<tauri::menu::Submenu<tauri::Wry>>>);

/// Stop flag for `spawn_tray_recent_resync`. Set to `true` in `RunEvent::Exit`
/// so the background polling loop exits cleanly instead of holding the
/// `AppHandle` forever and blocking teardown.
pub(crate) struct TrayResyncStop(pub(crate) std::sync::Arc<std::sync::atomic::AtomicBool>);

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Truncate a preview string to at most `max_chars` characters, collapsing
/// interior newlines to a single space and appending "…" when cut.
pub(crate) fn truncate_preview(s: &str, max_chars: usize) -> String {
    // Collapse newlines / tabs into a space so the label is single-line.
    let flat: String = s
        .chars()
        .map(|c| {
            if c == '\n' || c == '\r' || c == '\t' {
                ' '
            } else {
                c
            }
        })
        .collect();
    let flat = flat.trim();
    let chars: Vec<char> = flat.chars().collect();
    if chars.len() <= max_chars {
        chars.iter().collect()
    } else {
        // Leave room for the ellipsis character.
        let cut: String = chars[..max_chars.saturating_sub(1)].iter().collect();
        format!("{}…", cut.trim_end())
    }
}

// ---------------------------------------------------------------------------
// Tray setup
// ---------------------------------------------------------------------------

/// Build and register the menu-bar tray icon.
///
/// Gracefully degrades when the daemon is offline: Recent submenu shows a
/// disabled "No recent items" entry, and Private Mode defaults to unchecked.
pub(crate) fn setup_tray(app: &tauri::App) -> tauri::Result<()> {
    use tauri::menu::{
        CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
    };
    use tauri::tray::TrayIconBuilder;

    // --- "Open CopyPaste" ---
    let open = MenuItemBuilder::with_id("open", "Open CopyPaste").build(app)?;

    // --- "Recent" submenu ---
    // `setup_tray` runs synchronously on the main thread during app startup
    // (CopyPaste-8ebg.23): it must NOT block on IPC here, or app launch stalls
    // for up to the daemon read-timeout. Build with a placeholder and let
    // `spawn_tray_recent_resync` (already started right after `setup_tray`
    // returns, see `lib.rs`) populate the real items in the background.
    let recent_submenu = {
        let mut builder = SubmenuBuilder::new(app, "Recent");
        let placeholder = MenuItemBuilder::with_id("recent:none", "No recent items")
            .enabled(false)
            .build(app)?;
        builder = builder.item(&placeholder);
        builder.build()?
    };

    // Store a clone of the submenu handle in managed state so the background
    // poller can rebuild it without re-entering setup_tray.
    {
        let state: tauri::State<RecentSubmenu> = app.state();
        let mut guard = state.0.lock().expect("mutex poisoned");
        *guard = Some(recent_submenu.clone());
    }

    // --- "Private Mode" check item ---
    // Same rationale as the Recent submenu above: do not block the main thread
    // on IPC during startup. Default to unchecked and let
    // `spawn_tray_private_mode_resync` (started right after `setup_tray`
    // returns) write the real daemon value once it responds.
    let private_mode_on = false;

    let private_mode = CheckMenuItemBuilder::with_id("private_mode", "Private Mode")
        .checked(private_mode_on)
        .build(app)?;

    // V-21-A: Store the CheckMenuItem in managed state so the background
    // daemon-ready poller (`spawn_tray_private_mode_resync`) can re-sync the
    // checkmark once the socket becomes available after a startup race.
    // CheckMenuItem<Wry> is Arc-backed; storing a clone here is cheap.
    {
        let state: tauri::State<PrivateModeMenuItem> = app.state();
        let mut guard = state.0.lock().expect("mutex poisoned");
        *guard = Some(private_mode.clone());
    }

    // Second clone used by the on_menu_event closure below (V-21-B rollback).
    // CheckMenuItem<R> is internally Arc-backed, so clone is cheap.
    let private_mode_clone = private_mode.clone();

    // --- Separator + Quit ---
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit CopyPaste").build(app)?;

    let menu = MenuBuilder::new(app)
        .items(&[&open, &recent_submenu])
        .item(&private_mode)
        .item(&separator)
        .item(&quit)
        .build()?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            let id = event.id().as_ref();
            match id {
                "open" => crate::popup::show_main(app),
                "quit" => app.exit(0),
                "private_mode" => {
                    // Tauri pre-toggles the checkmark before firing the event.
                    // Read the new (already-toggled) state from the cloned item.
                    let new_state = private_mode_clone.is_checked().unwrap_or(false);
                    // CopyPaste-8ebg.23: `on_menu_event` runs on the main thread, so a
                    // blocking IPC call here (up to the read-timeout, ipc.rs) freezes
                    // the UI on every click. Offload to a background thread, same
                    // pattern as `spawn_tray_private_mode_resync` already uses to
                    // mutate the CheckMenuItem off the main thread.
                    let app = app.clone();
                    let private_mode_clone = private_mode_clone.clone();
                    std::thread::spawn(move || {
                        let result = crate::ipc::call(
                            METHOD_SET_PRIVATE_MODE,
                            serde_json::json!({ "enabled": new_state }),
                        );
                        match result {
                            Ok(_) => {
                                // M4: Broadcast the confirmed toggle so the Settings
                                // window (and any other listener) converges on the
                                // same value, regardless of where the toggle began.
                                let _ =
                                    tauri::Emitter::emit(&app, "private-mode-changed", new_state);
                            }
                            Err(e) => {
                                // V-21-B: IPC failed — the daemon did not change state.
                                // Revert the checkmark so the tray reflects daemon truth
                                // rather than staying in the (incorrect) toggled position.
                                tracing::warn!("set_private_mode IPC error (reverting tray): {e}");
                                let _ = private_mode_clone.set_checked(!new_state);
                                // Broadcast the reverted (daemon-truth) value too.
                                let _ =
                                    tauri::Emitter::emit(&app, "private-mode-changed", !new_state);
                            }
                        }
                    });
                }
                other if other.starts_with("recent:") && other != "recent:none" => {
                    // CopyPaste-8ebg.23: same rationale as the "private_mode" arm —
                    // move the blocking `copy_item` IPC call off the main thread.
                    let item_id = other["recent:".len()..].to_string();
                    std::thread::spawn(move || {
                        let result = crate::ipc::call(
                            METHOD_COPY_ITEM,
                            serde_json::json!({ "id": item_id }),
                        );
                        match &result {
                            Ok(reply) if reply.ok => {
                                // Mirror the sound/notification that row-click copy fires so
                                // tray copies are consistent with the "always sound on copy"
                                // promise (audit finding P1 / M12 parity).
                                crate::popup::play_copy_sound();
                                // Build rich title + body from the content_type / preview
                                // returned by the copy_item IPC response.
                                let (title, body) =
                                    crate::notifications::notification_title_body_from_reply(reply);
                                crate::notifications::show_copy_notification(title, body);
                            }
                            Ok(_) => {}
                            Err(e) => {
                                tracing::warn!("copy_item IPC error: {e}");
                            }
                        }
                    });
                }
                _ => {}
            }
        });

    let tray_img =
        tauri::image::Image::from_bytes(include_bytes!("../../assets/tray-icon-32.png"))?;
    builder = builder.icon(tray_img).icon_as_template(true);
    builder.build(app)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Background submenu rebuild
// ---------------------------------------------------------------------------

/// Rebuild the Recent tray submenu from a fresh history_page call.
///
/// Clears all existing items in the submenu and repopulates with up to 10
/// items from the daemon. Falls back to the "No recent items" placeholder if
/// the daemon is offline or returns an empty list. The submenu handle is
/// shared via `RecentSubmenu` managed state; no re-registration of the tray
/// menu is needed because `Submenu<Wry>` is Arc-backed and mutations are
/// reflected live in the displayed menu.
///
/// The existing `on_menu_event` handler dispatches on `other.starts_with("recent:")`
/// so it automatically handles any item ID written here — no re-registration needed.
fn rebuild_recent_submenu(
    handle: &tauri::AppHandle,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use tauri::menu::MenuItemBuilder;

    let state = handle
        .try_state::<RecentSubmenu>()
        .ok_or("RecentSubmenu state not registered")?;
    let guard = state.0.lock().map_err(|e| format!("mutex poisoned: {e}"))?;
    let submenu = guard
        .as_ref()
        .ok_or("RecentSubmenu not yet populated by setup_tray")?;

    // Fetch up to 10 items. On any error, fall back to a placeholder.
    let items_opt: Option<Vec<(String, String)>> = crate::ipc::call(
        METHOD_HISTORY_PAGE,
        serde_json::json!({ "limit": 10, "offset": 0 }),
    )
    .ok()
    .and_then(|reply| {
        if !reply.ok {
            return None;
        }
        reply
            .data
            .as_ref()
            .and_then(|d| d["items"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let id = item["id"].as_str()?.to_owned();
                        let preview = item["preview"].as_str().unwrap_or("").to_owned();
                        Some((id, preview))
                    })
                    .collect::<Vec<_>>()
            })
    });

    // Remove all existing items (iterate in reverse so indices stay valid).
    let existing = submenu.items()?;
    for i in (0..existing.len()).rev() {
        let _ = submenu.remove_at(i);
    }

    // Append fresh items.
    match items_opt {
        Some(items) if !items.is_empty() => {
            for (id, preview) in &items {
                let label = truncate_preview(preview, 40);
                let menu_id = format!("recent:{id}");
                let item = MenuItemBuilder::with_id(menu_id, label).build(handle)?;
                submenu.append(&item)?;
            }
        }
        _ => {
            let placeholder = MenuItemBuilder::with_id("recent:none", "No recent items")
                .enabled(false)
                .build(handle)?;
            submenu.append(&placeholder)?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Background pollers
// ---------------------------------------------------------------------------

/// V-21-A: Startup-race tray re-sync.
///
/// `setup_tray` runs synchronously during app setup, before the daemon socket
/// is necessarily bound.  If `get_private_mode` fails at that point the
/// checkmark defaults to false even though the daemon may have loaded
/// `private_mode = true` from its persisted settings.  This function spawns a
/// background thread that polls with short sleeps until the daemon responds,
/// then writes the real value back to the CheckMenuItem.
///
/// ## Stale-daemon race
///
/// `ensure_daemon_running_async` evicts any old daemon (SIGTERM + socket-
/// released poll) and spawns a fresh one, but this runs on a **separate**
/// background thread.  Because `setup_tray` and this resync thread both start
/// before eviction completes, the first successful IPC reply may come from the
/// **old** daemon (still alive during its graceful shutdown window).  If the
/// old daemon's in-memory state differs from the persisted file (e.g. a prior
/// `persist_private_mode` write failed silently), we would cache the stale
/// value and exit, leaving the tray desynchronised from the new daemon.
///
/// Guard: require **two consecutive, identical** successful IPC replies before
/// exiting.  The old daemon typically closes its socket within ~100 ms of
/// SIGTERM; the 250 ms poll interval gives it time to die.  If the first reply
/// came from the old daemon and the second call fails (socket gone), the
/// counter resets and we keep polling until the new daemon is stable.
pub(crate) fn spawn_tray_private_mode_resync(handle: tauri::AppHandle) {
    use std::thread;
    use std::time::{Duration, Instant};

    thread::spawn(move || {
        const POLL_INTERVAL: Duration = Duration::from_millis(250);
        const GIVE_UP_AFTER: Duration = Duration::from_secs(30);
        // Two consecutive identical replies are required before exiting.
        // This prevents caching a stale response from a dying old daemon.
        const CONFIRM_ROUNDS: usize = 2;

        let deadline = Instant::now() + GIVE_UP_AFTER;
        let mut last_value: Option<bool> = None;
        let mut confirm_count: usize = 0;

        loop {
            if Instant::now() >= deadline {
                tracing::warn!(
                    "tray private-mode re-sync: daemon not ready after 30 s — giving up"
                );
                return;
            }

            let result = crate::ipc::call(METHOD_GET_PRIVATE_MODE, serde_json::json!({}));
            match result {
                Ok(reply) if reply.ok => {
                    let real_value = reply
                        .data
                        .as_ref()
                        .and_then(|d| d["private_mode"].as_bool())
                        .unwrap_or(false);

                    // Track consecutive identical responses to confirm stability.
                    if last_value == Some(real_value) {
                        confirm_count += 1;
                    } else {
                        last_value = Some(real_value);
                        confirm_count = 1;
                    }

                    // Update the CheckMenuItem immediately with the best-known value.
                    if let Some(state) = handle.try_state::<PrivateModeMenuItem>() {
                        if let Ok(guard) = state.0.lock() {
                            if let Some(ref item) = *guard {
                                // Only write if the value actually differs from
                                // what setup_tray already set, to avoid a
                                // spurious visual flicker.
                                let current = item.is_checked().unwrap_or(!real_value);
                                if current != real_value {
                                    tracing::info!(
                                        "tray private-mode re-sync: {} → {}",
                                        current,
                                        real_value
                                    );
                                    let _ = item.set_checked(real_value);
                                }
                            }
                        }
                    }

                    if confirm_count >= CONFIRM_ROUNDS {
                        // Stable for CONFIRM_ROUNDS consecutive polls — done.
                        return;
                    }
                    // Wait before the next confirmation poll.
                    thread::sleep(POLL_INTERVAL);
                }
                // Daemon not yet ready or socket changed; reset stability counter.
                _ => {
                    last_value = None;
                    confirm_count = 0;
                    thread::sleep(POLL_INTERVAL);
                }
            }
        }
    });
}

/// Startup-race + periodic Recent submenu resync.
/// `setup_tray` runs at startup before the daemon socket is necessarily bound,
/// so the Recent submenu often shows a placeholder. This function spawns a
/// background thread that:
///
/// 1. Polls until the daemon responds, then does an initial rebuild.
/// 2. Continues polling every `REFRESH_INTERVAL` so the tray stays current as
///    the user copies things.
/// 3. On each poll, checks whether a new clipboard item appeared (by comparing
///    the most recent item's `wall_time` to the last seen value) and, if so,
///    fires a rich `UNUserNotificationCenter` banner — respecting the daemon's
///    `notify_on_copy` setting.  This bridges background-clipboard-captures
///    (items copied from other apps while the UI is running) to the Tauri
///    bundle so they show the CopyPaste app icon rather than a generic icon.
///
/// The refresh is intentionally cheap: 1-item `history_page` call, only runs
/// while the app is alive, stops after `GIVE_UP_AFTER` of daemon silence.
pub(crate) fn spawn_tray_recent_resync(handle: tauri::AppHandle) {
    use std::sync::atomic::Ordering;
    use std::thread;
    use std::time::{Duration, Instant};

    // Grab the stop flag from managed state so the RunEvent::Exit handler can
    // signal this thread to exit cleanly without holding the AppHandle.
    let stop_flag: std::sync::Arc<std::sync::atomic::AtomicBool> = handle
        .try_state::<TrayResyncStop>()
        .map(|s| std::sync::Arc::clone(&s.0))
        .unwrap_or_else(|| std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)));

    thread::spawn(move || {
        /// How long to wait between refreshes once the daemon is up.
        const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
        /// Poll interval while waiting for the daemon to come up initially.
        const POLL_INTERVAL: Duration = Duration::from_millis(250);
        /// Give up entirely if the daemon never responds within this window.
        const GIVE_UP_AFTER: Duration = Duration::from_secs(30);

        // Phase 1: wait for the daemon to become ready.
        let deadline = Instant::now() + GIVE_UP_AFTER;
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }
            if Instant::now() >= deadline {
                tracing::warn!("tray Recent re-sync: daemon not ready after 30 s — giving up");
                return;
            }

            // A successful, ok=true history_page reply is the readiness signal.
            let ready = crate::ipc::call(
                METHOD_HISTORY_PAGE,
                serde_json::json!({ "limit": 1, "offset": 0 }),
            )
            .map(|r| r.ok)
            .unwrap_or(false);

            if ready {
                break;
            }
            thread::sleep(POLL_INTERVAL);
        }

        // Seed the "last seen" wall_time so the first poll doesn't fire a
        // spurious notification for an item that was already in history before
        // the app launched.
        let mut last_seen_wall_time: i64 = {
            crate::ipc::call(
                METHOD_HISTORY_PAGE,
                serde_json::json!({ "limit": 1, "offset": 0 }),
            )
            .ok()
            .and_then(|r| r.data)
            .and_then(|d| d["items"].as_array().and_then(|a| a.first().cloned()))
            .and_then(|item| item["wall_time"].as_i64())
            .unwrap_or(0)
        };

        // Phase 2: rebuild now and then periodically; exit when stop flag is set.
        loop {
            if stop_flag.load(Ordering::Relaxed) {
                return;
            }
            if let Err(e) = rebuild_recent_submenu(&handle) {
                tracing::warn!("tray Recent re-sync: rebuild failed: {e}");
            }

            // Check for a background-captured item (clipboard copy from another
            // app).  Query the most recent item and compare its wall_time.
            // If it is newer AND the daemon config has notify_on_copy enabled,
            // fire a rich UNUserNotificationCenter banner.
            crate::notifications::check_and_notify_new_capture(&mut last_seen_wall_time);

            thread::sleep(REFRESH_INTERVAL);
        }
    });
}
