//! Slint UI windows for the CopyPaste daemon.
//!
//! Exposes the following user-facing surfaces:
//! * [`windows::SettingsWindowHandle`] — app configuration window.
//! * [`windows::PairWindowHandle`] — P2P device pairing window.
//!
//! The history window lives in the `copypaste-ui` binary (`src/main.rs`); the
//! `lib.rs` surface intentionally re-exports only the secondary windows and
//! the value types they need.
//!
//! UI state is driven entirely from Rust — every Slint property is a one-way
//! `in` binding updated through the handle's setters. The handles also expose
//! `on_*` registration methods so a host application can wire callbacks
//! without depending on the generated Slint types directly.

pub mod fingerprint;
pub mod settings;
pub mod tray_menu;
pub mod updater;
pub mod windows;

pub use settings::{AppSettings, HistoryLimit, PairedDevice};
pub use tray_menu::{
    MenuEntry, RecentItem, TrayAction, TrayMenuHandle, TrayMenuState,
    MAX_PREVIEW_CHARS, MAX_RECENT_ITEMS,
};
pub use windows::{SettingsWindowHandle, PairWindowHandle, SearchableHistoryItem, filter_history_items};
pub use fingerprint::{
    format_fingerprint,
    format_fingerprint_short,
    format_fingerprint_long,
    format_fingerprint_truncated,
    is_valid_fingerprint,
};
pub use updater::{
    apply_update, check_for_update, CommandRunner, SystemRunner, UpdateInfo, UpdateStatus,
    CHECK_INTERVAL,
};
