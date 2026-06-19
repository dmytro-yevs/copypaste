# CopyPaste — Parity Fix Summary

Date: 2026-06-19 · Tracking: CopyPaste-t5qm · Status: **PRE-FIX (audit complete, no parity fixes applied yet).**
Companion: `PARITY_MATRIX.md`, `PLATFORM_GAP_REPORT.md` (PG-#).

This document is the running record of the parity campaign. The matrix + gap report are built; **fixes have not been applied** (per the "build the matrix before assigning fixes" instruction, and pending your go-ahead to run the P0/P1 fix phase). Sections 1–2 will be filled as fixes land.

---

## 1. Parity gaps FIXED
*(none yet — to be populated as Phase-1 fixes land)*

| PG-# | Title | Commit | Tests added |
|---|---|---|---|
| — | — | — | — |

## 2. Remaining differences that are still OPEN
All 61 gaps in `PLATFORM_GAP_REPORT.md` are currently open: **15 P1 · 28 P2 · 18 P3**. Fix order is the Phase plan below.

---

## 3. Differences that are INTENTIONAL (documented, not bugs)

These are real platform constraints; the user-facing model stays consistent and disabled/unavailable states are shown.

| Difference | Why | Where surfaced |
|---|---|---|
| Android has no daemon process | Mobile has no long-lived user daemon; UniFFI in-process + SharedPreferences is the authority | docs/android-background.md; ARCHITECTURE.md (to update) |
| Android background capture needs AccessibilityService; sync degrades to 15 min under Doze | Android 10+ blocks background clipboard read; OEM killers/Doze stop the FGS | docs/android-background.md; onboarding |
| Android API<31 uses bullet-mask instead of GPU blur | `Modifier.blur()` is a no-op below Android 12 | ClipboardService bullet fallback |
| macOS has no QR camera scan (Android scans macOS's QR) | No camera API in the daemon/Tauri context | pairing flow; PG-doc |
| macOS has no always-on FGS (uses launch agent / app-owned daemon) | OS model difference | ADR-014 |
| CLI is display/scripting only (no blur, countdown, SAS, devices/status UI; `watch` terminal-only; `backup`/`restore` shell scripts) | Terminal UX | CLI command parity table |
| Glass opacity 0.40 (macOS NSVisualEffectView) vs 0.55 (Android RenderEffect) | Different blur renderers | PARITY-SPEC §A.6 |
| About/Logs in macOS sidebar vs reached via Settings on Android | Phone nav budget | PARITY-SPEC §C (accepted) |
| Windows frozen | ADR-012 | excluded from parity |

**Caveat — currently UNDOCUMENTED differences that must become documented-or-fixed:** the Android sensitive-item drop (PG-3/PG-15), the per-platform settings-storage split (daemon config vs SharedPreferences), and the Android local re-implementation of classification/search/copy (PG-16/17/18). Until resolved, these are gaps, not accepted differences.

---

## 4. Manual QA still needed (cannot be fully verified by static read)

1. **Rust gate suite on rustc ≥ 1.96** — local toolchain is 1.95; `cargo fmt/clippy/test/check` were BLOCKED. Re-run before trusting any fix.
2. **Android build + on-device** — `make android-so` / Gradle were not run (toolchain + OOM guard). Verify ABI-17 bindings, FFI panic boundary, and each fixed flow on a real device.
3. **Sensitive sync end-to-end** — copy a secret on macOS → confirm whether it reaches Android (PG-15 decision); confirm Android no longer drops at capture after PG-3.
4. **Sync-badge state machine** — kill daemon / drop network / idle 6 min on both platforms; confirm identical badge after the shared-enum fix (PG-10/11).
5. **QR blur persistence** — reveal QR on Android PairActivity, wait for TTL auto-refresh, confirm it stays revealed (PG-8).
6. **Pairing fingerprint verification** — confirm full fingerprint visible in the Android SAS modal (PG-47) and on the macOS own-device card (PG-9).
7. **P2P unpair over inbound listener** — unpair from macOS while Android FGS alive; confirm Android drops the peer (PG-1).
8. **Relay registration from Android** — confirm PoP accepted after the FFI export (PG-2).
9. **Export/import sensitive handling** — verify default-exclude + tamper-resistant import re-classification (PG-5/PG-26).
10. **Screenshot guard** — `setContentProtected` blanks the macOS history window when maskSensitive is on (PG-25).
11. **Settings persistence** — change each setting on both platforms, restart, confirm persistence and single source of truth (PG-29..36).
12. **Cross-platform list ordering** — sync a bumped item; confirm identical order macOS vs Android after the lamport-ordering fix (PG-19).

---

## 5. Phase plan (proposed — confirm before running)

- **Phase 1 (P1, 15 gaps):** PG-1..PG-15. Security/privacy + pairing/identity + sync-status drift. Includes the shared `SyncBadgeState` enum (PG-10/11), Android sensitive contract decision (PG-3/15), span masking (PG-4), QR blur (PG-8), Tauri protocol_version (PG-6), Supabase UI (PG-13), relay PoP FFI (PG-2), inbound Unpair (PG-1).
- **Phase 2 (P2, 28 gaps):** settings parity, device-card fields, transport latency, Android re-impl consolidation (prefer shared FFI/daemon-emitted values over Kotlin copies).
- **Phase 3 (tests):** parity contract tests — IPC error-code mapping, content-type classifier cross-check (core vs TextKind.kt), sync-status mapping, QR privacy-state, settings persistence, token parity (extend parity-check.mjs to `:root`).
- **Phase 4 (P3, 18 gaps):** visual/UX (chip colors, icons, toast dot, timestamp format, tab placement).
- **Phase 5:** doc reconciliation (PARITY-SPEC, ARCHITECTURE.md crate graph, relay-api.md, android-background.md), close the matrix.

**Definition of done** (from the brief): all P0/P1 parity gaps fixed; all remaining differences documented in §3; gates re-run on a ≥1.96 toolchain; this summary's §1–2 completed.

---

## Orchestrator remediation status (2026-06-19)

Driven alongside the AUDIT_REPORT campaign. **No Rust/Android toolchain in this environment** — UI (pnpm), parity-check, docs/config verified locally; all Rust/Kotlin fixes are diff-reviewed and flagged `CI/GRADLE-REQUIRED`.

### Fixed & closed (PG)
| PG | bd | What | Verified |
|---|---|---|---|
| PG-5 | mv1v | export skips sensitive by default (tj9s) + CLI `--include-sensitive` | CI |
| PG-6 | zfqa | Tauri bridge forwards `protocol_version`; mismatch handler fires | pnpm (TS) / CI (src-tauri) |
| PG-7 | 2hxr | shared `isIpcNotReady` applied in all views | pnpm |
| PG-9 | wb6s | own-device fingerprint on macOS ThisDeviceCard (**reverses 55vf**; flag for design owner) | pnpm |
| PG-13 | jhvl | Supabase email+password inputs in macOS UI | pnpm |
| PG-14 | tpvi | private-mode restored on degraded boot (no silent capture) | CI |
| PG-21 | jfxl | supabase_email/password made writable in ipc.ts | pnpm |
| PG-26 | vuxs | import recomputes `is_sensitive` (TTL-bypass closed) | CI |
| PG-30 | j9xj | macOS master sync toggle (UI done; daemon `sync_enabled` → **tke7**) | pnpm |
| PG-32 | noui | history display limit persists (dup of audit 2b1g) | pnpm |
| PG-34 | n9gp | `showSensitiveWarnings` toggle + reveal-flow gating | pnpm |
| PG-36 | fcpf | macOS `:root` Liquid-Blue `#3D8BFF`→`#4D8DFF` (7 sites) | pnpm + parity 53/53 |
| PG-37 | efdc | Android offline dot grey→red (parity w/ macOS) | GRADLE |
| PG-38 | zdzy | device-name source divergence **documented** (Android user-name = future) | — |
| PG-40 | i2sr | hybrid relative/absolute last-sync + "Synced" label | pnpm |

Also: macOS upload half of **PG-15 (qh1c)** fixed by audit `jbao`.

### New follow-ups filed
- **tke7** — daemon `AppConfig::sync_enabled` (PG-30 UI is inert until this lands).
- **nq39** — dedicated IPC verb for Supabase password (audit P1-6 residual).

### Remaining — BLOCKED on absent Rust/Android toolchain (cannot build/verify here)
**~22 Android PG issues** (Kotlin/UniFFI): PG-1 (7d8x), PG-2 (kmcr), PG-3 (349q), PG-4 (ojsh), PG-8 (l7n0), PG-11 (71cf), PG-12 (8qcm), and the P2 Android rows PG-16…PG-47 (89ve, mxoq, o0t3, cvns, l9z8, 5tnx, 8cu0, qsz4, yqn5, 08r1, mtf5, 65gv, ksrs, up7a, xmvj, msx1, oy8s, …). Plus the Android half of PG-15. These need cargo-ndk + gradle (single OOM-guarded cross-compile) on a toolchained machine.

### Remaining — deferred-documented (verifiable only on a toolchain, or needs a decision)
- PG-10 (5qbe) sync-badge offline-signal model — cross-platform decision needed.
- PG-22 (mtf5) is_sensitive_app wiring; PG-25 (13a3) macOS screenshot guard; PG-20 (ro0r) migration_in_progress backoff — Rust, unverifiable here.
- macOS-side PG-31 (58ou auto_apply_synced_clip) needs an IPC AppConfig field (cross-crate).

**Net:** every macOS/Tauri/CLI/core PG gap verifiable without a compiler is fixed + locally green; the Android bulk and cross-crate-daemon items are implemented-where-safe or bucketed for a ≥1.96 + Android toolchain. CI/gradle is the gate before merge.
