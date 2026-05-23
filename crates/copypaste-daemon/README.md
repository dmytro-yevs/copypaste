# copypaste-daemon

## Purpose
Background process that watches the system clipboard, persists items to the encrypted store, exposes a Unix-socket IPC API, and orchestrates P2P / cloud sync.

## Public API
Hybrid `bin` + `lib` crate. Binary entry point is `src/main.rs`; library surface (from `src/lib.rs`) lets integration tests reach internal modules:

- `clipboard` — system clipboard watcher (per-platform under `platform/`).
- `daemon` — main event loop / supervisor.
- `ipc` (unix) — JSON-over-Unix-socket request dispatch (~89 KB).
- `keychain` — OS keychain integration for the DB key.
- `launchd` — macOS LaunchAgent install/uninstall.
- `logging` — file + stdout tracing wiring.
- `p2p` — P2P listener + outbound connector built on `copypaste-p2p`.
- `paths` — per-platform `data_dir` / `log_dir` / `app_support_dir`.
- `peers` — paired-device registry.
- `platform` — per-OS shims.
- `protocol` — legacy IPC types (being migrated to `copypaste-ipc`).
- `sync_orch` — sync orchestrator (drives `copypaste-sync` sessions).
- `cloud` (feature `cloud-sync`) — Supabase Realtime sync.
- `tray` (macOS only) — menu-bar tray UI integration.

## Platform support
- **macOS**: full (tray, LaunchAgent, keychain).
- **Linux**: daemon + IPC + P2P; no tray.
- **Windows**: separate `ipc_win.rs` path; partial.
- **Android**: not applicable (use `copypaste-android` directly).

## Status
beta.

## Internal vs published
Internal binary crate. Not published to crates.io.

## Quick example
Run the daemon in the foreground:

```bash
cargo run -p copypaste-daemon
```

Inspect via the CLI (separate crate):

```bash
cargo run -p copypaste-cli -- status --json
```

## Tests
10 integration tests under `tests/` covering clipboard, health, IPC, keychain (macOS), launchd install, lifecycle, P2P init, pairing E2E, resilience, sync orchestrator.

```bash
cargo test -p copypaste-daemon
```

## Related ADRs
- [ADR-002](../../docs/adr/ADR-002-unix-socket-ipc.md) — Unix-socket IPC.
- [ADR-007](../../docs/adr/ADR-007-ipc-protocol-versioning.md) — IPC protocol versioning.
- [ADR-010](../../docs/adr/ADR-010-codesigning-ad-hoc.md) — Ad-hoc codesigning.
