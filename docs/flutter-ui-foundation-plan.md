# Flutter UI foundation plan

Build one CopyPaste application foundation for macOS, Android, and Windows
before implementing product screens. The foundation covers windows, adaptive
navigation, headers, footers, route transitions, focus, and shared state
presentation.

Status on October 4, 2026: the foundation uses `shadcn_flutter: 0.0.55` and
`flutter_animate: 4.5.2`. The selected library APIs were checked in the
installed package source. All code, comments, documentation, and application
strings must be in English.

## Confirmed product decisions

| Area | Contract |
| --- | --- |
| Desktop chrome | Native macOS and Windows title bars, with a Flutter page header inside the window |
| Primary navigation | History, Devices, Settings; side navigation when wide and bottom navigation when narrow |
| Onboarding | A separate flow outside primary navigation. macOS uses Welcome → Setup → Sync; Setup offers Accessibility for auto-paste and defaults Start at login to on. Android uses Welcome → Background capture → Sync, requires a Full or Limited choice, and lets Limited users reopen Full setup from Settings. Windows onboarding remains future platform work. |
| Desktop close | Hide the window; provide Show/Open and Quit through the tray or menu bar |
| Explicit Quit | Terminate the application; closing the window does not mean Quit |
| Language | English code, comments, documentation, and application strings |
| UI library | `shadcn_flutter: 0.0.55`; use its ready-made components before introducing application UI |
| Motion library | `flutter_animate: 4.5.2`; application-owned timing, easing, and reusable effects come only from `AppMotion` |

## Ready-made UI contract

Use `ShadcnApp` with one system-aware `ThemeData` and `ThemeData.dark` pair.
Use the library's `Scaffold` for each page: `AppBar` belongs in `headers`, and
page actions belong in `footers` only when the page needs them. Use library
buttons, inputs, tooltips, dialogs, sheets, menus, and state indicators
directly.

Navigation has one destination model and controller outside widgets. Render
the same `NavigationItem` data with the component matched to available width:

| Available width in logical pixels | Component | Content |
| --- | --- | --- |
| Below 640 | `NavigationBar` | One column |
| 640 through 1023 | Compact `NavigationRail` | Flexible content region |
| 1024 and above | Expandable `NavigationRail`, expanded by default | Flexible content; forms may constrain their reading width |

`NavigationBar` and `NavigationRail` receive `children`, `selectedKey`, and
`onSelected`; every destination is a `NavigationItem` with its icon as `child`
and text as `label`. The desktop rail follows the library's expandable pattern:
`expanded`, `NavigationLabelType.expanded`, `expandedSize: 250`, and an
icon-density `Button.ghost` panel toggle. Use the library components directly
instead of recreating a sidebar.

The medium rail is compact. A new wide desktop rail starts expanded until the
user chooses its presentation. Preserve that manual choice through destination
switches and resizing while desktop navigation remains mounted. Do not add
demo profiles or fixture content to populate the navigation.

`AppShell` is permitted only as the application-specific adaptive composition
and state-preservation boundary around these components. Do not add a
`PageLayout`, reimplement a library layout or navigation primitive, or expose
a pass-through wrapper API. Custom UI is limited to application-specific
behavior that the package cannot provide. Feature logic, data access, and
shared feature state remain outside widgets.

Layout depends on available space rather than the operating system. A narrow
desktop window and a wide Android window use the same destinations, state, and
actions. Resizing preserves the selected destination, navigation state, scroll
position, and entered text. The page header contains its title, Back for a
nested route, and local actions. Long titles and enlarged text must remain
readable. Apply safe-area padding once. With the keyboard open, `Scaffold`
resizes the content and hides its footer by design. The form remains scrollable
and the focused field reachable; dismiss the keyboard before using a footer
action.

## Navigation, transitions, and state

History is the initial destination. Switching primary destinations does not add
Back history; each destination preserves state for the current session. Use
Flutter `Navigator` and `ShadcnPageRoute` for nested routes. Add a routing
dependency only for a concrete requirement Flutter APIs cannot meet.

Back first dismisses the top dismissible overlay, then a nested route. At the
Android root, allow the system to handle Back. Desktop Escape dismisses an
overlay or nested route without quitting the application. Restore focus to the
initiating control after dismissing an overlay. Tab and Shift+Tab have a
predictable order, and icon-only controls have accessible labels.

Primary destinations switch without decorative motion. Nested routes use
standard platform-compatible transitions and respect `disableAnimations` for
application-owned effects.

`AppMotion` is the only application-owned motion contract. It defines quick,
standard, and emphasized timing; restrained enter, exit, and standard curves;
and reusable fade, short-slide, and subtle-scale effects. Do not declare local
animation durations, curves, controllers, tweens, implicit animation widgets,
or effect chains. Configure public component timing with `AppMotion`; treat
fixed internal `shadcn_flutter` transitions as vendor behavior rather than
forking or recreating them. Do not add decorative motion merely because an
effect exists in the library.

When the operating system requests reduced motion, application-owned spatial
and decorative effects resolve to zero duration. Functional indeterminate
progress remains animated so users can still distinguish active work from a
stalled state.

Use one thin application-specific `StateView` for loading, empty, and error
states. It maps presentation data and optional real actions to library
primitives; controllers own retry behavior and data loading. Until product
screens exist, show honest placeholders instead of fake history, devices,
permission results, or online status.

## Platform boundaries

Initial desktop size is 1100 by 760 with a minimum of 360 by 480 logical
pixels. Restore saved bounds within an available monitor work area, accounting
for DPI and disconnected displays. If the work area is smaller, keep the window
reachable. Geometry persistence belongs to the desktop host.

Enable hiding only after a working reopen mechanism is available. A tray setup
failure must keep the window accessible and surface a recoverable error. A
narrow typed desktop-window adapter and lifecycle owner own Show, Close, and
Quit. Do not introduce a general platform service that mixes navigation, theme,
data, and permissions. Standard Flutter APIs own Android safe areas, keyboard
insets, and Back; add native code only for a missing capability.

Functional and UI parity means the same destinations, actions, states, and
outcomes on macOS, Android, and Windows, adapted to each operating system and
input method. Android does not need desktop window controls.

## Acceptance criteria

| Area | Required evidence |
| --- | --- |
| Library use | `shadcn_flutter: 0.0.55` and `flutter_animate: 4.5.2` are pinned, `shadcn_ui` and `bottom_navigator` are absent, and the shell uses `NavigationBar`, expandable `NavigationRail`, `Scaffold`, and `AppBar` directly |
| Layout | Widths 320, 360, 640, 768, 1024, 1440; breakpoint edges plus or minus one; compact and expanded rail states; short landscape window; no clipped actions, overflow, or doubled insets |
| State preservation | Resizing and destination changes preserve controller state, scroll, text, and the user's desktop rail choice; nested routes return correctly |
| Keyboard and touch | Tab/Shift+Tab, Enter/Space, Escape, Android Back; visible focus, labels for icon-only controls, usable touch targets |
| Overlays and keyboard | Back/Escape dismissal, focus containment and restoration; with the IME open, content scrolls and the focused field remains reachable; the `Scaffold` footer returns after IME dismissal |
| Appearance and accessibility | Light/dark/system, text scales 1.0/1.3/2.0, long labels, themed overlays, reduced motion |
| StateView | Loading/empty/error, meaningful retry callback, accessible semantics, one loading API |
| Desktop lifecycle | Resize/minimize/maximize, DPI, bounds restoration, Close, reopen and Quit matching the approved contract |
| Platform builds | macOS, APK, and Windows debug builds from the same integrated state; native checks reported separately |

Run the foundation gate from the repository root:

```bash
COPYPASTE_FLUTTER_BUILD_TARGET=macos ./scripts/ci/verify-flutter-foundation.sh
COPYPASTE_FLUTTER_BUILD_TARGET=apk ./scripts/ci/verify-flutter-foundation.sh
COPYPASTE_FLUTTER_BUILD_TARGET=windows ./scripts/ci/verify-flutter-foundation.sh
git diff --check
```

Widget tests establish shared layout and behavior; builds establish platform
compilation. OS window, tray/menu bar, Android system gestures, and keyboard
behavior require native evidence from the same state. Validation after the
current `shadcn_flutter` migration is pending. The earlier baseline passed
macOS and APK debug builds; Windows and native interactions are unverified.

## Reference documentation

- [Flutter adaptive layout](https://docs.flutter.dev/ui/adaptive-responsive/general): use available constraints and shared destination data for different navigation presentations.
- [Flutter predictive back](https://docs.flutter.dev/platform-integration/android/predictive-back): standard Back and `PopScope` integration.
- [shadcn_flutter 0.0.55](https://pub.dev/packages/shadcn_flutter/versions/0.0.55): pinned UI library and source of application, theme, scaffold, header, navigation, dialog, and control components.
- [flutter_animate 4.5.2](https://pub.dev/packages/flutter_animate/versions/4.5.2): pinned engine for the reusable effects exposed by `AppMotion`.
