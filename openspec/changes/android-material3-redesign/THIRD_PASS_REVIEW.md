# Third-pass review

Verdict: **the architecture is now substantially correct; one reconciliation pass remains before
S0 may begin**.

Most findings from `POST_UPDATE_REVIEW.md` are resolved. The remaining problems are mainly stale
statements that contradict the newly repaired specs, plus two incomplete inventory/gate claims.

## 1. Blocking internal contradictions

### 1.1 Remove every remaining `Save→recreate` instruction

The correct decision is now D5/application-scoped observable committed appearance state. These
old statements still contradict it:

- `proposal.md` capability summary: `Save→recreate`
- `design.md` Goals: `Save→recreate`
- `design.md` R6: discusses calling `recreate()` on every appearance Save
- `tasks.md` 3.3: `Save→persist then recreate()`

Replace them with Save→commit→publish committed appearance state. If `recreate()` remains useful
for a specific unrelated setting, state that separately; it must not be the appearance
propagation mechanism.

### 1.2 Reconcile the content-kind/color counts everywhere

The repaired specs correctly define:

- 12 supported kinds;
- 10 unique content-color fields;
- PHONE→`cNum` and PATH→`cFile` aliases.

Stale text remains:

- `proposal.md`: "11 content-type colors"
- `design.md` D2: "11 content-type colors"
- `tasks.md` 5.1: "11 c-* colors"

Change all three to "12 kinds mapped to 10 canonical content colors".

### 1.3 Remove the obsolete blur wording from `android-design-system`

`design.md` D7 and `android-navigation-chrome/spec.md` now correctly reject `Modifier.blur` on
the pill's own layer. However `android-design-system/spec.md` still says real blur may be
implemented with `RenderEffect`/`Modifier.blur`/`graphicsLayer` without distinguishing backdrop
capture.

Rewrite that requirement to reference the proven backdrop-capture policy and S0 spike. It must
explicitly reject own-layer blur for a frosted surface, while allowing own-layer blur only for
cases where blurring the content itself is actually intended (for example sensitive masking).

### 1.4 Reconcile migration wording

The new `android-appearance` spec correctly requires canonical keys `theme_mode`/`accent` and a
versioned migration that stops deleting them. Stale ambiguous wording remains:

- `design.md` D6: "use migration-safe keys or drop ..."
- `design.md` R5: "migration-safe keys"
- `tasks.md` 3.1/3.4: "migration-safe keys"

This can be misread as renaming the canonical keys. Replace it with the precise chosen contract:
version/update the migration latch, remove only legacy keys, retain `theme_mode` and `accent`, and
run before first appearance read.

### 1.5 Remove resolved "open decision" references

The specs already resolve both decisions:

- Share receiver stays UI-less.
- ZXing activity is accepted without `FLAG_SECURE` with an explicit ownership/privacy boundary.

Stale task text remains:

- `tasks.md` 8.3: `open decision #5`
- `tasks.md` 12.2: `open dec #3`

Mark both resolved and reference their capability specs. Do not leave phantom decision numbers.

### 1.6 Fix Paparazzi ownership in the traceability table

The task plan now correctly establishes Paparazzi in S2 and requires every later screen slice to
add its own baseline. The bottom traceability table still assigns golden infrastructure to S14,
and the following sentence says S14 attaches every preview/golden.

Correct model:

- S0: compatibility spike/version/storage decision;
- S2: framework/config/catalog/baseline policy established;
- S4-S13: owning slice adds fixtures and baselines for its surfaces;
- S14: coverage/matrix audit and gaps only.

Update the table and explanatory sentence accordingly.

## 2. Inventory accuracy issues

### 2.1 Component inventory counts are off by one

`component-inventory.md` claims 118 `@Composable` functions + 12 Activities. Repository checks
currently find:

- 117 `@Composable` annotations;
- 13 Activity subclasses.

The 13 Activities are Main, Onboarding, History, Pair, PortraitCapture, Settings, ShareReceiver,
LogViewer, PermissionsSettings, About, BackgroundCaptureSetup, ClipboardFloating and Devices.

Either fix the counts or document the extraction rule that intentionally excludes one Activity
and includes one non-function composable. A "complete enumeration" must be reproducible.

### 2.2 The state inventory is still representative, not complete

`tasks.md` now labels its rows "Representative" and says the full per-screen list lives in each
slice, but no separate full state-to-evidence table currently exists. `component-inventory.md`
also says state coverage is tracked in `tasks.md`.

Before S0 closes, require a complete table with:

`state → existing/new → trigger/data fixture → preview fixture → golden → automated test → manual
check/N/A`.

This does not need to enumerate every color combination, but it must enumerate every reachable
loading/empty/error/disabled/masked/dialog/in-flight state. The present representative table is
not sufficient to prove "absolutely everything".

### 2.3 Correct the golden-infrastructure row in `component-inventory.md`

If component inventory continues to group golden infra under S14 or refers generically to later
attachment, align it with the S0/S2/per-screen/S14 lifecycle above.

## 3. Decisions that may remain as S0 spikes

These no longer need to block approval of the specification structure, provided S0 cannot close
without resolving them and S1 depends on S0:

- exact Lucide Maven coordinate/version or curated generated subset;
- exact Paparazzi version and whether a toolchain upgrade is required;
- direct PNG versus Git LFS baseline storage;
- measured backdrop-capture implementation/fallback.

However, remove claims such as "Paparazzi verified compatible" until the S0 proof actually passes.
The current `design.md` baseline and R8 still overstate compatibility while the exact version is
explicitly unresolved.

## 4. Minor but worthwhile cleanup

- Replace CSS vocabulary (`--token`, px, `currentColor`) in Android SHALL text with Kotlin semantic
  property names and dp/sp; retain the CSS name only as a style-guide cross-reference.
- D2 still summarizes M3 mapping as `surfaceContainer*=...`; point to the exact role table instead
  of keeping a second, less precise mapping.
- The navigation landscape scenario is acceptable only as a functional fallback. Keep it clearly
  outside visual/golden acceptance, as the latest spec now does.
- `component-inventory.md` says the error/degraded History state is new. Ensure S5 explicitly owns
  the presentation-state plumbing and does not change repository/IPC behaviour.
- Add the exact connected-test command/emulator ownership once the runner is selected; generic
  "Compose semantics/a11y tests" is not enough for CI execution.
- Keep the pre-existing `docs/design/copypaste-app-demo.html` deletion out of every spec/slice
  commit and verification diff.

## 5. Readiness decision

After sections 1 and 2 are repaired, the change is ready to start **S0 only**.

S1 must remain blocked until S0 records:

1. Lucide source/version and license/SBOM handling;
2. Paparazzi compatibility proof and baseline storage policy;
3. backdrop-blur prototype result and fallback;
4. complete state/evidence inventory;
5. exact gate commands and serial build ownership.

Do not require all implementation details before S0 — those spikes are legitimately part of S0.
Do require every contradictory old statement to be removed before the repaired OpenSpec is committed.
