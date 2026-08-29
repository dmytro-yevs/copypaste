# GitHub Actions systemic failure audit

Date: 2026-08-30
Repository: `dmytro-yevs/copypaste`
Scope: failed, cancelled, and queued GitHub Actions work observed during
2026-07-27..2026-08-27, with a fresh validation sample on
`10c3a8f494a83bec305258865a780ab3872fed5b`.

## Executive finding

The inherited month-window inventory recorded 1,700 workflow runs: 906
successful, 341 failed, 452 cancelled, and 1 queued. It also recorded 426
automatic, 19 manual, and 7 Android-timeout cases. Cancelled jobs consumed
82.862 elapsed runner-hours; that is elapsed capacity, not billable usage.

These counts are inherited audit findings. The raw inventory is not retained in
this checkout, so this report does not freshly recount the window and does not
claim that all 341 failures have been individually classified. The root
families below combine the retained findings with source/workflow inspection.

The systemic pattern is evidence loss: event-insensitive concurrency cancelled
or hid runs; reusable-workflow expansion duplicated Android work; platform
assertions observed stale or non-authoritative surfaces; and local gates could
diverge from CI. The remedy is to make event identity, ownership, readiness,
and evidence provenance executable. No timeout increase, retry added solely to
hide a defect, skipped assertion, or security weakening is an acceptable fix.

## Root families and systemic controls

### 1. Cancellation, queueing, and duplicate fan-out

The old concurrency groups conflated push, pull request, schedule, and manual
dispatch. A manual/nightly run could cancel the run that owned evidence, while
the called Android workflow could expand a five-API sweep once per matrixed
caller. This explains the cancellation volume and the duplicated Android
nightly cost.

Implemented controls:

- `72890c651` makes action groups event-aware and cancels only superseded
  push/PR work; `e67b00acc` applies the same semantics to release and nightly
  workflows.
- `6d3d21ac0` calls the scheduled Android sweep once; `d709fbb13` isolates
  nightly evidence from manual dispatch.
- The wiring checker and mutation fixtures reject a missing event discriminator,
  an unsafe cancellation policy, and the duplicated scheduled shape.

Status: implemented in the tested baseline. The fresh sample does not recreate
the historical 82.862-hour cancellation total; it validates the changed wiring
through the workflow and mutation gates.

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

Status: Supply chain run 33136251962 is green for dependency review, deny, and
audit. One synthetic fixture tree was corrected locally (`local2e`); eight
exact history fixtures were found and remain queued for a narrow ignore rule,
not silently accepted.

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
- `6235aeb35` locks Android document scrolling and adds focused geometry
  evidence.
- `76cf2735`, `f531e01d`, and `1fa9a5bc6` make lazy route retries recreate the
  failed import, validate the Vite graph/CSS ownership, and clean temporary
  manifests on every outcome.

Status: code and structural checks are implemented. Native assertions below
still contain unresolved observation failures; green lower-layer tests do not
promote them to native evidence.

### 4. Protected Windows evidence and sensitive output

Pairing evidence must not capture SAS/password content, and a visible named
control is insufficient without the required UI Automation semantics.

Implemented controls:

- `b010013e8` protects the pairing evidence from capture and binds the required
  protected UIA shape.
- `3c4ec9e74` redacts failure output; `9606919a4` secures the close transition.
- `810a1f31c` validates the protected root separately; `555d37ace` binds the
  receipt to that UIA root. The pairing code requires `IsPassword=true`.

Status: implemented, but the CI observation below still reports a missing
named password element after an earlier affinity check passed. That is an
evidence failure, not permission to weaken the requirement.

## Fresh validation sample

The following runs are bound to SHA `10c3a8f49`. Run links are retained so each
claim can be checked against the complete log and artifact set.

| Workflow | Result and remaining signal |
| --- | --- |
| [CI 33136251867](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251867) | 21/22 jobs green. [Windows job 98737516489](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251867/job/98737516489) still cannot observe the named `IsPassword=true` pairing-code element after `affinity17` passed. |
| [Windows E2E 33136251881](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251881) | Two observation failures remain around state clearing and the Pin toast. Stored toolbar connectivity is not direct `aria-label` evidence. |
| [Browser 33136251778](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251778) | Same class of two observation failures: state clearing/Pin toast and stored toolbar connectivity without direct `aria-label` proof. Linux WebKitGTK remains a shared-UI layer, not Windows/macOS/Android native evidence. |
| [Android 33136251808](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251808) | API 33 leg is green; storage reports 18/0. Cloud sign-in/sync works, but stale immediate-error/sign-out assertions remain. [Debug job 98737897489](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251808/job/98737897489) still shows a Devices skeleton, compact search was not opened, icon-only Unpin was incorrectly expected as text, and the IME CDP proof fails. |
| [Mutation 33136251938](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251938) | Green: Linux 91/0 and Windows 18/0 mutation verdicts. |
| [Supply chain 33136251962](https://github.com/dmytro-yevs/copypaste/actions/runs/33136251962) | Dependency review, cargo-deny, and cargo-audit are green; fixture findings remain tracked above. |

## Remaining work and ownership

1. **Native observation contract — queued.** Repair the Windows pairing
   selector/evidence path so the named password element is observed directly;
   preserve `IsPassword=true`, protected-root binding, capture exclusion, and
   redaction. A missing native observation remains a blocker.
2. **Desktop observation contract — queued.** Replace stored toolbar
   connectivity assumptions with direct semantic attributes and make state
   clearing and Pin-toast assertions wait on authoritative transitions. Keep
   WebKitGTK findings separate from shipping-native claims.
3. **Android harness — queued.** Resolve the Devices readiness state, open the
   compact search surface before asserting it, assert icon-only Unpin by its
   accessible semantics, and obtain a valid IME proof. Re-run API 33 and the
   full scheduled matrix on the same commit.
4. **Cloud evidence — candidate, not integrated.** `f76274600` aligns Android
   cloud evidence with rendered states; it is not counted as done in this
   report.
5. **Android accessibility harness — candidate, not integrated.**
   `f1ce450fc` hardens accessibility assertions; it is not counted as done in
   this report. Any adoption must retain fail-closed semantics.
6. **Supply-chain fixtures — queued.** Correct the single synthetic fixture
   tree in the local reproduction and add only the narrow, reviewed ignore
   for the eight exact history fixtures. Do not broaden an ignore pattern.
7. **Release qualification — required.** Fresh code/local and GitHub checks do
   not replace same-commit macOS, physical Android, and installed Windows
   release receipts. No rulesets were found in a fresh GitHub API check (`[]`);
   GHAS unsupported-model advisory [33136252039](https://github.com/dmytro-yevs/copypaste/actions/runs/33136252039)
   is external guidance only, with no configuration change authorized here.

## Acceptance rule

The audit closes only when the remaining observations are directly evidenced
on the owning layer, the eight fixture cases have a narrow reviewed decision,
and release receipts bind commit, run, platform, scenario, accessibility
evidence, and measured budgets. A passing lower-layer test, a timeout, a retry,
an omitted assertion, a text guess, or a cancelled run is not a substitute.
