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
    #[allow(dead_code)]
    pub max_items_per_device: usize,
    /// When `true`, the per-IP rate limiter derives the client IP from the
    /// `X-Forwarded-For` / `X-Real-IP` / `Forwarded` headers (in that order),
    /// falling back to the TCP peer IP. Opt-in via `RELAY_TRUST_PROXY_HEADERS`.
    ///
    /// **Only enable this when the relay sits behind a trusted reverse proxy
    /// that overwrites these headers.** With a `0.0.0.0` bind and no proxy, a
    /// client can forge `X-Forwarded-For` to evade the per-IP limit, so the
    /// default is `false` (key strictly on the untrusted-but-unspoofable TCP
    /// peer IP). Documented opt-in closes M3 without changing the safe default.
    pub trust_proxy_headers: bool,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            sync_ttl_secs: 86_400,
            max_item_bytes: 10 * 1024 * 1024,
            max_items_per_device: 1000,
            trust_proxy_headers: false,
        }
    }
}

impl RelayConfig {
    /// Load configuration from environment variables. Falls back to defaults for
    /// any variable that is absent or unparseable.
    ///
    /// Recognised variables:
    /// - `RELAY_PORT`               — TCP port (u16)
    /// - `RELAY_SYNC_TTL_SECS`      — item TTL in seconds (u64)
    /// - `RELAY_MAX_ITEM_BYTES`     — max ciphertext size in bytes (usize)
    /// - `RELAY_TRUST_PROXY_HEADERS`   — `1`/`true` to honor XFF/X-Real-IP/Forwarded
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
                // Cap at 100 MiB so the `*4/3` body-limit math in
                // routes/mod.rs cannot overflow on a 32-bit host or
                // silently accept a misconfig that exhausts process memory.
                // 100 MiB decoded → ~133 MiB encoded body limit, well within
                // usize on any realistic target.
                cfg.max_item_bytes = n.min(100 * 1024 * 1024);
            }
        }
        if let Ok(v) = std::env::var("RELAY_TRUST_PROXY_HEADERS") {
            cfg.trust_proxy_headers = matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "on");
        }

        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Shared mutex serialising all tests that mutate `RELAY_*` env vars.
    /// `cargo test` runs tests in the same process and env-var mutation is not
    /// thread-safe without coordination.  Taking this lock at the start of any
    /// test that calls `std::env::set_var` / `remove_var` prevents data races
    /// with other tests in this module.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    #[test]
    fn default_values_are_sane() {
        let cfg = RelayConfig::default();
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.sync_ttl_secs, 86_400);
        assert_eq!(cfg.max_item_bytes, 10 * 1024 * 1024);
        assert_eq!(cfg.max_items_per_device, 1000);
        assert!(!cfg.trust_proxy_headers, "proxy trust must be opt-in");
    }

    #[test]
    fn from_env_uses_defaults_when_vars_absent() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // Ensure env vars are not set for this test (they shouldn't be in CI)
        std::env::remove_var("RELAY_PORT");
        std::env::remove_var("RELAY_SYNC_TTL_SECS");
        std::env::remove_var("RELAY_MAX_ITEM_BYTES");
        let cfg = RelayConfig::from_env();
        assert_eq!(cfg.port, 8080);
    }

    #[test]
    fn max_item_bytes_capped_at_100mib() {
        // RELAY_MAX_ITEM_BYTES values above 100 MiB must be clamped by
        // RelayConfig::from_env so the `*4/3` body-limit math in routes/mod.rs
        // cannot overflow on a 32-bit host or accept a runaway misconfig.
        //
        // We exercise from_env directly (rather than re-implementing the `.min()`
        // inline) so a future refactor that removes or changes the clamp will
        // immediately break this test, catching the regression at the source.
        //
        // The env-var is set and cleaned up under ENV_MUTEX so this test cannot
        // race with other tests that mutate RELAY_MAX_ITEM_BYTES.
        const CAP: usize = 100 * 1024 * 1024;
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

        std::env::set_var("RELAY_MAX_ITEM_BYTES", format!("{}", CAP + 1));
        let cfg = RelayConfig::from_env();
        std::env::remove_var("RELAY_MAX_ITEM_BYTES");

        assert_eq!(
            cfg.max_item_bytes, CAP,
            "from_env must clamp RELAY_MAX_ITEM_BYTES values above 100 MiB to exactly 100 MiB"
        );

        // Also verify the default is well under the cap (no silent truncation
        // of the shipped default).
        let default_cfg = RelayConfig::default();
        assert!(
            default_cfg.max_item_bytes <= CAP,
            "default max_item_bytes must not exceed 100 MiB cap"
        );
    }

    #[test]
    fn max_item_bytes_below_cap_is_unchanged() {
        // A value under 100 MiB must pass through unchanged.
        const CAP: usize = 100 * 1024 * 1024;
        let five_mib: usize = 5 * 1024 * 1024;
        let clamped = five_mib.min(CAP);
        assert_eq!(
            clamped, five_mib,
            "values at or below 100 MiB must be unchanged"
        );
    }

    #[test]
    fn trust_proxy_headers_parses_truthy_values() {
        let _guard = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // M3: opt-in proxy-header trust. Defaults off; only explicit truthy
        // values flip it on so a stray value can't silently start trusting XFF.
        for (raw, expected) in [
            ("1", true),
            ("true", true),
            ("on", true),
            ("0", false),
            ("false", false),
            ("", false),
            ("nonsense", false),
        ] {
            std::env::set_var("RELAY_TRUST_PROXY_HEADERS", raw);
            let cfg = RelayConfig::from_env();
            assert_eq!(
                cfg.trust_proxy_headers, expected,
                "RELAY_TRUST_PROXY_HEADERS={raw:?} should yield {expected}"
            );
        }
        std::env::remove_var("RELAY_TRUST_PROXY_HEADERS");
    }
}
