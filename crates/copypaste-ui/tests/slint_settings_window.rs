// tests/slint_settings_window.rs — headless Slint tests for SettingsWindow.
//
// Verifies: properties round-trip, all major callbacks are wired and fire,
// SettingsWindowHandle helper methods work correctly without a display.

use std::cell::Cell;
use std::rc::Rc;

slint::include_modules!();

fn init_backend() {
    i_slint_backend_testing::init_no_event_loop();
}

// ── Property smoke-tests ────────────────────────────────────────────────────

#[test]
fn settings_window_can_be_created_headless() {
    init_backend();
    // Verify the window can be created without a display.
    let win = SettingsWindow::new().expect("SettingsWindow::new must succeed in headless mode");
    // Touch any property to ensure the generated code is alive.
    let _ = win.get_launch_at_login();
}

#[test]
fn settings_window_launch_at_login_roundtrip() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");
    win.set_launch_at_login(true);
    assert!(
        win.get_launch_at_login(),
        "launch-at-login must round-trip true"
    );
    win.set_launch_at_login(false);
    assert!(
        !win.get_launch_at_login(),
        "launch-at-login must round-trip false"
    );
}

#[test]
fn settings_window_private_mode_roundtrip() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");
    win.set_private_mode(true);
    assert!(win.get_private_mode());
    win.set_private_mode(false);
    assert!(!win.get_private_mode());
}

#[test]
fn settings_window_history_size_roundtrip() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");
    win.set_history_size(500);
    assert_eq!(win.get_history_size(), 500, "history-size must round-trip");
    win.set_history_size(0);
    assert_eq!(win.get_history_size(), 0);
}

#[test]
fn settings_window_supabase_fields_roundtrip() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");
    win.set_supabase_url("https://example.supabase.co".into());
    win.set_supabase_key("eyJtest".into());
    assert_eq!(
        win.get_supabase_url().as_str(),
        "https://example.supabase.co"
    );
    assert_eq!(win.get_supabase_key().as_str(), "eyJtest");
}

#[test]
fn settings_window_device_name_roundtrip() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");
    win.set_device_name("My MacBook".into());
    assert_eq!(win.get_device_name().as_str(), "My MacBook");
}

#[test]
fn settings_window_app_version_roundtrip() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");
    win.set_app_version("0.3.0-alpha.1".into());
    assert_eq!(win.get_app_version().as_str(), "0.3.0-alpha.1");
}

#[test]
fn settings_window_device_fingerprint_roundtrip() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");
    let fp = "AA BB CC DD EE FF 00 11 22 33 44 55 66 77 88 99";
    win.set_device_fingerprint(fp.into());
    assert_eq!(win.get_device_fingerprint().as_str(), fp);
}

#[test]
fn settings_window_sync_status_roundtrip() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");
    win.set_sync_connected(true);
    win.set_sync_status_msg("Connected".into());
    assert!(win.get_sync_connected());
    assert_eq!(win.get_sync_status_msg().as_str(), "Connected");

    win.set_sync_connected(false);
    assert!(!win.get_sync_connected());
}

// ── Callback wiring tests ───────────────────────────────────────────────────

#[test]
fn settings_window_save_callback_fires() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");

    let fired = Rc::new(Cell::new(false));
    win.on_save({
        let fired = fired.clone();
        move || fired.set(true)
    });

    win.invoke_save();
    assert!(fired.get(), "save callback must fire when invoked");
}

#[test]
fn settings_window_clear_history_callback_fires() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");

    let fired = Rc::new(Cell::new(false));
    win.on_clear_history({
        let fired = fired.clone();
        move || fired.set(true)
    });

    win.invoke_clear_history();
    assert!(fired.get(), "clear-history callback must fire when invoked");
}

#[test]
fn settings_window_close_callback_fires() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");

    let fired = Rc::new(Cell::new(false));
    win.on_close({
        let fired = fired.clone();
        move || fired.set(true)
    });

    win.invoke_close();
    assert!(fired.get(), "close callback must fire when invoked");
}

#[test]
fn settings_window_connect_supabase_callback_receives_args() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");

    let received = Rc::new(std::cell::RefCell::new((String::new(), String::new())));
    win.on_connect_supabase({
        let received = received.clone();
        move |url, key| {
            *received.borrow_mut() = (url.to_string(), key.to_string());
        }
    });

    win.invoke_connect_supabase("https://test.supabase.co".into(), "anon-key".into());
    let (url, key) = received.borrow().clone();
    assert_eq!(
        url, "https://test.supabase.co",
        "connect-supabase must receive url"
    );
    assert_eq!(key, "anon-key", "connect-supabase must receive key");
}

#[test]
fn settings_window_disconnect_supabase_callback_fires() {
    init_backend();
    let win = SettingsWindow::new().expect("SettingsWindow::new");

    let fired = Rc::new(Cell::new(false));
    win.on_disconnect_supabase({
        let fired = fired.clone();
        move || fired.set(true)
    });

    win.invoke_disconnect_supabase();
    assert!(fired.get(), "disconnect-supabase callback must fire");
}

// ── SettingsWindowHandle helper tests ──────────────────────────────────────
//
// The handle wraps the raw Slint window and provides ergonomic setters +
// the `perform_save` / `perform_clear_history` pure functions.

#[test]
fn settings_window_handle_new_populates_from_settings() {
    init_backend();
    use copypaste_ui::settings::{AppSettings, HistoryLimit};

    let settings = AppSettings {
        launch_at_login: true,
        private_mode: false,
        history_limit: HistoryLimit::from_count(200),
        supabase_url: "https://proj.supabase.co".into(),
        supabase_key: "key123".into(),
        device_name: "Test Mac".into(),
    };
    let handle = copypaste_ui::windows::SettingsWindowHandle::new(&settings, "0.3.0-test", "AABB")
        .expect("SettingsWindowHandle::new must succeed in headless mode");

    let current = handle.current_settings();
    assert!(current.launch_at_login, "launch_at_login must be populated");
    assert!(!current.private_mode, "private_mode must be populated");
    assert_eq!(current.device_name, "Test Mac");
    assert_eq!(current.supabase_url, "https://proj.supabase.co");
}

#[test]
fn settings_window_handle_apply_settings_updates_fields() {
    init_backend();
    use copypaste_ui::settings::{AppSettings, HistoryLimit};

    let initial = AppSettings {
        launch_at_login: false,
        private_mode: false,
        history_limit: HistoryLimit::from_count(100),
        supabase_url: String::new(),
        supabase_key: String::new(),
        device_name: "Old Name".into(),
    };
    let handle = copypaste_ui::windows::SettingsWindowHandle::new(&initial, "0.1", "FP")
        .expect("SettingsWindowHandle::new");

    let updated = AppSettings {
        launch_at_login: true,
        private_mode: true,
        history_limit: HistoryLimit::from_count(500),
        supabase_url: "https://new.supabase.co".into(),
        supabase_key: "newkey".into(),
        device_name: "New Name".into(),
    };
    handle.apply_settings(&updated);

    let current = handle.current_settings();
    assert!(
        current.launch_at_login,
        "apply_settings must update launch_at_login"
    );
    assert!(
        current.private_mode,
        "apply_settings must update private_mode"
    );
    assert_eq!(current.device_name, "New Name");
}

// ── perform_save / perform_clear_history pure-function tests ───────────────

#[test]
fn perform_save_ok_returns_success_status() {
    use copypaste_ui::settings::{AppSettings, HistoryLimit};
    use copypaste_ui::windows::{perform_save, SettingsIpc};

    struct OkIpc;
    impl SettingsIpc for OkIpc {
        fn get_settings(&mut self) -> Result<AppSettings, String> {
            Ok(AppSettings {
                launch_at_login: false,
                private_mode: false,
                history_limit: HistoryLimit::from_count(100),
                supabase_url: String::new(),
                supabase_key: String::new(),
                device_name: String::new(),
            })
        }
        fn save_settings(&mut self, _: &AppSettings) -> Result<(), String> {
            Ok(())
        }
        fn delete_all_history(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    let settings = AppSettings {
        launch_at_login: false,
        private_mode: false,
        history_limit: HistoryLimit::from_count(100),
        supabase_url: String::new(),
        supabase_key: String::new(),
        device_name: String::new(),
    };

    let result = perform_save(&mut OkIpc, &settings);
    assert!(!result.is_error, "successful save must not be an error");
    assert!(
        result.message.contains("saved"),
        "success message must mention 'saved', got: {}",
        result.message
    );
}

#[test]
fn perform_save_err_returns_error_status_with_message() {
    use copypaste_ui::settings::{AppSettings, HistoryLimit};
    use copypaste_ui::windows::{perform_save, SettingsIpc};

    struct FailIpc;
    impl SettingsIpc for FailIpc {
        fn get_settings(&mut self) -> Result<AppSettings, String> {
            Err("not connected".into())
        }
        fn save_settings(&mut self, _: &AppSettings) -> Result<(), String> {
            Err("daemon offline".into())
        }
        fn delete_all_history(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    let settings = AppSettings {
        launch_at_login: false,
        private_mode: false,
        history_limit: HistoryLimit::from_count(100),
        supabase_url: String::new(),
        supabase_key: String::new(),
        device_name: String::new(),
    };

    let result = perform_save(&mut FailIpc, &settings);
    assert!(result.is_error, "failed save must be an error");
    assert!(
        result.message.contains("daemon offline"),
        "error message must include IPC error, got: {}",
        result.message
    );
}

#[test]
fn perform_clear_history_ok_returns_success() {
    use copypaste_ui::settings::{AppSettings, HistoryLimit};
    use copypaste_ui::windows::{perform_clear_history, SettingsIpc};

    struct OkIpc;
    impl SettingsIpc for OkIpc {
        fn get_settings(&mut self) -> Result<AppSettings, String> {
            Ok(AppSettings {
                launch_at_login: false,
                private_mode: false,
                history_limit: HistoryLimit::from_count(100),
                supabase_url: String::new(),
                supabase_key: String::new(),
                device_name: String::new(),
            })
        }
        fn save_settings(&mut self, _: &AppSettings) -> Result<(), String> {
            Ok(())
        }
        fn delete_all_history(&mut self) -> Result<(), String> {
            Ok(())
        }
    }

    let result = perform_clear_history(&mut OkIpc);
    assert!(!result.is_error, "successful clear must not be an error");
    assert!(
        result.message.contains("cleared"),
        "success message must mention 'cleared', got: {}",
        result.message
    );
}
