# Tauri Keyboard Shortcuts

## Current
- `Escape` — hide clipboard history window (implemented in app.js)
- Tray icon click — toggle window visibility

## Planned (Phase 3.1)
- `Cmd+Shift+V` (macOS) / `Ctrl+Shift+V` (Windows/Linux) — global hotkey to show history

## Implementation

Requires `tauri-plugin-global-shortcut`:
```toml
# crates/copypaste-app/src-tauri/Cargo.toml
tauri-plugin-global-shortcut = "2"
```

```rust
// In lib.rs setup:
app.global_shortcut().register("CommandOrControl+Shift+V", |app, _| {
    let window = app.get_webview_window("main").unwrap();
    if window.is_visible().unwrap_or(false) {
        window.hide().unwrap();
    } else {
        window.show().unwrap();
        window.set_focus().unwrap();
    }
})?;
```

Note: global shortcuts on macOS require the app to be code-signed for system-wide capture.
