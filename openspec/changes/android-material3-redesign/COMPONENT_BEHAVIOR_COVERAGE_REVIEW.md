# Component and behavior coverage review

Purpose: verify the claim that every user-visible component and behaviour is listed and has an
explicit disposition.

Verdict: **all currently detected composable names and Activity classes appear somewhere in
`component-inventory.md`, but the inventory is not yet complete enough to prove behavioural
coverage**. It is a view-symbol inventory, not a full component/interaction/state inventory.

## 1. Reproducible symbol coverage result

Repository scan result at review time:

- 116 uniquely named `@Composable` functions/extensions detected by a line-oriented annotation
  scan;
- 13 Activity subclasses;
- every one of those 116 composable names appears in `component-inventory.md`;
- every one of the 13 Activity class names appears in `component-inventory.md`.

The inventory header is therefore inaccurate when it claims "118 `@Composable` functions + 12
Activities". Fix the count or commit the exact AST extraction script and explain its different
classification. Current source has 13 Activities:

1. MainActivity
2. OnboardingActivity
3. HistoryActivity
4. PairActivity
5. PortraitCaptureActivity
6. SettingsActivity
7. ShareReceiverActivity
8. LogViewerActivity
9. PermissionsSettingsActivity
10. AboutActivity
11. BackgroundCaptureSetupActivity
12. ClipboardFloatingActivity
13. DevicesActivity

Symbol presence alone does not establish that every behaviour/state has a requirement and task.

## 2. Non-composable user-facing behaviour missing from the component inventory

The following files own user-visible behaviour or launch user-visible system surfaces but do not
have their own inventory rows/dispositions:

| Owner | User-visible responsibility | Required disposition |
|---|---|---|
| `HistoryItemActions.kt` | copy, bulk copy, delete, open/share/save file/image and action failures | Preserve behaviour; restyle/localize feedback; add outcome tests |
| `HistoryUriHelper.kt` | URI grants and external open/share targets | Preserve grant/intent semantics; test success/no-handler/failure |
| `LogExportHelper.kt` | log export/share ZIP and failure paths | Preserve IO/grants; localize and style success/failure feedback |
| `LogcatCaptureService.kt` | foreground notification and background-capture state surfaced to user | Preserve service lifecycle; localize/brand notification; test actions |
| `NotificationPermissionHelper.kt` | permission request/permanently-denied routing | Preserve state machine; map each state to onboarding/permission UI |
| `OemAutoStartHelper.kt` | OEM-specific settings labels/intents/fallbacks | Preserve resolver ordering; localize app-owned labels; test resolvable/unresolvable |
| `OnboardingPermissions.kt` | permission-state derivation and request routing | Preserve behaviour; explicit state→card/action mapping |
| `ServiceRestartWorker.kt` | restart failure/foreground notification behaviour | Preserve scheduling; inventory any posted user-visible notification |
| `ClipboardService.kt` | foreground status, pause/resume actions, copy-event feedback | Preserve service/actions; localize/brand every notification state |
| `BootReceiver.kt` | post-boot service restart that can produce notification state | Preserve; behavioural regression test, no visual restyle |
| `CaptureControlReceiver.kt` | Pause/Resume notification actions and resulting state | Preserve action semantics; test label→PendingIntent→state transition |
| `AppIconHelper.kt` | source-app icon loading/fallback shown in History | Define icon fallback/cache behaviour and golden fixture |
| `ErrorMessages.kt` | user-facing error mapping | Resource/localize every message; preserve error classification |
| `HistoryRowModel.kt` | masking, placeholder and row-state derivation | Preserve/security-test; explicitly link to History + Preview contracts |
| `DevicesRevokeActions.kt` | unpair/revoke ordering and error outcomes | Preserve ordering and local-only semantics; bind each outcome to dialog/feedback state |
| `PairController.kt` and pairing helpers | scan/SAS/progress/error transitions | Preserve protocol; map every controller state to a specified UI state |

Add a separate **behaviour-owner inventory** or extend `component-inventory.md`. Do not classify
these all as components; classify them as behaviour/state owners with Preserve/Refactor/Test
dispositions.

## 3. Manifest component coverage must be explicit

The manifest contains more than Activities. The plan must inventory every app component that can
cause visible UI, notification, system navigation or privacy behaviour:

- 13 Activities;
- `ClipboardService`;
- `LogcatCaptureService`;
- `BootReceiver`;
- `CaptureControlReceiver`;
- `ServiceRestartWorker` (WorkManager, not a manifest service);
- AndroidX WorkManager initializer;
- FileProvider paths used by open/share/export actions.

Providers do not need redesign, but URI/file providers need a Preserve contract wherever a
redesigned action depends on them. Services/receivers need notification/action/localization tests.

## 4. Per-component rows need stronger dispositions

Many current rows only say `Restyle`. That does not tell the implementation agent what behaviour
must remain or which states must be covered. Each interactive component row should include:

- visual action: Restyle/Refactor/Preserve/Remove/New;
- behaviour preserved or intentionally changed;
- inputs/states;
- user actions and outputs;
- accessibility contract;
- localization ownership;
- fixture/golden ownership or N/A rationale;
- automated behaviour/security test or manual check.

Minimum schema:

| Component/owner | Existing states | User actions | Behaviour disposition | Visual disposition | Evidence |
|---|---|---|---|---|---|

Example: `RevokeRotateDialog` must say more than Restyle: passphrase input, validation error,
in-flight lock, Cancel rules, retry, audit-before-removal invariant, localized copy, focus restore,
golden fixtures and controller test.

## 5. Interaction coverage that must be enumerated

The following behaviours exist across the current UI and cannot be inferred merely from component
names. Every item needs one owning requirement/task/test:

### App shell

- tab selection, dirty-Settings navigation guard, back behaviour;
- selected-tab restoration boundary;
- sync badge placement and sheet opening;
- system/gesture/IME inset changes;
- committed appearance propagation versus draft isolation.

### History

- tap-to-copy and echo suppression outcome;
- long-press/select mode, selection count, select all/clear;
- pin/unpin and pinned reordering;
- single/bulk delete and confirmation;
- bulk copy excluding sensitive/non-text items;
- search, device filter, clear filter;
- open/share/save for files/images/URLs and no-handler/error paths;
- reveal guard, reveal, re-mask and partial-span masking;
- load more/pagination and concurrent refresh;
- source-app icon fallback;
- too-large/unavailable content actions.

### Preview

- open/close/back/swipe gesture arbitration;
- copy/open/share/save action availability by kind;
- image loading/decode failure;
- file URI/grant failure;
- sensitive reveal guard and semantics replacement;
- large content scrolling/zoom behaviour where present.

### Devices

- discovery start/stop/refresh;
- expand/collapse device card if retained;
- fingerprint copy and feedback;
- pair discovered device;
- unpair, revoke, rotate/revoke-all, cancellation, retry and in-flight dismissal rules;
- online/offline/reconnecting derivation;
- QR refresh/countdown;
- auto-open SAS from notification;
- cloud-account mismatch remains inert.

### Pairing

- QR generation/reveal/expiry/regeneration;
- camera request, denial, permanent denial and settings recovery;
- scanner result, deep link and malformed/expired input;
- scan review confirm/cancel;
- SAS accept/reject;
- connecting/provisioning/bootstrap/sync phases;
- retry/cancel/success dismissal;
- unconditional pairing-window privacy.

### Settings

- every tab switch with draft retention;
- dirty guard from tab navigation, system back and top-bar back;
- validation and Save enablement;
- commit failure retaining dirty state;
- Save success and appearance publish;
- Discard and Keep editing;
- slider snapping/limits;
- excluded-app selection;
- destructive storage/history actions;
- diagnostics/log/about/permissions navigation.

### Onboarding/permissions/background capture

- first-launch routing and completion gate;
- granted/denied/permanently-denied/not-applicable;
- notification/camera/overlay/battery/OEM settings launches;
- resume-time state refresh;
- OEM intent unavailable fallback;
- ADB command copy;
- crash dialog export/dismiss;
- background-capture setup step completion/retry.

### Feedback/diagnostics/system surfaces

- toast replacement/timeout/action;
- banner retry/dismiss rules;
- sync badge states and detail sheet;
- log load/filter/clear/copy/export success/failure;
- About links/licenses and no-handler path;
- foreground/copy/pair/native-unavailable/restart notifications;
- notification actions Pause/Resume/Open/SAS;
- share receiver ACTION_SEND/ACTION_SEND_MULTIPLE, URI lifetime and failure logging;
- invisible overlay permission/focus/suppress/restore lifecycle.

## 6. State inventory is not yet complete

`tasks.md` explicitly labels its state table "Representative rows". Therefore it cannot support
the claim that all behaviours are enumerated. Replace or supplement it with a complete table.

For every state/action above, record:

`owner → trigger/precondition → visible result → next actions → preserved/changed → fixture →
golden/N-A → automated test → manual test/N-A`.

Important distinctions:

- existing state to preserve;
- existing behaviour with new presentation;
- new presentation state requiring plumbing;
- intentionally invisible/system-owned state;
- unreachable/dead state to remove explicitly.

No `New` state may be added solely because a design spec mentions it; the plan must identify its
state source and plumbing task.

## 7. Concrete inventory corrections

- Fix header count to the reproducible current count or add the extraction script.
- Add rows for the behaviour-owner files in section 2.
- Correct Paparazzi ownership: S0 spike, S2 infrastructure, each screen slice owns baselines, S14
  audits coverage.
- Add `LogcatCaptureService` and `ServiceRestartWorker` explicitly under system/notification
  behaviour.
- Add FileProvider/URI-grant preservation under History/Logs/Share actions.
- Add app icon loading/fallback under History row behaviour.
- Add error-message mapping/localization ownership.
- Replace generic `Restyle` on interactive dialogs/screens with behaviour-preservation notes.
- Link every helper row to its consuming surface and specify whether it is Preserve, Refactor or
  Remove; `—` is not a sufficient disposition for state owners.

## 8. Coverage readiness rule

Coverage is ready only when automated checks can answer all three questions:

1. Does every current composable/Activity/manifest-visible owner appear in an inventory?
2. Does every reachable user action/state have exactly one owning slice and disposition?
3. Does every owned state have acceptance evidence or an explicit N/A reason?

The current revision answers question 1 for composables and Activities, but not questions 2 and 3.
