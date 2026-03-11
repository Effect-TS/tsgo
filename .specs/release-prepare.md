# Release Prepare Command

## Goal
Define a single command that prepares release-ready workspace packages for preview or snapshot publishing from CI.

## Scope
- Applies to the repository-level release preparation workflow.
- Covers publishable npm packages defined under `_packages`.
- Covers preparation of platform-specific `tsgo` binary artifacts into the publishable package locations under `_packages`.
- Covers behavior of the single command contract exposed to CI.
- Covers the GitHub Actions release workflow's integration with target selection.

## Requirements
1. The project must expose a single `release:prepare` command that CI can call as the canonical release-preparation entrypoint.
2. Publishable package manifests must be maintained as real workspace packages under `_packages` rather than synthesized into a detached output tree.
3. Running `release:prepare` must prepare publish-ready workspace packages in `_packages`, including all required package metadata for packaging/publishing workflows.
4. `release:prepare` must cross-compile platform-specific `tsgo` binaries directly into the corresponding publishable package locations under `_packages`. No intermediate build directory should be used.
5. `release:prepare` must perform its steps in this order: first cross-compile all platform binaries sequentially, then build the CLI bundle.
6. The script must accept a repeatable `--target <platform-arch>` flag to select which platform targets to build (e.g., `--target linux-x64 --target darwin-arm64`). When no `--target` flags are provided, all targets are built. Target identifiers use the `<npm_platform>-<npm_arch>` format matching the package naming convention (e.g., `linux-x64`, `darwin-arm64`, `win32-x64`).
7. Artifact validation must only check the targets that were actually selected for the current run.
8. The GitHub Actions release workflow must use a static matrix strategy to build platform binaries in parallel. Platform selection must be controlled by `workflow_dispatch` boolean inputs (checkboxes in the manual trigger UI), not by hardcoded matrix `enabled` values in the workflow file. Each matrix job evaluates whether its target is selected by those inputs; selected targets run `release-prepare --target <platform> --skip-cli` and upload artifacts, while unselected targets are skipped via step-level `if` conditions and remain visible in the job list. A final publish job (with `needs` on the matrix job) downloads only artifacts from selected targets into the correct `_packages` locations, builds the CLI bundle, and runs a single `pkg.pr.new publish` invocation with all selected packages.
10. The script must accept a `--skip-cli` flag that skips the CLI bundle build step. When `--skip-cli` is set, only binary compilation and validation are performed. This is used by the matrix build jobs, which only need to produce the binary artifact.
11. `release:prepare` must fail fast with actionable errors if required release inputs or artifacts are missing.
12. `release:prepare` must be repeatable in CI: reruns should converge to a consistent publish-ready state without manual cleanup.
13. The command must preserve existing architecture and setup constraints documented for this repository (including required generated artifacts and patch setup prerequisites).
14. Release workflow dependency installation must run with lockfile immutability enabled, and the repository must include the root `pnpm-lock.yaml` required for frozen installs.

## Non-Goals
- Performing the publish step itself.
- Defining external registry release policy.
- Changing end-user CLI behavior.

## Acceptance Criteria
1. A CI job can call one command, `release:prepare`, to produce the full publish-ready workspace package set.
2. After `release:prepare`, the main CLI package and platform-specific binary packages in `_packages` are ready to be published.
3. The prepared workspace package paths can be consumed directly by `pkg.pr.new publish` invocations (one per platform binary in the matrix, one for the CLI package in the publish job).
4. Missing inputs or incomplete artifacts are surfaced as clear command failures before publish.
5. In a manual workflow dispatch, users can choose target platforms through checkbox inputs, and only selected platform jobs are built/published.
6. A clean checkout can execute release workflow dependency installation with `pnpm install --frozen-lockfile` without lockfile-related failures.
