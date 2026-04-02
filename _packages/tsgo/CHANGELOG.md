# @effect/tsgo

## 0.2.1

### Patch Changes

- 6a7d03c: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/8f15d7f682574fa20a2cfd8b67a2fe22a83e0e27) to commit `8f15d7f682574fa20a2cfd8b67a2fe22a83e0e27`.

## 0.2.0

### Minor Changes

- 24a8a96: Refactor Effect plugin option handling to support per-file `overrides`, simplify
  TypeScript-Go merge hooks, and clean up the internal config model used by
  diagnostics, completions, and refactors.
- 344fdba: Refactor internal rules, fixables, refactors, and completions to thread program,
  checker, and type parser state explicitly through shared contexts. Simplify the
  typescript-go hooks and move completion coverage onto the real fourslash-based
  language-service pipeline.

### Patch Changes

- 90adf4f: Add Effect v4 completion coverage for `ServiceMap.Service` class helpers, including package-aware key generation cases that match the upstream language-service fixtures.
- e209b5b: Report floating `Stream` expressions in the `floatingEffect` diagnostic for Effect v4, and add the matching diagnostic and quick-fix baselines.
- 22e8dcd: Sync Effect diagnostic wording with the updated language-service tone so diagnostic text stays neutral and factual while severity is controlled by configuration.

  This also refreshes generated metadata and committed diagnostic baselines to match the new emitted messages.

- 73a7ff0: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/a3eef87a57955a90a0d492b4bed9a7bab5d17838) to commit `a3eef87a57955a90a0d492b4bed9a7bab5d17838`.

## 0.1.1

### Patch Changes

- 9fa65d2: Update the setup CLI to detect existing `@typescript/native-preview` dependencies and preserve whether they are installed in `dependencies` or `devDependencies`.

  When enabling the language service, the setup flow now also adds `@typescript/native-preview@latest` if it is missing.

- 604119c: Update the automation so `refresh-flake-hash` runs as a reusable workflow after
  `update-typescript-go` completes validation, instead of depending on PR events
  triggered by the GitHub Actions bot.
- 6284611: Fix `effectFnImplicitAny` so it only checks the primary `Effect.fn` callback body instead of reporting helper callback parameters that are contextually typed by the `Effect.fn` result.

  This avoids false positives for secondary callbacks such as `Effect.fn(function* (...) { ... }, (effect, ...args) => ...)`.

- cb0d9bb: Fix the flake refresh workflows so TypeScript-Go submodule updates also refresh `flake.nix` and `flake.lock`.

  This keeps the Nix flake build inputs aligned with the checked-in submodule and generated shim state.

- f7584fa: Refactor typeparser and duplicate-package caching to keep checker-local cache state on `EffectLinks` instead of using process-global cache variables.

  This removes manual cache reset hooks and simplifies repeated package and type lookups without changing diagnostics behavior.

- ff3c088: Refactor typeparser package export matching to reuse shared package source-file descriptors and canonical checker symbol helpers.

  This removes repeated node-to-module export matching logic across Effect-related recognizers while preserving existing diagnostics and quick-fix behavior.

- 542440f: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/8a834dad086d6912b091e8b467e98499dab68cd9) to commit `8a834dad086d6912b091e8b467e98499dab68cd9`.

## 0.1.0

### Minor Changes

- 4477bfb: Add Effect v4 support for the `runEffectInsideEffect` diagnostic and quick fix.

  Nested `Effect.run*` calls inside generators now suggest and apply `Effect.run*With` fixes using extracted services.

### Patch Changes

- 5642de7: Fix `effectFnImplicitAny` so contextual union types suppress the diagnostic when any union member provides a callable contextual type.

  This aligns nested `Effect.fnUntraced` callbacks in union-typed APIs with TypeScript's `noImplicitAny` behavior.

## 0.0.20

### Patch Changes

- 46d9376: Add boolean plugin flags for Effect diagnostics, refactors, quickinfo, and completions, and honor them in the Go language-service hooks.
- 51d09a9: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/025d5aa3913ad54c5eae6be37677d3b85f783fd9) to commit `025d5aa3913ad54c5eae6be37677d3b85f783fd9`.

## 0.0.19

### Patch Changes

- cd9663a: Fix `tsgo` CLI suggestion filtering so `includeSuggestionsInTsc: false` is respected during command-line runs.

## 0.0.18

### Patch Changes

- d2ff6d8: Generate the README plugin configuration example from code metadata so the documented JSONC defaults stay aligned with the implementation and schema updates.
- 7a38643: Fix `@effect-diagnostics *:off` handling so only `skip-file` disables an entire file, allowing later rule-specific preview directives to re-enable diagnostics as in the upstream Effect language service.
- b851e0a: Align the Go editor setup with the repository lint configuration and expand Go lint coverage with additional correctness, modernization, and test-focused checks.
- af7a319: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/46ed96437ee4714316aa142176959f37905e91d6) to commit `46ed96437ee4714316aa142176959f37905e91d6`.

## 0.0.17

### Patch Changes

- cc49924: Add explicit ServiceMap coverage for the class self mismatch diagnostic.
- b1c4cac: Update the pinned `typescript-go` submodule to `a4325da30f285ff85b7b55afe1c65d74f54794af` and regenerate shims for the new upstream API surface.
- 47cb4bf: Ship package-specific README files with every published `@effect/tsgo` package.
- c7c86e1: Add back includeSuggestionsInTsc setting

## 0.0.16

### Patch Changes

- 7a94f7e: Update typescript-go to 50a70608. Upstream changes include auto-import fixes, linked editing support, signature help trigger characters, JSON syntax validation, formatting rule fixes, and various bug fixes.

## 0.0.15

### Patch Changes

- e1c3844: Prefer the property name for graphs and locations
- b8ff941: Handle existing prepare script

## 0.0.14

### Patch Changes

- 18c2262: Fix refactor trigger range

## 0.0.13

### Patch Changes

- 5dfeba1: Add more info to missingEffectContext
- 90b4919: Port severity selection

## 0.0.12

### Patch Changes

- 931ef77: Add document symbols

## 0.0.11

### Patch Changes

- 5d8164e: Skip typeatlocation for class ... implements .. X.Y.Z as well

## 0.0.10

### Patch Changes

- 19b0677: Update typescript-go to 03b31eb

## 0.0.9

### Patch Changes

- 8c7092a: Caching and perf allocations

## 0.0.8

### Patch Changes

- 454cae6: Add caching inside Checker

## 0.0.7

### Patch Changes

- 3f23d3d: Adjust layer links

## 0.0.6

### Patch Changes

- 594ad7a: Added completions

## 0.0.5

### Patch Changes

- f6da8fb: Fix issue caused by nested expression with type arguments in tsgo

## 0.0.4

### Patch Changes

- d42c0d2: Cache test runs properly
- e06f941: Align floatingEffect effect subtype behaviour

## 0.0.3

### Patch Changes

- cc6d58c: Update tsgo upstream

## 0.0.2

### Patch Changes

- 99ca88b: prepare oidc and trusted publishing setup

## 0.0.1

### Patch Changes

- d601f50: Fix the Nix flake build and keep setup-generated tsconfig plugin entries aligned with the Effect plugin name parsed by tsgo.
- 12dfcf7: Fix release workflow
