#!/bin/bash
# Clones Visor (github.com/AttilaTheFun/visor) inside the Linux VM, adds rules_applecross to it and
# builds the iOS app there for the arm64 simulator (a device build needs the
# team's provisioning profiles, which only the Mac has).
#
#   tools/linux_vm/build-visor.sh [--local] [VISOR_REF]
#
# By default rules_applecross comes from GitHub at this checkout's HEAD, which
# must be pushed; --local uses this checkout. The checkout lives in the VM's
# visor-src volume, not on the Mac. The .ipa is copied to .compare/visor-linux.ipa.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
mode=(--commit "$(git -C "$ROOT" rev-parse HEAD)")
if [ "${1:-}" = --local ]; then mode=(--local); shift; fi
REF="${1:-main}"
mkdir -p "$ROOT/.compare"

podman volume exists visor-src || podman volume create visor-src >/dev/null
export RULES_APPLECROSS_VOLUMES="visor-src:/src" RULES_APPLECROSS_WORKDIR=/src
"$ROOT/tools/linux_vm/run.sh" bash -euc "
  rm -rf /src/visor
  git clone -q https://github.com/AttilaTheFun/visor.git /src/visor
  cd /src/visor
  git checkout -q '$REF'
  git log --oneline -1
  '$ROOT/tools/linux_vm/visor/integrate.sh' ${mode[*]}
  bazel build --ios_multi_cpus=sim_arm64 //applications/visor_ios:visor_ios
  cp -fL bazel-bin/applications/visor_ios/visor_ios.ipa '$ROOT/.compare/visor-linux.ipa'
"
shasum -a 256 "$ROOT/.compare/visor-linux.ipa"
