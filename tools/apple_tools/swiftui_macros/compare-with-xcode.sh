#!/bin/bash
# On a Mac with Xcode: type-checks each file in probes/ against the iOS
# simulator SDK twice, with Apple's SwiftUIMacros and with this one, and
# compares the macro expansions the compiler prints.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
swift build -q -c release
plugin="$PWD/.build/release/SwiftUIMacros#SwiftUIMacros"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT
status=0
for probe in probes/*.swift; do
  name="$(basename "$probe" .swift)"
  # The expansions name the file's own module, so both runs compile it alike.
  check=(xcrun --sdk iphonesimulator swiftc -target arm64-apple-ios18.0-simulator
         -typecheck -module-name Probe -Xfrontend -dump-macro-expansions "$probe")
  "${check[@]}" > "$out/$name.xcode" 2>&1
  "${check[@]}" -load-plugin-executable "$plugin" > "$out/$name.clean-room" 2>&1
  if cmp -s "$out/$name.xcode" "$out/$name.clean-room"; then
    echo "identical: $probe"
  else
    echo "DIFFERENT: $probe"
    diff "$out/$name.xcode" "$out/$name.clean-room" || true
    status=1
  fi
done
exit $status
