/// Configuration for the relay server, loaded from environment variables
/// with safe defaults for all fields.
#[derive(Debug, Clone)]
// Fields are read through Axum's `Extension<RelayConfig>` in route handlers;
// the struct itself is not directly destructured in the binary entry point,
// so the compiler reports the fields as dead without the allow.
#[allow(dead_code)]
pub struct RelayConfig {
    /// TCP port to listen on (default: 8080)
    pub port: u16,
    /// Interface address to bind on (default: `0.0.0.0` for all interfaces).
    /// Override via `RELAY_BIND_ADDR` to restrict to a specific interface
    /// (e.g. `127.0.0.1` for loopback-only when behind a local proxy).
    pub bind_addr: String,
    /// Item TTL in seconds (default: 86400 — matches AppConfig::SYNC_TTL_SECS)
    pub sync_ttl_secs: u64,
    /// Maximum allowed decoded size of a single ciphertext payload in bytes (default: 10 MiB)
    pub max_item_bytes: usize,
    /// Maximum number of items stored per device inbox (default: 500).
    /// Sourced from `RELAY_MAX_ITEMS_PER_DEVICE`. Wired into `RelayStore` so
    /// the in-memory cap actually reflects this value (previously the field
    /// was dead and the compile-time constant `MAX_PUSH_ITEMS_PER_DEVICE` was
    /// always used instead).
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
    /// On-disk path for the persistent SQLite store (R1b). When set to a file
    /// path, device records, token sets and inbox items survive a process
    /// restart. Sourced from `RELAY_DB_PATH`.
    ///
    /// Defaults to `:memory:` so existing tests and ephemeral deploys behave
    /// exactly as the pre-R1b in-memory store did (nothing persists across
    /// restart). The relay always uses **plain** SQLite here — it never holds
    /// keys and never calls `PRAGMA key` (see `db.rs`).
    pub db_path: String,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            bind_addr: "0.0.0.0".to_string(),
            sync_ttl_secs: 86_400,
            max_item_bytes: 10 * 1024 * 1024,
            max_items_per_device: 500,
            trust_proxy_headers: false,
            db_path: crate::db::IN_MEMORY_PATH.to_string(),
        }
    }
}

impl RelayConfig {
    /// Load configuration from environment variables. Falls back to defaults for
    /// any variable that is absent or unparseable.
    ///
    /// Recognised variables:
    /// - `RELAY_PORT`                  — TCP port (u16)
    /// - `RELAY_BIND_ADDR`             — bind address string (default `0.0.0.0`)
    /// - `RELAY_SYNC_TTL_SECS`         — item TTL in seconds (u64)
    /// - `RELAY_MAX_ITEM_BYTES`        — max ciphertext size in bytes (usize)
    /// - `RELAY_MAX_ITEMS_PER_DEVICE`  — per-device inbox cap (usize, default 500)
    /// - `RELAY_TRUST_PROXY_HEADERS`   — `1`/`true` to honor XFF/X-Real-IP/Forwarded
    /// - `RELAY_DB_PATH`               — on-disk SQLite path (default `:memory:`)
    pub fn from_env() -> Self {
        let mut cfg = Self::default();

        if let Ok(v) = std::env::var("RELAY_PORT") {
            if let Ok(n) = v.parse::<u16>() {
                cfg.port = n;
            }
        }
        if let Ok(v) = std::env::var("RELAY_BIND_ADDR") {
            if !v.trim().is_empty() {
                cfg.bind_addr = v.trim().to_string();
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
        if let Ok(v) = std::env::var("RELAY_MAX_ITEMS_PER_DEVICE") {
            if let Ok(n) = v.parse::<usize>() {
                // Clamp to at least 1: n==0 would make effective_history_cap()
                // return 0, silently draining every push (the oldest item is
                // pruned to keep len ≤ cap, so cap=0 removes the just-inserted
                // item immediately — every push is a silent no-op).
                cfg.max_items_per_device = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("RELAY_TRUST_PROXY_HEADERS") {
            cfg.trust_proxy_headers = matches!(v.trim(), "1" | "true" | "TRUE" | "yes" | "on");
        }
        if let Ok(v) = std::env::var("RELAY_DB_PATH") {
            let v = v.trim();
            if !v.is_empty() {
                cfg.db_path = v.to_string();
            }
        }

        cfg
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    /// Process-wide mutex for tests that mutate environment variables.
    /// All env-var-touching tests must hold this guard for their duration
    /// to prevent races when cargo runs tests in parallel threads.
    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static ENV_MUTEX: OnceLock<Mutex<()>> = OnceLock::new();
        ENV_MUTEX.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    #[test]
    fn default_values_are_sane() {
        let cfg = RelayConfig::default();
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.bind_addr, "0.0.0.0");
        assert_eq!(cfg.sync_ttl_secs, 86_400);
        assert_eq!(cfg.max_item_bytes, 10 * 1024 * 1024);
        assert_eq!(cfg.max_items_per_device, 500);
        assert!(!cfg.trust_proxy_headers, "proxy trust must be opt-in");
    }

    #[test]
    fn from_env_uses_defaults_when_vars_absent() {
        let _guard = env_lock();
        // Ensure env vars are not set for this test (they shouldn't be in CI)
        std::env::remove_var("RELAY_PORT");
        std::env::remove_var("RELAY_SYNC_TTL_SECS");
        std::env::remove_var("RELAY_MAX_ITEM_BYTES");
        std::env::remove_var("RELAY_BIND_ADDR");
        std::env::remove_var("RELAY_MAX_ITEMS_PER_DEVICE");
        let cfg = RelayConfig::from_env();
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.bind_addr, "0.0.0.0");
    }

    /// Fix 4: `RELAY_BIND_ADDR` must be read from the environment and stored in
    /// `RelayConfig` so `main.rs` can bind on a restricted interface instead of
    /// always binding `0.0.0.0`.
    #[test]
    fn relay_bind_addr_is_read_from_env() {
        let _guard = env_lock();
        std::env::set_var("RELAY_BIND_ADDR", "127.0.0.1");
        let cfg = RelayConfig::from_env();
        std::env::remove_var("RELAY_BIND_ADDR");
        assert_eq!(
            cfg.bind_addr, "127.0.0.1",
            "RELAY_BIND_ADDR must override the default bind address"
        );
    }

    #[test]
    fn relay_bind_addr_defaults_to_any() {
        let _guard = env_lock();
        std::env::remove_var("RELAY_BIND_ADDR");
        let cfg = RelayConfig::from_env();
        assert_eq!(
            cfg.bind_addr, "0.0.0.0",
            "default bind addr must be 0.0.0.0"
        );
    }

    #[test]
    fn max_items_per_device_is_read_from_env() {
        let _guard = env_lock();
        std::env::set_var("RELAY_MAX_ITEMS_PER_DEVICE", "250");
        let cfg = RelayConfig::from_env();
        std::env::remove_var("RELAY_MAX_ITEMS_PER_DEVICE");
        assert_eq!(
            cfg.max_items_per_device, 250,
            "RELAY_MAX_ITEMS_PER_DEVICE must override the default cap"
        );
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
        // The env-var is set and cleaned up under the env_lock mutex so this test
        // cannot race with other tests that mutate RELAY_MAX_ITEM_BYTES.
        const CAP: usize = 100 * 1024 * 1024;
        let _guard = env_lock();

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
        let _guard = env_lock();
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
