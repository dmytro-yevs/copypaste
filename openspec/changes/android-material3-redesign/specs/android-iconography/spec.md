## ADDED Requirements

### Requirement: Lucide as the canonical icon provider
The Android app SHALL source all in-app iconography from a single Lucide Compose icon set, rendered as 24×24 line glyphs with rounded joins/caps and `currentColor` tinting, replacing every ad hoc icon source used on app-owned surfaces.

#### Scenario: Nav and action icons resolve from Lucide
- **WHEN** a navigation, list-action, or settings icon is rendered
- **THEN** its `ImageVector` comes from the Lucide provider, not from `material-icons-extended` or a bespoke drawable

#### Scenario: Stroke and viewBox match the guide
- **WHEN** any Lucide icon is inspected
- **THEN** it uses a 24×24 viewBox and ~1.6px stroke consistent with STYLEGUIDE §8

### Requirement: Fixed box per icon role
Every rendered icon MUST be given an explicit `width`/`height` sized to its role, and the tile
CONTAINER size MUST be distinguished from the glyph BOX size, using the exact `CpDimensions` roles:
the content-type tile is a `tileSm`(32dp)/`tileMd`(36dp) container holding an 18dp glyph (`glyphBox`),
the nav glyph is 24dp (`navGlyph`) within the active pill, and the inline meta icon is 20dp (`iconMeta`). An icon never scales to an unintended intrinsic size, and the 48dp touch
target is separate from the visual icon/container dimensions.

#### Scenario: Icon renders at its role size regardless of source
- **WHEN** an icon composable is placed in a nav bar item versus a content-type tile
- **THEN** it is boxed at the nav bar's 24dp (`navGlyph`) and an 18dp glyph (`glyphBox`) inside the 32/36dp tile container respectively, never at an arbitrary intrinsic size

#### Scenario: Shared icon composable enforces an explicit size
- **WHEN** a caller invokes the shared icon composable without a size parameter
- **THEN** the call does not compile/match the composable's contract, which requires an explicit size

### Requirement: Fallback for a missing glyph
The icon provider MUST return a defined fallback glyph — never a blank composable or a crash — when a requested icon name has no mapped Lucide entry.

#### Scenario: Unknown icon name degrades gracefully
- **WHEN** a caller requests an icon key that is not present in the Lucide provider's map
- **THEN** the provider renders the fallback glyph instead of throwing or rendering nothing

### Requirement: Migration off legacy icon sources
`NavIcons.kt` and any app-owned usage of `material-icons-extended` SHALL be retired and replaced by the Lucide provider; no app-owned composable SHALL reference either after migration.

#### Scenario: NavIcons.kt no longer exists
- **WHEN** the codebase is searched for `NavIcons`
- **THEN** no reference remains outside migration history/changelog

#### Scenario: material-icons-extended is unused for app icons
- **WHEN** the codebase is searched for `androidx.compose.material.icons.extended` imports in app-owned screens
- **THEN** none are found

### Requirement: Icon semantics — informative vs decorative
An icon SHALL carry a non-null `contentDescription` only when it is informative or actionable on its own (e.g. an icon-only button, a status glyph with no adjacent label); purely decorative icons that duplicate an adjacent text label SHALL be excluded from the accessibility tree.

#### Scenario: Icon-only action button is labelled
- **WHEN** a button renders only an icon with no visible text (e.g. delete, pin)
- **THEN** it exposes a non-null `contentDescription` describing the action

#### Scenario: Decorative icon next to a label is hidden
- **WHEN** an icon sits beside a text label that already conveys the same meaning (e.g. a chevron next to "Settings")
- **THEN** the icon is marked decorative and excluded from TalkBack traversal

### Requirement: Icons render only through token colors
Icon tint SHALL come from `LocalCpColors`/`LocalAccent`/`MaterialTheme.colorScheme`, matching the content-type or semantic role it represents, and MUST NOT be a hardcoded hex value.

#### Scenario: Content-type tile glyph matches its token
- **WHEN** a history row renders a URL tile
- **THEN** the glyph color is `cUrl` at full strength and the tile background is `cUrl` at 14% alpha, per STYLEGUIDE §3.7

#### Scenario: Status icon matches its status token
- **WHEN** a banner or badge shows a warning icon
- **THEN** its tint is the `warn` token, not a literal color value
