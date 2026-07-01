//! macOS-only managed state for the popup: prior-app tracking + CGEventTap
//! install status.

// `Mutex` backs only the macOS-only managed state (`PriorApp`/`TapActive`), so
// gate the import to macOS — otherwise it is an unused import on Linux (-D warnings).
#[cfg(target_os = "macos")]
use std::sync::Mutex;

/// Bundle ID (or process identifier as fallback) of the app that was
/// frontmost when the popup was last shown.  Used to restore focus after
/// the user picks an item.
#[cfg(target_os = "macos")]
pub(crate) struct PriorApp(pub(crate) Mutex<Option<String>>);

/// Whether the CGEventTap is active (Accessibility permission was granted and
/// the tap was successfully installed).
#[cfg(target_os = "macos")]
pub(crate) struct TapActive(pub(crate) Mutex<bool>);
