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
use std::sync::{Arc, Mutex};
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

fn main() -> Result<()> {
    // Beta hot-fix: on macOS, install the Launch Agent plist + bootstrap the
    // daemon in the background so the user does not have to run
    // `copypaste daemon install && copypaste daemon start` after a fresh DMG
    // install. Runs in a dedicated thread — UI rendering must NOT block on
    // launchctl. See `crates/copypaste-ui/src/autostart.rs` for the flow.
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| match copypaste_ui::autostart::ensure_daemon_running() {
        Ok(copypaste_ui::autostart::DaemonStatus::AlreadyRunning) => {
            eprintln!("[autostart] daemon already running");
        }
        Ok(copypaste_ui::autostart::DaemonStatus::Started) => {
            eprintln!("[autostart] daemon started via launchctl");
        }
        Ok(copypaste_ui::autostart::DaemonStatus::FailedToStart(reason)) => {
            eprintln!("[autostart] daemon failed to start: {reason}");
        }
        Err(e) => {
            eprintln!("[autostart] error: {e}");
        }
    });

    // Beta-bonus i18n: bind the gettext domain (auto-set from CARGO_PKG_NAME =
    // "copypaste-ui" by slint-build) to the `lang/` catalog directory shipped
    // with the crate. At runtime Slint resolves `@tr("…")` against
    // `lang/<locale>/LC_MESSAGES/copypaste-ui.mo`; missing locales fall back
    // to the literal msgid. Locale is selected by LC_ALL / LANG / LC_MESSAGES.
    slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang"));

    let window = HistoryWindow::new()?;
    let state = Arc::new(Mutex::new(AppState::new()));

    // v0.3 T3: load the redaction preference from disk and push it into the
    // window before the first render so sensitive rows are masked on the
    // initial paint (no flash of un-redacted previews).
    {
        let prefs = copypaste_ui::sensitive_helpers::load();
        window.set_hide_sensitive(prefs.hide_sensitive);
    }

    // v0.3 T3: persist the toggle whenever the user flips the in-window
    // CheckBox. Save runs on the UI thread — `serde_json::to_vec_pretty`
    // + a small write is microseconds, no need for a background thread.
    {
        window.on_hide_sensitive_changed(move |value: bool| {
            let prefs = copypaste_ui::sensitive_helpers::UiPrefs {
                hide_sensitive: value,
            };
            copypaste_ui::sensitive_helpers::save(&prefs);
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
            }
        });
        // v0.3 T3: tray "Recent items" row click → paste via IPC. The
        // closure spawns a worker thread so we never block the UI on the
        // daemon socket (paste involves a write + ack).
        let paste_socket = {
            let s = state.lock().unwrap();
            s.socket_path.clone()
        };
        let on_paste_item: copypaste_ui::tray_host::PasteCb = Box::new(move |id: &str| {
            let socket = paste_socket.clone();
            let id_owned = id.to_string();
            std::thread::spawn(move || {
                if let Err(e) = paste_item(&socket, &id_owned) {
                    tracing::warn!(error = %e, id = %id_owned, "tray paste failed");
                }
            });
        });
        let callbacks = copypaste_ui::tray_host::TrayCallbacks {
            on_open_history: Some(on_open_history),
            on_open_preferences: None,
            on_quit: None, // default = slint::quit_event_loop()
            on_paste_item: Some(on_paste_item),
        };
        if let Err(e) = copypaste_ui::tray_host::install(callbacks) {
            eprintln!("[tray] install failed: {e} — running without menu-bar tray");
        } else {
            // v0.3 T3: prime the tray with current history immediately and
            // then refresh on a slint::Timer so changes show up without
            // requiring the user to open the history window first.
            spawn_tray_recents_refresh(Arc::clone(&state));
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
                    let s = state.lock().unwrap();
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
                let s = state.lock().unwrap();
                s.socket_path.clone()
            };
            let id_str = id.to_string();
            std::thread::spawn(move || {
                let result = paste_item(&socket_path, &id_str);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        match result {
                            Ok(_) => win.set_status_text(
                                format!("Pasted: {}", &id_str[..8.min(id_str.len())]).into(),
                            ),
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
                let s = state.lock().unwrap();
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
                let s = state.lock().unwrap();
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
                // v0.3 T3: feed the visible-count badge so the search
                // affordance shows "N / total" without waiting for the
                // status line to update.
                win.set_visible_count(matched as i32);
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
    {
        let window_weak = window.as_weak();
        let state = Arc::clone(&state);
        window.on_load_next_page(move || {
            let window_weak = window_weak.clone();
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let (socket_path, new_offset) = {
                    let mut s = state.lock().unwrap();
                    let socket = s.socket_path.clone();
                    let offset = s.current_offset + PAGE_SIZE;
                    s.current_offset = offset;
                    (socket, offset)
                };
                let result = load_history_page(&socket_path, PAGE_SIZE, new_offset);
                let state_for_apply = Arc::clone(&state);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, &state_for_apply, result, new_offset);
                    }
                });
            });
        });
    }

    // --- Wire: load-prev-page ---
    {
        let window_weak = window.as_weak();
        let state = Arc::clone(&state);
        window.on_load_prev_page(move || {
            let window_weak = window_weak.clone();
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let (socket_path, new_offset) = {
                    let mut s = state.lock().unwrap();
                    let offset = s.current_offset.saturating_sub(PAGE_SIZE);
                    s.current_offset = offset;
                    (s.socket_path.clone(), offset)
                };
                let result = load_history_page(&socket_path, PAGE_SIZE, new_offset);
                let state_for_apply = Arc::clone(&state);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, &state_for_apply, result, new_offset);
                    }
                });
            });
        });
    }

    // --- v0.3 in-app updater (Homebrew Cask, ADR-012) ---------------------
    //
    // Periodically asks `brew outdated --cask copypaste` if a newer version
    // is published. For v0.3 we log the outcome; a follow-up wire-up will
    // surface a banner / tray-menu badge through a Slint `updates-available`
    // property. The check is macOS-only because the daemon is macOS-only;
    // gating with `cfg!(target_os = "macos")` keeps cross-compile / CI on
    // other hosts free of spurious `brew` calls.
    #[cfg(target_os = "macos")]
    std::thread::spawn(|| {
        use copypaste_ui::updater::{self, SystemRunner, UpdateStatus};
        loop {
            match updater::check_for_update(&SystemRunner) {
                UpdateStatus::UpdateAvailable(info) => {
                    // TODO(v0.3-followup): hook into Slint `updates-available`
                    // property + tray-menu "Update to vX" item.
                    eprintln!(
                        "[updater] update available: {} → {}",
                        info.current_version, info.latest_version
                    );
                }
                UpdateStatus::UpToDate => {
                    eprintln!("[updater] up to date");
                }
                UpdateStatus::BrewNotInstalled => {
                    eprintln!("[updater] brew not installed; in-app auto-update unavailable");
                    // No point in retrying every 24h if brew is absent.
                    break;
                }
                UpdateStatus::CheckFailed(e) => {
                    eprintln!("[updater] check failed: {e}");
                }
            }
            std::thread::sleep(updater::CHECK_INTERVAL);
        }
    });

    // Initial load on startup
    {
        let window_weak = window.as_weak();
        let state_clone = Arc::clone(&state);
        std::thread::spawn(move || {
            // Small delay to let the window render first
            std::thread::sleep(Duration::from_millis(100));
            let (socket_path, offset) = {
                let s = state_clone.lock().unwrap();
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
fn load_history_page(
    socket_path: &std::path::Path,
    limit: u64,
    offset: u64,
) -> std::result::Result<ipc_client::HistoryPage, String> {
    let mut client = IpcClient::connect(socket_path).map_err(|e| format!("daemon offline: {e}"))?;
    client
        .history_page(limit, offset)
        .map_err(|e| e.to_string())
}

/// Call `paste` on the daemon.
fn paste_item(socket_path: &std::path::Path, id: &str) -> std::result::Result<String, String> {
    let mut client = IpcClient::connect(socket_path).map_err(|e| format!("daemon offline: {e}"))?;
    client.paste(id).map_err(|e| e.to_string())
}

/// v0.3 T3: keep the tray's "Recent items" block in sync with the daemon.
///
/// Polls `history_page(MAX_TRAY_RECENTS, 0)` every `TRAY_REFRESH_INTERVAL`
/// from a Slint timer on the UI thread, then hands the result to
/// `tray_host::update_recents` (which mutates muda menu state — must run
/// main-thread on macOS).
///
/// The IPC call itself runs synchronously inside the timer tick because
/// (a) it's bounded at MAX_TRAY_RECENTS rows so latency is in the low
/// milliseconds, and (b) `slint::Image` types on the row Vec aren't
/// involved here — the tray only needs id/preview/wall_time/type, all
/// `Send`. If profiling shows hitches we can move the read off-thread
/// and post-back via `invoke_from_event_loop`.
#[cfg(target_os = "macos")]
fn spawn_tray_recents_refresh(state: Arc<Mutex<AppState>>) {
    use copypaste_ui::tray_host::{update_recents, RecentTrayItem, MAX_TRAY_RECENTS};

    // Refresh cadence — 5s is fast enough to feel live without burning
    // socket bandwidth. The IPC server is a single-threaded unix socket
    // so we keep concurrent reads modest.
    const TRAY_REFRESH_INTERVAL: Duration = Duration::from_secs(5);

    let refresh = move || {
        let socket = {
            let Ok(s) = state.lock() else { return };
            s.socket_path.clone()
        };
        // Off-thread fetch so the tick stays cheap; post results back via
        // invoke_from_event_loop because `update_recents` is main-thread-
        // only on macOS (muda::Menu).
        std::thread::spawn(move || {
            let result = load_history_page(&socket, MAX_TRAY_RECENTS as u64, 0);
            let recents: Vec<RecentTrayItem> = match result {
                Ok(page) => page
                    .items
                    .into_iter()
                    .map(|e| RecentTrayItem {
                        id: e.id,
                        content_type: e.content_type,
                        preview: e.preview,
                        wall_time_ms: e.wall_time,
                    })
                    .collect(),
                Err(e) => {
                    tracing::debug!(error = %e, "tray refresh: history_page failed");
                    return;
                }
            };
            let _ = slint::invoke_from_event_loop(move || {
                update_recents(recents);
            });
        });
    };

    // Prime once immediately so the first menu open shows real history.
    refresh();

    // Repeating timer on the Slint event loop.
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, TRAY_REFRESH_INTERVAL, move || {
        refresh();
    });
    // Leak so the timer outlives this scope.
    std::mem::forget(timer);
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
fn apply_history_result(
    win: &HistoryWindow,
    state: &Arc<Mutex<AppState>>,
    result: std::result::Result<ipc_client::HistoryPage, String>,
    offset: u64,
) {
    win.set_loading(false);
    match result {
        Err(e) => {
            let msg = if e.contains("Daemon not running") || e.contains("daemon offline") {
                "Daemon not running. Start with `copypaste daemon start`.".to_string()
            } else {
                format!("Error: {e}")
            };
            win.set_status_text(msg.into());
            win.set_items(Default::default());
            win.set_total_count(0);
            // Clear the cache so a stale search doesn't show pre-error rows.
            if let Ok(mut s) = state.lock() {
                s.last_page_items.clear();
            }
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

            // Cache the entries for the debounced search filter.
            let query = win.get_search_query().to_string();
            if let Ok(mut s) = state.lock() {
                s.last_page_items = page.items.clone();
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

            // v0.3 T3: feed the visible-count badge.
            win.set_visible_count(slint_items.len() as i32);
            let model = slint::VecModel::from(slint_items);
            win.set_items(slint::ModelRc::new(model));
        }
    }
}
