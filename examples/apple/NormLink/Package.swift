// swift-tools-version:5.9
import Foundation
import PackageDescription

// Command-line tools need an embedded Info.plist for Bluetooth permission.
let cliInfoPlist = URL(fileURLWithPath: #filePath)
    .deletingLastPathComponent()
    .appendingPathComponent("Sources/NormConsoleCLI/Info.plist").path

let package = Package(
    name: "NormLink",
    platforms: [.iOS(.v15), .macOS(.v12)],
    products: [
        .library(name: "NormLink", targets: ["NormLink"]),
        .executable(name: "normconsole", targets: ["NormConsoleCLI"]),
    ],
    dependencies: [
        .package(name: "NormSDK", path: "../../.."),
    ],
    targets: [
        .target(name: "NormLink", dependencies: [.product(name: "NormSDK", package: "NormSDK")]),
        .executableTarget(
            name: "NormConsoleCLI",
            dependencies: ["NormLink"],
            exclude: ["Info.plist"],
            linkerSettings: [.unsafeFlags(["-Xlinker", "-sectcreate", "-Xlinker", "__TEXT",
                                           "-Xlinker", "__info_plist", "-Xlinker", cliInfoPlist])]
        ),
        .testTarget(name: "NormLinkTests", dependencies: ["NormLink"]),
    ]
)
