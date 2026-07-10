## ADDED Requirements

### Requirement: Paparazzi as the Golden Screenshot Framework

Visual regression testing SHALL use Paparazzi at a pinned version proven compatible with the repo
toolchain (AGP 8.3.0 / Kotlin 1.9.23 / Compose compiler 1.5.11) to render deterministic screenshot
baselines on the JVM without a physical device or emulator, covering the committed Pixel-phone portrait
configuration (tablet and foldable configs are added ONLY if the S0 tablet/fold gate is approved). The exact Maven coordinate and version
SHALL be pinned in the version catalog, and the standard tasks `recordPaparazziDebug` /
`verifyPaparazziDebug` (or the version-appropriate equivalents) SHALL be the record/verify entry
points.

#### Scenario: Version compatibility is proven before adoption
- **WHEN** the Paparazzi plugin is introduced (S0/S1 spike)
- **THEN** a zero-production-code proof task applies the plugin and snapshots one bundled-font Compose
  fixture on the current toolchain successfully
- **AND** if no Paparazzi release is compatible, the design records whether an AGP/Kotlin/Gradle bump
  is permitted before proceeding

#### Scenario: Tests run without a device
- **WHEN** the visual regression suite executes
- **THEN** it renders and compares screenshots via Paparazzi on the JVM through `verifyPaparazziDebug`,
  requiring no connected device or emulator

#### Scenario: Committed form factor, conditional wider screens
- **WHEN** a screen has golden coverage
- **THEN** it is rendered on the committed Pixel-phone portrait config; tablet and fold configs are
  added only if the S0 tablet/fold gate is approved

### Requirement: Central Preview Catalog

A single central preview catalog SHALL supply the fixtures used for golden rendering, built on
`PreviewParameterProvider` (or an equivalent shared fixture mechanism), rather than each
composable declaring its own duplicated `@Preview` annotations.

#### Scenario: New composable added
- **WHEN** a themed composable is added and needs golden coverage
- **THEN** its fixtures are registered in the central preview catalog rather than as bespoke
  per-composable `@Preview` functions

### Requirement: Representative Fixture Matrix Without Full Cross-Product

Golden fixtures SHALL cover dark and light themes, at least two accents rendered visually,
English and Ukrainian for text-heavy screens, the normal/loading/empty/error/disabled/selected
states applicable to a screen, a masked-sensitive fixture using a synthetic placeholder, font
scales 1.0 and 2.0, and translucent/solid variants where blur is deterministic — and SHALL NOT
render the full theme × accent × translucency × locale × state cross-product as screenshots; the
remaining accent and theme combinations are covered by token/contrast tests instead.

#### Scenario: Baseline fixture set for a screen
- **WHEN** golden fixtures are defined for a themed screen
- **THEN** they include dark and light themes, at least two accents, EN and UK for text-heavy
  content, and that screen's applicable states from normal/loading/empty/error/disabled/selected

#### Scenario: Masked-sensitive fixture uses a placeholder
- **WHEN** a fixture renders sensitive/masked content
- **THEN** it uses a synthetic placeholder value and never a real secret

#### Scenario: Font-scale fixtures
- **WHEN** font-scale fixtures are captured
- **THEN** both 1.0 and 2.0 scale are rendered

#### Scenario: No full cross-product is generated
- **WHEN** the full combination of 3 themes × 6 accents × 2 translucency states × 2 locales × all
  states is considered
- **THEN** it is not rendered as a screenshot matrix; color/contrast coverage for the remaining
  accents and themes comes from token-level contrast tests, not additional goldens

### Requirement: Deterministic Golden Rendering Configuration

Every golden render SHALL fix device, API level, orientation, navigation mode, locale, timezone,
clock, animation clock, fonts, and fake data, and the translucency/blur policy SHALL be
injectable or overridable so blurred surfaces render deterministically rather than sampling live
content.

#### Scenario: Repeated runs match within the pinned threshold
- **WHEN** the same fixture is rendered twice on unchanged code in the pinned environment
- **THEN** the outputs match within the configured diff threshold (which MAY be 0 for exact equality),
  with no variance from wall-clock time, live fonts, or animation state

#### Scenario: Blur is deterministic
- **WHEN** a fixture has translucency/blur enabled
- **THEN** the blur implementation is injected or overridden with a deterministic stand-in for
  the golden render, not a live `RenderEffect` sample of arbitrary content

### Requirement: Baseline Directory, Record, and Verify Commands

The framework SHALL define a baseline directory and file-naming convention, a record command
that (re)generates baselines and a verify command that compares against them with a defined diff
threshold, and CI SHALL capture failing-comparison images as build artifacts. The threshold SHALL
record its metric, per-pixel tolerance, allowed differing-pixel percentage, alpha handling, and image
dimensions; the default target is pixel-level (0% differing pixels) and any nonzero tolerance requires
named-owner approval. The design SHALL
also state whether baseline PNGs are stored directly in git or via Git LFS, with the expected
per-baseline size and total repository-cost estimate.

#### Scenario: Recording a new baseline
- **WHEN** a developer intentionally changes a themed surface and runs the record command
- **THEN** the affected baseline images are written to the baseline directory under the
  established naming convention

#### Scenario: CI verify failure produces artifacts
- **WHEN** CI runs the verify command and a rendered output differs from its baseline beyond the
  diff threshold
- **THEN** the build fails and the failing/diff images are attached as CI artifacts

### Requirement: Baseline Update Approval Gate

A changed baseline SHALL never be auto-accepted; merging a baseline change SHALL require explicit
owner approval as part of code review.

#### Scenario: Baseline diff appears in a PR
- **WHEN** a pull request includes regenerated baseline images
- **THEN** the diff is visible for review and the PR cannot merge without an owner explicitly
  approving the new baselines

#### Scenario: No auto-accept in CI
- **WHEN** CI detects a baseline mismatch
- **THEN** it fails the build rather than silently accepting or overwriting the baseline
