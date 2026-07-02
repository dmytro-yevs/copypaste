## Why

The Android app carries no brand identity: `ui/theme/` is a documented "design-strip" stub on
bare Material 3 (a read-only code audit confirmed `CpColors`/`AccentColor`/`CopyPasteTheme`/
`Shapes.kt`/`Type.kt`/`MotionSpec.kt` are **absent** — `Palette.kt`/`Skin.kt` were deleted but
never replaced). Meanwhile `STYLEGUIDE.md` defines one cross-platform design language and the
desktop app already implements it. Android must reach **full design parity**, not a partial
theme touch-up.

`STYLEGUIDE.md` is the **single source of truth** (the HTML reference is secondary; where the
product-owner review conflicts with either, the review wins). Material 3 is used only as a
behavioural/accessibility substrate — it does **not** define the visible design. This supersedes
the earlier "theme-level branding, screens stay de-styled" proposal, which contradicted the goal.

## What Changes

A complete, token-driven redesign of every user-visible Android surface and every state a user can
see, plus the test/localization infrastructure to keep it correct.

- **Semantic design system** (STYLEGUIDE §3–§11): build `CpColors` (surfaces, lines, text ramp,
  status `ok/warn/err/info`, and 10 content-type colors for the 12 kinds — PHONE→cNum, PATH→cFile
  aliases), `AccentColor`
  (6 hues × dark/light/onAccent/variant), `CopyPasteTheme(isDark, accent, translucency)` exposing
  `LocalCpColors`/`LocalAccent`, plus `CpShapes` (fixed radii), `CpTypography` (Inter + JetBrains
  Mono semantic roles, tabular-nums), `CpMotion` (120/200/300 ms + reduced), and a translucency/
  blur policy. Raw hex/dp/sp/alpha live only in token/effect files; screens consume tokens.
- **Iconography**: adopt a real **Lucide** Compose set; migrate off bespoke `NavIcons.kt` +
  `material-icons-extended`; every icon boxed and, if informative, described.
- **Appearance** (Settings → Display): Theme (Dark/Light/System), Accent (6), Translucency, Mask
  sensitive. Live preview, **persist on Save**, discard on unsaved exit; on Save the app propagates
  the change through an **app-scoped committed-appearance state** that `CopyPasteTheme` reads (not
  `Activity.recreate()`, which cannot re-theme the back stack). Local to Android (no cross-device sync).
- **Every surface redesigned to the guide**: app shell + floating-pill nav (§9.12 frosted blur),
  History list/rows/tiles/chips/filters/selection/empty+error states (12 kinds → 10 content-type colors,
  glyphs, swatch/thumbnail), full-screen Preview (+ fixing a masking a11y gap), Devices card grid
  (§9.7), Pairing flow (QR/scan/SAS/success/errors), all Settings tabs, Onboarding/Permissions,
  feedback (toasts/banners/sync badge+sheet/dialogs), About/Logs.
- **System-owned & invisible surfaces**: notifications, share target, ZXing scanner, and the
  intentionally UI-less `ClipboardFloatingActivity`/capture overlay/`ShareReceiverActivity` are
  **preserved** (branded/localized/tested where app-owned; never decorated or golden-parity'd).
- **Localization**: move ALL user-visible strings to resources; add complete `values-uk`; add a
  hardcoded-string lint gate. Locale-aware dates/numbers.
- **Accessibility & privacy**: AA contrast for all 6 accents × both themes; ≥48 dp targets; roles/
  state descriptions; color never the only signal; masked content never reaches semantics/goldens/
  logs.
- **Visual regression**: adopt **Paparazzi**; one central preview catalog + deterministic goldens.

Explicitly OUT of scope: quick-paste sheet / QS tile — **Deferred / separate epic** (STYLEGUIDE §9.13
declares it, but this epic ships NO placeholder UI for it; tracked in `cross-platform-parity.md`; desktop
Popup stays Desktop-only); any Rust crate,
`crates/copypaste-android`, UDL/FFI, generated bindings; landscape phone (portrait target; tablet/
foldable covered as responsive width, portrait).

## Capabilities

### New Capabilities
- `android-design-system`: semantic tokens (`CpColors`/`AccentColor`/shapes/type/motion) + M3
  mapping + translucency/blur policy — the visual contract all screens consume.
- `android-iconography`: the Lucide icon system, sizing/boxing rules, and migration off legacy icons.
- `android-appearance`: theme/accent/translucency preferences, live preview, Save→commit→publish
  app-scoped committed appearance state, versioned migration.
- `android-navigation-chrome`: app shell, floating-pill nav (§9.12), system bars/insets, adaptive
  (tablet/foldable) layout.
- `android-history`: history list, content-type tiles/rows, chips, device filter, selection, empty/
  error states, and sensitive masking in the list.
- `android-preview`: full-screen content preview, actions, gestures, and masking parity (a11y fix).
- `android-devices`: device-card field grid (§9.7), status/transport chips, dialogs, reduced-motion
  presence — keeping cloud-account-mismatch detection inert (CopyPaste-gldr).
- `android-pairing`: QR display/scan, scan-review, SAS confirmation, success, deep-link, errors —
  preserving `FLAG_SECURE`, IPC/revoke, and `peer_supabase_account_id` behaviour.
- `android-settings`: all tabs, fields/validation, Save/Discard, destructive flows.
- `android-onboarding-permissions`: onboarding, permission rationale/status/recovery, background-
  capture setup (app-owned surfaces only).
- `android-feedback-states`: toasts, banners, sync badge + detail sheet, confirm/destructive
  dialogs, progress — semantic status tokens + redundant icon/text.
- `android-system-surfaces`: preservation contract for notifications, share target, invisible
  overlays, and OS-owned surfaces (behaviour/flags kept; app-owned properties branded/localized).
- `android-localization-accessibility`: EN/UK resourcing, hardcoded-string gate, a11y baselines.
- `android-visual-regression`: Paparazzi golden framework, preview catalog, baseline policy, gates.

### Modified Capabilities
<!-- None: openspec/specs/ is empty; all capabilities above are new. -->

## Impact

- **Kotlin (UI-only), broad**: new `ui/theme/{Color,Theme,Shapes,Type,MotionSpec}.kt`; rewritten
  `Components.kt`/`SliderComponents.kt`/`SettingsComponents.kt`/`NavIcons.kt`; every screen file
  under `android/app/src/main/java/com/copypaste/android/` (shell, History*, Preview*, Devices*,
  Pair*, Settings*+tabs, Onboarding*, Permissions*, About, LogViewer, `ui/GlassToast`,
  `ui/SyncStatusBadge`); notification content sources (branding/localization only).
- **Resources**: `res/font` wiring; `res/values/strings.xml` (externalize hardcoded strings) +
  new `res/values-uk/strings.xml`; plurals where needed.
- **Build/test (new deps)**: Paparazzi plugin + a Lucide-Compose artifact — exact versions and
  toolchain compatibility (AGP 8.3.0 / Kotlin 1.9.23 / Compose compiler 1.5.11) are a **blocking S0
  proof, not pre-verified**; central preview catalog; Compose a11y/semantics tests; token/contrast
  tests. This removes the earlier "no dependency changes" constraint.
- **Behaviour/IPC/security**: PRESERVED — internal Compose component APIs may be refactored, but no
  behavioural/IPC/FFI drift; masking, pairing/revoke, FLAG_SECURE, invisible-surface flags intact.
- **Verification**: `scripts/android-verify.sh` is one gate among several (it does not prove device
  readiness); full gate set in `design.md`/`tasks.md`. OOM guard: one Android build at a time.
