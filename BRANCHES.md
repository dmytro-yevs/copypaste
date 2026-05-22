# CopyPaste Branch Status Report

Generated: 2026-05-23
Base: main (76c4636) — feat(core,relay): pin_item storage function + relay GET /devices listing
Integration target: release/v0.1.0-alpha

## Summary

- Total branches (excluding main): 65
- Done + merge-ready: 38
- Partially done / needs review: 5
- Empty / stub / behind main: 22

---

## Branch Status Table

> **Files** column: number of files changed relative to main (via `git diff --name-only main..<branch>`).
> Branches with 0 commits ahead show 4314 files — this is the inverse diff (main moved past them); their actual contribution is 0.

### Android

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/android-clipboard-impl | Clipboard monitoring + encrypt + store (ClipboardService) | DONE | 1 | 4319 | NEEDS_REVIEW | Depends on android-uniffi-bindings |
| feature/android-gradle-cross-build | cargo-ndk Gradle task + jniLibs .so pipeline | DONE | 1 | 4318 | NEEDS_REVIEW | Dependency for android-clipboard-impl |
| feature/android-jni-wire | UniFFI Kotlin bindings + Gradle JNI config fix | DONE | 1 | 4318 | NEEDS_REVIEW | Fixes assembleDebug crash |
| feature/android-sync-decrypt | SyncManager decrypt via UniFFI binding | DONE | 1 | 4316 | NEEDS_REVIEW | Depends on android-jni-wire |
| feature/android-uniffi-bindings | UDL consistency check + uniffi-bindgen generation script | DONE | 1 | 4324 | NEEDS_REVIEW | Foundation for all Android JNI work |
| feature/ci-android-build | Android CI build job | STUB | 0 | 0* | NO | Never started; 0 commits ahead of main |

### CLI

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/cli-copy | `copy` command (INDEX/--id/--search/--list modes) | DONE | 1 | 4318 | YES | Self-contained |
| feature/cli-import-impl | `import` command implementation | STUB | 0 | 0* | NO | 0 commits ahead; stub only in main |
| feature/fix-cli-dead-code | Suppress dead-code warning on Response.id | DONE | 1 | 4316 | YES | Housekeeping fix, no conflicts |

### Daemon — Core

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/daemon-config-ipc | Daemon config IPC | STUB | 0 | 0* | NO | 0 commits ahead; not started |
| feature/daemon-dedup-hash | SHA-256 content dedup (prevent duplicate items) | DONE | 1 | 4323 | YES | No daemon IPC conflicts |
| feature/daemon-paste-back | Paste-back to NSPasteboard in copy handler | DONE | 1 | 4316 | YES | May conflict with daemon-config-ipc when that lands |
| feature/daemon-private-mode | Private/pause mode IPC + sensitive app detection | DONE | 1 | 4326 | YES | Touches daemon IPC handler |
| feature/daemon-sensitive-ttl | Auto-wipe sensitive items after TTL (default 30s) | DONE | 1 | 4320 | YES | Coordinates with daemon-private-mode |
| feature/daemon-tracing | tracing spans in IPC handler + clipboard monitor | DONE | 3 | 4321 | YES | 3 commits; most work in daemon group |

### UI (Slint)

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/tauri-macos-ui | copypaste-ui Slint shell crate | DONE | 1 | 4321 | NEEDS_REVIEW | Superseded by intg-ui-merge; check for duplication |
| feature/history-window-ipc | Slint HistoryWindow wired to daemon IPC | DONE | 1 | 4325 | NEEDS_REVIEW | Depends on ui-daemon-wire |
| feature/image-clipboard | NSPasteboard PNG/TIFF capture + image ClipboardItem | DONE | 2 | 4327 | NEEDS_REVIEW | 2 commits; daemon + core changes |
| feature/merge-ui-slint | Merge copypaste-ui crate (Slint HistoryWindow) | DONE | 1 | 4322 | YES | Integration/merge commit |
| feature/settings-pair-windows | SettingsWindow.slint + PairWindow.slint + Rust bindings | DONE | 1 | 4326 | NEEDS_REVIEW | Depends on history-window-ipc |
| feature/tray-icon-macos | macOS tray icon + launchd autostart | DONE | 1 | 4324 | NEEDS_REVIEW | Partially overlaps with merge-tray-icon |
| feature/tray-open-history | Tray open history window | STUB | 0 | 0* | NO | 0 commits ahead; not started |
| feature/ui-daemon-wire | Wire copypaste-ui IPC client to daemon history_page + paste | DONE | 1 | 4319 | NEEDS_REVIEW | Depends on history-window-ipc |

### macOS Platform

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/macos-bundle-daemon | Include daemon as externalBin in macOS .app bundle | DONE | 1 | 4314 | YES | Self-contained bundle config |
| feature/macos-launchagent-install | install-daemon.sh + uninstall-daemon.sh for LaunchAgent | DONE | 1 | 4317 | YES | Scripts only, no conflict |
| feature/macos-smoke-test | macOS smoke test | STUB | 0 | 0* | NO | 0 commits ahead; not started |
| feature/merge-tray-icon | Merge tray icon + launchd autostart | DONE | 1 | 4322 | YES | Integration/merge commit; overlaps tray-icon-macos |

### Windows Platform

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/windows-cfg-gate-ipc | Gate UnixListener/UnixStream with #[cfg(unix)] | DONE | 1 | 4318 | YES | Compatibility fix, no conflicts |
| feature/windows-daemon | Windows daemon plan + platform abstraction stubs | PARTIAL | 1 | 4329 | NO | docs + stubs only; impl pending |
| feature/windows-ipc-named-pipe | Windows named-pipe IPC server stub | PARTIAL | 1 | 4319 | NO | Stub under #[cfg(windows)]; not functional |

### Linux Platform

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/linux-daemon | Linux daemon X11/Wayland plan + platform stubs | PARTIAL | 1 | 4321 | NO | docs + stubs only; impl pending |

### P2P

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/p2p-mdns-discovery | mDNS-SD peer discovery via mdns-sd crate | DONE | 1 | 4320 | NEEDS_REVIEW | Depends on p2p-tls-handshake |
| feature/p2p-sync-protocol | P2P sync engine — Lamport clock, LWW merge, item exchange | DONE | 1 | 4323 | NEEDS_REVIEW | Core P2P feature |
| feature/p2p-tls-handshake | TCP listener + rustls mutual TLS handshake between peers | DONE | 1 | 4322 | NEEDS_REVIEW | Foundation for P2P stack |

### Relay

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/relay-device-register | POST /devices registration + auth token issuance | DONE | 1 | 4321 | YES | Self-contained relay route |
| feature/relay-sqlite | Relay SQLite persistence | STUB | 0 | 0* | NO | 0 commits ahead; not started |
| feature/relay-sync-routes | POST + GET /items push/pull sync routes | DONE | 1 | 4320 | YES | Depends on relay-device-register |
| feature/relay-v2-quotas | Rate limiting + device quotas middleware | DONE | 1 | 4326 | YES | Can be layered on relay-sync-routes |

### Supabase / Cloud

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/supabase-auth | GoTrue auth client (sign-in, refresh, sign-out, get_user) | DONE | 1 | 4324 | NEEDS_REVIEW | Basis for realtime |
| feature/supabase-realtime | Realtime WebSocket via Phoenix Channel protocol | DONE | 1 | 4322 | NEEDS_REVIEW | Depends on supabase-auth |

### Security / Core

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/fts5-search | FTS5 full-text search | STUB | 0 | 0* | NO | 0 commits ahead; stub exists in core |
| feature/pattern-detection | Sensitive data pattern detection + redaction | DONE | 1 | 4320 | YES | Core feature, self-contained |
| feature/sqlcipher | SQLCipher KeychainStore abstraction + DB key wiring | DONE | 1 | 4325 | NEEDS_REVIEW | Security-critical; needs review |

### Performance / CI

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/benchmarks | Criterion benchmarks | STUB | 0 | 0* | NO | 0 commits ahead; not started |
| feature/ci-github-actions | Windows matrix + cargo audit + fmt + scheduled audit | DONE | 1 | 4316 | YES | CI only, no conflicts |
| feature/ci-windows-runner | windows-latest cargo check in CI | DONE | 1 | 4315 | YES | CI only, no conflicts |
| feature/perf-poll-bench | Criterion benchmarks for encrypt/FTS5/insert | DONE | 1 | 4318 | YES | Core perf benchmarks |
| feature/release-workflow-macos | macOS DMG release workflow | STUB | 0 | 0* | NO | 0 commits ahead; not started |

### Integration — feature/intg-* (merge/wiring commits)

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| feature/intg-branch-create | Delete copypaste-app (Tauri) permanently | DONE | 0 | 0 | YES | Already at same tip as main (already merged) |
| feature/intg-daemon-cli-e2e | Integration test: daemon + CLI IPC roundtrip | DONE | 1 | 4319 | YES | Test-only changes |
| feature/intg-p2p-crates | Add copypaste-p2p + copypaste-sync crates | DONE | 1 | 4330 | NEEDS_REVIEW | Workspace Cargo.toml changes |
| feature/intg-p2p-ipc | Add P2P IPC methods (fingerprint/list_peers/pair/unpair) | DONE | 1 | 4318 | NEEDS_REVIEW | Requires p2p-tls-handshake first |
| feature/intg-p2p-orch | P2P orchestrator stub + broadcast channel | DONE | 1 | 4321 | NEEDS_REVIEW | Requires intg-p2p-crates + intg-p2p-ipc |
| feature/intg-rel-ci | release.yml workflow for macOS DMG + GitHub Release | DONE | 1 | 4315 | YES | CI only |
| feature/intg-rel-scripts | smoke_test.sh + make_app_bundle.sh + make_dmg.sh + Makefile | DONE | 1 | 4318 | YES | Scripts only |
| feature/intg-rel-version | Bump workspace to v0.1.0-alpha.1 | DONE | 1 | 4323 | YES | Version bump |
| feature/intg-sub-cloud | Optional Supabase cloud-sync (CloudOrchestrator, push+realtime) | DONE | 1 | 4321 | NEEDS_REVIEW | Requires supabase-auth + supabase-realtime |
| feature/intg-sub-crates | Merged copypaste-supabase crate (auth + realtime) | DONE | 1 | 4326 | NEEDS_REVIEW | Requires intg-sub-cloud |
| feature/intg-ui-ipc | Paste alias + get_config/set_config + cloud/peer stubs | DONE | 1 | 4324 | NEEDS_REVIEW | Requires ui-daemon-wire |
| feature/intg-ui-merge | Merged copypaste-ui (HistoryWindow + SettingsWindow + PairWindow) | DONE | 1 | 4329 | NEEDS_REVIEW | Largest UI merge; review carefully |

### Integration — integration/* (top-level orchestration)

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| integration/p2p-daemon | P2P + daemon integration | STUB | 0 | 0* | NO | 0 commits ahead; orchestration not started |
| integration/release-readiness | Release readiness gate | STUB | 0 | 0* | NO | 0 commits ahead; not started |
| integration/supabase-cloud-sync | Supabase cloud sync integration | STUB | 0 | 0* | NO | 0 commits ahead; not started |
| integration/ui-daemon-wiring | UI + daemon IPC integration | STUB | 0 | 0* | NO | 0 commits ahead; not started |

### Release

| Branch | Feature | Status | Commits | Files | Merge-Ready | Notes |
|--------|---------|--------|---------|-------|-------------|-------|
| release/v0.1.0-alpha | Release integration branch | PARTIAL | 1 | 1 | NO | Only branch-creation commit; no features merged yet |

---

## Merge Order Recommendation

### Phase 0 — Housekeeping (no conflicts, merge first)

These are isolated fixes or CI changes that unblock everything else:

1. `feature/fix-cli-dead-code` — fixes workspace build warning
2. `feature/windows-cfg-gate-ipc` — #[cfg(unix)] guard, unblocks Windows work
3. `feature/ci-github-actions` — Windows CI matrix + audit workflow
4. `feature/ci-windows-runner` — windows-latest cargo check
5. `feature/intg-rel-ci` — release.yml
6. `feature/intg-rel-scripts` — release scripts (shell only)

### Phase 1 — Core / Daemon features (independent of UI and P2P)

7. `feature/daemon-tracing` — 3 commits; span instrumentation
8. `feature/daemon-dedup-hash` — SHA-256 dedup
9. `feature/daemon-paste-back` — paste-back to NSPasteboard
10. `feature/daemon-sensitive-ttl` — TTL wipe
11. `feature/daemon-private-mode` — private mode IPC
12. `feature/pattern-detection` — sensitive data detection
13. `feature/perf-poll-bench` — criterion benchmarks
14. `feature/cli-copy` — CLI copy command
15. `feature/relay-device-register` — relay /devices POST
16. `feature/relay-sync-routes` — relay /items routes
17. `feature/relay-v2-quotas` — rate limiting middleware
18. `feature/macos-launchagent-install` — install scripts
19. `feature/macos-bundle-daemon` — .app bundle config
20. `feature/intg-rel-version` — version bump to v0.1.0-alpha.1
21. `feature/intg-daemon-cli-e2e` — E2E integration test

### Phase 2 — Security layer (requires Phase 1 core)

22. `feature/sqlcipher` — SECURITY CRITICAL; review carefully before merge
23. `feature/supabase-auth` — auth client
24. `feature/supabase-realtime` — realtime WebSocket

### Phase 3 — UI (requires Phase 1 daemon IPC stable)

25. `feature/merge-ui-slint` — merge copypaste-ui crate
26. `feature/history-window-ipc` — HistoryWindow ↔ daemon
27. `feature/ui-daemon-wire` — IPC client wiring
28. `feature/image-clipboard` — PNG/TIFF clipboard (2 commits)
29. `feature/settings-pair-windows` — Settings + Pair windows
30. `feature/tray-icon-macos` — tray icon
31. `feature/merge-tray-icon` — merge commit for tray
32. `feature/intg-ui-ipc` — paste alias + config IPC
33. `feature/intg-ui-merge` — full UI merge (largest)

### Phase 4 — P2P stack (requires Phase 2 security for TLS)

34. `feature/p2p-tls-handshake` — TCP + rustls mutual TLS
35. `feature/p2p-mdns-discovery` — mDNS peer discovery
36. `feature/p2p-sync-protocol` — Lamport clock + LWW sync
37. `feature/intg-p2p-crates` — add workspace crates
38. `feature/intg-p2p-ipc` — P2P IPC methods
39. `feature/intg-p2p-orch` — P2P orchestrator

### Phase 5 — Cloud / Supabase integration (requires Phase 2 + Phase 4)

40. `feature/intg-sub-crates` — merged supabase crate
41. `feature/intg-sub-cloud` — CloudOrchestrator + push/realtime

### Phase 6 — Android (requires Phase 2 security)

42. `feature/android-uniffi-bindings` — UDL + bindgen script
43. `feature/android-gradle-cross-build` — cargo-ndk Gradle task
44. `feature/android-jni-wire` — UniFFI Kotlin bindings fix
45. `feature/android-sync-decrypt` — SyncManager decrypt
46. `feature/android-clipboard-impl` — clipboard monitoring + store

### Phase 7 — Release branch population

47. Cherry-pick / merge all Phase 0–6 into `release/v0.1.0-alpha`

---

## Empty / Stub / Behind-Main Branches

These branches have **0 commits ahead of main** — they were created as stubs or placeholders and no implementation was committed:

| Branch | Notes |
|--------|-------|
| feature/benchmarks | Criterion suite placeholder; use feature/perf-poll-bench instead |
| feature/ci-android-build | Android CI job; not started |
| feature/cli-import-impl | import command; stub exists in main via ADR-004 |
| feature/daemon-config-ipc | Daemon config IPC; not started |
| feature/fts5-search | FTS5 search; stub exists in core but this branch has no work |
| feature/macos-smoke-test | macOS smoke test; not started |
| feature/relay-sqlite | Relay SQLite persistence; not started |
| feature/release-workflow-macos | macOS DMG release workflow; use feature/intg-rel-ci instead |
| feature/tray-open-history | Tray open history; not started |
| integration/p2p-daemon | Top-level P2P+daemon orchestration; not started |
| integration/release-readiness | Release gate; not started |
| integration/supabase-cloud-sync | Supabase integration; not started |
| integration/ui-daemon-wiring | UI+daemon wiring orchestration; not started |

---

## Partial / Docs-Only Branches (not merge-ready)

| Branch | Notes |
|--------|-------|
| feature/windows-daemon | Implementation plan + stubs only; no working Windows daemon |
| feature/windows-ipc-named-pipe | Named pipe server stub; compile-guarded but not functional end-to-end |
| feature/linux-daemon | X11/Wayland plan + stubs; no working Linux daemon |
| release/v0.1.0-alpha | Branch-creation commit only; awaiting Phase 0–7 merges |

---

## Conflict Risk Map

High conflict pairs (touch overlapping files):

- `feature/daemon-tracing` ↔ `feature/daemon-private-mode` ↔ `feature/daemon-config-ipc` — all modify `crates/copypaste-daemon/src/ipc.rs`
- `feature/tray-icon-macos` ↔ `feature/merge-tray-icon` — same tray integration; merge-tray-icon is the integration commit
- `feature/tauri-macos-ui` ↔ `feature/merge-ui-slint` ↔ `feature/intg-ui-merge` — Cargo.toml workspace membership
- `feature/intg-p2p-crates` ↔ `feature/intg-sub-crates` — both modify workspace Cargo.toml
- `feature/sqlcipher` ↔ `feature/daemon-*` — DB key wiring touches daemon init path

**Recommendation:** Merge daemon-* branches sequentially (not in parallel) and resolve IPC conflicts before merging UI or P2P layers on top.
