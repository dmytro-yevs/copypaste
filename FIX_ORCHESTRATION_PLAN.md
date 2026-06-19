# CopyPaste — Fix Orchestration Plan

**Driver:** Orchestrator agent · **Date:** 2026-06-19 · **Source:** `AUDIT_REPORT.md` / `AUDIT_FINDINGS.md` / `AUDIT_FIX_PLAN.md` (tracking `CopyPaste-o7me`, closed)
**Findings:** 0 P0 · 13 P1 · ~26 P2 · ~24 P3.

## ⚠️ Environment constraint (governs verifiability)

**No Rust toolchain in this environment.** `rustc`, `cargo`, `rustup` are all absent from PATH and not installed (`~/.cargo/bin` holds only the `cargo-audit`/`cargo-deny`/`cargo-ndk` *plugins*, which need the missing `cargo` driver). The audit itself was blocked at rustc 1.95 < MSRV 1.96; this environment is worse (no toolchain at all).

Consequence:
- **Cannot** locally run `cargo build/test/clippy/fmt/check` or any Rust gate.
- **Cannot** build the Android `.so` (UniFFI needs `cargo` + cargo-ndk; also OOM guard).
- **Can** run: `pnpm` UI gates (vitest / tsc / vite build), `node scripts/parity-check.mjs`, all doc/config/CI edits (verified by reading), and possibly the `cargo-audit`/`cargo-deny` standalone binaries.

**Therefore:** every Rust source fix in this plan is *implemented + reviewed by reading*, and explicitly **flagged `CI-COMPILE-REQUIRED`** — CI (which installs ≥1.96) is the compile/test gate. No Rust fix is claimed "verified" locally. Doc/config/UI fixes are completed and locally verified where a gate exists.

## Architecture invariants enforced during all fixes
- CLI & UI stay IPC-only (no `copypaste-core` link).
- Relay stays ciphertext-only (never `PRAGMA key`, never plaintext).
- `copypaste-core` keeps zero deps on daemon/cli/ui.
- No plaintext storage, no plaintext sync, no secret/token/QR/clipboard logging.
- No IPC protocol break unless unavoidable (P1-6 adds a verb additively).

## Agent assignment — disjoint file ownership (no two concurrent agents touch the same file)

| Agent | Owns (files) | Findings | Compile risk |
|---|---|---|---|
| **A · docs-config** | `docs/*`, `README.md`, `SECURITY.md`, `ARCHITECTURE.md`, `contrib/systemd/*.service`, `crates/copypaste-daemon/Cargo.toml`, other crates' MSRV comments, `crates/copypaste-ui/package.json`, `android/app/build.gradle.kts` (version), `.github/workflows/{audit,ci-matrix}.yml`, `deny.toml`, `ADR-010` | P1-5, P1-9, P1-10, P1-11, P1-12, P1-13; P2 17lj/2915/g4rs/4rui/m7mm; P3 docs | none (text/config) |
| **B · daemon-core** | `crates/copypaste-daemon/src/daemon.rs` only | P1-2(lszh), P1-3(26pd), P1-4(oti6); P2 ugv7; P3 config-warn/hot-reload/size-precheck | CI-COMPILE |
| **C · relay-push** | `crates/copypaste-daemon/src/relay.rs`, `cloud.rs`, `sync_orch.rs`; `crates/copypaste-relay/src/routes/mod.rs` | P1-1(jbao); P2 7185 | CI-COMPILE |
| **D · ipc-cli** | `crates/copypaste-daemon/src/ipc.rs`, `crates/copypaste-cli/src/commands/{cloud,pair_qr}.rs`, `crates/copypaste-cli/Cargo.toml` | P1-6(v6wh); P2 ptb8/8u2b/iqkm/tj9s/jqcp/44u4 | CI-COMPILE (highest) |
| **E · android** | `crates/copypaste-android/src/lib.rs`, `android/.../CopypasteBindings.kt`, `android/.../Color.kt` | P1-8(xxsw); P2 ar2r/2ffx/vo79; P3 Panicked | CI-COMPILE |
| **F · ui-ts** | `crates/copypaste-ui/src-tauri/tauri.conf.json`, `crates/copypaste-ui/src/*` | P1-7(wb2c); P2 3e6g/54h5/00ae | pnpm-VERIFIABLE |
| **G · detector** | `crates/copypaste-core/src/sensitive/patterns.rs`, `detector.rs` | P2 fb3e/r6cw/ozzt | CI-COMPILE |

**Deferred (Wave 2, sequential — file collisions or needs prior wave):**
- P2 k89j (move `init_global`/`tracing-subscriber` out of core): edits `core/Cargo.toml` (collides with A) + `logging.rs` + binary crates. Run after A finishes Cargo edits.
- P2 o8ew (`ipc_win.rs` orphan): tiny, after D frees the daemon crate.
- P3 cluster (encrypt.rs `pub(crate)`, relay HKDF salt, export Zeroizing, etc.).

**Out of immediate audit scope (pre-existing parity/feature roadmap; need Android build = blocked, or are net-new features, not audit bugs):**
- P1 lcmq/0qpn/vfai/f797/vjqc (Android sync protocol + Supabase schema + structural tests) — *not* in the audit's P1-1..P1-13 set; pre-existing Liquid-Glass/Android campaign beads.
- P2 dtq3/g3z4/ojsq/otb7/q649/2b1g/85n9 (Android cloud-transport model, bulk copy, localization, diagnostics, a11y, desktop history-limit, export/import UI) — feature work, tracked separately.

## Dependency-aware order
1. **Wave 1 (parallel, P1-first):** A, B, C, D, E, F, G — all disjoint files, launched together.
2. **Review gate:** orchestrator reviews each diff; runs pnpm + parity for F; reads Rust diffs for B/C/D/E/G.
3. **Wave 2 (sequential):** k89j, o8ew, P3 cluster.
4. **Final verification:** pnpm vitest/tsc/build, parity-check, cargo-audit/deny (if standalone runnable), doc link check.

## Master findings table
Status legend: `todo` · `in-progress` · `done-local` (fixed + locally verified) · `done-ci` (fixed, needs CI compile) · `deferred` · `out-of-scope` · `invalid`.

### P1 (13)
| Audit | bd | Title | Agent | Status | Tests |
|---|---|---|---|---|---|
| P1-1 | jbao | sensitive items pushed to relay/cloud/P2P vs guarantee | C | todo | daemon: sensitive never enters push channel |
| P1-2 | lszh | lsappinfo fail-open captures password managers | B | todo | tick test w/ stubbed failure |
| P1-3 | 26pd | blocking lsappinfo on async tick | B | todo | async tick test |
| P1-4 | oti6 | `unreachable!()` crash path daemon.rs:152 | B | todo | Open+Locked unit |
| P1-5 | 68uk | systemd ReadWritePaths macOS path on Linux | A | todo | n/a (config) |
| P1-6 | v6wh | CLI writes Keychain + plaintext pw over IPC | D | todo | daemon keychain verb; CLI no-send |
| P1-7 | wb2c | Tauri null CSP | F | todo | CSP present in conf |
| P1-8 | xxsw | Android raw DB key retained unzeroized | E | todo | open→close→no live key |
| P1-9 | ivqa | daemon Cargo.toml missing rust-version | A | todo | n/a |
| P1-10 | 9evm | version drift 0.7.1 vs 0.7.4 | A | todo | n/a |
| P1-11 | xmsz | docs/known-issues.md missing (dead link) | A | todo | link resolves |
| P1-12 | z5hl | README x86_64 vs arm64-only release | A | todo | n/a |
| P1-13 | x2c6 | protocol.md missing 3 error codes | A | todo | n/a |

### P2 (selected, audit-derived bugs)
| bd | Title | Agent | Status |
|---|---|---|---|
| ugv7 | startup TTL purge before socket bind | B | todo |
| fb3e | detector FP confidence < 0.70 | G | todo |
| r6cw | openai_legacy comment fix | G | todo |
| ozzt | new cloud-cred detector patterns | G | todo |
| ptb8 | IPC version_mismatch code | D | todo |
| 8u2b | INVALID_ARGUMENT on legacy IPC arms | D | todo |
| iqkm | 9 spawn_blocking key copies → Zeroizing | D | todo |
| tj9s | export audit log + include_sensitive flag | D | todo |
| jqcp | pair-qr --raw secret warning | D | todo |
| 44u4 | supabase_password IPC/config.json (P1-6 facet) | D | todo |
| ar2r | DB_BY_PATH evict on close | E | todo |
| 2ffx | Android eprintln → tracing | E | todo |
| vo79 | Liquid-Blue IdeSelection/IdeMultiSel #4D8DFF | E | todo |
| 7185 | relay unauth GET /devices | C | todo |
| 17lj | rewrite relay-api.md | A | todo |
| g4rs | update SECURITY.md to shipped design | A | todo |
| 2915 | ARCHITECTURE/README/protocol crate+method list | A | todo |
| 4rui | audit.yml retry split | A | todo |
| m7mm | #[allow] explanatory comments | A | todo |
| o8ew | ipc_win.rs orphan | Wave2 | deferred |
| k89j | tracing-subscriber out of core | Wave2 | deferred |
| 3e6g | default theme light-first | F | todo |
| 54h5 | sanitize DOM error strings | F | todo |
| 00ae | IMAGE chip color (confirm vs spec §6) | F | todo |
| dtq3,g3z4,ojsq,otb7,q649,2b1g,85n9 | Android/desktop feature parity | — | out-of-scope (feature roadmap) |

### P3 (24) — Wave 2 / documented
Handled where cheap & local (MSRV comments, ADR-010 path, security email placeholder, deny.toml stale ignore, ci-matrix scope). Rust-only P3 (empty-AAD `pub(crate)`, relay HKDF salt, export Zeroizing, PoP Zeroizing, Panicked Kotlin variant) → `done-ci` or deferred with rationale in `FIX_SUMMARY.md`.

## Coordination rules for agents this wave
- Edit only your owned files. Do **not** run `git`, `bd`, or `pnpm` (orchestrator owns those — avoids index/Dolt/permission races).
- Surgical edits; match surrounding style; no large rewrites.
- Add/adjust tests inline where the crate has a test module (tests document intent even though they can't compile here).
- Return a short report: files changed, what/why, any uncertainty, test added, and a self-flag if a change is risky without a compiler.

---

## FINAL STATUS (campaign complete — 2026-06-19)

All 7 Wave-1 agents (A–G) completed; Wave-2 o8ew done inline; k89j + Rust-P3 deferred-documented.

**Outcome:** 0 P0 · **13/13 P1 fixed** · **19 audit-derived P2 fixed** · P3 cheap-cluster done.
- `done-local` (verified here): P1-5,7,9,10,11,12,13 + UI P2 (3e6g/54h5/00ae) + all docs/CI P2 + parity 53/53 + UI 171/171.
- `done-ci` (implemented + diff-reviewed, **CI-COMPILE-REQUIRED**): P1-1,2,3,4,6,8 + detector/IPC/relay/android P2 + o8ew.
- `deferred` (no compiler / risk): **k89j** (cross-crate refactor), Rust-P3 (empty-AAD `pub(crate)`, relay HKDF salt [migration-sensitive], export/PoP `Zeroizing`).
- `follow-up filed`: **nq39** (P1-6 residual — dedicated IPC verb / non-macOS).
- `out-of-scope` (feature roadmap, Android build blocked): dtq3, g3z4, ojsq, otb7, q649, 2b1g, 85n9, qjfb; and pre-existing non-audit Android P1s lcmq/0qpn/vfai/f797/vjqc.
- `invalid/not-a-bug`: **00ae** (IMAGE chip sky is intentional per `1hqt` comment).

bd issues closed: jbao, lszh, 26pd, oti6, 68uk, v6wh, wb2c, xxsw, ivqa, 9evm, xmsz, z5hl, x2c6, 7185, 17lj, g4rs, 2915, 4rui, m7mm, fb3e, r6cw, ozzt, 3e6g, 54h5, 00ae, vo79, ar2r, 2ffx, 44u4, ptb8, 8u2b, iqkm, tj9s, jqcp, ugv7, o8ew. (k89j left open/deferred; nq39 new.)

See `FIX_SUMMARY.md` and `VERIFICATION_REPORT.md`. Nothing committed/pushed — working tree only.
