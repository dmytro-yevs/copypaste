# Critical review — `android-material3-redesign`

Status: **the current change MUST NOT be implemented as written**.

This document is a review brief for the agent that will repair `proposal.md`,
`design.md`, `tasks.md`, and the delta specs. It records confirmed product decisions,
repository evidence, contradictions, missing scope, and the required shape of the
replacement plan.

## 1. Confirmed product decisions

These decisions come from the product owner and override the current change text where
they conflict:

- Root `STYLEGUIDE.md` is the source of truth.
- `copypaste-design-reference.html` is the working visual reference. If it conflicts with
  the style guide, the style guide wins.
- The output is a production Android design implemented in Kotlin/Jetpack Compose. Material
  3 may provide behaviour/accessibility primitives, but it does not define the visible design.
- Appearance options remain: Dark, Light, System, six accents, and Translucency.
- Defaults: Dark + Indigo + Translucency on.
- Appearance preferences are local to Android; no cross-device/macOS synchronization.
- Scope is **all user-visible Android UI and every state that can be shown to a user**, not
  only the screens currently listed in S4-S6.
- Bottom navigation, history rows, devices, and other covered surfaces follow the approved
  design, not generic Material defaults.
- Use a new Lucide icon set on Android.
- A semantic `CpColors`/token layer is allowed and required.
- Component radii must match the exact style-guide roles.
- Interactive hit targets are at least 48 dp.
- Real blur is required where the design calls for translucency on API 31+, with a solid
  fallback on API 26-30 and when translucency/effects are disabled.
- Appearance changes preview live, persist only on Save, and unsaved exit discards them.
- Saved appearance must propagate throughout the app; implementation details must be made
  explicit in the repaired design.
- Theme/accent transitions and reduced-motion handling are in scope.
- Use one centralized preview gallery/fixture system, not duplicated preview boilerplate.
- Screenshot and golden testing are required.
- Golden target is a Pixel-class phone; exact deterministic device configuration still must
  be specified in the repaired plan.
- Default locale is English. Ukrainian is required now. Localization must be extensible.
- Branch from the current local `main` HEAD.
- The current task is specification repair/review, not implementation.
- `bd`, subagent workflow, slice commits, and gates are allowed. Do not push without explicit
  approval.

## 2. Executive finding

The current OpenSpec is formally valid (`openspec validate android-material3-redesign`
passes) but substantively invalid for the confirmed goal.

It describes a narrow branded-theme restoration with a few screen slices. The confirmed
goal is a complete Android UI redesign. The current proposal's central constraints —
"screen level stays de-styled", "not a pixel port", "no per-screen cosmetics", no dependency
changes, and only S4-S6 screen work — directly contradict the desired result.

The repair must be a scope and architecture rewrite, not wording cleanup.

## 3. Statements that must be removed or rewritten

### 3.1 `proposal.md`

Rewrite these ideas:

- "THEME level gets branding back; SCREEN level stays de-styled."
- Screens keep only bare shared components and generic `MaterialTheme.colorScheme` roles.
- "recover-and-modernise" as the overall project model. Old code can be reference material,
  but it cannot define the new full redesign.
- The narrow Impact file list.
- "No API/dependency changes."
- `@Preview` per surface as the main visual validation strategy.

Replace them with:

- full user-visible UI inventory and redesign scope;
- semantic design-system tokens/components as the visual contract;
- permission for internal Compose API refactors and required UI/test dependencies;
- centralized previews plus deterministic goldens;
- explicit exclusions only for genuinely invisible/background implementation surfaces.

### 3.2 `design.md`

Rewrite:

- Non-goal "not a pixel port" where it is used to justify generic M3 output. The correct
  rule is: reproduce the approved Android reference faithfully while adapting only for
  platform/system constraints.
- D7 `Shapes(small=8, medium=10, large=13)`; this cannot express the canonical radii.
- D9 "screen slices stay de-styled".
- D11 alpha-only translucency.
- The non-goal excluding app-wide live propagation if it contradicts the chosen Save
  behaviour.
- D12's six-slice workflow; it does not cover the real surface area.

### 3.3 `tasks.md`

The current S4-S6 are not complete slices. Do not append a generic "other screens" task.
Rebuild the slices from a UI inventory with acceptance evidence for every surface/state.

### 3.4 Delta spec

Six broad requirements are insufficient. Split capabilities by independently verifiable
behaviour. Recommended capability set:

- `android-design-system`
- `android-appearance`
- `android-navigation-chrome`
- `android-history`
- `android-preview`
- `android-devices-pairing`
- `android-settings`
- `android-onboarding-permissions`
- `android-feedback-states`
- `android-system-surfaces`
- `android-localization-accessibility`
- `android-visual-regression`

The exact names may differ, but one appearance spec must not carry the entire epic.

## 4. Full user-visible surface inventory

The repair agent must verify this list against the code and create a traceability table:

`surface/state -> Kotlin owner -> requirement -> task slice -> preview fixture -> golden ->
a11y/localization test`.

### 4.1 App shell and navigation

- `MainActivity.kt`
- floating bottom navigation: Clips / Devices / Settings
- selected/unselected/pressed/focused states
- content fade behind the floating bar
- system bars, navigation bar, gesture inset, cutout and IME interactions
- back navigation and restored selected tab
- sync status placement and overlays

The bottom bar must follow style guide section 9.12: floating frosted pill, 12 dp side
insets, 10-12 dp above bottom inset, full radius, border/shadow, 22 dp blur, and accent-selected
icon treatment. Do not replace it with a stock `NavigationBar` appearance.

### 4.2 History

Files include:

- `HistoryScreen.kt`, `HistoryList.kt`, `HistoryRow.kt`
- `HistoryChips.kt`, `HistoryDeviceFilter.kt`
- `HistoryNormalTopBar.kt`, `HistorySelectionBar.kt`
- `HistoryEmptyStates.kt`, `HistoryFilePicker.kt`

Required states/features:

- loading, populated, no history, private mode, no search results, error/degraded;
- pinned/date grouping and sticky headers;
- all content types: TEXT, URL, EMAIL, PHONE, CODE, JSON, NUMBER, COLOR, PATH, FILE,
  IMAGE, SECRET;
- correct semantic content color, glyph, type word, mono/sans selection;
- actual swatch for COLOR and thumbnail for IMAGE;
- metadata: type, source app, relative time, origin device;
- pin, delete, copy, share/open/save and unavailable-action states;
- selected row, multi-select, bulk actions, confirmation dialogs;
- device filter chips and overflow;
- swipe/long-press/tap interactions;
- pagination/loading-more if exposed;
- sensitive masked, reveal warning, revealed, and re-masked states;
- no secret in any semantics node, screenshot fixture, log, or test data.

### 4.3 Full-screen content preview

Currently absent from the plan:

- `PreviewOverlay.kt`
- `PreviewChrome.kt`
- `PreviewContent.kt`
- `PreviewActionRow.kt`
- `PreviewGesture.kt`

Cover text, URL/code/JSON, image loading/success/failure, file metadata/open/save failure,
masked sensitive content, action availability, gestures, close/back, and large content.
Masking guarantees must match History; do not assume a History-only test protects preview.

### 4.4 Devices

Files include `DevicesScreen.kt`, `PeerRow.kt`, `PairedPeerList.kt`,
`DevicesDialogs.kt`, and supporting animation/state files.

Cover:

- own-device card;
- paired peer collapsed and expanded;
- discovered peer;
- no peers, scanning, offline, reconnecting, error;
- online status with label, not color alone;
- P2P/Cloud/This phone/Verified badges;
- model, OS, app version, IP, last seen, sync state, RTT where available;
- fingerprint display, truncation, copy feedback;
- QR lifetime/progress/warning;
- unpair, revoke, revoke-all, key rotation and in-flight/failed/success states;
- all modal states in `DevicesDialogs.kt`;
- reduced-motion behaviour for status animation.

### 4.5 Pairing

Cover the complete flow, not only SAS styling:

- `PairActivity.kt`, `PairScreen.kt`, `PairQrCard.kt`, `QrHelper.kt`
- scanner launch and pre/post scanner Compose UI;
- QR display, scan, manual/deep-link input where supported;
- connecting/provisioning/bootstrap/sync progress;
- SAS confirmation dialog;
- invalid/expired QR, camera denied, network error, protocol error;
- cancellation and retry;
- `PairSuccessPopup.kt`;
- preservation of pairing, IPC, revoke and `peer_supabase_account_id` behaviour.

The ZXing `PortraitCaptureActivity` is a third-party/system-like surface. Specify what can be
themed and what can only be branded/configured/tested.

### 4.6 Settings

The current plan covers only Display. Scope includes:

- `SettingsActivity.kt`
- `GeneralTab.kt`
- `DisplayTab.kt`
- `SyncTab.kt`
- `NotificationsTab.kt`
- `StorageTab.kt`
- `SettingsComponents.kt`, sliders, fields, nav rows, validation and dialogs

Cover normal, focused, disabled, dirty, saved, validation error, destructive action,
loading and permission-dependent states. Preserve all existing settings and Save/Discard
semantics.

Appearance behaviour must be specified precisely:

- Dark/Light/System resolve to only two canonical token sets;
- six accents use their exact per-mode base and on-accent values;
- draft changes re-theme Settings immediately;
- Save uses the existing single synchronous `commit()` batch;
- Back/exit with dirty state offers discard handling and persists nothing on discard;
- after Save, app-wide propagation strategy is explicit and tested;
- process death and activity recreation preserve only committed values.

### 4.7 Onboarding and permissions

Currently absent:

- `OnboardingActivity.kt`, `OnboardingScreen.kt`, `OnboardingCards.kt`
- `OnboardingDialogs.kt` including crash-detected state
- `PermissionsSettingsActivity.kt`
- `BackgroundCaptureSetupActivity.kt`

Cover granted/denied/permanently denied/not-applicable states for notifications, camera,
overlay, battery optimization and OEM autostart guidance. System permission/settings pages
cannot be restyled; redesign the app-owned rationale/status/recovery screens and test intents.

### 4.8 About and diagnostics

- `AboutActivity.kt`
- `LogViewerActivity.kt`
- log levels, loading, empty, filtering, copy/export success/failure
- app version/build, links, licenses and feature information
- gradient/accent brand mark as specified

### 4.9 Feedback surfaces

- `ui/GlassToast.kt`
- `ui/SyncStatusBadge.kt`, including its detail sheet/tooltip
- success/info/warning/error banners
- confirmation and destructive dialogs
- progress indicators, retry affordances and disabled/in-flight states

Feedback must use semantic status tokens and redundant icon/text labels. Color alone is not
an acceptable state indicator.

### 4.10 Notifications and OS-owned surfaces

Include user-visible notification content from:

- `ServiceNotifications.kt`
- `NotificationHelper.kt`
- `LogcatCaptureService.kt`
- `ServiceRestartWorker.kt`

Android controls notification layout. Do not promise pixel parity. Specify app-controlled
properties: small icon, accent color where respected, localized titles/bodies/actions,
channel names/descriptions, priority/category, lock-screen visibility and PendingIntent paths.

Also document OS-owned surfaces:

- runtime permission dialogs;
- Android sharesheet;
- overlay/battery/app-notification settings;
- OEM autostart pages;
- ZXing scanner UI.

For these, acceptance is correct labels, icons, intents, pre/post screens and behaviour — not
Compose golden parity.

### 4.11 Share target and invisible windows

`ShareReceiverActivity` is currently intentionally UI-less and logs failures. Decide in the
repaired spec whether sharing must produce user feedback. If feedback is added, specify toast,
notification or result UI and its error/success states.

`ClipboardFloatingActivity` and the capture overlay are intentionally invisible functional
surfaces. Do not decorate them. Preserve invisibility, focus timing, window flags and privacy;
cover them with regression tests rather than visual goldens.

No Android AppWidget provider was found. Do not invent widget scope unless product explicitly
adds one.

### 4.12 Quick-paste

The style guide defines a mobile quick-paste bottom sheet summoned from a quick-settings tile
or share target. The current Kotlin inventory does not establish a complete equivalent. The
repaired proposal must classify it explicitly as either:

- an existing user-visible flow to redesign; or
- a new product capability, separated from presentation-only redesign work and given its own
  behavioural requirements.

Do not silently add new functionality under a visual slice.

## 5. Design-system architecture required

### 5.1 Material 3 role

Use Material 3 as the behavioural/accessibility substrate. The visible contract is the
CopyPaste semantic system. Standard M3 roles alone cannot represent status colors,
content-kind colors, exact radii, spacing, shadows, typography families, selected overlays
and motion.

Recommended locals/holders:

- `CpColors`
- `CpAccent`
- `CpSpacing`
- `CpRadii`
- `CpElevation`
- `CpTypography`
- `CpMotion`
- translucency/effects policy

Rule: raw hex, dp/sp design constants and arbitrary alphas live only in token/effect files.
Screens consume semantic components/tokens. Behaviour-specific dimensions such as safe insets
may remain local when they are not visual-design tokens.

### 5.2 Exact ColorScheme mapping

The current `surfaceVariant/surfaceContainer=elevated/raised` statement is ambiguous. Define
every role used by the app. Recommended mapping:

- `background = bg`
- `onBackground = text`
- `surface = panel`
- `onSurface = text`
- `surfaceContainerLowest = bg`
- `surfaceContainerLow = panel`
- `surfaceContainer = elevated/card`
- `surfaceContainerHigh = raised`
- `surfaceContainerHighest = raised2`
- `onSurfaceVariant = dim`
- `outline = border`
- `outlineVariant = divider`
- `primary = active accent`
- `onPrimary = active on-accent`
- `error = err`
- `scrim = scrim`
- `surfaceTint = Transparent` to prevent tonal elevation from changing canonical colors

Define secondary, tertiary, inverse, disabled and container roles, or prohibit their direct
use. Keep `selected`, hover, pressed and content/status colors in `CpColors`; do not misuse
`primaryContainer` as a universal selected color.

### 5.3 Canonical colors

Mirror every color in root `STYLEGUIDE.md`, including:

- all six surfaces/raised levels;
- border/divider;
- text/dim/faint/mute;
- hover/pressed/selected/scrim;
- ok/warn/err/info;
- every content-kind color;
- accent base per light/dark, one canonical accent-2, and theme-sensitive on-accent.

The current design invents `accent2Dark/accent2Light`; the style guide specifies one
accent-2 per accent. Do not invent variants without changing the source of truth.

### 5.4 Radii and shapes

Exact roles:

- chip: 7 dp
- pill: fully rounded
- control/button: 8 dp
- input/search: 9 dp
- card/banner/modal: 13 dp

`MaterialTheme.shapes` can provide broad defaults but cannot replace `CpRadii`. The HTML's
occasional 14 px card or 92% nav alpha does not override the style guide's 13 px/90% values.

### 5.5 Spacing

Use the canonical scale: `2, 4, 6, 8, 11, 14, 16, 20, 24` dp. The current prohibition on
fixed dp is wrong: canonical spacing is necessarily expressed in dp. The actual rule should
prohibit un-tokenized, arbitrary screen-local visual dimensions.

### 5.6 Typography

Bundle and use Inter for UI and JetBrains Mono for machine-shaped content. Define semantic
styles rather than vaguely mapping an 11-value scale to generic M3 roles:

- title: 21-24 sp, 700, 1.2 line-height ratio;
- section: 13-15 sp, 600, 1.3;
- body/row: 13-14 sp, 400-500, 1.45;
- meta: 11-12 sp, 400, 1.4;
- micro/eyebrow: 9.5-10.5 sp, 500-600, 1.0, uppercase with 0.06-0.1 em tracking;
- machine content, fingerprints, code, IP, timestamps, hex and counts use Mono;
- numeric content must use stable/tabular figures where Android/font support allows.

The project already contains Inter and JetBrains Mono resources, but the repair must verify
weights and fallback behaviour.

### 5.7 Icons

Add and standardize a Lucide Compose source/dependency. The repaired proposal must permit the
dependency/API changes needed for this and record version/license obligations.

Specify:

- one canonical icon provider;
- 24x24 line geometry, rounded caps/joins and consistent optical sizing;
- rendered sizes by role;
- fixed boxes so icons cannot affect layout unexpectedly;
- fallback policy for a missing Lucide glyph;
- removal/migration policy for Material icons, emoji, text glyphs, custom inline vectors and
  the old `NavIcons.kt` set;
- content descriptions only on actionable/informative icons; decorative icons are hidden from
  semantics.

### 5.8 Motion

Tokenize:

- fast: 120 ms
- normal: 200 ms
- theme/accent: 300 ms
- easing: cubic-bezier(.2,.8,.2,1)

Specify which transitions apply to theme, screen state, modal/sheet, list insertion/removal,
selection, press and status presence. Motion must be quiet and functional.

Reduced motion uses the system animator duration scale/Compose motion-duration signal: all
token durations become zero and transforms/presence glow are disabled. Do not add a separate
user preference unless product requests one.

### 5.9 Translucency and blur

Required policy:

- API 31+: real blur/effect for chrome/sheets where the design calls for it;
- API 26-30: opaque canonical fallback;
- Translucency off: opaque canonical fallback;
- reduced-effects policy: opaque fallback;
- never use reduced alpha without blur over arbitrary content because it damages contrast;
- never block first paint on effect detection;
- inject/override the effect policy in previews/goldens for determinism.

Android does not expose a universal reduced-transparency accessibility setting. The repaired
design must name an actually detectable signal or avoid claiming one. Battery saver is not an
accessibility transparency preference and must not be treated as equivalent without a product
decision.

## 6. Security and behavioural invariants

The current security requirement is too narrow.

### 6.1 Window privacy

Inventory every Activity/window. `SecureWindowChrome` only protects activities that compose
through it. Explicitly classify:

- Compose activities using `SecureWindowChrome`;
- ZXing `PortraitCaptureActivity`;
- translucent `ShareReceiverActivity`;
- invisible `ClipboardFloatingActivity`;
- dialogs, sheets and previews.

Test `FLAG_SECURE`/recents privacy for every window that can show clipboard or pairing data.
Do not claim "every themed window" without proving the wrapper coverage.

### 6.2 Sensitive content

Test masking independently in:

- History row;
- full-screen preview;
- search/filter results;
- selection/bulk UI;
- toast/banner/error text;
- accessibility semantics tree;
- screenshots/golden fixtures;
- logs and notifications.

Fixtures must use obvious synthetic placeholders and tests must assert the secret does not
appear anywhere in merged or unmerged semantics.

### 6.3 Behaviour preservation

Visual work must not alter:

- clipboard capture/copy/paste/delete/pin behaviour;
- reveal guard;
- pairing protocol, IPC, account identity and revoke behaviour;
- share URI grants;
- foreground service lifecycle;
- settings persistence/migration;
- overlay focus timing;
- deep-link parsing;
- notification action intents.

Allow internal Compose component API refactors, but forbid behavioural/IPC/FFI contract drift.
The current blanket "public signatures unchanged" blocks legitimate design-system work and
should be narrowed.

## 7. Accessibility requirements

The repaired spec must go beyond icon content descriptions:

- 48 dp minimum interactive hit target without inflating the visible 22-38 dp control;
- semantic roles, checked/selected/expanded/disabled/error state descriptions;
- meaningful TalkBack traversal and grouping;
- dialogs/sheets receive and restore focus correctly;
- no duplicate label from merged descendants;
- color is never the only signal;
- AA: 4.5:1 body text, 3:1 large text/UI affordances;
- contrast computed after alpha compositing on the actual surface;
- all six accents in dark and light, including on-accent;
- keyboard/D-pad/switch access where Android exposes it;
- font scales 1.0, 1.3 and 2.0 without clipping or lost actions;
- narrow width and long Ukrainian strings;
- reduced motion;
- masking secrecy in merged and unmerged semantics.

Automate what is deterministic with Compose UI tests and token-level contrast tests. Keep a
manual TalkBack checklist for behaviour automation cannot reliably prove.

## 8. Localization requirements

Current state has only `res/values` and `values-night`; there is no `values-uk`. Many English
strings are hardcoded in Compose/services/notifications.

Required scope:

- move **all** user-visible strings and accessibility labels to resources, not only new
  appearance labels;
- `values/strings.xml` is default English;
- add complete `values-uk/strings.xml`;
- use formatted strings and plurals correctly;
- do not concatenate translatable sentence fragments;
- localize notification channels, notifications, dialogs, errors and content descriptions;
- establish a missing-translation/hardcoded-text lint gate;
- test EN and UK layouts, including long strings and 200% font scale;
- define how future locale directories can be added without changing component APIs.

Dates, relative times, numbers and file sizes must use locale-aware formatting unless a
machine protocol format is intentionally shown.

## 9. Preview, screenshot and golden strategy

There are currently no real `@Preview` declarations and no UI screenshot/golden framework.
The `golden_vectors.json` files are crypto fixtures, not UI goldens.

### 9.1 Central preview catalog

Create one catalog/fixture system using `PreviewParameterProvider` or an equivalent shared
model. Avoid four near-identical annotations on every composable.

Fixtures must cover representative:

- dark/light;
- at least two accents visually, while token tests cover all six;
- English/Ukrainian;
- normal/loading/empty/error/disabled/selected;
- masked sensitive data;
- large font/long text;
- translucent and solid policy where deterministic.

### 9.2 Golden framework

The repair must choose and justify a framework compatible with this AGP/Kotlin/Compose stack
(for example Paparazzi, Roborazzi, or an approved Compose screenshot solution). This requires
dependency/plugin changes, so remove the "no dependency changes" constraint.

Define:

- exact Pixel device/model or explicit width/height/density;
- API level, orientation, navigation mode and font scale;
- deterministic locale, timezone, clock, animation clock, fonts and fake data;
- baseline directory and naming convention;
- record/update command;
- verify command;
- pixel/diff threshold;
- CI artifacts on failure;
- baseline-review policy (never auto-accept changed images);
- deterministic blur policy;
- owner approval for intentional baseline updates.

Recommended practical matrix:

- one canonical Pixel portrait baseline for full screen goldens;
- dark/light x representative accents for screens;
- component/token contract tests for all six accents;
- EN/UK for text-heavy screens;
- 1.0 and 2.0 font scale stress cases;
- compact-width stress case;
- masked-sensitive fixture;
- translucency on/off only where rendering is stable.

Do not generate the full Cartesian product of 3 themes x 6 accents x 2 translucency modes x
2 locales x all states as screenshots. That produces expensive, low-signal baselines. Cover
the full color matrix with deterministic token/contrast tests and use representative visual
goldens.

## 10. Verification gates

`scripts/android-verify.sh` currently performs:

1. UniFFI regeneration
2. native `.so` build
3. debug APK assembly
4. JVM unit tests

It explicitly does **not** run connected/instrumented tests or prove real-device readiness.
Therefore "android-verify GREEN" cannot be the only slice gate.

Define separate gates:

- OpenSpec validation;
- formatting/lint and warning policy;
- JVM unit tests;
- token mapping/contrast tests;
- screenshot golden verification;
- Compose UI semantics/a11y tests;
- connected emulator tests where required;
- `android-verify.sh` native/build regression;
- manual Pixel smoke/TalkBack checklist at milestones;
- final post-command `git status --short` and generated-binding diff check.

Add tests for:

- exact dark/light token values and M3 role mapping;
- all six accent hues/on-accent contrast;
- System theme reaction;
- migration exactly once and idempotence;
- committed values survive process death;
- live preview, Save and Discard;
- effect fallback by API/policy;
- no sensitive plaintext in semantics;
- 48 dp targets/roles where testable;
- localization completeness and hardcoded user text;
- Lucide icon/license coverage.

"Warning-clean" must have an actual compiler/lint command or warnings-as-errors configuration.

## 11. Git and dirty-tree constraint

Observed state during review:

- current branch: local `main`, 21 commits ahead of `origin/main`;
- user-owned deletion: `docs/design/copypaste-app-demo.html`;
- `openspec/` is untracked;
- `android-verify.sh` refuses any dirty tree by default.

The repaired setup must preserve user changes and explicitly define:

- branch `android-redesign` from the current local `main` HEAD;
- how the existing deletion is preserved without accidental commit/revert;
- an initial intentional commit containing the repaired OpenSpec if approved;
- never use destructive reset/checkout;
- one logical commit per green slice;
- no `ANDROID_VERIFY_ALLOW_DIRTY=1` in automated workflow;
- inspect status/diff after code generation and verification;
- no push without explicit approval.

The present instruction "commit, then verify" is necessary for the clean-tree preflight, but
the workflow must define what happens if verification generates diffs or fails after commit.
Do not close a slice merely because a commit exists.

## 12. `bd` and implementation workflow

Create one epic and child issues based on repaired slices. Each issue needs:

- Ukrainian title;
- English description and acceptance criteria;
- explicit parent/dependency links;
- file/surface scope;
- invariants and do-not-touch boundaries;
- required fixtures/goldens/tests;
- exact gate commands;
- evidence required before close;
- failure/reopen procedure.

Recommended sequential flow for each slice:

1. Orchestrator confirms scope and dependencies.
2. Builder implements only the slice.
3. Independent reviewer audits diff, design fidelity, security, a11y and localization.
4. One verification runner executes builds serially; never run concurrent Android native/
   Gradle builds due to the known OOM constraint.
5. Fix failures and repeat review/gates.
6. Commit the verified logical unit.
7. Add `bd` note with commands/results/golden artifacts.
8. Close only when acceptance evidence is complete.

Subagent use must not replace main-agent review. Handoffs should include exact task boundaries,
the canonical style-guide sections, prohibited behavioural changes and acceptance commands.

## 13. Recommended slice structure

The repair agent may adjust boundaries, but the plan needs coverage equivalent to:

- S0: scope lock, full UI inventory, branch/`bd`, fixture and traceability matrix
- S1: semantic design-system foundation (colors, spacing, radii, type, motion, effects)
- S2: Lucide icon system and shared components
- S3: appearance persistence, live preview, app-wide propagation and migration tests
- S4: app shell, system bars and floating navigation
- S5: History list, filters, selection and all states
- S6: full-screen preview and content actions
- S7: Devices and every dialog/state
- S8: complete pairing/scanner/deep-link flow
- S9: all Settings tabs, validation, Save/Discard and destructive flows
- S10: onboarding, permissions and background-capture setup
- S11: feedback, sync sheet, About, logs and diagnostics
- S12: notifications, share target and system-owned surface integration
- S13: English/Ukrainian localization sweep and hardcoded-string gate
- S14: centralized preview catalog and screenshot/golden infrastructure
- S15: a11y/security regression suite, real Pixel acceptance and close-out

Foundation/test infrastructure can be introduced earlier where it is required to gate later
slices. Every user-visible inventory row must have exactly one owning slice.

## 14. Open questions the repair agent must not silently guess

These require product confirmation if they materially affect behaviour:

1. Exact golden device: Pixel model/viewport, API level, orientation, density, navigation
   mode and font-scale matrix.
2. After Save, should existing activities be explicitly recreated immediately, or is applying
   the committed theme when each activity next resumes/recomposes acceptable?
3. What actual Android signal should disable blur besides the user Translucency preference?
   Battery saver and reduced motion are different concerns.
4. Is quick-paste bottom sheet/Quick Settings tile existing functionality to redesign, or new
   functionality requiring a separate product capability?
5. Should the currently invisible share receiver surface success/failure feedback? If yes,
   via toast, notification or result screen?
6. Must landscape be supported/covered, or is portrait-only the acceptance target except the
   portrait-locked scanner?
7. Is tablet/foldable responsive behaviour explicitly out of scope for this epic?
8. Which Lucide Compose distribution/version is approved if more than one implementation is
   compatible with the project?

Until answered, the repaired spec should mark these as explicit decisions/TODOs, not bury
assumptions in implementation tasks.

## 15. Definition of ready for implementation

The change is ready only when all of the following are true:

- proposal describes full Android UI redesign, not theme restoration;
- every user-visible surface/state has an owner in the traceability matrix;
- system-owned and intentionally invisible surfaces are classified correctly;
- semantic tokens exactly reflect `STYLEGUIDE.md`;
- M3 mapping and non-M3 semantic locals are complete;
- exact radii/type/spacing/motion/translucency contracts are written;
- Lucide dependency and migration policy are approved;
- EN/UK localization requirements and gates exist;
- screenshot/golden framework, Pixel configuration and baseline policy are defined;
- accessibility and privacy requirements are independently testable;
- behaviour/IPC/security invariants are explicit;
- tasks have serial dependencies and real gate commands;
- dirty-tree/branch/commit workflow is safe;
- all delta specs contain WHEN/THEN scenarios for success, error and boundary states;
- `openspec validate android-material3-redesign` passes after the rewrite;
- no unresolved contradiction remains between proposal, design, tasks, specs, style guide and
  confirmed product decisions.
