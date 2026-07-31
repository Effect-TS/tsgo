# Patched Oxlint and tsgolint Integration

## Goal

Build Effect semantic diagnostics into Oxlint's type-aware pipeline while keeping this repository's `main` branch as the source of truth.

The generated integration must:

- expose Effect diagnostics as built-in `effect/*` Oxlint rules;
- run all selected Effect rules once per file through a patched tsgolint process;
- use one exact TypeScript-Go revision for Effect and tsgolint;
- preserve existing Oxlint JavaScript plugin support;
- preserve existing `typescript-eslint/*` tsgolint behavior;
- publish reproducible platform artifacts; and
- generate integration branches automatically from `main`.

## Confirmed Architecture

- The npm `oxlint` command always uses a Node launcher and a platform N-API `.node` addon. It does not switch to the standalone Rust executable when JavaScript plugins are disabled.
- A patched Oxlint N-API addon can replace the stock addon when built from the exact matching Oxlint version. The prototype exported the same N-API surface as Oxlint 1.42:
  - `Severity`
  - `getBufferOffset`
  - `lint`
  - `parseRawSync`
  - `rawTransferSupported`
- The patched tsgolint executable is still required. Oxlint starts it as a subprocess for type-aware rules.
- A separate standalone Oxlint executable is not required for npm distribution.
- Effect and tsgolint must compile against one TypeScript-Go module instance. Separate checkouts produce incompatible Go type identities even when they point at the same commit.

## Main Branch Changes

### Upstream Sources

- [ ] Add Oxlint/Oxc as an upstream submodule or reproducibly cloned build input.
- [ ] Add tsgolint as an upstream submodule or reproducibly cloned build input.
- [ ] Record the supported Oxlint tag, tsgolint commit, and package versions in one metadata file.
- [ ] Store all Effect-owned Oxlint changes under `_patches/oxlint/`.
- [ ] Store all Effect-owned tsgolint changes under `_patches/tsgolint/`.
- [ ] Do not commit patched or generated files directly inside dirty upstream submodules.

### Unified TypeScript-Go

- [ ] Read the TypeScript-Go gitlink SHA from the selected tsgolint commit.
- [ ] Pin this repository's root `typescript-go` submodule to that exact SHA in the generated branch.
- [ ] Ignore tsgolint's nested TypeScript-Go checkout during the Go build.
- [ ] Apply tsgolint's TypeScript-Go patches to the root checkout.
- [ ] Apply this repository's Effect TypeScript-Go patches to the same checkout.
- [ ] Define and validate a deterministic patch order. The successful prototype applied tsgolint patches before Effect patches.
- [ ] Fail generation immediately when either patch set no longer applies.
- [ ] Generate one shim set containing the union of Effect and tsgolint exports.
- [ ] Point Effect and tsgolint at the same shim modules and TypeScript-Go module through `go.work`.
- [ ] Generate Effect diagnostics into the combined TypeScript-Go diagnostics package.

### Public Effect Runner Boundary

- [ ] Add a narrow exported package, for example `etsrulerunner`, that wraps `internal/rulerunner`.
- [ ] Accept shim `Program`, `Checker`, and `SourceFile` values without exposing Effect internals.
- [ ] Return diagnostics with Effect rule identity, ranges, messages, labeled ranges, and fix metadata.
- [ ] Expose an operation that runs a selected set of Effect rules once for one file.
- [ ] Keep Effect option normalization in this repository rather than duplicating it in tsgolint.
- [ ] Do not change tsgolint's module path to bypass Go `internal` visibility. That was only a prototype shortcut.

## Patched tsgolint Changes

### Rule Registration and Batching

- [ ] Recognize qualified payload rules named `effect/<kebab-case-rule>`.
- [ ] Keep existing unqualified tsgolint rule names unchanged.
- [ ] Partition Effect rules from standard typescript-eslint rules for each file.
- [ ] Create one synthetic configured Effect runner per file, not one Go runner invocation per Effect rule.
- [ ] Convert selected Oxlint rule names back to Effect metadata rule names.
- [ ] Run the selected Effect rules through the exported Effect runner package.
- [ ] Map returned Effect diagnostic codes back to their qualified Oxlint rule names.

### Type Checker Lifecycle

- [ ] Precheck the requested file with Effect checker hooks suppressed.
- [ ] Acquire the file-affine checker with `GetTypeCheckerForFileExclusive`.
- [ ] Run Effect rules only after the suppressed precheck completes.
- [ ] Release the checker lease after the Effect pass.
- [ ] Ensure concurrent files do not mutate shared Effect options.
- [ ] Clone and normalize Effect options for the selected Oxlint rules.
- [ ] Clear path-scoped diagnostic severity overrides when Oxlint is the severity authority.

### Diagnostics and Fixes

- [ ] Emit qualified rule names such as `effect/floating-effect` in the headless protocol.
- [ ] Preserve UTF-16/byte range behavior expected by Oxlint.
- [ ] Carry related or labeled ranges through the headless protocol.
- [ ] Convert Effect code actions into tsgolint fixes or suggestions.
- [ ] Preserve multi-edit actions as one atomic suggestion.
- [ ] Decide which Effect actions are safe automatic fixes and which remain suggestions.
- [ ] Skip files without Effect configuration instead of returning a process-level error.
- [ ] Add timing records for the aggregate Effect pass and, optionally, individual Effect rules.

### Performance and Memory

- [ ] Avoid computing TypeScript program diagnostics unless Oxlint requested `--type-check`.
- [ ] Measure checker count and cap checker concurrency independently from file worker count.
- [ ] Release completed programs between project groups where safe.
- [ ] Investigate explicit GC or lower checker concurrency to reduce the approximately 2.7 GiB prototype peak RSS.
- [ ] Add process-tree RSS measurement so child-process memory is not omitted.

## Patched Oxlint Changes

### Built-in Effect Plugin

- [ ] Add `effect` to Oxlint's built-in plugin flags and configuration schema.
- [ ] Generate one metadata-only tsgolint rule stub for every rule in `_packages/tsgo/src/metadata.json`.
- [ ] Generate Rust module names, rule structs, descriptions, categories, and fix metadata deterministically.
- [ ] Run Oxlint's linter code generator after generating Effect stubs.
- [ ] Ensure generated rule IDs and ordering are stable.
- [ ] Include Effect rules in `--list-rules`, configuration validation, and editor schema output.

### Headless Payload

- [ ] Send Effect rules to tsgolint as qualified names, for example `effect/floating-effect`.
- [ ] Continue sending existing TypeScript rules as unqualified names.
- [ ] Group files with identical rule configurations as Oxlint already does.
- [ ] Keep source overrides, fix flags, and type-check flags compatible with the existing protocol.

### Diagnostic Scope

- [ ] Interpret qualified tsgolint diagnostic names as `<plugin>/<rule>`.
- [ ] Match qualified diagnostics against both `RuleEnum::plugin_name()` and `RuleEnum::name()`.
- [ ] Render Effect diagnostics as `effect/<rule>`.
- [ ] Keep unqualified diagnostics rendered as `typescript-eslint/<rule>`.
- [ ] Keep existing TypeScript rule URLs and error codes unchanged.
- [ ] Support `effect/<rule>` disable directives without changing TypeScript directive aliases.
- [ ] Add regression tests for qualified Effect diagnostics and existing unqualified TypeScript diagnostics.

## Generated Branch Workflow

- [ ] Create a dedicated `generated/oxlint` branch rather than changing `generated/latest` semantics.
- [ ] Trigger generation on changes to `main`, manual dispatch, and optional scheduled upstream checks.
- [ ] Check out `main` with recursive submodules.
- [ ] Resolve the configured Oxlint and tsgolint upstream revisions.
- [ ] Derive the TypeScript-Go SHA from tsgolint rather than from `typescript@latest`.
- [ ] Update the root TypeScript-Go gitlink to the derived SHA.
- [ ] Apply both TypeScript-Go patch sets to the root checkout.
- [ ] Apply the tsgolint and Oxlint patch sets.
- [ ] Regenerate combined shims, diagnostics, Effect Rust stubs, Oxlint rule tables, schemas, and metadata.
- [ ] Write generated metadata containing the source `main` SHA and every upstream SHA/version.
- [ ] Build tsgolint, the Oxlint N-API addon, and the existing Effect TypeScript-Go binary.
- [ ] Create an immutable candidate from the current `generated/oxlint` head, following `generate-latest-branch.yml`.
- [ ] Validate the exact candidate commit without repairing it locally.
- [ ] Fast-forward `generated/oxlint` directly after validation succeeds.

## Distribution

### Recommended npm Layout

- [ ] Publish an Effect-scoped Oxlint launcher or provide an exact-version patch command for the stock `oxlint` package.
- [ ] Publish platform packages containing the patched Oxlint N-API addon.
- [ ] Use separate Linux GNU and musl addon targets.
- [ ] Publish a tsgolint meta-package exposing a `tsgolint` command.
- [ ] Publish platform packages containing the patched tsgolint executable.
- [ ] Ensure the tsgolint command is discoverable through `node_modules/.bin` or set `OXLINT_TSGOLINT_PATH` in the launcher.
- [ ] Keep the Oxlint JS launcher and N-API addon on the exact same upstream version.
- [ ] Record the Oxlint upstream commit and package version beside each addon.
- [ ] Reject addon swaps when the installed Oxlint version does not match.
- [ ] Restore the original addon during unpatch or package removal.

### Distribution Choices

- [ ] Choose between an `@effect/oxlint` launcher that loads Effect-owned platform packages and an `effect-tsgo patch-oxlint` command that swaps the stock platform `.node`.
- [ ] Prefer the Effect-owned launcher if avoiding mutation of `node_modules` is more important than keeping the `oxlint` package name.
- [ ] Prefer exact-version addon swapping if minimal consumer configuration is more important.
- [ ] Do not publish the standalone Rust Oxlint executable unless a Node-free distribution is explicitly desired.

## Build Matrix

- [ ] Build Oxlint N-API addons for `win32-x64`, `win32-arm64`, `linux-x64-gnu`, `linux-arm64-gnu`, `linux-x64-musl`, `linux-arm64-musl`, `darwin-x64`, and `darwin-arm64`.
- [ ] Build tsgolint for the corresponding operating systems and CPU architectures.
- [ ] Build Go binaries with `CGO_ENABLED=0` where supported.
- [ ] Use the Rust toolchain pinned by the selected Oxlint revision.
- [ ] Reuse Oxc's napi-rs packaging conventions and platform filenames.
- [ ] Verify each addon exports the same N-API symbols as the matching stock addon.
- [ ] Smoke-test each produced tsgolint executable through the matching addon or launcher.

## Validation

- [ ] Verify all standard Oxlint tests still pass with no Effect rules enabled.
- [ ] Verify existing TypeScript type-aware rules still render under `typescript-eslint`.
- [ ] Verify Effect rules render under `effect`.
- [ ] Verify Effect and TypeScript rules can be enabled together.
- [ ] Verify JavaScript plugins continue to work through the patched N-API addon.
- [ ] Verify CLI, configuration file, overrides, disable directives, fixes, suggestions, silent mode, and type-check mode.
- [ ] Compare diagnostic identities, ranges, severities, and messages against direct Effect TypeScript-Go diagnostics.
- [ ] Benchmark representative Effect rules and all Effect rules.
- [ ] Record wall time, CPU time, configured project count, inferred project count, diagnostics, and aggregate process-tree RSS.
- [ ] Run the repository's normal `pnpm setup-repo`, `pnpm lint`, `pnpm check`, and `pnpm test` validation.

## Prototype Baseline

The local prototype established the following baseline on the Effect repository:

| Path | Fix lookup | Time | Peak process-tree RSS | Effect diagnostics |
| --- | --- | ---: | ---: | ---: |
| Patched Oxlint plus tsgolint | No | 28.26s | 2,788,600 KiB | 4,143 |

These numbers validate the architecture but are not release acceptance thresholds. The prototype did not implement Effect fixes in tsgolint and requires further memory optimization.

## Completion Criteria

- [ ] `main` contains only Effect-owned APIs, metadata, patch files, generators, workflows, and package definitions.
- [ ] `generated/oxlint` reproducibly pins all upstream source revisions and generated output.
- [ ] A clean checkout can build the patched N-API addon and tsgolint executable without manual source edits.
- [ ] npm consumers can install or patch the integration without setting development-only environment variables.
- [ ] Existing Oxlint JavaScript and TypeScript rules retain their behavior.
- [ ] Effect diagnostics render under the `effect` scope and support fixes and directives.
- [ ] CI validates correctness and platform artifacts before publishing.
