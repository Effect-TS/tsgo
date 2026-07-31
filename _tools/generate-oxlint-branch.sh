#!/usr/bin/env bash
set -euo pipefail

export LC_ALL=C

ROOT_DIR="$(git -C "$(dirname "${BASH_SOURCE[0]}")" rev-parse --show-toplevel)"
METADATA_FILE="$ROOT_DIR/_packages/tsgo/upstream.json"
RESOLVED_METADATA_FILE="$ROOT_DIR/_generated/oxlint/metadata.json"
OXLINT_DIR="$ROOT_DIR/oxlint"
TSGOLINT_DIR="$ROOT_DIR/tsgolint"
TSGO_DIR="$ROOT_DIR/typescript-go"
BUILD_DIR="$ROOT_DIR/build/oxlint-tsgolint"

BUILD=false
CHECK_ONLY=false
FORCE=false

usage() {
  cat <<'EOF'
Usage: bash _tools/generate-oxlint-branch.sh [--build] [--check] [--ci] [--force]

Generate the working tree for the generated/oxlint branch from a clean main
checkout. The script adds pinned Oxlint and tsgolint submodules to the generated
tree only; it does not switch branches, create a commit, or push.

  --build  Install Oxlint dependencies and build tsgolint and the Oxlint N-API
           addon after generation.
  --check  Validate metadata and any generated submodule pins without changing
           the workspace.
  --ci     Skip local-only setup. Accepted when invoked through setup-repo.
  --force  Discard changes inside upstream submodules during generation.
  --help   Show this help.

Generation resets and cleans the oxlint, tsgolint, and typescript-go submodules.
Permanent upstream changes must be stored under _patches/.
EOF
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

log() {
  printf '==> %s\n' "$*"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

canonical_dir() {
  (cd "$1" && pwd -P)
}

while [ "$#" -gt 0 ]; do
  case "$1" in
    --build)
      BUILD=true
      ;;
    --check)
      CHECK_ONLY=true
      ;;
    --ci)
      ;;
    --force)
      FORCE=true
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      usage >&2
      die "unknown argument: $1"
      ;;
  esac
  shift
done

require_command git
require_command node

[ -f "$METADATA_FILE" ] || die "metadata file not found: $METADATA_FILE"

profile_json="$(node "$ROOT_DIR/_tools/upstream.mjs" profile oxlint)"
metadata_values="$({ node --input-type=module - "$METADATA_FILE" "$profile_json" <<'NODE'
const metadataPath = process.argv[2]
const profile = JSON.parse(process.argv[3])
const values = [
  profile.tsGitHead,
  profile.oxlintRepository,
  profile.oxlintTag,
  profile.oxlintHead,
  profile.oxlintVersion,
  profile.tsgolintRepository,
  profile.tsgolintHead,
  profile.tsgolintVersion,
]

if (values.some((value) => value === undefined || value === null || value === "")) {
  throw new Error(`Incomplete integration metadata in ${metadataPath}`)
}
if (values.some((value) => String(value).includes("\t") || String(value).includes("\n"))) {
  throw new Error(`Integration metadata contains an unsupported delimiter in ${metadataPath}`)
}

process.stdout.write(values.join("\t"))
NODE
} 2>&1)" || die "$metadata_values"

IFS=$'\t' read -r PROFILE_TSGO_REVISION OXLINT_REPOSITORY OXLINT_TAG OXLINT_REVISION OXLINT_PACKAGE_VERSION TSGOLINT_REPOSITORY TSGOLINT_REVISION TSGOLINT_PACKAGE_VERSION <<<"$metadata_values"

[[ "$PROFILE_TSGO_REVISION" =~ ^[0-9a-f]{40}$ ]] || die "invalid profile TypeScript-Go revision: $PROFILE_TSGO_REVISION"
[[ "$OXLINT_REVISION" =~ ^[0-9a-f]{40}$ ]] || die "invalid Oxlint revision: $OXLINT_REVISION"
[[ "$TSGOLINT_REVISION" =~ ^[0-9a-f]{40}$ ]] || die "invalid tsgolint revision: $TSGOLINT_REVISION"

submodule_gitlink() {
  local path="$1"
  local entry
  local mode
  local revision

  entry="$(git -C "$ROOT_DIR" ls-files --stage -- "$path")"
  [ -n "$entry" ] || return 1
  read -r mode revision _ <<<"$entry"
  [ "$mode" = 160000 ] || die "expected a submodule gitlink for $path"
  printf '%s\n' "$revision"
}

validate_generated_submodule() {
  local name="$1"
  local path="$2"
  local repository="$3"
  local revision="$4"
  local configured_repository
  local head_revision
  local gitlink

  configured_repository="$(git -C "$ROOT_DIR" config -f .gitmodules --get "submodule.$name.url")"
  [ "$configured_repository" = "$repository" ] || die "$name URL does not match $METADATA_FILE"
  gitlink="$(submodule_gitlink "$name")" || die "$name is configured but has no gitlink"
  [ "$gitlink" = "$revision" ] || die "$name gitlink $gitlink does not match metadata revision $revision"
  [ -e "$path/.git" ] || die "$name is not initialized"
  git -C "$path" cat-file -e "$revision^{commit}" 2>/dev/null || die "$name revision is unavailable locally: $revision"
  head_revision="$(git -C "$path" rev-parse HEAD)"
  [ "$head_revision" = "$revision" ] || die "$name checkout $head_revision does not match gitlink $revision"
}

derive_typescript_go_revision() {
  local entry
  local mode
  local revision
  local path

  entry="$(git -C "$TSGOLINT_DIR" ls-tree "$TSGOLINT_REVISION" typescript-go)"
  read -r mode _ revision path <<<"$entry"
  [ "$mode" = 160000 ] || die "tsgolint revision does not contain a typescript-go gitlink"
  [ "$path" = typescript-go ] || die "unexpected tsgolint TypeScript-Go path: $path"
  printf '%s\n' "$revision"
}

derive_typescript_revision() {
  local entry
  local mode
  local revision
  local path

  entry="$(git -C "$TSGO_DIR" ls-tree "$TSGO_REVISION" _submodules/TypeScript)"
  read -r mode _ revision path <<<"$entry"
  [ "$mode" = 160000 ] || die "TypeScript-Go revision does not contain a TypeScript gitlink"
  [ "$path" = _submodules/TypeScript ] || die "unexpected TypeScript path: $path"
  printf '%s\n' "$revision"
}

validate_remote_metadata() (
  local check_dir
  local entry
  local mode
  local revision
  local path
  local resolved_oxlint_tag
  local actual_oxlint_version
  local actual_tsgolint_version

  require_command mktemp
  check_dir="$(mktemp -d "${TMPDIR:-/tmp}/effect-tsgo-oxlint-check.XXXXXX")"
  trap 'rm -rf "$check_dir"' EXIT

  git -C "$check_dir" init --quiet oxlint
  git -C "$check_dir/oxlint" remote add origin "$OXLINT_REPOSITORY"
  git -C "$check_dir/oxlint" fetch --quiet --depth 1 origin "$OXLINT_REVISION"
  git -C "$check_dir/oxlint" fetch --quiet --depth 1 origin "refs/tags/$OXLINT_TAG:refs/tags/$OXLINT_TAG"
  resolved_oxlint_tag="$(git -C "$check_dir/oxlint" rev-parse "$OXLINT_TAG^{commit}")"
  [ "$resolved_oxlint_tag" = "$OXLINT_REVISION" ] || die "$OXLINT_TAG resolves to $resolved_oxlint_tag, expected $OXLINT_REVISION"
  actual_oxlint_version="$(git -C "$check_dir/oxlint" show "$OXLINT_REVISION:apps/oxlint/package.json" | node -e '
let input = ""
process.stdin.on("data", (chunk) => input += chunk)
process.stdin.on("end", () => process.stdout.write(JSON.parse(input).version))
')"
  [ "$actual_oxlint_version" = "$OXLINT_PACKAGE_VERSION" ] || die "Oxlint package version $actual_oxlint_version does not match metadata version $OXLINT_PACKAGE_VERSION"

  git -C "$check_dir" init --quiet tsgolint
  git -C "$check_dir/tsgolint" remote add origin "$TSGOLINT_REPOSITORY"
  git -C "$check_dir/tsgolint" fetch --quiet --depth 50 --tags origin "$TSGOLINT_REVISION"
  actual_tsgolint_version="$(git -C "$check_dir/tsgolint" describe --tags --always FETCH_HEAD)"
  [ "$actual_tsgolint_version" = "$TSGOLINT_PACKAGE_VERSION" ] || die "tsgolint version $actual_tsgolint_version does not match metadata version $TSGOLINT_PACKAGE_VERSION"
  entry="$(git -C "$check_dir/tsgolint" ls-tree FETCH_HEAD typescript-go)"
  read -r mode _ revision path <<<"$entry"
  [ "$mode" = 160000 ] || die "tsgolint revision does not contain a typescript-go gitlink"
  [ "$path" = typescript-go ] || die "unexpected tsgolint TypeScript-Go path: $path"
  [ "$revision" = "$PROFILE_TSGO_REVISION" ] || die "tsgolint TypeScript-Go revision $revision does not match oxlint profile $PROFILE_TSGO_REVISION"
  printf 'TypeScript-Go: %s (derived from tsgolint)\n' "$revision"
)

if [ "$CHECK_ONLY" = true ]; then
  validate_remote_metadata
  if submodule_gitlink oxlint >/dev/null 2>&1 || submodule_gitlink tsgolint >/dev/null 2>&1; then
    validate_generated_submodule oxlint "$OXLINT_DIR" "$OXLINT_REPOSITORY" "$OXLINT_REVISION"
    validate_generated_submodule tsgolint "$TSGOLINT_DIR" "$TSGOLINT_REPOSITORY" "$TSGOLINT_REVISION"
    TSGO_REVISION="$(derive_typescript_go_revision)"
    root_tsgo_revision="$(submodule_gitlink typescript-go)"
    [ "$root_tsgo_revision" = "$TSGO_REVISION" ] || die "root TypeScript-Go gitlink $root_tsgo_revision does not match tsgolint revision $TSGO_REVISION"
    root_tsgo_head="$(git -C "$TSGO_DIR" rev-parse HEAD)"
    [ "$root_tsgo_head" = "$TSGO_REVISION" ] || die "root TypeScript-Go checkout $root_tsgo_head does not match gitlink $TSGO_REVISION"
  else
    log "generated submodules are absent, as expected on main"
  fi
  log "integration metadata is valid"
  printf 'Oxlint:   %s (%s)\n' "$OXLINT_PACKAGE_VERSION" "$OXLINT_REVISION"
  printf 'tsgolint: %s (%s)\n' "$TSGOLINT_PACKAGE_VERSION" "$TSGOLINT_REVISION"
  exit 0
fi

require_command go
if [ "$BUILD" = true ]; then
  require_command cargo
  require_command pnpm
fi

[ -z "$(git -C "$ROOT_DIR" status --porcelain --untracked-files=all)" ] || die "generation requires a clean checkout"

SOURCE_REVISION="$(git -C "$ROOT_DIR" rev-parse HEAD)"

ensure_commit() {
  local checkout="$1"
  local revision="$2"

  if ! git -C "$checkout" cat-file -e "$revision^{commit}" 2>/dev/null; then
    git -C "$checkout" fetch --depth 1 origin "$revision"
  fi
  git -C "$checkout" cat-file -e "$revision^{commit}" 2>/dev/null || die "revision is unavailable after fetch: $revision"
}

ensure_tag() {
  local checkout="$1"
  local tag="$2"

  if ! git -C "$checkout" rev-parse "$tag^{commit}" >/dev/null 2>&1; then
    git -C "$checkout" fetch --depth 1 origin "refs/tags/$tag:refs/tags/$tag"
  fi
}

ensure_generated_submodule() {
  local name="$1"
  local repository="$2"
  local revision="$3"
  local path="$ROOT_DIR/$name"
  local configured_path
  local configured_repository
  local cached_git_dir
  local cached_repository
  local submodule_add_args=()

  configured_path="$(git -C "$ROOT_DIR" config -f .gitmodules --get "submodule.$name.path" || true)"
  if [ -n "$configured_path" ]; then
    [ "$configured_path" = "$name" ] || die "$name submodule uses unexpected path: $configured_path"
    configured_repository="$(git -C "$ROOT_DIR" config -f .gitmodules --get "submodule.$name.url")"
    [ "$configured_repository" = "$repository" ] || die "$name submodule uses unexpected URL: $configured_repository"
    if [ ! -e "$path/.git" ]; then
      git -C "$ROOT_DIR" submodule update --init "$name"
    fi
  else
    [ ! -e "$path" ] || die "$path exists but is not a configured submodule"
    cached_git_dir="$(git -C "$ROOT_DIR" rev-parse --absolute-git-dir)/modules/$name"
    if [ -d "$cached_git_dir" ]; then
      cached_repository="$(git config --file="$cached_git_dir/config" --get remote.origin.url)"
      [ "$cached_repository" = "$repository" ] || die "cached $name repository uses unexpected URL: $cached_repository"
      submodule_add_args=(--force)
    fi
    git -C "$ROOT_DIR" submodule add "${submodule_add_args[@]}" --name "$name" --depth 1 "$repository" "$name"
  fi

  git -C "$ROOT_DIR" config -f .gitmodules "submodule.$name.ignore" dirty
  ensure_commit "$path" "$revision"
}

assert_disposable_checkout() {
  local checkout="$1"
  local dirty

  dirty="$(git -C "$checkout" status --porcelain --untracked-files=all --ignored=matching)"
  if [ -n "$dirty" ] && [ "$FORCE" = false ]; then
    die "$checkout has local changes; rerun with --force to discard them"
  fi
}

reset_checkout() {
  local checkout="$1"
  local revision="$2"

  git -C "$checkout" reset --hard
  git -C "$checkout" clean -fdx
  git -C "$checkout" checkout --detach "$revision"
  git -C "$checkout" reset --hard "$revision"
  git -C "$checkout" clean -fdx
}

apply_patch_dir() {
  local checkout="$1"
  local patch_dir="$2"
  local label="$3"
  local patches

  shopt -s nullglob
  patches=("$patch_dir"/*.patch)
  shopt -u nullglob

  if [ "${#patches[@]}" -eq 0 ]; then
    log "No $label patches found"
    return
  fi

  for patch in "${patches[@]}"; do
    log "Applying $label patch: ${patch#"$ROOT_DIR/"}"
    git -C "$checkout" apply --check "$patch"
    git -C "$checkout" apply "$patch"
  done
}

log "Adding generated-branch upstream submodules"
ensure_generated_submodule oxlint "$OXLINT_REPOSITORY" "$OXLINT_REVISION"
ensure_tag "$OXLINT_DIR" "$OXLINT_TAG"
ensure_generated_submodule tsgolint "$TSGOLINT_REPOSITORY" "$TSGOLINT_REVISION"

TSGO_REVISION="$(derive_typescript_go_revision)"
[[ "$TSGO_REVISION" =~ ^[0-9a-f]{40}$ ]] || die "invalid TypeScript-Go revision derived from tsgolint: $TSGO_REVISION"
[ "$TSGO_REVISION" = "$PROFILE_TSGO_REVISION" ] || die "tsgolint TypeScript-Go revision $TSGO_REVISION does not match oxlint profile $PROFILE_TSGO_REVISION"

git -C "$ROOT_DIR" submodule sync -- typescript-go
if [ ! -e "$TSGO_DIR/.git" ]; then
  git -C "$ROOT_DIR" submodule update --init typescript-go
fi
ensure_commit "$TSGO_DIR" "$TSGO_REVISION"

log "Resetting upstreams to their pinned revisions"
assert_disposable_checkout "$OXLINT_DIR"
assert_disposable_checkout "$TSGOLINT_DIR"
assert_disposable_checkout "$TSGO_DIR"
reset_checkout "$OXLINT_DIR" "$OXLINT_REVISION"
reset_checkout "$TSGOLINT_DIR" "$TSGOLINT_REVISION"
reset_checkout "$TSGO_DIR" "$TSGO_REVISION"

actual_tsgolint_version="$(git -C "$TSGOLINT_DIR" describe --tags --always "$TSGOLINT_REVISION")"
if [ "$actual_tsgolint_version" != "$TSGOLINT_PACKAGE_VERSION" ]; then
  git -C "$TSGOLINT_DIR" fetch --quiet --depth 50 --tags origin "$TSGOLINT_REVISION"
  actual_tsgolint_version="$(git -C "$TSGOLINT_DIR" describe --tags --always "$TSGOLINT_REVISION")"
fi
[ "$actual_tsgolint_version" = "$TSGOLINT_PACKAGE_VERSION" ] || die "tsgolint version $actual_tsgolint_version does not match metadata version $TSGOLINT_PACKAGE_VERSION"

resolved_oxlint_tag="$(git -C "$OXLINT_DIR" rev-parse "$OXLINT_TAG^{commit}")"
[ "$resolved_oxlint_tag" = "$OXLINT_REVISION" ] || die "$OXLINT_TAG resolves to $resolved_oxlint_tag, expected $OXLINT_REVISION"
actual_oxlint_version="$(node -p "require(process.argv[1]).version" "$OXLINT_DIR/apps/oxlint/package.json")"
[ "$actual_oxlint_version" = "$OXLINT_PACKAGE_VERSION" ] || die "Oxlint package version $actual_oxlint_version does not match metadata version $OXLINT_PACKAGE_VERSION"

git -C "$ROOT_DIR" add .gitmodules oxlint tsgolint typescript-go

log "Initializing TypeScript-Go's TypeScript submodule"
git -C "$TSGO_DIR" submodule sync --recursive
git -C "$TSGO_DIR" submodule update --init --force _submodules/TypeScript
TYPESCRIPT_REVISION="$(derive_typescript_revision)"
actual_typescript_revision="$(git -C "$TSGO_DIR/_submodules/TypeScript" rev-parse HEAD)"
[ "$actual_typescript_revision" = "$TYPESCRIPT_REVISION" ] || die "TypeScript checkout $actual_typescript_revision does not match gitlink $TYPESCRIPT_REVISION"

# The successful prototype established this patch order. Keep it explicit.
apply_patch_dir "$TSGO_DIR" "$TSGOLINT_DIR/patches" "tsgolint TypeScript-Go"
apply_patch_dir "$TSGO_DIR" "$ROOT_DIR/_patches" "Effect TypeScript-Go"
apply_patch_dir "$TSGOLINT_DIR" "$ROOT_DIR/_patches/tsgolint" "Effect tsgolint"
apply_patch_dir "$OXLINT_DIR" "$ROOT_DIR/_patches/oxlint" "Effect Oxlint"

log "Synchronizing tsgolint's internal collections from the shared TypeScript-Go checkout"
rm -rf "$TSGOLINT_DIR/internal/collections"
mkdir -p "$TSGOLINT_DIR/internal/collections"
shopt -s nullglob
collection_files=("$TSGO_DIR/internal/collections/"*)
shopt -u nullglob
for collection_file in "${collection_files[@]}"; do
  if [ -f "$collection_file" ] && [[ "$collection_file" != *_test.go ]]; then
    cp "$collection_file" "$TSGOLINT_DIR/internal/collections/"
  fi
done

log "Merging tsgolint shim requirements into the shared Effect shim configuration"
node --input-type=module - "$ROOT_DIR" "$TSGOLINT_DIR" <<'NODE'
import { mkdirSync, readFileSync, readdirSync, writeFileSync } from "node:fs"
import { dirname, join, relative } from "node:path"

const root = process.argv[2]
const tsgolint = process.argv[3]
const sourceRoot = join(tsgolint, "shim")
const targetRoot = join(root, "shim")

const sourceFiles = []
const visit = (directory) => {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name)
    if (entry.isDirectory()) visit(path)
    else if (entry.name === "extra-shim.json") sourceFiles.push(path)
  }
}
visit(sourceRoot)
sourceFiles.sort()

const sortedUnion = (left = [], right = []) => [...new Set([...left, ...right])].sort()
const sortObject = (value) => Object.fromEntries(Object.entries(value).sort(([left], [right]) => left.localeCompare(right)))

for (const sourceFile of sourceFiles) {
  const targetFile = join(targetRoot, relative(sourceRoot, sourceFile))
  const source = JSON.parse(readFileSync(sourceFile, "utf8"))
  let target = {}
  try {
    target = JSON.parse(readFileSync(targetFile, "utf8"))
  } catch (error) {
    if (error.code !== "ENOENT") throw error
  }

  for (const [key, value] of Object.entries(source)) {
    if (Array.isArray(value)) {
      target[key] = sortedUnion(target[key], value)
      continue
    }
    if (value && typeof value === "object") {
      const merged = { ...(target[key] ?? {}) }
      for (const [name, entries] of Object.entries(value)) {
        merged[name] = sortedUnion(merged[name], entries)
      }
      target[key] = sortObject(merged)
      continue
    }
    throw new Error(`Unsupported ${key} value in ${sourceFile}`)
  }

  mkdirSync(dirname(targetFile), { recursive: true })
  writeFileSync(targetFile, `${JSON.stringify(sortObject(target), null, 2)}\n`)
}
NODE

log "Merging tsgolint's handwritten shim helpers into the shared shim tree"
while IFS= read -r -d '' helper_file; do
  relative_helper="${helper_file#"$TSGOLINT_DIR/shim/"}"
  target_helper="$ROOT_DIR/shim/$relative_helper"
  if [ -f "$target_helper" ] && ! cmp -s "$helper_file" "$target_helper"; then
    die "conflicting handwritten shim helper: shim/$relative_helper"
  fi
  mkdir -p "$(dirname "$target_helper")"
  cp "$helper_file" "$target_helper"
done < <(find "$TSGOLINT_DIR/shim" -type f -name '*.go' ! -name 'shim.go' -print0)

log "Generating combined TypeScript-Go diagnostics and shims"
pushd "$TSGO_DIR/internal/diagnostics" >/dev/null
go run generate.go -diagnostics ./diagnostics_generated.go -loc ./loc_generated.go -locdir ./loc
popd >/dev/null
pushd "$ROOT_DIR" >/dev/null
go run ./_tools/gen_shims
go work edit -use=./tsgolint

workspace_replacements="$(GOWORK=off go mod edit -json "$TSGOLINT_DIR/go.mod" | node -e '
let input = ""
process.stdin.on("data", (chunk) => input += chunk)
process.stdin.on("end", () => {
  const module = JSON.parse(input)
  const versions = new Map((module.Require ?? []).map(({ Path, Version }) => [Path, Version]))
  const prefix = "github.com/microsoft/typescript-go/shim/"
  const replacements = (module.Replace ?? [])
    .filter(({ Old }) => Old.Path.startsWith(prefix))
    .map(({ Old }) => `${Old.Path}@${versions.get(Old.Path) ?? "v0.0.0"}\t./shim/${Old.Path.slice(prefix.length)}`)
    .sort()
  process.stdout.write(replacements.join("\n"))
})
')"
while IFS=$'\t' read -r module_version shim_path; do
  [ -n "$module_version" ] || continue
  [ -d "$ROOT_DIR/${shim_path#./}" ] || die "shared shim path does not exist: $shim_path"
  go work edit -replace="$module_version=$shim_path"
done <<<"$workspace_replacements"
popd >/dev/null

log "Verifying the shared Go module graph"
pushd "$ROOT_DIR" >/dev/null
resolved_tsgo_dir="$(GOWORK="$ROOT_DIR/go.work" go list -m -f '{{.Dir}}' github.com/microsoft/typescript-go)"
resolved_checker_dir="$(GOWORK="$ROOT_DIR/go.work" go list -m -f '{{.Dir}}' github.com/microsoft/typescript-go/shim/checker)"
popd >/dev/null
[ "$(canonical_dir "$resolved_tsgo_dir")" = "$(canonical_dir "$TSGO_DIR")" ] || die "Go workspace does not resolve the shared TypeScript-Go checkout"
[ "$(canonical_dir "$resolved_checker_dir")" = "$(canonical_dir "$ROOT_DIR/shim/checker")" ] || die "Go workspace does not resolve the shared checker shim"

log "Writing resolved integration metadata"
mkdir -p "$(dirname "$RESOLVED_METADATA_FILE")" "$BUILD_DIR"
node --input-type=module - "$METADATA_FILE" "$RESOLVED_METADATA_FILE" "$SOURCE_REVISION" "$TSGO_REVISION" "$TYPESCRIPT_REVISION" <<'NODE'
import { readFileSync, writeFileSync } from "node:fs"

const sourceFile = process.argv[2]
const outputFile = process.argv[3]
const sourceRevision = process.argv[4]
const typescriptGoRevision = process.argv[5]
const typescriptRevision = process.argv[6]
const metadata = JSON.parse(readFileSync(sourceFile, "utf8")).oxlint

writeFileSync(outputFile, `${JSON.stringify({
  ...metadata,
  sourceRevision,
  typescriptGo: { revision: typescriptGoRevision },
  typescript: { revision: typescriptRevision },
}, null, 2)}\n`)
NODE

if [ "$BUILD" = true ]; then
  log "Building tsgolint"
  pushd "$ROOT_DIR" >/dev/null
  GOWORK="$ROOT_DIR/go.work" CGO_ENABLED=0 go build -trimpath -ldflags='-s -w' -o "$BUILD_DIR/tsgolint" ./tsgolint/cmd/tsgolint
  popd >/dev/null

  if [ -f "$ROOT_DIR/_tools/gen-oxlint-effect-rules.mjs" ]; then
    log "Generating Effect Oxlint rule stubs"
    node "$ROOT_DIR/_tools/gen-oxlint-effect-rules.mjs" "$OXLINT_DIR" "$ROOT_DIR/_packages/tsgo/src/metadata.json"
  else
    log "No Effect Oxlint rule generator is present; relying on the Oxlint patch set"
  fi

  log "Generating Oxlint rule tables"
  pushd "$OXLINT_DIR" >/dev/null
  cargo lintgen
  popd >/dev/null

  log "Installing Oxlint dependencies and building the N-API addon"
  pnpm --dir "$OXLINT_DIR" install --frozen-lockfile
  pnpm --dir "$OXLINT_DIR/apps/oxlint" run build-napi-release
fi

log "generated/oxlint bootstrap is ready"
printf 'Resolved metadata: %s\n' "$RESOLVED_METADATA_FILE"
