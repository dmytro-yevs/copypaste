## Context

`STYLEGUIDE.md` is the single source of truth for one cross-platform design language (calm,
graphite, "color is information not decoration", two axes: theme × accent). It ships the exact
Android reference code in §11 (`CpColors`, `AccentColor`, `CopyPasteTheme`) and full component
specs in §9. The desktop app already implements it; Android does not.

A read-only code audit (results folded into this change) established the true baseline:
- **Design system absent (but recoverable from git history).** No `CpColors`/`AccentColor`/
  `CopyPasteTheme`/`Shapes.kt`/`Type.kt`/`MotionSpec.kt` on HEAD. `Palette.kt`/`Skin.kt` deleted
  (STYLEGUIDE §12 migration). A working `CpColors`/`AccentColor` implementation DID exist in
  `ui/theme/Color.kt` (commits `3fa8618e` two-axis theme → `86113198` CompositionLocal →
  `b734a9c2` on-accent AA fix + scrim, with `CpColorsTwoAxisTest` at `9553ff4e`) and was deleted
  later in the WIP snapshot `c10d193e`. **Decision: S1 recovers from `b734a9c2` and modernises**
  (re-tokenised to `tokens.css` @ pinned `6960539d`, incl. `errStrong`/`infoStrong`/`okStrong`) —
  not a rebuild from scratch. `Theme.kt`/`Components.kt`/`SliderComponents.kt` are documented
  "design-strip" stubs on bare M3. Inter + JetBrains Mono bundled in `res/font` but unwired.
- **No visual-test infra.** 0 `@Preview`; 0 golden/screenshot framework; zero instrumented Kotlin
  test sources (androidTest holds only an asset; gradle comments cite tests not in the tree). 22
  test files (170 `@Test` methods) exist.
- **Localization gap.** 438 strings; no source `values-uk`; ~8% of on-screen `Text()` hardcoded.
- **Gradle.** AGP 8.3.0, Kotlin 1.9.23, Compose BOM 2024.04.01, Compose compiler 1.5.11,
  minSdk 26, targetSdk 35 — Paparazzi and a Lucide-Compose artifact are candidate additions whose
  compatibility is to be proven by the S0 spikes (not yet verified).
- **Screen reality** (selected): no `MainShell.kt` — `FloatingTabBar` is private in `MainActivity`,
  text-only, no blur. `HistoryChips.chipColorFor` maps 12 kinds onto ~5 M3 roles; tiles render
  text labels not glyphs; List and Preview show **different** colors for the same kind (documented
  bug). `PreviewTextContent`/`PreviewImageContent` omit `clearAndSetSemantics` when masked (real
  a11y/security leak). Device card mostly present but chips are plain `Text` (not pills) and the
  roster fingerprint is not tap-to-copy. `PairActivity` sets `FLAG_SECURE` unconditionally.
  `peer_supabase_account_id` is sent as `None` and `detectCloudAccountMismatch` is fed `[]` (inert).
  Invisible surfaces are genuinely UI-less. `PairedPeerList.kt` is misnamed (pairing UI, not roster).

## Goals / Non-Goals

**Goals**
- Implement the STYLEGUIDE design system on Android exactly (§11 tokens, §9 components), token-driven.
- Redesign every user-visible surface and every state to the guide; one encoding per fact.
- Two-axis appearance (theme × accent) + Translucency + Mask sensitive; live preview, persist on
  Save, then publish via app-scoped committed appearance state (not `recreate()`); local to Android.
- Real Lucide icon system; EN + UK localization; AA a11y for all accents/themes; masked-secret safety.
- Deterministic visual-regression (Paparazzi) + preview catalog + real gate set.
- Preserve all behaviour/IPC/security invariants.

**Non-Goals**
- No Rust/`crates/copypaste-android`/UDL/FFI/generated-bindings changes.
- No Material You / dynamic color (brand accent authoritative; `surfaceTint = Transparent`).
- No quick-paste sheet / QS tile (net-new; deferred to a separate epic).
- No landscape phone layouts (portrait target); tablet/foldable is behind an S0 approval gate
  (best-effort non-regression until approved), not a committed deliverable.
- No behavioural/IPC drift; no decoration of invisible/OS-owned surfaces; no golden-parity on
  ZXing/share/OS surfaces.
- Do NOT activate the cloud-account-mismatch banner (peer ids remain unplumbed — CopyPaste-gldr).

## Decisions

- **D1 Source of truth.** STYLEGUIDE.md governs; product-owner review overrides on conflicts
  (theme adds **System**; EN+**UK** now; real blur; golden tests). HTML reference is illustrative.
  Token *values* are sourced from `crates/copypaste-ui/src/styles/tokens.css` at pinned desktop
  commit `6960539d`, not copied verbatim from a (possibly stale) §11 Markdown snippet; S0 re-pins
  STYLEGUIDE §10/§11 from that same commit so the human-readable mirror matches (tasks.md S0.14).
  Recorded design-system overrides from §11's reference code: (1) the §11 `cPath` field is dropped in
  favour of a PATH→`cFile` alias (10 unique content colors); (2) a `card` field is added as an
  explicit Kotlin-only alias of `elevated`, for STYLEGUIDE-parity naming, since §11's reference
  `CpColors` code omits it. Both are explicit overrides, not a byte-for-byte copy of §11's reference code.
- **D2 Semantic layer per §11, with the documented D1 overrides.** `CpColors` (incl. overlays
  hover/pressed/scrim, status incl. the additive `errStrong`/`infoStrong`/`okStrong` AA-text variants,
  and 10 content-type colors for the 12 kinds; PHONE→cNum, PATH→cFile aliases),
  `AccentColor`, `CopyPasteTheme` provide `LocalCpColors`/`LocalAccent`; M3 `ColorScheme` mapped
  fully per the explicit role table in android-design-system (primary=accent base, onPrimary=on-accent, background=bg, surface=panel,
  the container ladder per the explicit M3 role table (android-design-system), onSurface=text, onSurfaceVariant=dim, outline=border,
  outlineVariant=divider, error=err, scrim, `surfaceTint=Transparent`). Non-M3 concepts
  (selected/hover/pressed, content/status colors, accent-2) live only in `CpColors`/`AccentColor`.
- **D3 Shapes/Type/Motion tokens.** `CpShapes` fixed radii (chip 7 / ctl 8 / input 9 / card 13 /
  pill 999). `CpTypography` maps §4 roles onto Inter + JetBrains Mono `FontFamily` from `res/font`,
  tabular-nums for machine text. `CpMotion` durations 120/200/300 + `reduced` from the system
  animator-duration signal (no user motion setting).
- **D4 Theme = Dark/Light/System.** `System` resolves via `isSystemInDarkTheme()`. Persisted keys
  `theme_mode` + `accent`; `translucency` already exists. Defaults dark/indigo/translucent.
- **D5 Draft preview + app-scoped committed state (NOT recreate).** The Settings screen hoists
  appearance *draft* state above `CopyPasteTheme` for live in-screen preview. On Save it writes prefs
  in the single `saveScreenSettings` batch AND updates an **application-scoped observable
  committed-appearance state** (e.g. a `StateFlow`/`mutableStateOf` in the Application) that
  `CopyPasteTheme` reads, so every composed and future Activity re-themes. `Activity.recreate()` is
  NOT the propagation mechanism — it recreates only the current instance and cannot re-theme stopped
  back-stack/other-task activities; embedded `SettingsScreen` (MainActivity tab) and standalone
  `SettingsActivity` both propagate via the shared committed state. Draft never feeds committed state
  before Save. Fold the few fields currently written outside the batch into it; a failed `commit()`
  keeps dirty and reports failure (M6).
- **D6 Versioned `migrateThemeForTwoAxis()`.** Keep the canonical keys `theme_mode`/`accent`; version
  the migration latch so it removes only genuinely stale Liquid-Glass keys and stops deleting the
  canonical keys before the new getters are introduced; invoke once in `CopyPasteApp.onCreate` before
  the first appearance read. Do NOT rename the canonical keys.
- **D7 Backdrop-blur policy (real strategy, not `Modifier.blur`).** Chrome/sheets get a real
  **backdrop** blur that samples content behind them via a captured-layer `RenderNode`/`RenderEffect`
  strategy (Haze-style) on API 31+, or window-level blur for own-window surfaces; `Modifier.blur` on
  the pill's own layer is explicitly rejected (it blurs the pill's children, not the backdrop).
  Foreground icons/text compose above the blur layer. API 26–30, translucency off, or non-viable
  effect → opaque canonical fallback. Never block first paint; blur policy is injectable for
  golden determinism. **An S1 spike proves the strategy (perf/clipping/edge) before the design
  system commits.** Blur is disabled only by translucency-off or API<31 — Android exposes no
  "reduced transparency" API and battery-saver is not treated as one (resolved).
- **D8 Icons via Lucide.** One canonical provider, 24×24 line, rounded caps, fixed box per role;
  fallback for missing glyphs; migrate/retire `NavIcons.kt` + `material-icons-extended`;
  contentDescription only on actionable/informative icons, decorative hidden from semantics.
- **D9 Adaptive layout.** Portrait phone is the committed primary; tablet/foldable responsive width is
  CONDITIONAL on the S0 approval gate (not a committed deliverable until then); it otherwise uses responsive width
  (WindowSizeClass) with the same components — no separate landscape. Goldens cover the committed phone
  width; if the S0 gate approves, they also cover representative tablet and fold widths (portrait).
- **D10 Component APIs may change internally.** Narrow the earlier blanket "public signatures
  unchanged" to: no behavioural/IPC/FFI contract drift. Shared-component refactors for the token
  system are allowed; callers updated in the same slice.
- **D11 Screens consume tokens/components.** No raw hex/dp/sp/arbitrary alpha in screen files;
  behaviour-only dimensions (safe insets) may stay local. This replaces the old, wrong "no fixed
  dp" rule — canonical spacing is expressed in dp via the token scale.
- **D12 Masking is a hard contract, tested per surface.** Reuse `HistoryRowModel` masking logic;
  **fix the Preview gap** (`clearAndSetSemantics` on masked preview). Secret never appears in any
  semantics node (merged/unmerged), golden fixture, log, or notification. Fixtures use synthetic
  placeholders.
- **D13 Visual regression via Paparazzi.** Central `PreviewParameterProvider` catalog (no
  duplicated per-composable annotations). Deterministic device/locale/clock/font/fake-data; baseline
  dir + record/verify commands + diff threshold + never-auto-accept policy. Full accent matrix
  covered by token/contrast tests; representative goldens for screens (dark/light × 2 accents,
  EN/UK for text-heavy, 1.0/2.0 font scale, masked fixture, translucent+solid where stable).
- **D14 Sequential slices, one build at a time.** Foundation (S1/S2) gates screens; test/golden/l10n
  infra is established in **S2** (not S14) so it gates the screen slices — S14 is a late coverage
  audit only. OOM guard: never two concurrent Android native/Gradle builds.
- **D15 Git.** Branch `android-redesign` from local `main` HEAD; preserve the existing
  `docs/design/copypaste-app-demo.html` deletion; commit the repaired spec first; one logical
  commit per green slice; no `ANDROID_VERIFY_ALLOW_DIRTY=1` in CI; inspect `git status --short` +
  generated-binding diff after each run; no push without approval. Branch creation and any commit
  require the same explicit approval boundary as bd (spec-only until authorized); record the local
  `main` base SHA as evidence when execution is authorized.
- **D16 System chrome & first paint.** A system-chrome layer driven by the RESOLVED app theme SHALL
  set status-bar/nav-bar icon appearance via `WindowInsetsControllerCompat.isAppearanceLightStatusBars`/
  `isAppearanceLightNavigationBars` (in addition to the two preserved `SecureWindowChrome` SideEffects),
  and XML window-background + Android-12 splash resources SHALL paint a canonical first frame with no
  wrong-theme flash. Launcher/adaptive icon, splash, recents thumbnail, and sharesheet entry are
  inventoried as Preserve/Restyle/N-A. Tests: light/dark system-bar + manual Pixel first-paint.
- **D17 Cross-platform parity.** `cross-platform-parity.md` is the normative Android↔desktop contract
  (Exact / Native adaptation / Platform-only / Deferred), pinned to STYLEGUIDE sha256 `25b9bd05…`;
  tokens are machine-checked and components use paired structural fixtures (not pixel diff). Recorded
  Native adaptations: theme System-resolver, single-slot toast (actionable never dropped), pre-31
  geometry-preserving masking fallback, gesture/input, `preview_lines`. Quick-paste = Deferred. Any
  post-freeze shared-design change updates both platforms or records an exception.

## Risks / Trade-offs

- **R1 Scope size.** Full redesign across ~all surfaces + new test/localization infra is large;
  mitigated by the capability split, traceability matrix, and sequential gated slices.
- **R2 Greenfield foundation.** S1/S2 are from-scratch; a wrong token/type/shape mapping propagates
  everywhere. Mitigation: build straight from §11, add token/contrast tests first.
- **R3 Masking regressions.** Redesigning rows/preview risks re-leaking secrets. Mitigation:
  centralize masking, per-surface semantics tests asserting no plaintext, golden masked fixtures.
- **R4 Do-not-regress logic.** `peer_supabase_account_id`/mismatch banner, revoke ordering,
  unconditional `FLAG_SECURE`, invisible-surface flags. Mitigation: preservation requirements +
  tests; these are stated as invariants in the specs.
- **R5 Migration key collision** (D6). Mitigation: version the migration latch; remove only legacy
  keys; retain `theme_mode`/`accent`; run before first appearance read.
- **R6 Appearance-publish UX.** Publishing committed appearance state triggers app-wide recomposition;
  risk of flicker/lost transient state. Mitigation: publish only on actual change; rely on
  `rememberSaveable`; 300ms theme crossfade. `recreate()` is NOT the propagation mechanism.
- **R7 Tablet/foldable adds golden combinations.** Mitigation: WindowSizeClass with shared
  components; limited representative large-screen baselines, not a full cross-product.
- **R8 New deps (Paparazzi/Lucide) version drift.** Mitigation: pin via version catalog; confirm
  the Lucide artifact and the Paparazzi version (S0 spikes); compatibility is proven by the S0 proof
  task, not assumed.
- **R9 android-verify preconditions** (NDK/JDK≤21/clean tree). Mitigation: fast Kotlin-compile
  inner loop; full chain as the slice gate; commit-then-verify workflow with post-diff inspection.
- **R10 Verify gate ≠ device readiness.** `android-verify.sh` is build+JVM only. Mitigation:
  Paparazzi goldens + a11y/semantics tests + a manual Pixel/TalkBack checklist at milestones.
- **R11 Forced theme vs system-bar/first-paint flash.** Forced Dark/Light independent of OS can leave
  wrong status-bar icon contrast or a pre-Compose window-background flash. Mitigation: D16 system-chrome.
- **R12 Scanner screenshot leaks pairing token.** Mitigation: `FLAG_SECURE` on `PortraitCaptureActivity` (P0-1).
- **R13 M3 unmapped-role color leakage.** Material components may render default tonal/purple roles.
  Mitigation: full explicit ColorScheme map for every role (single strategy) + component-gallery leakage golden.
- **R14 Golden baseline/repo-size explosion.** Mitigation: representative matrix (no cross-product) +
  baseline size budget + LFS-vs-direct decision (S0).
- **R15 Paparazzi screens instantiate Settings/native/services.** Mitigation: stateless presentation
  seams + fakes (S2.9).
- **R16 Localization conversion alters formatted security/error messages.** Mitigation: format-argument
  parity tests + translator review.
- **R17 App-scoped state diverges from a failed preference commit.** Mitigation: publish only after
  `commit()==true` (D5/M6).
- **R18 CI runtime explosion (native + Paparazzi + connected per PR).** Mitigation: per-slice gate
  template (Required/Optional/N-A); heavy jobs gated to relevant paths.

## Resolved decisions (previously open; no SHALL may conflict with these)
- **Form factor** — **committed scope: Pixel-class portrait phone.** Tablet/foldable responsive work
  is behind an **S0 decision gate (not yet product-approved)**; until approved, tablet/fold goldens are
  NOT unconditional requirements and wider widths are best-effort non-regression only. Landscape = a
  functional fallback only (not golden-tested). Committed golden device config (phone):
  phone `1080×2400 @2.75x (Pixel-class), API 34`, portrait, EN+UK, font scale {1.0, 2.0}. Tablet
  `1600×2560 @2.0x` and fold `1840×2208 @2.6x` are added ONLY if the S0 tablet/fold gate is approved.
- **Share receiver** — stays UI-less (matches current behaviour); no success/failure UI added.
- **ZXing `PortraitCaptureActivity`** — `FLAG_SECURE` **REQUIRED** before the camera preview (the
  scanned peer QR is a valid pairing credential: fingerprint + token). S8 owns its window security,
  orientation, decoder, and lifecycle; ZXing's preview visuals stay unskinned. (Reverses the earlier
  incorrect "no FLAG_SECURE" decision.)
- **Blur-disable signal** — translucency-off or API<31 only (no Android reduced-transparency API;
  battery-saver ≠ transparency preference).
- **Golden framework** — Paparazzi (JVM, no device).
- **Connected-test CI availability** — `:app:connectedDebugAndroidTest` (the `android-instrumented`
  job in `.github/workflows/ci-android-build.yml`) is **CI advisory-only until CopyPaste-k1l0 is
  resolved**: that job runs with `continue-on-error: true` because the managed AVD does not boot on
  arm64 macOS runners. Interim pre-merge catch mechanism until CopyPaste-k1l0 lands: a mandatory
  local `:app:connectedDebugAndroidTest` run for security-relevant slices (S4, S5/S6, S8, S9/S10,
  S12, S15), backed by Paparazzi/JVM proxies. Nightly instrumented runs become possible only after
  CopyPaste-k1l0 is resolved; no nightly instrumented job exists today.

## Remaining spikes (S0/S1, before dependent slices are accepted)
1. **Lucide artifact** — pin the exact Maven coordinate + version (candidate `com.composables:icons-lucide`),
   verify Kotlin 1.9.23 / Compose compiler 1.5.11 compatibility, record repo/ISC-license/SBOM and
   APK-size impact; if none acceptable, generate a curated `ImageVector` subset from upstream ISC SVGs
   with an update script + license notice. (B9)
2. **Paparazzi version** — pin the exact version compatible with AGP 8.3.0 / Kotlin 1.9.23; a
   zero-production-code proof task snapshots one bundled-font fixture; decide whether an AGP/Kotlin/
   Gradle bump is permitted if none fits; decide direct-PNG vs Git LFS baseline storage. (B10)
3. **Backdrop-blur strategy** — prototype the captured-layer approach (perf/clipping/edge) before S1
   commits the design system. (B2)
