/// copypaste-ui — Slint HistoryWindow wired to copypaste-daemon via Unix IPC.
///
/// Architecture:
///   - Slint renders the HistoryWindow on the main thread.
///   - A dedicated background thread polls the daemon IPC socket.
///   - Results are sent back to the Slint event loop via `slint::invoke_from_event_loop`.
///   - IPC methods: `history_page` (list), `paste` (activate by id), `status` (health).
///
/// Data flow:
///   Slint callback → Rust callback closure → IPC call → slint::invoke_from_event_loop → Slint update

mod ipc_client;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use anyhow::Result;
use ipc_client::{IpcClient, format_wall_time};

// Include generated Slint bindings.
slint::include_modules!();

const PAGE_SIZE: u64 = 50;

/// Application state shared between callbacks.
struct AppState {
    socket_path: PathBuf,
    current_offset: u64,
}

impl AppState {
    fn new() -> Self {
        let socket_path = home::home_dir()
            .expect("HOME must exist")
            .join("Library/Application Support/CopyPaste/daemon.sock");
        Self { socket_path, current_offset: 0 }
    }
}

fn main() -> Result<()> {
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
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, result, offset);
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
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, result, new_offset);
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
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, result, new_offset);
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
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(win) = window_weak.upgrade() {
                    apply_history_result(&win, result, offset);
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

/// Apply a `history_page` result to the Slint window.
fn apply_history_result(
    win: &HistoryWindow,
    result: std::result::Result<ipc_client::HistoryPage, String>,
    offset: u64,
) {
    win.set_loading(false);
    match result {
        Err(e) => {
            win.set_status_text(format!("Error: {e}").into());
            win.set_items(Default::default());
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

            let slint_items: Vec<HistoryItem> = page
                .items
                .into_iter()
                .map(|entry| HistoryItem {
                    id: entry.id.into(),
                    content_type: entry.content_type.into(),
                    preview: entry.preview.into(),
                    timestamp: format_wall_time(entry.wall_time).into(),
                    is_sensitive: entry.is_sensitive,
                })
                .collect();

            let model = slint::VecModel::from(slint_items);
            win.set_items(slint::ModelRc::new(model));
        }
    }
}
