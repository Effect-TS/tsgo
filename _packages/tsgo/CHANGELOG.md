# @effect/tsgo

## 0.5.0

### Minor Changes

- d1f126e: Refresh execution-flow graph baselines after the latest graph relationship updates.

  This captures the current local flow outputs used by tests, including the updated graph edge semantics in execution-flow snapshots.

### Patch Changes

- 2f9e068: Fix the refresh flake hash workflow so scheduled TypeScript Go update runs check out the generated branch before refreshing the vendor hash.
- ab3cf34: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/83b8d2aa23b2d385087dabe5a5a8afd5e296013d) to commit `83b8d2aa23b2d385087dabe5a5a8afd5e296013d`.

## 0.4.0

### Minor Changes

- 923cf53: Improve execution-flow graphing for `Effect.gen` and generator-based `Effect.fn` calls by modeling `yield*` operands as yieldable links and preserving the generator result type through piped calls.

  This updates flow baselines for cases like `yield* Effect.succeed(1)` and `yield* Effect.fail(error)`, and exports the TypeScript-Go `forEachYieldExpression` helper through the checker shim for reuse.

### Patch Changes

- f23566c: Update the bundled `typescript-go` submodule to `8c45757f8` and refresh the local compatibility layer for the upstream hover and printer API changes.

  This updates the hover patch, regenerates shims, and adjusts local callers to the new `TypeToStringEx` and `SignatureToStringEx` signatures so setup, check, test, and lint continue to pass.

- 94d5134: Update the bundled `typescript-go` submodule to `cf6d69d83` and refresh the local compatibility layer for the upstream AST and language-service API changes.

  This includes a refreshed code-actions patch plus shim regeneration so repository setup, checks, tests, and lint all pass on the new upstream revision.

## 0.3.1

### Patch Changes

- c3b4084: Port the Effect context tracking refactor from the TypeScript reference implementation so diagnostics also recognize Effect constructor thunks such as `Effect.sync`, `Effect.promise`, `Effect.try`, and `Effect.tryPromise`.

  This updates related metadata and baselines and adds thunk-focused test coverage for both Effect v3 and v4 fixtures.

## 0.3.0

### Minor Changes

- 377d99c: Add `asyncFunction` and `newPromise` diagnostics to warn on `async` functions and manual `new Promise(...)` construction in favor of Effect-native async patterns.

  This ports the upstream language-service change into the Go implementation and adds matching v3/v4 fixtures, baselines, metadata, and README updates.

- 57c1b81: Add an execution-flow parser that models value flow more directly than the existing piping-flow parser and emits Mermaid flow baselines for Effect fixtures.

  This lays the groundwork for more precise diagnostics around nested, parenthesized, data-first, data-last, and function-pipe transformations while preserving richer flow structure for future rule analysis.

- 8e9578c: Add the `lazyPromiseInEffectSync` diagnostic for `Effect.sync` thunks that return the global `Promise<T>` type.

  This ports the upstream language-service behavior to the Go implementation, including v3/v4 examples, baselines, and exact Promise detection via TypeScriptGo checker shims.

- 8e26cfe: Add `cryptoRandomUUID` and `cryptoRandomUUIDInEffect` diagnostics for Effect v4 to warn on `crypto.randomUUID()` usage and prefer the Effect `Random` module.

  This ports the upstream language-service change into the Go implementation and adds matching v4 fixtures, baselines, metadata, and schema entries.

- 51c3283: Port the `effectDoNotation` diagnostic from the reference language service.

  This adds Effect v3 and v4 examples, generated metadata/schema updates, and committed baselines for diagnostics and disable-style code actions.

- 67f699d: Port the `effectMapFlatten` diagnostic from the reference language service.

  This adds Effect v3 and v4 examples, generated metadata and schema updates, and committed baselines for diagnostics, quick fixes, flows, layers, and pipings.

- 086dff3: Add the `nestedEffectGenYield` diagnostic for nested bare `yield* Effect.gen(...)` calls inside existing Effect generator contexts.

  This ports the upstream language-service behavior to the Go implementation, including v3/v4 examples, generated metadata, schema entries, and reference baselines.

- 7cffed0: Add the `unnecessaryArrowBlock` diagnostic and quick fix for arrow functions whose block body only returns an expression.

  This ports the upstream language-service behavior to the Go implementation, including v3/v4 examples, quickfix baselines, and generated metadata/schema documentation.

- dcb4af3: Add data-first and data-last piping flow normalization so data-first Effect and Layer APIs contribute the same flow structure as their pipeable forms.

  This also extracts the shared bundled Effect test VFS helper into `internal/bundledeffect` and updates the affected flow and diagnostics baselines.

- 5a8e7fa: Add `processEnv` and `processEnvInEffect` diagnostics to warn on `process.env` reads and recommend using Effect `Config` instead.

  This ports the upstream language-service change into the Go implementation and adds matching v3/v4 fixtures, baselines, metadata, and schema entries.

### Patch Changes

- 3cddb7c: Fix execution-flow graph generation for single-argument inline calls such as `Layer.succeed(Service)(value)`.

  This updates the flow parser to connect inline call subjects and transforms correctly, and refreshes the generated reference baselines and metadata outputs to match the new local results.

- e80be4f: Fix Effect v4 service parsing for `effect@4.0.0-beta.43` and update the embedded v4 test packages to that version.

  This keeps `ServiceMap.Service` detection working with the new `Identifier` / `Service` type shape while preserving the existing v3-only `Context.Tag` behavior.

- 41798ca: Fix the toggle-pipe-style refactor to avoid formatter panics on nested callback bodies such as SQL effects using `.pipe(Effect.flatMap(...))`.

  This adds a regression test and updates the affected refactor baselines to match the new text-preserving rewrite output.

- 3689458: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/c25de70d251d4b717a1cb6f4f6289d2e68fef159) to commit `c25de70d251d4b717a1cb6f4f6289d2e68fef159`.

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
