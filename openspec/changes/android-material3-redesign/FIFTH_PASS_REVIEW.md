# Fifth-pass review

Verdict: **FOURTH_PASS is materially addressed. Five small corrections remain; one is a real
behavioural error discovered by checking the Kotlin implementation.**

## 1. Fix `soundOnCopy` behaviour — current matrix is wrong

`behavior-and-state-coverage.md` E5 says:

> Sound-on-copy ... ineffective when notify-on-copy off (dependency rule)

The current implementation does not have that dependency. In all three capture paths in
`ClipboardCapturePipeline.kt`, notification and sound are independent:

```kotlin
if (settings.notifyOnCopy) ServiceNotifications.postCopyNotification(context)
if (settings.soundOnCopy) ServiceNotifications.playCopySound(context)
```

Required correction:

- Sound-on-copy remains effective when Notify-on-copy is off.
- Preserve the independent toggles unless product explicitly requests a behavioural change.
- Add truth-table tests for all four combinations of the two booleans.
- Remove the dependency-rule statement from the coverage matrix and any spec/task that copied it.

## 2. Remove the stale `if retained` device-card row

`behavior-and-state-coverage.md` C4 still contains:

> expand/collapse card (if retained)

Section F correctly resolves the decision to **no collapse; always-expanded natural-height cards**.
Delete or rewrite the C4 row so the source of truth has only one answer.

## 3. Make the restart-notification contract concrete

The notification list still says:

> Restart (`ServiceRestartWorker`) — inventory any posted notification

The implementation is already known. `ServiceRestartWorker.getForegroundInfo()` posts notification
ID 1010 on `ClipboardService.CHANNEL_ID`, with localized active title, launcher foreground icon,
`PRIORITY_LOW`, `ongoing=true`, and `FOREGROUND_SERVICE_TYPE_SPECIAL_USE` where supported.

Replace “inventory any” with the actual preservation/localization contract and tests. Include the
API 26–30 expedited-worker path, because that is where the foreground notification is required.

## 4. Finish the remaining traceability cleanup

`tasks.md` now correctly describes the lifecycle in the Golden infra row, but its Slice cell still
ends with `S14`. Prefer either:

- `S0/S2/S4–S14`, or
- split rows for compatibility spike, infrastructure, per-surface baselines and final audit.

This avoids tooling/readers treating S14 as the sole owner despite the descriptive text.

`design.md` D14 should likewise say explicitly that S2 establishes test infrastructure and S14 only
audits coverage.

## 5. Correct the dependency-compatibility statement in `proposal.md`

The Impact section still describes the Paparazzi/Lucide additions parenthetically as compatible with
the current stack. The design correctly says compatibility is not yet proven and must be established
by S0 spikes.

Change the proposal to:

- candidate dependencies targeting the current stack;
- exact coordinates/versions and compatibility are blocking S0 outputs;
- no dependency is adopted until its proof passes.

## 6. Status after these corrections

After these five edits, the documents are sufficiently complete to begin S0:

- composables/Activities/manifest components are inventoried;
- behaviour owners and control-level Settings persistence modes are documented;
- major reachable interactions/states have dispositions and evidence categories;
- evidence categories map to proposed test classes, runners and commands;
- unresolved dependency/blur choices are correctly isolated as blocking S0 spikes.

S1 must remain blocked until those S0 proofs and the full inventory reconciliation pass are complete.
