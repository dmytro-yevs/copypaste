# v0.3.0 Cleanup Audit — 2026-05-23

Read-only audit. v0.3.0 release will delete items below. Performed against
`release/v0.3.0-dev` (HEAD `2dc0a25`) in worktree
`/Users/dmytro/Documents/CopyPaste-v0.3-dev`.

Project policy: macOS + Android + Windows are supported runtimes. **Linux and
iOS are FROZEN/banned.** Anything mentioning them must be evaluated against
that policy; references that exist only because the same Cargo target triple
encodes Android-on-Linux-kernel (`aarch64-linux-android`) are NOT removable.

---

## Summary

- Crates to remove: **0** (all 13 are referenced; some are stubs but deliberately so)
- Modules to remove: **2** clear (≈40 LOC) + 1 review (~40 LOC)
- Docs to remove: **3** files (~265 LOC)
- Scripts to remove / merge: **2** orphan files
- Workflows to remove: **0** clear (1 review — `ci-matrix.yml` is partially redundant)
- Workspace deps to remove: **0** (workspace `[workspace.dependencies]` is empty — all
  dep entries live in per-crate Cargo.toml; nothing to prune at the workspace level)
- Crate-local deps: no clear-cut unused entries flagged for deletion (machete CI job
  already exists; trust its signal in v0.3 cleanup PR)
- Frozen-platform LOC to remove: **~50 LOC** (Linux stubs + iOS branch in telemetry)
- Build artefacts in git: **0** (gitignore is clean; only `builds/.gitkeep` is tracked)

### Top-10 deletion candidates (highest confidence)

| # | Path | Why | LOC |
|---|------|-----|-----|
| 1 | `docs/plans/windows-daemon-plan.md` | Plan executed; Windows daemon now builds in CI | ~600 |
| 2 | `docs/migrations/alpha-to-beta.md` + `scripts/migrate-alpha-to-beta.sh` | One-shot migration done; v0.3 deprecates alpha entirely | ~120 |
| 3 | `docs/ios-plan.md` | iOS is FROZEN per project policy | ~140 |
| 4 | `crates/copypaste-daemon/src/platform/linux.rs` | Linux backend stub for "Phase 5b" that policy says will never ship | 40 |
| 5 | `crates/copypaste-daemon/src/platform/mod.rs` line 53 | `#[cfg(target_os = "linux")] pub mod linux;` declaration | 1 |
| 6 | `crates/copypaste-telemetry/src/error.rs` Linux+iOS arms of `OsTag` (`Linux`, `Ios` enum variants + `cfg!` branches) | Frozen platforms in low-cardinality tag enum | ~6 |
| 7 | `scripts/find-cycle` (extension-less, empty file) | Replaced by `scripts/find-cycles.sh`; current file has 0 bytes | 1 |
| 8 | `scripts/check-macos-permissions.sh` | 0 callers anywhere (workflows, docs, scripts) | ~30 |
| 9 | `crates/copypaste-ipc/src/types.rs` | "Currently empty" placeholder; no consumer added it in 0.2; defer or delete | 7 |
| 10 | `crates/copypaste-core/src/crypto/mod.rs` body / `crates/copypaste-relay/src/api/mod.rs` / `crates/copypaste-relay/src/middleware/mod.rs` | Pure re-export stubs (1-line `pub use`); fine but flagged for review-before-delete | 3 |

---

## DELETE — High confidence (no callers, no refs)

### Crates

**None.** All 13 workspace members are referenced:

- `copypaste-core` — used by daemon, cli, sync, android, bench
- `copypaste-daemon` — bin, has lib.rs for integration tests
- `copypaste-cli` — bin, only depends on ipc + core
- `copypaste-ui` — bin, depends on ipc
- `copypaste-ipc` — used by daemon, cli, ui, bench
- `copypaste-p2p` — used by daemon
- `copypaste-sync` — used by daemon + bench
- `copypaste-relay` — standalone bin
- `copypaste-supabase` — gated behind `cloud-sync` feature on daemon
- `copypaste-android` — UniFFI lib for Android app
- `copypaste-bench` — criterion harness, separate package
- `copypaste-config` — has its own integration tests; **NOT** wired into daemon/cli/ui yet
  (only its own `tests/load_save.rs` consumes it). See REVIEW section.
- `copypaste-telemetry` — stub crate by design; consumed by `tests/opt_out.rs` only.
  No production wire-up yet. See REVIEW section.

### Modules

- `crates/copypaste-daemon/src/platform/linux.rs` (40 LOC)
  — Stub backend for "Phase 5b" using `unimplemented!()`. Linux runtime is FROZEN.
  Drop the file, drop the `#[cfg(target_os = "linux")] pub mod linux;` line in
  `crates/copypaste-daemon/src/platform/mod.rs:53`.

- `crates/copypaste-ipc/src/types.rs` (7 LOC, doc-only)
  — Doc says "Currently empty — added in this wave as a placeholder so consumer
  crates have a single module to extend when Wave 2/3 migrates payload structs."
  Wave 2/3 happened; consumers still pass `serde_json::Value`. Either populate it
  this release or drop it; carrying an empty placeholder file across versions is
  noise. Listed here because no Rust code outside this file references it.

### Docs

- `docs/plans/windows-daemon-plan.md`
  — Windows daemon now exists (`platform/windows.rs`, `ipc/windows.rs` per ci.yml
  matrix, sqlcipher+openssl cross-compile image at commit `ab6119f`). Plan
  document is executed; superseded by implementation.

- `docs/migrations/alpha-to-beta.md`
  — One-shot migration from 0.1.0-alpha → 0.2.0-beta. v0.3.0-dev is two versions
  past alpha; CHANGELOG no longer mentions alpha as a supported upgrade source.
  Pair with `scripts/migrate-alpha-to-beta.sh` deletion (below).

- `docs/ios-plan.md`
  — iOS is FROZEN. Document is "Status: Future (Phase 6)" planning notes.
  Project policy explicitly bans iOS work. If iOS ever returns, recover from git.

### Scripts

- `scripts/find-cycle` (extension-less, 0 bytes)
  — Genuine zero-byte orphan. The real script is `scripts/find-cycles.sh`
  (5794 bytes, 3 callers). Almost certainly a stale shell history artifact.

- `scripts/check-macos-permissions.sh`
  — Search shows 0 callers across `.github/`, `docs/`, `Makefile`, README, and
  other scripts. Useful as a developer one-liner but currently undiscoverable.
  Either wire it into `scripts/dev.sh` / Makefile or delete. Recommend delete
  unless a `make doctor` target is being planned.

- `scripts/migrate-alpha-to-beta.sh`
  — Companion to the migration doc above. Same reasoning; delete together.

### Workflows

**None to delete with high confidence.** All 11 workflows are wired to events
(push/PR/tag/schedule). See REVIEW for `ci-matrix.yml` overlap concern.

### Deps (Cargo.toml)

- **Workspace deps:** `[workspace.dependencies]` in root `Cargo.toml` is
  **empty/whitespace** — there is nothing to prune at the workspace level.
  All crate deps live in per-crate Cargo.toml files and are pinned individually.
  (This was likely intentional during the Rust 1.75 MSRV pinning work.)

- **Crate-local deps:** The `unused-deps` job in `nightly.yml` runs
  `cargo machete` with `continue-on-error: true`. Treat its next CI run as
  the authoritative pruning list rather than re-implementing the analysis
  here. No clearly-dead per-crate dep found by manual review.

### Frozen-platform code

- `crates/copypaste-daemon/src/platform/linux.rs` — entire file (see Modules above).
- `crates/copypaste-daemon/src/platform/mod.rs:53` — `#[cfg(target_os = "linux")] pub mod linux;`
- `crates/copypaste-telemetry/src/error.rs:79` — `cfg!(target_os = "linux")` arm + `OsTag::Linux` enum variant.
- `crates/copypaste-telemetry/src/error.rs:85` — `cfg!(target_os = "ios")` arm + `OsTag::Ios` enum variant.
- `crates/copypaste-daemon/src/logging.rs:151,172,216` — Linux log-path branches in
  what is otherwise a macOS-only crate (`#[cfg(target_os = "linux")]` for
  `~/.config/...` paths). Daemon is gated to macOS in `ci.yml`; these branches
  are dead at runtime. **Recommend remove**.
- `crates/copypaste-cli/src/commands/daemon.rs:64` — `unsupported_platform()`
  emits a friendly bail message for Linux pointing at `packaging/linux/copypaste-daemon.service`,
  **which does not exist** (`packaging/` only contains `packaging/macos/...`).
  The Linux branch references a non-existent file. Either delete the Linux branch
  or delete the dangling reference. Recommend: drop the Linux branch, keep Windows.
- `contrib/systemd/copypaste-daemon.service` + `contrib/systemd/install.sh` —
  systemd unit for Linux daemon. Linux is FROZEN. Recommend delete `contrib/systemd/`
  entirely. If you keep it for "community contrib"-style courtesy, add a top-of-file
  comment "UNSUPPORTED — Linux runtime is frozen per project policy" and move it
  under `contrib/unsupported/`.
- `docker/Dockerfile.linux` — explicitly self-documents as "exists ONLY for
  cross-build sanity checks." **Keep** (it does cross-build a real artefact),
  but the project should decide whether sanity-build of a frozen platform is
  worth ~5min CI per nightly run. Recommend keep, document, do not ship.
- `.github/workflows/ci-matrix.yml` — runs `ubuntu-latest` + `macos-14` matrix.
  Ubuntu runner is fine (it's the host OS, not a runtime target); the daemon
  crate is excluded from the Windows job in `ci.yml` but `ci-matrix.yml` runs
  `cargo test --workspace` unconditionally on Ubuntu, which will fail any
  test referencing macOS-only crates. **Either narrow to `-p copypaste-core
  -p copypaste-cli -p copypaste-relay -p copypaste-config -p copypaste-ipc
  -p copypaste-sync -p copypaste-supabase -p copypaste-telemetry` or delete
  the Ubuntu row of the matrix.** Workflow filters on
  `branches: [release/v0.2.0-beta]` which is also stale — branch is now
  `release/v0.3.0-dev`. **High confidence the branch filter is stale**;
  this workflow has been silently not-running on v0.3 PRs.

---

## REVIEW — Needs human decision

### Borderline cases

- `crates/copypaste-config` — Has good integration tests but **no production
  consumer**. The daemon does its own config loading; CLI parses its own flags
  via clap; UI reads Slint properties. Either:
  (a) wire `AppConfig` into daemon `paths::Settings` (its declared purpose), OR
  (b) flag the crate as "future use" with a `publish = false` and a TODO, OR
  (c) delete the crate and reclaim 425 LOC.
  Recommend: **(a) wire it in v0.3 OR (c) delete**. Carrying a tested-but-unused
  crate across another release is technical debt.

- `crates/copypaste-telemetry` — Stub crate (343 LOC) by design ("not implemented
  in 0.2-beta"). No production consumer; only its own opt-out test imports it.
  Status: keep if 0.3 wires Sentry; delete + reintroduce later if 0.3 also won't
  wire it. Recommend: **defer one release, then delete if still unused**.

- `crates/copypaste-bench/src/lib.rs` — empty by design (benches live in
  `benches/*.rs`). **Keep**; this is a valid criterion-harness layout.

- `crates/copypaste-android/build.rs` (3 LOC) + `uniffi-bindgen.rs` (3 LOC) —
  UniFFI scaffolding; **keep**.

- `crates/copypaste-core/src/crypto/mod.rs` (4 LOC) — looks empty but is a
  re-export module gating the encrypt/decrypt submodules. **Keep**.

- `crates/copypaste-relay/src/api/mod.rs` and
  `crates/copypaste-relay/src/middleware/mod.rs` (1 LOC each) — `mod metrics;`
  and `mod whatever;` declarations. **Keep**.

- `scripts/dev.sh` — Only callers are itself + (probably) developer muscle
  memory. Useful as a developer shortcut. Doc it in README or delete; do not
  carry undocumented dev shortcuts.

- `scripts/fuzz-seed.sh` — One caller; supports cargo-fuzz workflow. **Keep**.

- `scripts/release/_sign-and-dmg.sh` — One caller (`build-dmg-ci.sh`); helper.
  **Keep**.

- `scripts/build-android.sh` vs `scripts/build-android-pkg.sh` — Both exist,
  **both serve different purposes** documented in `scripts/build/README.md`:
  - `build-android.sh` → writes `.so` to `android/app/src/main/jniLibs/` for
    Gradle (developer workflow), also regenerates UniFFI bindings.
  - `build-android-pkg.sh` → writes to `builds/android-<abi>/` for distribution.
  CI uses neither directly (it calls `cargo ndk` inline). Recommend: **keep
  both, but consider consolidating into one script with a `--target` flag** in
  a follow-up. Not blocking for v0.3.

### Historical archives

- `reports/` — directory exists but is **empty**. No archive to evaluate.
  Remove the directory entirely or commit a `.gitkeep` with documented purpose.

- `CHANGELOG.md` — only 34 lines, contains `[0.3.0-dev]` (empty) and
  `[0.1.0-alpha.1]`. The 0.2-beta entry is missing — likely intentional
  (cliff/release flow generates it from git on tag push). **Keep as-is**;
  adding an empty "Unreleased" header is correct git-cliff workflow.

- `docs/architectural-debt.md` — register of post-alpha debt. Living document.
  **Keep**.

---

## KEEP — Confirmed used (don't delete)

Brief confirmations for the items most likely to be flagged by an over-eager
cleanup pass:

- `crates/copypaste-telemetry`: imported by its own tests, stub crate by intent.
  Keep one more release, reassess (see REVIEW).
- `crates/copypaste-bench/src/lib.rs` empty: criterion convention, not a stub.
- `crates/copypaste-relay/src/{api,middleware}/mod.rs` 1-liners: legitimate
  module-declaration files, not stubs.
- `crates/copypaste-supabase`: optional dep on daemon under `cloud-sync` feature.
- `crates/copypaste-android/{build.rs,uniffi-bindgen.rs}` 3-line files: UniFFI
  scaffolding, not stubs.
- `docs/relay-v2-quotas-plan.md`: describes the *implemented* relay v2 design
  (rate-limit + quotas); reference doc, not a TODO list. Keep.
- `docs/known-issues.md`, `docs/architectural-debt.md`: living documents.
- `docker/Dockerfile.linux`: self-documents as cross-build-sanity-only; not for
  distribution. Keep but do not promote.
- `Cargo.lock` (218 KB): correct for an app/workspace; do **not** add to
  `.gitignore`.
- `.devcontainer/`: VSCode dev container; keep.
- `Casks/copypaste.rb`: Homebrew cask source; keep.
- `launch/`, `packaging/macos/`, `man/`: distribution artefacts; keep.
- All 11 workflows in `.github/workflows/` (modulo `ci-matrix.yml` branch-filter
  fix called out above).

---

## Suggested cleanup PR sequence

Apply in order; re-run `cargo test --workspace` after each stage.

1. **Stage 1 — safe doc/script cleanup (no code, no semantics):**
   - Delete `docs/plans/windows-daemon-plan.md`
   - Delete `docs/ios-plan.md`
   - Delete `docs/migrations/alpha-to-beta.md`
   - Delete `scripts/migrate-alpha-to-beta.sh`
   - Delete `scripts/find-cycle` (empty 0-byte file)
   - Delete `scripts/check-macos-permissions.sh` (or wire it into `dev.sh`)
   - Remove empty `reports/` directory

2. **Stage 2 — frozen-platform code:**
   - Delete `crates/copypaste-daemon/src/platform/linux.rs`
   - Remove `#[cfg(target_os = "linux")] pub mod linux;` from
     `crates/copypaste-daemon/src/platform/mod.rs:53`
   - Strip Linux branches from `crates/copypaste-daemon/src/logging.rs:151,172,216`
   - Strip Linux branch from `crates/copypaste-cli/src/commands/daemon.rs:64`
     (removes the dead reference to non-existent `packaging/linux/...`)
   - Remove `OsTag::Linux`, `OsTag::Ios` variants and their `cfg!` arms from
     `crates/copypaste-telemetry/src/error.rs:79,85` (breaking API change —
     bump telemetry crate version; not consumed in production so impact = 0)
   - Decide: delete `contrib/systemd/` or move to `contrib/unsupported/`
   - Re-run `cargo test --workspace` on macOS — expect green

3. **Stage 3 — workflow hygiene:**
   - Fix `ci-matrix.yml` branch filter
     (`release/v0.2.0-beta` → `release/v0.3.0-dev` or just `release/**`)
   - Either narrow its Ubuntu matrix row to cross-platform crates only OR
     drop the Ubuntu row entirely

4. **Stage 4 — borderline modules (one-by-one, with consumer wired or removed):**
   - Decide on `crates/copypaste-config` (wire-in OR delete)
   - Decide on `crates/copypaste-telemetry` (defer OR delete)
   - Decide on `crates/copypaste-ipc/src/types.rs` (populate OR delete)
   - For each: a separate commit so revert is surgical

5. **Stage 5 — verification:**
   - `cargo test --workspace`
   - `cargo build --workspace --release`
   - `bash scripts/build-all.sh` (host platforms)
   - `cargo machete` (let it flag any deps that the manual audit missed)
   - `cargo +nightly udeps --workspace` if nightly available
   - `git grep -i tauri` — confirm zero matches (already true at HEAD)
   - `git grep -i 'target_os = "linux"'` — confirm only `docker/Dockerfile.linux`
     and `scripts/*` Android-target-triple references remain

---

## Estimated impact

- **LOC reduction:** ~800 LOC (mostly docs: ~600 plan + 140 ios + 120 migration)
- **Rust LOC reduction:** ~50 LOC (Linux stubs + telemetry frozen-platform arms)
- **Crate count:** 13 (unchanged) — or 11 if config + telemetry both deleted
- **Build time (cargo check):** Negligible improvement from this audit alone;
  most savings come from the Stage 4 borderline crate decision (≈10–15% if both
  deleted)
- **CI minute reduction:** ~5 min/nightly if `ci-matrix.yml` Ubuntu row is
  pruned or its scope narrowed; otherwise neutral
- **Risk:** Stage 1 has near-zero risk. Stage 2 risk is *only* on Linux build
  paths which CI does not currently exercise as a runtime target. Stage 3 is
  CI-config only. Stage 4 is real refactoring — keep behind feature work.

---

## Audit method

For each section above:

- Crate refs verified with `grep -rn 'path = ' crates/*/Cargo.toml`
- Module usage verified with `grep -rn 'pub mod' src/lib.rs` per crate + module-name
  search across workspace
- Frozen-platform code via `grep -rn 'target_os = "linux"' --include='*.rs' crates/`
  and `target_os = "ios"` equivalents
- Script callers: per-script `grep -rln <basename> --exclude-dir=target --exclude-dir=.git`
  counted across workflows, docs, Makefile, README, and other scripts
- Workflow events: each `.github/workflows/*.yml` `on:` trigger inspected
- Tracked build artefacts: `git ls-files | grep -E '(target|builds|dist|node_modules)/'`
  returned only `builds/.gitkeep` (intentional)
- Tauri scrub: `grep -rln -i tauri` returned **zero matches** at HEAD (confirms commit
  `135f352` was thorough)
