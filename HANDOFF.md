# CopyPaste performance package — handoff

> **TEMPORARY FILE. DELETE IT once the work below has been picked up on the new machine.**
> `git rm HANDOFF.md && git commit -m "Drop the migration handoff"`
> It exists only to carry context across a machine swap. Anything here worth keeping
> permanently belongs in `docs/rewrite/performance.md` or an ADR, not in this file.

Everything below is self-contained on purpose. The working notes lived in a session
scratchpad on the old machine and do not travel, so the parts you need are restated here.

## Status in one line

A project-wide performance audit found 34 issues; 32 are fixed and pushed, one was measured
and deliberately reverted, one (F-LOCK-1) was never started. **Nothing on the final merged
tree was built, linted, formatted or tested** — local cargo runs were stopped so the machine
could be swapped. CI is the only gate. Expect to fix things.

## Exactly where it is

Pushed to `origin/main` as `d3956ca6..d019e6f3`: 38 commits, 118 files, +5717/-823, four
merges, zero conflicts. Clone and you have everything.

    git clone https://github.com/dmytro-yevs/copypaste
    git config core.hooksPath .githooks     # required once per clone
    git config commit.template .gitmessage

FIRST THING ON THE NEW MACHINE — nothing below matters until this passes:

    cargo build --workspace && cargo test --workspace
    cargo clippy --workspace --all-targets -- -D warnings
    cargo fmt --check && scripts/check-comments.sh && scripts/check-file-size.sh
    npx tsc --noEmit && npm run build && npx vitest run    # from crates/copypaste-ui

Then see what CI made of the push: `gh run list --limit 10`.

GitHub's 1 moderate Dependabot alert on the default branch is **decided — accepted, not
fixed.** It is `glib 0.18.5`, held by Tauri's Linux desktop GTK stack, which is a CI test
surface and never shipped. `.github/dependabot.yml` stops the weekly retry it could not
resolve, and [ADR-0014](docs/adr/0014-accept-the-glib-advisory-as-unshipped.md) records the
exposure and what would reverse it. The alert stays open in the Security tab until someone
dismisses it there.

The only dependency this work added was `sha2-asm 0.6.4`; `cargo audit` and `cargo deny`
were clean on it, and `Supply chain` is green on the final commit.

## What is where

- Remote: `github.com/dmytro-yevs/copypaste`
- Everything lands on **`main`**. Work branches exist locally but the intent is one merge,
  not a branch pile.
- `docs/rewrite/performance.md` was rewritten in-repo with re-taken measurements and
  travels with the clone. It is the durable record; sections 2.2–6 are current, and
  **section 3's old text-stage table was known-bad (totals did not reconcile) and was
  re-taken** — do not quote any figure you find that predates that.

## Measured results (criterion A/B, Apple M4, p=0.00 where stated)

| Change | Before | After |
|---|---|---|
| p2p converged round, 10k summaries | 5,140,332 B / 342.73 ms | **896 B / 1.86 ms** |
| …and bytes are now **constant** at 100/1k/10k | O(history) | **O(changes)** |
| p2p idle cost per peer | ~62 MB/hour | **~10.7 KB/hour** |
| SHA-256, 4 MiB | 27.24 ms / 146.9 MiB/s | **3.968 ms / 1.008 GiB/s** |
| SHA-256, 64 KiB | 348.9 µs | **48.63 µs** |
| FTS row lookup (SQLCipher, 2k rows) | 2.89 ms | **1.9 µs** |
| `summaries()` | 6.049 ms | **1.074 ms** |
| sensitive detector, 99 KB config text | 39.40 ms | **1.81 ms** |
| 200-item sync session at 8k rows | 987 ms | **467 ms** |
| 4 MiB text insert (regression we caused, then fixed) | 104.7 ms | **69.4 ms** |
| WAL bytes per 1 MiB capture | 2.11× payload | **1.08×** |
| UI idle list IPC, 3 pages held | 60/min | **20/min, page-count independent** |
| UI row renders per 5 status polls | 60 | **0** |
| First keystroke in search, 1000 items | 164.4 ms | **84.0 ms** |
| 200-item page payload | 477 KB | **272 KB** |

## Decisions that are not obvious from the diff

- **F-STOR-3 was reverted.** Moving the retention gate query outside the IMMEDIATE
  transaction measured 205 → 208 µs uncontended and 0.22 ms/capture contended — no win.
  The gate query *is* the cost; relocating it changes nothing. Do not reintroduce it.
- **Tombstones ride a per-peer relay floor, not a restamp.** The audit proposed restamping
  tombstones above the cursor; that churns between two devices that both hold the same
  tombstone. The relay floor also covers a wider hole the audit missed: *any* relayed
  version with an older stamp, not only deletes. Getting this wrong loses deletes
  permanently (manifest T-3, `CopyPaste-bfiu`).
- **`is_sensitive` write-time check changed shape.** The REG-1 fix inserts the FTS row
  before the item row, so the old SQL re-read had nothing to read. It is now a check on the
  values the same transaction is about to write — argued as strictly stronger under BEGIN
  IMMEDIATE. **This is the sensitive-items-never-reach-the-index boundary (rule 4). It was
  never independently reviewed. Review it.**
- **`sha2`'s aarch64 hardware path is gated on the Cargo `asm` feature alone** and ignores
  `target_feature`. `aarch64-apple-darwin` was reporting `target_feature=sha2` while still
  running the portable path, which is why two separate measurements showed portable speed
  on hardware that has the extension. Proof the fix took: `SHA256H/H2/SU0/SU1` instruction
  counts went 0 → 56 (macOS) and 0 → 24 (aarch64 Android object).

## Known-unfinished / needs attention

1. **Nothing on the final merged tree was built or tested.** Individual workers verified
   their own branches (1,141–1,263 tests green at various points), but the combination was
   not. Run the full gate first thing.
2. **Android CI has never once reached its reporting stage.** The `dependencyCheckAggregate`
   failure was a ClassLoader defect: `buildSrc` exports AGP's commons-compress 1.21 from a
   parent scope, so the 1.27.1 on the buildscript classpath never loaded (`ZipFile.builder()`
   arrived in 1.26.0). Fixed with an init script forcing 1.27.1 in buildSrc's own resolution.
   **If the next run goes red with an actual CVE list rather than a stack trace, that is the
   check working for the first time** — not a regression. There is no `NVD_API_KEY` secret;
   adding one cuts a cold update from ~50 min to ~11.
3. **macOS idle-daemon after-measurement was never taken.** The harness blocks on an
   interactive login-Keychain prompt and the only workaround changes the default keychain.
   Before-figure: 281.7 wakeups/min, 0.012 CPU-s/hour, 17 threads. So the two idle fixes
   (an always-on 30 s cleanup thread; a 10 s cloud-refresh tick on unconfigured daemons)
   remain argued, not measured, on the shipping platform.
4. **F-LOCK-1 was never started.** No `arc_swap`, `Cargo.toml` untouched. The finding:
   `config::set` holds the settings write lock across a SQLCipher KV write while the capture
   loop reads that lock on a reactor thread. Auditor C rated the impact low and it is the
   least valuable item in the package — but it is open.
5. **F-CORE-6 is committed unverified.** `insert_or_bump_late_sealed` evaluates the payload
   closure inside the transaction that already took the dedup decision, so a re-copy pays no
   HKDF, no AEAD pass and no plaintext clone; `is_sensitive`, `content_hash` and the
   AAD-bound item id stay eager. Before-baseline only (duplicate ingest p50, history 2000,
   contended host: 256 B 351 µs, 4 KiB 365 µs, 64 KiB 1448 µs, 1 MiB 24.1 ms) — there is no
   after number, and rustfmt never ran on those files.
6. **I-6 is in, and it nearly was not.** A cloud-applied older row must pull the p2p relay
   floor back. It was parked on a scratch branch because it could not compile until
   `pub(crate) fn cursors()` was widened to `pub` (`465121b5`, by the worker owning that
   file), so it was NOT in the first push. Merged afterwards as `43df70e`, and
   `cargo check -p copypaste-daemon` passes — the one build that was run, because a commit
   whose own message said "DOES NOT COMPILE" should not reach `main` unchecked. Its test
   (`applying_an_older_version_pulls_every_other_peer_back_to_it`, mirrored for the cloud
   path: peer-1 at 5,000, apply a cloud row stamped 1,000, assert `relay_floor_ms` is
   `Some(1000)`) has still never been executed. Run it.
   Note the wiring went into `StoreSource::apply_remote` in `cloud/source.rs`, NOT the
   `on_applied` closure in `daemon/src/sync.rs` that the original prescription named —
   `sync.rs` is the peer source and its own doc says the cloud transport deliberately does
   not take that hook.
7. **Residual, unfixed, measured:** the retention gate still runs
   `SELECT COUNT(*) ... WHERE deleted = 0` on every capture — a full index scan, 331 µs at
   8k rows. The fix is to make the gate cheap (a maintained counter, or
   `SELECT 1 ... LIMIT 1 OFFSET max_items`), not to move it.
8. **Storage benches cannot see large-payload write regressions** at ROW_BYTES=512. Only
   `capture/stage/*/insert` and the WAL-bytes test can. A future storage change can regress
   big writes with every storage bench green.
9. **`copypaste list --json` now returns bounded bodies** (1 KB preview). Export remains the
   full-body path. Behaviour change, belongs in release notes.
10. **The p2p wire protocol is now v3.** Devices on older builds stop syncing until both
   update. Deliberate under CLAUDE.md rule 3. Belongs in release notes.

## Release, when you get there

- Version lives in exactly three files: root `Cargo.toml`, `crates/copypaste-ui/package.json`,
  `crates/copypaste-ui/package-lock.json`. No bump script exists.
- Release fires on a pushed tag `v*`. Last good run was `v2.0.0-alpha.4`; it completed all
  six jobs including a signed Android APK and a smoke test of the published artifact, so the
  signing secrets exist and work. Target: `v2.0.0-alpha.5`.
- `cliff.toml` expects Conventional Commits but this repo's commit style has no type prefix
  (CLAUDE.md rule 10). Check what alpha.4's changelog entry actually looked like before
  relying on git-cliff.

## Working with Orca on the new machine

    orca status --json                      # runtime must be reachable first
    orca orchestration run-create --objective "..." --json
    orca orchestration task-create --spec "..." --json
    orca orchestration worker-start --task <id> --worktree new-child --name <slug> --agent claude --setup run --json
    orca orchestration check --wait --types worker_done,escalation,question --timeout-ms 900000 --json
    orca orchestration check --ack <delivery_id> --json
    orca orchestration reply --id <msg_id> --body "..." --json
    orca orchestration worker-release --dispatch <id> --json

Three things learned the hard way tonight, worth carrying:

- **`worker-start` returning `dispatch_input accepted` is NOT proof the prompt was
  submitted.** It failed silently three times, each with `terminal reused_agent_terminal`
  in the same receipt. Verify by checking the worker's transcript actually grew
  (`orca orchestration worker-read --dispatch <id> --limit 5`); on failure,
  `orca terminal wait --terminal <h> --for tui-idle`, then re-send the spec plus a
  lifecycle preamble with `orca terminal send --terminal <h> --text "..." --enter`.
- **One worker per file, always.** Every cross-boundary need became an escalation with an
  exact diff instead of an edit, and the six-way wave-1 merge had zero conflicts as a
  result. It is worth the extra round trips.
- **Remove each worktree as its branch merges**, with `orca worktree remove`, not raw git.
  ~30 worktrees each carrying a cargo `target/` filled a 460 GB volume twice; bash calls
  then fail with ENOSPC *before executing*, which is invisible from inside. The repo has
  `scripts/clean-target.sh` for cheap in-place reclaim — it drops the incremental cache and
  stale test binaries in seconds without forcing a rebuild.

## Machine-local things that do NOT travel

- `cargo-sweep` installed at `~/.cargo/bin` and a LaunchAgent at
  `~/Library/LaunchAgents/com.dmytro.cargo-sweep.plist` (daily 04:30, `--time 7`, logs to
  `~/Library/Logs/cargo-sweep.log`). macOS-only, machine-only. To reproduce elsewhere:
  `cargo install cargo-sweep` plus whatever scheduler that OS has.
- The audit reports, the ranked package and the integration ledger. Their conclusions are
  summarised above; the raw files are gone with the old machine unless separately copied.
