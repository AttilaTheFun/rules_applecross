// swift-tools-version: 6.0

// A clean-room implementation of the macros SwiftUI declares with
// `#externalMacro(module: "SwiftUIMacros", ...)`, for compiling SwiftUI code
// on Linux, where Xcode's plugin (a macOS dylib) cannot load. The expansions
// reproduce the ones Xcode's compiler dumps (docs/clean-room/swiftui-macros.md).
import PackageDescription

let package = Package(
    name: "swiftui_macros",
    platforms: [.macOS(.v13)],
    dependencies: [
        .package(url: "https://github.com/swiftlang/swift-syntax.git", exact: "604.0.0"),
    ],
    targets: [
        .executableTarget(
            name: "SwiftUIMacros",
            dependencies: [
                .product(name: "SwiftCompilerPlugin", package: "swift-syntax"),
                .product(name: "SwiftSyntax", package: "swift-syntax"),
                .product(name: "SwiftSyntaxMacros", package: "swift-syntax"),
            ]
        ),
        .testTarget(
            name: "SwiftUIMacrosTests",
            dependencies: [
                "SwiftUIMacros",
                .product(name: "SwiftSyntaxMacrosTestSupport", package: "swift-syntax"),
            ]
        ),
    ]
)
