#!/bin/bash
# Builds the ported Apple tools for this Linux host's CPU with this
# repository's clean-room tools in them. Run inside the build container
# (tools/linux_vm/run.sh, whose image has the build dependencies):
#
#   tools/linux_vm/run.sh tools/linux_vm/build-ported-tools.sh
#
# Builds tools/build-ported-tools.sh's archive (the ported cctools, ld64,
# libtapi and xcbuild, ported-tools-linux-<arch>.tar.xz in the workspace
# root), then adds the clean-room tools to it. With --apple-tools-only, adds
# them to the archive already there instead.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARCH="$(uname -m)"
WORK="$(mktemp -d)"
OUT="$WORK/out"
trap 'rm -rf "$WORK"' EXIT
archive="$ROOT/ported-tools-linux-$ARCH.tar.xz"
mkdir -p "$OUT"

if [ "${1:-}" != --apple-tools-only ]; then
  "$ROOT/tools/build-ported-tools.sh"
fi
tar -C "$OUT" -xf "$archive"

# The clean-room tools in this repository, replacing xcbuild's partial actool
# and the codesign, ibtool and swift-stdlib-tool placeholders. The marker
# tells the toolchain repository that this codesign signs, so it keeps it.
(cd "$ROOT/tools/apple_tools" && CARGO_TARGET_DIR="$WORK/apple_tools" cargo build -q --release --locked)
for tool in actool codesign ibtool swift-stdlib-tool; do
  install -m 755 "$WORK/apple_tools/release/$tool" "$OUT/bin/$tool"
done
# SwiftUI's macros, for the Swift compiler (with its runtime linked in, so it
# runs wherever the toolchain does).
swift build -q -c release --static-swift-stdlib \
  --package-path "$ROOT/tools/apple_tools/swiftui_macros" --scratch-path "$WORK/swiftui_macros"
install -m 755 "$WORK/swiftui_macros/release/SwiftUIMacros" "$OUT/bin/SwiftUIMacros"
git -C "$ROOT" rev-parse HEAD > "$OUT/bin/.apple-tools-revision" 2>/dev/null || echo unknown > "$OUT/bin/.apple-tools-revision"

tar -C "$OUT" -Jcf "$archive" bin lib
sha256sum "$archive"
