# hotkey_manager_macos

This package is a local Swift Package Manager-compatible patch of
`hotkey_manager_macos` 0.2.0. It preserves the upstream Dart and native plugin
contract while Flutter transitions macOS plugins from CocoaPods to SwiftPM.

The package layout and `Package.swift` follow the upstream implementation from
leanflutter/hotkey_manager#71. Remove this override after an equivalent release
is available on pub.dev.
