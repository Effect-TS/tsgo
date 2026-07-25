#!/usr/bin/env bash
set -euo pipefail

# check-platform-package-bins.sh — Guards the executable bit on published binaries.
#
# pnpm pack/publish normalises every packed file to 0644 and grants 0755 only to
# files referenced by the published manifest's `bin` field; the on-disk mode is
# ignored. Each POSIX platform package is packed here with placeholder binaries
# (deliberately 0644 on disk) and the resulting tarball entries are asserted to
# be 0755, so removing `publishConfig.bin` fails CI instead of shipping a broken
# release.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BINARIES=(tsc tsc-next)

# Discovered, not hardcoded: a new POSIX platform package must not be able to
# escape the guard by being forgotten here. win32 is excluded because the mode
# bit is meaningless for .exe.
TARGETS=()
for dir in "${REPO_ROOT}"/_packages/tsgo-*/; do
  name="$(basename "${dir}")"
  name="${name#tsgo-}"
  case "${name}" in win32-*) continue ;; esac
  TARGETS+=("${name}")
done
if [ "${#TARGETS[@]}" -eq 0 ]; then
  echo "ERROR: no POSIX platform packages found under _packages/" >&2
  exit 1
fi

workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT

status=0
for target in "${TARGETS[@]}"; do
  package_dir="${workdir}/tsgo-${target}"
  mkdir -p "${package_dir}/lib"
  cp "${REPO_ROOT}/_packages/tsgo-${target}/package.json" "${package_dir}/package.json"

  for binary in "${BINARIES[@]}"; do
    printf '#!/bin/sh\nexit 0\n' > "${package_dir}/lib/${binary}"
    printf '{}\n' > "${package_dir}/lib/${binary}.json"
    chmod 0644 "${package_dir}/lib/${binary}"
  done

  if ! (cd "${package_dir}" && pnpm pack --pack-destination "${package_dir}" > /dev/null); then
    echo "  FAIL tsgo-${target}: pnpm pack failed"
    status=1
    continue
  fi
  tarball="$(find "${package_dir}" -maxdepth 1 -name '*.tgz' | head -n 1)"

  listing="$(tar -tvf "${tarball}")"
  for binary in "${BINARIES[@]}"; do
    mode="$(printf '%s\n' "${listing}" | awk -v entry="package/lib/${binary}" '$NF == entry { print $1 }')"
    if [ "${mode}" = "-rwxr-xr-x" ]; then
      echo "  ok   @effect/tsgo-${target} lib/${binary} ${mode}"
    else
      echo "  FAIL @effect/tsgo-${target} lib/${binary} ${mode:-<missing from tarball>} (expected -rwxr-xr-x)"
      status=1
    fi
  done
done

if [ "${status}" -ne 0 ]; then
  echo ""
  echo "ERROR: packed platform binaries are not executable."
  echo "Every POSIX platform package must declare publishConfig.bin for each packaged binary."
  exit 1
fi

echo ""
echo "All packed platform binaries are executable."
