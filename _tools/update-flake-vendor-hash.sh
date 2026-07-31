#!/usr/bin/env bash
#
# Refreshes flake.lock from the upstream manifest and updates the Go vendor hash.
#
# Usage: ./_tools/update-flake-vendor-hash.sh

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

flake_file="flake.nix"
flake_attr=".#effect-tsgo"
build_log="$(mktemp)"
trap 'rm -f "$build_log"' EXIT
extra_args=()

if [[ -n "${NIX_BUILD_ARGS:-}" ]]; then
  # shellcheck disable=SC2206
  extra_args=(${NIX_BUILD_ARGS})
fi

# --- Step 1: Refresh locked inputs from upstream.json ---

tsgo_commit="$(node _tools/upstream.mjs field next tsGitHead)"
ts_commit="$(git -C typescript-go ls-tree "$tsgo_commit" _submodules/TypeScript | awk '{print $3}')"
if [[ ! "$ts_commit" =~ ^[0-9a-f]{40}$ ]]; then
  echo "error: cannot derive the TypeScript source commit from TypeScript-Go $tsgo_commit" >&2
  exit 1
fi

node --input-type=module - "$flake_file" "$tsgo_commit" "$ts_commit" <<'NODE'
import { readFileSync, writeFileSync } from "node:fs"

const [flakePath, tsgoCommit, tsCommit] = process.argv.slice(2)
let flake = readFileSync(flakePath, "utf8")

const replaceInput = (pattern, replacement, name) => {
  if (!pattern.test(flake)) {
    throw new Error(`Unable to find ${name} input in ${flakePath}`)
  }
  flake = flake.replace(pattern, replacement)
}

replaceInput(
  /github:microsoft\/typescript-go\/[0-9a-f]{40}\?submodules=1/,
  `github:microsoft/typescript-go/${tsgoCommit}?submodules=1`,
  "typescript-go-src",
)
replaceInput(
  /github:microsoft\/TypeScript\/[0-9a-f]{40}/,
  `github:microsoft/TypeScript/${tsCommit}`,
  "typescript-src",
)

writeFileSync(flakePath, flake)
NODE

nix flake lock

# --- Step 2: Refresh vendor hash ---

run_build() {
  nix build "$flake_attr" --no-write-lock-file -L "${extra_args[@]}" >"$build_log" 2>&1
}

extract_new_hash() {
  perl -ne '
    if (/To correct the hash mismatch for effect-tsgo-\S*go-modules, use "([^"]+)"/) {
      print "$1\n";
      exit 0;
    }

    if (/got:\s+(sha256-[^\s]+)/) {
      print "$1\n";
      exit 0;
    }
  ' "$build_log"
}

replace_hash() {
  local new_hash="$1"
  NEW_HASH="$new_hash" perl -0pi -e 's/vendorHash = (?:"[^"]+"|lib\.fakeHash);/vendorHash = "$ENV{NEW_HASH}";/' "$flake_file"
}

if run_build; then
  echo "flake vendor hash is already up to date"
  exit 0
fi

new_hash="$(extract_new_hash || true)"

if [[ -z "$new_hash" ]]; then
  cat "$build_log" >&2
  echo "failed to extract a replacement hash from nix build output" >&2
  exit 1
fi

current_hash="$(
  perl -ne '
    if (/vendorHash = "([^"]+)"/) {
      print "$1\n";
      exit 0;
    }

    if (/vendorHash = lib\.fakeHash;/) {
      print "lib.fakeHash\n";
      exit 0;
    }
  ' "$flake_file"
)"

if [[ "$current_hash" == "$new_hash" ]]; then
  cat "$build_log" >&2
  echo "flake hash is unchanged but the build still failed" >&2
  exit 1
fi

replace_hash "$new_hash"
echo "updated flake vendor hash: $current_hash -> $new_hash"

if ! run_build; then
  cat "$build_log" >&2
  echo "flake build still fails after refreshing the vendor hash" >&2
  exit 1
fi

echo "flake vendor hash refreshed successfully"
