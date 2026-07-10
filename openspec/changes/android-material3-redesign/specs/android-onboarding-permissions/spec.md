## ADDED Requirements

### Requirement: Onboarding permission flow and completion gate

The Android onboarding flow SHALL present `OnboardingScreen`'s permission cards in a fixed
order — Notifications (required), Background Capture ADB/overlay guidance, Battery
Optimization (optional), OEM Autostart guidance (shown only when
`OemAutoStartHelper.hasOemScreen()` resolves a target), Foreground Service (informational,
always granted), and Export Logs — each rendered as a token-driven `PermissionCard` whose
visual state (neutral outline / accent primary / error border) reflects `granted: Boolean?`
(`null` = indeterminate, never rendered as an error). The primary call-to-action SHALL read
"Continue to CopyPaste" (`R.string.btn_continue_to_copypaste`) once
`OnboardingActivity.allCriticalGranted()` (notification permission only) is satisfied and
"Skip for now" otherwise, without blocking on any optional permission.

#### Scenario: Notification permission gates completion
- **WHEN** the user has not granted POST_NOTIFICATIONS and is on API 33+
- **THEN** the Notifications card renders with the required/error treatment and the CTA reads
  "Skip for now"

#### Scenario: Optional permissions never block completion
- **WHEN** notifications are granted but battery-optimization exemption and OEM autostart are not
- **THEN** the CTA reads "Continue to CopyPaste" and those cards render their neutral/optional
  treatment, not an error state

#### Scenario: OEM card appears only when resolvable
- **WHEN** `OemAutoStartHelper.hasOemScreen(context)` returns false for the device OEM
- **THEN** the OEM Autostart card is omitted from the onboarding flow entirely

### Requirement: Crash-detected recovery dialog

`OnboardingActivity` SHALL detect an uncaught crash from the previous run via
`CrashHandler.consumeCrashedLastRun` and, when true, present a `CrashDetectedDialog` offering
an "Export" action (invoking `LogExportHelper.shareLogsZip`) and a "Dismiss" action, styled per
STYLEGUIDE §9.9 modal conventions.

#### Scenario: Crash detected on relaunch
- **WHEN** the app relaunches after `CrashHandler` recorded an uncaught exception on the
  previous run
- **THEN** the `CrashDetectedDialog` appears once, offering Export and Dismiss, and the
  crashed-last-run flag is cleared so it does not reappear

#### Scenario: No crash, no dialog
- **WHEN** the previous run terminated normally
- **THEN** onboarding renders without the crash-detected dialog

### Requirement: Permission status conveyed by redundant signal

Every permission/status card SHALL convey its granted / not-granted / permanently-denied /
not-applicable state through an icon and status text in addition to color or border treatment,
across onboarding, `PermissionsSettingsActivity`, and `BackgroundCaptureSetupActivity`,
consistent with STYLEGUIDE §7 ("color is never the only signal").

#### Scenario: Permanently-denied notification permission
- **WHEN** `NotificationPermissionHelper.isPermanentlyDenied` is true (not granted, previously
  requested, rationale suppressed)
- **THEN** the card shows an error-tinted icon and explicit "permanently denied" status text,
  and its action button routes to `ACTION_APP_NOTIFICATION_SETTINGS` (falling back to
  `ACTION_APPLICATION_DETAILS_SETTINGS`) instead of re-requesting the OS dialog

#### Scenario: Not-applicable permission
- **WHEN** a permission is not applicable to the running API level (e.g. POST_NOTIFICATIONS
  below API 33)
- **THEN** its card shows a neutral "not needed on this device" status, not a denied/error
  treatment

### Requirement: Revisitable permissions settings screen

`PermissionsSettingsActivity` SHALL remain reachable at any time from Settings (not gated by
onboarding completion) and SHALL always render its action buttons enabled, allowing the user
to re-open any recovery flow — Notifications, Background Capture, Battery Optimization, OEM
Autostart (conditional), Foreground Service — regardless of current status.

#### Scenario: Revisit after granting everything
- **WHEN** all applicable permissions are already granted
- **THEN** `PermissionsSettingsActivity` still opens normally and shows each card in its
  granted state with its recovery action still tappable

### Requirement: Background-capture setup wizard

`BackgroundCaptureSetupActivity` SHALL walk the user through overlay permission (required) and
battery-optimization exemption (required) as ordered steps, SHALL show the OEM Autostart step
only when `OemAutoStartHelper` resolves a target for the device and the user has not already
acknowledged it (`setOemAcknowledged`), and SHALL render that step in a compact
acknowledged/granted state once resolved. The final step SHALL instruct the user to force-stop
and reopen the app so the capture overlay re-initializes, and every step's status SHALL be
re-evaluated on `onResume`, not cached across recompositions.

#### Scenario: OEM step acknowledged
- **WHEN** the user taps "Done — I've enabled it" on the OEM Autostart step
- **THEN** `OemAutoStartHelper.setOemAcknowledged` persists the acknowledgment and the step
  collapses to its compact granted card on next composition

#### Scenario: No OEM screen resolvable
- **WHEN** the device OEM has no known autostart settings screen
- **THEN** the wizard shows a static "not needed" informational card instead of an actionable
  step

#### Scenario: Status re-checked on resume
- **WHEN** the user returns to `BackgroundCaptureSetupActivity` after granting overlay
  permission in system Settings
- **THEN** the overlay step re-evaluates on `onResume` and reflects the granted state without
  requiring a manual refresh

### Requirement: OS-owned permission surfaces are not restyled

The redesign SHALL style only app-owned rationale, status, and recovery screens; it SHALL NOT
attempt to alter the appearance of OS-rendered runtime permission dialogs or system Settings
pages. Acceptance for these transitions SHALL be verified by correct `Intent` actions and
correct app-owned pre/post screens, not visual parity with OS chrome.

#### Scenario: Correct settings intent fired
- **WHEN** the user taps "Open Settings" for overlay permission
- **THEN** the app fires `Settings.ACTION_MANAGE_OVERLAY_PERMISSION` (or the appropriate
  battery/notification/OEM equivalent) and does not attempt to render its own substitute for
  the OS settings page

#### Scenario: Return from OS settings updates app-owned status
- **WHEN** the user returns from an OS settings page after changing a permission
- **THEN** the app-owned status card updates to reflect the new state without requiring the OS
  page itself to be restyled
