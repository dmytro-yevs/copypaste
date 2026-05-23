//! copypaste-ui — Slint HistoryWindow wired to copypaste-daemon via Unix IPC.
//!
//! Architecture:
//!   - Slint renders the HistoryWindow on the main thread.
//!   - A dedicated background thread polls the daemon IPC socket.
//!   - Results are sent back to the Slint event loop via `slint::invoke_from_event_loop`.
//!   - IPC methods: `history_page` (list), `paste` (activate by id), `status` (health).
//!
//! Data flow:
//!   Slint callback → Rust callback closure → IPC call → slint::invoke_from_event_loop → Slint update

mod ipc_client;

use anyhow::Result;
use ipc_client::{format_wall_time, IpcClient};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

// Include generated Slint bindings.
slint::include_modules!();

// server enforces MAX_PAGE=1000 (see ipc_client::MAX_PAGE / Wave 2.3);
// PAGE_SIZE is well under that, but ipc_client::history_page() also clamps
// at MAX_PAGE so any future bump here stays safe.
const PAGE_SIZE: u64 = 50;

/// Application state shared between callbacks.
struct AppState {
    socket_path: PathBuf,
    current_offset: u64,
    /// beta-bonus (slint-search): the most recent unfiltered page fetched
    /// from the daemon. Cached so the `search-changed` callback can
    /// re-filter without a second IPC round-trip per keystroke.
    last_page_items: Vec<ipc_client::HistoryEntry>,
    /// Most recent `page.total` reported by the daemon, or `None` before
    /// the first successful `history_page` reply. Used by
    /// `on_load_next_page` to refuse advancing past the last page (the
    /// daemon returns an empty slice for out-of-range offsets, which
    /// previously left the UI on a blank "Showing 51-50 of 50" row).
    last_known_total: Option<u64>,
}

impl AppState {
    fn new() -> Self {
        let socket_path = home::home_dir()
            .expect("HOME must exist")
            .join("Library/Application Support/CopyPaste/daemon.sock");
        Self {
            socket_path,
            current_offset: 0,
            last_page_items: Vec::new(),
            last_known_total: None,
        }
    }
}

/// Adapter so the generic `filter_history_items` in the library crate can
/// match against the daemon's `HistoryEntry` without leaking Slint types
/// into the library.
impl copypaste_ui::windows::SearchableHistoryItem for ipc_client::HistoryEntry {
    fn preview(&self) -> &str {
        &self.preview
    }
}

/// Acquire a `Mutex` guard, recovering from poisoning instead of panicking.
///
/// Every callback in this binary holds an `Arc<Mutex<AppState>>`. A panic
/// on the UI thread (e.g. a bug in a Slint property setter) would poison
/// the mutex and turn every subsequent `state.lock().unwrap()` into a
/// hard crash of the whole process — the user loses access to clipboard
/// history because of an unrelated transient panic in another callback.
///
/// `AppState` is a value type with no resource invariants that depend on
/// the panic site, so recovering the inner guard is safe: the only
/// observable effect of an interrupted critical section is whatever
/// half-finished assignment the panicking thread left behind, and the
/// next call site will overwrite or read past it.
///
/// We deliberately avoid pulling in `parking_lot` for this — std's
/// `unwrap_or_else(|e| e.into_inner())` is the canonical recovery
/// idiom and keeps the dependency footprint flat.
fn lock_or_recover<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Bring the UI process to the foreground.
///
/// `LSUIElement=true` (set in Info.plist so the app does not show up in the
/// Dock) means a freshly-shown window stays *behind* whatever the user was
/// using. `NSApplication::activate` is the modern (Sonoma+) replacement for
/// the deprecated `activateIgnoringOtherApps:` and works back to macOS 11 via
/// the AppKit shim.
///
/// Tray menu callbacks are invoked from the Slint event loop, which runs on
/// the main thread on macOS, so `MainThreadMarker::new()` should always
/// succeed here. If it ever returns `None` we silently no-op — losing focus
/// activation is preferable to crashing the UI.
#[cfg(target_os = "macos")]
fn activate_app() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        app.activate();
    }
}

#[cfg(not(target_os = "macos"))]
fn activate_app() {}

fn main() -> Result<()> {
    // Beta-bonus i18n: bind the gettext domain (auto-set from CARGO_PKG_NAME =
    // "copypaste-ui" by slint-build) to the `lang/` catalog directory shipped
    // with the crate. At runtime Slint resolves `@tr("…")` against
    // `lang/<locale>/LC_MESSAGES/copypaste-ui.mo`; missing locales fall back
    // to the literal msgid. Locale is selected by LC_ALL / LANG / LC_MESSAGES.
    slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang"));

    let window = HistoryWindow::new()?;
    let state = Arc::new(Mutex::new(AppState::new()));

    // Beta hot-fix: on macOS, install the Launch Agent plist + bootstrap the
    // daemon in the background so the user does not have to run
    // `copypaste daemon install && copypaste daemon start` after a fresh DMG
    // install. Runs in a dedicated thread — UI rendering must NOT block on
    // launchctl. See `crates/copypaste-ui/src/autostart.rs` for the flow.
    //
    // beta.5 Bug-2/3: once autostart succeeds, post a UI event that re-runs
    // the initial history load. Without this, the first load fires before
    // the daemon socket exists, the UI caches "Daemon not running", and the
    // user is stuck unless they click Refresh manually. Spawned AFTER the
    // window + state are created so the closure can capture a weak window
    // handle and the shared `AppState` for the post-startup refresh.
    #[cfg(target_os = "macos")]
    {
        let window_weak = window.as_weak();
        let state_for_autostart = Arc::clone(&state);
        std::thread::spawn(move || {
            match copypaste_ui::autostart::ensure_daemon_running() {
                Ok(copypaste_ui::autostart::DaemonStatus::AlreadyRunning) => {
                    eprintln!("[autostart] daemon already running");
                }
                Ok(copypaste_ui::autostart::DaemonStatus::Started) => {
                    eprintln!("[autostart] daemon started via launchctl");
                    // Daemon just came up — refresh history so the UI drops
                    // the stale "Daemon not running" placeholder.
                    refresh_history_after_autostart(window_weak, state_for_autostart);
                }
                Ok(copypaste_ui::autostart::DaemonStatus::FailedToStart(reason)) => {
                    eprintln!("[autostart] daemon failed to start: {reason}");
                }
                Err(e) => {
                    eprintln!("[autostart] error: {e}");
                }
            }
        });
    }

    // v0.3: install the macOS menu-bar tray BEFORE Slint takes over the main
    // run loop. The tray host registers a slint::Timer that polls menu events
    // on the UI thread, so we never spin a competing native run loop.
    // Failure is non-fatal — log + continue as a window-only app.
    #[cfg(target_os = "macos")]
    {
        let window_weak = window.as_weak();
        let on_open_history: copypaste_ui::tray_host::ActionCb = Box::new(move || {
            if let Some(win) = window_weak.upgrade() {
                win.show().ok();
                // LSUIElement=true (menu-bar-only) apps stay in the
                // background after `show()` — focus stays on whatever app
                // the user was last using. NSApplication::activate brings
                // the process + its visible windows to the foreground.
                activate_app();
            }
        });
        let callbacks = copypaste_ui::tray_host::TrayCallbacks {
            on_open_history: Some(on_open_history),
            on_open_preferences: None,
            on_quit: None, // default = slint::quit_event_loop()
        };
        if let Err(e) = copypaste_ui::tray_host::install(callbacks) {
            eprintln!("[tray] install failed: {e} — running without menu-bar tray");
        }
    }

    // --- Wire: refresh-requested ---
    {
        let window_weak = window.as_weak();
        let state = Arc::clone(&state);
        window.on_refresh_requested(move || {
            let window_weak = window_weak.clone();
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let (socket_path, offset) = {
                    let s = lock_or_recover(&state);
                    (s.socket_path.clone(), s.current_offset)
                };
                let result = load_history_page(&socket_path, PAGE_SIZE, offset);
                let state_for_apply = Arc::clone(&state);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, &state_for_apply, result, offset);
                    }
                });
            });
        });
    }

    // --- Wire: item-clicked (paste) ---
    {
        let window_weak = window.as_weak();
        let state = Arc::clone(&state);
        window.on_item_clicked(move |id: slint::SharedString| {
            let window_weak = window_weak.clone();
            let socket_path = {
                let s = lock_or_recover(&state);
                s.socket_path.clone()
            };
            let id_str = id.to_string();
            std::thread::spawn(move || {
                let result = paste_item(&socket_path, &id_str);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        match result {
                            Ok(_) => {
                                // beta.5 fix: `&id_str[..8.min(id_str.len())]` slices by
                                // bytes and panics when the 8th byte falls inside a
                                // multi-byte UTF-8 codepoint. Use a char-based take so
                                // any well-formed `String` works, regardless of script.
                                let short: String = id_str.chars().take(8).collect();
                                win.set_status_text(format!("Pasted: {short}").into());
                            }
                            Err(e) => win.set_status_text(format!("Paste failed: {e}").into()),
                        }
                    }
                });
            });
        });
    }

    // --- Wire: settings-requested ---
    {
        let window_weak = window.as_weak();
        let state = Arc::clone(&state);
        window.on_settings_requested(move || {
            let window_weak = window_weak.clone();
            let socket_path = {
                let s = lock_or_recover(&state);
                s.socket_path.clone()
            };
            std::thread::spawn(move || {
                // Fetch current settings from daemon and log them.
                // Full Settings UI to follow in a separate feature branch.
                let result: Result<ipc_client::AppSettings, String> =
                    IpcClient::connect(&socket_path)
                        .map_err(|e| format!("daemon offline: {e}"))
                        .and_then(|mut c| c.get_settings().map_err(|e| e.to_string()));
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        match result {
                            Ok(settings) => win.set_status_text(
                                format!(
                                    "Settings: p2p={}, supabase={}",
                                    settings.p2p_enabled,
                                    settings.supabase_url.as_deref().unwrap_or("(none)")
                                )
                                .into(),
                            ),
                            Err(e) => win.set_status_text(format!("Settings error: {e}").into()),
                        }
                    }
                });
            });
        });
    }

    // --- Wire: search-changed (beta-bonus slint-search) ---
    //
    // Runs synchronously on the Slint thread because `slint::Image` (held
    // by `HistoryItem.thumb_source`) is `!Send` — the freshly-built row
    // Vec cannot cross thread boundaries. Filtering operates over the
    // cached page (≤ PAGE_SIZE = 50 entries) so the keystroke handler
    // stays cheap; no IPC round-trip per character.
    {
        let window_weak = window.as_weak();
        let state = Arc::clone(&state);
        window.on_search_changed(move |query: slint::SharedString| {
            let snapshot = {
                let s = lock_or_recover(&state);
                s.last_page_items.clone()
            };
            let q = query.to_string();
            let total = snapshot.len();
            let filtered = copypaste_ui::filter_history_items(&snapshot, &q);
            let slint_items: Vec<HistoryItem> =
                filtered.into_iter().map(entry_to_slint_item).collect();
            let matched = slint_items.len();
            if let Some(win) = window_weak.upgrade() {
                let model = slint::VecModel::from(slint_items);
                win.set_items(slint::ModelRc::new(model));
                let status = if q.trim().is_empty() {
                    format!("Showing {total} items")
                } else {
                    format!("Matched {matched} of {total} items")
                };
                win.set_status_text(status.into());
            }
        });
    }

    // --- Wire: load-next-page ---
    //
    // beta.5 Bug-1/2 fix: previously this handler advanced
    // `state.current_offset` *before* awaiting the IPC reply. Two failure
    // modes followed:
    //   - past-end:  no guard against `current + PAGE_SIZE >= total`,
    //                so spamming Next walked into "Showing 51-50 of 50".
    //   - on-error:  if the daemon returned Err, the offset stayed bumped
    //                and the next Refresh fetched the wrong page.
    //
    // New shape: compute the candidate offset locally, refuse the
    // request if the cached `last_known_total` says we're already on
    // the last page, and let `apply_history_result` commit the offset
    // to `AppState` only on Ok.
    {
        let window_weak = window.as_weak();
        let state = Arc::clone(&state);
        window.on_load_next_page(move || {
            let window_weak = window_weak.clone();
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let (socket_path, candidate_offset) = {
                    let s = lock_or_recover(&state);
                    // Refuse to advance past the last known page. The
                    // guard is best-effort — before the first successful
                    // load `last_known_total` is None and we let the
                    // request through so the daemon can populate it.
                    if let Some(total) = s.last_known_total {
                        if s.current_offset + PAGE_SIZE >= total {
                            return;
                        }
                    }
                    (s.socket_path.clone(), s.current_offset + PAGE_SIZE)
                };
                let result = load_history_page(&socket_path, PAGE_SIZE, candidate_offset);
                let state_for_apply = Arc::clone(&state);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, &state_for_apply, result, candidate_offset);
                    }
                });
            });
        });
    }

    // --- Wire: load-prev-page ---
    //
    // beta.5 Bug-2 fix (mirror of load-next-page): keep `current_offset`
    // stable until `apply_history_result` confirms the daemon returned a
    // page. On Err the previously-mutated offset would otherwise leave
    // the UI looking at the wrong window of history after a subsequent
    // Refresh.
    {
        let window_weak = window.as_weak();
        let state = Arc::clone(&state);
        window.on_load_prev_page(move || {
            let window_weak = window_weak.clone();
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let (socket_path, candidate_offset) = {
                    let s = lock_or_recover(&state);
                    (
                        s.socket_path.clone(),
                        s.current_offset.saturating_sub(PAGE_SIZE),
                    )
                };
                let result = load_history_page(&socket_path, PAGE_SIZE, candidate_offset);
                let state_for_apply = Arc::clone(&state);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, &state_for_apply, result, candidate_offset);
                    }
                });
            });
        });
    }

    // Initial load on startup
    {
        let window_weak = window.as_weak();
        let state_clone = Arc::clone(&state);
        std::thread::spawn(move || {
            // Small delay to let the window render first
            std::thread::sleep(Duration::from_millis(100));
            let (socket_path, offset) = {
                let s = lock_or_recover(&state_clone);
                (s.socket_path.clone(), s.current_offset)
            };
            let result = load_history_page(&socket_path, PAGE_SIZE, offset);
            let state_for_apply = Arc::clone(&state_clone);
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = window_weak.upgrade() {
                    apply_history_result(&win, &state_for_apply, result, offset);
                }
            });
        });
    }

    window.run()?;
    Ok(())
}

/// Call `history_page` on the daemon and return parsed results.
///
/// beta.5 Bug-3: stop collapsing every connect error into "daemon offline:" —
/// the funnel in `apply_history_result` previously did a substring match on
/// that prefix and mistranslated transient IO errors into "Daemon not running"
/// status text. Now we return the raw [`IpcError`] display, which already
/// formats `IpcError::DaemonOffline` as "Daemon not running..." (the funnel
/// keys off that specific phrase, see [`apply_history_result`]).
fn load_history_page(
    socket_path: &std::path::Path,
    limit: u64,
    offset: u64,
) -> std::result::Result<ipc_client::HistoryPage, String> {
    let mut client = IpcClient::connect(socket_path).map_err(|e| ipc_error_to_string(&e))?;
    client
        .history_page(limit, offset)
        .map_err(|e| e.to_string())
}

/// Map an `anyhow::Error` produced by `IpcClient::connect` to a display
/// string that preserves the underlying [`ipc_client::IpcError`] formatting
/// when present (so `DaemonOffline` keeps its actionable "Daemon not running"
/// prefix). Falls back to the raw error display for non-Ipc errors.
fn ipc_error_to_string(e: &anyhow::Error) -> String {
    if let Some(ipc_err) = e.downcast_ref::<ipc_client::IpcError>() {
        ipc_err.to_string()
    } else {
        e.to_string()
    }
}

/// Worker spawned from the autostart thread once the daemon socket comes up.
/// Posts the same `load_history_page` + `apply_history_result` sequence the
/// `on_refresh_requested` callback runs, but from a background thread that
/// uses [`slint::invoke_from_event_loop`] to hop back to the UI thread.
///
/// Why this exists: the initial `std::thread::spawn` near the bottom of
/// `main()` waits 100ms after window construction and then fires its single
/// load attempt. On a fresh install the daemon needs ~4s to come up, so that
/// attempt fails and the UI shows "Daemon not running". Without an explicit
/// post-autostart refresh, the user is stuck on the stale placeholder until
/// they click Refresh manually.
#[cfg(target_os = "macos")]
fn refresh_history_after_autostart(
    window_weak: slint::Weak<HistoryWindow>,
    state: Arc<Mutex<AppState>>,
) {
    let (socket_path, offset) = {
        let s = lock_or_recover(&state);
        (s.socket_path.clone(), s.current_offset)
    };
    let result = load_history_page(&socket_path, PAGE_SIZE, offset);
    let state_for_apply = Arc::clone(&state);
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(win) = window_weak.upgrade() {
            apply_history_result(&win, &state_for_apply, result, offset);
        }
    });
}

/// Call `paste` on the daemon.
fn paste_item(socket_path: &std::path::Path, id: &str) -> std::result::Result<String, String> {
    let mut client = IpcClient::connect(socket_path).map_err(|e| format!("daemon offline: {e}"))?;
    client.paste(id).map_err(|e| e.to_string())
}

/// Convert a daemon-side history entry into the Slint row struct.
/// Pulled out of `apply_history_result` so the search-changed handler can
/// re-render filtered rows without duplicating the row shape.
fn entry_to_slint_item(entry: &ipc_client::HistoryEntry) -> HistoryItem {
    HistoryItem {
        id: entry.id.clone().into(),
        content_type: entry.content_type.clone().into(),
        preview: entry.preview.clone().into(),
        timestamp: format_wall_time(entry.wall_time).into(),
        is_sensitive: entry.is_sensitive,
        // TODO(beta-w3.5): when daemon exposes `get_image_thumbnail(id)`
        // IPC, call it for image rows and convert (rgba, w, h) via
        // slint::Image::from_rgba8.
        thumb_source: slint::Image::default(),
        thumb_width: 0,
        thumb_height: 0,
    }
}

/// Apply a `history_page` result to the Slint window.
///
/// Wave 3.1 fix #25: when the daemon is offline (socket missing or refused),
/// surface an empty-state hint with the recovery command instead of the raw
/// IO error string. The HistoryWindow renders the "No clipboard items..."
/// placeholder whenever `items` is empty, so we just clear the list and put
/// the actionable message in the status line.
///
/// Beta-bonus (slint-search): caches the unfiltered entries in `state` so
/// the `search-changed` callback can re-filter without another IPC call.
/// A pending search query is re-applied on top of the new page so the
/// user's filter survives a refresh / pagination event.
///
/// beta.5 Bug-1/2: this is the single point where `state.current_offset`
/// and `state.last_known_total` are written. Callers compute the candidate
/// offset they want to display and pass it as `offset`; we commit it to
/// state only on the Ok branch so an IPC error never advances or rewinds
/// the user's view.
fn apply_history_result(
    win: &HistoryWindow,
    state: &Arc<Mutex<AppState>>,
    result: std::result::Result<ipc_client::HistoryPage, String>,
    offset: u64,
) {
    win.set_loading(false);
    match result {
        Err(e) => {
            // beta.5 Bug-3: only show the "Daemon not running" hint when the
            // error is genuinely `IpcError::DaemonOffline`. Other failures
            // (parse errors, transient IO, daemon-side error codes) get
            // surfaced verbatim so misdiagnosis doesn't repeat. The
            // `DaemonOffline` Display starts with "Daemon not running." (see
            // `ipc_client::IpcError::fmt`), so a prefix check is exact —
            // we no longer match on the loose "daemon offline" substring
            // that previously caught any wrapped connect error.
            let msg = if e.starts_with("Daemon not running") {
                "Daemon not running. Start with `copypaste daemon start`.".to_string()
            } else {
                format!("Error: {e}")
            };
            win.set_status_text(msg.into());
            win.set_items(Default::default());
            win.set_total_count(0);
            // Clear the cache so a stale search doesn't show pre-error rows.
            lock_or_recover(state).last_page_items.clear();
        }
        Ok(page) => {
            let count = page.items.len() as u64;
            let status = if page.total == 0 {
                "No items".to_string()
            } else {
                format!(
                    "Showing {}-{} of {} items",
                    offset + 1,
                    offset + count,
                    page.total
                )
            };
            win.set_status_text(status.into());
            win.set_total_count(page.total as i32);

            // Cache the entries for the debounced search filter, commit
            // the new page offset to state (Bug-2: callers do not mutate
            // `current_offset` until we confirm Ok), and record
            // `page.total` so the next `on_load_next_page` can guard
            // against advancing past the end (Bug-1).
            let query = win.get_search_query().to_string();
            {
                let mut s = lock_or_recover(state);
                s.last_page_items = page.items.clone();
                s.current_offset = offset;
                s.last_known_total = Some(page.total);
            }

            // Wave 3.4: image rows ship without inline pixels for now.
            // `thumb_width == 0` tells the Slint row to render the "IMG"
            // placeholder instead of an empty `Image`. A follow-up IPC
            // method `get_image_thumbnail(id)` will populate `thumb_source`
            // lazily — see TODO in `entry_to_slint_item`.
            let slint_items: Vec<HistoryItem> =
                copypaste_ui::filter_history_items(&page.items, &query)
                    .into_iter()
                    .map(entry_to_slint_item)
                    .collect();

            let model = slint::VecModel::from(slint_items);
            win.set_items(slint::ModelRc::new(model));
        }
    }
}
