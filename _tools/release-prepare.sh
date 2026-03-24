#!/usr/bin/env bash
set -euo pipefail

# release-prepare.sh — Single entrypoint for preparing publish-ready workspace packages.
#
# Usage: release-prepare.sh [--target <platform-arch> ...] [--skip-cli]
#   --target    Select specific platform targets (repeatable). Omit to build all.
#   --skip-cli  Skip the CLI bundle build and its validation. Used by matrix build jobs.
#
# Steps:
#   1. Cross-compile tsgo binaries for selected platforms directly into _packages
#   2. Build CLI bundle
#   Followed by artifact validation.

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PACKAGES_DIR="${REPO_ROOT}/_packages"

# Unified target matrix: goos goarch goarm npm_platform npm_arch binary_name
# Build order: Darwin first, then Windows, then Linux
ALL_TARGETS=(
  "darwin  arm64 - darwin arm64 tsgo"
  "darwin  amd64 - darwin x64   tsgo"
  "windows amd64 - win32  x64   tsgo.exe"
  "windows arm64 - win32  arm64 tsgo.exe"
  "linux   amd64 - linux  x64   tsgo"
  "linux   arm64 - linux  arm64 tsgo"
  "linux   arm   6 linux  arm   tsgo"
)

# Build list of valid target identifiers from the matrix
valid_ids=()
for target in "${ALL_TARGETS[@]}"; do
  read -r _goos _goarch _goarm npm_platform npm_arch _binary <<< "$target"
  valid_ids+=("${npm_platform}-${npm_arch}")
done

# ── Argument parsing ──────────────────────────────────────────────────────────
selected_ids=()
skip_cli=false
while [ $# -gt 0 ]; do
  case "$1" in
    --target)
      if [ $# -lt 2 ]; then
        echo "ERROR: --target requires a value (e.g., --target linux-x64)."
        echo "Valid targets: ${valid_ids[*]}"
        exit 1
      fi
      selected_ids+=("$2")
      shift 2
      ;;
    --skip-cli)
      skip_cli=true
      shift 1
      ;;
    *)
      echo "ERROR: Unknown flag '$1'."
      echo "Usage: release-prepare.sh [--target <platform-arch> ...] [--skip-cli]"
      echo "Valid targets: ${valid_ids[*]}"
      exit 1
      ;;
  esac
done

# ── Target validation & filtering ─────────────────────────────────────────────
if [ ${#selected_ids[@]} -gt 0 ]; then
  for id in "${selected_ids[@]}"; do
    found=0
    for valid in "${valid_ids[@]}"; do
      if [ "$id" = "$valid" ]; then
        found=1
        break
      fi
    done
    if [ "$found" -eq 0 ]; then
      echo "ERROR: Invalid target '${id}'."
      echo "Valid targets: ${valid_ids[*]}"
      exit 1
    fi
  done

  # Filter ALL_TARGETS to only selected entries
  TARGETS=()
  for target in "${ALL_TARGETS[@]}"; do
    read -r _goos _goarch _goarm npm_platform npm_arch _binary <<< "$target"
    tid="${npm_platform}-${npm_arch}"
    for id in "${selected_ids[@]}"; do
      if [ "$tid" = "$id" ]; then
        TARGETS+=("$target")
        break
      fi
    done
  done
else
  TARGETS=("${ALL_TARGETS[@]}")
fi

# ── Step 1: Cross-compile ─────────────────────────────────────────────────────
if [ "$skip_cli" = "true" ]; then
  total_steps=1
else
  total_steps=2
fi
echo "==> Step 1/${total_steps}: Cross-compiling ${#TARGETS[@]} target(s) sequentially"

for target in "${TARGETS[@]}"; do
  read -r goos goarch goarm npm_platform npm_arch binary_name <<< "$target"

  dest_dir="${PACKAGES_DIR}/tsgo-${npm_platform}-${npm_arch}/lib"
  dest="${dest_dir}/${binary_name}"

  echo "  Building ${goos}/${goarch} -> ${dest}"
  mkdir -p "${dest_dir}"

  export CGO_ENABLED=0
  export GOOS="$goos"
  export GOARCH="$goarch"
  if [ "$goarm" != "-" ]; then
    export GOARM="$goarm"
  else
    unset GOARM
  fi
  go build -ldflags="-s -w" -o "${dest}" ./typescript-go/cmd/tsgo
done

echo "  Cross-compilation complete."

# ── Step 2: Build CLI bundle ─────────────────────────────────────────────────
if [ "$skip_cli" != "true" ]; then
  echo "==> Step 2/${total_steps}: Building CLI bundle"
  pnpm build:cli
fi

# ── Validation ────────────────────────────────────────────────────────────────
echo "  Validating artifacts..."
errors=()

# Check CLI bundle
if [ "$skip_cli" != "true" ]; then
  cli_bundle="${PACKAGES_DIR}/tsgo/dist/effect-tsgo"
  if [ ! -s "${cli_bundle}" ]; then
    errors+=("Missing or empty CLI bundle: ${cli_bundle}")
  fi
fi

# Check platform binaries
for target in "${TARGETS[@]}"; do
  read -r _goos _goarch _goarm npm_platform npm_arch binary_name <<< "$target"
  bin_path="${PACKAGES_DIR}/tsgo-${npm_platform}-${npm_arch}/lib/${binary_name}"
  if [ ! -s "${bin_path}" ]; then
    errors+=("Missing or empty binary: ${bin_path}")
  fi
done

if [ ${#errors[@]} -gt 0 ]; then
  echo ""
  echo "ERROR: Validation failed. Missing artifacts:"
  for err in "${errors[@]}"; do
    echo "  - ${err}"
  done
  exit 1
fi

echo ""
echo "release:prepare complete. All artifacts validated."
