# Post-update critical review

Review target: the expanded 15-slice / 14-capability revision.

Verdict: **major improvement, but not implementation-ready yet**. The rewrite now reflects the
full-redesign intent, passes `openspec validate`, and covers most previously missing surfaces.
The remaining issues are narrower than before, but several are implementation-blocking or encode
incorrect Android behaviour.

## 1. What is now correct

- The proposal explicitly supersedes the old "screens stay de-styled" direction.
- `STYLEGUIDE.md` is correctly declared the source of truth and HTML is secondary.
- Scope now includes History, Preview, Devices, Pairing, all Settings tabs, onboarding,
  permissions, feedback, diagnostics, notifications, invisible/system-owned surfaces,
  localization, accessibility and visual regression.
- Material 3 is correctly treated as a behavioural/accessibility substrate rather than the
  visible design contract.
- Semantic tokens, exact radii, Inter/JetBrains Mono, motion, Lucide, EN/UK, 48 dp targets,
  masking invariants, Paparazzi and separate gates are now represented.
- Quick-paste/QS tile is correctly called out as net-new and deferred.
- Internal Compose APIs may now be refactored while IPC/FFI/behaviour remain protected.
- The traceability table is a useful start and is materially closer to the requested scope.

Do not discard this rewrite. Repair the issues below in place.

## 2. Blocking correctness issues

### B1. `recreate()` does not re-theme the whole back stack

Affected:

- `proposal.md` Appearance bullet
- `design.md` D5/R6
- `android-appearance/spec.md` "Persist on Save with recreate() propagation"
- `tasks.md` 3.3

The current text claims that calling `recreate()` on `SettingsActivity` causes the whole back
stack/app to re-theme deterministically. Android's `Activity.recreate()` recreates only that
Activity instance. It does not recreate stopped activities below it or other task activities.

This is especially confused because Settings is also embedded as a tab inside `MainActivity`.
Recreating the current Activity behaves differently depending on whether the user reached
Settings through the main shell or a standalone `SettingsActivity`.

Required repair:

- choose a real app-wide propagation mechanism;
- distinguish embedded `SettingsScreen` in `MainActivity` from standalone `SettingsActivity`;
- recommended: an application-scoped observable committed appearance state consumed by
  `CopyPasteTheme`, while draft preview remains locally scoped to Settings;
- alternatively define a reliable recreate-on-resume/version-token mechanism for every themed
  Activity, but do not claim `recreate()` alone updates the back stack;
- add scenarios for Save from embedded Settings, Save from standalone Settings, another Activity
  already in the task, and process recreation;
- ensure the Settings draft never leaks before Save.

### B2. The blur implementation proposal confuses content blur with backdrop blur

Affected:

- `design.md` D7
- `android-design-system/spec.md` translucency requirement
- `android-navigation-chrome/spec.md` frosted blur requirement
- `tasks.md` 1.4/4.2

`Modifier.blur` and a normal `RenderEffect.createBlurEffect` applied to the pill blur the pill's
own rendered layer/children. They do not automatically sample and blur content behind the pill.
The spec promises a 22 px **backdrop blur**, which requires a different rendering architecture
(captured backdrop/layer, window-level blur where applicable, or another proven implementation).

Required repair:

- name one technically valid backdrop-blur strategy for the app shell and sheets;
- specify API, performance, clipping, edge treatment and accessibility implications;
- protect icons/text from being blurred with the backdrop;
- define a measured fallback when true backdrop blur is not viable;
- keep the product requirement (real blur on API 31+) but stop listing `Modifier.blur` as if it
  independently satisfies it;
- add a prototype/spike gate in S1 before the design system commits to this architecture.

### B3. Visual-test infrastructure is scheduled too late to gate the screen slices

Affected:

- `design.md` D13/D14
- `tasks.md` global Gates and S2/S4-S14 ordering

The global gates require Paparazzi per slice, but Paparazzi is not added until S14, after nearly
all UI is implemented. The phrase "record earlier where useful" is not executable.

Required repair:

- move Paparazzi plugin/config/device fixtures/record+verify tasks to S1 or a new early S2;
- establish the baseline directory and CI command before the first screen slice;
- each screen slice must add and verify its own fixtures/baselines;
- keep a late close-out slice only for coverage audit and matrix completion;
- similarly introduce the localization and hardcoded-text gate before screen implementation,
  not after twelve slices of new UI text.

### B4. The content-type model is internally contradictory and miscounted

Affected:

- `proposal.md` ("11 content-type colors")
- `android-design-system/spec.md` CpColors
- `android-history/spec.md` "All eleven kinds map to distinct tokens"
- `tasks.md` 5.1

The guide lists these kinds: TEXT, URL, EMAIL, PHONE, CODE, JSON, NUMBER, COLOR, PATH, FILE,
IMAGE, SECRET — **12 kinds**. They do not map to 12 distinct colors:

- PHONE and NUMBER share `c-num`;
- PATH and FILE share `c-file` (the guide/table also uses path/file naming inconsistently);

The updated scenario simultaneously says "eleven distinct tokens" and then says two pairs share
tokens. Both cannot be true.

Required repair:

- define 12 supported kinds;
- define the exact canonical token field set and aliases;
- recommended canonical color fields: `cText`, `cUrl`, `cMail`, `cNum`, `cCode`, `cJson`,
  `cColor`, `cFile`, `cImage`, `cSecret` (10 unique semantic colors), with PHONE→cNum and
  PATH→cFile aliases;
- if retaining both `cPath` and `cFile` fields for parity, explicitly require identical values
  and do not call them distinct;
- update every count and scenario consistently.

### B5. `CpColors` is still incomplete relative to its own design decision

`design.md` D2 says selected/hover/pressed, content/status colors and accent-2 live in semantic
holders. `android-design-system/spec.md` defines surfaces, lines, text, status and content types,
but omits required overlay/state fields:

- `hover`
- `pressed`
- `selected`
- `scrim`
- disabled treatment or an explicit derivation rule
- card alias/relationship to elevated

Yet later requirements use `--hover`, `--pressed`, `--selected` and `scrim` as if these fields
already exist.

Required repair:

- add all style-guide semantic fields and exact dark/light values/derivation rules to the
  design-system requirement;
- define active accent and accent-2 ownership unambiguously (`AccentColor`/`CpAccent`);
- no screen spec should reference a CSS variable that has no Kotlin token contract.

### B6. M3 surface-container mapping remains underspecified

`design.md` says `surfaceContainer*=elevated/raised`, while the design-system spec only names
`surfaceVariant=elevated`. This still leaves `surfaceContainerLowest/Low/High/Highest`,
`primaryContainer`, inverse roles, disabled content and secondary/tertiary roles undefined.

Required repair:

- add an explicit role table, not a wildcard;
- recommended mapping remains:
  - lowest=bg
  - low=panel
  - container=elevated/card
  - high=raised
  - highest=raised2
- define or prohibit use of secondary/tertiary/inverse/container roles;
- define disabled alpha/tokens centrally;
- test every role the app actually consumes.

### B7. Several "open decisions" are already silently decided elsewhere

`design.md` still lists five open decisions, but specs/tasks already commit to answers:

- Share receiver: `android-system-surfaces` says it stays UI-less.
- ZXing `FLAG_SECURE`: `android-pairing` explicitly accepts its absence.
- tablet/foldable: proposal/specs already require them.
- Paparazzi is selected even though the exact compatible version/toolchain is unresolved.

Required repair:

- either mark these as resolved decisions with rationale, or remove the conflicting SHALLs;
- no implementation-ready spec may contain a SHALL whose governing product decision is still
  called open;
- keep only genuinely unresolved decisions in S0.

### B8. The proposed form-factor scope exceeds the confirmed decision

The confirmed answer was "Pixel". The new proposal adds Pixel phone + tablet + foldable goldens
and adaptive WindowSizeClass work, while landscape is excluded. This may be a good enhancement,
but it is a scope expansion not established by the prior decision.

Required repair:

- obtain explicit product approval for tablet/foldable implementation and golden coverage;
- otherwise set the epic acceptance target to one exact Pixel phone portrait configuration and
  treat wider screens as non-regression/stretch goals rather than dedicated layouts;
- specify exact width/height/density/API/navigation mode, not only marketing device names.

### B9. Lucide is not yet an actionable dependency decision

There is no official Lucide Kotlin/Compose artifact specified in the change. "A
Lucide-Compose artifact" is insufficient because community artifacts differ in package naming,
coverage, generated-code size, maintenance and license metadata.

Required repair:

- record exact Maven coordinate and pinned version;
- record repository, license and SBOM implications;
- verify compatibility with Kotlin 1.9.23 / Compose compiler 1.5.11;
- decide whether importing the whole icon pack violates APK/dependency-size expectations;
- if no acceptable artifact exists, generate/own a curated Lucide `ImageVector` subset from the
  upstream ISC-licensed SVG source and define its update script/license notice;
- do this in S0, before S2 can be estimated or accepted.

### B10. Paparazzi compatibility is asserted without a pinned compatible version

The proposal says Paparazzi is compatible with AGP 8.3.0/Kotlin 1.9.23 but gives no version.
Paparazzi releases are tightly coupled to LayoutLib/AGP/Gradle/Compose generations. Current
official releases include versions built around substantially newer toolchains; compatibility
must be demonstrated, not assumed.

Required repair:

- pin the exact Paparazzi version in the design;
- add a zero-production-code proof task that applies the plugin and snapshots one bundled-font
  Compose fixture on the current toolchain;
- define whether upgrading AGP/Kotlin/Gradle is allowed if no suitable version works;
- include Git LFS policy or explicitly reject it and quantify baseline repository cost;
- use Paparazzi's real generated task names in the gate (`recordPaparazziDebug`,
  `verifyPaparazziDebug` or version-appropriate equivalents).

Official Paparazzi documentation confirms its JVM snapshot/record/verify model and default
snapshot path, but the change must still select a compatible release:
https://cashapp.github.io/paparazzi/

## 3. Major specification gaps

### M1. The hardcoded-string gate is too narrow

It only promises to catch literals passed directly to Compose `Text()` or
`contentDescription`. It will miss literals passed through:

- shared component parameters (`title`, `subtitle`, `message`);
- Toast/Snackbar/notification builders;
- `stateDescription`, `onClickLabel`, dialog copy;
- error mappers and service code;
- string concatenation before reaching a composable.

Define either Android Lint plus a project AST/script gate across all user-facing sinks, or an
explicit allowlisted resource-only policy. The check must cover Kotlin UI, services,
notifications and accessibility semantics.

### M2. Localization completeness must account for non-translatable resources

"Every English key has a Ukrainian key" is too broad if `translatable="false"` keys, app name,
protocol literals or machine labels exist. Require every **translatable user-facing** key to have
UK coverage, fail placeholders, and explicitly allowlist non-translatable keys.

### M3. The traceability matrix is file-level, not state-level

The review requested `surface/state -> owner -> requirement -> slice -> fixture -> golden ->
a11y/l10n test`. The current matrix only maps groups of files to slices and then says S14/S15
will attach tests later.

Required repair:

- add a separate machine-checkable or Markdown state inventory;
- every loading/empty/error/disabled/masked/dialog/in-flight state must name its fixture and test;
- distinguish states that already exist from new states being intentionally introduced;
- do not invent an "error/degraded" state unless the current state model can emit it or the task
  explicitly includes the necessary presentation-state plumbing.

### M4. The proposal overstates the current baseline in a few places

- It says there are zero `androidTest/*.kt`; the directory currently contains only an asset, so
  this is technically true for Kotlin tests, but phrase it as "zero instrumented Kotlin test
  sources" to avoid ambiguity.
- It refers to `OwnDeviceInfo` and `PairedDevice` in the devices spec, but the actual roster model
  is `PairedPeer`; no `OwnDeviceInfo`/`PairedDevice` type exists in the inspected source.
- If those names are proposed presentation DTOs, mark them as new and assign their construction
  to S7. Otherwise use actual types.

### M5. Android units and CSS vocabulary are mixed

Specs repeatedly use `px`, `--token`, `currentColor`, `font-variant` and CSS-like focus-ring
language as executable Android acceptance criteria.

Cross-platform references are useful, but Kotlin acceptance must define:

- dp/sp units;
- Compose `Color`/alpha values;
- semantic token property names;
- how focus ring is drawn in Compose;
- how tabular figures are achieved (JetBrains Mono's fixed-width digits or a verified font
  feature API), rather than requiring a nonexistent generic CSS `font-variant` setting.

### M6. Save atomicity needs implementation-accurate wording

The spec requires all settings to be in one `SharedPreferences.Editor.commit()`, which is
reasonable. It also claims a force-stop yields either the fully old or fully new set. Keep this
only if tests can establish the storage guarantee on supported Android versions. More
importantly, define how write failure (`commit() == false`) is surfaced: Save must not clear
dirty state or show success when commit fails.

### M7. Theme migration contract should preserve the established key names

The text says migration "must not use the same key strings it deletes". The real issue is that
the legacy migration currently removes `theme_mode`/`accent`. It is not necessary to invent new
persistence keys; the safer repair is to version/update the one-time migration so it stops
deleting the new canonical keys before the new getters are introduced.

Specify:

- migration latch/version key;
- ordering in `CopyPasteApp.onCreate`;
- old keys removed;
- canonical new keys retained (`theme_mode`, `accent` unless intentionally changed);
- upgrade tests from pre-redesign preferences and from already-migrated installs.

### M8. Current navigation restoration requirement adds a product behaviour

The existing `MainShell` uses `rememberSaveable`, which restores across Activity recreation but
not necessarily a fresh process/task in the way the spec promises. Persisting last tab across
process death is a new behavioural choice. Confirm it, and specify storage/SavedState behaviour,
or narrow the requirement to state restoration supported by `rememberSaveable`.

### M9. Landscape language is contradictory

The proposal says landscape phone is out of scope. The navigation spec nevertheless has a
landscape scenario requiring the portrait-derived adaptive layout to be used. That is still a
landscape acceptance requirement.

Either:

- explicitly support functional landscape fallback and test it; or
- declare landscape unsupported/locked where appropriate and remove the scenario.

### M10. Accessibility gates need executable ownership

The global gate says Compose semantics/a11y tests run per slice, but instrumented Compose tests
are not part of `android-verify.sh` and Paparazzi accessibility snapshots have different
capabilities. Assign each assertion to one runner:

- JVM token/contrast tests;
- Paparazzi visual/accessibility snapshots;
- connected Compose UI tests;
- manual TalkBack checklist.

Define the connected emulator configuration and CI availability. Do not list a generic
"Compose semantics test" gate without the command that executes it.

### M11. Build cleanliness workflow is still incomplete

`android-verify.sh` requires a clean tree. The plan says one logical commit per green slice but
does not give the exact order.

Define:

1. run fast checks while dirty;
2. review/stage/commit the logical slice;
3. run clean-tree `android-verify.sh`;
4. inspect generated diffs/status;
5. if failure or diff occurs, fix/amend and rerun;
6. only then add evidence/close the `bd` issue.

Also resolve how the pre-existing deleted HTML file is kept out of the spec/setup commit.

## 4. Missing or weak acceptance details

- Theme crossfade is named but not assigned an explicit task/scenario.
- `CpMotion.reduced` must affect all animations, including the existing spring-based tab pop;
  the current nav code uses a spring not covered by only setting duration tokens to zero.
- Selected/hover semantics on touch should not require desktop-only hover behaviour.
- The 48 dp requirement needs a test method; semantics bounds and visual bounds must be
  distinguished.
- Notifications should test channel migration: Android channel importance cannot be changed
  after channel creation, so "existing settings left untouched" must be reconciled with any new
  desired metadata.
- Pairing/Devices specs encode detailed current behaviour; preservation tests should focus on
  public outcomes, not freeze incidental private call ordering unless security requires it.
- `ShareReceiverActivity` is declared UI-less in the system spec; remove open-decision wording
  everywhere if this is final.
- `PortraitCaptureActivity` is accepted without `FLAG_SECURE`; record the privacy rationale as a
  resolved decision, not an open item.
- The visual baseline policy must state whether images are stored directly or through Git LFS.
- Add RTL/pseudo-locale stress even if RTL translation is not shipped yet; at minimum do not
  hardcode left/right where start/end is required.

## 5. Required edit order

1. Resolve product decisions: form factors, share feedback, ZXing privacy, blur fallback,
   Lucide source, exact Paparazzi/toolchain version.
2. Fix architectural errors: app-wide theme propagation and true backdrop blur.
3. Complete design-system contracts: token fields, exact M3 role table, kind/token aliases,
   Android units.
4. Move golden/localization/a11y infrastructure ahead of screen slices.
5. Replace the file-level traceability table with state-level evidence mapping.
6. Make every gate executable with exact commands and ownership.
7. Reconcile all specs/tasks with the resolved decisions.
8. Run `openspec validate android-material3-redesign --strict` and a repository-reference audit
   for every named type/file/task.

## 6. Readiness gate

The change can move to implementation only after:

- B1-B10 are resolved, not merely acknowledged;
- open decisions no longer conflict with SHALL requirements;
- exact dependency versions/coordinates are pinned or an explicit pre-implementation spike is
  the first blocking task;
- Paparazzi/localization gates exist before screen implementation;
- all token counts/aliases and M3 roles are internally consistent;
- every named model either exists or is explicitly introduced by a task;
- app-wide Save behaviour is technically correct for both embedded and standalone Settings;
- backdrop blur has a proven strategy and fallback;
- form-factor scope is approved;
- all gate commands are concrete and can run in the current repository/toolchain.
