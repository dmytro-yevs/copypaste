# CopyPaste — Verification Report

**Date:** 2026-06-19 · Post audit-fix campaign. Verdict at bottom.

## Environment
- **Rust toolchain: ABSENT** — `cargo`, `rustc`, `rustup` not installed (`~/.cargo/bin` holds only the `cargo-audit`/`cargo-deny`/`cargo-ndk` plugins, which need the missing `cargo` driver). The original audit was blocked at rustc 1.95 < MSRV 1.96; this env has no toolchain at all.
- Node v24.7.0, pnpm 11.4.0, Java (OpenJDK) 26 present.

## Commands run

| Command | Result | Notes |
|---|---|---|
| `node scripts/parity-check.mjs` | ✅ **PASS 53/53** | re-run after Android `Color.kt` `#4D8DFF` change; still within ±5 |
| `pnpm -C crates/copypaste-ui run test` (vitest) | ✅ **171/171** (18 files) | 1 initial failure (`SettingsView.test.tsx:88` expected raw error text) was caused by the intentional P2-54h5 DOM sanitization; test updated to assert sanitized behavior → green |
| `pnpm -C crates/copypaste-ui run build` (`tsc && vite build`) | ✅ **clean** | TypeScript typecheck + production bundle OK; new CSP is config-only (no build impact) |
| `package.json` JSON validity + version | ✅ 0.7.4 | parsed via node |
| daemon `Cargo.toml` rust-version | ✅ present (line 6) | |
| android `Cargo.toml` coherence (A+E both edited) | ✅ sha2+tracing deps AND MSRV comment present, 101 lines, no corruption | |
| `docs/known-issues.md` exists (README link) | ✅ resolves | |
| cli `Cargo.toml` / source: no orphan `security_framework` | ✅ none | P1-6 left no dangling refs |
| `ipc.rs`: no bare `**self.local_key` copies remain | ✅ 0 remaining, 11 `Zeroizing::new(**self.local_key)` | iqkm |
| `mod ipc_win` declaration | ✅ added under `#[cfg(windows)]` | o8ew (was genuinely orphaned) |

### Could NOT run (environment-blocked)
| Command | Why |
|---|---|
| `cargo fmt --all --check` | no rustc/cargo |
| `cargo clippy --workspace --all-targets --all-features -D warnings` | no rustc/cargo |
| `cargo test --workspace --all-features` | no rustc/cargo |
| `cargo check` (`--no-default-features` / `--all-features`) | no rustc/cargo |
| `cargo deny check` | needs `cargo metadata` → cargo driver missing |
| `cargo-audit audit` | advisory-db is a broken partial clone; `--no-fetch` fails (`advisory-db/crates` missing). **No new external crates were introduced** (E's sha2/tracing are existing workspace deps already in Cargo.lock; D *removed* security-framework; G added regex patterns only) → the audit's prior **clean** verdict is unchanged by these edits |
| Android `./gradlew test/lint/assembleDebug` | `assembleDebug` needs the UniFFI `.so` (cargo-ndk + cargo) → blocked; also OOM guard |
| Tauri build/check | needs cargo for the Rust shell |

## Rust changes — review-only gate
With no compiler, every Rust diff was reviewed by reading. Specific verifications done:
- **P1-1:** filter lands before any crypto on all 5 outbound paths; cloud backlog marks items synced so the sweep terminates; inbound decrypt untouched. The `:1633/:1847` hardcoded `is_sensitive:false` were confirmed to be **test fixtures**, correctly left.
- **P1-4:** routed to the existing `run_degraded` path (same as `DbStartupPlan::Degraded`), no new panic.
- **ugv7:** confirmed `run_ttl_cleanup`'s `sensitive_ttl_ms` feeds **only** the sensitive branch; `do_general` calls `delete_expired(now_ms)` (per-item expiry) → **no general-history data loss** at startup.
- **P1-6:** verified no orphan `security_framework` references remain after dep removal.
- **iqkm:** verified 0 bare double-deref key copies remain.

### Open compile-risk flags (must be cleared by CI)
1. `ipc.rs` iqkm: `&Zeroizing<[u8;32]>` deref-coercion in some function-call contexts (agent made ambiguous spots explicit with `&*`).
2. relay/cloud filter: `item.id` vs `item.item_id` field names across the two structs.
3. daemon `ConfigError::Io` module path used in the new TOML-parse warn.
4. android: `Sha256::digest(...).into()` → `[u8;32]` (standard sha2 0.10 API).

## Production readiness verdict
**NOT yet production-ready from this environment alone — but materially safer, pending one CI run.**

- The verifiable surface (UI, parity, docs, config, version consistency, CSP) is **green**.
- All 13 P1 and 19 audit-derived P2 bugs are **implemented and diff-reviewed**, but the Rust subset (the majority) is **unbuilt**. Production-ready requires a single green run of `cargo fmt/clippy/test --workspace` on a ≥1.96 toolchain. Until then the Rust fixes are "implemented, unverified."
- **No changes were committed or pushed** — nothing ships unverified without an explicit gate.

**Exact gate to flip the verdict to ready:** on a ≥1.96 toolchain, run `cargo fmt --all --check && cargo clippy --workspace --all-targets --all-features -- -D warnings && cargo test --workspace --all-features`. If green, the campaign is production-ready (modulo the documented `nq39`/`k89j`/Android-feature remainders).
