#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

CI_MODE=false
PROFILE=next

usage() {
  cat <<'EOF'
Usage: bash _tools/setup-repo.sh [--profile <next|stable|oxlint>] [--ci]

Materialize one upstream profile from _packages/tsgo/upstream.json. The default
profile is next. --stable and --oxlint are shortcuts for their profile names.

  --profile  Select next, stable, or oxlint.
  --stable   Select the stable profile.
  --oxlint   Select the oxlint profile.
  --ci       Skip local-only reference repository setup.
  --help     Show this help.
EOF
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --ci)
      CI_MODE=true
      ;;
    --profile)
      [ "$#" -ge 2 ] || { echo "error: --profile requires a value" >&2; exit 1; }
      PROFILE="$2"
      shift
      ;;
    --stable)
      PROFILE=stable
      ;;
    --oxlint)
      PROFILE=oxlint
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      echo "error: unknown argument: $1" >&2
      exit 1
      ;;
  esac
  shift
done

case "$PROFILE" in
  next|stable|oxlint) ;;
  *)
    echo "error: unknown upstream profile: $PROFILE" >&2
    exit 1
    ;;
esac

node _tools/upstream.mjs validate

if [ "$PROFILE" = oxlint ]; then
  oxlint_args=()
  if [ "$CI_MODE" = true ]; then
    oxlint_args+=(--ci)
  fi
  exec bash _tools/generate-oxlint-branch.sh "${oxlint_args[@]}"
fi

TS_VERSION="$(node _tools/upstream.mjs field "$PROFILE" tsVersion)"
TSGO_REVISION="$(node _tools/upstream.mjs field "$PROFILE" tsGitHead)"

echo "Setting up $PROFILE profile: TypeScript $TS_VERSION ($TSGO_REVISION)"

if [ "$CI_MODE" = false ]; then
  # Keep submodule config in sync before updating.
  git submodule sync --recursive

  # Clear stale git index locks that can break recursive submodule updates.
  rm -f .git/modules/typescript-go/modules/_submodules/TypeScript/index.lock
  rm -f typescript-go/.git/modules/_submodules/TypeScript/index.lock

  git submodule update --init --force typescript-go
fi

if [ ! -e typescript-go/.git ]; then
  git submodule update --init typescript-go
fi

git -C typescript-go reset --hard
git -C typescript-go clean -fdx

if ! git -C typescript-go cat-file -e "$TSGO_REVISION^{commit}" 2>/dev/null; then
  git -C typescript-go fetch --depth 1 origin "$TSGO_REVISION"
fi
git -C typescript-go checkout --detach "$TSGO_REVISION"
git -C typescript-go reset --hard "$TSGO_REVISION"
git -C typescript-go clean -fdx

actual_typescript_revision="$(git -C typescript-go ls-tree "$TSGO_REVISION" _submodules/TypeScript | awk '{print $3}')"
if [[ ! "$actual_typescript_revision" =~ ^[0-9a-f]{40}$ ]]; then
  echo "error: unable to derive the TypeScript source commit for $PROFILE profile" >&2
  exit 1
fi
TYPESCRIPT_REVISION="$actual_typescript_revision"

git -C typescript-go submodule sync --recursive
if ! git -C typescript-go submodule update --init --force _submodules/TypeScript; then
  git -C typescript-go/_submodules/TypeScript fetch --depth 1 origin "$TYPESCRIPT_REVISION"
  git -C typescript-go/_submodules/TypeScript checkout --detach "$TYPESCRIPT_REVISION"
fi

nested_typescript_revision="$(git -C typescript-go/_submodules/TypeScript rev-parse HEAD)"
if [ "$nested_typescript_revision" != "$TYPESCRIPT_REVISION" ]; then
  echo "error: TypeScript checkout $nested_typescript_revision does not match $PROFILE profile $TYPESCRIPT_REVISION" >&2
  exit 1
fi

# Apply patches in bytewise filename order.
for patch in _patches/*.patch; do
    if [ -f "$patch" ]; then
        echo "Applying patch: $patch"
        git -C typescript-go apply --check "../$patch"
        git -C typescript-go apply "../$patch"
    fi
done

echo "All patches applied successfully"

if [ "$CI_MODE" = false ]; then
  ensure_effect_reference_repo() {
    local repo_dir=".repos/effect"
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

  ensure_effect_reference_repo

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

# Generate shims only; release version sync is handled by _tools/version-prepare.sh.
echo "Generating shims..."
go run ./_tools/gen_shims
