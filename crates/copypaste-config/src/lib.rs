//! Unified runtime [`AppConfig`] for the CopyPaste workspace.
//!
//! This crate centralises **path / port / log-level** configuration that is
//! shared across the daemon, CLI, relay, and UI binaries. It is intentionally
//! **separate** from `copypaste_core::config::AppConfig`, which owns
//! user-facing tunables (history limits, TTLs, image quality, etc.).
//!
//! ## Loading order
//!
//! [`AppConfig::load`] resolves values in this precedence (first wins):
//!
//! 1. Environment overrides:
//!    - `COPYPASTE_DATA_DIR`
//!    - `COPYPASTE_SOCKET_PATH`
//!    - `COPYPASTE_LOG_LEVEL`
//!    - `COPYPASTE_DB_KEY_PATH`
//!    - `COPYPASTE_RELAY_PORT`
//!    - `COPYPASTE_MDNS_SERVICE`
//! 2. On-disk `config.json` inside the resolved `data_dir` (if present).
//! 3. Per-platform defaults from [`AppConfig::defaults`].
//!
//! ## Persistence
//!
//! [`AppConfig::save`] writes `data_dir/config.json` (pretty JSON), creating
//! `data_dir` if it does not yet exist.
//!
//! ## Wiring
//!
//! Consumer crates (daemon, cli, relay, ui) are **not** wired in this task —
//! that is a follow-up. This crate ships standalone with round-trip tests.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![deny(rust_2018_idioms)]

mod error;

pub use error::ConfigError;

use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Project qualifier triple used to resolve per-platform data directories
/// via [`directories::ProjectDirs`].
const QUALIFIER: &str = "io";
const ORGANIZATION: &str = "CopyPaste";
const APPLICATION: &str = "CopyPaste";

/// File name persisted inside `data_dir`.
pub const CONFIG_FILE_NAME: &str = "config.json";

/// Default relay port (matches existing relay defaults across the workspace).
pub const DEFAULT_RELAY_PORT: u16 = 7777;

/// Default mDNS service type advertised by p2p / relay discovery.
pub const DEFAULT_MDNS_SERVICE: &str = "_copypaste._tcp.local.";

/// Default tracing log level filter.
pub const DEFAULT_LOG_LEVEL: &str = "info";

/// Unix-domain-socket file name used by the daemon IPC layer.
pub const DEFAULT_SOCKET_FILE: &str = "daemon.sock";

/// Keychain-fallback DB key file name (used when the OS keychain is unavailable).
pub const DEFAULT_DB_KEY_FILE: &str = "db.key";

/// Unified runtime configuration shared by daemon, CLI, relay, and UI.
///
/// All fields are concrete (no `Option`s) — defaults always resolve. Use
/// [`AppConfig::load`] to honour env overrides + on-disk persistence; use
/// [`AppConfig::defaults`] for an in-memory baseline.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Per-platform writable data directory (db, key, logs, config.json).
    pub data_dir: PathBuf,
    /// Path to the daemon Unix-domain socket.
    pub socket_path: PathBuf,
    /// `tracing-subscriber` env-filter compatible log level (`info`, `debug`, …).
    pub log_level: String,
    /// On-disk fallback for the database encryption key (keychain preferred).
    pub db_key_path: PathBuf,
    /// TCP port the local relay listens on.
    pub relay_port: u16,
    /// mDNS-SD service type (must end with `.local.`).
    pub mdns_service: String,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self::defaults()
    }
}

impl AppConfig {
    /// In-memory defaults derived from the per-platform project data directory.
    ///
    /// Falls back to `./.copypaste` (relative to CWD) if no platform-standard
    /// directory can be resolved — keeps the type total and panic-free.
    #[must_use]
    pub fn defaults() -> Self {
        let data_dir = resolve_data_dir().unwrap_or_else(|| PathBuf::from(".copypaste"));
        Self::with_data_dir(data_dir)
    }

    /// Build defaults rooted at an explicit `data_dir`. Useful for tests and
    /// for callers that want full control (e.g. CLI `--data-dir` flag).
    #[must_use]
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        let socket_path = data_dir.join(DEFAULT_SOCKET_FILE);
        let db_key_path = data_dir.join(DEFAULT_DB_KEY_FILE);
        Self {
            data_dir,
            socket_path,
            log_level: DEFAULT_LOG_LEVEL.to_string(),
            db_key_path,
            relay_port: DEFAULT_RELAY_PORT,
            mdns_service: DEFAULT_MDNS_SERVICE.to_string(),
        }
    }

    /// Load merged configuration: defaults → on-disk `config.json` → env overrides.
    ///
    /// `data_dir` is resolved first from `COPYPASTE_DATA_DIR` (if set), then
    /// from platform defaults. The on-disk `config.json` is read from inside
    /// the resolved `data_dir`. Missing file is **not** an error.
    pub fn load() -> Result<Self, ConfigError> {
        // 1. Resolve effective data_dir (env override > platform default).
        let env_data_dir = std::env::var_os("COPYPASTE_DATA_DIR").map(PathBuf::from);
        let base_data_dir = env_data_dir
            .clone()
            .or_else(resolve_data_dir)
            .ok_or(ConfigError::Path)?;

        // 2. Seed from defaults rooted at the resolved data_dir.
        let mut cfg = Self::with_data_dir(base_data_dir.clone());

        // 3. Overlay on-disk config.json if present.
        let cfg_path = base_data_dir.join(CONFIG_FILE_NAME);
        if cfg_path.is_file() {
            let text = std::fs::read_to_string(&cfg_path)
                .map_err(|e| ConfigError::io(&cfg_path, e))?;
            cfg = serde_json::from_str(&text)?;
            // If the on-disk file was written with a stale data_dir, but the
            // user explicitly overrode it via env, prefer the env value.
            if env_data_dir.is_some() {
                cfg.data_dir = base_data_dir;
            }
        }

        // 4. Apply environment overrides (highest precedence).
        cfg.apply_env_overrides();

        Ok(cfg)
    }

    /// Persist `self` as pretty JSON at `data_dir/config.json`, creating
    /// `data_dir` if missing.
    pub fn save(&self) -> Result<(), ConfigError> {
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|e| ConfigError::io(&self.data_dir, e))?;
        let path = self.data_dir.join(CONFIG_FILE_NAME);
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, json).map_err(|e| ConfigError::io(&path, e))?;
        Ok(())
    }

    /// Convenience accessor: full path to the persisted config file.
    #[must_use]
    pub fn config_file_path(&self) -> PathBuf {
        self.data_dir.join(CONFIG_FILE_NAME)
    }

    /// Apply `COPYPASTE_*` environment variables on top of `self`.
    /// Invalid values (e.g. non-numeric `COPYPASTE_RELAY_PORT`) are ignored
    /// rather than failing — env overrides should never crash the daemon.
    pub fn apply_env_overrides(&mut self) {
        if let Some(v) = std::env::var_os("COPYPASTE_DATA_DIR") {
            self.data_dir = PathBuf::from(v);
        }
        if let Some(v) = std::env::var_os("COPYPASTE_SOCKET_PATH") {
            self.socket_path = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("COPYPASTE_LOG_LEVEL") {
            if !v.trim().is_empty() {
                self.log_level = v;
            }
        }
        if let Some(v) = std::env::var_os("COPYPASTE_DB_KEY_PATH") {
            self.db_key_path = PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("COPYPASTE_RELAY_PORT") {
            if let Ok(port) = v.parse::<u16>() {
                self.relay_port = port;
            }
        }
        if let Ok(v) = std::env::var("COPYPASTE_MDNS_SERVICE") {
            if !v.trim().is_empty() {
                self.mdns_service = v;
            }
        }
    }
}

/// Resolve per-platform project data directory using `directories::ProjectDirs`.
fn resolve_data_dir() -> Option<PathBuf> {
    ProjectDirs::from(QUALIFIER, ORGANIZATION, APPLICATION).map(|p| p.data_dir().to_path_buf())
}

/// Internal helper, exposed for `cfg(test)` integration tests to construct
/// configs rooted at arbitrary directories without going through env vars.
#[doc(hidden)]
pub fn __test_with_data_dir(data_dir: &Path) -> AppConfig {
    AppConfig::with_data_dir(data_dir.to_path_buf())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_have_non_empty_fields() {
        let cfg = AppConfig::defaults();
        assert!(!cfg.log_level.is_empty());
        assert!(!cfg.mdns_service.is_empty());
        assert_eq!(cfg.relay_port, DEFAULT_RELAY_PORT);
        assert!(cfg.socket_path.ends_with(DEFAULT_SOCKET_FILE));
        assert!(cfg.db_key_path.ends_with(DEFAULT_DB_KEY_FILE));
        // mDNS service convention.
        assert!(cfg.mdns_service.ends_with(".local."));
    }

    #[test]
    fn with_data_dir_roots_all_paths() {
        let cfg = AppConfig::with_data_dir(PathBuf::from("/tmp/cp-x"));
        assert_eq!(cfg.data_dir, PathBuf::from("/tmp/cp-x"));
        assert_eq!(cfg.socket_path, PathBuf::from("/tmp/cp-x/daemon.sock"));
        assert_eq!(cfg.db_key_path, PathBuf::from("/tmp/cp-x/db.key"));
    }

    #[test]
    fn config_file_path_lives_inside_data_dir() {
        let cfg = AppConfig::with_data_dir(PathBuf::from("/tmp/cp-y"));
        assert_eq!(
            cfg.config_file_path(),
            PathBuf::from("/tmp/cp-y/config.json")
        );
    }
}
