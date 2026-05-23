# copypaste-ui

## Purpose
Slint-based desktop UI for CopyPaste. Provides the history window, settings, and the P2P pairing flow; runs in-process with the daemon and talks to it through the IPC client.

## Public API
Hybrid `bin` + `lib` crate. Binary entry point is `src/main.rs` (history window). Library re-exports (`src/lib.rs`):

- `windows::SettingsWindowHandle` — app-settings window.
- `windows::PairWindowHandle` — P2P pairing window.
- `windows::{SearchableHistoryItem, filter_history_items}` — history filtering helpers.
- `settings::{AppSettings, HistoryLimit, PairedDevice}`.
- `tray_menu::{TrayMenuHandle, TrayMenuState, MenuEntry, RecentItem, TrayAction, MAX_PREVIEW_CHARS, MAX_RECENT_ITEMS}`.
- `fingerprint::{format_fingerprint, format_fingerprint_short, format_fingerprint_long, format_fingerprint_truncated, is_valid_fingerprint}`.
- `ipc_client` — async IPC client used by all windows (~30 KB).

All Slint properties are one-way `in` bindings driven from Rust; callbacks register through `on_*` handle methods so callers never depend on generated Slint types directly.

## Platform support
- **macOS**: primary.
- **Linux / Windows**: builds; tray integration disabled outside macOS.
- **Android**: not applicable.

## Status
beta.

## Internal vs published
Internal binary + library crate. Not published to crates.io.

## Quick example

```bash
# Run the UI alongside the daemon.
cargo run -p copypaste-ui
```

## Tests
2 integration tests under `tests/`: IPC client roundtrip, windows snapshot.

```bash
cargo test -p copypaste-ui
```

## Related ADRs
- [ADR-005](../../docs/adr/ADR-005-slint-ui-framework.md) — Slint chosen as the UI framework.
