// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "hotkey_manager_macos",
    platforms: [
        .macOS("10.15")
    ],
    products: [
        .library(name: "hotkey-manager-macos", targets: ["hotkey_manager_macos"])
    ],
    dependencies: [
        .package(name: "FlutterFramework", path: "../FlutterFramework")
    ],
    targets: [
        .target(
            name: "hotkey_manager_macos",
            dependencies: [
                .product(name: "FlutterFramework", package: "FlutterFramework")
            ],
            path: "Classes"
        )
    ]
)
