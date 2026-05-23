//! Integration tests for [`copypaste_config::AppConfig`] load/save round-trips.
//!
//! These tests use a `tempdir` and pin `COPYPASTE_DATA_DIR` so they do not
//! touch the user's real platform data directory.
//!
//! Env-touching tests are kept in this single integration binary so they run
//! in one process (env vars are process-global). Inside the binary they run
//! sequentially via a `Mutex` guard to avoid races.

use copypaste_config::{AppConfig, CONFIG_FILE_NAME, DEFAULT_RELAY_PORT};
use std::path::PathBuf;
use std::sync::Mutex;
use tempfile::tempdir;

/// Serialises env-mutating tests so they cannot race each other.
static ENV_LOCK: Mutex<()> = Mutex::new(());

const ENV_KEYS: &[&str] = &[
    "COPYPASTE_DATA_DIR",
    "COPYPASTE_SOCKET_PATH",
    "COPYPASTE_LOG_LEVEL",
    "COPYPASTE_DB_KEY_PATH",
    "COPYPASTE_RELAY_PORT",
    "COPYPASTE_MDNS_SERVICE",
];

fn clear_env() {
    for k in ENV_KEYS {
        std::env::remove_var(k);
    }
}

#[test]
fn round_trip_save_then_load_preserves_all_fields() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();

    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    std::env::set_var("COPYPASTE_DATA_DIR", &data_dir);

    let mut original = AppConfig::with_data_dir(data_dir.clone());
    original.log_level = "debug".into();
    original.relay_port = 9090;
    original.mdns_service = "_copypaste-test._tcp.local.".into();
    original.socket_path = data_dir.join("custom.sock");
    original.db_key_path = data_dir.join("custom.key");

    original.save().expect("save");

    // Persisted file is exactly where we expect.
    let cfg_path = data_dir.join(CONFIG_FILE_NAME);
    assert!(cfg_path.is_file(), "config.json must exist at {cfg_path:?}");

    let loaded = AppConfig::load().expect("load");
    assert_eq!(loaded, original);

    clear_env();
}

#[test]
fn load_without_file_returns_defaults_rooted_at_env_data_dir() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();

    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    std::env::set_var("COPYPASTE_DATA_DIR", &data_dir);

    let cfg = AppConfig::load().expect("load");
    assert_eq!(cfg.data_dir, data_dir);
    assert_eq!(cfg.relay_port, DEFAULT_RELAY_PORT);
    assert_eq!(cfg.socket_path, data_dir.join("daemon.sock"));
    assert_eq!(cfg.db_key_path, data_dir.join("db.key"));

    clear_env();
}

#[test]
fn env_overrides_take_precedence_over_disk() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();

    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();

    // Write a disk config with port 1111 and log_level=warn.
    std::env::set_var("COPYPASTE_DATA_DIR", &data_dir);
    let mut on_disk = AppConfig::with_data_dir(data_dir.clone());
    on_disk.relay_port = 1111;
    on_disk.log_level = "warn".into();
    on_disk.save().unwrap();

    // Now overlay env overrides.
    std::env::set_var("COPYPASTE_RELAY_PORT", "2222");
    std::env::set_var("COPYPASTE_LOG_LEVEL", "trace");
    std::env::set_var("COPYPASTE_MDNS_SERVICE", "_cp-env._tcp.local.");

    let cfg = AppConfig::load().expect("load");
    assert_eq!(cfg.relay_port, 2222, "env must beat disk");
    assert_eq!(cfg.log_level, "trace");
    assert_eq!(cfg.mdns_service, "_cp-env._tcp.local.");

    clear_env();
}

#[test]
fn invalid_relay_port_env_is_ignored() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();

    let dir = tempdir().unwrap();
    let data_dir = dir.path().to_path_buf();
    std::env::set_var("COPYPASTE_DATA_DIR", &data_dir);
    std::env::set_var("COPYPASTE_RELAY_PORT", "not-a-port");

    let cfg = AppConfig::load().expect("load");
    assert_eq!(
        cfg.relay_port, DEFAULT_RELAY_PORT,
        "garbage env value must not panic; default retained"
    );

    clear_env();
}

#[test]
fn save_creates_data_dir_if_missing() {
    let _guard = ENV_LOCK.lock().unwrap();
    clear_env();

    let dir = tempdir().unwrap();
    let nested: PathBuf = dir.path().join("a/b/c");
    assert!(!nested.exists());

    let cfg = AppConfig::with_data_dir(nested.clone());
    cfg.save().expect("save should mkdir -p");

    assert!(nested.is_dir());
    assert!(nested.join(CONFIG_FILE_NAME).is_file());

    clear_env();
}
