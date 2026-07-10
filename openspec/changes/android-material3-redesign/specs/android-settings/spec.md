## ADDED Requirements

### Requirement: Tabbed settings navigation on tokens

Settings SHALL present General, Display, Sync, Storage, and Notifications as separate tabs, styled
entirely from STYLEGUIDE tokens (§9 components, §3 colors) rather than raw or hardcoded values.

#### Scenario: Five tabs render with a token-styled active indicator
- **WHEN** the Settings screen is composed
- **THEN** General, Display, Sync, Storage, and Notifications tabs are shown, with the active tab
  distinguished using `--selected`/`--text` tokens and inactive tabs using `--dim`

#### Scenario: Switching tabs preserves unsaved draft state
- **WHEN** the user edits a field on one tab, then switches to another tab and back
- **THEN** the unsaved draft value is still shown, not reset to the persisted value

### Requirement: Settings control states

Every settings control (input, toggle, segmented control, slider) SHALL visually distinguish each
state that APPLIES to it — from the set normal/focused/disabled/dirty/saved/validation-error/
destructive/loading — using STYLEGUIDE §9.1–9.3 tokens. A control is not required to support states
that do not apply (e.g. a plain toggle has no validation-error).

#### Scenario: Focused input shows the accent focus ring
- **WHEN** a settings text input gains focus
- **THEN** its border switches to `--accent` and a focus-visible ring is shown

#### Scenario: Dirty control is visually distinct from saved
- **WHEN** a control's draft value differs from the last-saved value
- **THEN** the control (and the Save action) reflect a dirty state, clearing to a saved state once
  persisted

#### Scenario: Disabled control blocks interaction
- **WHEN** a setting is disabled by a precondition (e.g., a dependent toggle is off)
- **THEN** the control renders at reduced opacity and does not respond to input

### Requirement: Draft model with Save and Discard

Settings SHALL continue to use a draft model **for Save-owned fields**: their edits are staged
locally and do not persist until Save is tapped; immediate and ephemeral controls (see the
persistence-mode requirement) are exempt. Navigating away with unsaved draft changes SHALL trigger a
dirty guard that opens a discard-confirmation dialog.

#### Scenario: Dirty guard intercepts navigation away
- **WHEN** the user has unsaved changes and attempts to leave the Settings screen
- **THEN** a discard-confirmation dialog opens instead of navigating away immediately

#### Scenario: Discard reverts to the last-saved values
- **WHEN** the user confirms Discard on the dirty-guard dialog
- **THEN** all draft edits are dropped and the screen reflects the last-persisted settings

#### Scenario: Save persists the current draft
- **WHEN** the user taps Save with a dirty draft
- **THEN** the draft values are written to persistent settings and the dirty state clears

### Requirement: Atomic settings persistence

`saveScreenSettings` SHALL write every field mutated by a settings Save in a single synchronous
`commit()` batch. The reveal guard, preview-lines count, max-items limit, excluded-apps list,
auto-apply-synced-clip toggle, max-file-size-bytes limit, sensitive-TTL, collect-public-IP toggle, and
paste-as-plain-text toggle — all nine currently written outside that batch via individual `.apply()`
calls in `SettingsActivity.persistAll()` — SHALL be folded into the same atomic batch, so a single Save
either persists all changed fields or none.

#### Scenario: A single Save persists all fields atomically
- **WHEN** the user changes the reveal guard, preview-lines count, max-items limit, excluded-apps
  list, auto-apply-synced-clip, max-file-size-bytes, sensitive-TTL, collect-public-IP, and
  paste-as-plain-text together and taps Save
- **THEN** all nine values are written within the same `commit()` batch as the rest of
  `saveScreenSettings`, not as separate uncommitted writes

#### Scenario: Force-stop immediately after Save leaves no partial state
- **WHEN** the process is force-stopped immediately after `saveScreenSettings` returns
- **THEN** the relaunched app reads either the fully-updated values or the fully-previous values for
  every field in the batch — never a mix

### Requirement: Explicit persistence mode per setting (draft / immediate / ephemeral)

Each Settings control SHALL declare exactly one persistence mode, and the redesign SHALL preserve it
rather than silently converting it under a visual change. The atomic `saveScreenSettings` batch
applies ONLY to draft/Save-owned fields; immediate controls persist on change with their runtime side
effect; ephemeral controls hold session-only state.

- **Immediate (persist on toggle, preserved):** `allowScreenshots` — also toggles `FLAG_SECURE` on the
  current window immediately; `relayEnabled` and `supabaseEnabled` — independent additive transports
  written immediately in `SyncTab`, not gated by Save.
- **Ephemeral (session-only, not persisted):** the export "include sensitive" toggle (default off).
- **Draft/Save-owned:** every remaining Settings field.

#### Scenario: allowScreenshots applies immediately with FLAG_SECURE
- **WHEN** the user toggles `allowScreenshots`
- **THEN** `Settings.allowScreenshots` is written immediately and `FLAG_SECURE` is added/cleared on
  the current window at once, without waiting for Save

#### Scenario: Transport enables persist immediately
- **WHEN** the user toggles `relayEnabled` or `supabaseEnabled`
- **THEN** the value persists immediately (the fan-out reads it live) and is not part of the Save-batch draft

#### Scenario: Ephemeral include-sensitive resets
- **WHEN** the user opens Storage export after a previous export
- **THEN** the "include sensitive" toggle defaults to off and is never persisted across sessions

#### Scenario: Atomic batch covers only Save-owned fields
- **WHEN** the atomic `saveScreenSettings` commit runs
- **THEN** it includes only draft/Save-owned fields, never the immediate or ephemeral controls

### Requirement: All existing user settings are preserved

The redesign SHALL preserve EVERY existing persisted user setting — its SharedPreferences key, default,
valid range/clamp, persistence mode, and runtime effect — and SHALL NOT drop, rename, relocate, or
silently change any of them; the new appearance keys (`theme_mode`, `accent`) are purely additive. The
complete field-by-field inventory is `behavior-and-state-coverage.md §I`. This covers the Settings tabs
AND non-tab user-controlled state: capture pause/resume (`capture_enabled`), history sort-by-device
(`sort_by_device`), and recent searches (`recent_searches`). Internal/non-UI keys (device id, crypto
keys, sync cursors, tokens) are untouched.

#### Scenario: Every existing setting survives the redesign
- **WHEN** a user upgrades to the redesigned build with prior values for any existing setting
- **THEN** that setting is still present, editable via its (possibly restyled) control, reads its prior
  value from the same key, and has the same runtime effect

#### Scenario: No key renamed or dropped
- **WHEN** the redesigned `Settings` façade is compared against the current one
- **THEN** no existing SharedPreferences key is renamed or removed; only `theme_mode`/`accent` are added

#### Scenario: Non-tab user state still works
- **WHEN** the user uses notification Pause/Resume, History "sort by device", or recent searches
- **THEN** each still functions and persists to its existing key, unchanged by the redesign

### Requirement: Every functional setting has a runtime consumer (no persist-only no-ops)

A setting presented as a functional control SHALL have a production runtime consumer outside
Settings/UI/storage code. A persisted value with no consumer SHALL be classified **Repair** (wire an
Android-side consumer — no forbidden Rust/UDL edits) or reclassified legacy/internal and NOT shown as
an effective control. The following current no-ops are Repair:

- `auto_apply_synced_clip` — an Android inbound-transport seam applies a remote item to the clipboard
  when enabled, and stores-only (no auto-apply) when disabled.
- `notify_on_sensitive_skip` — the sensitive-upload-suppression branch emits exactly one localized
  non-secret toast/notification when true and none when false (legacy `show_sensitive_warnings`
  migration preserved).
- `max_file_size_bytes` — every file-acquisition path (clipboard, share receiver, import) enforces the
  limit with a localized rejection at the inclusive boundary.
- `sync_backend` — legacy hint only (relay/supabase enable flags are the real additive gates); its key
  is retained for migration but it SHALL NOT be shown as an effective selector unless given a
  non-conflicting runtime meaning.

#### Scenario: Auto-apply actually applies (or not)
- **WHEN** a remote clip arrives with `auto_apply_synced_clip` enabled vs disabled
- **THEN** it is written to the Android clipboard when enabled, and stored/surfaced without auto-apply when disabled

#### Scenario: Sensitive-skip toast honours the flag
- **WHEN** a sensitive item's upload is suppressed with the flag true vs false
- **THEN** true emits one localized non-secret toast and false emits none; neither logs/exposes content

#### Scenario: File over-limit is rejected
- **WHEN** a file at limit+1 is acquired via clipboard/share/import
- **THEN** it is rejected with a localized message before expensive persistence, without a restart

#### Scenario: Legacy sync_backend is not an effective control
- **WHEN** Settings → Sync is inspected
- **THEN** transport is governed by the relay/supabase enables; `sync_backend` is not presented as an effective selector

### Requirement: Defined activation timing per setting

Each setting SHALL declare WHEN its change becomes observable — immediate-in-composition, next-capture,
next-sync-iteration, service-reconfiguration, activity-recreation, or next-launch — and hot-apply
settings SHALL name their listener/state-flow and be tested while the service/activity is already
running: `lan_visibility` (live NSD register/unregister), `sync_enabled`/P2P/Wi-Fi-only/relay/supabase
(deterministic loop start/stop), logcat capture (start/stop by permission+flag), appearance/
translucency/masking/list-dimensions (recompose active + future surfaces), max-items/quota/TTL (state
whether pruning runs on Save or on next write).

#### Scenario: Hot-apply while running
- **WHEN** the user changes `lan_visibility` while the service is running
- **THEN** NSD registration/unregistration happens live via the named listener, without a restart

#### Scenario: Activation is recorded per row
- **WHEN** the settings matrix is inspected
- **THEN** every setting row names its activation timing (and, for hot-apply, its listener/state-flow)

### Requirement: allow-screenshots propagation and unconditional-secure exceptions

`allowScreenshots` (immediate) SHALL apply the committed preference to all ordinary active and future
app windows (existing windows on resume or via app-scoped state), while `PairActivity` and the scanner
(`PortraitCaptureActivity`) remain unconditionally `FLAG_SECURE` regardless of the preference. Recents
behaviour SHALL be verified per window class, for both standalone `SettingsActivity` and embedded Settings.

#### Scenario: Ordinary windows follow the preference; pairing stays secure
- **WHEN** `allowScreenshots=true`
- **THEN** ordinary windows allow capture, but `PairActivity` and the scanner remain `FLAG_SECURE`

### Requirement: Private mode observable behaviour

Private mode SHALL mean: no DB persistence, no sync/fan-out, no sensitive-payload feedback leakage, and
the private empty/UI state — the capture listener/service is NOT stopped. Behaviour is verified by these
observable effects, not by the phrase "stops capture".

#### Scenario: Private mode suppresses persistence and sync
- **WHEN** private mode is on and a clip is copied
- **THEN** nothing is persisted to the DB and no sync/fan-out occurs, while the service keeps running

### Requirement: Reveal guard at every entry point

The reveal guard (`show_sensitive_warnings_reveal_guard`) SHALL apply consistently at every reveal
entry point — list row, preview, search/filter results, and any copy/open that reveals content — SHALL
define whether one confirmation unlocks one item/action or a session, and SHALL guarantee plaintext
never enters the semantics tree before confirmation.

#### Scenario: Guard enforced across surfaces
- **WHEN** the guard is on and the user attempts to reveal in the list, preview, or search results
- **THEN** each requires confirmation and no plaintext is in semantics until confirmed

### Requirement: Control kinds and dependency/disabled rules

Settings SHALL classify each control as `DraftPreference` / `ImmediatePreference` / `EphemeralUiState`
/ `Action` / `ReadOnlyStatus` (only the first two participate in preference-preservation tests), and
SHALL define enabled/visible preconditions with UI tests asserting BOTH the visual disabled state and
blocked callbacks: global sync-off disables transport/credential/test controls; unconfigured transport
gates test-connection; notification-permission-denied changes the notify toggle + shows status; logcat
permission gates its toggle/status/recovery; mask-off makes reveal-guard irrelevant; private-mode shows
capture/sync status; API/OEM-dependent controls are hidden/N-A/recovery.

#### Scenario: Sync-off disables dependent controls
- **WHEN** global sync is off
- **THEN** transport/credential/test controls are disabled (visually + callbacks blocked), per the dependency rules

#### Scenario: Ingest limits reject at the boundary; display settings only recompose
- **WHEN** a text/image/file item hits its size limit, or image-max-height/preview-lines change
- **THEN** ingest limits reject at the exact inclusive boundary before persistence with a localized error,
  while image-max-height/preview-lines only recompose (no data mutation); quota/max-items/TTL define
  prune order (pinned/sensitive policy) and when pruning runs

### Requirement: Appearance controls live in the Display tab only

Theme, accent, and translucency pickers SHALL be presented within the Display tab, as specified by the
`android-appearance` capability. The Settings tab structure SHALL NOT duplicate or relocate these
controls onto any other tab.

#### Scenario: Appearance pickers appear only under Display
- **WHEN** the user browses General, Sync, Storage, and Notifications tabs
- **THEN** no theme, accent, or translucency control appears on any of them; they are found only on
  the Display tab

### Requirement: Validation-error presentation

Fields with invalid input SHALL show an inline validation-error state — error-tinted border, text, and
message — and SHALL block Save while any field is in an invalid state.

#### Scenario: Invalid value blocks Save
- **WHEN** a settings field (e.g., a numeric limit) holds an out-of-range value
- **THEN** the field shows the validation-error state and the Save action is disabled

#### Scenario: Correcting the value clears the error and unblocks Save
- **WHEN** the user edits the invalid field to a valid value
- **THEN** the validation-error state clears and Save becomes available again (if otherwise dirty)

### Requirement: Destructive settings actions

Destructive settings actions (e.g., clear history, revoke-all reachable from Settings) SHALL use the
`danger` button style and SHALL require a confirmation dialog (STYLEGUIDE §9.9) before executing,
showing a loading state while the operation is in flight.

#### Scenario: Destructive action requires confirmation
- **WHEN** the user taps a destructive settings action
- **THEN** a confirmation dialog (ghost Cancel / danger confirm) opens before anything is executed

#### Scenario: In-flight destructive action shows loading
- **WHEN** the user confirms a destructive action
- **THEN** the dialog or control shows a loading state until the operation completes, then reflects
  success or an error state
