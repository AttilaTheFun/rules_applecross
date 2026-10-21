# Using rules_applecross from another module

What a root module needs beyond the [Setup](../README.md#setup) steps to build
its Apple apps on Linux with the clean-room tools, as established by building
Logan's Visor app (`tools/linux_vm/build-visor.sh`, which applies all of it
through `tools/linux_vm/visor/integrate.sh`).

## Overrides only the root module can make

Repeat this repository's `rules_apple` and `apple_support` overrides and copy
`patches/`, as the [Setup](../README.md#setup) says. Before rules_swift 4.2.0
the Swift worker fails incremental rebuilds on Linux with `Could not copy ...
(File exists)` (fixed by rules_swift#1905), so use 4.2.0 or later. Visor pins
4.1.0.isomer.2 from its own registry, which carries a backport, with a
`single_version_override`, because rules_applecross asks for 4.2.2, which
would otherwise be chosen.

## Toolchains

```starlark
apple_cross_toolchain = use_extension("@rules_applecross//toolchain:extensions.bzl", "apple_cross_toolchain")
use_repo(apple_cross_toolchain, "apple_cross_toolchain")

register_toolchains(
    "@rules_applecross//toolchain:apple_bundling_on_linux",
    "@apple_cross_toolchain//:all",
)
```

rules_applecross's own `configure` and `swift` tags supply the toolchain
unless the root configures one of the same name. The Swift toolchains come
from that extension, not rules_swift's `swift` extension: rules_swift's
extension serves only the root module, and a dependency using it would
take over the root's toolchain.

The ported tools archive (`tools/linux_vm/build-ported-tools.sh`) is read from
the root module's workspace: copy `ported-tools-linux-<arch>.tar.xz` there.

## .bazelrc

The Setup's settings, for Linux hosts only, plus the linker:

```
startup --host_jvm_args=-DBAZEL_TRACK_SOURCE_DIRECTORIES=1
common --enable_platform_specific_config
common:linux --repo_env=BAZEL_NO_APPLE_CPP_TOOLCHAIN=1
common:linux --incompatible_repo_env_ignores_action_env
build:linux --xcode_version_config=@apple_cross_toolchain//:host_xcodes
build:linux --@rules_applecross//toolchain:linker=lld
build:linux --@rules_apple//apple:sdk_tool_files=@apple_cross_toolchain//:sdk_tool_files
build:linux --action_env=DEVELOPER_DIR=external/rules_applecross++apple_cross_toolchain+apple_cross_toolchain/Xcode.app/Contents/Developer
build:linux --action_env=PATH=external/rules_applecross++apple_cross_toolchain+apple_cross_toolchain/Xcode.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin:/bin:/usr/bin:/usr/local/bin
```

The canonical repository name in the two paths is the one Bazel gives the
extension's repository when rules_applecross is a dependency.

Use LLVM's linker. The ported ld64 ignores the per-symbol `$ld$previous$`
entries in the SDK's `libswiftCore.tbd`, which move `Span`'s symbols to
`@rpath/libswiftCompatibilitySpan.dylib` for deployment targets before the 26
releases. An app linked with it binds them to `libswiftCore` and cannot load
on iOS 18.

## Swift packages

rules_swift_package_manager runs `swift package` to read package manifests,
so the build host needs Swift on `PATH` (the build image is based on
`swift:6.4-noble`). Manifests are evaluated on the host, as SwiftPM always
does, so `#if canImport(Darwin)` in a manifest is false on Linux. swift-nio
and swift-crypto use it to add their privacy manifests, which would then be
missing from the app. Visor patches those manifests to take the Darwin branch:

```starlark
swift_deps.configure_package(
    name = "swift-nio",
    patch_cmds = ["sed -i.orig 's/^#if canImport(Darwin)$/#if true/' Package.swift"],
)
```

## SwiftUI's macros

Since the iOS 27 SDK, `@State` is a macro implemented by Xcode's
`SwiftUIMacros` plugin, a macOS dylib. On Linux the toolchain loads the
clean-room plugin from the ported tools instead
([clean-room/swiftui-macros.md](clean-room/swiftui-macros.md)). It implements
`@State` and the macros its expansion uses; `@Entry`, `@Animatable` and
`#Preview` are not implemented, and code using them fails to compile on Linux.

## What differs from an Xcode build

Built for the arm64 simulator, Visor's Linux app has the same files as the
Xcode build, and Apple's `codesign --verify --deep --strict` accepts it. It
launches and runs in the iOS 18.4 and 27.0 simulators. Byte for byte:

- the bundled `libswiftCompatibilitySpan.dylib` and the privacy manifests are
  identical;
- the executables differ: Xcode's Swift compiler against swift.org's;
- `Assets.car` and the icon PNGs differ: Visor's icon is an Icon Composer
  document, which actool renders flat
  ([clean-room/actool.md](clean-room/actool.md#icon-composer-documents));
- the property lists hold the same values with keys in a different order:
  Visor's own Mac build uses rules_apple 5.1 and its own plist tooling.

A device build needs the team's provisioning profiles, which the Linux VM does
not have.
