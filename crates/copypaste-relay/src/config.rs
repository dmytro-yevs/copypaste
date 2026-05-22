/// Configuration for the relay server, loaded from environment variables
/// with safe defaults for all fields.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct RelayConfig {
    /// TCP port to listen on (default: 8080)
    pub port: u16,
    /// Item TTL in seconds (default: 86400 — matches AppConfig::SYNC_TTL_SECS)
    pub sync_ttl_secs: u64,
    /// Maximum allowed decoded size of a single ciphertext payload in bytes (default: 10 MiB)
    pub max_item_bytes: usize,
    /// Maximum number of items stored per device inbox (default: 1000)
    pub max_items_per_device: usize,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            sync_ttl_secs: 86_400,
            max_item_bytes: 10 * 1024 * 1024,
            max_items_per_device: 1000,
        }
    }
}

impl RelayConfig {
    /// Load configuration from environment variables. Falls back to defaults for
    /// any variable that is absent or unparseable.
    ///
    /// Recognised variables:
    /// - `RELAY_PORT`           — TCP port (u16)
    /// - `RELAY_SYNC_TTL_SECS`  — item TTL in seconds (u64)
    /// - `RELAY_MAX_ITEM_BYTES` — max ciphertext size in bytes (usize)
    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        if let Ok(v) = std::env::var("RELAY_PORT") {
            if let Ok(n) = v.parse::<u16>() {
                cfg.port = n;
            }
        }
        if let Ok(v) = std::env::var("RELAY_SYNC_TTL_SECS") {
            if let Ok(n) = v.parse::<u64>() {
                cfg.sync_ttl_secs = n;
            }
        }
        if let Ok(v) = std::env::var("RELAY_MAX_ITEM_BYTES") {
            if let Ok(n) = v.parse::<usize>() {
                cfg.max_item_bytes = n;
            }
        }

        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_values_are_sane() {
        let cfg = RelayConfig::default();
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.sync_ttl_secs, 86_400);
        assert_eq!(cfg.max_item_bytes, 10 * 1024 * 1024);
        assert_eq!(cfg.max_items_per_device, 1000);
    }

    #[test]
    fn from_env_uses_defaults_when_vars_absent() {
        // Ensure env vars are not set for this test (they shouldn't be in CI)
        std::env::remove_var("RELAY_PORT");
        std::env::remove_var("RELAY_SYNC_TTL_SECS");
        std::env::remove_var("RELAY_MAX_ITEM_BYTES");
        let cfg = RelayConfig::from_env();
        assert_eq!(cfg.port, 8080);
    }
}
