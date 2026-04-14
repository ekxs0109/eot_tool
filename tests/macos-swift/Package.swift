// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "FonttoolCoreTextProbe",
    platforms: [
        .macOS(.v13),
    ],
    products: [
        .executable(
            name: "FonttoolCoreTextProbe",
            targets: ["FonttoolCoreTextProbe"]
        ),
    ],
    targets: [
        .executableTarget(
            name: "FonttoolCoreTextProbe",
            path: "Sources"
        ),
    ]
)
