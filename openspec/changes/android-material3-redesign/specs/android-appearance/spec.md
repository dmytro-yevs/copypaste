## ADDED Requirements

### Requirement: Two-axis plus two booleans appearance model, local to Android
The app SHALL scope user-configurable appearance to exactly four preferences — theme (dark / light / system), accent (one of six hues), translucency (on/off), and mask sensitive data (on/off) — with no additional palette, skin, density, contrast, or motion setting, and these preferences SHALL be local to the Android device only, never synced to other paired devices.

#### Scenario: Appearance subsection has exactly four controls; other Display controls remain
- **WHEN** Settings → Display is inspected
- **THEN** the **Appearance subsection** exposes exactly Theme, Accent, Translucency, and Mask-sensitive
- **AND** all existing functional Display controls (sensitive warnings, reveal guard, allow
  screenshots, image max height, preview delay, preview lines) remain present and unchanged

#### Scenario: Appearance does not sync across devices
- **WHEN** the user changes theme or accent on their phone
- **THEN** no sync/relay message carrying appearance state is sent, and a paired Mac's theme is unaffected

### Requirement: Appearance defaults
On first run, or whenever no persisted appearance value exists, the app SHALL default to theme=dark, accent=indigo, translucency=on, and mask sensitive data=on.

#### Scenario: Fresh install renders the defaults
- **WHEN** the app launches with no prior `Settings` row for appearance
- **THEN** it renders dark theme, indigo accent, translucent chrome, and masked sensitive clips

### Requirement: Draft-staged live preview in Settings
The Display tab SHALL hoist appearance draft state above `CopyPasteTheme` so that changing the theme segmented control, an accent swatch, or the translucency switch re-themes the Settings screen immediately, without writing to persisted `Settings` until the user taps Save.

#### Scenario: Live preview without persistence
- **WHEN** the user taps a different accent swatch but has not tapped Save
- **THEN** the Settings screen re-themes to that accent immediately AND the persisted accent value is unchanged AND the Save action becomes enabled (dirty)

#### Scenario: Draft state does not leak to other screens
- **WHEN** the user is previewing an unsaved accent on the Display tab
- **THEN** navigating to another activity shows the last-saved theme, not the unsaved draft

### Requirement: App-wide appearance propagation on Save
On Save, the app MUST write `themeMode`, `accent`, `translucency`, and `maskSensitive` in the single `saveScreenSettings` batched commit AND update an application-scoped observable committed-appearance state that `CopyPasteTheme` reads, so every currently-composed and every future Activity re-themes. The design MUST NOT rely on `Activity.recreate()` alone, because it recreates only the current Activity instance and cannot re-theme stopped back-stack or other-task activities. The draft/live-preview state SHALL remain scoped to the Settings screen and MUST NOT feed the application-scoped state until Save.

#### Scenario: Save from embedded Settings (MainActivity tab)
- **WHEN** the user Saves an appearance change while Settings is the active tab inside `MainActivity`
- **THEN** the committed-appearance state updates and the whole `MainActivity` shell (all tabs) recomposes to the new theme, without relying on `recreate()`

#### Scenario: Save from standalone SettingsActivity
- **WHEN** the user Saves from a standalone `SettingsActivity` launched over the back stack
- **THEN** the committed-appearance state updates so `SettingsActivity` and every Activity beneath it apply the new theme on next resume/recomposition

#### Scenario: Another Activity already in the task
- **WHEN** a `HistoryActivity` or `DevicesActivity` is already on the back stack when an appearance Save occurs
- **THEN** that Activity reflects the new theme on its next resume/recomposition by reading the committed-appearance state through `CopyPasteTheme`, not a stale captured snapshot

#### Scenario: Draft never leaks before Save
- **WHEN** the user is live-previewing an unsaved accent on the Display tab
- **THEN** the application-scoped committed-appearance state is unchanged and no other Activity shows the draft

#### Scenario: No app-wide change when unchanged
- **WHEN** the user taps Save without changing any appearance value
- **THEN** the committed-appearance state is not rewritten and no spurious app-wide recomposition occurs

### Requirement: Discard on unsaved exit
If the user navigates away from Settings with unsaved appearance changes and confirms Discard, no draft appearance value SHALL be persisted and the previously saved theme SHALL remain in effect.

#### Scenario: Discard reverts the preview
- **WHEN** the user changes appearance, then navigates back and confirms Discard
- **THEN** no appearance value is persisted and the app continues rendering the last-saved theme, not the discarded draft

### Requirement: Versioned one-time theme migration
The one-time theme migration SHALL be updated (via its latch/version key) so it stops deleting the canonical `theme_mode`/`accent` keys before the new getters are introduced, retaining those key names rather than inventing new ones. It SHALL remove only the genuinely stale Liquid-Glass-era keys, run before the appearance getters are first read (in `CopyPasteApp.onCreate`), and run at most once per install.

#### Scenario: Migration keeps the canonical keys
- **WHEN** a user upgrades from a build with stale palette/skin keys and then saves a new theme/accent
- **THEN** the stale keys are cleared exactly once on first launch, the migration no longer removes `theme_mode`/`accent`, and a freshly saved value survives

#### Scenario: Already-migrated install is untouched
- **WHEN** the app launches on an install whose migration latch is already set
- **THEN** the migration does not run again and existing `theme_mode`/`accent` values are preserved

#### Scenario: Ordering before first read
- **WHEN** `CopyPasteApp.onCreate` runs
- **THEN** the migration executes before any appearance getter reads `theme_mode`/`accent`

### Requirement: Committed persistence, commit-failure handling, and preserved security invariants
Only values committed via the batched `saveScreenSettings` commit — not draft/in-memory state — SHALL survive process death; a failed commit (`commit()` returns false) SHALL NOT clear the dirty state or report success, and SHALL surface an error so the user can retry. `SecureWindowChrome`'s edge-to-edge and `FLAG_SECURE` SideEffects SHALL remain driven by `Settings.allowScreenshots`, unaffected by any appearance change.

#### Scenario: Force-stop-safe save
- **WHEN** the user saves an appearance change and the process is force-stopped immediately after
- **THEN** the relaunched app reads the saved values, not any uncommitted draft

#### Scenario: Commit failure keeps dirty state
- **WHEN** the batched `commit()` returns false
- **THEN** the Save is reported as failed, the dirty state is retained, and no success feedback is shown

#### Scenario: FLAG_SECURE unaffected by appearance change
- **WHEN** an appearance change is applied while `allowScreenshots` is false
- **THEN** `FLAG_SECURE` remains set on every themed Activity and edge-to-edge insets remain applied
