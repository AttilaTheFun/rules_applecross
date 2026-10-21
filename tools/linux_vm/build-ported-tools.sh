#!/bin/bash
# Build the ported Apple tools for this Linux host's architecture.
#
# The same tools, at the same revisions, as apple-cross-toolchain/ci's
# tools/build.sh, which only publishes x86_64. Run inside the build container
# (tools/linux_vm/run.sh, whose image already has the build dependencies):
#
#   tools/linux_vm/run.sh tools/linux_vm/build-ported-tools.sh
#
# Writes ported-tools-linux-<arch>.tar.xz to the workspace root. With
# --apple-tools-only, rebuilds only this repository's clean-room tools into
# the archive already there.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ARCH="$(uname -m)"
WORK="$(mktemp -d)"
OUT="$WORK/out"
trap 'rm -rf "$WORK"' EXIT
archive="$ROOT/ported-tools-linux-$ARCH.tar.xz"
mkdir -p "$OUT/lib" "$OUT/bin"
cd "$WORK"

# The tools ported from Apple's open-source releases.
build_ported() {
  git clone -q https://github.com/apple-cross-toolchain/xcbuild-rust.git
  (cd xcbuild-rust && git checkout -q aeef77013f26d89d7b19247ccc3f74b6bdb29929 \
    && cargo build -q --release \
    && find target/release -maxdepth 1 -type f -executable -exec cp {} "$OUT/bin" \;)

  curl -fsSL https://sourceforge.net/projects/pmt/files/pngcrush/1.8.13/pngcrush-1.8.13.tar.gz/download -o pngcrush.tar.gz
  tar -xf pngcrush.tar.gz
  (cd pngcrush-1.8.13 && make -s -j"$(nproc)" && mv pngcrush "$OUT/bin")

  git clone -q https://github.com/tpoechtrager/apple-libtapi.git
  (cd apple-libtapi && git checkout -q 664b8414f89612f2dfd35a9b679c345aa5389026 \
    && INSTALLPREFIX="$OUT" ./build.sh >/dev/null && ./install.sh >/dev/null)

  git clone -q https://github.com/tpoechtrager/cctools-port.git
  (cd cctools-port/cctools && git checkout -q faa1f24cb7e31be132f98f503cc447c90ce2fd87 \
    && ./configure -q --prefix="$OUT" --with-libtapi="$OUT" \
    && make -s -j"$(nproc)" && make -s install)
  patchelf --set-rpath '$ORIGIN/../lib' "$OUT/bin/ld"
  rm -f "$OUT/bin/dsymutil" "$OUT/bin/nm"
}

if [ "${1:-}" = --apple-tools-only ]; then
  tar -C "$OUT" -xf "$archive"
else
  build_ported
fi

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
