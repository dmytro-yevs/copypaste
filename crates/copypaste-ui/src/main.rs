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
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Per-command in-flight flags.
///
/// beta.5 Bug-3 (unbounded thread spawn): every Refresh / Next / Prev
/// click previously did `std::thread::spawn`. Spamming Next or holding
/// ⌘R produced one OS thread per event plus an interleaving race in
/// which a stale IPC response clobbered a newer one via
/// `invoke_from_event_loop`. We coalesce duplicate clicks by reserving
/// one in-flight slot per command — a click that arrives while the slot
/// is taken is dropped on the floor.
///
/// Search is intentionally NOT covered: it filters the cached page
/// synchronously on the UI thread (no IPC, no spawn) and de-duping
/// keystrokes here would just add latency.
#[derive(Clone)]
struct InFlight {
    refresh: Arc<AtomicBool>,
    next_page: Arc<AtomicBool>,
    prev_page: Arc<AtomicBool>,
}

impl InFlight {
    fn new() -> Self {
        Self {
            refresh: Arc::new(AtomicBool::new(false)),
            next_page: Arc::new(AtomicBool::new(false)),
            prev_page: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// RAII guard that clears an `AtomicBool` on drop.
///
/// Used to ensure the in-flight flag is released even if the spawned
/// worker panics partway through. Without this, a single panic in
/// `load_history_page` would leave the flag stuck at `true` forever
/// and the corresponding button would silently stop responding for
/// the rest of the session.
///
/// Holds an `Arc<AtomicBool>` (not a borrow) so the guard can be
/// moved into the spawned worker thread.
struct InFlightGuard {
    flag: Arc<AtomicBool>,
}

impl InFlightGuard {
    /// Try to take ownership of the in-flight slot.
    /// Returns `None` if the slot is already taken (caller should drop
    /// the click on the floor).
    fn acquire(flag: Arc<AtomicBool>) -> Option<Self> {
        flag.compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| Self { flag })
    }
}

impl Drop for InFlightGuard {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
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
/// Starting as `.accessory` (tray-only, set via `set_activation_policy_accessory()`
/// at launch instead of the former `LSUIElement=true` plist key) means a
/// freshly-shown window stays *behind* whatever the user was using. `NSApplication::activate` is the modern (Sonoma+) replacement for
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

/// Switch to `.regular` activation policy so the app appears in cmd-tab and
/// the Dock while the History window is visible.
///
/// Called every time the History window is shown via the tray "Open History"
/// action. The policy change is idempotent — calling it when already `.regular`
/// is a no-op. Returns whether the change succeeded (failures are logged but
/// not fatal; worst case the user cannot cmd-tab to the window).
///
/// Note: `setActivationPolicy:` must be called on the main thread. Tray
/// callbacks run on the Slint event loop, which IS the main thread on macOS,
/// so `MainThreadMarker::new()` succeeds here in the same way it does in
/// `activate_app`.
#[cfg(target_os = "macos")]
fn set_activation_policy_regular() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        let ok = app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        if !ok {
            tracing::warn!("setActivationPolicy(.regular) returned false — cmd-tab may not work");
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn set_activation_policy_regular() {}

/// Switch back to `.accessory` (tray-only) policy once no window is visible.
///
/// Called when the History window is closed. After macOS 10.9 the policy may
/// be toggled in either direction; the function logs a warning if the system
/// refuses the change (which should not happen on supported OS versions).
///
/// Apple note: changing *to* `.accessory` hides the Dock icon and removes the
/// app from cmd-tab immediately. The tray icon is unaffected — it continues to
/// work because it is driven by the Slint event loop, not the Dock.
#[cfg(target_os = "macos")]
fn set_activation_policy_accessory() {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
    if let Some(mtm) = MainThreadMarker::new() {
        let app = NSApplication::sharedApplication(mtm);
        let ok = app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);
        if !ok {
            tracing::warn!(
                "setActivationPolicy(.accessory) returned false — app may stay in cmd-tab"
            );
        }
    }
}

#[cfg(not(target_os = "macos"))]
#[allow(dead_code)]
fn set_activation_policy_accessory() {}

fn main() -> Result<()> {
    // Beta-bonus i18n: bind the gettext domain (auto-set from CARGO_PKG_NAME =
    // "copypaste-ui" by slint-build) to the `lang/` catalog directory shipped
    // with the crate. At runtime Slint resolves `@tr("…")` against
    // `lang/<locale>/LC_MESSAGES/copypaste-ui.mo`; missing locales fall back
    // to the literal msgid. Locale is selected by LC_ALL / LANG / LC_MESSAGES.
    slint::init_translations!(concat!(env!("CARGO_MANIFEST_DIR"), "/lang"));

    let window = HistoryWindow::new()?;
    let state = Arc::new(Mutex::new(AppState::new()));
    // Per-command in-flight flags — see `InFlight` doc-comment.
    let in_flight = InFlight::new();

    // ── Settings window ────────────────────────────────────────────────────────
    // Constructed eagerly so it is ready when the tray "Preferences" item fires.
    // The IPC adapter bridges `ipc_client::IpcClient` (binary-private) to the
    // library's `SettingsIpc` trait without leaking IPC types into the lib crate.
    let socket_path_for_settings = lock_or_recover(&state).socket_path.clone();
    // Try to fetch the current settings for the initial window population; fall
    // back to defaults if the daemon is offline at startup.
    let (initial_settings, initial_fp) = {
        use copypaste_ui::settings::{AppSettings as UiSettings, HistoryLimit};
        match IpcClient::connect(&socket_path_for_settings)
            .map_err(|e| e.to_string())
            .and_then(|mut c| {
                let s = c.get_settings().map_err(|e| e.to_string())?;
                let fp = c.get_own_fingerprint().map_err(|e| e.to_string())?;
                Ok((s, fp))
            }) {
            Ok((s, fp)) => {
                let ui = UiSettings {
                    launch_at_login: false,
                    private_mode: false,
                    history_limit: HistoryLimit::Hundred,
                    supabase_url: s.supabase_url.unwrap_or_default(),
                    supabase_key: s.supabase_anon_key.unwrap_or_default(),
                    device_name: String::from("My Mac"),
                };
                (ui, fp)
            }
            Err(e) => {
                tracing::warn!(error = %e, "settings pre-load failed — using defaults");
                (UiSettings::default(), String::new())
            }
        }
    };
    let settings_window = copypaste_ui::windows::SettingsWindowHandle::new(
        &initial_settings,
        env!("CARGO_PKG_VERSION"),
        &initial_fp,
    )?;
    // Wire Save / Clear History via the SettingsIpc adapter.
    {
        use copypaste_ui::settings::{AppSettings as UiSettings, HistoryLimit};
        use copypaste_ui::windows::SettingsIpc;
        use std::ops::Not;
        struct IpcAdapter(std::path::PathBuf);
        impl SettingsIpc for IpcAdapter {
            fn get_settings(&mut self) -> Result<UiSettings, String> {
                let mut c = IpcClient::connect(&self.0).map_err(|e| e.to_string())?;
                let s = c.get_settings().map_err(|e| e.to_string())?;
                Ok(UiSettings {
                    launch_at_login: false,
                    private_mode: false,
                    history_limit: HistoryLimit::Hundred,
                    supabase_url: s.supabase_url.unwrap_or_default(),
                    supabase_key: s.supabase_anon_key.unwrap_or_default(),
                    device_name: String::from("My Mac"),
                })
            }
            fn save_settings(&mut self, settings: &UiSettings) -> Result<(), String> {
                let mut c = IpcClient::connect(&self.0).map_err(|e| e.to_string())?;
                let ipc_settings = ipc_client::AppSettings {
                    p2p_enabled: settings.supabase_url.is_empty().not(),
                    supabase_url: if settings.supabase_url.is_empty() {
                        None
                    } else {
                        Some(settings.supabase_url.clone())
                    },
                    supabase_anon_key: if settings.supabase_key.is_empty() {
                        None
                    } else {
                        Some(settings.supabase_key.clone())
                    },
                };
                c.save_settings(&ipc_settings).map_err(|e| e.to_string())
            }
            fn delete_all_history(&mut self) -> Result<(), String> {
                // TODO(v0.3.x): wire to a dedicated IPC method when daemon exposes one.
                tracing::warn!("delete_all_history: no IPC method yet — no-op");
                Ok(())
            }
        }
        let adapter = std::rc::Rc::new(std::cell::RefCell::new(IpcAdapter(
            lock_or_recover(&state).socket_path.clone(),
        )));
        if let Err(e) = settings_window.wire_to_ipc(adapter) {
            tracing::warn!(error = %e, "settings initial load failed");
        }
    }
    // Close button hides the window (does not quit).
    {
        let sw = settings_window.as_weak();
        settings_window.on_close(move || {
            if let Some(w) = sw.upgrade() {
                w.hide().ok();
            }
        });
    }

    // ── Pair window ────────────────────────────────────────────────────────────
    let socket_path_for_pair = lock_or_recover(&state).socket_path.clone();
    let (own_fp, paired_devices) = {
        match IpcClient::connect(&socket_path_for_pair)
            .map_err(|e| e.to_string())
            .and_then(|mut c| {
                let fp = c.get_own_fingerprint().map_err(|e| e.to_string())?;
                let peers = c.list_peers().map_err(|e| e.to_string())?;
                let pd: Vec<copypaste_ui::settings::PairedDevice> = peers
                    .into_iter()
                    .map(|p| copypaste_ui::settings::PairedDevice::new(p.name, p.fingerprint))
                    .collect();
                Ok((fp, pd))
            }) {
            Ok((fp, pd)) => (fp, pd),
            Err(e) => {
                tracing::warn!(error = %e, "pair window pre-load failed — using defaults");
                (String::new(), vec![])
            }
        }
    };
    let pair_window = copypaste_ui::windows::PairWindowHandle::new(&own_fp, &paired_devices)?;
    // Wire Pair / Remove / Revoke / Close callbacks.
    {
        let socket = socket_path_for_pair.clone();
        let pw = pair_window.as_weak();
        pair_window.on_pair(move |fp| {
            let socket = socket.clone();
            let pw = pw.clone();
            std::thread::spawn(move || {
                let result = IpcClient::connect(&socket)
                    .map_err(|e| e.to_string())
                    .and_then(|mut c| c.pair_peer(&fp, "").map_err(|e| e.to_string()));
                let (msg, is_err) = match result {
                    Ok(()) => ("Paired successfully.".to_string(), false),
                    Err(e) => (format!("Pair failed: {e}"), true),
                };
                slint::invoke_from_event_loop(move || {
                    if let Some(w) = pw.upgrade() {
                        w.set_status_message(msg.into());
                        w.set_status_is_error(is_err);
                    }
                })
                .ok();
            });
        });
    }
    {
        let socket = socket_path_for_pair.clone();
        let pw = pair_window.as_weak();
        pair_window.on_remove_peer(move |fp| {
            let socket = socket.clone();
            let pw = pw.clone();
            std::thread::spawn(move || {
                let result = IpcClient::connect(&socket)
                    .map_err(|e| e.to_string())
                    .and_then(|mut c| c.unpair_peer(&fp).map_err(|e| e.to_string()));
                let (msg, is_err) = match result {
                    Ok(()) => ("Device removed.".to_string(), false),
                    Err(e) => (format!("Remove failed: {e}"), true),
                };
                slint::invoke_from_event_loop(move || {
                    if let Some(w) = pw.upgrade() {
                        w.set_status_message(msg.into());
                        w.set_status_is_error(is_err);
                    }
                })
                .ok();
            });
        });
    }
    {
        let socket = socket_path_for_pair.clone();
        let pw = pair_window.as_weak();
        pair_window.on_revoke_peer(move |fp| {
            let socket = socket.clone();
            let pw = pw.clone();
            std::thread::spawn(move || {
                let result = IpcClient::connect(&socket)
                    .map_err(|e| e.to_string())
                    .and_then(|mut c| c.revoke_peer(&fp).map(|_| ()).map_err(|e| e.to_string()));
                let (msg, is_err) = match result {
                    Ok(()) => ("Device revoked.".to_string(), false),
                    Err(e) => (format!("Revoke failed: {e}"), true),
                };
                slint::invoke_from_event_loop(move || {
                    if let Some(w) = pw.upgrade() {
                        w.set_status_message(msg.into());
                        w.set_status_is_error(is_err);
                    }
                })
                .ok();
            });
        });
    }
    {
        let pw = pair_window.as_weak();
        pair_window.on_close(move || {
            if let Some(w) = pw.upgrade() {
                w.hide().ok();
            }
        });
    }

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
                    // beta.5 Bug-7: surface the failure to the user instead
                    // of leaving them with the generic "Daemon not running"
                    // placeholder. We piggy-back on the existing
                    // `apply_history_result` Err branch (which already
                    // formats `Error: {e}` into the status line and clears
                    // the items list) by posting a synthetic error result
                    // to the UI thread.
                    report_autostart_failure(window_weak, state_for_autostart, reason);
                }
                Err(e) => {
                    eprintln!("[autostart] error: {e}");
                }
            }
        });
    }

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
    //
    // Bug-1 fix: LSUIElement=true has been removed from Info.plist so that
    // cmd-tab works when a window is visible. We start hidden by calling
    // set_activation_policy_accessory() here — same tray-only behaviour as
    // LSUIElement, but allows runtime flip to .regular when opening a window.
    #[cfg(target_os = "macos")]
    set_activation_policy_accessory();

    #[cfg(target_os = "macos")]
    {
        let window_weak = window.as_weak();
        let on_open_history: copypaste_ui::tray_host::ActionCb = Box::new(move || {
            if let Some(win) = window_weak.upgrade() {
                // Switch to .regular so the app appears in cmd-tab / Dock
                // while the window is visible.
                set_activation_policy_regular();
                win.show().ok();
                // .accessory-policy apps stay in the background after
                // `show()` — focus stays on whatever the user had last.
                // NSApplication::activate brings the process + its visible
                // windows to the foreground.
                activate_app();
            }
        });

        // Tray "Preferences" → show the Settings window.
        let sw_weak = settings_window.as_weak();
        let on_open_preferences: copypaste_ui::tray_host::ActionCb = Box::new(move || {
            if let Some(w) = sw_weak.upgrade() {
                set_activation_policy_regular();
                w.show().ok();
                activate_app();
            }
        });

        // Bug-6: tray "Pair Device…" → show the PairWindow.
        let pw_weak = pair_window.as_weak();
        let on_open_pair: copypaste_ui::tray_host::ActionCb = Box::new(move || {
            if let Some(w) = pw_weak.upgrade() {
                set_activation_policy_regular();
                w.show().ok();
                activate_app();
            }
        });

        // v0.3 T3: tray "Recent items" row click → paste via IPC. The
        // closure spawns a worker thread so we never block the UI on the
        // daemon socket (paste involves a write + ack).
        let paste_socket = lock_or_recover(&state).socket_path.clone();
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
            on_open_preferences: Some(on_open_preferences),
            on_open_pair: Some(on_open_pair),
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
        let in_flight_refresh = Arc::clone(&in_flight.refresh);
        window.on_refresh_requested(move || {
            // beta.5 Bug-3: drop the click if a refresh is already
            // running. Without this, holding ⌘R fired one thread per
            // event and the older response could clobber the newer one
            // via `invoke_from_event_loop`.
            let Some(guard) = InFlightGuard::acquire(Arc::clone(&in_flight_refresh)) else {
                return;
            };
            let window_weak = window_weak.clone();
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let _guard = guard; // released on thread exit (incl. panic)
                let (socket_path, offset) = {
                    let s = lock_or_recover(&state);
                    (s.socket_path.clone(), s.current_offset)
                };
                let result = load_history_page(&socket_path, PAGE_SIZE, offset);
                let state_for_apply = Arc::clone(&state);
                if let Err(e) = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, &state_for_apply, result, offset);
                    }
                }) {
                    tracing::debug!(error = %e, "ui update dropped during event-loop shutdown");
                }
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
                if let Err(e) = slint::invoke_from_event_loop(move || {
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
                }) {
                    tracing::debug!(error = %e, "ui update dropped during event-loop shutdown");
                }
            });
        });
    }

    // --- Wire: settings-requested ---
    // Show the SettingsWindow when the user clicks the gear/settings button
    // in the HistoryWindow toolbar. The window was constructed and wired
    // above; here we just upgrade the weak handle and call show().
    {
        let sw_weak = settings_window.as_weak();
        window.on_settings_requested(move || {
            if let Some(w) = sw_weak.upgrade() {
                #[cfg(target_os = "macos")]
                set_activation_policy_regular();
                w.show().ok();
                activate_app();
            }
        });
    }

    // --- Wire: pair-requested (Bug-6) ---
    // Show the PairWindow when the user clicks the "Pair…" button in the
    // HistoryWindow toolbar. Mirrors on_settings_requested exactly.
    {
        let pw_weak = pair_window.as_weak();
        window.on_pair_requested(move || {
            if let Some(w) = pw_weak.upgrade() {
                #[cfg(target_os = "macos")]
                set_activation_policy_regular();
                w.show().ok();
                activate_app();
            }
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
        let in_flight_next = Arc::clone(&in_flight.next_page);
        window.on_load_next_page(move || {
            // beta.5 Bug-3: coalesce duplicate Next clicks — see InFlight doc.
            let Some(guard) = InFlightGuard::acquire(Arc::clone(&in_flight_next)) else {
                return;
            };
            let window_weak = window_weak.clone();
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let _guard = guard;
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
                if let Err(e) = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, &state_for_apply, result, candidate_offset);
                    }
                }) {
                    tracing::debug!(error = %e, "ui update dropped during event-loop shutdown");
                }
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
        let in_flight_prev = Arc::clone(&in_flight.prev_page);
        window.on_load_prev_page(move || {
            // beta.5 Bug-3: coalesce duplicate Prev clicks — see InFlight doc.
            let Some(guard) = InFlightGuard::acquire(Arc::clone(&in_flight_prev)) else {
                return;
            };
            let window_weak = window_weak.clone();
            let state = Arc::clone(&state);
            std::thread::spawn(move || {
                let _guard = guard;
                let (socket_path, candidate_offset) = {
                    let s = lock_or_recover(&state);
                    (
                        s.socket_path.clone(),
                        s.current_offset.saturating_sub(PAGE_SIZE),
                    )
                };
                let result = load_history_page(&socket_path, PAGE_SIZE, candidate_offset);
                let state_for_apply = Arc::clone(&state);
                if let Err(e) = slint::invoke_from_event_loop(move || {
                    if let Some(win) = window_weak.upgrade() {
                        apply_history_result(&win, &state_for_apply, result, candidate_offset);
                    }
                }) {
                    tracing::debug!(error = %e, "ui update dropped during event-loop shutdown");
                }
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
                let s = lock_or_recover(&state_clone);
                (s.socket_path.clone(), s.current_offset)
            };
            let result = load_history_page(&socket_path, PAGE_SIZE, offset);
            let state_for_apply = Arc::clone(&state_clone);
            if let Err(e) = slint::invoke_from_event_loop(move || {
                if let Some(win) = window_weak.upgrade() {
                    apply_history_result(&win, &state_for_apply, result, offset);
                }
            }) {
                tracing::debug!(error = %e, "ui update dropped during event-loop shutdown");
            }
        });
    }

    // --- Wire: window close → revert to .accessory activation policy ---
    //
    // When the user closes the History window the app should disappear from
    // cmd-tab and the Dock again (back to tray-only / .accessory behaviour).
    // `window().on_close_requested()` fires on the Slint event loop (main
    // thread on macOS) which is where `setActivationPolicy:` must be called.
    //
    // We return `CloseRequestResponse::HideWindow` (the default) to preserve
    // Slint's hide-on-close behaviour — the process stays alive and the tray
    // continues to work.
    #[cfg(target_os = "macos")]
    {
        window.window().on_close_requested(|| {
            set_activation_policy_accessory();
            slint::CloseRequestResponse::HideWindow
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
/// Surface a `DaemonStatus::FailedToStart(reason)` to the UI by posting
/// a synthetic IPC error through [`apply_history_result`].
///
/// beta.5 Bug-7: the autostart worker previously only `eprintln!`-ed the
/// reason, so the user saw the generic
/// `Daemon not running. Start with \`copypaste daemon start\``
/// placeholder with no hint about *why* launchctl could not bring the
/// daemon up (missing plist, sandbox denial, dangling ProgramArguments
/// path, etc.). Out of scope: adding a dedicated Slint property for
/// the failure reason — we route the message through the existing
/// `set_status_text` path so this fix stays main.rs-only.
///
/// We synthesise a `Result::Err` whose Display does NOT start with
/// "Daemon not running" so the funnel in `apply_history_result` falls
/// through to the verbatim `Error: {e}` branch. That branch also
/// clears the items list and the cached `last_page_items` for us.
#[cfg(target_os = "macos")]
fn report_autostart_failure(
    window_weak: slint::Weak<HistoryWindow>,
    state: Arc<Mutex<AppState>>,
    reason: String,
) {
    let offset = lock_or_recover(&state).current_offset;
    let msg = format!("Daemon autostart failed: {reason}");
    let result: std::result::Result<ipc_client::HistoryPage, String> = Err(msg);
    let payload: Arc<Mutex<Option<std::result::Result<ipc_client::HistoryPage, String>>>> =
        Arc::new(Mutex::new(Some(result)));

    // Same retry shape as `refresh_history_after_autostart` — the
    // failure handler can fire before `window.run()` installs the
    // event loop and would otherwise be lost to `EventLoopError`.
    for attempt in 0..3 {
        let window_weak_attempt = window_weak.clone();
        let state_for_apply = Arc::clone(&state);
        let payload_for_attempt = Arc::clone(&payload);
        let post = slint::invoke_from_event_loop(move || {
            if let Some(win) = window_weak_attempt.upgrade() {
                if let Some(result) = lock_or_recover(&payload_for_attempt).take() {
                    apply_history_result(&win, &state_for_apply, result, offset);
                }
            }
        });
        match post {
            Ok(()) => return,
            Err(e) => {
                eprintln!(
                    "[autostart] failure-report attempt {} could not post to UI: {e}",
                    attempt + 1
                );
                if attempt < 2 {
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
    }
}

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

    // beta.5 Bug-5 fix: `invoke_from_event_loop` returns
    // `EventLoopError::NoEventLoopProvider` if the Slint loop has not
    // started yet. The autostart worker is spawned *before*
    // `window.run()`, so on a fast machine the post-startup post can
    // fire before the loop is ready. Previously the `let _ = ...`
    // swallowed the error and the UI stayed stuck on the
    // "Daemon not running" placeholder.
    //
    // Retry up to 3 times with a 500 ms backoff. We share `result`
    // across attempts via `Arc<Mutex<Option<...>>>` so the closure can
    // .take() it without moving the outer binding.
    let payload: Arc<Mutex<Option<std::result::Result<ipc_client::HistoryPage, String>>>> =
        Arc::new(Mutex::new(Some(result)));

    for attempt in 0..3 {
        let window_weak_attempt = window_weak.clone();
        let state_for_apply = Arc::clone(&state);
        let payload_for_attempt = Arc::clone(&payload);
        let post = slint::invoke_from_event_loop(move || {
            if let Some(win) = window_weak_attempt.upgrade() {
                if let Some(result) = lock_or_recover(&payload_for_attempt).take() {
                    apply_history_result(&win, &state_for_apply, result, offset);
                }
            }
        });
        match post {
            Ok(()) => return,
            Err(e) => {
                eprintln!(
                    "[autostart] invoke_from_event_loop attempt {} failed: {e}",
                    attempt + 1
                );
                if attempt < 2 {
                    std::thread::sleep(Duration::from_millis(500));
                }
            }
        }
    }
    eprintln!(
        "[autostart] giving up on post-autostart refresh after 3 attempts — \
         user can click Refresh once the window appears"
    );
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
            if let Err(e) = slint::invoke_from_event_loop(move || {
                update_recents(recents);
            }) {
                tracing::debug!(error = %e, "ui update dropped during event-loop shutdown");
            }
        });
    };

    // Prime once immediately so the first menu open shows real history.
    refresh();

    // Repeating timer on the Slint event loop.
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        TRAY_REFRESH_INTERVAL,
        move || {
            refresh();
        },
    );
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

            // v0.3 T3: feed the visible-count badge.
            win.set_visible_count(slint_items.len() as i32);
            let model = slint::VecModel::from(slint_items);
            win.set_items(slint::ModelRc::new(model));
        }
    }
}
