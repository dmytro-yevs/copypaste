# Release Readiness Audit — CopyPaste v0.1.0-alpha.1 → Full Release Path
**Auditor:** reviewer (auditor-release-readiness)
**Date:** 2026-05-23
**Commit:** `c5d12bdc4ee748f3a0b7ccbb122582c2e1637448`
**Branch:** `release/v0.1.0-alpha`
**Scope:** read-only meta-audit; estimate % to FULL v1.0 release (not alpha)

---

## TL;DR

**Overall completeness toward FULL release: ~38%**

- Alpha (v0.1.0-alpha) ready in **~2 weeks** after Wave 1+2 fixes (8 critical+19 high audit findings, plus 5 critical edge cases). Foundation is solid: 9 crates, ~16.7K LOC Rust, 426 tests, SQLCipher storage, FTS5 search, mTLS p2p, Supabase scaffolding all merged. Caveats documented.
- Beta (v0.2.0) ready in **~10–14 weeks**. Requires: Android APK builds end-to-end + real PAKE pairing + IPC versioning + real device fingerprint + Windows daemon parity + image clipboard wired to history + auto-update channel.
- Full v1.0 release ready in **~6–9 months**. Requires: Apple Developer signing & notarisation, Authenticode-signed MSI, Play Store onboarding, full localisation policy decision, ≥80% test coverage on platform crates, all 8 deferred architectural-debt items resolved, opt-in telemetry, privacy/ToS pages.
- **Top blocker (alpha):** 2 CRITICAL security findings (fake device fingerprint, deterministic relay bearer token) plus relay token can leak via `GET /devices`.
- **Top blocker (beta):** Android app has 0 native tests and APK is unsigned/un-released; Windows daemon is a stub.
- **Top blocker (full):** No code-signing pipeline, no auto-update mechanism, no store presence on any platform.

---

## Category breakdown (weighted)

| Category | Weight | % | Weighted | Evidence | Critical Gap |
|----------|--------|---|----------|----------|--------------|
| Core functionality | 15% | 88% | 13.2 | copypaste-core 101 tests (crypto/storage/sensitive/FTS5/SQLCipher); daemon IPC roundtrip integration test; clipboard text+image capture merged | Edge cases: concurrent writers (#4), Lamport overflow tested only in sync, DB corruption recovery (#35), schema downgrade |
| macOS daemon | 12% | 60% | 7.2 | NSPasteboard polling, Keychain, launchd plist + install scripts, tray icon, .app bundle script (`make_app_bundle.sh`), DMG script (`make_dmg.sh`), Accessibility permission check script | No Developer ID signing, no notarisation, no Sparkle/auto-update, no Homebrew cask; tray paste-back stub |
| Android | 12% | 30% | 3.6 | 13 Kotlin files (MainActivity, ClipboardService, SyncManager, RelayClient, NotificationHelper, ViewModel, Settings, Repository), UniFFI bindings generated, cargo-ndk Gradle wiring, foreground service, CI workflow file | 0 native tests; no signed APK; no Play Store metadata; no privacy policy required for Accessibility; not validated end-to-end on device |
| Windows | 12% | 12% | 1.4 | `#[cfg(windows)]` named-pipe IPC stub merged; platform abstraction sketched | Daemon never wired; no NSPasteboard equivalent (clipboard polling); no Service registration; no MSI/installer; no Authenticode |
| Slint UI | 8% | 70% | 5.6 | copypaste-ui crate merged with HistoryWindow, SettingsWindow, PairWindow; 41 tests; ipc_client extended; Slint 1.8 | No global hotkey wired in app (separate task `task-tauri-global-hotkey` is BLOCKED); cold-boot UX (#25), virtual scrolling (#26), empty-state PairWindow (#27); fingerprint shown is fake |
| P2P sync | 8% | 55% | 4.4 | copypaste-p2p (31 tests) + copypaste-sync (36 tests) merged: self-signed cert, mDNS-SD discovery, mTLS handshake, Lamport clock, LWW merge, sync orchestrator | No real PAKE pairing (out-of-band only); no NAT traversal; p2p daemon IPC methods return stubs; rogue mDNS untested |
| Cloud sync | 6% | 50% | 3.0 | copypaste-supabase crate (32 tests): auth, realtime WS, Phoenix protocol, exponential backoff; daemon CloudOrchestrator merged with push+realtime loops | Auth silently falls back to anon_key on failure (sec HIGH #3); no Supabase project provisioned; no RLS schema doc; refresh race, 429 backoff, schema-drift untested |
| Security posture | 10% | 45% | 4.5 | XChaCha20-Poly1305, SQLCipher at rest, mTLS for p2p, rustls everywhere, cargo audit workflow | **2 CRITICAL** (fake fingerprint, deterministic relay token) + **6 HIGH** + 23 lower findings unresolved; 521 `unwrap()` occurrences (audit-flagged); 23 TODOs in production paths |
| Testing | 8% | 55% | 4.4 | 426 tests total across 8 native crates; integration test for daemon+CLI; clippy clean under `-D warnings` | copypaste-android = 0 tests (complete gap); 37 edge-case findings unaddressed; no fuzz targets running; no coverage report (tarpaulin) wired; no E2E desktop matrix |
| Documentation | 4% | 50% | 2.0 | 5 ADRs (001 XChaCha20, 002 Unix socket IPC, 003 SQLCipher, 004 SQLite WAL, 005 Slint UI); audit reports; relay-api doc; design spec | No user manual; no install guide; no troubleshooting; no API reference for IPC protocol; CHANGELOG missing; no support/issue templates beyond `.github` skeleton |
| Packaging + distribution | 3% | 25% | 0.75 | macOS: `.app` bundle + DMG scripts exist (ad-hoc signed); LaunchAgent plist; install/uninstall scripts; release.yml workflow | **No** Developer ID signing, **no** notarisation, **no** signed APK, **no** Windows installer, **no** store listings (App Store / Play / WinGet), **no** auto-update channel |
| CI/CD pipeline | 2% | 60% | 1.2 | 4 workflows: `ci.yml`, `audit.yml`, `ci-android-build.yml`, `release.yml`; clippy + cargo-audit + Windows matrix | Release workflow does not produce signed/notarised artefacts; no smoke/E2E job; no coverage gate; no SLSA attestation |
| **TOTAL** | **100%** | — | **~38.3** | — | — |

---

## Per-feature gap analysis (toward FULL release)

**Audit baseline (from 2026-05-23 reports):**
- security: 2 CRITICAL, 6 HIGH, 8 MEDIUM, 15+ LOW/INFO (31 sections)
- architecture: 5 CRITICAL, 8 HIGH, 27+ MEDIUM/LOW (40 sections)
- best-practices: 1 CRITICAL, 8 HIGH, 23 lower (32 sections)
- edge-cases: 4 CRITICAL, 11 HIGH, 22 MEDIUM (37 sections)
- Total findings: **12 CRITICAL + 33 HIGH** across four audits.

**Stubs / not-yet-functional pieces:**
1. P2P pairing UI (`pair_peer`, `list_peers`, `unpair_peer`, `get_own_fingerprint`) — daemon returns stubs.
2. Real out-of-band pairing protocol (PAKE / SAS) — only TLS-pinned fingerprint exists; no Magic Wormhole–style flow.
3. Image clipboard captured but not yet surfaced in HistoryWindow (text-only render path).
4. Global hotkey (formerly Tauri task) is BLOCKED — Slint replacement not wired.
5. Windows clipboard polling + IPC server.
6. Auto-update mechanism (Sparkle / Squirrel / in-app update).
7. Telemetry opt-in (error reporting, anonymous metrics).
8. Privacy policy, ToS, support channel, code of conduct.
9. Localisation framework (or explicit "English only for v1" decision).
10. Linux support is explicitly **frozen** — formal "not a target" notice needed in README.

**Architectural debt deferred to post-alpha** (from `fix-plan.md`):
- Integrate or remove orphan crates `copypaste-p2p`, `copypaste-sync`, `copypaste-supabase` (CRITICAL arch #1).
- Extract `copypaste-ipc` crate to stop type drift (CRITICAL arch #2).
- Relay persistence (delete in-memory `db.rs` or wire it through).
- `Arc<Mutex<Database>>` → connection pool + `spawn_blocking`.
- Unified `AppConfig` (currently split between core::config and daemon::ipc).
- IPC protocol versioning (clients + daemon must currently match commit).
- Workspace dep dedup (reqwest 0.11→0.12, rustls 0.21→0.23, hyper 0→1).

---

## Roadmap to 100%

| Milestone | Date target | Gating work |
|-----------|-------------|-------------|
| **v0.1.0-alpha** | T+2w (early June 2026) | Wave 1 (6 critical fixes) + Wave 2 (9 high) + Wave 3 (7 cleanups) per `2026-05-23-fix-plan.md`. Ship ad-hoc-signed DMG for invited testers only; document caveats in README. |
| **v0.2.0 beta** | T+10–14w (Aug–Sep 2026) | Android: signed APK + on-device QA + relay-mode pairing working. Real device fingerprint from p2p cert. PAKE pairing (SAS or Magic Wormhole). IPC versioning. Image clipboard rendered. Global hotkey wired. Auto-update (Sparkle for macOS, in-app for Android). Resolve 5 of 8 deferred arch-debt items. |
| **v0.3.0 RC** | T+5–6m (Oct 2026) | Windows daemon parity: clipboard polling, named-pipe IPC server real, MSI installer, Authenticode signing. Connection-pool refactor. Unified AppConfig. Workspace dep dedup. Coverage ≥70% all crates. Localisation framework or formal English-only declaration. |
| **v1.0.0 GA** | T+8–9m (Jan–Feb 2027) | Apple Developer ID signing + notarisation in CI. Play Store + App Store + WinGet listings. Privacy policy, ToS, support page, status page. Opt-in telemetry (Sentry / OTLP). E2E matrix in CI (macOS + Windows + Android emulator). SLSA L2 attestation. Coverage ≥80% on core+daemon, 100% on crypto. Zero CRITICAL findings; ≤5 HIGH with documented mitigations. |

---

## What's already strong

1. **Crypto foundation** — XChaCha20-Poly1305 (ADR-001), SQLCipher at rest (ADR-003), mTLS for p2p, rustls stack; 100% native Rust crypto; chunked encryption with KAT-style tests in copypaste-core (101 tests).
2. **Sync algorithm** — Lamport clock with LWW merge fully tested in copypaste-sync (36 tests); Wave 1.4 already applied (`saturating_add` + idempotent merge test, commit `c5d12bd`).
3. **Storage** — SQLite + FTS5 search + SQLCipher all merged; pin_item, sensitive detector, backup feature enabled; rusqlite 0.31 with bundled-sqlcipher.
4. **Daemon IPC + integration** — Unix-socket IPC with end-to-end integration test (daemon + CLI roundtrip).
5. **Release engineering scaffolding** — 4 CI workflows, 10 well-organised scripts (bundle, DMG, install/uninstall LaunchAgent, smoke test, permissions check, Android NDK build, UniFFI bindings, completions).
6. **Documentation discipline** — 5 ADRs covering critical decisions; comprehensive audit suite delivered today; design spec preserved.
7. **Clean compile state** — clippy passes `-D warnings` (commit `bc2687b`), workspace test compile unblocked (commit `7c6dc25`).

---

## What's most missing

1. **Signing & notarisation** — zero code signing on any platform; this single gap alone blocks any kind of public store distribution.
2. **Android end-to-end** — 0 tests in copypaste-android, no signed APK build verified, no Play Store privacy policy (required for Accessibility/Foreground service).
3. **Windows beyond stub** — only `#[cfg(windows)]` IPC stub merged; no clipboard polling, no Service support, no MSI.
4. **Real pairing UX** — UI shows a *fake* fingerprint (security CRITICAL #2); pairing methods return stubs; no PAKE/SAS flow.
5. **Auto-update + telemetry** — neither exists; both are table-stakes for a v1.0 GA product.
6. **IPC protocol versioning** — clients and daemon must be built from the same commit, which is acceptable for alpha but blocks any kind of independent app/daemon release cadence.
7. **Coverage gate + fuzzing** — no tarpaulin in CI, no cargo-fuzz targets running, no E2E desktop runners.
8. **Legal/support surface** — no privacy policy, no ToS, no support channel, no localisation policy.
9. **Architectural-debt backlog** — 8 deferred items including the orphan-crate integration and `copypaste-ipc` extraction; these will compound if not scheduled.
10. **521 `unwrap()` calls** flagged by best-practices audit + 23 in-source TODOs in `ipc.rs`, `p2p.rs`, `tray.rs` (icon, history window, preferences window).

---

## Confidence & methodology notes

- Numbers derived from reading the six 2026-05-23 audit reports + `cargo metadata` + repo inventory (no fresh code analysis).
- "%" per category is engineering judgement weighted against the FULL-release definition (signed/notarised/coverage/legal/store presence), not just code completeness.
- Estimates assume **single-developer pace with the current parallel-swarm workflow**; signing + store onboarding are calendar-bound (Apple/Google review windows), not effort-bound.
- 426 native test count from `grep '#\[test\]|#\[tokio::test\]'`; coverage % not measured (no tarpaulin run available).
