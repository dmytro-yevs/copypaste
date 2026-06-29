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

// reason: the lib facade re-exports internal modules for integration tests;
// many items are not used by the lib itself but are reached by the binary
// (main.rs) or by tests/*.rs — the compiler can't see those use-sites from lib.rs.
#![allow(dead_code)]

pub mod app_icon;
/// Upload bandwidth throttler (token-bucket) for the relay and cloud push
/// paths (CopyPaste-crh3.107).
pub mod bandwidth;
pub mod clipboard;
/// Shared IPC config type re-export + structural-consistency tests.
/// See [`copypaste_ipc::AppConfig`] for the canonical definition.
pub mod config;
pub mod daemon;
pub mod device_meta;
/// Atomic 0600-mode file write helper and pasteboard sentinel helpers,
/// consolidated from `ipc::config::atomic_write_0600` and
/// `daemon::startup::write_text_atomic_0600` (CopyPaste-54it #7/#9).
pub mod fs_atomic;
#[cfg(unix)]
pub mod ipc;
/// Shared HTTPS URL validation guard (g06m.32 #2 — replaces duplicate
/// `cloud::config::is_https_url` and `relay::registration::is_relay_url_ok`).
pub mod url_guard;
// P2-o8ew: wire the Windows named-pipe IPC skeleton under cfg(windows) so it is
// no longer an undeclared orphan on disk (its own header wrongly claimed it was
// already declared). Compiled out on unix; Windows is frozen per ADR-012, so no
// active CI target builds it. Kept (not deleted) per the module's "Do not delete".
#[cfg(windows)]
pub mod ipc_win;
pub mod keychain;
pub mod logging;
pub mod p2p;
pub mod pairing_sm;
pub mod paths;
pub mod peers;
pub mod platform;
pub mod protocol;
pub mod public_ip;
/// Shared `(wall_time, id)` keyset-pagination cursor (CopyPaste-w47w #3).
pub mod sync_cursor;
/// RAII guard for the `sync_in_flight` `AtomicBool` flag (CopyPaste-1jms.22).
pub mod sync_in_flight;
pub mod sync_orch;

#[cfg(feature = "cloud-sync")]
pub mod cloud;

/// Relay-as-database sync client (register / push / subscribe).
#[cfg(feature = "relay-sync")]
pub mod relay;

/// Shared sync pipeline helpers reused by both the Supabase ([`cloud`]) and
/// relay ([`relay`]) paths. Gated on either feature.
#[cfg(any(feature = "cloud-sync", feature = "relay-sync"))]
pub mod sync_common;

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
