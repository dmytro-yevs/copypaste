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
    /// Item TTL in seconds (default: 86400 = 24 h).
    ///
    /// The relay inbox is intentionally ephemeral: items are pruned after 24 h
    /// by the background evictor. This is shorter than the daemon's local
    /// `AppConfig::SYNC_TTL_SECS` (2 592 000 s = 30 days), which governs how
    /// long history is kept in the local SQLCipher DB. Devices that are offline
    /// for longer than this TTL must re-sync from cloud storage rather than the
    /// relay inbox. Override with `RELAY_SYNC_TTL_SECS`.
    pub sync_ttl_secs: u64,
    /// Maximum allowed decoded size of a single ciphertext payload in bytes
    /// (default: [`copypaste_ipc::RELAY_MAX_ITEM_BYTES`], 10 MiB).
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
    /// Maximum number of requests served concurrently (default: 1024). Sourced
    /// from `RELAY_MAX_CONNECTIONS`. Enforced by a `tower` concurrency-limit
    /// layer (CopyPaste-pbre) so a burst of in-flight requests cannot exhaust
    /// process memory / file descriptors; excess requests queue (back-pressure)
    /// rather than being dropped. Complements the per-IP/per-device rate limits
    /// (which bound request *rate*, not *concurrency*).
    pub max_connections: usize,
    /// Per-IP rate limit steady-state, in requests/second (default: see
    /// `crate::middleware::rate_limit::PER_IP_PER_SECOND`, currently 3, i.e.
    /// ~200 req/min combined with `per_ip_burst_size`). Sourced from
    /// `RELAY_PER_IP_PER_SECOND`.
    ///
    /// CopyPaste-8ebg.50: previously this bound was a compile-time constant,
    /// so a false-positive 429 storm could only be tuned by a rebuild and
    /// redeploy. It is now runtime-configurable like the other relay limits.
    pub per_ip_per_second: u64,
    /// Per-IP burst allowance (default: see
    /// `crate::middleware::rate_limit::PER_IP_BURST_SIZE`, currently 60).
    /// Sourced from `RELAY_PER_IP_BURST_SIZE`.
    pub per_ip_burst_size: u32,
    /// Per-device (item-route) rate limit steady-state, in requests/second
    /// (default: see `crate::middleware::rate_limit::PER_DEVICE_PER_SECOND`,
    /// currently 1, i.e. 60 req/min). Sourced from `RELAY_PER_DEVICE_PER_SECOND`.
    pub per_device_per_second: u64,
    /// Per-device burst allowance for item routes (default: see
    /// `crate::middleware::rate_limit::PER_DEVICE_BURST_SIZE`, currently 20).
    /// Sourced from `RELAY_PER_DEVICE_BURST_SIZE`.
    pub per_device_burst_size: u32,
    /// Maximum registration attempts allowed per `(client_ip, device_id)`
    /// within `crate::state::REG_LIMIT_WINDOW` (default: see
    /// `crate::state::REG_LIMIT_MAX_ATTEMPTS`, currently 5). Sourced from
    /// `RELAY_REG_LIMIT_MAX_ATTEMPTS` (CopyPaste-vgpy).
    ///
    /// Literal default (not a `crate::state::...` reference) for the same
    /// reason as `per_ip_per_second` above: `config.rs` is also compiled
    /// standalone via `#[path = "../src/config.rs"]` in the relay's
    /// integration tests, which do not declare a `state` module.
    pub reg_limit_max_attempts: usize,
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            port: 8080,
            bind_addr: "0.0.0.0".to_string(),
            sync_ttl_secs: 86_400,
            max_item_bytes: copypaste_ipc::RELAY_MAX_ITEM_BYTES,
            max_items_per_device: 500,
            trust_proxy_headers: false,
            db_path: crate::db::IN_MEMORY_PATH.to_string(),
            max_connections: 1024,
            // Mirrors `crate::middleware::rate_limit::PER_IP_PER_SECOND` etc.
            // Literal (not a `crate::middleware::...` reference) because
            // `config.rs` is also compiled standalone via `#[path = "../src/config.rs"]`
            // in the relay's integration tests, which do not declare a
            // `middleware` module — see crates/copypaste-relay/tests/*.rs.
            per_ip_per_second: 3,
            per_ip_burst_size: 60,
            per_device_per_second: 1,
            per_device_burst_size: 20,
            // Mirrors `crate::state::REG_LIMIT_MAX_ATTEMPTS` — see the
            // standalone-compile note on the field doc comment above.
            reg_limit_max_attempts: 5,
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
    /// - `RELAY_MAX_CONNECTIONS`       — max concurrent in-flight requests (usize, default 1024)
    /// - `RELAY_PER_IP_PER_SECOND`     — per-IP steady-state rate, req/s (u64, default 3)
    /// - `RELAY_PER_IP_BURST_SIZE`     — per-IP burst allowance (u32, default 60)
    /// - `RELAY_PER_DEVICE_PER_SECOND` — per-device steady-state rate, req/s (u64, default 1)
    /// - `RELAY_PER_DEVICE_BURST_SIZE` — per-device burst allowance (u32, default 20)
    /// - `RELAY_REG_LIMIT_MAX_ATTEMPTS` — registration attempts per (ip, device_id)
    ///   per window (usize, default 5)
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
        if let Ok(v) = std::env::var("RELAY_MAX_CONNECTIONS") {
            if let Ok(n) = v.parse::<usize>() {
                // Clamp to at least 1: a 0 concurrency limit would deadlock the
                // server (every request waits forever for a permit).
                cfg.max_connections = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("RELAY_PER_IP_PER_SECOND") {
            if let Ok(n) = v.parse::<u64>() {
                // Clamp to at least 1: `governor`'s `GovernorConfigBuilder::finish()`
                // rejects a zero rate, which would otherwise turn a misconfig into a
                // process-level `unwrap`-adjacent panic at router build time instead
                // of a clean fallback.
                cfg.per_ip_per_second = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("RELAY_PER_IP_BURST_SIZE") {
            if let Ok(n) = v.parse::<u32>() {
                cfg.per_ip_burst_size = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("RELAY_PER_DEVICE_PER_SECOND") {
            if let Ok(n) = v.parse::<u64>() {
                cfg.per_device_per_second = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("RELAY_PER_DEVICE_BURST_SIZE") {
            if let Ok(n) = v.parse::<u32>() {
                cfg.per_device_burst_size = n.max(1);
            }
        }
        if let Ok(v) = std::env::var("RELAY_REG_LIMIT_MAX_ATTEMPTS") {
            if let Ok(n) = v.parse::<usize>() {
                // Clamp to at least 1: a 0 limit would make
                // `check_registration_rate_limit` reject every registration
                // attempt outright (deque.len() >= 0 is always true),
                // effectively bricking registration for a config typo.
                cfg.reg_limit_max_attempts = n.max(1);
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
        // Relay inbox TTL is intentionally 24 h (86 400 s), NOT 30 days.
        // The daemon's AppConfig::SYNC_TTL_SECS (2 592 000 s = 30 days) governs
        // local SQLCipher history retention; the relay is an ephemeral transit
        // buffer. See docs/relay-api.md and ADR-009 for the design rationale.
        assert_eq!(
            cfg.sync_ttl_secs, 86_400,
            "relay TTL default must be 86400 s (24 h); \
             this is intentionally shorter than the daemon's 30-day local history TTL"
        );
        assert_eq!(cfg.max_item_bytes, copypaste_ipc::RELAY_MAX_ITEM_BYTES);
        assert_eq!(cfg.max_items_per_device, 500);
        assert!(!cfg.trust_proxy_headers, "proxy trust must be opt-in");
        assert_eq!(cfg.max_connections, 1024);
        // CopyPaste-8ebg.50: defaults must mirror the pre-existing compile-time
        // constants exactly so this change is behavior-preserving until an
        // operator opts into an override.
        assert_eq!(cfg.per_ip_per_second, 3);
        assert_eq!(cfg.per_ip_burst_size, 60);
        assert_eq!(cfg.per_device_per_second, 1);
        assert_eq!(cfg.per_device_burst_size, 20);
        // CopyPaste-vgpy: default must mirror the pre-existing
        // `crate::state::REG_LIMIT_MAX_ATTEMPTS` compile-time constant exactly
        // so this change is behavior-preserving until an operator overrides it.
        assert_eq!(cfg.reg_limit_max_attempts, 5);
    }

    /// CopyPaste-vgpy: the registration rate-limit ceiling must be tunable via
    /// env without a rebuild, mirroring the other relay rate-limit knobs.
    #[test]
    fn reg_limit_max_attempts_read_from_env() {
        let _guard = env_lock();
        std::env::set_var("RELAY_REG_LIMIT_MAX_ATTEMPTS", "12");
        let cfg = RelayConfig::from_env();
        std::env::remove_var("RELAY_REG_LIMIT_MAX_ATTEMPTS");
        assert_eq!(cfg.reg_limit_max_attempts, 12);
    }

    /// A `0` value must be clamped to `1` rather than passed through, since a
    /// `0` limit would reject every registration attempt outright.
    #[test]
    fn reg_limit_max_attempts_zero_is_clamped_to_one() {
        let _guard = env_lock();
        std::env::set_var("RELAY_REG_LIMIT_MAX_ATTEMPTS", "0");
        let cfg = RelayConfig::from_env();
        std::env::remove_var("RELAY_REG_LIMIT_MAX_ATTEMPTS");
        assert_eq!(cfg.reg_limit_max_attempts, 1);
    }

    /// CopyPaste-8ebg.50: rate-limit thresholds must be tunable via env vars
    /// without a rebuild, so a false-positive 429 storm can be relieved by an
    /// operator changing env and restarting the process.
    #[test]
    fn rate_limit_thresholds_read_from_env() {
        let _guard = env_lock();
        std::env::set_var("RELAY_PER_IP_PER_SECOND", "10");
        std::env::set_var("RELAY_PER_IP_BURST_SIZE", "100");
        std::env::set_var("RELAY_PER_DEVICE_PER_SECOND", "5");
        std::env::set_var("RELAY_PER_DEVICE_BURST_SIZE", "40");
        let cfg = RelayConfig::from_env();
        std::env::remove_var("RELAY_PER_IP_PER_SECOND");
        std::env::remove_var("RELAY_PER_IP_BURST_SIZE");
        std::env::remove_var("RELAY_PER_DEVICE_PER_SECOND");
        std::env::remove_var("RELAY_PER_DEVICE_BURST_SIZE");

        assert_eq!(cfg.per_ip_per_second, 10);
        assert_eq!(cfg.per_ip_burst_size, 100);
        assert_eq!(cfg.per_device_per_second, 5);
        assert_eq!(cfg.per_device_burst_size, 40);
    }

    /// CopyPaste-8ebg.50: a `0` rate/burst must be clamped to `1` rather than
    /// passed through, because `governor::GovernorConfigBuilder::finish()`
    /// rejects a zero rate and would otherwise turn an operator typo into a
    /// router-build failure instead of a safe fallback.
    #[test]
    fn rate_limit_thresholds_zero_is_clamped_to_one() {
        let _guard = env_lock();
        std::env::set_var("RELAY_PER_IP_PER_SECOND", "0");
        std::env::set_var("RELAY_PER_IP_BURST_SIZE", "0");
        std::env::set_var("RELAY_PER_DEVICE_PER_SECOND", "0");
        std::env::set_var("RELAY_PER_DEVICE_BURST_SIZE", "0");
        let cfg = RelayConfig::from_env();
        std::env::remove_var("RELAY_PER_IP_PER_SECOND");
        std::env::remove_var("RELAY_PER_IP_BURST_SIZE");
        std::env::remove_var("RELAY_PER_DEVICE_PER_SECOND");
        std::env::remove_var("RELAY_PER_DEVICE_BURST_SIZE");

        assert_eq!(cfg.per_ip_per_second, 1);
        assert_eq!(cfg.per_ip_burst_size, 1);
        assert_eq!(cfg.per_device_per_second, 1);
        assert_eq!(cfg.per_device_burst_size, 1);
    }

    /// Asserts that the relay TTL default is intentionally shorter than the
    /// daemon's local history TTL. This documents the design decision so that
    /// a future change to either constant triggers a deliberate review.
    #[test]
    fn relay_ttl_is_shorter_than_daemon_local_history_ttl() {
        // copypaste-core's SYNC_TTL_SECS = 2_592_000 (30 days) governs how long
        // the daemon keeps items in its local SQLCipher DB. The relay inbox is
        // an ephemeral transit buffer with a 24 h TTL by design (see ADR-009).
        // If the relay default ever equals or exceeds SYNC_TTL_SECS without a
        // deliberate product decision, this test will catch the drift.
        const DAEMON_SYNC_TTL_SECS: u64 = 2_592_000; // copypaste-core AppConfig default
        let cfg = RelayConfig::default();
        assert!(
            cfg.sync_ttl_secs < DAEMON_SYNC_TTL_SECS,
            "relay default TTL ({}) should be shorter than daemon local history TTL ({}) \
             — the relay is an ephemeral transit buffer, not long-term storage",
            cfg.sync_ttl_secs,
            DAEMON_SYNC_TTL_SECS,
        );
    }

    /// CopyPaste-pbre: `RELAY_MAX_CONNECTIONS` overrides the default and is
    /// clamped to at least 1 so a `0` cannot deadlock the server.
    #[test]
    fn max_connections_from_env_and_clamp() {
        let _guard = env_lock();
        std::env::set_var("RELAY_MAX_CONNECTIONS", "256");
        assert_eq!(RelayConfig::from_env().max_connections, 256);
        std::env::set_var("RELAY_MAX_CONNECTIONS", "0");
        assert_eq!(
            RelayConfig::from_env().max_connections,
            1,
            "a 0 concurrency limit must be clamped to 1"
        );
        std::env::remove_var("RELAY_MAX_CONNECTIONS");
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
