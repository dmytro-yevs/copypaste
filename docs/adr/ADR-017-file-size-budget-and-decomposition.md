# ADR-017: File-size budget and god-file decomposition methodology

## Status

Proposed
Date: 2026-06-30

## Context

The codebase has accumulated ~25 production source files over 1000 lines and
~20 more in the 800–999 band (excluding naturally-large test files and
generated UniFFI bindings). The worst offenders mix many responsibilities in a
single file — e.g. `daemon/p2p/mod.rs` (2415), `ClipboardService.kt` (2111),
`daemon/capture.rs` (2061), `ipc/handlers_items.rs` (1892), `Settings.kt`
(1888).

Large multi-responsibility files hurt in concrete ways: they are hard to hold in
context (for humans and for AI agents), edits are riskier because unrelated
concerns share a blast radius, tests are coarse, and reuse is discouraged
because cohesive logic is buried among unrelated code. A prior cleanup
(`CopyPaste-c4q2`, closed) split the original `ipc.rs` (~12.5K lines) into the
current `ipc/handlers_*` modules — proof the pattern works, but the campaign was
never generalized into a standard.

Constraints: behavior must be preserved (this is refactoring, not redesign);
crate-boundary rules in CLAUDE.md still hold (UI/CLI never link
`copypaste-core`; relay handles ciphertext only); `-D warnings` and MSRV 1.96
remain enforced; tests are the only safety net and some Android tests were
recently deleted, so coverage gaps must be closed before moving code.

## Decision

We adopt a **soft file-size budget** and a **standard decomposition methodology**
for splitting god-files.

**Size budget (guidance, not a hard CI gate initially):**
- Target: production source files ≤ 500 lines.
- Warn band: 500–800 lines — acceptable if the file is genuinely one cohesive
  responsibility; otherwise schedule a split.
- Action band: > 800 lines — a decomposition issue must exist.
- Excluded: test files (`*tests.rs`, `tests/`, `androidTest/`), generated code
  (UniFFI `copypaste_android.kt`, `CopypasteBindings.kt`), and data tables
  (large `match`/schema literals) where splitting harms readability — these are
  exempt but must be annotated with a one-line `// size-exempt: <reason>`.

**Decomposition methodology — every split follows these phases:**

1. **Characterize.** Inventory the file's distinct responsibilities and the
   cohesion clusters (groups of types/functions that change together).
2. **Establish the safety net.** Confirm tests cover the behavior to be moved.
   If coverage is thin (especially post-deletion Android code), write
   characterization tests FIRST and commit them before touching structure.
3. **Identify seams.** Choose split boundaries along responsibility lines, not
   technical layers. Files that change together stay together.
4. **Extract, one cluster at a time.** Move one cohesive cluster into a focused
   module/class; the original file keeps a thin orchestration/facade shell that
   re-exports or delegates. Compile + test + commit after each extraction.
5. **Dedup / reuse.** When extraction reveals logic duplicated across files
   (e.g. HTTP retry in `relay/mod.rs` vs `cloud/push.rs`, mapping plumbing
   across the Android data layer), promote it to a shared helper rather than
   copying.
6. **Verify behavior-preserving.** Full test suite green (`cargo test
   --workspace` / `./gradlew :app:testDebug`), `cargo clippy -- -D warnings`
   clean, no public-API/wire-contract change unless explicitly intended.

**Language-specific guidance:**
- *Rust:* split a `mod.rs` into submodules under the same directory; keep
  `mod.rs` as a thin re-export + orchestration shell. Preserve `pub(crate)`
  visibility; do not widen public API to make extraction easier.
- *Kotlin/Android:* keep `Activity`/`Service` classes thin Android-lifecycle
  shells; extract collaborators (managers, repositories, mappers, Composable
  components) and wire via constructor injection. Separate state/persistence
  from Composable UI. Prefer composition over inheritance.

**Process:** the campaign is tracked under a single bd epic with one child issue
per file (prioritized by size × risk), grouped into tracks
(daemon-p2p/sync, daemon-ipc, core-storage, android-service, android-ui, relay/
cloud). The top offenders carry a concrete split sketch; the long tail carries a
high-level note and gets a detailed sketch when claimed.

## Consequences

- **Positive:** smaller, single-responsibility files; lower edit blast radius;
  better test granularity; reuse surfaces extracted as shared helpers; AI agents
  can hold whole files in context, improving edit reliability.
- **Positive:** a repeatable methodology means each split is mechanical and
  low-risk rather than an ad-hoc rewrite.
- **Negative / cost:** large number of small behavior-preserving commits and
  review effort; temporary churn in `git blame`; risk of regressions where the
  safety net is thin (mitigated by phase 2 — tests first).
- **Neutral:** the size budget is guidance, not a CI gate, to avoid gaming
  (artificial splits). A CI warn-only check may be added later once the backlog
  is drained.
- **Security:** splits must not alter AEAD AAD binding, constant-time
  comparisons, or crate-boundary rules; these are explicit review checkpoints in
  the affected issues (capture.rs, relay/mod.rs, ipc handlers, crypto).

## Alternatives Considered

- **Hard CI line-count gate now** — rejected: would force artificial splits and
  block unrelated work before the backlog is drained; revisit after the campaign.
- **Big-bang rewrite of each god-file** — rejected: high regression risk,
  unreviewable diffs, violates behavior-preserving constraint.
- **Leave as-is / do nothing** — rejected: the files keep growing and the prior
  `ipc.rs` split already demonstrated the maintenance and agent-context payoff.
- **Split purely by technical layer (all handlers / all types / all UI)** —
  rejected: scatters cohesive logic; we split by responsibility so files that
  change together live together.
