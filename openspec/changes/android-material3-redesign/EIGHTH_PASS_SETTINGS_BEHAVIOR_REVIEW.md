# Eighth-pass review — settings display and runtime behavior

Date: 2026-07-02  
Scope: all SEVENTH_PASS fixes, `android-settings`, Settings matrices, and actual Android settings
producers/consumers.  
Validation: `openspec validate android-material3-redesign --strict` — **valid**.

## Verdict

The four SEVENTH_PASS rows are structurally fixed. However, the new “all settings are preserved”
contract proves persistence only; it does not yet prove that every control changes the displayed UI
and/or runtime behavior correctly. Several existing controls currently have no runtime consumer.
Preserving their current “effect” would preserve a bug/no-op, contrary to the requested outcome.

The settings epic is **not ready** until the inventory is changed from a key-preservation list into a
read → edit → persist → publish → consume → observe → verify matrix.

## P0 — contradictions or non-functional settings

### 1. Frozen typography requests font weights that do not exist

The new normative table requires Inter 700 and 450, JetBrains Mono 450 and 600, while also saying
“Only bundled weights; no synthetic weights.” The bundled families expose:

- Inter: 400, 500, 600;
- JetBrains Mono: 400, 500.

Therefore title 700, body 450, body-mono 450, and micro 600 cannot satisfy the contract. Either bundle
the exact required weights (including whether 450 is a real variable/static instance) and license
them, or freeze the table to available weights. Add a font-resource test proving every requested
weight maps to a real bundled face without synthesis/fallback.

### 2. `auto_apply_synced_clip` is explicitly pref-only, but the matrix claims it applies remote clips

`Settings.kt` documents this setting as “Pref-only on Android until daemon IPC exposes a config
knob.” Repository search finds reads/writes only in Settings UI/facade; no incoming-sync application
path consumes it. §E3 nevertheless states “applies remote clips,” and §I marks the effect Preserve.

Add an implementation task and behavior test at every inbound transport path: when enabled, a remote
item is written to the Android clipboard; when disabled, it is stored/surfaced but not automatically
applied. If that requires forbidden Rust/UDL work, define an Android-side enforcement seam or obtain
scope approval. Do not call the setting working until such a consumer exists.

### 3. `notify_on_sensitive_skip` has no capture-path consumer

The setting is read and saved by Settings UI, but no production capture/sync pipeline reads
`settings.notifyOnSensitiveSkip`. The matrix claims it controls a capture-skip toast. Persistence and
upgrade tests cannot prove that.

Assign the exact sensitive-upload-suppression branch that consumes it. Test both values: `true`
emits one localized non-secret notification/toast; `false` emits none; neither path logs or exposes
content. Also test the legacy-key migration from `show_sensitive_warnings`.

### 4. `max_file_size_bytes` has no file-ingest runtime consumer

The property is exposed in Settings/config and shown by Storage UI, but repository search finds no
production ingest path reading `settings.maxFileSizeBytes`. In contrast, text and image limits have
real consumers. A slider that only persists is misleading.

Bind the value to every file acquisition path (clipboard, share receiver, import/file copy as
applicable), define boundary behavior at limit−1/limit/limit+1, and surface a localized rejection.
Test that changing the setting changes acceptance without restart.

### 5. `sync_backend` is presented as functional despite being a legacy UI hint

`Settings.kt` says relay/supabase enable flags are the real additive runtime gates and
`syncBackend` remains only a legacy UI hint; §I still lists “Sync backend” as a preserved setting and
`saveScreenSettings` persists it. A selector that does not select runtime behavior is deceptive and
can contradict the two independent transport toggles.

Choose a product contract:

- retain the key only for migration/internal compatibility and remove/hide the user control; or
- define an actual runtime meaning that does not conflict with additive transport toggles.

The upgrade test should preserve the legacy key either way, but UI tests must ensure it is not shown
as an effective control unless it has an effect.

## P1 — insufficient behavior contracts

### 6. §I is grouped, so it is not a complete field-by-field inventory

The row “Max text/image/file size, quota, TTL” combines six distinct keys and says only “native
defaults”; Supabase credentials combine multiple ordinary/keystore values; excluded apps omit the
actual storage key; actions omit availability and effects. This cannot drive a test that verifies
every key/default/range/effect.

Use one row per control/key with columns:

`control · facade property · storage owner · exact key/secret alias · exact default · valid domain ·
mode · UI owner · enabled/visible precondition · write trigger · runtime consumer(s) · hot/restart ·
failure feedback · unit/integration/UI evidence · Preserve/New/Repair`.

Mark currently non-functional controls as **Repair**, not Preserve.

### 7. Upgrade survival does not test behavior

S9.4 loads old SharedPreferences and asserts values survive. That detects key loss, but not whether a
consumer reads the value, whether a composable refreshes, or whether a service hot-applies it.

For every row require three layers:

1. persistence/migration test (old key/value/default/clamp);
2. consumer test proving true/false or boundary values change behavior;
3. UI test proving the stored value is displayed and changing the control reaches the consumer.

An upgrade fixture also needs legacy keys, corrupt/out-of-range values, missing values, keystore
secrets, and old `history_size`/appearance migration cases—not only valid current values.

### 8. Hot-apply semantics are unspecified for most behavior settings

The matrix says what settings do but not when the effect becomes observable. Some consumers read per
operation; others cache state; services use preference listeners; UI uses a `settingsVersion` tick.
Without an explicit policy, Save can succeed while an already-running service/screen continues with
stale values.

Add `activation` per row: immediate in current composition, next capture, next sync iteration,
service reconfiguration, activity recreation, or next launch. For settings that promise hot apply,
name the listener/state-flow and test it while the service/activity is already running. In particular:

- `lan_visibility`: register/unregister NSD live;
- `sync_enabled`, P2P, Wi-Fi-only, relay/supabase: stop/start the relevant loop deterministically;
- logcat capture: start/stop based on permission and setting;
- appearance/translucency/masking/list dimensions: recompose all active/future surfaces;
- max-items/quota/TTL: define whether pruning runs immediately on Save or on next write.

### 9. “Allow screenshots” updates only the current window in the settings spec

The contract says the immediate toggle adds/clears `FLAG_SECURE` on the current window. Multiple
activities/tasks may exist, and pairing/scanner must remain unconditionally secure regardless of the
preference. Define propagation:

- ordinary active/future app windows follow the committed preference;
- existing ordinary windows update when resumed or through app-scoped state;
- PairActivity and scanner ignore `allowScreenshots=true` and remain secure;
- recents behavior is verified for each class.

Test both standalone SettingsActivity and embedded Settings in MainActivity.

### 10. Persistence mode is confused with action execution mode

§E uses `Immediate action` for export/import/clear/vacuum/navigation. These actions do not persist a
setting and should not share the Immediate preference mode. This weakens automated checks and can
cause an implementing agent to treat action-local state as preferences.

Use separate kinds: `DraftPreference`, `ImmediatePreference`, `EphemeralUiState`, `Action`, and
`ReadOnlyStatus`. Only the first two participate in preference-preservation tests.

### 11. Private-mode side-effect wording is inaccurate

§E1 says private mode “stops capture/History recording.” Current pipeline still observes a clipboard
event and branches before persistence/sync; “stops capture” can imply the listener/service stops.
Specify exact externally observable behavior: no DB persistence, no sync/fan-out, no sensitive
payload feedback leakage, appropriate empty/private UI, and what happens to notification/sound.
Test those effects rather than the ambiguous phrase.

### 12. Reveal guard coverage is not tied to every reveal entry point

`show_sensitive_warnings_reveal_guard` is persisted, but the matrix merely says “reveal
confirmation.” Require the guard consistently for list row, preview, search/filter results, and any
copy/open action that reveals content; define whether one confirmation unlocks one item/action or a
session. Test guard on/off and ensure plaintext never enters semantics before confirmation.

### 13. Limits need exact consumer and rejection semantics

Text/image/file limits, quota, TTL, max-items, and image display height are not interchangeable.
The current grouped row cannot prove correct behavior. Specify separately:

- ingest limits: reject before expensive allocation/persistence, exact inclusive boundary;
- quota/max-items: prune order, pinned/sensitive policy, and when pruning runs;
- TTL: start timestamp, expiry worker/event, pinned-item policy;
- image max height/preview lines: visual recomposition only, no data mutation;
- preview delay: exact interaction timer reset/cancel behavior.

Each needs boundary tests and visible error/status behavior where user action is rejected.

### 14. Settings dependencies and disabled states remain implicit

The matrix lacks concrete enabled/visible rules. Add at minimum:

- global sync off → which transport/credential/test controls are disabled vs still editable;
- relay/supabase disabled or unconfigured → test-connection behavior;
- notification permission denied → notify toggle behavior and explanatory status;
- logcat permission unavailable → toggle/status/recovery;
- mask off → reveal guard relevance;
- private mode → sync/capture-related status;
- API/OEM-dependent controls → hidden, N-A, or recovery action.

UI tests must assert both visual disabled state and blocked callbacks.

## Recheck of SEVENTH_PASS

| Item | Status | Note |
|---|---|---|
| Exact typography/dimensions | **Reopened P0** | Tables exist, but font weights cannot be backed by bundled faces |
| Single full M3 mapping | Closed | D2/S1.7/R13 now select the full explicit map and leakage golden |
| Conditional tablet/fold goldens | Closed | D9 and downstream requirements are conditional |
| Duplicate S4.2 | Closed | Exactly one S4.2 remains |

## Required correction order

1. Resolve impossible font weights.
2. Mark and repair the four misleading/no-op settings: auto-apply, sensitive-skip feedback,
   max-file-size, and legacy sync-backend UI.
3. Expand §I to one row per preference/control with exact runtime consumer and evidence.
4. Add consumer and UI propagation tests; keep S9.4 as migration coverage only.
5. Define activation timing, dependencies, and failure behavior for every row.
6. Add precise security behavior for screenshot preference and reveal guard.
7. Run strict validation and a source-level coverage check that fails when a user-facing preference
   has no production consumer outside Settings/UI/storage code.

Approval criterion: every visible setting must either demonstrably alter UI/runtime behavior or be
explicitly classified as legacy/internal and not presented as a functional control. Merely preserving
its key is not sufficient.
