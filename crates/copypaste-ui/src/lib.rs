// lib.rs — copypaste-ui crate root
// Provides Slint UI windows for CopyPaste: SettingsWindow and PairWindow.

pub mod fingerprint;
pub mod settings;
pub mod windows;

pub use settings::{AppSettings, HistoryLimit, PairedDevice};
pub use windows::{SettingsWindowHandle, PairWindowHandle};
pub use fingerprint::{
    format_fingerprint,
    format_fingerprint_short,
    format_fingerprint_long,
    format_fingerprint_truncated,
    is_valid_fingerprint,
};
