#!/bin/bash
# Adds rules_applecross to a checkout of Logan's visor, from its root inside
# the build container: the module's overrides and toolchains, the Linux
# settings in .bazelrc.user, and the ported tools archive.
#
#   integrate.sh --commit SHA   rules_applecross from GitHub, at SHA
#   integrate.sh --local        this checkout of rules_applecross
#
# What it changes, and why each is needed, is in docs/consumers.md.
set -euo pipefail
SRC="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
case "${1:-}" in
  --commit) AC=""; COMMIT="$2" ;;
  --local) AC="$SRC"; COMMIT="" ;;
  *) echo "usage: integrate.sh --commit SHA | --local" >&2; exit 2 ;;
esac

mkdir -p third_party/patches/applecross
cp "$SRC"/patches/*.patch third_party/patches/applecross/
cp "$SRC/ported-tools-linux-$(uname -m).tar.xz" .

python3 - "$AC" "$COMMIT" <<'PY'
import sys
path, commit = sys.argv[1], sys.argv[2]
p = "MODULE.bazel"
s = open(p).read()
# rules_swift 4.1.0.isomer.2 carries the Swift worker fix the Linux build
# needs (rules_applecross's patches/rules_swift_worker_copy_overwrite.patch);
# visor#124 moves visor to it.
s = s.replace('''bazel_dep(name = "rules_swift", version = "4.1.0.isomer.1")''',
              '''bazel_dep(name = "rules_swift", version = "4.1.0.isomer.2")''')
assert '''bazel_dep(name = "rules_swift", version = "4.1.0.isomer.2")''' in s
old_apple = '''bazel_dep(name = "rules_apple", version = "5.1.0")'''
assert old_apple in s
s = s.replace(old_apple, '''bazel_dep(name = "rules_apple", version = "5.2.0")''')
old = '''    patches = ["//third_party/patches:rules_apple_device_xctrunner.patch"],
)'''
assert old in s
s = s.replace(old, '''    patches = [
        "//third_party/patches:rules_apple_device_xctrunner.patch",
        "//third_party/patches/applecross:rules_apple_execution_platform.patch",
        "//third_party/patches/applecross:rules_apple_bundletool_path.patch",
        "//third_party/patches/applecross:rules_apple_no_absolute_tool_paths.patch",
        "//third_party/patches/applecross:rules_apple_codesign_inputs_sdk_tools.patch",
        "//third_party/patches/applecross:rules_apple_python_stub_interpreter.patch",
    ],
    version = "5.2.0",
)''')
if path:
    override = 'local_path_override(\n    module_name = "rules_applecross",\n    path = "%s",\n)' % path
else:
    override = 'git_override(\n    module_name = "rules_applecross",\n    commit = "%s",\n    remote = "https://github.com/AttilaTheFun/rules_applecross.git",\n)' % commit
s += '''
# --- rules_applecross: the Apple apps built on Linux, with the SDKs of an
# --- Xcode mounted read-only and clean-room Apple tools.
bazel_dep(name = "rules_applecross", version = "0.0.3")
%s

single_version_override(
    module_name = "apple_support",
    patch_strip = 1,
    patches = ["//third_party/patches/applecross:apple_support_bazel8_xcode_properties.patch"],
    version = "2.8.4",
)

# rules_applecross asks for rules_swift 4.1.2, which would otherwise win over
# visor's 4.1.0.isomer.2 (isomer's patches and the worker fix).
single_version_override(
    module_name = "rules_swift",
    version = "4.1.0.isomer.2",
)

apple_cross_toolchain = use_extension("@rules_applecross//toolchain:extensions.bzl", "apple_cross_toolchain")
use_repo(apple_cross_toolchain, "apple_cross_toolchain")

register_toolchains(
    "@rules_applecross//toolchain:apple_bundling_on_linux",
    "@apple_cross_toolchain//:all",
)
''' % override
# SwiftPM evaluates manifests on the build host: on Linux, swift-nio's and
# swift-crypto's `#if canImport(Darwin)` drop their privacy manifests from
# the iOS app. Take the Darwin branch, as a Mac does.
configure = "".join('''swift_deps.configure_package(
    name = "%s",
    patch_cmds = ["sed -i.orig 's/^#if canImport(Darwin)$/#if true/' Package.swift"],
)

''' % name for name in ["swift-crypto", "swift-nio"])
anchor = "use_repo(\n    swift_deps,"
assert anchor in s
s = s.replace(anchor, configure + anchor, 1)
open(p, "w").write(s)
PY

echo "exports_files(glob([\"*.patch\"]))" > third_party/patches/applecross/BUILD.bazel

TC=external/rules_applecross++apple_cross_toolchain+apple_cross_toolchain/Xcode.app/Contents/Developer
cat > .bazelrc.user <<RC
# rules_applecross on Linux.
common --enable_platform_specific_config
common:linux --repo_env=BAZEL_NO_APPLE_CPP_TOOLCHAIN=1
common:linux --incompatible_repo_env_ignores_action_env
build:linux --xcode_version_config=@apple_cross_toolchain//:host_xcodes
# LLVM's linker: the ld64 port ignores the SDK's per-symbol \$ld\$previous\$
# entries, binding Span's symbols to libswiftCore instead of
# libswiftCompatibilitySpan for deployment targets before iOS 26.
build:linux --@rules_applecross//toolchain:linker=lld
build:linux --@rules_apple//apple:sdk_tool_files=@apple_cross_toolchain//:sdk_tool_files
build:linux --action_env=DEVELOPER_DIR=$TC
build:linux --action_env=PATH=$TC/Toolchains/XcodeDefault.xctoolchain/usr/bin:/bin:/usr/bin:/usr/local/bin
RC
