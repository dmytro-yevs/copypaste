# GitHub Actions systemic failure audit

Date: 2026-08-30
Repository: `dmytro-yevs/copypaste`
Scope: failed, cancelled, and queued GitHub Actions work observed during
2026-07-27..2026-08-27, with a fresh validation sample on
`10c3a8f494a83bec305258865a780ab3872fed5b`.

This is an immutable recovery snapshot, not a live recount. Statuses below
are bound to the named commit, ref, and run IDs captured on 2026-08-30.

## Executive finding

The verified closed-day inventory contains 1,764 runs: 929 successful, 376
failed, 458 cancelled, and 1 queued. It covers 32 daily segments and 39 API
pages; the largest segment contained 269 runs. There were zero duplicate rows,
out-of-cutoff rows, or segment-count mismatches. Coverage is complete for the
enumerated run rows, not an individual causal review of all 376 failures.

The raw run inventory is preserved verbatim as
[2026-08-30-github-actions-runs.tsv](2026-08-30-github-actions-runs.tsv)
(602,063 bytes; SHA256
`34c290a55af777cb62073a2954d116c99f417bf00c9d49b22cadcff5a039937b`). The
artifact is a mechanical, no-reformat copy from the verified source; byte
comparison and SHA256 equality both passed. The companion `workflows.tsv` and
`segments.tsv` files supplied the aggregates and coverage checks. An earlier,
partial inherited snapshot recorded 1,700 runs
(906 successful, 341 failed, 452 cancelled, 1 queued), 426 automatic, 19
manual, and 7 Android-timeout cases; those figures are not directly comparable
to this complete closed-day inventory.

The earlier snapshot's 82.862 elapsed cancelled-job hours are retained as an
initial-snapshot-only measurement. No duration is assigned here to the fresh
458 cancellations, and elapsed hours are not billable usage.

All 12 workflows with nonzero failure conclusions are listed below. The counts
sum to 376; they are inventory counts, not a claim that every failure has been
causally classified.

| Workflow | Failures |
| --- | ---: |
| CI | 102 |
| Android emulator | 74 |
| Browser (WebKitGTK, Linux) | 49 |
| Windows native desktop E2E | 40 |
| E2E (real WebView) | 34 |
| Release | 22 |
| Nightly native matrix | 19 |
| Supply chain | 13 |
| GitHub Advanced Security | 9 |
| Mutation gate | 8 |
| Dependabot Updates | 3 |
| Nightly | 3 |

The root families below combine this verified inventory with retained findings
and source/workflow inspection.

The systemic pattern is evidence loss: reusable-workflow expansion duplicated
Android work; platform assertions observed stale or non-authoritative surfaces;
and local gates could diverge from CI. Event-aware concurrency now addresses a
latent manual/schedule/main collision while preserving intentional same-ref
push supersession. The remedy is to make event identity, ownership, readiness,
and evidence provenance executable. No timeout increase, retry added solely to
hide a defect, skipped assertion, or security weakening is an acceptable fix.

## Root families and systemic controls

### 1. Cancellation, queueing, and duplicate fan-out

The old groups used `github.ref`, which kept pull-request merge refs distinct
from `main`; it did not prove a PR/main conflation. The 426 automatic
cancellations were largely intentional same-ref push supersessions. A separate
latent risk was that manual and scheduled runs on the same default ref could
collide, while the called Android workflow demonstrably expanded a five-API
sweep once per matrixed caller.

Implemented controls:

- `72890c651` makes action groups event-aware and cancels only superseded
  push/PR work; `e67b00acc` applies the same semantics to release and nightly
  workflows.
- `6d3d21ac0` calls the scheduled Android sweep once; `d709fbb13` isolates
  nightly evidence from manual dispatch.
- The wiring checker and mutation fixtures reject a missing event discriminator,
  an unsafe cancellation policy, and the duplicated scheduled shape.

Status: implemented in the tested baseline. The fresh inventory records 458
cancellations but computes no duration for them. The changed wiring is
validated through the workflow and mutation gates; the historical cancellation
total is not attributed to the latent collision without per-run evidence.

### 2. Tooling and gate drift

The dependency gate previously depended on a Docker-oriented action, creating a
runner/tooling failure mode unrelated to dependency health. Workflow shards,
artifact names, and local-vs-CI checks also needed structural ownership.

Implemented controls:

- `6d7314cea` installs pinned `cargo-deny@0.20.2` with checksum verification
  and runs the four checks directly with `--locked --show-stats`.
- `check-wiring.py` and its mutation fixtures validate workflow shape,
  producer/consumer wiring, toolchain pins, Android matrices, and workspace
  shard coverage. The intended policy is one executable gate definition shared
  by local and CI entry points.

Status: Supply chain run 33136251962 is an overall FAILURE because its secret
scan/gitleaks gate failed; dependency review, cargo-deny, and cargo-audit were
successful. `f6494b413` reviewed and integrated the eight exact HEAD-history
fingerprints. The all-public-refs correction was reviewed and integrated as
`bd29cd710` (from `f50a16b54`), covering archive/tag fixtures including
`75ba2a72d`; the default clone's 2,859-commit all-refs scan is clean. That
later source scan does not retroactively change the captured workflow result
or claim that a scheduled workflow is green.

### 3. Readiness and contract observation

Several red jobs were not proof of a product regression: they were assertions
against stale state, a transient skeleton, or a control whose semantics were
inferred from visible text. The corrective direction is to wait on and assert
the authoritative state, and to fail when the required accessibility contract
cannot be observed.

Implemented controls:

- `b0bd35874` and `6c952b4aa` make Android startup render deterministically
  across permission hydration and fence the race.
- `2606282ad` repairs the Android pairing provider seam; `3d1efef78` checks
  that the tracked provider extension is actually wired into Wry.
- `6235aeb35` locks Android document scrolling; fresh
  `10c3a8f49` provides the focused geometry proof.
- `76cf2735`, `f531e01d`, and `1fa9a5bc6` make lazy route retries recreate the
  failed import, validate the Vite graph/CSS ownership, and clean temporary
  manifests on every outcome.

Status: code and structural checks are implemented. The earlier lazy-route
hypothesis is not the final Devices failure: the route was mounted, and the
remaining predicate is a rendered-text case mismatch described below. Native
assertions still contain unresolved observation failures; green lower-layer
tests do not promote them to native evidence.

### 4. Protected Windows evidence and sensitive output

Pairing evidence must not capture SAS/password content, and a visible named
control is insufficient without the required UI Automation semantics.

Implemented controls:

- `b010013e8` protects the pairing evidence from capture and binds the required
  protected UIA shape.
- `3c4ec9e74` redacts failure output; `9606919a4` secures the close transition.
- `810a1f31c` validates the protected root separately; `555d37ace` binds the
  receipt to that UIA root. The pairing code requires `IsPassword=true`.
- `c6b217ec4` corrects the toolbar mode-key/identity contract; the code is
  reviewed and integrated, but its native rerun remains pending. It is not the
  root fix for the Pin/Done wait bug described below.

Status: implemented, but the CI observation below still reports a missing
named password element after an earlier affinity check passed. That is an
evidence failure, not permission to weaken the requirement.

### 5. Windows build safety boundary

New F5 Windows builds reject `unsafe_code` at six sites under the library-level
deny. A worker is correcting the narrowly documented FFI boundary; no global
`unsafe` allowance is authorized.

Status: queued; the correction is not counted as a green Windows build until
the exact F5 job passes.

## Fresh validation sample

The following runs are bound to SHA `10c3a8f49`. Run links are retained so each
claim can be checked against the complete log and artifact set.

| Workflow | Result and remaining signal |
| --- | --- |
| [CI 33136251867](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251867) | 21/22 jobs green. [Windows job 98737516489](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251867/job/98737516489) still cannot observe the named `IsPassword=true` pairing-code element after `affinity17` passed. |
| [Windows E2E 33136251881](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251881) | Pin/Done receipts prove `checkedIDs` cleared and old actions disconnected; Windows also observes the Pin success toast. Remaining failures concern toolbar existence/identity observation. A diagnostic captured old-node connectivity, not its `aria-label`, so it is not direct label proof. |
| [Browser 33136251778](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251778) | Pin/Done receipts prove `checkedIDs` cleared and old actions disconnected. Remaining failures concern toolbar existence/identity observation; the diagnostic does not prove an `aria-label`. Linux WebKitGTK remains a shared-UI layer, not Windows/macOS/Android native evidence. |
| [Android 33136251808](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251808) | API 33 leg is green; storage reports 18/0. Cloud sign-in/sync works, but stale immediate-error/sign-out assertions remain. [Debug artifact 9672382866](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251808) shows three mounted DeviceRoster sections: `DeviceRoster.module.css` uppercases `body.innerText` to `YOUR DEVICES`, while the case-sensitive setup predicate expects `Your devices`, leaving `beforeAll` and the first-screen test false despite the mounted route. This is not a slow-route or timeout finding. Compact search was not opened, icon-only Unpin was incorrectly expected as text, and the IME CDP proof fails. |
| [Mutation 33136251938](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251938) | Green: Linux 91/0 and Windows 18/0 mutation verdicts. |
| [Supply chain 33136251962](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251962) | Overall FAILURE from the secret scan/gitleaks gate; dependency review, cargo-deny, and cargo-audit succeeded. Fixture findings remain tracked above. |

Subsequent evidence note (the table above remains the captured `10c3a8f49`
sample): Node verification found three E2E expressions of the form
`!(await browser.$(...)).isExisting()`. They negate an unawaited Promise and are
therefore always false. This is the definitive remaining Pin/Done wait bug,
not proof of a stale accessibility label. `edc450ede` is under review; its
native rerun remains pending.

## Remaining work and ownership

1. **Native observation contract — queued.** Repair the Windows pairing
   selector/evidence path so the named password element is observed directly;
   preserve `IsPassword=true`, protected-root binding, capture exclusion, and
   redaction. A missing native observation remains a blocker.
2. **Desktop observation contract — queued.** Correct the three unawaited
   toolbar-absence checks under review in `edc450ede`; the checked-ID clearing,
   old-action disconnection, and Windows Pin-toast receipts remain passing
   evidence. Keep WebKitGTK findings separate from shipping-native claims.
3. **Android harness — queued.** Correct the test-only structural
   `aria-labelledby`/`textContent` predicate for the rendered Devices heading
   (worker `fix_android_devices_heading_contract`); do not add a route delay or
   larger timeout. Open the compact search surface before asserting it, assert
   icon-only Unpin by its accessible semantics, and obtain a valid IME proof.
   The IME correction was code-reviewed and integrated as `7102adb98` and
   `4e72e315d`; native rerun remains pending. Re-run API 33 and the full
   scheduled matrix on the same commit.
4. **Cloud evidence — under review.** `f76274600` aligns Android cloud
   evidence with rendered states, but its full self-test currently fails and is
   under investigation. It is not counted as done in this report.
5. **Android accessibility harness — reviewed/integrated code, native rerun
   pending.** The `f1ce450fc` change was reviewed and integrated as
   `4c731cd18`; it does not make the unresolved native evidence green. Any
   follow-up must retain fail-closed semantics.
6. **Windows naming — reviewed/integrated code, native rerun pending.**
   `7de55c07c` was code-reviewed and integrated as `8a0dbc264`; the associated
   COM-teardown correction is `074087a16`. These changes are not counted as
   native evidence until rerun.
7. **Supply-chain verification — required.** The all-public-refs correction
   is reviewed/integrated as `bd29cd710`, and the default 2,859-commit clone
   scan is clean. Rerun the failed secret-scan workflow on the corrected tree;
   do not call a HEAD-only or local scan a scheduled green result.
8. **Release qualification — required.** Fresh code/local and GitHub checks do
   not replace same-commit macOS, physical Android, and installed Windows
   release receipts. No rulesets were found in a fresh GitHub API check (`[]`);
   GHAS unsupported-model advisory [33136252039](https://github.com/dmytro-yevs/copypaste/actions/runs/33136252039)
   is external guidance only, with no configuration change authorized here.
   The physical-native requirement and the existing `NOT VERIFIED IN CI`
   documentation describe different states; this audit does not request a
   broad documentation rewrite.

## Acceptance rule

The audit closes only when the remaining observations are directly evidenced
on the owning layer, the eight fixture cases have a narrow reviewed decision,
and release receipts bind commit, run, platform, scenario, accessibility
evidence, and measured budgets. A passing lower-layer test, a timeout, a retry,
an omitted assertion, a text guess, or a cancelled run is not a substitute.
