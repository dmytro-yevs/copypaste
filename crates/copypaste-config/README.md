# copypaste-config

## Purpose
Unified runtime configuration (paths, ports, log level) shared by the daemon, CLI, relay, and UI binaries. Intentionally separate from `copypaste_core::config::AppConfig`, which owns user-facing tunables (history limits, TTLs, …).

## Public API
From `src/lib.rs`:

- `AppConfig` — `data_dir`, `socket_path`, `log_level`, `db_key_path`, `relay_port`, `mdns_service`. All fields concrete (no `Option`s).
- `AppConfig::defaults()` / `AppConfig::with_data_dir(path)` — in-memory baseline.
- `AppConfig::load()` — defaults → `data_dir/config.json` → env overrides.
- `AppConfig::save()` — pretty-JSON write into `data_dir`.
- `AppConfig::apply_env_overrides()` — re-apply `COPYPASTE_*` env vars.
- `ConfigError` — error enum.
- Constants: `CONFIG_FILE_NAME`, `DEFAULT_RELAY_PORT = 7777`, `DEFAULT_MDNS_SERVICE = "_copypaste._tcp.local."`, `DEFAULT_LOG_LEVEL = "info"`, `DEFAULT_SOCKET_FILE = "daemon.sock"`, `DEFAULT_DB_KEY_FILE = "db.key"`.

Env overrides honoured: `COPYPASTE_DATA_DIR`, `COPYPASTE_SOCKET_PATH`, `COPYPASTE_LOG_LEVEL`, `COPYPASTE_DB_KEY_PATH`, `COPYPASTE_RELAY_PORT`, `COPYPASTE_MDNS_SERVICE`.

Lint discipline: `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, `#![deny(rust_2018_idioms)]`.

## Platform support
All platforms; per-platform `data_dir` resolution via `directories::ProjectDirs`.

## Status
beta. Ships standalone — consumer crates (daemon, cli, relay, ui) are wired in a follow-up wave.

## Internal vs published
Internal workspace crate. Not published to crates.io.

## Quick example

```rust,no_run
use copypaste_config::AppConfig;

let cfg = AppConfig::load()?;
println!("socket: {}", cfg.socket_path.display());
println!("relay port: {}", cfg.relay_port);
cfg.save()?;
# Ok::<_, copypaste_config::ConfigError>(())
```

## Tests
1 integration test under `tests/`: load/save round-trip, plus inline unit tests.

```bash
cargo test -p copypaste-config
```
