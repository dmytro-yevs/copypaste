# Staff engineering review: design-system-redesign

## Review status

**Recommendation: needs revision before implementation.**

The overall direction is sound: token-driven CSS, shared visual primitives, reuse between the main
window and popup, and a mock-mode component gallery fit the existing stack. The proposal is not yet
implementation-ready because several requirements contradict the proposed implementation, some
acceptance criteria are not testable, and the change contains behavioral and persistence work that
is described as purely presentational.

This review covers:

- `proposal.md`
- `design.md`
- `tasks.md`
- `specs/design-tokens/spec.md`
- `specs/component-library/spec.md`
- `specs/preview-gallery/spec.md`
- relevant current code in `crates/copypaste-ui`

## Blocking findings

### B1. Theme application does not meet the pre-paint requirement

`design.md` Decision 3 and tasks 1.7/1.8 apply `data-theme` and `data-accent` in a React
`useEffect`. `design-tokens/spec.md` requires persisted preferences to be applied before the first
meaningful paint. React effects run after the browser has painted, so a user with a persisted light
theme can see the static dark theme first.

Required revision:

- Add a synchronous theme bootstrap before React renders, shared in behavior by `index.html` and
  `popup.html`.
- Read and validate persisted `theme`, `accent`, and translucency values there.
- Keep the React effect for live updates after mount.
- Specify fallback behavior for missing, malformed, and unsupported values.
- Verify compatibility with the Tauri CSP before choosing an inline script.

Acceptance criteria should explicitly test that a persisted non-default theme is present on
`document.documentElement` before app content becomes visible.

### B2. The proposed gallery gate does not guarantee production bundle exclusion

Adding `"gallery"` to the global `ViewId` and conditionally hiding its sidebar item only controls
reachability. It does not remove a statically imported `GalleryView` or its fixtures from the
production module graph. The current `App.tsx` uses a statically constructed `Record<ViewId, ...>`,
which encourages a static gallery import.

Required revision:

- Load the gallery through a DEV-gated dynamic import, following the existing `mockIpc.ts` pattern.
- Avoid requiring a gallery entry in the production `Record<ViewId, ...>`.
- Consider separate `ProductionViewId` and `DevViewId` types.
- Define recovery behavior if stale state contains `"gallery"` in a production build.
- Keep the bundle-content assertion from task 8.7, but also inspect emitted chunks/module graphs;
  searching for one unique string is only a heuristic.

### B3. The claimed presentational scope conflicts with required behavior

`design.md` lists new component behavior as a non-goal, but the specs require behavioral contracts:

- modal focus trapping;
- Escape dismissal;
- backdrop-click dismissal;
- focus restoration after dismissal, implicitly required for accessible modal behavior;
- sensitive-content reveal and optional warning flow;
- expandable device rows;
- confirmation before destructive actions.

Required revision:

- Either include interaction behavior in scope and test it, or document, component by component,
  which existing implementation already provides each behavior.
- Do not describe the whole change as purely presentational if behavior must be added or repaired.
- Add explicit requirements for focus restoration and initial modal focus.

### B4. Translucency is marked resolved but is missing from the capability specs

`design.md` says the translucency toggle will ship and default to on. The proposal, token spec, and
task list do not fully define it. Task 5.3 still points to an already resolved open question.

Required revision:

- Add translucency to the proposal's affected capabilities and persistence impact.
- Define its `UIPrefs` type, default, migration, validation, and DOM representation.
- Enumerate exactly which surfaces are translucent and which remain opaque.
- Define the off state and platform fallback when backdrop filtering is unavailable or disabled.
- Define behavior for `prefers-reduced-transparency` if supported by the target WebView/platform.
- Add gallery examples and visual acceptance criteria for both values.

## Architecture and scalability findings

### A1. A single authored CSS file will become a shared hotspot

One runtime stylesheet is reasonable. Requiring every authored rule for tokens, reset, primitives,
patterns, app shell, popup, and gallery to live in one physical file creates merge conflicts, weak
ownership boundaries, and difficult dead-code analysis. Banner comments do not enforce cascade
order or specificity.

Recommended revision:

- At minimum use native cascade layers:
  `@layer reset, tokens, base, primitives, patterns, shell, utilities`.
- Prefer multiple maintainable source files compiled/imported into one runtime stylesheet if the
  actual requirement is one network/build asset.
- Add a documented specificity policy: low-specificity class selectors, state via attributes or
  modifiers, and no view-specific selector escalation.
- State whether gallery-only CSS may be excluded from production.

### A2. The `occurs >=2 times` DRY rule promotes premature abstraction

Visual similarity at two call sites is not sufficient evidence that two elements share a component
contract. They may differ in semantics, interaction, accessibility, lifecycle, or future change
direction. Conversely, one class can be insufficient reuse for behavior-heavy components.

Replace the rule with semantic reuse criteria:

> Reuse a component when anatomy, semantics, interaction contract, accessibility contract, and
> supported variants align. Reuse tokens or low-level primitives when only presentation aligns.

Require React primitives for behavior-heavy patterns such as modal/dialog, toggle, segmented
control, banner actions, and expandable disclosure rows. A CSS class alone cannot guarantee their
behavior or accessibility.

### A3. HistoryRow and PopupRow can drift despite sharing `.row`

The two rows need different layouts, but their content-kind interpretation should not be duplicated.
Sharing only CSS leaves duplicate mappings for icons, token selection, secret masking, source-app
fallback, metadata labels, thumbnails, and accessible names.

Recommended shared units:

- `normalizeContentKind()`;
- a typed kind-to-token/icon/label mapping;
- `ContentTile`;
- `ClipPreview` including sensitive-state presentation;
- `ClipMetadata` or shared metadata formatting functions;
- source-application fallback presentation.

`HistoryRow` and `PopupRow` should remain separate layout wrappers composed from these shared units.

### A4. Clipboard kinds are not a closed backend type

The proposal treats the eleven gallery kinds as a closed union, while the current
`HistoryEntry.kind` is an optional string. A newer daemon, malformed fixture, or legacy item can
produce unknown or absent values.

The spec must define:

- normalization of case and aliases;
- precedence between `kind` and `content_type`;
- mappings for `PATH/FILE` and `PHONE/NUMBER`;
- fallback icon, token, and label for unknown values;
- behavior for an image MIME type with absent or contradictory `kind`;
- tests for unknown, undefined, and future values.

### A5. Theme state synchronization between windows is underspecified

The main window and popup initialize independent React/Zustand instances. The proposal covers
restart persistence but does not state what happens when Settings changes the theme while the popup
is already open.

Define whether live cross-window synchronization is required. If yes, specify the mechanism:
storage event, Tauri event, shared persisted store notification, or theme refresh each time the popup
opens. Add a two-window acceptance scenario.

### A6. Gallery state isolation needs a precise DOM strategy

Applying `data-theme`/`data-accent` to a gallery wrapper is compatible only if all token selectors
support scoped attributes, not exclusively `:root[data-theme=...]`. Applying them to `<html>` would
temporarily override the real application theme and require reliable cleanup.

Specify one strategy:

- token selectors supported on `.theme-scope[data-theme][data-accent]`; or
- temporary root mutation with guaranteed restoration on unmount/error.

The scoped-wrapper approach is safer and supports multiple combinations rendered simultaneously.

### A7. The popup should not necessarily pay for every main-window style

One shared stylesheet means the popup loads selectors for Settings, Devices, About, Logs, and the
gallery. This may be acceptable at the current size but should be measured because popup latency is
called out as important.

Add bundle/CSS-size and popup-open performance budgets, or explicitly accept the cost with measured
baseline data.

## Persistence and migration findings

### P1. TypeScript casts do not validate stored preferences

The existing store merges parsed JSON into `UIPrefs` through a cast. After adding enums, values such
as `theme: "system"`, `accent: "purple"`, wrong primitive types, or corrupt partial state can reach
the DOM.

Required tests and behavior:

- validate every enum at runtime;
- retain valid existing fields when one field is invalid;
- default invalid theme/accent/translucency independently;
- test malformed JSON;
- test unknown keys;
- test v1/v2/v3/v4 to v5 paths;
- test normal v5 reload.

### P2. The rollback claim is inaccurate

`design.md` says rollback is a plain git revert with no migration complexity. Once new builds write
only the v5 key, an older build reading v4 will not see preferences changed under v5. A rollback can
therefore restore stale values or defaults.

Required revision:

- Document the downgrade behavior explicitly.
- Decide whether the migration retains v4 for one release, dual-writes temporarily, or accepts
  preference loss on downgrade.
- Do not claim zero rollback complexity unless verified.

### P3. Migration terminology is ambiguous

Changing the storage key is not an additive schema change in operational terms; it creates a new
source of truth. The design should distinguish:

- additive fields within the preferences object;
- persisted-key version migration;
- cleanup timing for old keys;
- downgrade behavior.

## Component model findings

### C1. Variants and state naming are inconsistent

The proposal mixes BEM modifiers (`.btn--primary`), generic classes (`.sm`, `.off`, `.on`,
`.danger`), and state classes (`.sel`, `.removing`, `.copied`). Generic selectors can leak or become
ambiguous in a global stylesheet.

Choose a consistent contract, for example:

- component modifier classes: `.btn--small`, `.btn--danger`;
- native states where possible: `:disabled`, `[aria-selected=true]`, `[aria-expanded=true]`;
- explicit data state: `[data-state=removing]`, `[data-kind=secret]`.

Document which state is authoritative: React prop, ARIA state, or CSS-only modifier.

### C2. Modal visual reuse is not enough

`ConfirmModal`, `SasPairingModal`, `RevokeConfirmDialog`, and `DetailsModal` should share a dialog
behavior primitive, not merely `.scrim`/`.modal` classes. Otherwise focus and dismissal behavior can
diverge.

Specify a reusable `Dialog`/`Modal` primitive responsible for:

- portal/container strategy;
- `role="dialog"` and `aria-modal`;
- labelled-by/described-by wiring;
- initial focus;
- focus trap;
- Escape and backdrop policy;
- restoring trigger focus;
- scroll locking if relevant.

### C3. Button enforcement should distinguish semantic exceptions

The spec says any raw button must use the shared `.btn` family, but tabs, icon buttons, disclosure
headers, chips, and row actions are also buttons with intentionally different anatomy. Define the
allowed primitives rather than forcing all buttons into one family.

### C4. Device actions are ambiguous

The spec requires Unpair and Revoke as equal-width danger actions but does not clearly explain the
semantic difference, availability rules, pending state, error state, or whether both actions are
always valid. Visual redesign should not accidentally expose an invalid destructive operation.

Add behavior/state tables for own device, paired peer, discovered device, offline peer, pending
action, and failed action.

### C5. Source-app icon reservation lacks a data contract

The resolved decision reserves source-app icon space, while daemon changes are explicitly out of
scope. Specify which existing field is used today, when the generic fallback appears, accessible
label behavior, and whether every row pays the layout cost even when the feature is unavailable.

## Accessibility findings

### X1. Copying reference tokens does not prove WCAG AA

The requirement claims AA contrast from reference values but provides no executable validation.
All 12 theme/accent combinations must be checked for:

- normal text contrast;
- large text contrast;
- non-text UI boundaries and controls;
- focus indicators;
- text and icons on accent surfaces;
- status surfaces;
- content-type metadata.

Add a token contrast test or script. Manual visual inspection is not sufficient.

### X2. Existing attributes on the same DOM element are the wrong invariant

Preserving accessible behavior is important, but requiring every `role`, `id`, and `aria-*`
attribute to remain on exactly the same element can block a necessary semantic correction.

Use observable contracts instead:

- accessible role and name remain correct;
- labelled/described relationships resolve;
- state is exposed correctly;
- keyboard behavior is unchanged or improved;
- stable test IDs remain only where they are an intentional test contract.

### X3. Focus-visible requirements are incomplete

A universal `2px` ring does not establish that the ring has sufficient contrast against every
adjacent surface, is not clipped by overflow, and remains visible in forced-colors mode.

Add checks for:

- 3:1 focus indicator contrast;
- no clipping in rows, modals, tabs, and popup;
- forced-colors fallback;
- logical focus order;
- focus restoration after modal close.

### X4. Hover-revealed actions need separate pointer and keyboard contracts

The spec does not resolve whether invisible action buttons remain in the tab order, how a keyboard
user discovers them, or how touch users invoke them.

Define behavior for:

- fine pointer hover;
- `:focus-within` keyboard visibility;
- coarse pointer/touch, where hover does not exist;
- selection mode;
- screen-reader navigation.

Avoid controls that are focusable while visually hidden.

### X5. Additional accessibility coverage is missing

Add acceptance criteria for:

- 200% zoom and text scaling;
- reflow without two-dimensional scrolling where applicable;
- minimum target sizes or documented desktop exception;
- `aria-expanded`/`aria-controls` for device disclosures;
- tab semantics and arrow-key behavior for segmented controls/tabs;
- toast/status live regions;
- reduced motion for every animation, including any CSS transitions not using duration tokens;
- sensitive-content announcements without exposing the secret in accessible names while masked.

### X6. Sensitive blur can leak information

The requirement intentionally preserves the real text width while blurred. This leaks approximate
length and visual shape and may still expose content through selection, copy, accessibility tree,
DOM inspection, screenshots, or insufficient blur.

The product/security decision must be explicit. Define:

- whether masked content remains in the accessibility tree;
- whether it can be selected/copied before reveal;
- behavior in popup and main history;
- reveal timeout/re-mask policy;
- warning and audit behavior;
- whether length leakage is accepted.

## CSS and token findings

### S1. The no-pixel rule conflicts with legitimate runtime geometry

The proposal bans hardcoded pixel values outside tokens, while current virtualized layouts require
per-instance pixel positions and sizes. The focus requirement also literally specifies `2px`.

Split the policy into:

- design constants must use tokens;
- runtime-computed geometry may use inline styles or CSS custom properties;
- structural values such as `0`, percentages, fractions, and transforms are allowed;
- focus width/offset, hairlines, icon sizes, and control heights receive explicit tokens.

### S2. `color-mix()` compatibility is assumed

The specs depend on `color-mix(in srgb, ...)`. Confirm the minimum WebView versions supported by
Tauri targets and define a fallback if unsupported. The reference rendering in a current browser is
not enough to prove runtime compatibility on all supported systems.

### S3. Reduced motion via zero-duration tokens may not cover every animation

Animations can remain through hardcoded keyframe durations, browser-native smooth scrolling,
transforms, or transitions that do not use the three duration tokens.

Add a reduced-motion audit that disables `animation-duration`, `animation-iteration-count`,
`transition-duration`, and smooth scrolling at the appropriate layer, while preserving necessary
progress feedback without motion.

### S4. Token source-of-truth remains duplicated

Copying the reference HTML token block byte-for-byte initially prevents transcription errors but
creates two maintained copies afterward. A variable-name diff does not catch value drift.

Choose one:

- generate both reference and application tokens from one source;
- import the same token stylesheet into the reference;
- add an exact name-and-value parity test.

Do not claim the files “can never silently drift” if only names are compared.

### S5. Typography and responsive/layout tokens are underspecified

The proposal names typography but does not clearly specify font stacks, weights, line heights,
letter spacing, text scaling, minimum window sizes, popup width constraints, or long localized text.
Spacing tokens alone do not provide a responsive layout system.

Add typography tokens and layout constraints, then test long strings and at least one expansion
case representative of localization.

## Gallery and verification findings

### G1. “Every component in every state” is not currently testable

Native pseudo-states such as `:hover`, `:active`, and `:focus-visible` cannot all be persistently
rendered as ordinary static examples.

Choose an explicit mechanism:

- debug-only forced-state attributes/classes with CSS parity tests; or
- Playwright interaction screenshots using `hover()`, mouse down, and keyboard focus.

“Hover-capable” is not an acceptance criterion because it does not state the expected visual result.

### G2. Gallery breadth can cause an unmaintainable matrix

Every primitive and pattern multiplied by every state, long text, empty variants, two themes, and six
accents can become a very large DOM and slow visual surface.

Structure the gallery as:

- canonical component/state sections;
- local theme/accent switcher for the full set;
- a compact token/critical-component matrix for all 12 combinations;
- deterministic deep links or section IDs for automation.

Do not render twelve complete interactive applications simultaneously unless performance is measured
and accepted.

### G3. Gallery fixtures may duplicate mock IPC fixtures

The proposal allows gallery-specific fixtures while also reusing mock data. Duplicated fixtures will
drift from real IPC shapes.

Create typed fixture factories shared by mock IPC and gallery, with per-story overrides. Ensure
production exclusion applies to the factories and any secret-like sample values.

### G4. Manual verification is not sufficient for this blast radius

The repository already has Playwright visual infrastructure. A redesign touching every surface
should not leave automated visual regression as a stretch goal.

Minimum automated suite:

- main surfaces in dark and light;
- popup in dark and light;
- critical accent/on-accent matrix;
- modal keyboard/focus behavior;
- reduced-motion mode;
- long-text overflow;
- production gallery exclusion;
- automated contrast checks;
- accessibility scan if an approved tool is available.

Manual verification should remain exploratory, not the only gate.

### G5. Verification terminology is inconsistent

The proposal says every theme/accent/state combination must be rendered and verified, while tasks
10.6–10.8 only spot-check portions of the matrix. Align the requirement and tasks: either the full
matrix is required and automated, or explicitly identify the representative combinations and why
they cover the risk.

## Scope and delivery findings

### D1. The change is too broad for one implementation unit

The proposal combines tokens, persistence migration, every application view, popup, accessibility
behavior, component extraction, icon restoration, settings behavior, gallery, and verification.
That creates a large review surface and makes regressions difficult to isolate.

Recommended delivery slices:

1. Tokens, cascade layers, pre-paint bootstrap, prefs migration and validation.
2. Typed primitives and dialog/disclosure accessibility foundations.
3. History and popup using shared clipboard presentation units.
4. Devices.
5. Settings, sidebar, About, Logs, banners, and toast.
6. Gallery and automated visual/accessibility coverage.

These may remain one OpenSpec change, but each slice should build, test, and be independently
reviewable.

### D2. “Every file under components/views/popup” is not a useful impact boundary

The affected-code declaration is too broad to help reviewers understand risk. Add an inventory that
maps each existing component to one of:

- unchanged behavior, class-only styling;
- composed from a new shared primitive;
- behavior changed;
- deprecated/removed;
- gallery-only.

### D3. No rollout or fallback strategy is defined

A full UI replacement can produce unusable screens even when compilation succeeds. A git revert is
not a runtime fallback.

Consider:

- incremental landing with every commit keeping the app usable;
- screenshot evidence per delivery slice;
- explicit minimum-window and popup smoke checks;
- preference migration monitoring/logging;
- a release rollback note covering v5 preferences.

## Factual and consistency issues

### F1. The “zero inline style” premise is not literally true

Current source contains legitimate inline styles in virtualization, glide-highlight geometry,
popup-row dynamic values, tab indicator geometry, and other components. The proposal should say that
design styling was stripped, not that all inline styles are absent.

This distinction matters because some inline styles are required for runtime-computed geometry and
should not be removed to satisfy an over-broad audit.

### F2. Icon approach differs between proposal and design

`proposal.md` describes inline SVG/lucide-style icons, while `design.md` prefers `lucide-react` and
allows inline SVG only as fallback. Use one normative rule across documents.

### F3. The gallery URL conditions are inconsistent

The preview spec says the gallery is available with `?mock=1`, but its absence scenario mentions
both `?mock=1` and `?bridge=1`. The design specifically gates on `MOCK`, which is false in bridge
mode. Decide whether bridge mode should expose the gallery and make all documents consistent.

### F4. Empty-state counts are inconsistent

Tasks describe “3 empty states” and then list offline, starting up, no matches, and nothing copied
yet, which is four states. Fix the count and define whether startup/error states use the same public
component API.

### F5. Resolved questions remain referenced as open

Tasks still direct implementers to Open Questions 1 and 4 even though `design.md` contains resolved
decisions. Move resolved decisions into the normative Decisions section, remove stale open-question
references, and keep only genuinely unresolved questions under Open Questions.

### F6. `devcard` is specified but intentionally unused in production

An unused production primitive increases CSS and maintenance surface. If it is gallery-only future
design documentation, either exclude it from production CSS or defer it until a real consumer
exists. Avoid implementing speculative component APIs under a DRY mandate.

## Missing non-functional requirements

The spec should add measurable requirements for:

- maximum CSS and JS bundle delta;
- popup open/render latency;
- gallery production exclusion;
- supported OS/WebView/browser matrix;
- minimum main-window and popup dimensions;
- zoom/text scaling behavior;
- localization/long-string behavior;
- visual regression baselines and update process;
- ownership/source-of-truth for tokens and component contracts.

## Strong parts worth preserving

- The active and superseded design sources are identified clearly.
- Reusing the installed `lucide-react` dependency is appropriate.
- Tokens, component library, and gallery are separated into capabilities.
- Popup parity, reduced motion, persistence, and production fixture exclusion are recognized.
- The device row versus card decision is reasoned from the current product structure.
- Gallery-local preferences are the correct product behavior.
- The proposal recognizes that backend content-kind coverage and design coverage are different.

## Required changes before approval

1. Replace effect-only theme initialization with a pre-paint bootstrap plus live synchronization.
2. Define a DEV-only dynamic import architecture for the gallery.
3. Reconcile presentational non-goals with required interaction behavior.
4. Fully specify translucency across proposal, design, specs, tasks, persistence, and gallery.
5. Add runtime validation and downgrade semantics for preferences.
6. Define shared clipboard-kind presentation logic for History and Popup.
7. Define unknown-kind fallback behavior.
8. Replace mechanical DRY with semantic component reuse rules.
9. Introduce enforceable cascade layers and a specificity policy.
10. Expand accessibility requirements beyond preserving existing attributes.
11. Make visual, contrast, keyboard, reduced-motion, and bundle checks automated gates.
12. Reconcile all stale counts, URL conditions, icon rules, open questions, and verification matrix
    inconsistencies.

## Questions for a second reviewer/agent

1. Can the gallery be proven absent from production with the proposed `ViewId` architecture, or is
   a separate DEV-only registry required?
2. What is the safest CSP-compatible pre-paint theme bootstrap for both Tauri WebViews?
3. Should theme changes propagate live to an already open popup, and what existing Tauri mechanism
   is most appropriate?
4. Which modal and sensitive-content behaviors already exist, and which would be new scope?
5. Is one authored CSS file a hard product constraint or only one emitted stylesheet?
6. Does `color-mix()` work across the project's actual minimum WebView support matrix?
7. What security guarantees are expected while sensitive text is visually blurred?
8. Which gallery states require forced-state helpers versus Playwright interaction?
9. Can token values be generated or imported from a single source instead of copied?
10. Which parts should be split into separate implementation PRs while retaining one OpenSpec
    change?
