//! copypaste-daemon — library facade.
//!
//! Beta-bonus: promoted from binary-only to hybrid `bin` + `lib` so that
//! integration tests under `tests/*.rs` can reach the internal modules
//! (`sync_orch`, `p2p`, `ipc`, `protocol`, etc.) through their public
//! surface instead of duplicating logic via stand-ins.
//!
//! The binary entry point lives in `src/main.rs` and re-uses these modules
//! via `use copypaste_daemon::*;`.  All cfg gates (`#[cfg(unix)]`,
//! `#[cfg(target_os = "macos")]`, `#[cfg(feature = "cloud-sync")]`) mirror
//! the original `mod` declarations in `main.rs` exactly — no behavioural
//! change.

#![allow(dead_code)]

pub mod clipboard;
pub mod daemon;
#[cfg(unix)]
pub mod ipc;
pub mod keychain;
pub mod launchd;
pub mod logging;
pub mod p2p;
pub mod paths;
pub mod peers;
pub mod platform;
pub mod protocol;
pub mod sync_orch;

#[cfg(feature = "cloud-sync")]
pub mod cloud;

// v0.3: the menu-bar tray module moved to `copypaste-ui::tray_host`. The
// daemon process is started by launchd and cannot host an NSApplication
// main run loop on macOS, which `tray-icon` / `muda::Menu` require.
