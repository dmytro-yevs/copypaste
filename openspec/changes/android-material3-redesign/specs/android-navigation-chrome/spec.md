## ADDED Requirements

### Requirement: Floating Pill Navigation Shell
The Android app SHALL render its bottom navigation as a floating pill-shaped bar hosted by a
shared shell component, replacing the private, text-only `FloatingTabBar` embedded in
`MainActivity`.

#### Scenario: Shell hosts three tabs
- **WHEN** the app displays the main navigation surface
- **THEN** the shell renders exactly three tabs — Clips, Devices, Settings — each with a Lucide
  icon and a label
- **AND** the tab bar is rendered as a single floating pill shape, not a full-width bottom bar

### Requirement: Frosted backdrop blur with a proven strategy and fallback
The navigation pill and bottom sheets SHALL apply the STYLEGUIDE §9.12 frosted treatment using a
real **backdrop** blur that samples the content rendered behind them, produced by a
backdrop-capture strategy — a `RenderNode`/`RenderEffect`-based captured-layer blur (Haze-style) on
API 31+, or a window-level blur for surfaces that are their own window. It SHALL NOT be implemented
with `Modifier.blur`/`RenderEffect` applied to the pill's own layer, which blurs the pill's children
rather than the backdrop. Foreground icons and labels SHALL be composed above the blur layer so they
are never blurred. A prototype SHALL prove the chosen strategy (performance, clipping, edge
treatment) before the design system commits to it.

#### Scenario: Backdrop blur samples content behind the pill (API 31+, translucency on)
- **WHEN** the device is API 31+ and Translucency is enabled and content scrolls behind the pill
- **THEN** the pill renders `card @ 90%` composited over a ~22px blur of the content behind it, with a
  hairline border and `--sh2`-equivalent shadow

#### Scenario: Foreground stays sharp
- **WHEN** the frosted pill is rendered with the backdrop blurred
- **THEN** the tab icons and labels remain sharp, drawn above the blur layer, never blurred with the backdrop

#### Scenario: Opaque fallback
- **WHEN** the device is below API 31, Translucency is off, or the backdrop-blur effect is not viable
- **THEN** the pill renders the canonical opaque fallback background, never a reduced-alpha-without-blur
  layer over arbitrary content

### Requirement: Navigation Insets and Placement
The navigation pill SHALL be positioned with fixed insets from the screen edges and SHALL honor
system bar, gesture, cutout, and IME insets so it never overlaps system UI or an open keyboard.

#### Scenario: Default placement
- **WHEN** the navigation pill is laid out with no IME visible
- **THEN** it is inset 12dp from the left and right screen edges
- **AND** it sits exactly 12dp (`CpDimensions.navBottomClearance`) above the resolved bottom
  system-bar/gesture inset

#### Scenario: IME visible
- **WHEN** a text input gains focus and the on-screen keyboard opens
- **THEN** the navigation pill is hidden while the IME is visible
- **AND** it does not overlap the keyboard or any focused input

#### Scenario: Display cutout present
- **WHEN** the device has a display cutout that intersects the navigation area
- **THEN** the pill's insets account for the cutout safe area
- **AND** no tab icon or label is obscured by the cutout

### Requirement: Active and Inactive Tab Styling
Each tab SHALL be styled to a single unambiguous selected state driven by the accent color, with
inactive tabs kept visually quiet.

#### Scenario: Selected tab
- **WHEN** a tab is the currently active destination
- **THEN** its icon sits inside a rounded pill filled with `accent @ 18%`
- **AND** its icon and label are rendered in the accent color

#### Scenario: Inactive tab
- **WHEN** a tab is not the currently active destination
- **THEN** its icon and label are rendered in the `--faint` token
- **AND** it shows no accent-pill background

### Requirement: Background Gradient Fade
The shell SHALL render a background gradient fade beneath the floating navigation pill so
scrolling content does not appear to cut off abruptly under the bar.

#### Scenario: Content scrolls under the nav bar
- **WHEN** scrollable content (e.g. the Clips list) extends behind the floating pill
- **THEN** a `--bg`-colored gradient fade is rendered between the content and the pill
- **AND** content visually fades out before reaching the pill rather than being hard-clipped

### Requirement: Selected Tab Restoration
The shell SHALL restore the last-selected tab via `rememberSaveable`/saved-instance-state across
configuration changes and system-initiated process death that preserves saved instance state. A
cold start with no saved instance state SHALL open the default (Clips) tab; persisting the tab
across a genuine cold start is out of scope unless explicitly added.

#### Scenario: Configuration change
- **WHEN** the device is rotated or otherwise triggers a configuration change while a
  non-default tab is selected
- **THEN** the same tab remains selected after recomposition

#### Scenario: System-initiated process death with saved state
- **WHEN** the system kills the process while a non-default tab is selected and the user returns,
  with saved instance state available
- **THEN** the shell restores the previously selected tab from saved instance state

#### Scenario: Cold start defaults to Clips
- **WHEN** the app is launched cold with no saved instance state
- **THEN** the shell opens the default Clips tab

### Requirement: Adaptive Width for Tablet and Foldable
The shell and navigation pill SHALL adapt their width responsively for tablet and foldable form
factors in portrait orientation, without introducing a landscape-specific layout.

#### Scenario: Tablet or foldable portrait width
- **WHEN** the app runs on a tablet or unfolded foldable device in portrait orientation
- **THEN** the shell content column and navigation pill widen responsively using window-size-class
  breakpoints
- **AND** the pill remains horizontally centered rather than stretching edge-to-edge

#### Scenario: Landscape is a functional fallback, not a supported layout
- **WHEN** the device is rotated to landscape orientation
- **THEN** the portrait-derived layout is reused as a functional fallback (no clipped or lost
  actions), but landscape is not a golden-tested acceptance target and no dedicated landscape
  layout is provided

### Requirement: Reduced motion disables the tab-selection spring
The shell's selected-tab transition SHALL be disabled under the system reduced-motion signal, not
merely have its duration tokens zeroed — the existing spring-based tab pop MUST resolve to an
instant state change so no residual spring animation runs.

#### Scenario: Spring pop suppressed under reduced motion
- **WHEN** the system reduced-motion signal is active and the user switches tabs
- **THEN** the selected-tab indicator changes instantly with no spring/scale animation, while still
  showing the correct accent-selected state

### Requirement: Sync Status Indicator Placement
The shell SHALL surface the sync status indicator in a fixed, unobstructed location that is never
covered by the floating navigation pill or clipped by system insets.

#### Scenario: Sync status visible alongside navigation
- **WHEN** the shell is displaying the floating navigation pill
- **THEN** the sync status indicator is rendered in a shell-owned position that does not overlap
  the pill
- **AND** the indicator respects the same system-bar/cutout insets as the rest of the shell

### Requirement: System bars and first-paint chrome follow the resolved theme
The app SHALL set status-bar and navigation-bar icon appearance from the RESOLVED app theme (Dark/
Light/System) via `WindowInsetsControllerCompat.isAppearanceLightStatusBars` /
`isAppearanceLightNavigationBars`, in addition to the preserved `SecureWindowChrome` edge-to-edge and
`FLAG_SECURE` SideEffects, and SHALL paint a canonical pre-Compose first frame (XML window background
+ Android-12 splash) with no wrong-theme flash — even when the forced app theme differs from the OS theme.

#### Scenario: Forced Light over OS Dark keeps legible bars
- **WHEN** the app theme is forced Light while the OS is Dark
- **THEN** status/nav-bar icons render dark (light-appearance bars) so they are legible over the light app surface

#### Scenario: No wrong-theme first-paint flash
- **WHEN** any themed Activity launches cold
- **THEN** the pre-Compose window background matches the resolved theme's canonical `bg`, with no flash of a non-canonical color

#### Scenario: System-bar appearance updates on committed theme change
- **WHEN** the committed appearance theme changes
- **THEN** status/nav-bar icon appearance updates to match the new resolved theme
