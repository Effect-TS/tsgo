#!/usr/bin/env bash
#
# Syncs flake.nix inputs with the git submodule commits (source of truth)
# and refreshes the Go vendor hash.
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

# --- Step 1: Sync flake inputs with submodule commits ---

tsgo_commit="$(git ls-tree HEAD typescript-go | awk '{print $3}')"
if [[ -z "$tsgo_commit" ]]; then
  echo "error: cannot determine typescript-go submodule commit" >&2
  exit 1
fi

ts_commit="$(git -C typescript-go ls-tree HEAD _submodules/TypeScript | awk '{print $3}')"
if [[ -z "$ts_commit" ]]; then
  echo "error: cannot determine TypeScript submodule commit (is the submodule initialized?)" >&2
  exit 1
fi

update_input_commit() {
  local input_name="$1"
  local new_commit="$2"
  local repo_url="$3"
  local suffix="${4:-}"

  local current
  current="$(perl -ne "
    if (/\"github:${repo_url}\\/([a-f0-9]+)${suffix}\"/) {
      print \"\$1\\n\";
      exit 0;
    }
  " "$flake_file")"

  if [[ "$current" == "$new_commit" ]]; then
    echo "$input_name: already at $new_commit"
  else
    local escaped_suffix
    escaped_suffix="$(printf '%s' "$suffix" | sed 's/[?]/\\?/g')"
    perl -pi -e "s|github:${repo_url}/[a-f0-9]+${escaped_suffix}|github:${repo_url}/${new_commit}${suffix}|" "$flake_file"
    echo "$input_name: updated ${current:-unknown} -> $new_commit"
  fi
}

update_input_commit "typescript-go-src" "$tsgo_commit" "microsoft/typescript-go" '?submodules=1'
update_input_commit "typescript-src" "$ts_commit" "microsoft/TypeScript"

# --- Step 2: Refresh vendor hash ---

run_build() {
  nix build "$flake_attr" --no-write-lock-file -L "${extra_args[@]}" >"$build_log" 2>&1
}

extract_new_hash() {
  perl -ne '
    if (/To correct the hash mismatch for effect-tsgo-go-vendor, use "([^"]+)"/) {
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
