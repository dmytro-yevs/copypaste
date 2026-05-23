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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use anyhow::Result;
use ipc_client::{IpcClient, format_wall_time};

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
    // Beta-bonus i18n: bind the gettext domain (auto-set from CARGO_PKG_NAME =
    // "copypaste-ui" by slint-build) to the `lang/` catalog directory shipped
    // with the crate. At runtime Slint resolves `@tr("…")` against
    // `lang/<locale>/LC_MESSAGES/copypaste-ui.mo`; missing locales fall back
    // to the literal msgid. Locale is selected by LC_ALL / LANG / LC_MESSAGES.
    slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang"));

    let window = HistoryWindow::new()?;
    let state = Arc::new(Mutex::new(AppState::new()));

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
                                format!("Pasted: {}", &id_str[..8.min(id_str.len())]).into()
                            ),
                            Err(e) => win.set_status_text(
                                format!("Paste failed: {e}").into()
                            ),
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
                            Err(e) => win.set_status_text(
                                format!("Settings error: {e}").into()
                            ),
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
            let slint_items: Vec<HistoryItem> = filtered
                .into_iter()
                .map(entry_to_slint_item)
                .collect();
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
    let mut client = IpcClient::connect(socket_path)
        .map_err(|e| format!("daemon offline: {e}"))?;
    client
        .history_page(limit, offset)
        .map_err(|e| e.to_string())
}

/// Call `paste` on the daemon.
fn paste_item(socket_path: &std::path::Path, id: &str) -> std::result::Result<String, String> {
    let mut client = IpcClient::connect(socket_path)
        .map_err(|e| format!("daemon offline: {e}"))?;
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

            let model = slint::VecModel::from(slint_items);
            win.set_items(slint::ModelRc::new(model));
        }
    }
}
