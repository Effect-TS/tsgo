#!/usr/bin/env bash

set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
cd "$repo_root"

flake_file="flake.nix"
flake_attr=".#effect-tsgo"
build_log="$(mktemp)"
trap 'rm -f "$build_log"' EXIT
extra_args=()

if [[ -n "${NIX_BUILD_ARGS:-}" ]]; then
  # Intentionally allow shell-like splitting for simple nix CLI flags.
  # shellcheck disable=SC2206
  extra_args=(${NIX_BUILD_ARGS})
fi

run_build() {
  nix build "$flake_attr" --no-write-lock-file -L "${extra_args[@]}" >"$build_log" 2>&1
}

extract_new_hash() {
  perl -ne '
    if (/To correct the hash mismatch for effect-tsgo-workspace-gomodcache, use "([^"]+)"/) {
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
  NEW_HASH="$new_hash" perl -0pi -e 's/workspaceModuleCacheHash = (?:"[^"]+"|lib\.fakeHash);/workspaceModuleCacheHash = "$ENV{NEW_HASH}";/' "$flake_file"
}

if run_build; then
  echo "flake module cache hash is already up to date"
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
    if (/workspaceModuleCacheHash = "([^"]+)"/) {
      print "$1\n";
      exit 0;
    }

    if (/workspaceModuleCacheHash = lib\.fakeHash;/) {
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
echo "updated flake module cache hash: $current_hash -> $new_hash"

if ! run_build; then
  cat "$build_log" >&2
  echo "flake build still fails after refreshing the module cache hash" >&2
  exit 1
fi

echo "flake module cache hash refreshed successfully"
