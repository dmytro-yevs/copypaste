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
    /// Local auto-wipe TTL for sensitive items (seconds). Default: 30.
    pub sensitive_ttl_secs: u64,
    pub image_quality: u8,
    pub sqlite_cache_mb: u32,
    pub encryption_chunk_kb: u32,
    pub sync_on_wifi_only: bool,
    pub max_bandwidth_kbps: u32,
    pub max_decoded_image_mb: u32,
    /// Bundle IDs of apps whose clipboard copies are silently skipped (macOS).
    /// Empty by default — no apps are excluded.  Example:
    /// `["com.1password.1password", "com.agilebits.onepassword"]`
    #[serde(default)]
    pub excluded_app_bundle_ids: Vec<String>,
    /// When `true`, paste-back writes only `public.utf8-plain-text`, stripping
    /// all rich types (RTF, HTML, attributed strings).  Default: `false`.
    #[serde(default)]
    pub paste_as_plain_text: bool,
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
            sensitive_ttl_secs: SENSITIVE_TTL_SECS,
            image_quality: IMAGE_QUALITY,
            sqlite_cache_mb: SQLITE_CACHE_MB,
            encryption_chunk_kb: ENCRYPTION_CHUNK_KB,
            sync_on_wifi_only: false,
            max_bandwidth_kbps: MAX_BANDWIDTH_KBPS,
            max_decoded_image_mb: MAX_DECODED_IMAGE_MB,
            excluded_app_bundle_ids: Vec::new(),
            paste_as_plain_text: false,
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

        // Write to a sibling temp file then atomically rename over the target.
        // A crash mid-write can therefore only leave the temp file behind; the
        // previous (valid) config is never truncated. The temp file lives in
        // the same directory so the rename stays within one filesystem.
        let dir = path.parent().unwrap_or_else(|| Path::new("."));
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "config.toml".to_owned());
        let tmp_path = dir.join(format!(".{file_name}.tmp"));

        std::fs::write(&tmp_path, text.as_bytes())?;
        // rename is atomic on the same filesystem; on failure clean up the temp.
        if let Err(e) = std::fs::rename(&tmp_path, path) {
            let _ = std::fs::remove_file(&tmp_path);
            return Err(e.into());
        }
        Ok(())
    }

    fn clamp_values(&mut self) {
        self.poll_interval_ms = self
            .poll_interval_ms
            .clamp(POLL_INTERVAL_MIN_MS, POLL_INTERVAL_MAX_MS);
        self.image_quality = self.image_quality.clamp(1, 100);
        self.encryption_chunk_kb = self.encryption_chunk_kb.clamp(16, 4096);

        // Fix 7: floor values that must never be 0 to prevent wipe-all / divide-by-zero.
        // history_limit = 0 would silently return no history rows from every page query.
        self.history_limit = self.history_limit.max(1);
        // Size limits of 0 would accept nothing (max_text/image/file) or keep nothing (quota).
        self.max_text_size_bytes = self.max_text_size_bytes.max(1);
        self.max_image_size_bytes = self.max_image_size_bytes.max(1);
        self.max_file_size_bytes = self.max_file_size_bytes.max(1);
        self.storage_quota_bytes = self.storage_quota_bytes.max(1);
        // max_decoded_image_mb = 0 would produce a 0-byte image decode limit (reject all images).
        self.max_decoded_image_mb = self.max_decoded_image_mb.max(1);
        // sensitive_ttl_secs = 0 would wipe all sensitive items immediately on every cleanup tick.
        self.sensitive_ttl_secs = self.sensitive_ttl_secs.max(1);
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
        assert_eq!(loaded.history_limit, HISTORY_LIMIT);
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
    fn save_writes_valid_parseable_file_atomically() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let cfg = AppConfig::default();
        cfg.save(&path).unwrap();

        // Final file exists and parses; no temp file is left behind.
        assert!(path.exists());
        let reparsed = AppConfig::load(&path).unwrap();
        assert_eq!(reparsed.history_limit, cfg.history_limit);
        let tmp = dir.path().join(".config.toml.tmp");
        assert!(!tmp.exists(), "temp file should be renamed away");
    }

    #[test]
    fn save_overwrites_existing_config_in_place() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");

        let first = AppConfig {
            history_limit: 111,
            ..Default::default()
        };
        first.save(&path).unwrap();
        assert_eq!(AppConfig::load(&path).unwrap().history_limit, 111);

        let second = AppConfig {
            history_limit: 222,
            ..Default::default()
        };
        second.save(&path).unwrap();
        assert_eq!(AppConfig::load(&path).unwrap().history_limit, 222);
    }

    #[test]
    fn failed_save_leaves_prior_config_intact() {
        // Simulate an interrupted write: point `save` at a path whose parent is
        // not a directory. The rename (and the temp write) cannot succeed, so
        // the previously written, valid config must remain untouched.
        let dir = tempdir().unwrap();
        let good_path = dir.path().join("config.toml");
        let original = AppConfig {
            history_limit: 777,
            ..Default::default()
        };
        original.save(&good_path).unwrap();

        // A path *inside* a regular file — its "parent" is not a directory, so
        // writing the sibling temp file fails before any rename happens.
        let bogus_path = good_path.join("nested").join("config.toml");
        let doomed = AppConfig {
            history_limit: 999,
            ..Default::default()
        };
        assert!(doomed.save(&bogus_path).is_err());

        // Prior config is byte-for-byte valid and unchanged.
        assert_eq!(AppConfig::load(&good_path).unwrap().history_limit, 777);
    }

    #[test]
    fn unknown_config_keys_are_ignored() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, "config_version = 1\nunknown_future_key = true\n").unwrap();
        AppConfig::load(&path).unwrap();
    }
}
