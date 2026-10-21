#!/bin/bash
# sw_vers for Linux execution platforms: the ported tool's answers, except the
# build version, which reports the Mac a build stands in for (rules_apple
# records it as BuildMachineOSBuild). Set by RULES_APPLECROSS_BUILD_MACHINE_OS_BUILD
# when the toolchain repository is fetched.
build="%{build_machine_os_build}"
case "${1:-}" in
  -buildVersion|--buildVersion) echo "$build"; exit 0 ;;
esac
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
if [ -z "${1:-}" ]; then
  "$here/sw_vers.ported" | sed "s/^\(BuildVersion:[[:space:]]*\).*/\1$build/"
else
  exec "$here/sw_vers.ported" "$@"
fi
