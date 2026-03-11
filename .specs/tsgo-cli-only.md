# Tsgo CLI-Only Contract

## Goal
Define `@effect/tsgo` as a CLI-only package, with command-driven usage and no library API surface.

## Scope
- Applies to the `@effect/tsgo` package and its published package contract.
- Covers package entrypoint and distribution expectations for consumers.
- Covers dependency version contract for `effect` and `@effect/platform-node` used by the CLI package.
- Covers dependency graph coherence and install determinism required for reproducible CLI builds in CI.

## Requirements
1. `@effect/tsgo` must be treated as a CLI package, not a reusable library package.
2. The supported usage model is invoking documented CLI commands via the `effect-tsgo` binary.
3. The package must not define or maintain a `/lib` distribution contract for consumers.
4. Consumers must not be expected to import exported helper functions from `@effect/tsgo`.
5. Any behavior currently accessible by exported functions must be available through CLI commands instead.
6. The CLI package dependency on `effect` must use this exact package version target: `4.0.0-beta.11`.
7. The CLI package dependency on `@effect/platform-node` must use this exact package version target: `4.0.0-beta.11`.
8. CLI builds must use a coherent Effect pre-release dependency graph: resolved `@effect/*` packages consumed by the CLI must be mutually compatible with the pinned `effect` target and must not mix incompatible beta lines.
9. CI and release workflows that build the CLI must use lockfile-immutable dependency installation so dependency resolution does not drift between local and CI runs.
10. Any intentional dependency graph update for CLI runtime packages must be accompanied by lockfile updates and successful CLI build validation in the same change.

## Non-Goals
- Defining new CLI features beyond parity with existing command capabilities.
- Defining internals of command implementation.

## Acceptance Criteria
1. Package contract documentation and release expectations present `@effect/tsgo` as CLI-only.
2. Published package expectations do not include `/lib` as a supported consumer API surface.
3. Users can accomplish supported workflows through CLI commands without relying on library imports.
4. The CLI package dependency contract pins `effect` and `@effect/platform-node` to the exact package targets defined in this spec.
5. A clean CI run that installs from lockfile immutably and executes the CLI build does not fail due to missing exports from mismatched `@effect/*` package versions.
