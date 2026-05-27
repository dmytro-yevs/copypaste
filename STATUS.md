# CopyPaste — Project Status

**Generated:** 2026-05-27
**Branch:** main @ `8fb3fd9`
**Audit session:** full-stack audit + fix sweep (Waves A–G + Android+test followups)

---

## TL;DR

Audit aggregated **0 CRIT, 9 HIGH, 14 MED, 13 LOW** findings across security, network, UI, CI/CD, Android, and crypto. **Most HIGH and MED fixes are landed in main** (8 commits beyond `v0.3.2`). Local build/clippy/test are GREEN after the latest two follow-up commits (`aac14f0` t4 tests fix + `8fb3fd9` Android Kotlin fix). CI run that prompted the follow-ups (`26524293855`/`294638`/`294784`) is now **stale** — fresh CI is needed; not yet verified.

### Top blockers before a v0.3.3 tag

1. **CI runs from the latest push (`8fb3fd9`) not yet validated.** Local tests pass, but Android UniFFI hand-edit might be reverted by build-time codegen — see §Known Risks.
2. **Wave A.M1 — silent plaintext→SQLCipher migration** still unresolved. Awaiting product decision: TOFU (current behaviour) vs explicit `migrate` subcommand.
3. **Wave B.H6 — `tower_governor 0.4 → 0.8`** still pending. Dependabot PR exists; merge requires `KeyExtractor` API rewrite + a 429-response regression test.
4. **Wave C.H9 — "Private Mode" tray item not wired to daemon IPC.** Toggle only flips in-memory; lost on restart.

If those 4 are accepted as-is, ship `v0.3.3` as a security/CI hygiene release. Tag triggers `release.yml` → DMG + APK + Cask bump + GitHub release.

---

## What works (verified GREEN locally on `8fb3fd9`)

- `cargo build --workspace` — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — clean
- `cargo test --workspace` — all suites pass after t4 fix
- `cargo fmt --all -- --check` — clean
- App on macOS (visual + tray + history view) per recent `feature/ui-redesign-v04` merge
- IPC validation: UUID parse + size caps applied at all id/payload boundaries
- Crypto: AEAD with `(item_id, schema_version, key_version)`-bound AAD; HKDF-v2 with purpose-separated info strings; ZeroizeOnDrop on `DeviceKeypair`/`SharedSecret`/PAKE state
- Relay: bearer-token constant-time compare; per-(IP,device_id) sliding-window rate limit; new expiry check on `verify_token`
- P2P: mTLS both directions, fingerprint pinning, handshake timeout + ±50 ms jitter retry; new constant-time FP compare
- Storage: SQLCipher with crash-safe ATTACH/export/rename rekey; v4 key-version migration sweep with progress tracking
- Sync: `MAX_FRAME_SIZE` enforced; LWW deterministic on `lamport_ts > wall_time > origin_device_id`
- Android: UniFFI scaffolding panic-safe via `catch_result`; `targetSdk=35`/`compileSdk=35`; FGS_SPECIAL_USE has Play-Console-ready justification

---

## What is broken / unknown

### CI (3 workflows on the last validated push — may all be stale now)
- **CI Android Build** failed at `compileDebugKotlin` — fixed by `8fb3fd9` but the UniFFI generated-file edit is fragile (see Known Risks).
- **CI** + **Coverage** failed at `t4_force_complete_*` tests — fixed by `aac14f0`.
- **Release** workflow does not run on `push to main` — only on `git tag v*`. No release artifact exists for any post-`v0.3.2` commit.

### Pre-existing test failures fixed in this session
- `crates/copypaste-core/tests/key_version_tests.rs:413/462` — root cause: `Database::open_in_memory()` auto-seeds `migration_state` to Complete via `apply_migrations`, blocking the test's `INSERT OR IGNORE` InProgress seed. Fix: explicit `DELETE` before `INSERT`.

### Deferred audit findings (won't ship without explicit decision)

| ID | Domain | What | Why deferred |
|----|--------|------|--------------|
| A.M1 | Security | Silent plaintext→SQLCipher migration on `Database::open` | Product/security trade-off: TOFU is friendlier on upgrade, but lets an attacker who drops a plaintext file onto the path get it auto-encrypted under the current key |
| B.H6 | Relay | `tower_governor 0.4 → 0.8` API migration | Wide signature change (`KeyExtractor` rename); needs the dependabot branch reviewed alongside a 429-response regression test |
| C.H9 | UI | "Private Mode" tray IPC | Only affects in-session toggle; survives restart needs daemon-side state — low value vs scope |
| D.H5 | CI | `cargo audit` advisory-db pre-cache | Picked Option B (`--no-fetch || cargo audit`) as a minimal unblock; Option A (`actions/cache@v4` keyed by date) is the real fix |

### Tracked-elsewhere issues that bite this repo
- `ruflo` issue **#1425**: many MCP tools are stubs. `mcp__ruflo__agent_spawn` accepts no `swarmId` parameter — `swarm_init` and the agent registry are decoupled by design. TUI counter `Swarm N/15` stays at 0 even after correct dispatch; verify via `mcp__ruflo__agent_list` instead.
- `ruflo` issue **#1257**: SQLite contention with >8 concurrent agents — keep parallel agent cap at 8.
- `ruflo` issue **#640**: agents may self-report success when underlying tests fail — every CI/test claim cross-checked against real exit code in this session.

---

## What was changed in this audit (post-`v0.3.2`)

| Commit | Wave | Files | Highlights |
|--------|------|-------|------------|
| `8fb3fd9` | Android-Kotlin | 5 | `ic_menu_clipboard` → `ic_menu_edit`, `init {}` collapse, var→val Settings, override-fix on generated UniFFI file, suppressUnsupportedCompileSdk |
| `aac14f0` | t4-tests | 1 | DELETE-before-INSERT for `migration_state` seed in both `force_complete` tests |
| `858022b` | F+G | 4 | CT fingerprint compare in p2p verifier, subtle dep pull, `.gitignore` for ruflo session artifacts, Cargo.lock refresh |
| `20aacf9` | E (Android) | 3 | compileSdk/targetSdk → 35, FGS-SPECIAL-USE justification, db_handles mutex poison recovery ×2 |
| `2351162` | D (CI) | 8 | `@master` → `@stable` ×6 across 4 workflows, remove `gradlew assembleDebug` from release.yml, `cargo audit --no-fetch` fallback, drop `--features android-uniffi-live` on macOS runner, drop `windows-msvc` from deny.toml, add attest-build-provenance step, drop image crate `<0.25.10` cap, remove dead HOMEBREW_TAP_TOKEN |
| `8665a2e` | C+F (UI) | 5 | ClipItem.id stays SharedString end-to-end (no i32 trunc), home_dir().expect → propagated, keyboard ↵/⌘C/⌘P/⌘D wired to IPC, settings error → Slint banner, peer JSON parse → tracing::warn, .ok()-swallow → if-let-Err |
| `5e57325` | B (Relay) | 2 | `#![deny(clippy::await_holding_lock)]`, token expiry check in `verify_token`, new test `verify_token_expired_is_unauthorized` |
| `521d40a` | A (Security) | 5 | `#[deprecated]` on `secret_key_bytes` (use _zeroizing), `#[deprecated]` on empty-AAD `encrypt_item`/`decrypt_item`, `MAX_IMPORT_ITEM_BYTES = 4 MiB` pre-decode gate, drop redundant `Vec::clone` in `import` |

**Total:** 30 files touched, +198/-69, 8 commits.

---

## What still needs doing

### Immediate (before tag v0.3.3)

1. **Watch CI from latest push `8fb3fd9`** — confirm CI + Coverage + CI Android Build all GREEN.
2. **If Android UniFFI fix gets overwritten by codegen** — regenerate bindings properly via `cargo-ndk` + `uniffi-bindgen` flow, OR edit the `.udl` interface so the generated `message` field doesn't collide with `Throwable.message`.
3. **Tag `v0.3.3`** — `git tag -a v0.3.3 -m "Security + CI hygiene release"` then `git push origin v0.3.3`. release.yml fires → DMG + APK + Cask bump.
4. **Verify Cask update lands** — Homebrew Cask is the only distribution path on macOS.

### Short-term (next 1–2 weeks)

- **Wave A.M1 decision** — TOFU vs explicit `migrate` subcommand. Affects upgrade UX vs threat model.
- **Wave B.H6** — tower_governor 0.4 → 0.8 migration. Merge dependabot branch + add 429 regression test.
- **Wave C.H9** — Private Mode tray IPC. Pick a daemon endpoint and wire.
- **Wave D.H5 Option A** — `actions/cache@v4` for advisory-db. Less brittle than the current fetch-fallback.
- **Branch cleanup** — 14 zero-ahead branches (release/v0.1, v0.2, v0.3-dev, ui-redesign-v04, wave1b-*, wave3-*, post-v3-followups, cleanup-old-docs, windows-ipc-named-pipe), 3 ahead-but-superseded (wave1a-atomic already merged via `8e5c5e5`, fix/ci-postv032 Linux-only, fix/cask-app-name-v033 mostly Linux dupes — cherry-pick only `4b9366f`). Requires user confirm per project rule.
- **Dependabot remote branches** — 6 androidx patches SAFE to merge; AGP 9.2.1 + Kotlin 2.3.21 need REVIEW (major/K2 compat); rust dep bumps (tower 0.5, dirs 6, generic-array 1.4, criterion 0.8, rcgen 0.14, hkdf 0.13, etc.) review one-by-one.

### Medium-term (v0.4 horizon)

- Android Play Store submission prep: confirm targetSdk=35 doesn't break clipboard behavior on Android 15; finalize FGS justification copy.
- P2P discovery interface scoping (`if-addrs` filter for non-loopback/non-virtual) — Wave F.L12 stub left in plan.
- Slint runtime accessibility observation (reduced-motion OS toggle) — Wave F.L13 deferred.
- `tracing::warn` instead of `.ok()` swallow in remaining `crates/copypaste-ui/src/main.rs` window show/hide calls (Wave F.L7+L8 partially deferred for safety).

### Long-term (per release plan)

- v0.4 (~4–6 weeks): post-audit polish, deferred items above
- v1.0 (~2–3 weeks stabilization): OI-2 / cloud-sync still DEFERRED per existing release plan; no Windows; no Apple notarization (Cask handles distribution); no Sparkle

---

## Architecture snapshot

**Workspace:** 12 crates, Rust 2021, channel=stable (rust-toolchain.toml), MSRV implied 1.89 (Cargo.lock floor)

- `copypaste-core` — crypto (x25519, chacha20poly1305, hkdf, AEAD AAD, image chunks), storage (SQLCipher v6 schema, migration sweep), sensitive-content detection
- `copypaste-daemon` — IPC server (UUID-validated), macOS NSPasteboard glue, keychain integration
- `copypaste-cli` — CLI surface
- `copypaste-relay` — Axum bearer-token-auth fallback relay with sliding-window rate limit + DefaultBodyLimit + token expiry
- `copypaste-p2p` — mDNS-SD discovery + mTLS with rustls (ring), fingerprint-pinned TOFU
- `copypaste-sync` — frame-bounded LWW sync engine
- `copypaste-supabase` — cloud sync fallback (OI-2 DEFERRED)
- `copypaste-android` — UniFFI bindings (`#[uniffi::export]` + macro-generated FFI)
- `copypaste-ui` — Slint UI (HistoryView, SettingsView, DevicesView, PairView, AboutView, CommandPalette ⌘K), macOS tray + vibrancy
- `copypaste-ipc` — IPC schema
- `copypaste-bench` — Criterion benches
- `copypaste-telemetry` — opt-in telemetry stub

**Distribution:** macOS Homebrew Cask only (no Apple notarization, no Sparkle); Android sideload APK (no Play Store yet)

**Platforms supported:** macOS + Android. Windows FROZEN (cfg-only, no maintenance). Linux similarly frozen — recent `fix/ci-postv032` branch is Linux/Ubuntu CI fixes that won't be merged.

---

## Memory / decisions stored this session

Stored in ruflo memory (HNSW-indexed, cross-session recall):

| Namespace | Key | Purpose |
|-----------|-----|---------|
| decisions | `rule-swarm-first-dispatch-2026-05-27` | 2+ agents must use swarm_init + agent_spawn before native Agent() |
| decisions | `rule-cargo-allowed-2026-05-27-reversed` | Cargo allowed locally (reversed an earlier session ban) |
| reasoning-bank | `ruflo-swarm-agent-decoupling-2026-05-27` | swarm/agent registries decoupled by design; counter UI may stay 0 |
| bugs | `test-fail-key-version-tests-t4-20260527` | t4 test pre-existing root cause + fix landed in `aac14f0` |
| patterns | `merge-redesign-v04-to-main-2026-05-27` | prior ui-redesign-v04 merge (67 files, zero conflicts) |
| audit/main-2026-05-27/* | `audit-security`, `audit-ui`, `audit-network`, `audit-android`, `audit-cicd`, `wave-a-security-progress`, `wave-b-network-progress`, `wave-c-ui-progress`, `wave-d-cicd-progress`, `wave-e-android-progress`, `fix-android-kotlin`, `branch-assessment` | full audit + fix-wave trail |

Plan written: `~/.claude/superpowers/plans/2026-05-27-main-audit-fix.md`

---

## Open questions for product/owner

1. **Wave A.M1** — should plaintext→SQLCipher migration stay automatic (TOFU on first encrypted open) or move to an explicit `migrate` subcommand? Current behaviour is friendlier on upgrade but expands the threat model.
2. **v0.3.3 vs v0.4.0-beta.1** — tag bump scope. Picked v0.3.3 (patch) for this round per session decision.
3. **Branch cleanup confirmation** — 17 branches identified as DROP/CHERRY-PICK candidates; waiting on explicit go-ahead before deletion.
4. **UniFFI regeneration** — should the gradle build's `generateUniffiBindings` be wired so hand-edits aren't needed, or accept the manual override + add a CI check that flags drift?
5. **Dependabot dispatch** — auto-merge the 6 SAFE AndroidX patches now, or batch with v0.3.3?

---

*Generated by audit orchestrator. Cross-checked against ruflo memory + live git state.*
