# Flutter foundation

The replacement client lives at `apps/copypaste_flutter`.

- Flutter SDK: `.flutter-version`
- Product version: root `Cargo.toml` `workspace.package.version`
- UI dependency: `shadcn_flutter: 0.0.55`
- Android targets: ARM32, ARM64, and x64; x86 is retired

The foundation uses the ready-made `shadcn_flutter` application, theme,
scaffold, header, footer, and navigation components. CopyPaste adds only the
application-specific coordination and platform boundaries the package cannot
provide: `AppShell` preserves state while selecting an adaptive navigation
component, the navigation controller, the desktop lifecycle owner, and one
thin `StateView` mapping library primitives to loading, empty, and error
states. History, Devices, and Settings remain honest placeholders until their
product work begins.

`ShadcnApp` owns application theming with `ThemeData` and `ThemeData.dark`.
Each page uses `Scaffold` with `AppBar` in `headers` and page actions in
`footers` when needed. Navigation uses `NavigationBar` below 640 logical
pixels and the library's expandable `NavigationRail` at desktop widths. The
rail is compact from 640 through 1023 and starts expanded at 1024 and above;
a `Button.ghost` with `ButtonStyle.ghostIcon` lets the user choose either
desktop presentation.
Every destination is a `NavigationItem`. All application UI icons use
`LucideIcons` from `shadcn_flutter` through component public APIs.

macOS and Windows use native title bars. Closing the window hides it after the
tray is ready; Show CopyPaste reopens it and Quit CopyPaste exits. Desktop
bounds are restored within an available display. Android uses the same shell
with system safe areas, keyboard insets, and predictive-back support.

The Rust bridge, clipboard features, updater, and release packaging are still
pending. `release.yml` remains intentionally blocked until they are implemented
and qualified. See [the implementation plan](flutter-ui-foundation-plan.md).

The foundation check requires `COPYPASTE_FLUTTER_BUILD_TARGET` set to `macos`,
`apk`, or `windows`. It verifies the pinned dependency contract, formatting,
analysis, tests, and a debug build. The prior baseline passed macOS and APK
debug builds. Validation after the `shadcn_flutter` migration remains pending;
Windows and native interaction evidence are also pending.
