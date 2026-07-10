## ADDED Requirements

### Requirement: GlassToast semantic kinds and single-slot queueing

`ui/GlassToast` SHALL support four semantic kinds — `SUCCESS`, `DANGER`, `INFO`, `ACCENT` —
each conveyed by a leading colored dot mapped to the corresponding status/accent token, SHALL
use exactly one active toast slot (capacity 1, NO backlog queue) with this policy — a mobile **Native
adaptation** of desktop's stacked array, recorded in `cross-platform-parity.md`:
(a) a **non-actionable** toast is replaced/coalesced by newer feedback;
(b) an **actionable** toast (Undo/error/retry) is NEVER replaced by a non-actionable one;
(c) a **second actionable** event while one is active is promoted to a persistent banner/status entry
(not a second toast, not a re-queue).
Each toast has a bounded duration (default 2500 ms; actionable toasts persist until acted or dismissed),
one optional action button that dismisses before invoking its callback, an accessibility live-region
announcement, and does NOT survive process death. The SUCCESS dot maps to the `ok` status token.

#### Scenario: Non-actionable feedback replaces/coalesces
- **WHEN** a non-actionable toast is visible and another non-actionable toast is shown
- **THEN** the newer one replaces/coalesces the current one (single slot, no backlog)

#### Scenario: Actionable is not replaced by non-actionable
- **WHEN** an actionable toast is visible and a non-actionable toast is shown
- **THEN** the actionable toast remains until acted/dismissed; the non-actionable one is dropped/coalesced

#### Scenario: Second actionable promotes to a banner
- **WHEN** an actionable toast is visible and a second actionable event occurs
- **THEN** the second is promoted to a persistent banner/status entry, not queued as a toast

#### Scenario: New toast replaces the visible one
- **WHEN** `GlassToastState.show()` is called while a previous toast is still visible
- **THEN** the previous toast's countdown is abandoned and the new toast's message/kind renders
  in its place, with no stacked backlog

#### Scenario: Danger toast is bordered distinctly
- **WHEN** a toast of kind `DANGER` is shown
- **THEN** it renders with a danger-tinted hairline border in addition to its dot color, and
  auto-dismisses after its duration (default 2500 ms) unless dismissed earlier by its action

#### Scenario: Action button dismisses before firing
- **WHEN** a toast with an action is shown and the user taps the action
- **THEN** the toast is dismissed immediately and the action's callback runs

### Requirement: Banners follow the problem-plus-fix voice

Top-of-content banners SHALL appear only when actionable, SHALL render as
`[icon] message [action(s)]` using the warn / error / info / success tint per STYLEGUIDE §9.8,
and SHALL be dismissible only where ignoring the condition is safe. The sync-error banner
(Settings → Sync) SHALL distinguish an authentication failure ("check credentials") from a
generic sync error ("retry"), and the cloud-account-mismatch banner SHALL remain wired to
`detectCloudAccountMismatch` but stay inert (fed no peer account ids) — it SHALL NOT be
activated as part of this redesign.

#### Scenario: Sync error shows the right fix
- **WHEN** `Settings.lastSyncError` is non-blank and represents a 401/unauthorized response
- **THEN** the banner reads an authentication-specific message ("check credentials") rather
  than the generic sync-error text

#### Scenario: Non-actionable condition shows no banner
- **WHEN** there is no sync error and no permission/service problem requiring the user's
  attention
- **THEN** no banner renders in the content zone

#### Scenario: Cloud-mismatch banner stays inert
- **WHEN** the redesign restyles `SyncTab`
- **THEN** the cloud-account-mismatch banner's detection call continues to receive an empty
  peer-account-id list and therefore never renders, preserving today's inert behaviour

### Requirement: SyncStatusBadge states and reduced-motion pulse

`ui/SyncStatusBadge` SHALL render a colored dot plus a text label reflecting one of four
states — `Connected`, `Idle`, `DaemonUnreachable`, `NetworkOffline` — resolved from the
authoritative IPC `badge_state` wire value when available, falling back to the on-device
heuristic otherwise, and SHALL render a separate amber "Misconfig" pill when a Supabase URL is
set but not fully configured. On a false→true transition into `Connected`, the dot SHALL play
a single one-shot scale pulse, suppressed entirely when the system reduced-motion signal
(`ANIMATOR_DURATION_SCALE == 0`) is active — the dot's color still conveys state when the pulse
is suppressed. Tapping the badge SHALL open a detail bottom sheet with device count, last-sync
time, and masked account email.

#### Scenario: Idle renders neutral, not danger
- **WHEN** the device is online with peers configured but no peer has synced within the
  recency window
- **THEN** the badge shows the `Idle` state (neutral/dim dot), not the danger/red treatment

#### Scenario: Pulse suppressed under reduced motion
- **WHEN** the system animator-duration scale is 0 and the badge transitions into `Connected`
- **THEN** the dot does not animate but still renders the `Connected` color and label

#### Scenario: Tap opens detail sheet
- **WHEN** the user taps the badge
- **THEN** a bottom sheet opens showing online device count, last-sync recency, and masked
  Supabase email when configured

### Requirement: Destructive confirmations name their target

Confirm and destructive dialogs SHALL follow STYLEGUIDE §9.9 — centered modal, title (600
weight), body text naming the specific target where one exists, a `ghost` Cancel action, and a
`danger`-variant confirm action for destructive operations — covering Unpair, Revoke, Revoke &
rotate key, Revoke all, and Clear logs.

#### Scenario: Unpair names the device
- **WHEN** the user opens the unpair confirmation for a specific paired device
- **THEN** the dialog body names that device by its display name and the confirm button reads
  "Unpair" in the danger variant

#### Scenario: Revoke-all does not falsely imply one device
- **WHEN** the user opens "Revoke all"
- **THEN** the dialog body clearly states the action applies to all paired devices, not a
  single named one

### Requirement: In-flight, retry, and disabled states for async actions

Interactive actions with an asynchronous outcome SHALL disable their triggering control and show
a progress indication while in flight, and SHALL surface a retry or corrective affordance
alongside the error message when the action fails with a recoverable error — covering Export logs,
Unpair/Revoke, pairing, and import/export/vacuum. Settings Save uses a synchronous
`SharedPreferences.commit()` and SHALL NOT be modelled as an async loading action.

#### Scenario: Duplicate submission is prevented (async actions)
- **WHEN** the user taps an async action (Export logs, Unpair/Revoke, pairing, import/export/vacuum)
  while a previous invocation is still in flight
- **THEN** the control is disabled and shows a progress indicator until the result arrives,
  preventing a second submission

#### Scenario: Synchronous Save has no async/loading state
- **WHEN** the user taps Save
- **THEN** the synchronous `commit()` runs with no async/loading spinner; the control may be disabled
  only for the duration of the call to prevent re-entry
- **AND** on `commit()==false` the dirty state is retained and a retryable failure is shown

#### Scenario: Recoverable failure offers a next step
- **WHEN** an async action fails with a recoverable error (e.g. sync error, export IO failure)
- **THEN** the resulting banner or toast names the problem and offers a retry or corrective
  action, not just an error message

### Requirement: About screen content

The About screen SHALL display the app's version name and build identifier, a link to the
project repository, license/attribution information, the feature summary, and the
accent-gradient brand mark, styled with token-driven typography.

#### Scenario: Version and build shown
- **WHEN** the About screen is opened
- **THEN** it displays `versionName` and a build identifier, the repository link,
  license/attribution text, and the feature bullets

### Requirement: Logs screen legibility and feedback

The Logs screen SHALL indicate each log line's level (ok/info/warn/err) through both color and
an icon-or-text marker (never color alone), SHALL present distinct loading, empty, and
no-filter-matches states, and SHALL report the outcome of a copy/export action via `GlassToast`
for both success and failure.

#### Scenario: Level conveyed redundantly
- **WHEN** an error-level log line renders
- **THEN** it is both color-coded and marked with an icon or text label identifying it as an
  error, independent of color perception

#### Scenario: Export failure and success both surface
- **WHEN** the user exports logs
- **THEN** a `GlassToast` reports success (`SUCCESS`/`ACCENT` kind) or failure (`DANGER` kind)
  of the export

#### Scenario: No matches under an active filter
- **WHEN** a substring filter matches no lines but the log file has content
- **THEN** an empty state distinguishes "no matches for this filter" from "log file is empty"
