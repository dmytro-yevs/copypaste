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
states. History, Devices, and Settings are backed by the Rust runtime.

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

The Flutter client owns a shared, channel-aware update flow. macOS delegates
installation to the project Homebrew cask, while Windows and Android consume
signed GitHub Release artifacts through typed native adapters. The production
workflow builds, signs, installs, smokes, and receipt-binds all three platform
artifacts before its explicitly gated publish job can run.

The foundation check requires `COPYPASTE_FLUTTER_BUILD_TARGET` set to `macos`,
`apk`, or `windows`. It verifies the pinned dependency contract, formatting,
analysis, tests, and a debug build. Production qualification is separate and
uses Release builds only; portable checks never substitute for physical native
evidence.
