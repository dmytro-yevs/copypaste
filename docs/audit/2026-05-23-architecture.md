# Architecture Audit — CopyPaste v0.1.0-alpha.1
**Auditor:** system-architect
**Date:** 2026-05-23
**Commit:** 7a577f7f9906c3504b789b394383ac9ebf1588b1
**Branch:** release/v0.1.0-alpha
**Total findings:** 27 (Critical: 4, High: 7, Medium: 9, Low: 4, Info: 3)

---

## Crate Graph

Workspace members (9) and their `path = "../X"` dependencies (cargo tree, not Cargo.toml metadata):

```
copypaste-core ──┬── copypaste-cli      (declared in Cargo.toml, NOT used in src/)
                 ├── copypaste-daemon   (used)
                 ├── copypaste-sync     (used — but copypaste-sync is itself orphan)
                 └── copypaste-android  (used)

copypaste-ui          (depends ONLY on slint, serde, anyhow, home — does NOT import copypaste-core)
copypaste-relay       (depends only on axum/tokio/sqlite — does NOT import copypaste-core)
copypaste-p2p         (NO crate depends on it — ORPHAN)
copypaste-sync        (NO crate depends on it — ORPHAN)
copypaste-supabase    (NO crate depends on it — ORPHAN)
copypaste-android     (used by Android build only)
```

**Key observation:** 5 of 9 crates (`copypaste-p2p`, `copypaste-sync`, `copypaste-supabase`, `copypaste-relay`, `copypaste-android`) have zero in-workspace consumers. The shipping binary (`copypaste-daemon`) reimplements stubbed P2P (`daemon/src/p2p.rs`) and stubbed cloud-sync (`daemon/src/cloud.rs`) instead of consuming the dedicated crates that exist. This is a **split-implementation** problem inherited from parallel feature-branch merges that were never integrated.

---

## Findings (sorted by severity)

| # | Severity | Category | Crate / File:Line | Finding | Recommendation |
|---|----------|----------|-------------------|---------|----------------|
| 1 | CRITICAL | Crate Boundaries | crates/copypaste-p2p, crates/copypaste-sync, crates/copypaste-supabase | Three feature crates exist with full implementations (mTLS transport, Lamport clocks, GoTrue/Realtime client) but **zero workspace consumers**. Daemon's `p2p.rs` (156 lines, stubs) and `cloud.rs` (384 lines, REST-poll) duplicate ~10% of these crates with `TODO(intg-p2p-crates)` markers. Result: a P2P/cloud feature shipped in the alpha binary that is a TCP-accept-and-drop stub, while the real implementation sits unused. | Add `copypaste-p2p`, `copypaste-sync`, `copypaste-supabase` to `copypaste-daemon` dependencies and replace stub modules with real adapters before alpha tag. If real wiring is out of scope for alpha, remove the orphan crates from the workspace (or move them under `crates/_wip/`) and stop building them. |
| 2 | CRITICAL | Layering | crates/copypaste-ui/Cargo.toml + crates/copypaste-ui/src/ipc_client.rs:22-86 | `copypaste-ui` does **not** depend on `copypaste-core`. It re-implements `IpcResponse`, `HistoryEntry`, `HistoryPage`, `AppSettings`, `PairedDevice` as separate types and talks to the daemon via raw JSON. Schema drift between UI and daemon types is undetectable at compile time. | Extract a `copypaste-ipc` crate containing `Request`, `Response`, `HistoryPage`, and every method-specific param/result struct. Have daemon + CLI + UI all depend on it. Schema correctness becomes a `cargo check` problem instead of a runtime drift problem. |
| 3 | CRITICAL | Crate Boundaries | crates/copypaste-cli/Cargo.toml:3 | `copypaste-cli` declares `copypaste-core = { path = "../copypaste-core" }` but **no `.rs` file in `cli/src/` imports `copypaste_core`**. Dead dependency that links the whole core crate (crypto, rusqlite, SQLCipher) into the CLI binary, inflating size + compile time and exposing the CLI process to a database-key surface it should never touch. | Remove `copypaste-core` from `copypaste-cli/Cargo.toml`. CLI should be a pure IPC client. |
| 4 | CRITICAL | Storage | crates/copypaste-relay/src/db.rs (whole file) | `copypaste-relay` has a `db.rs` module that uses `rusqlite` but (a) `rusqlite` is **not in relay's Cargo.toml**, so this file does not compile, and (b) `main.rs` does not declare `mod db;` so it is **never built**. All relay state lives in `Arc<Mutex<RelayStore>>` HashMaps and is lost on restart. The relay therefore cannot persist devices, tokens, or sync items across deployments. | Either delete `db.rs` (relay is intentionally in-memory for alpha) and document it in CHANGELOG, or add `rusqlite` + `mod db;` and wire `db::open(path)` into `RelayStore::new`. The current state is the worst of both — code looks persistent but isn't. |
| 5 | HIGH | State Management | crates/copypaste-core/src/storage/db.rs:16 + crates/copypaste-daemon/src/daemon.rs:38 | The entire daemon serialises every clipboard read, every IPC request, every TTL cleanup, every cloud push, and every P2P broadcast through a single `Arc<tokio::sync::Mutex<Database>>`. 16 distinct `db.lock().await` call sites compete for the same critical section. With SQLite WAL enabled there is no technical need for an outer Mutex — multiple read transactions can proceed in parallel. | Replace `Arc<Mutex<Database>>` with a connection pool (e.g. `r2d2_sqlite` or hand-rolled `deadpool`) and wrap blocking rusqlite calls in `tokio::task::spawn_blocking`. At minimum, split read paths (`get_page`, `count_items`, `search_items`) from write paths and use `RwLock`. |
| 6 | HIGH | Concurrency | crates/copypaste-core/src/storage/items.rs (all fns) + crates/copypaste-daemon/src/ipc.rs:174-313 | All rusqlite operations are **synchronous** but called from inside `async fn` handlers without `spawn_blocking`. While a `db.lock().await` is held, the tokio worker thread is blocked on disk I/O. Under load this starves the IPC listener and the clipboard ticker. `grep spawn_blocking` returns 0 matches across `daemon/` and `core/`. | Wrap every DB call in `tokio::task::spawn_blocking(move \|\| ...)`. Provides true async semantics and lets the runtime keep one worker free for the tray quit signal even when the DB is busy. |
| 7 | HIGH | Layering | crates/copypaste-core/src/config/mod.rs:20 + crates/copypaste-daemon/src/ipc.rs:13 | Two distinct `AppConfig` structs exist with **non-overlapping fields and different file formats**: `core::config::AppConfig` (TOML, `history_limit`, TTLs, image quality) vs `daemon::ipc::AppConfig` (JSON, `p2p_enabled`, `supabase_url`, `supabase_anon_key`). Both are written to disk by the daemon at different paths. Users editing config get unpredictable behaviour depending on which file they touch. | Merge into a single `AppConfig` in `copypaste-core` with all fields. Move the JSON variant's fields into the TOML schema. One file, one source of truth. |
| 8 | HIGH | IPC Protocol | crates/copypaste-daemon/src/protocol.rs:3-19 | `Request`/`Response` have **no version field**. The daemon advertises `"version": "1"` as a string inside the `stats` response payload only — there is no negotiation, no minimum-supported-version check, no `Unsupported` error response. When the alpha shipping CLI/UI is paired with a beta daemon (or vice versa) the client silently sees `ok: false, error: "unknown method"`. | Add a top-level `protocol_version: u32` to `Request`. Daemon rejects mismatched majors with a typed error. Bump on every breaking change. Document in `docs/adr/`. |
| 9 | HIGH | Platform Abstraction | crates/copypaste-daemon/src/platform/{mod,macos,windows,linux}.rs (whole module) + crates/copypaste-daemon/src/clipboard.rs:56 | `platform::ClipboardBackend` + `KeystoreBackend` traits are defined, `MacosClipboardBackend`/`WindowsClipboardBackend`/`LinuxClipboardBackend` impls are written — and **nothing uses them**. `ClipboardMonitor::poll()` instead has an inline `#[cfg(target_os = "macos")]` block calling `NSPasteboard` directly, with `#[cfg(not(target_os = "macos"))] Ok(None)` fallback. The trait abstraction is dead code. | Either delete `platform/` (and document macOS-only intent), or actually wire `ClipboardMonitor` to take `Box<dyn ClipboardBackend>` so Phase 5b can swap in Linux/Windows. Current state misleads contributors. |
| 10 | HIGH | Platform | crates/copypaste-daemon/src/platform/linux.rs (whole file) + crates/copypaste-daemon/src/platform/windows.rs (whole file) | Per `MEMORY.md` Linux is FROZEN (Phase 5b) and Windows is not on alpha. But both `linux.rs` and `windows.rs` ship with `unimplemented!()` panics in trait impls. If any future code accidentally constructs `LinuxClipboardBackend` (no `#[cfg]` gate on the structs themselves), the daemon panics. | Gate the whole linux.rs / windows.rs modules behind a `phase5b` feature flag, OR delete the stubs until that phase begins. Compiled-in `unimplemented!()` is a runtime panic time-bomb. |
| 11 | HIGH | Dependencies | Cargo.lock (workspace-wide) | `cargo tree --duplicates` reports **80 duplicate crates**, including two TLS stacks (`rustls 0.21` from supabase's `reqwest 0.11` and `rustls 0.23` from `copypaste-p2p`), two `hyper` major versions (`0.14` + `1.9`), two `http` versions (`0.2` + `1.4`), two `tokio-rustls`, two `thiserror`, two `image` crates, two `objc2`. Doubles binary size and ABI surface area; the dual `rustls` means two cert stores and two crypto-provider initialisations. | Upgrade `copypaste-supabase` to `reqwest 0.12` (already used by daemon's cloud-sync) — collapses `rustls 0.21 → 0.23`, `hyper 0.14 → 1`, `http 0.2 → 1`. Pin `thiserror = "1"` across workspace until a coordinated upgrade. |
| 12 | MEDIUM | Crate Boundaries | crates/copypaste-relay/Cargo.toml | Relay does not depend on `copypaste-core` despite needing the same `ClipboardItem` wire format that `copypaste-sync::WireItem` defines. Relay's `state::SyncItem` is a third independent definition of "a sync item on the wire". | Have relay depend on `copypaste-sync` (or extract a `copypaste-wire` crate) for `WireItem`. Eliminates serialization drift between client and server. |
| 13 | MEDIUM | Feature Flags | crates/copypaste-daemon/Cargo.toml:[features] | `cloud-sync` is opt-in (`default = []`), but the cloud module's runtime check (`CloudConfig::from_env`) returns `None` silently if env vars are missing. Users who build with `--features cloud-sync` but forget the env vars get no warning. Symmetrically, users who build without the feature and set `SUPABASE_URL` get silently no cloud. | Log a startup warning when `cloud-sync` feature is compiled but env vars are absent (already done in daemon.rs:123 — verify), and emit a "feature not compiled" log when env vars are set without the feature. |
| 14 | MEDIUM | Observability | (workspace) | `tracing::instrument` is used on 6 functions only (daemon-side). No metrics export (Prometheus/OTLP), no health endpoint on the daemon (only on relay), no cross-crate span linking. `tracing_subscriber` uses `EnvFilter` with default `copypaste=info,warn` (logging.rs:258) — fine for dev, but no JSON layer for production log shipping. | Phase 1: add `tracing_subscriber::fmt::json` layer behind `COPYPASTE_LOG_JSON=1`. Phase 2: add `metrics-exporter-prometheus` to daemon, expose `/metrics` on the IPC socket or a side HTTP port. |
| 15 | MEDIUM | Storage | crates/copypaste-core/src/storage/schema.rs:10-41 | Migration system is **forward-only by user_version comparison**. There is no down-migration, no migration metadata table, no transactional bracketing (each `ALTER TABLE` runs outside an explicit transaction). If `apply_migrations` is interrupted mid-batch, the user_version stays at the old value but the schema is partially updated — next startup will try to re-apply and fail. | Wrap migration block in a single `conn.execute_batch("BEGIN; ...; PRAGMA user_version=N; COMMIT;")`. For beta, introduce a `schema_migrations(version, applied_at)` table. |
| 16 | MEDIUM | Storage | crates/copypaste-core/src/storage/db.rs | SQLCipher key is loaded once at daemon start from Keychain and stored on the `Database` struct. **No key-rotation API exists.** If the device's keypair is ever rotated (compromised, lost, manual reset), the entire DB must be re-keyed via `PRAGMA rekey` — there is no code path for this. | Add `Database::rekey(&mut self, new_key: &[u8; 32]) -> Result<()>` that issues `PRAGMA rekey = "x'...'"`. Document key-rotation policy in ADR-003 follow-up. |
| 17 | MEDIUM | State Management | crates/copypaste-relay/src/state.rs:322 | `pub type AppState = Arc<Mutex<RelayStore>>;` — every relay request takes a write lock on the entire store, even read-only `GET /devices`. With 1000+ devices this is the obvious bottleneck. | Use `Arc<RwLock<RelayStore>>` and tag read methods (`get_device`, `pull_items`, `list_devices`, `stats`) so handlers can take `.read()`. For beta, move to per-device sharding or dashmap. |
| 18 | MEDIUM | IPC Protocol | crates/copypaste-daemon/src/protocol.rs:11-19 + crates/copypaste-daemon/src/ipc.rs (whole file) | Error responses use a free-form `error: Option<String>`. There is no error code enum, no machine-readable category (e.g. `not_found`, `auth_failed`, `invalid_argument`). Clients have to string-match (`"not found"`, `"missing param: id"`, `"unknown method: foo"`) which breaks on any rephrase. | Introduce `error_code: Option<&'static str>` alongside `error: Option<String>`. Document in `docs/protocol.md`. |
| 19 | MEDIUM | Configuration | crates/copypaste-daemon/src/{paths,daemon}.rs + crates/copypaste-cli/src/paths.rs + crates/copypaste-ui/src/main.rs:33-35 | Three independent path resolvers. `daemon/paths.rs` handles all three OSes via `#[cfg]`. `cli/paths.rs` is **hard-coded macOS only** (`Library/Application Support`). `ui/main.rs:33` also hard-codes macOS. CLI + UI literally cannot run on Linux/Windows even if the daemon could. | Move all path resolution into a `copypaste-paths` crate (or extend `copypaste-core::config`). Single `pub fn socket_path() -> PathBuf` consumed everywhere. |
| 20 | MEDIUM | Configuration | (workspace) | No hot-reload of config. `AppConfig` is read once at `daemon::run` start; changes require daemon restart. For alpha this is acceptable but should be ADR'd. | Document "restart required for config changes" in README. Optionally add an `IPC reload_config` method for beta. |
| 21 | LOW | Crate Boundaries | crates/copypaste-supabase/src/lib.rs:38-53 | Public re-exports include `protocol::PhoenixMessage`, `protocol::PhoenixEvent` — Phoenix Channel wire types are implementation details of the WebSocket transport and should not be in the crate's public API. | Mark `protocol` module as `pub(crate)`; expose only `ChangeEvent`/`ChangeType` publicly. |
| 22 | LOW | Concurrency | crates/copypaste-daemon/src/cloud.rs:74-83 | `start_cloud` creates a `oneshot::Sender<()>` for shutdown then wraps it in an `Arc<Notify>` via a spawned task. This is a workaround for not having `mpsc<()>(2)` or a `watch::channel`. Adds two task spawns for a signal that fires once. | Replace with `tokio::sync::watch::channel(false)`; both push_loop and realtime_loop poll the receiver. Single channel, no extra task. |
| 23 | LOW | Crate Boundaries | crates/copypaste-daemon/src/peers.rs + crates/copypaste-daemon/src/ipc.rs:72-100 | `peers.json` is read/written from `daemon::ipc` directly using `serde_json` against `Vec<serde_json::Value>` (untyped). A typed `Peer { name, fingerprint, added_at }` struct exists nowhere. | Move peer persistence into `copypaste-core::peers` with a typed `Peer` struct. |
| 24 | LOW | Observability | crates/copypaste-daemon/src/daemon.rs:69 | `device_id` is generated fresh via `Uuid::new_v4()` on every daemon start (`run_with_quit_flag`), not persisted. This breaks any P2P pairing that survives restart, breaks any cloud sync that uses `device_id` as a primary key, and makes log correlation impossible. | Persist `device_id` to `app_support_dir().join("device_id")` and reuse on startup. |
| 25 | INFO | Crate Boundaries | crates/copypaste-android/Cargo.toml:3 | Android crate is a `cdylib` that re-exports `copypaste-core` via UniFFI. Correctly isolated. No issue. | — |
| 26 | INFO | Layering | crates/copypaste-daemon/src/tray.rs + crates/copypaste-daemon/src/main.rs:54-93 | Tray icon code lives inside the daemon binary (correct — AppKit requires main-thread event loop). macOS-specific code properly gated with `#[cfg(target_os = "macos")]`. | — |
| 27 | INFO | Dependencies | Cargo.toml:[workspace.dependencies] | Workspace pins are well-organised with comments explaining MSRV constraints (`uuid >=1.0, <1.21`, `tempfile <3.14`). Pinning rationale is documented. Good practice. | — |

---

## Summary by Category

- **Crate Boundaries:** 6 (1 critical, 1 high, 1 medium, 2 low, 1 info)
- **Layering:** 4 (1 critical, 2 high, 1 info)
- **State Management:** 2 (1 high, 1 medium)
- **Storage:** 3 (1 critical, 2 medium)
- **Dependencies:** 2 (1 high, 1 info)
- **Concurrency:** 2 (1 high, 1 low)
- **IPC Protocol:** 2 (1 high, 1 medium)
- **Platform Abstraction:** 1 (1 high)
- **Platform:** 1 (1 high)
- **Feature Flags:** 1 (1 medium)
- **Configuration:** 2 (2 medium)
- **Observability:** 2 (1 medium, 1 low)

---

## Architectural Debt for Post-Alpha

1. **Integrate the orphan crates.** `copypaste-p2p`, `copypaste-sync`, `copypaste-supabase` must either be deleted or wired into the daemon. Currently 5 of 9 crates compile but are dead. This is the single biggest source of confusion and will only grow worse as new branches merge.
2. **Single source of truth for IPC types.** Extract `copypaste-ipc` crate. UI/CLI/daemon currently maintain three parallel definitions of every request/response.
3. **DB layer redesign.** Connection pool + `spawn_blocking` + RW separation. The current `Arc<Mutex<Database>>` will not scale past a few hundred items/sec or more than a handful of concurrent IPC clients.
4. **Protocol versioning.** Add `protocol_version` to `Request` before any breaking IPC change ships to users. Cheap insurance against painful migrations.
5. **Path/config unification.** Three path resolvers, two `AppConfig` structs — collapse to one each in `copypaste-core`.
6. **TLS stack convergence.** Upgrade `copypaste-supabase` to `reqwest 0.12` so rustls/hyper/http collapse to single versions. Halves the security-update surface.
7. **Relay persistence.** Decide: in-memory (delete `db.rs`) or persistent (wire `db.rs`). Currently neither.
8. **Migration safety.** Wrap `apply_migrations` in a transaction. Add a `schema_migrations` audit table for beta.
9. **Key rotation API.** `Database::rekey` for SQLCipher.
10. **Metrics export.** Prometheus or OTLP from daemon. Phase 2 — but design it now so the metric names don't churn.

---

## Blocker for alpha release?

**NO — with caveats.**

None of the critical findings break basic alpha functionality on macOS for a single user with P2P/cloud disabled. The shipping daemon's text/image clipboard capture, IPC, SQLCipher storage, and tray icon all work as a closed system.

**However**, three of the four CRITICAL findings (#1 orphan crates, #2 UI/daemon type drift, #4 relay phantom DB) should be addressed by **release notes** even if not by code fixes, because they will mislead any contributor or user who reads the repo:

- README should note: "P2P and cloud-sync in this alpha are stubs. The `copypaste-p2p`, `copypaste-sync`, `copypaste-supabase` crates are scaffolding for the next release and are not yet wired in."
- README should note: "The relay (`copypaste-relay`) is in-memory only; restarting the relay loses all registered devices and pending sync items."
- CHANGELOG entry: "IPC protocol is unversioned; clients and daemon must be built from the same commit."

If those three caveats are added to `docs/` and `README.md` before tagging, alpha can ship. The remaining HIGH findings should be on the v0.2 roadmap.

---

## Top 3 Architectural Risks

1. **Orphan crates with parallel stub implementations in the daemon (Finding #1).** The single greatest source of post-alpha confusion. The team has effectively built each feature twice — once as a proper crate, once as a stub in the daemon. Next merge from `intg-p2p-crates` or `intg-cloud-wire` will collide painfully.
2. **`Arc<Mutex<Database>>` chokepoint (Findings #5 + #6).** Every clipboard event, IPC request, TTL sweep, and background sync funnel through one async mutex around synchronous rusqlite calls. Under any meaningful load the tokio worker pool will stall on disk I/O while holding the mutex. This is fine for one user copying text occasionally; it will not survive the first beta tester who pastes 500 images.
3. **No IPC protocol versioning + 3 separate client implementations (Findings #2 + #8).** A daemon update that adds a field, renames a method, or changes an error string will silently break the CLI and UI built from a different commit. Alpha users running `brew upgrade copypaste-daemon` while keeping an older `copypaste-ui` in `~/Applications/` will hit this immediately.
