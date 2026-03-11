#!/usr/bin/env bash
set -euo pipefail

CI_MODE=false
if [ "${1:-}" = "--ci" ]; then
  CI_MODE=true
fi

if [ "$CI_MODE" = false ]; then
  # Keep submodule config in sync before updating.
  git submodule sync --recursive

  # Clear stale git index locks that can break recursive submodule updates.
  rm -f .git/modules/typescript-go/modules/_submodules/TypeScript/index.lock
  rm -f typescript-go/.git/modules/_submodules/TypeScript/index.lock

  # Attempt a full recursive update first.
  if ! git submodule update --init --recursive --force; then
    # Repair the nested TypeScript submodule if its pinned commit isn't present locally.
    if [ -d typescript-go ]; then
      git -C typescript-go submodule sync --recursive
      git -C typescript-go submodule update --init --force _submodules/TypeScript || true

      ts_commit="$(git -C typescript-go ls-tree HEAD _submodules/TypeScript | awk '{print $3}')"
      if [ -n "$ts_commit" ]; then
        git -C typescript-go/_submodules/TypeScript fetch --depth 1 origin "$ts_commit"
        git -C typescript-go/_submodules/TypeScript checkout --detach "$ts_commit"
        git -C typescript-go/_submodules/TypeScript reset --hard "$ts_commit"
      fi
    fi

    # Retry after repairing.
    git submodule update --init --recursive --force
  fi

  git submodule foreach --recursive 'git reset --hard HEAD && git clean -fdx'
fi

# Apply patches in order (glob sorts alphabetically)
# Using plain 'git apply' without --3way for non-interactive CI mode
# --3way can invoke merge tools interactively on conflicts
for patch in _patches/*.patch; do
    if [ -f "$patch" ]; then
        echo "Applying patch: $patch"
        git -C typescript-go apply "../$patch"
    fi
done

echo "All patches applied successfully"

if [ "$CI_MODE" = false ]; then
  ensure_effect_smol_reference_repo() {
    local repo_dir=".repos/effect-smol"
    local repo_url="https://github.com/Effect-TS/effect-smol"

    mkdir -p .repos

    if [ -d "$repo_dir/.git" ]; then
      echo "Updating reference repo: $repo_dir"
      git -C "$repo_dir" remote set-url origin "$repo_url"
      git -C "$repo_dir" fetch --prune origin
      return
    fi

    if [ -e "$repo_dir" ]; then
      echo "Error: $repo_dir exists but is not a git repository" >&2
      exit 1
    fi

    echo "Cloning reference repo: $repo_url -> $repo_dir"
    git clone --origin origin "$repo_url" "$repo_dir"
  }

  ensure_effect_smol_reference_repo

  ensure_effect_v3_reference_repo() {
    local repo_dir=".repos/effect-v3"
    local repo_url="https://github.com/Effect-TS/effect"

    mkdir -p .repos

    if [ -d "$repo_dir/.git" ]; then
      echo "Reference repo already present (one-time clone): $repo_dir"
      return
    fi

    if [ -e "$repo_dir" ]; then
      echo "Error: $repo_dir exists but is not a git repository" >&2
      exit 1
    fi

    echo "Cloning reference repo (one-time): $repo_url -> $repo_dir"
    git clone --origin origin "$repo_url" "$repo_dir"
  }

  ensure_effect_v3_reference_repo

  ensure_effect_language_service_reference_repo() {
    local repo_dir=".repos/effect-language-service"
    local repo_url="https://github.com/Effect-TS/language-service"

    mkdir -p .repos

    if [ -d "$repo_dir/.git" ]; then
      echo "Updating reference repo: $repo_dir"
      git -C "$repo_dir" remote set-url origin "$repo_url"
      git -C "$repo_dir" fetch --prune origin
      return
    fi

    if [ -e "$repo_dir" ]; then
      echo "Error: $repo_dir exists but is not a git repository" >&2
      exit 1
    fi

    echo "Cloning reference repo: $repo_url -> $repo_dir"
    git clone --origin origin "$repo_url" "$repo_dir"
  }

  ensure_effect_language_service_reference_repo
fi

# Generate diagnostics (must run before gen_shims so Effect diagnostics are included)
echo "Generating diagnostics..."
(cd typescript-go/internal/diagnostics && go run generate.go -diagnostics ./diagnostics_generated.go -loc ./loc_generated.go -locdir ./loc)

go run ./_tools/gen_shims
