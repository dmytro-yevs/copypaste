## ADDED Requirements

### Requirement: Complete EN/UK String Resourcing

All translatable user-visible strings and accessibility labels SHALL live in Android string
resources, with `res/values/strings.xml` as the default-English source and a complete
`res/values-uk/strings.xml` covering every translatable key, including notification channel names
and notification content, dialog copy, error messages, and content descriptions. Keys that are
intentionally non-translatable (app name, protocol literals, machine labels) SHALL be marked
`translatable="false"` and are explicitly exempt from Ukrainian coverage.

#### Scenario: New string added
- **WHEN** a new user-visible string or content description is introduced anywhere in the app
- **THEN** it is declared as a resource key in `res/values/strings.xml` (English) with a matching
  key present in `res/values-uk/strings.xml`

#### Scenario: System-facing text is localized
- **WHEN** a notification channel, notification, dialog, or error message renders
- **THEN** its text is sourced from string resources rather than an inline Kotlin literal

#### Scenario: Ukrainian locale is complete
- **WHEN** the device locale is Ukrainian
- **THEN** every translatable user-visible surface, including notifications and content
  descriptions, renders in Ukrainian with no fallback to the English default for a missing key

#### Scenario: Non-translatable keys are exempt
- **WHEN** the completeness check runs against `res/values-uk`
- **THEN** keys marked `translatable="false"` are not required in `values-uk` and their absence
  does not fail the gate, while a missing translatable key (or a placeholder value) does fail it

### Requirement: Formatted Strings, Plurals, and Locale-Aware Data Formatting

Strings that embed dynamic values SHALL use a single formatted string resource rather than
concatenated sentence fragments, count-bearing messages SHALL use `<plurals>` resources, and
dates, relative times, numbers, and file sizes SHALL render using locale-aware formatting APIs,
except values intentionally kept in a fixed machine format.

#### Scenario: Dynamic value embedded in a sentence
- **WHEN** a UI string embeds a dynamic value such as a device name, a count, or a time
- **THEN** it is produced from one formatted string resource with placeholders, never by
  concatenating separate string fragments

#### Scenario: Count-bearing message
- **WHEN** a message reports a quantity (e.g. number of items or devices)
- **THEN** it resolves through a `<plurals>` resource using the active locale's plural rules

#### Scenario: Locale-aware date and number rendering
- **WHEN** an absolute/relative timestamp, a number, or a file size is displayed
- **THEN** it is formatted per the active locale's conventions, except values intentionally kept
  as fixed machine format (IP addresses, hex fingerprints, RTT milliseconds)

### Requirement: Hardcoded-User-Text Lint Gate

The build SHALL enforce a gate (Android Lint plus a project AST/script check) that fails on a
hardcoded human-readable literal reaching ANY user-facing sink — Compose `Text()`,
`contentDescription`, `stateDescription`, `onClickLabel`, shared-component text parameters
(`title`/`subtitle`/`message`), Toast/Snackbar/notification builders, dialog copy, and error
mappers/service code — including literals assembled by string concatenation before the sink.
Literals that are intentionally unlocalized machine formats SHALL be exempt via an explicit
allowlist limited to protocol literals, format patterns, URLs, IDs, and raw values passed into
localized formatted resources; each allowlist entry records file, literal/pattern, reason, and owner,
and broad directory/sink exemptions are rejected. A version *value* may be exempt, but surrounding
labels/sentences remain localized.

#### Scenario: Hardcoded literal introduced
- **WHEN** a commit adds a Compose `Text()` or `contentDescription` with a hardcoded
  human-readable string
- **THEN** the lint gate fails the build

#### Scenario: Machine-format literal is exempt
- **WHEN** a literal represents a fixed machine format such as an IP address, a hex fingerprint,
  or a version string
- **THEN** the lint gate allows it without requiring a string resource

### Requirement: Localization Test Coverage and Locale Extensibility

The localization test suite SHALL exercise both English and Ukrainian for every localized
surface, including long-string and 200% font-scale variants, and adding a future locale SHALL
require only new resource files, with no change to any composable's public API.

#### Scenario: EN/UK suite runs
- **WHEN** the localization test suite executes
- **THEN** it renders every localized surface in English and Ukrainian, including long-string
  content and 200% font scale, and fails on a missing or placeholder translation key

#### Scenario: Adding a new locale
- **WHEN** a third locale is added later
- **THEN** it is added by supplying new `values-<locale>` resource files, and no composable's
  public function signature changes

### Requirement: AA Color Contrast and Non-Color Signaling

Text and UI affordances SHALL meet WCAG AA contrast — 4.5:1 for body text and 3:1 for large text
and UI affordances — computed after alpha compositing on the actual rendered surface, for all six
accents in both dark and light themes including on-accent foregrounds, and no state SHALL be
conveyed by color alone.

#### Scenario: Contrast measured post-compositing
- **WHEN** contrast is verified for body text, large text, or a UI affordance
- **THEN** the ratio is computed on the color as actually composited onto its surface (after any
  alpha blending), not the token's nominal value in isolation

#### Scenario: All accents pass in both themes
- **WHEN** any of the six accents is active in dark or light theme
- **THEN** its text/icon-on-accent (on-accent) foreground meets the applicable AA ratio

#### Scenario: Color is not the only signal
- **WHEN** a state such as online/offline, destructive, or content-type is conveyed
- **THEN** an equivalent non-color signal (icon, glyph, or label text) accompanies the color

### Requirement: Touch Targets, Focus, and Input Access

Interactive controls SHALL expose a hit target of at least 48dp without inflating the visible
control below its 22–38dp design size, SHALL show a 2px accent focus-visible ring with 2px
offset on keyboard/D-pad focus, dialogs and sheets SHALL receive focus on open and restore it to
the trigger on dismiss, and all exposed interactions SHALL be operable via keyboard, D-pad, or
switch access.

#### Scenario: Small visible control keeps a large hit target
- **WHEN** a control's visible size is between 22dp and 38dp
- **THEN** its touch target area is expanded to at least 48dp without changing the rendered
  control size

#### Scenario: Keyboard/D-pad focus is visible
- **WHEN** an interactive element receives focus via keyboard or D-pad
- **THEN** a 2px accent focus ring renders with a 2px offset

#### Scenario: Dialog/sheet focus handling
- **WHEN** a dialog or bottom sheet opens
- **THEN** focus moves into it, and on dismiss focus returns to the element that triggered it

#### Scenario: Non-touch input operates every exposed action
- **WHEN** a screen is navigated via keyboard, D-pad, or switch access
- **THEN** every exposed interactive action remains reachable and operable

### Requirement: Semantic Structure, Font Scaling, and Masked-Secret Safety

Interactive elements SHALL expose correct semantic roles and state (checked/selected/expanded/
disabled/error) without duplicate labels from merged descendants, layouts SHALL remain usable
without clipped content or lost actions at font scales 1.0, 1.3, and 2.0, and masked sensitive
content SHALL never expose plaintext in either the merged or unmerged semantics tree.

#### Scenario: State exposed via semantics
- **WHEN** a control has a checked, selected, expanded, disabled, or error state
- **THEN** that state is exposed through the corresponding Compose semantics property, readable
  by assistive technology

#### Scenario: No duplicate merged-descendant label
- **WHEN** a composable merges descendant semantics
- **THEN** the merged node exposes a single label, not the concatenation of a parent label and a
  duplicate child label

#### Scenario: Font scale resilience
- **WHEN** the system font scale is 1.0, 1.3, or 2.0
- **THEN** the affected screen renders without clipped text and without losing access to any
  action

#### Scenario: Masked content in both semantics trees
- **WHEN** a sensitive item is masked
- **THEN** neither its merged semantics node nor any unmerged descendant node exposes the
  underlying plaintext — only the masked placeholder is exposed

### Requirement: Reduced Motion and Accessibility Verification Strategy

Reduced-motion SHALL be honored automatically via the system signal with no separate in-app
motion setting, and each accessibility assertion SHALL be assigned to exactly one runner: JVM
token/contrast tests (contrast, token values), Paparazzi snapshots (visual/state rendering),
connected Compose UI tests (semantics roles/state, focus, 48dp bounds — run on a defined emulator
configuration whose CI availability is stated), and a maintained manual TalkBack checklist for
behavior that cannot be asserted deterministically. A gate SHALL NOT list a generic "Compose
semantics test" without the concrete command that executes it.

CI availability (stated per this requirement's own acceptance criterion): the connected/instrumented
job (`android-instrumented` in `.github/workflows/ci-android-build.yml`) is **CI advisory-only until
CopyPaste-k1l0 is resolved** — it runs with `continue-on-error: true` because the managed AVD does not
boot on arm64 macOS runners. The interim pre-merge catch mechanism until CopyPaste-k1l0 lands is a
mandatory local `:app:connectedDebugAndroidTest` run for security-relevant slices (S4, S5/S6, S8,
S9/S10, S12, S15), backed by Paparazzi/JVM proxies.

#### Scenario: CI availability is disclosed
- **WHEN** a connected Compose UI test is relied on as evidence for semantics/focus/hit-target
- **THEN** the emulator configuration's CI availability is stated inline: advisory-only until
  CopyPaste-k1l0 is resolved, with a mandatory local run as the interim pre-merge gate for
  security-relevant slices

#### Scenario: System reduced-motion is respected
- **WHEN** the OS reduced-motion setting is enabled
- **THEN** themed animations collapse to `MotionSpec.reduced` automatically, without requiring
  any in-app toggle

#### Scenario: Deterministic checks run in CI
- **WHEN** themed UI changes are submitted
- **THEN** automated Compose semantics tests and token-level contrast tests run and must pass

#### Scenario: Connected UI tests own semantics/focus/hit-target
- **WHEN** semantics roles/state, focus receive-and-restore, or the 48dp hit target are verified
- **THEN** the assertion runs as a connected Compose UI test on the defined emulator configuration,
  not as a JVM or Paparazzi check
- **AND** visible bounds (22–38dp) and semantics/touch bounds (≥48dp) are asserted separately

#### Scenario: Manual checklist covers the remainder
- **WHEN** a behavior cannot be deterministically asserted (e.g. TalkBack reading order or
  gesture navigation)
- **THEN** it is covered by a maintained manual TalkBack checklist exercised at milestones
