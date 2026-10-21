load("@build_bazel_rules_apple//apple:ios.bzl", "ios_application")
load("@build_bazel_rules_swift//swift:swift.bzl", "swift_library")

package(default_visibility = ["//visibility:public"])

# Built twice and compared byte for byte: on Linux with this repository's
# clean-room actool, ibtool and codesign (tools/apple_tools), and on a Mac
# with Xcode's. See tools/compare_ipa.sh.
swift_library(
    name = "CleanRoomLib",
    srcs = glob(["Sources/*.swift"]),
    linkopts = ["-Wl,-framework,SwiftUI"],
    module_name = "CleanRoom",
    tags = ["manual"],
)

ios_application(
    name = "CleanRoom",
    app_icons = glob(["Resources/Assets.xcassets/AppIcon.appiconset/**"]),
    bundle_id = "com.example.CleanRoom",
    families = [
        "iphone",
        "ipad",
    ],
    infoplists = ["Sources/Info.plist"],
    launch_storyboard = "Resources/LaunchScreen.storyboard",
    minimum_os_version = "18.0",
    # A synthetic profile signed by a throwaway certificate: device builds
    # need one, and ad hoc signing (--ios_signing_cert_name=-) only embeds it.
    provisioning_profile = "CleanRoom.mobileprovision",
    deps = [":CleanRoomLib"],
)
