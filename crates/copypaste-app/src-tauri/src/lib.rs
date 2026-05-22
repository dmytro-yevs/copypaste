pub mod commands;
pub mod ipc;
pub mod paths;

use tauri::{
    Manager,
    tray::{TrayIconBuilder, TrayIconEvent},
    WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_positioner::init())
        .setup(|app| {
            // Hide app from Dock — it lives only in menu bar
            #[cfg(target_os = "macos")]
            {
                app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            let handle = app.handle().clone();
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().unwrap().clone())
                .icon_as_template(true)
                .tooltip("CopyPaste")
                .on_tray_icon_event(move |_tray, event| {
                    if let TrayIconEvent::Click { .. } = event {
                        let window = handle
                            .get_webview_window("main")
                            .expect("main window must exist");
                        let _ = window.move_window(Position::BottomCenter);
                        if window.is_visible().unwrap_or(false) {
                            let _ = window.hide();
                        } else {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            Ok(())
        })
        .on_window_event(|window, event| {
            // Auto-hide when window loses focus (user clicks elsewhere)
            if let WindowEvent::Focused(false) = event {
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_items,
            commands::search_items,
            commands::delete_item,
        ])
        .run(tauri::generate_context!())
        .expect("error while running CopyPaste app");
}
