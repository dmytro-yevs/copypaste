mod defaults;
pub use defaults::*;

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("TOML parse error: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("TOML serialize error: {0}")]
    Serialize(#[from] toml::ser::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub config_version: u32,
    pub history_limit: usize,
    pub poll_interval_ms: u64,
    pub max_text_size_bytes: u64,
    pub max_image_size_bytes: u64,
    pub max_file_size_bytes: u64,
    pub storage_quota_bytes: u64,
    pub sync_ttl_secs: u64,
    pub sensitive_ttl_relay_secs: u64,
    pub sensitive_ttl_local_secs: u64,
    pub image_quality: u8,
    pub sqlite_cache_mb: u32,
    pub encryption_chunk_kb: u32,
    pub sync_on_wifi_only: bool,
    pub max_bandwidth_kbps: u32,
    pub max_decoded_image_mb: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            config_version: CONFIG_VERSION,
            history_limit: HISTORY_LIMIT,
            poll_interval_ms: POLL_INTERVAL_MS,
            max_text_size_bytes: MAX_TEXT_SIZE_BYTES,
            max_image_size_bytes: MAX_IMAGE_SIZE_BYTES,
            max_file_size_bytes: MAX_FILE_SIZE_BYTES,
            storage_quota_bytes: STORAGE_QUOTA_BYTES,
            sync_ttl_secs: SYNC_TTL_SECS,
            sensitive_ttl_relay_secs: SENSITIVE_TTL_RELAY_SECS,
            sensitive_ttl_local_secs: SENSITIVE_TTL_LOCAL_SECS,
            image_quality: IMAGE_QUALITY,
            sqlite_cache_mb: SQLITE_CACHE_MB,
            encryption_chunk_kb: ENCRYPTION_CHUNK_KB,
            sync_on_wifi_only: false,
            max_bandwidth_kbps: MAX_BANDWIDTH_KBPS,
            max_decoded_image_mb: MAX_DECODED_IMAGE_MB,
        }
    }
}

impl AppConfig {
    pub fn load(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path)?;
        let mut cfg: Self = toml::from_str(&text)?;
        cfg.clamp_values();
        Ok(cfg)
    }

    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        let text = toml::to_string_pretty(self)?;
        std::fs::write(path, text)?;
        Ok(())
    }

    fn clamp_values(&mut self) {
        self.poll_interval_ms = self
            .poll_interval_ms
            .max(POLL_INTERVAL_MIN_MS)
            .min(POLL_INTERVAL_MAX_MS);
        self.image_quality = self.image_quality.min(100).max(1);
        self.encryption_chunk_kb = self.encryption_chunk_kb.max(16).min(4096);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_config_serializes_and_roundtrips() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = AppConfig::default();
        cfg.save(&path).unwrap();
        let loaded = AppConfig::load(&path).unwrap();
        assert_eq!(loaded.history_limit, 1000);
        assert_eq!(loaded.poll_interval_ms, 500);
        assert!(!loaded.sync_on_wifi_only);
    }

    #[test]
    fn config_clamps_poll_interval_below_minimum() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "poll_interval_ms = 50\nconfig_version = 1\n").unwrap();
        let cfg = AppConfig::load(&path).unwrap();
        assert_eq!(cfg.poll_interval_ms, 100);
    }

    #[test]
    fn unknown_config_keys_are_ignored() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "config_version = 1\nunknown_future_key = true\n").unwrap();
        AppConfig::load(&path).unwrap();
    }
}
