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

pub mod app_icon;
pub mod clipboard;
pub mod daemon;
pub mod device_meta;
#[cfg(unix)]
pub mod ipc;
pub mod keychain;
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

/// Process-global lock shared by ALL test modules that mutate env vars
/// (`HOME`, `XDG_CONFIG_HOME`, `USERPROFILE`, etc.).
///
/// Env mutation is process-global and racy; every test that calls
/// `std::env::set_var` / `remove_var` **must** hold this lock for its
/// entire mutation window — including any `.await` points where the
/// redirected value must remain stable.  A single shared static guarantees
/// that tests in different modules (e.g. `ipc`, `paths`) cannot overlap
/// their env windows even though they run in separate OS threads under
/// `cargo test --test-threads=N`.
#[cfg(test)]
pub static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
