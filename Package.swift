// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "NormSDK",
    platforms: [.iOS(.v15), .macOS(.v12)],
    products: [
        .library(name: "NormSDK", targets: ["NormSDK"]),
    ],
    targets: [
        .binaryTarget(name: "NormSDKFFI", path: "dist/apple/NormSDKFFI.xcframework"),
        .target(name: "NormSDK", dependencies: ["NormSDKFFI"], path: "Sources/NormSDK"),
        .testTarget(name: "NormSDKTests", dependencies: ["NormSDK"], path: "Tests/NormSDKTests"),
    ]
)
