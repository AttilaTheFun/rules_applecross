#!/bin/bash
# Run a command in the rules_applecross Linux build container.
#
#   tools/linux_vm/run.sh bazel build //examples/ios/HelloWorldSwiftUI:HelloWorld
#
# The container sees this checkout at its macOS path (the podman VM mounts
# /Users), the host's Xcode read-only at /Applications/Xcode.app (the VM mounts
# it at $VM_XCODE), and keeps Bazel's caches in a named volume.
set -euo pipefail

IMAGE="${RULES_APPLECROSS_IMAGE:-localhost/rules-applecross-linux}"
VM_XCODE="${RULES_APPLECROSS_VM_XCODE:-/var/xcode/Xcode.app}"
CACHE_VOLUME="${RULES_APPLECROSS_CACHE_VOLUME:-rules-applecross-cache}"
WORKDIR="${RULES_APPLECROSS_WORKDIR:-$PWD}"

tty=()
if [ -t 0 ] && [ -t 1 ]; then tty=(-it); fi

# The VM runs on this Mac, so builds record its macOS build as
# BuildMachineOSBuild, as a build on the Mac itself would.
build_machine=()
if command -v sw_vers >/dev/null; then
  build_machine=(-e "RULES_APPLECROSS_BUILD_MACHINE_OS_BUILD=$(sw_vers -buildVersion)")
fi

exec podman run --rm ${tty[@]+"${tty[@]}"} \
  --security-opt label=disable \
  -v "$VM_XCODE:/Applications/Xcode.app:ro" \
  -v "/Users:/Users" \
  -v "$CACHE_VOLUME:/root/.cache" \
  ${build_machine[@]+"${build_machine[@]}"} \
  -w "$WORKDIR" \
  "$IMAGE" "$@"
