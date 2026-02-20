// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "TerminalApp",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(
            name: "TerminalApp",
            path: "Sources",
            linkerSettings: [
                .unsafeFlags([
                    "-L../../target/release",
                    "-llibterm",
                ]),
            ]
        ),
    ]
)
