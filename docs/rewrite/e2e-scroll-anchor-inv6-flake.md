# INV-6's flake: cause, fix and measurement

Commit `6e9d7b7f`, branch `dmytro-yevs/e2e-scroll-anchor-flake`.

## The cause

`e2e/tests/scroll-anchor.e2e.test.ts` compared two reads of a moving system.

The shrink it waits for is not one event. `Daemon.removeMany`
(`e2e/src/harness/daemon.ts`) deletes in batches of eight, so removing 105 of
150 items reaches the window as roughly fourteen separate shrinks, each with its
own clamp and re-render. The old test stopped waiting at the *first* total size
under half — mid-sequence — and then read the scroll box and the row window as
two separate WebDriver round trips. A further shrink step landing between those
two calls left `scrollTop` describing one state and the rows another, so no row
covered the offset and `covered` came back false.

Measured directly, twelve shrink trials in one app session under load, each
evaluated both ways:

```
trial 11: OLD covered=false top=4732 total=6160 rowStarts=3168…4136
          NEW covered=true  top=3500 total=3960 rowStarts=3168…3872
```

`top=4732` came from the first round trip; the rows came from the second, by
which time the list had shrunk further. The bounds assertion passes in that
state (4732 ≤ 6160 − 460 + 1), which is why the failure surfaces at the coverage
assertion and not the one above it — matching the reported signature exactly.

INV-1 has the same defect. Its `topRow` helper also read the scroll box and the
rows separately, and it failed the same way once in the control runs below.

## The fix

`listSnapshot()` reads scroll geometry and the rendered row window in **one**
evaluation, so the two can never describe different states, and carries a
`requestAnimationFrame` counter.

`settledList()` returns the first snapshot that satisfies a caller's predicate
*and* repeats its geometry across at least two rendering opportunities. Two
equal samples alone would not prove a repaint happened; two equal samples with
frames between them do.

The settle test is geometry and frame count only. It knows nothing about INV-6,
so it waits for rest rather than retrying the assertion — an assertion that
fails after it fails every time. Every assertion is unchanged: the bounds check,
`rows.length > 0` and `covered` all still fire, and no tolerance was raised.

The signature deliberately excludes row text, which carries a relative age that
ticks on its own; including it would mean nothing ever settled.

## Measurement

Load: a repeated `cargo build --release` of the workspace in an isolated target
directory plus 32 CPU spinners on 24 cores, load average 40–51 throughout. Each
run is a full 14-file suite.

| Harness | Runs | INV-6 failures | INV-1 failures | Unrelated failures |
|---|---|---|---|---|
| original | 10 | **2** | 1 | 0 |
| fixed | 20 | **0** | 0 | 2 |

The two original INV-6 failures were different faults: one is the reported
`scroll-anchor.e2e.test.ts:113:19 expected false to be true`, the other is the
daemon becoming unreachable mid-`removeMany` (see below). Twelve-trial probe,
same load: old read pattern 1 failure, new read pattern 0.

**Ten consecutive full-suite greens was not reached.** Over twenty loaded runs
with the fix the suite was green 18 times, and the longest unbroken streak
within one set was 6. Neither failure was this gate: one was `settings.e2e`
INV-22, one was `devices.e2e` "offers the irreversible revoke confirmation"
(`revoking did not ask for confirmation`, the alertdialog not displayed in
time). The scroll-anchor file itself passed 20 of 20.

## Four findings that are not this gate

**The `POST /session` timeout is a red herring.**
`harness-guard.e2e.test.ts` deliberately points `startApp` at a binary that is
not the app and expects the session to time out. Its stderr —
`WebDriverError: The operation was aborted due to timeout … POST /session` —
appears on every *green* run. It is not evidence of driver trouble. In vitest's
output it carries a `stderr | tests/harness-guard.e2e.test.ts` prefix.

**`settings.e2e` INV-22 is a second flaky gate, of the same shape.**
`expected 'indigo' to be 'teal'`, once in ten loaded runs, zero in the ten
control runs, and 8 of 8 green when the file runs alone. After
`location.reload()` it waits for `nav` to exist and then asserts
`document.documentElement.dataset.accent` — two different quantities, with the
gap widening under load. It is *not* obviously a test bug: the comment there
says "applied to the document that has just been painted", which reads as a
deliberate assertion that the accent lands on the first paint, i.e. no flash of
the wrong theme. If that is the intent, waiting for the accent would destroy the
property the test exists to protect and the failure is a product defect. It
needs its own decision.

**The daemon can become unreachable mid-`removeMany` under load.**
One control run failed with `` `copypaste delete` failed: cannot reach the
CopyPaste daemon `` from `daemon.ts:128`, inside the batch delete. Each delete is
a process spawn against a debug binary with a 20s execa timeout; under
contention the daemon does not answer. This makes the gate red for a reason that
is neither the product nor the assertion.

**`devices.e2e` "offers the irreversible revoke confirmation" is a third flaky
gate.** `revoking did not ask for confirmation` at `devices.e2e.test.ts:153` —
`dialog.waitForDisplayed` on `[role="alertdialog"]` timed out. Once in twenty
loaded runs. Not investigated; it is not this gate.

## Two harness constraints worth recording

**Port 1420 is a host-wide singleton.** `strictPort` is set and the Tauri debug
binary has `devUrl` baked in, so two e2e runs on one host cannot overlap: the
second dies in `assertPortFree` with "something is already serving
http://localhost:1420". Running each suite inside an unprivileged network
namespace (`unshare -rn bash -c 'ip link set lo up; npm test'`) isolates the port
completely and lets concurrent runs coexist.

**`e2e/tests/export-import.e2e.test.ts:113` does not typecheck.** `npx tsc
--noEmit` reports `TS2365: Operator '>=' cannot be applied to types
'Promise<number>' and 'number'`. Pre-existing, unrelated to this change, and
invisible to `npm test` because vitest does not typecheck.

## Rule 5 note

`e2e/src/harness/ui.ts` is now 317 lines, past the 300-line review trigger. Its
public surface is two concerns, not one: list geometry and settling
(`listSnapshot`, `settledList`, `rowBoxes`, `scroller`, `scrollTo`,
`waitForRows`, `focusList`, `activeRowId`) and generic page driving (`gotoView`,
`visibleText`, `waitForText`, `byLabel`, `clickButton`, `count`). They have
different callers and can change independently, so they are separate modules.
Splitting them is a behaviour-preserving move that belongs in its own commit,
not mixed into a flake fix.
