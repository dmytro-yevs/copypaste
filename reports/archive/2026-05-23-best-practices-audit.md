# Best Practices Audit — CopyPaste v0.1.0-alpha.1
**Auditor:** reviewer (best-practices)
**Date:** 2026-05-23
**Commit:** 7a577f7f9906c3504b789b394383ac9ebf1588b1
**Branch:** release/v0.1.0-alpha
**Scope:** Read-only static review of 9 workspace crates.
**Total findings:** 28 (Critical: 0, High: 7, Medium: 11, Low: 8, Info: 2)

## Findings (sorted by severity)

| # | Severity | Category | File:Line | Finding | Recommendation |
|---|----------|----------|-----------|---------|----------------|
| 1 | HIGH | Error Handling | crates/copypaste-daemon/src/paths.rs:16,27,40 | `dirs::home_dir().expect("HOME directory must exist")` panics on Windows containers / sandboxed environments and on Linux when HOME is unset (rare but possible). Daemon would die at startup with no telemetry. | Return `Result<PathBuf>` and propagate; the `app_support_dir()` caller in `main.rs` already uses `?` for `create_dir_all`. |
| 2 | HIGH | Error Handling | crates/copypaste-daemon/src/launchd.rs:16,28 | Same `HOME directory must exist` panic on plist generation. Install path failure becomes panic, not actionable error. | Return `Result` + `.context("home dir unavailable")`. |
| 3 | HIGH | Error Handling | crates/copypaste-daemon/src/tray.rs:185-193 | 7× `menu.append(&...).unwrap()` in `build_tray_menu()`. `MenuItem::append` returns `Result` because the AppKit bridge can reject items; panic in the tray init path means daemon starts but tray is dead with no log. | Use `?` and surface as `TrayInitError`. Log + continue without tray rather than panic. |
| 4 | HIGH | Error Handling | crates/copypaste-daemon/src/tray.rs:228 | `.build().expect("failed to create tray icon")` — tray creation failure (e.g., DISPLAY missing, headless CI smoke test) panics the entire process. | Convert to graceful degradation: log warning and run headless. |
| 5 | HIGH | Error Handling | crates/copypaste-daemon/src/logging.rs:67-69 | `tracing_appender::rolling::Builder::build(...)..unwrap_or_else(\|e\| panic!(...))` — log appender failure (disk full, perms changed mid-run) panics. After the careful fallback to `$TMPDIR`, falling further to stderr-only would be safer. | Drop the file layer instead of panicking; daemon should still run with stderr logs only. |
| 6 | HIGH | Concurrency | crates/copypaste-p2p/src/discovery.rs:96,106,123,147,186,266,283,296,301 | 9× `Mutex::lock().unwrap()` in production paths. A poisoned mutex (panic in another callback) takes down discovery entirely. Discovery callbacks are user-supplied (`on_peer_found`/`on_peer_lost`), so panics there are realistic. | Use `lock().unwrap_or_else(\|e\| e.into_inner())` or switch to `parking_lot::Mutex` to avoid poisoning. |
| 7 | HIGH | Error Handling | crates/copypaste-daemon/src/p2p.rs:62 | `listener.local_addr().unwrap()` in `accept_loop` debug log. Lossless but is the only `unwrap` on a syscall result in the hot path — kernel can return EBADF on race during shutdown. | Replace with `listener.local_addr().ok()` + `Option` formatting. |
| 8 | MEDIUM | Code Duplication | crates/copypaste-daemon/src/ipc.rs (lines around 350-380) | All `cloud_*` handlers return identical stub `Response::ok(req.id, json!({"note":"not yet implemented"}))`. 6+ near-duplicate match arms (`cloud_sign_in`, `cloud_sign_out`, future PAKE handlers). | Extract `Response::not_implemented(req.id, feature_name)` helper; route the 6 stubs through it. |
| 9 | MEDIUM | Code Duplication | crates/copypaste-cli/src/commands/{export,list,clear,delete,private,status,count,copy}.rs | Identical pattern repeated in **8 files**: `if !resp.ok { eprintln!("error: {}", resp.error.unwrap_or_default()); std::process::exit(1); }`. Cut-and-paste handler. | Extract `commands::common::exit_on_err(&resp)` once. Saves ~40 lines and centralises exit code policy. |
| 10 | MEDIUM | Code Duplication | crates/copypaste-daemon/src/ipc.rs:53-65 + crates/copypaste-daemon/src/p2p.rs (format_fingerprint) | `format_fingerprint(bytes: &[u8]) -> String` is reimplemented; the same logic also lives in `copypaste-ui/src/fingerprint.rs` (`format_fingerprint*` family). Three implementations of one canonical format. | Move to `copypaste-core::fingerprint` (or `copypaste-p2p::cert`) and re-export from UI/daemon. |
| 11 | MEDIUM | Error Handling | crates/copypaste-daemon/src/ipc.rs:read_config | `serde_json::from_str(&s).ok().unwrap_or_default()` silently swallows JSON parse errors → user edits config.json, gets a typo, daemon silently resets to defaults with no warning. | At minimum `tracing::warn!("config parse failed: {e}; using defaults")`. |
| 12 | MEDIUM | Error Handling | crates/copypaste-daemon/src/ipc.rs (peers_file_path) | `dirs::config_dir().unwrap_or_else(\|\| PathBuf::from("."))` — falling back to CWD silently writes `./copypaste/peers.json` in whatever directory the daemon was launched from. Surprising and hard to debug. | Either error out, or log a `warn!` once when fallback activates. |
| 13 | MEDIUM | Architecture / SRP | crates/copypaste-daemon/src/ipc.rs (931 lines) | Single file holds: `AppConfig` + serde, peer fingerprint formatting, peer file I/O, all 25+ IPC method handlers, pasteboard write (with two unsafe blocks). Violates project's 500-line cap (CLAUDE.md). | Split into `ipc/mod.rs` + `ipc/handlers/{config,peers,cloud,clipboard,private_mode}.rs`. |
| 14 | MEDIUM | Architecture / SRP | crates/copypaste-supabase/src/realtime.rs (680 lines), crates/copypaste-p2p/src/discovery.rs (623), crates/copypaste-sync/src/engine.rs (612), crates/copypaste-relay/src/state.rs (575) | Four other crates also exceed the 500-line file cap declared in `CLAUDE.md`. | Schedule a follow-up modularisation pass post-alpha; not blocking. |
| 15 | MEDIUM | Error Handling Mix | workspace-wide | Both `anyhow` and `thiserror` are workspace deps. `copypaste-core` / `copypaste-android` correctly use only `thiserror` (library style). `copypaste-daemon` uses `anyhow::Result` in `main` but its sub-modules (`keychain`, `platform/*`) define `thiserror` types — fine. **However**, only 8 `.context(...)` calls across all 9 crates. anyhow's biggest win (error chains) is being left on the table. | Add `.context("operation that failed")` to `?` sites in `daemon.rs`, `ipc.rs`, and `cloud.rs`. |
| 16 | MEDIUM | Logging | crates/copypaste-p2p/src/discovery.rs:621 | `println!("Found peers during integration test: ...")` in **non-test** code path (function not gated by `#[cfg(test)]` but lives in module body around an `#[ignore]`'d integration test setup). Will fire in production builds if that path is reached. | Replace with `tracing::debug!` or gate the entire helper behind `#[cfg(test)]`. |
| 17 | MEDIUM | Logging | crates/copypaste-daemon/src/logging.rs:51 | `eprintln!("copypaste-daemon: WARNING: cannot create log dir {}: {e}", ...)` runs *before* tracing is initialised, which is the correct intent, but the message goes to stderr unstructured. macOS launchd captures it but Linux/systemd journals won't categorise it. | Acceptable for bootstrap; add a `// pre-tracing bootstrap` comment to clarify intent. |
| 18 | MEDIUM | Logging Convention | crates/copypaste-daemon/src/keychain.rs:41 | `tracing::info!("Generated new device keypair; fingerprint={}", kp.fingerprint())` — fingerprint is public, so PII-safe, but log uses ad-hoc message format. Other tracing calls use structured fields (`%peer_addr`, `%item.id`). Inconsistent. | Use `tracing::info!(fingerprint = %kp.fingerprint(), "generated new device keypair")`. |
| 19 | MEDIUM | Cargo Hygiene | crates/copypaste-core/Cargo.toml, all crate Cargo.toml | None of the per-crate `[package]` sections declare `description`, `license`, `repository`, or `keywords`. `version.workspace = true` is set but workspace inheritance for `license`/`authors` is not used. | Add `license.workspace = true`, `repository.workspace = true`, `description = "..."` per crate. Required for any future crates.io publish. |
| 20 | MEDIUM | Cargo Hygiene | crates/copypaste-android/Cargo.toml:3 | `version = "0.1.0"` is hard-coded; doesn't inherit `version.workspace = true` like every other crate. Will drift from `0.1.0-alpha.1`. | Change to `version.workspace = true` for consistency. |
| 21 | MEDIUM | Result Swallowing | crates/copypaste-daemon/src/main.rs:84 | `let _ = tokio::time::timeout(...).await;` — discards both the timeout outcome AND the spawned daemon's `JoinError`. If the daemon panicked during shutdown, it's silent. | `if let Err(_elapsed) = tokio::time::timeout(...).await { tracing::warn!("daemon did not stop within 3s, forcing shutdown") }`. |
| 22 | LOW | Idiomatic Rust | crates/copypaste-core/src/crypto/{encrypt.rs:25, chunks.rs:81, keys.rs:46,63} | `expect("XChaCha20-Poly1305 ... cannot fail")` / `expect("HKDF expand 32 bytes always succeeds")` — these are **provably-unreachable** per algorithm contract. Comments document the reasoning. Acceptable, but `.unwrap()` with a `// SAFETY:`-style invariant comment is the more idiomatic Rust 1.75+ pattern. | Optional: tag with `#[allow(clippy::unwrap_used)]` and a doc comment explaining the invariant. |
| 23 | LOW | Idiomatic Rust | crates/copypaste-core/src/image.rs:220,226,262,268 | `expect(...)` in `#[cfg(test)]` paths — fine but pollutes the `expect` audit signal. | Replace test `expect` with Pest-style assertion helpers or just leave; not worth chasing. |
| 24 | LOW | Naming Convention | crates/copypaste-core/src/sensitive/detector.rs, crates/copypaste-relay/src/state.rs:154 | Mixed BE/AE spellings: `serialise`/`serialize`, `organisation`/`organization`. Project has no declared convention. | Pick one (project commits suggest BE: `serialise`, `colour`) and grep-replace. Low impact. |
| 25 | LOW | TODO Backlog | crates/copypaste-ui/src/ipc_client.rs:232,249,270,287,309,326 | 6 `TODO` markers for daemon-side gaps (X25519, PAKE, peer storage, Supabase auth). These are placeholders for unimplemented daemon RPCs — UI returns `Err("not implemented")`. | Track as GitHub issues; reference issue numbers in TODOs. Not blocking alpha if UI surfaces the "not implemented" message clearly. |
| 26 | LOW | TODO Backlog | crates/copypaste-daemon/src/p2p.rs:104,130,151 | 3 `TODO(intg-p2p-crates)` markers — entire P2P accept/subscriber/discovery path is documented stub. Code compiles but discards every inbound TCP stream and never publishes anything. | These are alpha-scope known gaps. Document in README that P2P is "discovery-only" in alpha. |
| 27 | LOW | TODO Backlog | crates/copypaste-daemon/src/tray.rs:68,250,289 | Tray icon embed, history-window-open, preferences-window-open are stubs. | OK for alpha if tray menu items are disabled or show "coming soon" — currently they just no-op silently which is worse. |
| 28 | LOW | Docs | crates/copypaste-ui/src/lib.rs:1-2 | `// lib.rs — copypaste-ui crate root` uses `//` line comments instead of `//!` inner-doc comments. Result: `cargo doc` produces a description-less crate page. Other crates (`copypaste-sync/lib.rs`) get this right. | Change `// lib.rs — ...` to `//! Slint UI windows for CopyPaste...`. |
| 29 | INFO | Idiomatic Rust | crates/copypaste-daemon/src/daemon.rs:421 | `tracing::warn!("Non-macOS platform: using ephemeral encryption key (data not persisted across restarts)")` — good user-facing warning, but on Linux this fires on every start. | Consider gating to once-per-process via `OnceLock` or `tracing::warn_once`. |
| 30 | INFO | Test Coverage | crates/copypaste-daemon/src/ipc.rs unsafe blocks (lines 517, 530) | Two `unsafe` blocks wrapping `NSPasteboard::setString_forType` / `setData_forType`. No integration test exercises the write path (only `tests/integration_ipc.rs` for reads). | Add a macOS-gated integration test that writes via the IPC handler and reads back via the pasteboard polling loop. |

## Summary by Crate

| Crate | Findings | Notes |
|---|---:|---|
| copypaste-core | 2 | Crypto `expect`s are provably-safe (#22). |
| copypaste-daemon | 13 | Most surface area — most issues. ipc.rs is the focal point. |
| copypaste-cli | 1 | CLI handlers cleanly structured; one duplication (#9). |
| copypaste-relay | 1 | Only file-size finding (#14). |
| copypaste-ui | 2 | Doc comment (#28) + TODOs (#25). |
| copypaste-p2p | 2 | Mutex poisoning (#6) + stray `println!` (#16). |
| copypaste-sync | 1 | File size (#14); otherwise the **best-documented** crate in the workspace. |
| copypaste-supabase | 1 | File size (#14). |
| copypaste-android | 1 | Version pin (#20). |
| **Workspace-wide** | 4 | Cargo hygiene (#19), error-handling mix (#15), file naming, anyhow context usage. |

## Top 5 Quick Wins

1. **Replace the 9 `Mutex::lock().unwrap()` in copypaste-p2p/discovery.rs** with `parking_lot::Mutex` or poison-tolerant unlock. One-line dep swap + sed; eliminates an entire class of latent panics in user-callback paths.
2. **Extract `cli::common::exit_on_err(&resp)`** to delete ~40 lines of duplicated `eprintln! + process::exit(1)` across 8 CLI command files. Pure refactor, zero behaviour change.
3. **Convert tray.rs `expect`/`unwrap` to graceful degradation** (`tray::run_headless()` fallback). Lets daemon start in headless CI / SSH-tunnel scenarios.
4. **Add `license.workspace = true` + `description`** to every per-crate `[package]`. Two-line edit per crate; unblocks any future crates.io / cargo-vendor work.
5. **Wrap `let _ = tokio::time::timeout(...)` in main.rs:84** with a warn-on-timeout. One-line change; saves future debugging of "why didn't daemon stop?" mysteries.

## Blocker for alpha release?

**NO.**

Rationale:
- Zero CRITICAL findings.
- All 7 HIGH findings are panic paths in **initialisation code** (paths/launchd/tray/logging) or **callback boundaries** (p2p mutex). They reduce robustness on non-happy paths but do not corrupt data, leak secrets, or cause incorrect behaviour on the happy path.
- The crypto layer (`copypaste-core`), the storage layer, and the sync protocol (`copypaste-sync`) are clean, well-documented, and correctly idiomatic. These are the surfaces an alpha *must* get right.
- The two `unsafe` blocks in IPC pasteboard writes and one in clipboard polling are minimal, well-scoped, and use the official `objc2` bindings — they pass review.
- TODO backlog (~14 markers) is concentrated in known-unimplemented integration points (p2p stub, UI→daemon RPCs, Supabase auth) which are documented as alpha gaps.

**Recommended pre-alpha actions** (≤1 day work):
- Fix #6 (p2p mutex poisoning) — actual robustness win.
- Fix #9 (CLI duplication) — actual code-quality win.
- Fix #20 (android version pin drift) — actual hygiene win.

Everything else is post-alpha follow-up.
