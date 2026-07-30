// swift-tools-version: 6.0
//
// CopyPaste for macOS — a menu-bar clipboard manager.
//
// Two targets, and the split is the point:
//
//   * `CopyPasteKit` — the IPC client and the observable stores. No AppKit, no
//     views. Everything here is testable without a screen.
//   * `CopyPasteApp` — the AppKit shell (status item, popover, windows, global
//     hotkey) and the SwiftUI views.
//
// The app embeds none of the Rust core. It is an IPC client that speaks
// newline-delimited JSON to the daemon over a Unix socket, exactly as
// `copypaste-cli` does; `crates/copypaste-ipc/src/lib.rs` is the contract.

import PackageDescription

let package = Package(
    name: "CopyPasteMac",
    platforms: [
        // macOS 14: `@Observable`, `ScrollView.scrollPosition(id:)` (which is
        // how the scroll-anchoring invariant is met), and `onKeyPress`.
        .macOS(.v14)
    ],
    products: [
        .library(name: "CopyPasteKit", targets: ["CopyPasteKit"]),
        .executable(name: "CopyPaste", targets: ["CopyPasteApp"]),
    ],
    dependencies: [
        // Global hotkey + the recorder control in Settings.
        //
        // CLAUDE.md rule 1: a maintained package rather than a hand-written
        // Carbon `RegisterEventHotKey` wrapper plus a bespoke key-capture view.
        // The capture rules alone (physical key, not the layout-dependent one —
        // manifest 06 INV-23) are a wheel this repository has already carved
        // once. Tradeoff stated: one third-party dependency, ~4k lines, no
        // transitive dependencies of its own, MIT.
        //
        // Every use of it is behind `GlobalHotkey.swift`.
        .package(url: "https://github.com/sindresorhus/KeyboardShortcuts", from: "2.0.0")
    ],
    targets: [
        .target(name: "CopyPasteKit"),
        .executableTarget(
            name: "CopyPasteApp",
            dependencies: [
                "CopyPasteKit",
                .product(name: "KeyboardShortcuts", package: "KeyboardShortcuts"),
            ]
        ),
        .testTarget(name: "CopyPasteKitTests", dependencies: ["CopyPasteKit"]),
    ]
)
