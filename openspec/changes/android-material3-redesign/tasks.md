## Conventions

- Sequential slices; foundation (S1/S2) gates screens; **one Android build at a time** (OOM guard).
- Test/localization/golden **infrastructure is established in S1/S2, before screen slices**; each
  screen slice then adds and verifies its own preview fixtures, Paparazzi baselines, UK strings, and
  a11y tests. See `component-inventory.md` for the full view-level component list.
- Every slice ends GREEN on its gate set (§ Gates) — a compile is the floor, not proof.
- **Per-slice commit workflow (M11):** (1) run fast checks while dirty (`(cd android && ./gradlew
  :app:compileDebugKotlin -x buildCargoNdk)`, lint, JVM tests); (2) review/stage/commit the logical
  slice; (3) run clean-tree `scripts/android-verify.sh` + `(cd android && ./gradlew
  :app:verifyPaparazziDebug)`; (4) inspect `git status --short` +
  generated-binding diff; (5) on failure/diff, fix and amend, rerun; (6) only then attach evidence /
  close the bd issue. `docs/design/copypaste-app-demo.html` was already deleted on `main` (commit
  `ff1d987d`) — no action needed when branching.
- bd epic + slice issues are (re)aligned to this structure **after approval** (bd untouched now).
- Branch `android-redesign` off local `main` HEAD; no push.

## Gates (run per slice; every gate names its exact command)

> Gradle wrapper lives at `android/gradlew` (NO root wrapper). All Gradle gates run from the module:
> `(cd android && ./gradlew :app:<task>)`, matching CI `working-directory: android`. Add
> `-x buildCargoNdk` for pure-Kotlin/UI tasks that don't need the native `.so`.

- [ ] `openspec validate android-material3-redesign --strict`
- [ ] fast inner loop (dirty): `(cd android && ./gradlew :app:compileDebugKotlin -x buildCargoNdk)` + `(cd android && ./gradlew :app:lintDebug)` — warnings-as-errors **once configured in S2**
- [ ] hardcoded-user-text gate (Lint + AST script across all sinks) — established/enforced from S2 (script path set in S2.6)
- [ ] JVM unit + token/AA-contrast (incl. error/onError): `(cd android && ./gradlew :app:testDebugUnitTest)` — from S1.
      **Amended S2.5:** `app.cash.paparazzi:1.3.4`'s plugin disables AGP's
      `isReturnDefaultValues` mockable-android.jar for the WHOLE module's test
      classpath (confirmed upstream, unresolved: cashapp/paparazzi#1908/#1331/
      #1922), so `testDebugUnitTest` now runs ONLY the Paparazzi-suite
      (`com.copypaste.android.paparazzi.*`) with a working classpath
      substitute (`build.gradle.kts`'s `androidLogStub` shim). Full JVM
      coverage — everything `testDebugUnitTest` covered through S1, incl.
      token/AA-contrast — now ALSO requires
      `(cd android && ./gradlew :app:testDebugUnitTestPreExisting)`. Run BOTH
      commands for this gate from S2 onward; see `app/build.gradle.kts`'s
      `testDebugUnitTestPreExisting` comment block for the full rationale.
- [ ] Paparazzi golden verify: `(cd android && ./gradlew :app:verifyPaparazziDebug)` — infra from S2 (requires `ANDROID_HOME`/`ANDROID_SDK_ROOT` set — Paparazzi's `Environment.detectEnvironment()` needs it even though AGP itself resolves the SDK via `local.properties`); each screen slice adds its baselines
- [ ] connected semantics/a11y (roles/state, focus, ≥48dp vs visible bounds, masked-secrecy merged+unmerged, FLAG_SECURE): `(cd android && ./gradlew :app:connectedDebugAndroidTest)` on the S0-defined AVD (API 34, Pixel profile) — **CI advisory-only until CopyPaste-k1l0 is resolved**; mandatory local run for security-relevant slices (S4, S5/S6, S8, S9/S10, S12, S15)
- [ ] `scripts/android-verify.sh` (bindings → native .so → assembleDebug → JVM), clean tree
- [ ] post-run `git status --short` + generated-binding diff inspection

**Per-slice gate template:** each slice marks each gate Required / Optional / N-A and records artifact
paths (Paparazzi diffs, test reports). "Scale to the slice" = which of the above are Required for that slice.
Connected semantics/a11y/security tests `:app:connectedDebugAndroidTest` are **required locally per
slice** for S4 (focus/insets/nav), S5/S6 (masked secrecy), S8 (`FLAG_SECURE`), S9/S10 (focus/input/
permission flows), S12 (window flags / invisible-surface security, per S12.2), and S15 — but **CI
advisory-only until CopyPaste-k1l0 is resolved** (`.github/workflows/ci-android-build.yml`'s
`android-instrumented` job is `continue-on-error: true` because the managed AVD does not boot on
arm64 macOS runners). The interim pre-merge catch mechanism until then is the mandatory local run of
this command for the slices above, backed by Paparazzi/JVM proxies. Nightly instrumented runs become
possible only after CopyPaste-k1l0 is resolved (no nightly instrumented job exists today). Other
slices may be N-A only with a recorded rationale.

## 0. S0 — Scope lock, spikes, branch, gate wiring

- [x] 0.1 Create `android-redesign` off `main` HEAD (the `docs/design/copypaste-app-demo.html`
      deletion, commit `ff1d987d`, is already on `main` — no action needed); no push.
- [x] 0.2 Finalize the traceability matrix + state inventory below (verify vs code + `component-inventory.md`).
- [x] 0.3 **Spike — Lucide (B9):** pin exact Maven coordinate + version, verify Kotlin 1.9.23 / Compose
      compiler 1.5.11, record ISC-license/SBOM + APK-size; else curate an `ImageVector` subset from
      upstream ISC SVGs with an update script. Gates S2.
- [x] 0.4 **Spike — Paparazzi (B10):** pin exact version vs AGP 8.3.0/Kotlin 1.9.23; zero-production-code
      proof snapshot of one bundled-font fixture; decide AGP/Kotlin bump-if-needed + direct-PNG vs Git LFS.
- [x] 0.5 **Spike — backdrop blur (B2):** prototype the captured-layer strategy with PASS/FAIL metrics
      — target devices/API, frame-time/jank budget, memory/allocation ceiling, clipping cases,
      nested-scroll behaviour, exact fallback trigger. The deterministic golden stand-in does NOT
      validate the device effect → add a manual on-device screenshot acceptance. Blocks S1.
- [x] 0.6 Confirm golden device configs + connected-test emulator config (design.md Resolved/Spikes).
- [x] 0.7 Confirm & specify `migrateThemeForTwoAxis()` call site/order (D6/M7) — audit/plan only;
      production migration code + tests land in **S3** (not S0).
- [x] 0.8 Create bd epic + all slice issues with dependencies + acceptance; pin the exact branch base
      commit SHA for `android-redesign`.
- [x] 0.9 **Tablet/foldable scope gate** — NOT yet approved; committed scope = Pixel portrait phone.
      If approved: add WindowSizeClass breakpoints/max widths + tablet/fold golden configs; else wider
      widths are best-effort non-regression only (no tablet/fold golden SHALL).
- [x] 0.10 System-bar + first-paint prototype/decision: resolved-theme status/nav icon appearance,
      XML window background + Android-12 splash to avoid wrong-theme flash.
- [x] 0.11 Content visual-kind decision: `ContentVisualKind` enum + resolver precedence (P0-6);
      confirm SECRET override is approved new behaviour.
- [x] 0.12 CI plan: name workflow file(s) (`.github/workflows/ci-android-build.yml` + optional
      visual-regression workflow), Paparazzi job + failure/diff artifact upload, hardcoded-text +
      l10n-completeness gates, Robolectric dependency/version, Kotlin + Lint warnings-as-errors config.
      The pre-existing `android-instrumented` job in `ci-android-build.yml` stays
      `continue-on-error: true` (CI advisory-only until CopyPaste-k1l0 is resolved — the managed AVD
      does not boot on arm64 macOS runners); this task does not fix that AVD-boot issue, only wires
      the new Paparazzi/hardcoded-text/l10n gates around it.
- [x] 0.13 **Parity freeze (`cross-platform-parity.md`):** confirm the pinned STYLEGUIDE sha256, record
      the desktop redesign base/target commit (pinned: `6960539d`, see `cross-platform-parity.md`),
      and set drift-handling. Prerequisite before S1/S2 close.
- [x] 0.14 **Re-pin STYLEGUIDE.md §10/§11:** refresh the fenced token blocks in §10/§11 from the
      current `crates/copypaste-ui/src/styles/tokens.css` (pinned commit `6960539d`), including the
      additive `--err-strong`/`--info-strong`/`--ok-strong` tokens absent from the current §10/§11
      snapshot, per the Drift rule in `cross-platform-parity.md`.

## 1. S1 — Design-system foundation  → `android-design-system`

- [x] 1.1 `ui/theme/Color.kt`: `CpColors` — surfaces (bg/panel/elevated/card-alias/raised/raised2),
      lines, text ramp, **overlays (hover/pressed/scrim)**, status ok/warn/err/info/errStrong/
      infoStrong/okStrong, and **10** content-type colors (PHONE→cNum, PATH→cFile aliases) Dark+Light,
      values sourced from `crates/copypaste-ui/src/styles/tokens.css` at pinned commit `6960539d`
      (not a stale §11 Markdown copy — see S0.14); `AccentColor` enum;
      central selected(16%/12% from accent) + disabled(mute/45%) derivation.
- [x] 1.2 `ui/theme/Theme.kt`: `CopyPasteTheme(isDark, accent, translucency)` → `LocalCpColors`/
      `LocalAccent` + **explicit M3 role table** (container ladder bg/panel/elevated/raised/raised2;
      surfaceTint=Transparent; non-mapped roles unused). Keep both `SecureWindowChrome` SideEffects verbatim.
- [x] 1.3 `CpShapes` (§5 radii); `CpTypography` (frozen table; **bundle a real Inter 700 face + license**
      for Title 700 — record upstream version/checksum/license + APK-size impact; other roles use existing
      Inter 400/500/600 + JBM 400/500; tabular figures, wire `res/font`) + **font-resource test** (every
      role → a real bundled face, no synthesis/fallback; paired type fixture proves desktop also renders Inter 700);
      `CpMotion` (§6 + `reduced` from system animator signal) — `reduced` MUST disable the nav spring,
      not just zero durations (§4).
- [x] 1.4 Backdrop-blur policy holder per the S0 spike (D7); injectable override for tests/previews.
- [x] 1.5 Token + AA-contrast unit tests (post-alpha-compositing) for all 6 accents × themes, incl on-accent.
- [x] 1.6 Theme/accent crossfade (`--dur-theme` 300ms) as an explicit transition, collapsed under reduced motion (§4).
- [x] 1.7 Full explicit light/dark `ColorScheme` map for **every** consumed M3 role (single strategy —
      no per-component-override alternative) incl. **contrast-safe onError** +
      errorContainer; invalid/corrupt persisted-enum fallback to defaults; `CpSpacing`/`CpElevation`/
      `CpDimensions`; system-bar appearance + XML window-background/splash first-paint tokens (D16).
- [x] 1.8 `ContentVisualKind` enum + resolver (precedence: isSensitive→SECRET, image/file, TextKind, TEXT) + unit tests (P0-6).
- [x] 1.9 **Implement + value-inspection-test the frozen `CpTypography`/`CpDimensions` tables** (values
      are normative in `android-design-system`; no ranges) — S1.9 implements/tests them, it does not decide them.

## 2. S2 — Icons + shared components  → `android-iconography` (+ `android-design-system`)

- [x] 2.1 Vendor the curated Lucide `ImageVector` subset (S0.3 decision — NO Maven dep, all published
      artifacts need Kotlin 2.0+): `scripts/generate-lucide-icons.sh` (svg-to-compose, pinned Lucide
      SHA) → `ui/theme/icons/` one-property-per-icon; third-party NOTICE (Lucide ISC +
      svg-to-compose MIT); canonical provider, boxed sizes per role, fallback policy.
      **Deviation:** DevSrSouza/svg-to-compose has no resolvable Maven Central/JitPack artifact —
      used the pre-authorized hand-port fallback (`generate-lucide-icons.mjs`, an independent
      SVG-path → Compose PathBuilder DSL converter) instead; recorded in `android/NOTICE` and bd notes.
- [x] 2.2 Migrate `NavIcons.kt` + `contentIconFor()` to the vendored Lucide set; retire
      `material-icons-extended` use. `NavIcons.kt` deleted entirely (not just refactored) to satisfy
      the literal "NavIcons.kt no longer exists" scenario; `contentIconFor` moved to `ui/theme/ContentIcon.kt`.
- [x] 2.3 Re-base `Components.kt`/`SliderComponents.kt`/`SettingsComponents.kt` on tokens
      (`LocalCpColors`/`LocalAccent`/`CpShapes`/`CpTypography`): buttons §9.1, toggle/segmented §9.2,
      inputs §9.3, chips/badges/tiles §9.4, card §9.7, banner §9.8, modal §9.9, empty §9.10. New
      `ui/theme/Banner.kt` (§9.8) + `CpBadgeChip` (transport/verified/this-device pill primitive).
- [x] 2.4 Central preview catalog scaffolding (`PreviewParameterProvider`) for the gallery —
      `ui/theme/preview/PreviewCatalog.kt` (`ThemeFixture`/`ThemeFixtures`/`ThemeFixtureProvider`/`CpPreviewScaffold`).
- [x] 2.5 **Establish golden infra (B3):** add pinned Paparazzi + config + baseline dir + naming +
      `record/verifyPaparazziDebug` + diff threshold + never-auto-accept + LFS decision. From here every
      screen slice adds its own fixtures/baselines. Proof test recorded+verified green
      (`BundledFontSnapshotTest`). **Toolchain fallout (documented, fixed, not silently absorbed):**
      required AGP 8.3.0→8.3.2 + Kotlin 1.9.23→1.9.24 + Compose Compiler 1.5.14 (the officially
      blessed pairing for Kotlin 1.9.24 — no suppression flag needed; Paparazzi's POM forces kotlin-gradle-plugin 1.9.24 on
      the plugin classpath); Paparazzi's plugin also disables AGP's `isReturnDefaultValues` mockable
      jar for the whole module, breaking 20 pre-existing JVM tests (confirmed upstream/unresolved:
      cashapp/paparazzi#1908/#1331/#1922) — fixed via a no-op `android.util.Log` shim
      (`src/androidLogStub/`) + a new `testDebugUnitTestPreExisting` task; see `app/build.gradle.kts`
      and this slice's Gates-section amendment for the two-command JVM-test-gate replacement.
- [x] 2.6 **Establish localization infra (B3):** string-resourcing discipline + the hardcoded-user-text
      lint/AST gate across all sinks (M1, exact script path) + `translatable="false"` allowlist (M2). Enforced from here on.
      `scripts/check-hardcoded-text.mjs` (baselined at 91 pre-existing entries) +
      `scripts/check-l10n-completeness.mjs` (allowlist blocking; EN→UK coverage report-only, S13 scope).
- [x] 2.7 **Deps + CI wiring (P0-9):** add Robolectric (pinned) + Paparazzi (pinned, per S0 proof);
      configure Kotlin compiler warnings-as-errors AND Lint warnings-as-errors; add the Paparazzi CI
      job + failure/diff artifact upload; wire the hardcoded-text + l10n-completeness gates into
      `.github/workflows/ci-android-build.yml`. **Partial by design:** Lint warnings-as-errors is live
      (`android.lint { warningsAsErrors = true }` + committed `app/lint-baseline.xml` grandfathering
      261 pre-existing warnings). Kotlin compiler `allWarningsAsErrors` was **deliberately NOT enabled**
      — kotlinc has no baseline/suppression mechanism, and ~25 pre-existing warnings live in files
      outside this slice's scope; enabling it now would force out-of-scope edits. Tracked as follow-up.
- [x] 2.8 **Remove `material-icons-extended` dependency** (not just imports); publish the exact icon-role
      size table using exact `CpDimensions` roles: tile container 32/36dp, glyph box 18dp, nav glyph 24dp, meta icon 20dp.
      **Contradiction found (not silently resolved):** 6 screen files outside the S2 shared-component
      boundary (owned by S6/S8/S10) still import `material-icons-extended`; removing the Gradle
      dependency now would break `:app:compileDebugKotlin` for out-of-scope files. Dependency kept in
      place, actual removal deferred to whichever slice migrates the last screen off it. Icon-role size
      table delivered as a KDoc table in `LucideIcons.kt` + machine-checked in `LucideIconsTest`.
- [x] 2.9 **Paparazzi seams:** add stateless/presentation models + fakes so golden screens never
      instantiate repositories, native FFI, or Android services. `CopyPasteTheme`'s
      `(view.context as Activity)` unconditional cast (blocks EVERY golden through this root
      composable) made a safe `as?` seam; `SecureWindowChrome`'s two SideEffects left verbatim per the
      explicit landmine.
- [x] 2.10 **No-op-preference source gate:** a static check that FAILS when a user-facing preference has
      no production consumer outside Settings/UI/storage code (adapter/DI-aware to avoid false positives).
      `scripts/check-noop-preferences.mjs` — not wired as a blocking CI gate (not in this slice's
      mandatory Gates list); reports 8 findings incl. `notifyOnSensitiveSkip`/`autoApplySyncedClip`,
      matching S9.5's named repair targets.
- [x] 2.11 **Cross-platform parity gate:** generate canonical `parity/tokens.json` from
      `crates/copypaste-ui/src/styles/tokens.css` at pinned commit `6960539d` (STYLEGUIDE §10/§11 is
      the re-pinned human-readable mirror, not the generator's input — see S0.14);
      `TokenParityTest` asserts Android tokens == it (web checked on the desktop side). Paired
      structural fixtures from shared `parity/fixtures/*.json` (history row, masked row, device card, SAS
      dialog, settings group, banner, destructive modal, empty state, sync status): Android structural
      conformance blocking here; desktop paired evidence (Playwright) is a desktop-epic dependency whose
      target commit is recorded in S0 (marked Blocked if unavailable, never silently green).
      `scripts/gen-parity-tokens.mjs` generates the JSON; every fixture's `desktop.status` is
      machine-asserted `"Blocked"` (`ParityFixturesTest`).
- [x] 2.11a **Retire the legacy parity gate:** delete `scripts/parity-check.mjs` and
      `.github/workflows/parity.yml` — the old script hardcodes the deleted
      `android/app/src/main/java/com/copypaste/android/ui/theme/Color.kt` path and cannot map the
      ~35 additive non-color tokens introduced by the desktop redesign; `TokenParityTest` +
      `parity/tokens.json` (above) fully replace it.

## 3. S3 — Appearance  → `android-appearance`

- [x] 3.1 `Settings.themeMode`/`accent` on the canonical keys `theme_mode`/`accent`; version the
      migration latch so it no longer deletes them (D6/M7); fold into `saveScreenSettings`.
- [x] 3.2 `DisplayTab`: Theme segmented (Dark/Light/System), Accent swatch row (6, selected ring),
      Translucency, Mask sensitive; delegate to shared components.
- [x] 3.3 Live preview: hoist appearance draft above `CopyPasteTheme`; Save→persist then publish the
      app-scoped committed appearance state that `CopyPasteTheme` reads (NOT `recreate()`; works for
      embedded MainActivity tab and standalone SettingsActivity).
- [x] 3.4 Tests: migration once/idempotent, retains canonical keys regardless of getter-read order
      (order-independence, not order-dependence, is the fixed D6 invariant — see
      `SettingsThemeMigrationTest`); committed-survives-process-death; live-preview/Save/Discard
      (`AppearanceStateTest` "discarding a draft change never touches AppearanceStore" — the
      store-level contract a Settings-screen Discard relies on); System reacts to OS change.

## 4. S4 — Shell + navigation  → `android-navigation-chrome`

- [x] 4.1 Extract shell/nav from `MainActivity` into reusable, previewable composables
      (`ui/shell/MainShell.kt`, `ui/shell/NavPill.kt`, `ui/shell/NavGradientFade.kt`) — `NavPill`
      is hermetic (stateless params, no repository/FFI/Activity) per the S2.9 seam rule.
- [x] 4.2 Floating-pill nav §9.12: frosted blur (D7 captured-layer strategy, real 31+/opaque
      fallback via `rememberResolvedBlurMode()`), 3 tabs w/ Lucide icons (24dp `navGlyph`),
      accent-selected pill (`accent @ 18%`), `--bg` gradient fade, system-bar/gesture/cutout/IME
      insets (pill hidden outright while IME visible), restored selected tab (`rememberSaveable`),
      reduced motion disables the selection spring (`cpMotionSpec`).
- [x] 4.3 S0 tablet/foldable gate NOT approved (design.md "Resolved decisions") — phone-portrait
      only, no `WindowSizeClass` adaptive width added. Sync-status: shell-owned position above the
      pill's measured footprint, never overlapping it, respecting the same insets.

## 5. S5 — History  → `android-history`

- [ ] 5.1 Content-type tiles §3.7/§9.4: 12 kinds → 10 c-* colors (PHONE→cNum, PATH→cFile), glyph (Lucide) or swatch(COLOR)/thumb(IMAGE),
      SECRET lock + c-secret. Single `chipColorFor` shared by list AND preview (kill divergence).
- [ ] 5.2 Row §9.5 (tile, preview mono/sans per kind incl URL=mono, meta with tinted type word,
      pin star, actions), date-group headers §9.6, device filter chips as pills, selection/multi.
- [ ] 5.3 States: loading, populated, empty (normal/private), no-results, **error/degraded** (NEW
      presentation-state — S5 owns the plumbing; NO repository/IPC behaviour change).
- [ ] 5.4 Masking in list preserved (blur (31+) / geometry-preserving opaque overlay over sanitized
      representation (<31), `clearAndSetSemantics`); bulk-copy excludes
      sensitive; masked contentDescription never leaks. **Partial-span strategy (P0-7):** replace
      sensitive spans with a localized placeholder BEFORE building the `AnnotatedString` (or clear at
      the parent) so plaintext never enters any node; test asserts every plaintext fragment is absent
      from the COMPLETE semantics dump (merged + unmerged), not just the merged contentDescription.
- [ ] 5.5 Previews (incl masked row) + goldens; semantics test (no plaintext).

## 6. S6 — Full-screen preview  → `android-preview`

- [ ] 6.1 Preview chrome/content/actions/gestures on tokens; content-type color = same source as list.
- [ ] 6.2 **Introduce Reveal (NEW)**: add a Reveal action to `PreviewActionRow` and wire a `revealed`
      state (keyed `remember(item.id)`) through `PreviewOverlay`/`PreviewContent`/`PreviewImageContent`,
      mirroring `HistoryRow`'s existing `revealed by remember(item.id)` pattern — today Preview has no
      Reveal control and `PreviewContent` hardcodes `revealed = false`. Also **fix masking a11y gap**:
      `clearAndSetSemantics` on masked text/image; mono per kind.
- [ ] 6.3 States: text/url/code/json, image loading/success/failure, file meta/open/save failure,
      masked/revealed, large content. Preview masked-secrecy semantics test.

## 7. S7 — Devices  → `android-devices`

- [ ] 7.1 Device card §9.7: own-device grid + paired-peer 8-field grid, natural height, labels/values
      mono tabular; fingerprint first16…last8 **tap-to-copy** (copies full); transport/Verified/
      This-device as pills/badges §9.4; footer Unpair+Revoke danger.
- [ ] 7.2 States: scanning, discovered, offline/reconnecting, no-peers, error; presence dot+label;
      reduced-motion presence glow; all `DevicesDialogs` states (unpair/revoke/revoke-rotate/
      revoke-error/revoke-all).
- [ ] 7.3 PRESERVE: `detectCloudAccountMismatch` inert (`[]`, gldr); revoke ordering (audit-first);
      local-only unpair (no peer signal).

## 8. S8 — Pairing  → `android-pairing`

- [ ] 8.1 QR display (`QrHelper`/`PairQrCard`, lifetime/progress/warning, blur-at-rest), scan launch,
      deep-link (`cppair://`), scan-review card (`PairedPeerList.kt`→pairing), **six-digit SAS confirm
      (Match/Doesn't-match; full fingerprint only supplemental; preserve polling/watchdog/waiting/
      terminal; no SAS/token logging)**, `PairSuccessPopup`, connecting/provisioning/errors/cancel/retry.
- [ ] 8.2 PRESERVE: `PairActivity` unconditional `FLAG_SECURE`; IPC via `PairController`/
      `PairProvisioning`/`PairBootstrapSync`; `peer_supabase_account_id=None`; revoke semantics.
- [ ] 8.3 `PortraitCaptureActivity`: **set `FLAG_SECURE` before `super.onCreate`/preview init** (P0-1 —
      the scanned peer QR is pairing material); own theme/orientation/decoder/lifecycle; ZXing preview
      visuals unskinned. Connected test asserts `FLAG_SECURE` + blocked recents for BOTH `PairActivity`
      and `PortraitCaptureActivity`.

## 9. S9 — Settings  → `android-settings`

- [ ] 9.1 All tabs (General/Display/Sync/Storage/Notifications) + `SettingsComponents` on tokens;
      states normal/focused/disabled/dirty/saved/validation-error/destructive/loading.
- [ ] 9.2 Preserve draft model + Save/Discard; fold Save-owned fields into the atomic
      `saveScreenSettings` batch; **immediate (`allowScreenshots`, `relayEnabled`, `supabaseEnabled`)
      and ephemeral (export include-sensitive) controls MUST stay OUT of the batch** (persistence-mode
      requirement).
- [ ] 9.3 Implement the per-tab/control matrix from `behavior-and-state-coverage.md §E` — each field:
      persistence mode · validation · disabled precondition · async owner · error surface · Save/Discard
      participation · process-death behaviour; incl. max-items prune confirm, import/export/vacuum results,
      transport-immediate behaviour, and notify/sound independence.
- [ ] 9.4 **Settings 3-layer verification (coverage §I):** per row — (1) persistence/migration test
      (old key/value/default/clamp; upgrade fixture incl. legacy keys, corrupt/out-of-range/missing
      values, keystore secrets, old `history_size`/appearance migration); (2) consumer test proving
      true/false/boundary values change behaviour; (3) UI test proving the stored value is displayed and
      changing the control reaches the consumer. Record activation timing per row. Non-tab state included.
- [ ] 9.5 **Repair the no-op/legacy settings (§I):** wire `auto_apply_synced_clip` (Android inbound-
      transport seam), `notify_on_sensitive_skip` (suppression-branch toast), `max_file_size_bytes`
      (clipboard/share/import enforcement); hide legacy `sync_backend` as an effective control (retain
      key). Behaviour test each. No forbidden Rust/UDL edits — Android-side seams only.

## 10. S10 — Onboarding + permissions  → `android-onboarding-permissions`

- [ ] 10.1 `Onboarding*` (Activity/Screen/Cards/Dialogs incl. crash-detected, `OnboardingPermissions`),
      `PermissionsSettingsActivity`, `BackgroundCaptureSetupActivity` on tokens.
- [ ] 10.2 Permission states granted/denied/permanently-denied/n-a for notifications/camera/overlay/
      battery/OEM-autostart; app-owned rationale/status/recovery only; test intents (OS pages not styled).

## 11. S11 — Feedback states  → `android-feedback-states`

- [ ] 11.1 **Feedback-producer inventory (#15):** enumerate every current Toast/Snackbar/custom-toast/
      dialog/banner/notification outcome → owning screen/service · semantic kind (success→`ok`) · message
      resource · action/retry · migration slice. Migrate all producers to `ui/GlassToast`/banners §9.8/
      `ui/SyncStatusBadge`(+sheet)/dialogs on status tokens; color never sole signal (icon+text).
- [ ] 11.2 About (repo link/licenses/version+build, no-handler graceful) and Logs (level via
      icon/text+color, load/empty/no-match, copy/export success+failure) — explicitly (traceability
      assigns both to S11).

## 12. S12 — System & invisible surfaces  → `android-system-surfaces`

- [ ] 12.1 Notifications — per-channel rows (ID · creating owner · importance · visibility · actions ·
      PendingIntent target · small icon · localized metadata · migration decision):
      `copypaste_service` (Open→`MainActivity`, Pause/Resume→`CaptureControlReceiver`),
      `copypaste_copy_event`, `copypaste_pair_request` (→`DevicesActivity` `EXTRA_AUTO_OPEN_SAS`),
      **`copypaste_sync`** (native/encryption-unavailable, move off hardcoded strings), + `ServiceRestartWorker`
      ID 1010 reuse. Test Open/Pause/Resume/pairing targets. §4: channel importance/metadata cannot change
      post-creation — state Preserve or migrate (new channel id) per ID.
- [ ] 12.2 PRESERVE invisible surfaces (`ClipboardFloatingActivity`, `CaptureOverlayController`,
      `ShareReceiverActivity`): window flags, focus timing, privacy — do not decorate (resolved:
      stays UI-less — see `android-system-surfaces`).
- [ ] 12.3 OS-owned (runtime perms, sharesheet, settings pages, OEM autostart): correct
      labels/icons/intents only; no golden parity. **ZXing is a library UI, not OS-owned** — S8 owns
      its window security (`FLAG_SECURE`)/orientation/decoder/lifecycle; only its internal preview visuals are unskinned.
- [ ] 12.4 Resource/system surfaces (D16 + `component-inventory.md` resource table) — each Preserve/
      Restyle/N-A with verification + EN/UK where text exists: legacy+adaptive+monochrome launcher icon,
      Android-12 splash icon/bg + pre-12 window background, recents color/label/thumbnail-privacy,
      sharesheet app label/icon + direct-share (if any), notification small icons (monochrome), XML
      DayNight themes + status/nav-bar defaults before Compose.

## 13. S13 — Localization completion + audit  → `android-localization-accessibility`
(the gate + resourcing discipline are established in S2; screen slices externalize as they go)

- [ ] 13.1 Audit that ALL user-visible strings are externalized (~8% legacy hardcoded); plurals; formatted args.
- [ ] 13.2 Complete `values-uk/strings.xml` (translatable keys only); localize notification/dialog/error/contentDescription.
- [ ] 13.3 Locale-aware dates/numbers/sizes; EN/UK + long-string + 200%-scale tests; RTL/pseudo-locale
      stress (start/end, no hardcoded left/right) even if RTL not shipped (§4).

## 14. S14 — Golden coverage audit + matrix completion  → `android-visual-regression`
(Paparazzi infra + baseline policy established in S2; each screen slice already added its baselines)

- [ ] 14.1 Audit fixture/golden coverage against the state inventory; fill representative gaps
      (dark/light × 2 accents, EN/UK for text-heavy, 1.0/2.0 scale, masked fixture, translucent+solid
      where stable; phone width committed, tablet/fold widths only if the S0 gate approves). Confirm the no-cross-product rule holds.
- [ ] 14.2 Verify baseline dir/naming, `record/verifyPaparazziDebug`, diff threshold, never-auto-accept,
      and the LFS-vs-direct decision are all in force.

## 15. S15 — A11y/security regression + close-out  → `android-localization-accessibility`

- [ ] 15.1 Compose a11y suite (roles/state/focus/traversal/48dp/contrast-after-alpha/masked-secrecy).
- [ ] 15.2 Manual Pixel smoke (+ tablet/fold only if the S0 gate approved) + TalkBack checklist; final gate sweep; `openspec archive`
      readiness; handoff (files, results, golden artifacts, bd states).

---

## Traceability matrix (verified against code; extend during S0)

Legend: Cap = owning capability · every visible surface has exactly one owning slice.

| Surface / state | Kotlin owner (verified) | Cap | Slice |
|---|---|---|---|
| Semantic tokens | `ui/theme/Color.kt`,`Theme.kt`,`Shapes.kt`,`Type.kt`,`MotionSpec.kt` (**new**) | design-system | S1 |
| Icons | `ui/theme/NavIcons.kt`→Lucide, `contentIconFor` | iconography | S2 |
| Shared components | `ui/theme/Components.kt`,`SliderComponents.kt`,`SettingsComponents.kt` | design-system/icono | S2 |
| Appearance prefs/pickers | `Settings.kt`,`DisplayTab.kt`,`SettingsActivity.kt` | appearance | S3 |
| App shell + floating nav | `MainActivity.kt` (`MainShell`/`FloatingTabBar` private → extract) | nav-chrome | S4 |
| History list/rows/tiles | `HistoryScreen/List/Row/RowModel/Chips.kt`, `HistoryScreenState.kt` | history | S5 |
| History top/selection/filter/empty | `HistoryNormalTopBar/SelectionBar/DeviceFilter/EmptyStates.kt` | history | S5 |
| History actions/pickers/cache | `HistoryItemActions/FilePicker/ImageCache/UriHelper/UrlUtils.kt` | history | S5 |
| Full-screen preview | `PreviewOverlay/Chrome/Content/ActionRow/Gesture.kt` | preview | S6 |
| Devices list/cards/dialogs | `DevicesActivity/Screen/Controller/Dialogs/Animations/OnlineState/RevokeActions/Utils.kt`,`PeerRow.kt` | devices | S7 |
| Pairing flow | `PairActivity/Screen/QrCard.kt`,`QrHelper.kt`,`SasPairingDialog.kt`,`PairSuccessPopup.kt`,`PairedPeerList.kt`(pairing),`PairController/Provisioning/BootstrapSync/PairingApi/PairUtils/QrUtils.kt` | pairing | S8 |
| ZXing scanner | `PortraitCaptureActivity.kt` | pairing | S8 |
| Settings tabs | `SettingsActivity.kt`,`General/Display/Sync/Storage/NotificationsTab.kt`,`SettingsComponents/Composables/Utils/Types.kt` | settings | S9 |
| Onboarding/permissions | `Onboarding{Activity,Screen,Cards,Dialogs,Permissions}.kt`,`PermissionsSettingsActivity.kt`,`BackgroundCaptureSetupActivity.kt` | onboarding-perms | S10 |
| Feedback | `ui/GlassToast.kt`,`ui/SyncStatusBadge.kt` | feedback-states | S11 |
| Notifications | `ServiceNotifications.kt`,`NotificationHelper.kt`,`ClipboardService.kt`,`ServiceRestartWorker.kt` | system-surfaces | S12 |
| Invisible surfaces | `ClipboardFloatingActivity.kt`,`CaptureOverlayController.kt`,`ShareReceiverActivity.kt` | system-surfaces | S12 (preserve) |
| About / Logs | `AboutActivity.kt`,`LogViewerActivity.kt` | feedback-states | S11 |
| Localization | `res/values/strings.xml`,`res/values-uk/strings.xml`(**new**) | l10n-a11y | S13 |
| Golden infra | Paparazzi + `PreviewParameterProvider` catalog (**new**) | visual-regression | S0/S2/S4–S14 (spike→infra→per-screen baselines→audit) |

Golden lifecycle (not S14-only): **S0** spikes version/storage; **S2** establishes Paparazzi + config +
baseline policy; each owning screen slice **S4–S13** adds its surface's fixtures + baselines; **S14**
audits coverage/gaps only. Localization + a11y tests attach in the owning slice (gate established S2).

## State inventory (M3 — state-level evidence, not just files)

The **complete** state→evidence matrix (every reachable loading/empty/error/disabled/masked/dialog/
in-flight state, plus the behaviour-owner and manifest-component inventories) is in
`behavior-and-state-coverage.md` — that document is the source of truth and **S0 cannot close until it
is complete and reconciled**. Summary rows:

| Screen | States | Existing / New | Fixture · golden · test |
|---|---|---|---|
| History list | loading · populated · empty · empty-private · no-results | existing | catalog · dark+light×2acc · semantics |
| History list | error/degraded | **NEW — needs presentation-state plumbing (S5)** | catalog · 1 · semantics |
| History row | normal · selected · multi-select · pinned · too-large | existing | catalog · selected · semantics |
| History row | sensitive masked · revealed | existing | catalog · masked(synthetic) · no-plaintext-semantics |
| Preview | text/url/code/json · image loading/success/failure · file open/save-fail · large | existing | catalog · 2 · — |
| Preview | masked · revealed | **NEW — Reveal action + `revealed` state introduced (S6 owns the plumbing; a11y leak also fixed)** | catalog · masked · no-plaintext-semantics |
| Devices | own · online · offline · discovered · scanning · no-peers · reconnecting · error | existing | catalog · online+offline · semantics |
| Devices dialogs | unpair · revoke · revoke-rotate(in-flight/invalid) · revoke-error · revoke-all(in-flight) | existing | catalog · 2 · focus |
| Pairing | qr · scan-review · SAS · connecting/provisioning/bootstrap/sync · success · invalid/expired/denied/error | existing | catalog · 2 · — |
| Settings | normal · focused · disabled · dirty · saved · validation-error · destructive · loading | existing | catalog · dirty+error · focus |
| Feedback | toast(4 kinds) · banners(warn/err/info/success) · sync badge(4)+sheet · dialogs · progress | existing | catalog · per-kind · non-color-signal |
| Onboarding/perms | granted · denied · perm-denied · not-applicable · crash-detected | existing | catalog · denied+granted · intents |

New states/components introduced by this change: History **error/degraded** (S5 plumbing), Preview
**Reveal action + `revealed` state** (S6 plumbing), plus the
new components in `component-inventory.md` (shared Banner, transport/Verified/This-device pills,
SECRET lock tile, About build-id/licenses). No state is invented without its plumbing task.
