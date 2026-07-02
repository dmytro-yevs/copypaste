# Full deep audit — android-material3-redesign

Audit basis:

- all core documents (`proposal.md`, `design.md`, `tasks.md`);
- all 14 capability specs;
- `component-inventory.md` and `behavior-and-state-coverage.md`;
- root `STYLEGUIDE.md` and `copypaste-design-reference.html`;
- current Kotlin, Manifest, resources, Gradle, scripts, and Android CI workflows.

Validation result: `openspec validate android-material3-redesign --strict` passes.

Overall verdict: **the change is structurally mature but still NOT ready to leave S0**. Formal
validity is not the problem. Several SHALL statements are factually false against the repository,
some security assumptions are unsafe, and a number of tasks/gates are not executable as written.

## 1. Critical blockers

### P0-1 — ZXing scanner privacy decision is based on a false premise

Affected:

- `design.md` resolved decision for `PortraitCaptureActivity`;
- `android-pairing/spec.md` scanner ownership boundary;
- `tasks.md` 8.3;
- behavior matrix C5.

The spec accepts missing `FLAG_SECURE` because “camera preview carries no secret.” That is false.
The scanner camera preview necessarily displays the other device's pairing QR. The documented QR
format contains `CPPAIR1.<fingerprint>.<token>...`; the token is pairing material. A screenshot or
recents capture of the scanner can therefore capture a still-valid pairing credential.

Required correction:

- reverse the resolved decision;
- set `FLAG_SECURE` on `PortraitCaptureActivity` before the camera preview renders;
- prevent recents/screenshot capture for the scanner window;
- add a connected window-flag test for PairActivity and PortraitCaptureActivity;
- keep ZXing's internal UI otherwise unskinned;
- document whether external scanner apps remain outside CopyPaste's control.

This is a security blocker, not a visual preference.

### P0-2 — Notification inventory is factually incomplete

Affected: `android-system-surfaces/spec.md`, S12, behavior matrix notification variants.

The spec claims exactly three channels. The code has four:

1. `copypaste_service`
2. `copypaste_copy_event`
3. `copypaste_pair_request`
4. `copypaste_sync` (`NotificationHelper.CHANNEL_SYNC`) for native/encryption-library failure

The fourth channel currently has hardcoded English name/description/content and must be part of
localization and channel migration.

The PendingIntent description is also wrong: foreground Open targets `MainActivity`, but
Pause/Resume targets `CaptureControlReceiver`, not MainActivity.

Required correction:

- enumerate all four channels and every notification ID/variant;
- specify native-unavailable importance/category/visibility/action behaviour;
- correct every PendingIntent target;
- define channel name/description update strategy for existing installs;
- test API 26+ idempotence and POST_NOTIFICATIONS-denied paths;
- include transient `ServiceRestartWorker` notification ID 1010 and its API 26–30 path.

### P0-3 — `onError = #FFFFFF` violates the project's own AA requirement

`android-design-system/spec.md` maps `onError` to white. Calculated contrast:

- white on dark `err #E5645F`: approximately **3.32:1**;
- white on light `err #D64545`: approximately **4.38:1**.

Both fail the 4.5:1 body-text requirement; the dark value fails substantially. Black/near-black
has approximately 6.32:1 and 4.80:1 respectively.

Required correction:

- introduce a contrast-safe `onError` token per theme or avoid filled error surfaces;
- add error/onError and error-container combinations to token contrast tests;
- do not claim all theme roles satisfy AA while hardcoding a failing pair.

### P0-4 — Android system chrome and pre-Compose first paint are unspecified

The app supports forced Dark/Light independent of OS theme. Current XML themes derive system icon
appearance from DayNight/OS state. `SecureWindowChrome` currently preserves only edge-to-edge and
FLAG_SECURE SideEffects.

Failure mode: app forced Light while OS is Dark can leave light status-bar icons over a light app
surface; the reverse can also fail. Before Compose draws, the MaterialComponents window background
can flash a non-canonical color.

Missing surfaces:

- status-bar icon appearance;
- navigation-bar icon appearance/contrast;
- pre-Compose window background;
- Android 12+ splash screen;
- recents thumbnail policy;
- launcher/share-target label and icon appearance.

Required correction:

- add a system-chrome requirement driven by resolved app theme;
- preserve the two existing SideEffects but add explicit
  `WindowInsetsControllerCompat.isAppearanceLightStatusBars/navigationBars` handling;
- specify XML theme/window/splash resources for first paint with no wrong-theme flash;
- inventory launcher icon, splash, recents and sharesheet entry as Preserve/Restyle/N-A;
- add light/dark system-bar tests and manual Pixel acceptance.

### P0-5 — Appearance capability contradicts the actual Display tab

`android-appearance/spec.md` says inspecting Settings → Display exposes **only** Theme, Accent,
Translucency and Mask sensitive.

The real Display tab also contains:

- sensitive warnings;
- reveal guard;
- allow screenshots;
- image max height;
- preview delay;
- preview lines.

Product already required preserving all existing functional settings. The scenario is literally
false and could instruct an implementer to delete valid controls.

Required correction: say the **Appearance subsection** contains exactly those four appearance
controls, while all existing Display behaviour controls remain present and unchanged.

### P0-6 — Content-kind model names a type that Android does not have

The specs repeatedly refer to twelve `HistoryEntry.kind` values. Android has no `HistoryEntry.kind`.
Current data is:

- stored `ClipboardItem.contentType` (`text`, MIME text, image, file);
- `TextKind.classify(snippet)` through Rust FFI for nine text sub-kinds;
- orthogonal `ClipboardItem.isSensitive`;
- IMAGE and FILE derived from content type.

SECRET is not a stored kind and current UI deliberately keeps the underlying kind label for
sensitive content. The redesign instead appears to introduce a SECRET visual override/lock tile.
That is a valid product choice, but it must be explicit.

Required correction:

- define a Kotlin presentation enum such as `ContentVisualKind`;
- define resolver precedence exactly:
  1. `isSensitive` → SECRET visual kind if that is the approved new behaviour;
  2. image/file from canonical content-type predicates;
  3. text subtype from `TextKind.classify`;
  4. unknown/stub → TEXT;
- state that stored/sync content type and Rust classifier contracts do not change;
- assign enum/resolver implementation and unit tests to S1/S5;
- replace references to nonexistent `HistoryEntry.kind`.

### P0-7 — Partial-span masking semantics are incomplete

The History spec requires only the sensitive spans to be visually masked. A Compose `Text`
semantics node can still expose its entire underlying AnnotatedString unless semantics are replaced.
The full-item masking scenarios do not prove span-level safety.

Required correction:

- define a sanitized accessibility string where sensitive spans are replaced by a localized
  placeholder while non-sensitive spans remain readable;
- assert plaintext is absent from merged and unmerged semantics for partial masking;
- add a dedicated span-masking semantics test with a synthetic mixed string;
- cover Preview if partial spans can reach it.

### P0-8 — Gradle gates do not run from the documented working directory

The repository root has no `./gradlew`; the wrapper is `android/gradlew`. Current gates use:

- `./gradlew compileDebugKotlin`
- `./gradlew lint`
- `./gradlew :app:testDebugUnitTest`
- `./gradlew verifyPaparazziDebug`
- `./gradlew connectedDebugAndroidTest`

Run from repository root, these fail immediately.

Required correction: standardize either:

- `cd android && ./gradlew :app:<task> -x buildCargoNdk`, or
- `android/gradlew -p android :app:<task> -x buildCargoNdk`.

Use module-qualified tasks and document when native build exclusion is safe. Match the existing
CI convention (`working-directory: android`).

### P0-9 — Test dependencies and CI work are not assigned

The evidence matrix relies on Robolectric, Paparazzi, connected UI tests, custom lint/AST checks,
and artifact upload. Current Gradle has no Robolectric dependency and no Paparazzi plugin. Current
Android CI does not run Paparazzi or the new hardcoded-string gate. Existing connected CI does run
instrumented tests, but new test sources/jobs/artifacts still need wiring.

Required correction:

- add explicit S0/S2 tasks for Robolectric dependency/version;
- add Paparazzi job or step to Android CI;
- upload Paparazzi failure/diff artifacts;
- wire hardcoded-text and localization completeness gates;
- configure Kotlin compiler warnings-as-errors separately from Android Lint warnings-as-errors;
- add exact workflow filenames/jobs to tasks;
- do not label `./gradlew lint` as warnings-as-errors until configuration is added.

### P0-10 — Tablet/foldable scope is asserted as product-approved without evidence

The confirmed product answer in this review thread was “Pixel.” The updated design declares
phone + tablet + foldable responsive work “Product-approved.” That approval was not recorded.

Required correction:

- obtain explicit approval for tablet/foldable implementation and three-form-factor goldens; or
- make one exact Pixel portrait target the acceptance baseline and treat wider widths as best-effort
  non-regression;
- do not label a proposed scope expansion “resolved/product-approved” without the decision.

### P0-11 — Scanner/Pairing spec confuses SAS and fingerprint responsibilities

The existing SAS flow displays a six-digit SAS and may also show peer metadata/fingerprint. The
spec heading “SAS confirmation shows the full fingerprint” risks replacing or obscuring the actual
Short Authentication String.

Required correction:

- explicitly require the six-digit SAS as the primary match decision;
- state whether full fingerprint is supplemental metadata;
- preserve Match/Doesn't match semantics and watchdog/terminal states;
- test that neither SAS nor pairing tokens enter logs/notifications.

### P0-12 — Preview requirement changes behaviour without acknowledging it

`android-preview/spec.md` says opening Preview makes the underlying list not visible. Current
Preview has phases: Peeking is a centered card over a scrim with the list behind it; Pinned is the
expanded interaction mode. The proposal simultaneously forbids behavioural drift.

Required correction:

- specify Peeking and Pinned phases explicitly;
- decide whether the list remains visually present behind the Peeking scrim;
- preserve drag-up-to-pin, swipe/dismiss and back arbitration unless intentionally changed;
- list all actual toolbar actions: Copy, Pin/Unpin, Delete, and conditional Open/Save;
- state sensitive-mode availability for non-plaintext actions;
- update tasks and golden fixtures for both phases.

## 2. Design-system completeness

### 2.1 Missing spacing, elevation and component-dimension contracts

The plan defines colors, shapes, typography and motion, but not:

- `CpSpacing` for `2/4/6/8/11/14/16/20/24`;
- `CpElevation`/shadow mapping for sh1/sh2/sh3;
- component geometry such as toggle 38×22, tile 32–36, active nav pill 50×38, icon sizes,
  QR size and SAS cells;
- centrally named alpha values beyond selected/disabled.

At the same time it bans raw dp/sp/alpha outside token files. That is impossible to follow for
documented component geometry.

Required correction:

- introduce `CpSpacing`, `CpElevation`, `CpDimensions` and semantic alpha/state tokens;
- or explicitly allow documented component-local constants in shared component files;
- define Compose shadow approximations because CSS blur/offset/spread cannot be copied verbatim;
- add tests/static checks for token use.

### 2.2 Typography is still underspecified for implementation

`CpTypography` names five semantic ranges, but no exact Android values or M3 role mapping are
selected. “21–24” and “13–15” allow visually different implementations.

Required correction:

- choose exact sp, weight, line height and tracking for every Android semantic role;
- map semantic roles to M3 Typography roles where Material components consume them;
- define available bundled weights versus synthetic weights;
- verify Inter/JetBrains font resources and licensing;
- define letter spacing in `em`/Compose units and large-font behaviour.

### 2.3 Icon sizing confuses tile size with glyph size

The iconography spec calls a 32–36dp content-type **glyph**. STYLEGUIDE defines a 32–36px tile;
the glyph is smaller (reference uses roughly 1em/15px inside a 30–34px tile). A full-size glyph
would crowd the tile.

Required correction:

- separate tile container size from glyph box size;
- select exact Android sizes by role;
- reconcile nav glyph 24dp with the HTML reference's 21px glyph inside a 50×38 active pill;
- keep 48dp touch target separate from visual icon/container dimensions.

### 2.4 Full M3 defaults can still leak brand-inconsistent colors

The explicit table leaves secondary, tertiary, containers, inverse roles and error containers at
M3 defaults and merely prohibits screens from referencing some of them. Material components may
consume roles internally even when app code does not explicitly reference them.

Required correction:

- either map the complete ColorScheme to canonical semantic values;
- or override every shared Material component's colors and prove no default role is rendered;
- include errorContainer/onErrorContainer and disabled states;
- add component goldens that detect default purple/tonal leakage.

### 2.5 Style-guide field parity is not actually verbatim

The style-guide Kotlin sample defines both `cPath` and `cFile` fields with identical values. The
new spec removes `cPath` and claims verbatim §11 parity. Semantic aliasing is reasonable, but the
contract is not verbatim.

Required correction: record this as an explicit product/design-system override, or retain both
fields and assert equality. Avoid claiming both “verbatim” and “field removed.”

## 3. Proposal audit

### Why

Status: mostly correct.

Issues:

- “three-agent audit” is unnecessary and unverifiable provenance; use “repository audit.”
- “desktop already implements it” should be verified against actual desktop state or softened to
  “desktop epic is implementing it.”
- full parity must acknowledge OS-owned surfaces and Android-specific affordances, which later text
  does correctly.

### What Changes

Status: good scope, but incomplete resources.

Missing from Impact/scope:

- XML themes and night themes;
- `colors.xml`, `dimens.xml` reconciliation;
- launcher/adaptive icon and splash resources;
- system-bar appearance;
- CI workflows and test dependencies;
- possible new stateless screen/presentation models for Paparazzi fixtures.

### Capabilities

Status: capability split is appropriate.

Missing/incorrect:

- system chrome/first-paint responsibilities are not clearly owned;
- content visual-kind resolver ownership is absent;
- fourth notification channel is absent.

### Impact

Status: underestimates build/process changes.

Add:

- `android/build.gradle.kts`, version catalog, app Gradle, Gradle properties if upgraded;
- `.github/workflows/ci-android-build.yml` and possibly visual-regression workflow;
- CODEOWNERS/baseline ownership if required;
- XML theme/icon resources;
- test source sets and baseline storage.

## 4. Design decisions D1–D15

### D1 — Source of truth

Status: acceptable, with one correction.

Record every approved override explicitly. System theme, UK, real blur and goldens are recorded;
removing `cPath` is not.

### D2 — Semantic layer/M3 mapping

Status: incomplete.

Fix onError contrast, full ColorScheme leakage, content-kind resolver, cPath claim, spacing and
elevation ownership.

### D3 — Shapes/Type/Motion

Status: direction correct, values incomplete.

Add exact typography and component dimensions. Reduced motion must affect every spring/infinite
transition, not only tokenized tween durations.

### D4 — Theme model

Status: acceptable.

Add invalid/corrupt persisted enum fallback and system-theme change tests. Define resolved
`isDark` as state, not a persisted third palette.

### D5 — Draft/app-scoped committed state

Status: good architecture but underdesigned.

Specify:

- holder type and ownership;
- initialization from preferences on process start;
- publish only after `commit()==true`;
- threading/main-state update;
- whether `maskSensitive` belongs in the same observable state;
- lifecycle behaviour for stopped Activities;
- no duplicate Settings instances producing stale snapshots.

### D6 — Migration

Status: generally correct.

S0 should audit/plan; production migration code should land in S3 with its tests. Define old and
new latch versions precisely and test upgrade matrices.

### D7 — Backdrop blur

Status: correctly recognizes real backdrop blur, but implementation remains a spike.

Add performance acceptance (frame time/memory), nested scroll behaviour, clipping/radius, capture
recursion avoidance and fallback trigger. A deterministic golden stand-in does not validate the
real device effect; add manual/device screenshot acceptance.

### D8 — Lucide

Status: reasonable pending spike.

Add explicit removal of the `material-icons-extended` dependency, not just imports. Define curated
icon list and APK-size budget.

### D9 — Adaptive layout

Status: blocked on actual product approval.

Also choose concrete WindowSizeClass breakpoints/max widths. “Widen responsively” is not testable.

### D10 — Internal API changes

Status: correct.

Add a rule that UI refactors cannot change state/event ordering or external intent contracts unless
the corresponding capability explicitly says so.

### D11 — Token-only screens

Status: currently impossible due missing dimension/elevation tokens.

Broaden exception to documented shared-component geometry or add complete token holders.

### D12 — Masking

Status: strong for full masking, incomplete for partial spans and scanner privacy.

Add sanitized partial-span semantics and pairing QR/scanner screenshot protection.

### D13 — Paparazzi

Status: direction correct.

“Byte-identical” repeated output conflicts with allowing a non-zero diff threshold. Require
deterministic output within a pinned threshold/environment, or require exact equality and set
threshold zero. Add real-blur device validation.

### D14 — Sequential slices

Status: correct concept.

Add explicit dependency graph and bd links. S1 depends on S0 blur outcome; S2 depends on S0
Lucide/Paparazzi; all screen slices depend on S1/S2; S14/S15 depend on all screens.

### D15 — Git

Status: safe direction.

Add the actual initial spec commit step and explicit exclusion path for the pre-existing deleted
HTML file. Define what happens when clean-tree verification fails after commit (fix/amend/re-run).

## 5. Risk audit

Existing R1–R10 are useful. Add:

- R11: forced theme versus system-bar/pre-Compose flash;
- R12: scanner screenshot leaks pairing QR token;
- R13: M3 unmapped-role color leakage;
- R14: golden baseline/repository-size explosion;
- R15: Paparazzi screens instantiate Settings/native/Android services and require presentation
  seams/fakes;
- R16: full localization conversion can alter formatted security/error messages;
- R17: app-scoped state can diverge from failed preference commits;
- R18: CI runtime explosion from full native build + Paparazzi + connected tests per slice/PR.

## 6. Task audit S0–S15

### Global gates

Problems:

- wrong Gradle working directory;
- warnings-as-errors not configured;
- Robolectric not added;
- no concrete Android Paparazzi CI task;
- no hardcoded-text script filename/command;
- connected CI exists but tasks incorrectly defer availability definition to S15/S0;
- “scale to the slice” is ambiguous.

Required: define a per-slice gate template with exact required/optional/N-A gates and artifact paths.

### S0

Good: dependency and blur spikes are correctly early.

Missing:

- create bd epic and all slice issues with dependencies/acceptance;
- pin exact branch base commit SHA;
- decide tablet/foldable scope with explicit product approval;
- system-bar/first-paint prototype/decision;
- content visual-kind model decision;
- exact CI job plan;
- Robolectric/version plan;
- state/evidence matrix reconciliation command/script.

Move production migration edits from 0.7 to S3; S0 should only confirm the call/order and specify
the migration.

### S1

Add:

- spacing/elevation/component dimensions;
- full ColorScheme or explicit component overrides;
- contrast-safe onError;
- system-bar appearance and XML first-paint tokens;
- invalid enum fallback;
- real-blur performance criteria;
- content visual-kind enum/resolver if foundation-owned.

### S2

Add:

- remove `material-icons-extended` dependency;
- exact icon-role size table;
- stateless/presentation seams so Paparazzi does not instantiate repositories/native/services;
- Robolectric dependency;
- CI workflow edits and artifact upload;
- actual hardcoded-text script path/command;
- warnings-as-errors configuration.

### S3

Add:

- holder implementation/initialization;
- commit-success-before-publish ordering;
- corrupted preference fallback;
- all existing Display controls preserved;
- system-bar icon update on live preview and committed update;
- maskSensitive propagation semantics.

### S4

Add:

- exact max width/breakpoints;
- status/navigation icon appearance;
- Peeking/overlay z-order interaction with nav if relevant;
- real blur performance/manual-device acceptance;
- per-slice golden/localization/a11y subtasks explicitly.

### S5

Add:

- effective content-kind resolver;
- absent source-app/origin metadata rules;
- all row actions (tap copy, pin, delete, reveal, selection/bulk);
- partial-span sanitized semantics;
- source-app icon fallback;
- pagination/concurrent refresh;
- exact error-state plumbing source;
- golden/localization/a11y checkboxes.

### S6

Add:

- Peeking versus Pinned modes;
- complete toolbar actions and kind-based availability;
- gesture arbitration with zoom/pan;
- file URI/open/save error details;
- text loading/null/error state;
- same partial/full masking resolver as History.

### S7

Add:

- always-expanded decision in tasks;
- absent field/placeholder policy for own and peer cards;
- exact presentation DTO mapping;
- connected copy-feedback test;
- full dialog dismissal/in-flight guard requirements;
- golden/localization/a11y subtasks.

### S8

Security correction required: scanner FLAG_SECURE.

Also add:

- six-digit SAS remains primary;
- watchdog/idle/timeout/abort/reset states;
- external deep-link validation and replay/expiry handling preservation;
- camera permanent-denial Settings path;
- no sensitive logs.

### S9

Current tasks are far too broad relative to the detailed settings matrix.

Fix 9.2 to say only Save-owned fields enter the batch; immediate/ephemeral fields stay outside.
Add checkboxes for each tab/control group, commit failure, max-items prune confirmation,
import/export/vacuum results, transport-immediate behaviour and notification-toggle independence.

### S10

Add explicit card order, skip/re-entry behaviour, crash export failure, per-API permission states,
force-stop guidance, OEM fallback and reduced-motion onboarding entry animations.

### S11

One checkbox is insufficient for toast, banners, sync badge/sheet, all dialogs, About and Logs.
Split into separate tasks with fixtures and behavioural acceptance. Remove Save from “async loading”
unless Save is intentionally made asynchronous; SharedPreferences `commit()` is synchronous.

### S12

Add:

- fourth notification channel;
- exact notification variants/targets;
- channel localization/migration strategy;
- scanner FLAG_SECURE boundary (or keep under S8);
- ShareReceiver log redaction (current code logs URI in failure paths);
- FileProvider grant tests;
- launcher/splash/sharesheet/recents dispositions.

### S13

Good as a completion audit. Add:

- translator/native-language review criteria for Ukrainian;
- plural resources (currently none);
- pseudo-locale/RTL command;
- allowlist review and format-argument parity test;
- all four notification channels.

### S14

Good as late audit. Resolve byte-equality versus threshold. Add baseline count/size budget and CI
artifact verification. Confirm no auto-update workflow can push Android baselines directly.

### S15

Add exact manual checklist path, device/AVD identifiers, evidence storage and archive conditions.
Do not archive until every bd slice and every required gate is complete.

## 7. Capability-spec audit

### android-design-system

Strengths: semantic colors, aliases, motion, blur distinction and M3 container ladder.

Required fixes:

- onError AA failure;
- missing spacing/elevation/dimension contracts;
- full M3 default leakage;
- exact typography;
- cPath/verbatim contradiction;
- presentation-kind resolver;
- system-bar/pre-Compose theme.

### android-appearance

Strengths: draft isolation, commit failure, versioned migration, app-scoped propagation.

Required fixes:

- “only four controls in Display” false;
- holder lifecycle/initialization;
- publish only after successful commit;
- malformed preference fallback;
- maskSensitive observable behaviour;
- live-preview system-bar icon contrast.

### android-iconography

Strengths: one provider, fallback, semantics and legacy removal.

Required fixes:

- tile versus glyph size;
- exact size table;
- remove Gradle dependency;
- launcher/notification/system icon boundary;
- fallback glyph semantics and test.

### android-navigation-chrome

Strengths: real backdrop requirement, insets, restored tab boundary and reduced spring.

Required fixes:

- exact breakpoint/max width;
- system-bar icon appearance;
- deterministic choice for IME (hide versus move, not either);
- exact bottom offset (choose 10, 11 or 12dp for golden stability);
- real-blur device acceptance.

### android-history

Strengths: anatomy, colors, states, grouping and full-item masking.

Required fixes:

- real Android presentation-kind model;
- SECRET precedence;
- partial-span semantics;
- complete action/bulk behaviour;
- absent metadata rules;
- Android dp/sp semantic names instead of CSS/px;
- error-state plumbing.

### android-preview

Strengths: image/file failures, large content and full masking gap recognized.

Required fixes:

- Peeking/Pinned behaviour;
- underlying list visibility decision;
- Open/Save actions;
- sensitive action availability;
- text-load failure;
- gesture arbitration with image transform;
- partial spans if applicable.

### android-devices

Strengths: field grids, lifecycle states, dialogs and security invariants.

Required fixes:

- use dp/sp/Compose wrapping, not CSS `word-break`;
- own-device absent-field policy;
- exact `OwnDeviceInfo`/`PairedDevice` adapters;
- whether Verified is truthful for every peer;
- exact offline/reconnecting source;
- avoid claiming “same weight” when missing rows are hidden;
- localized date/RTT/placeholder rules.

### android-pairing

Strengths: entry points, progress, errors, IPC preservation.

Required fixes:

- scanner FLAG_SECURE;
- six-digit SAS versus fingerprint distinction;
- current watchdog/idle/timeout/reset states;
- QR expiry source and replay rules;
- permanent camera denial;
- sensitive logging prohibition.

### android-settings

Strengths: persistence modes and atomic Save-owned batch.

Required fixes:

- qualify “all Settings use draft” to Save-owned fields;
- “every control supports every state” is impossible — use applicable states;
- Save is synchronous, not inherently an async loading action;
- complete per-control requirements from behavior matrix;
- notification sound remains independent of notification posting;
- commit failure and immediate side-effect rollback semantics.

### android-onboarding-permissions

Strengths: conditional OEM state, OS boundary and resume refresh.

Required fixes:

- reconcile “Notifications required” with a working Skip path and repeated onboarding behaviour;
- define below-API-33 card status/action;
- export failure feedback;
- reduced-motion entrance animations;
- ensure foreground-service “always granted” wording means install-time declaration, not runtime
  permission state.

### android-feedback-states

Strengths: toast replacement, non-color signals, sync states and log feedback.

Required fixes:

- success toast should map to `ok`, not current primary (task must ensure change);
- Save should not be treated as asynchronous unless redesigned intentionally;
- define banner dismissal/action semantics per banner;
- detail-sheet account masking format;
- clear-logs confirmation/error path;
- exact About repository/license sources.

### android-system-surfaces

Strengths: invisible overlay and OS ownership boundaries are detailed.

Required fixes:

- four channels, not three;
- PendingIntent targets;
- channel migration/localized metadata;
- scanner privacy;
- ShareReceiver URI/content log redaction;
- launcher/splash/recents/sharesheet entry;
- transient restart notification.

### android-localization-accessibility

Strengths: broad sink coverage, EN/UK, contrast, focus and test ownership.

Required fixes:

- “new string” scenario must say translatable string, consistent with exemptions;
- exact dp terminology instead of px;
- Ukrainian format-argument/plural parity test;
- pseudo-locale/RTL normative scenario;
- error/onError contrast;
- touch bounds test feasibility validated on Compose semantics;
- real translator review, not only key completeness.

### android-visual-regression

Strengths: pinned toolchain proof, representative matrix, no auto-accept principle.

Required fixes:

- byte-identical versus diff threshold contradiction;
- exact device configs after product approval;
- stateless/fake seams for screens;
- real blur excluded from JVM stand-in and covered elsewhere;
- CI job/artifact tasks;
- baseline size budget and storage decision;
- avoid full phone×tablet×fold for every trivial component unless justified.

## 8. Inventory/evidence audit

### Component inventory

The 116 unique composable names and 13 Activities are now covered. This is good.

Still missing resource/system-visible inventory:

- launcher/adaptive icon;
- splash/starting window;
- XML themes/day-night values;
- system bar appearance;
- recents thumbnail;
- sharesheet target label/icon;
- notification small icon variants.

### Behaviour/state coverage

The matrix is much improved and captures Settings persistence modes. Remaining corrections:

- notification channel count;
- scanner privacy;
- preview phases;
- partial-span masking;
- content visual-kind resolver;
- all evidence labels must map to actually configured dependencies/jobs;
- `SupabasePollWorker` itself does not post a notification; its UI consequence is sync-error state;
- ShareReceiver currently logs URI values on failure, contrary to desired privacy row;
- error/degraded History is new and requires exact state source/adaptation.

## 9. Required correction order

1. Fix security/factual blockers: scanner FLAG_SECURE, four notification channels, PendingIntents,
   onError contrast, Display-control scenario.
2. Define actual Android content visual-kind and partial-span semantics.
3. Add system chrome/first paint/launcher/splash resource scope.
4. Repair executable gates, test dependencies and CI ownership.
5. Resolve tablet/foldable approval.
6. Complete design tokens: spacing/elevation/dimensions/exact typography/full M3 colors.
7. Reconcile Preview phases and Settings applicability/async wording.
8. Expand thin tasks S9–S12 and add bd creation/dependency tasks.
9. Update inventory/evidence for resource surfaces and real test jobs.
10. Re-run strict OpenSpec validation plus repository-reference checks.

## 10. Readiness verdict

After these corrections, the change may start S0. S1 must remain blocked until S0 produces:

- approved device/form-factor matrix;
- pinned Lucide and Paparazzi decisions/proofs;
- backdrop-blur prototype and fallback;
- executable Gradle/CI gates;
- system-chrome/first-paint decision;
- content visual-kind contract;
- corrected security decisions;
- reconciled component/resource/behaviour inventory.

Current status: **formal OpenSpec valid, implementation readiness rejected**.
