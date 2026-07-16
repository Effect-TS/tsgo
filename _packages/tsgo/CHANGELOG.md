# @effect/tsgo

## 0.23.0

### Minor Changes

- 3d62271: Add the `syncToSucceed` diagnostic and quick fix, which replaces `Effect.sync` thunks returning stable constant values with `Effect.succeed`.

### Patch Changes

- 8128a86: Avoid excessive type-depth diagnostics when checking complex non-Effect types such as the return type of Remeda's `chunk` function.
- ee2ca99: Update to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260715.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/448614f8f409cbd8afb535b26c20f1a0020bc10d) commit `448614f8f409cbd8afb535b26c20f1a0020bc10d`.

## 0.22.0

### Minor Changes

- f839997: Add the `schemaOpaqueInstanceMember` diagnostic to prevent instance members in classes extending `Schema.Opaque`.

### Patch Changes

- 7419aeb: Fix staticcheck nil pointer warnings in tests.
- 97d0011: Update to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260714.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/8a749379d556f1bb4044218a1f94bc90fbfb6a03) commit `8a749379d556f1bb4044218a1f94bc90fbfb6a03`.

## 0.21.0

### Minor Changes

- dc7e706: Add the off-by-default `missingPipeableSignature` diagnostic for auditing exported fixed-arity functions without corresponding pipeable overloads.

## 0.20.0

### Minor Changes

- e534733: Add an `effect-tsgo diagnostics` command for reporting Effect diagnostics from a file or TypeScript project. The command supports pretty, text, JSON, and GitHub Actions output, severity filtering, progress reporting, strict warning handling, and inline language-service configuration.

### Patch Changes

- 55d6cf8: Enable `skipLibCheck` in the Effect test harness to avoid type-checking bundled declaration files that are not under test.
- 4f212fb: Start layer magic compositions from `Layer.empty` when the first layer does not provide a requested output service.
- 79768a1: Ship the generated schema and configure `tsconfig.json` to use the installed local copy during setup, fixing #346.
- 070a8ac: Update `effect-tsgo setup` to configure the current TypeScript Go VS Code settings, including the workspace TypeScript SDK path and additional SDK locations.
- 228f6d5: Update to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260713.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/168e7015edf98244febc8f4ae450b673b5d195d7) commit `168e7015edf98244febc8f4ae450b673b5d195d7`.

## 0.19.0

### Minor Changes

- 16a52e5: Add the `flatMapToMap` diagnostic and quick fix, which replaces `Effect.flatMap` callbacks that only wrap their result with `Effect.succeed` with `Effect.map`. The diagnostic supports pipe, pipeable, data-first, and data-last forms.

## 0.18.1

### Patch Changes

- d6cdab2: Allow `effect-tsgo patch` to skip TypeScript package aliases that do not include package metadata needed for native binary matching.

## 0.18.0

### Minor Changes

- 308652f: Add `effect-tsgo patch --typescript-package <name>` and automatically fall back from `typescript` to `@typescript/native` when resolving the native TypeScript package to patch.

## 0.17.0

### Minor Changes

- 74a7ccb: Modernize release workflow publishing and GitHub Actions runtimes.

## 0.16.4

### Patch Changes

- d7f6e86: Expose additional generated shims for TypeScript-Go language-service and checker internals, and add an `etsapi` helper for rendering structural schema statements from a resolved type.
- 082a955: Update TypeScript-Go automation so `main` tracks `typescript@next`, while release `tsc` binaries are built from a generated `generated/latest` branch pinned to `typescript@latest`.
- 98bbce0: Fix layer magic ordering for unrelated layers so layers that only require services are composed after layers that provide no services.
- 5cfbe23: Update to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260708.3), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/52168999f3dcfc9205432d47f6f600051f02f1a2) commit `52168999f3dcfc9205432d47f6f600051f02f1a2`.
- d46803d: Publish TypeScript upstream metadata beside packaged binaries and select the patched binary whose TypeScript git head matches the installed `typescript` package, with `effect-tsgo patch --force` as an explicit fallback.

## 0.16.3

### Patch Changes

- 633fbd6: Update to [`@typescript/native-preview@7.0.0-dev.20260707.2`](https://www.npmjs.com/package/@typescript/native-preview/v/7.0.0-dev.20260707.2), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/9977d6d38fcc78de8ae71770f3aa08256e6cc861) commit `9977d6d38fcc78de8ae71770f3aa08256e6cc861`.

## 0.16.2

### Patch Changes

- 0b9960d: Fix checker panics on `import.defer(...)` calls and bindingless import clauses.

  `import.defer` parses as a meta property, and the checker debug-asserts (panics) when asked for its symbol or type while it is used as an import-call callee. Rules resolving arbitrary call callees (e.g. `catchUnfailableEffect`, `globalFetch`, `globalTimers`) crashed tsc and the LSP on files containing:

  ```ts
  import.defer("./module");
  ```

  Symbol resolution now goes through a guarded `TypeParser.GetSymbolAtLocation` wrapper that skips meta properties, and all rule/refactor/LSP call sites were audited to use it. `TypeParser.GetTypeAtLocation` gained the same meta-property guard, plus a guard for import clauses without a default binding (`import { A } from "x"`), which previously hit a nil-symbol panic that was silently recovered.

  Also adds rule sweep stress tests that run the every-node diagnostics (`anyUnknownInErrorContext`, `effectInFailure`) over the typescript-go compiler test corpus, the effect-v4 fixtures, and effect's own package sources under a watchdog, failing on panics or non-termination.

## 0.16.1

### Patch Changes

- c45a407: Fix `internal/effecttest` LSP test helpers broken by the `typescript-go` update: the untyped `SendRequestWorker` now returns the response result as a raw `json.Value`, so the inlay hint, diagnostic, and code action helpers decode it via `RequestInfo.UnmarshalResult` instead of type-asserting the typed response struct.
- e094fda: Make the EffectLinks checker patch apply across newer typescript-go commits by anchoring it to stable Checker fields.
- cb7d6bc: Avoid suggesting `unnecessaryEffectGen` when a single-return generator contains nested `yield*` expressions.
- 64343c5: Fix the release workflow embedding a stale `EffectVersion` in the published `tsc` binary. The version bump from the changeset release PR only lands on `main`, while the `tsc` binary builds from `generated/stable`; the workflow now syncs `_packages/tsgo/package.json` from the release merge commit and re-runs `_tools/version-prepare.sh` before building. All release checkouts are also pinned to the merge commit SHA instead of the moving `main` ref so the release is deterministic.
- abfa2ef: Update to [`@typescript/native-preview@7.0.0-dev.20260703.1`](https://www.npmjs.com/package/@typescript/native-preview/v/7.0.0-dev.20260703.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/acfaa5bcc8631d3c51ad65a8562a656c8d6a4bd5) commit `acfaa5bcc8631d3c51ad65a8562a656c8d6a4bd5`.

## 0.16.0

### Minor Changes

- dbc279b: Add an ETS API helper for extracting layer magic from caller-provided layer nodes, and allow layer graph extraction to start from multiple nodes without exploding expressions.
- f5da105: Add the `catchToIgnore` diagnostic, which suggests `Effect.ignore` or `Effect.ignoreCause` when `Effect.catch` or `Effect.catchCause` returns `Effect.void` on a void success channel.

### Patch Changes

- 8078f7a: Add a public `etsapi` package exposing a narrow wrapper around the internal type parser for Effect, Layer, Stream, service, Context.Tag, Schema, union member, and YieldableError type inspection.

## 0.15.0

### Minor Changes

- cc68e52: Update TypeScript-Go and expose the internal sourcemap package through generated shims.
- af2ef42: Add support for the `typescript` package (>= 7, e.g. the 7.0 RC) as a native backend, alongside the existing `@typescript/native-preview` backend.

  `effect-tsgo patch`/`unpatch` now resolve the native TypeScript binary from whichever backend is installed: `@typescript/native-preview` is tried first (back-compat), then `typescript` >= 7, whose Go binary ships as `lib/tsc` under the `@typescript/typescript-<plat>-<arch>` platform sub-package. A version gate ensures `typescript` < 7 (the JavaScript compiler) is never treated as a native backend.

  `effect-tsgo setup` recognises an existing `typescript` >= 7 install as the native backend so it no longer redundantly re-adds `@typescript/native-preview`, writes the correct backend package to `package.json`, and points the VS Code `typescript.native-preview.tsdk` setting at `node_modules/typescript` for the `typescript` backend (vs `node_modules/@typescript/native-preview` otherwise).

  Before, a project using `typescript@^7.0.1-rc` without `@typescript/native-preview` failed with `NativePreviewNotInstalledError`:

  ```
  $ effect-tsgo patch
  ERROR: NativePreviewNotInstalledError: @typescript/native-preview is not installed.
  ```

  After, the same project patches successfully against the `typescript` >= 7 binary.

### Patch Changes

- 3b0c977: Make generated stable branch pull requests commit on top of `generated/stable` to avoid merge conflicts with `main`.
- 61cfb05: Remove the unused native `getEffectDiagnostics` API hook from the patched TypeScript-Go API.
- 7bb4780: Update to [`@typescript/native-preview@7.0.0-dev.20260628.1`](https://www.npmjs.com/package/@typescript/native-preview/v/7.0.0-dev.20260628.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/f7c4664176a1ac8be4ba7b4981d0b17c0457a74c) commit `f7c4664176a1ac8be4ba7b4981d0b17c0457a74c`.

## 0.14.6

### Patch Changes

- 070f2b5: Publish platform packages with both the `tsgo` binary built from `main` and the `tsc` binary built from `generated/stable`.
- ef53ddd: Add automation that publishes a generated `generated/stable` branch pinned to the TypeScript RC `typescript-go` commit and runs CI on that branch.
- eb0dbbc: Update the generated stable branch automation to open a pull request and validate it before merging.
- ef0a5a0: Fix the automated TypeScript-Go update workflow to pin the submodule to the commit shipped by `@typescript/native-preview@latest`.
- fb3096f: Update to [`@typescript/native-preview@7.0.0-dev.20260620.1`](https://www.npmjs.com/package/@typescript/native-preview/v/7.0.0-dev.20260620.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/dc37b5249ab60e2bbce936f71b883e6c8136167e) commit `dc37b5249ab60e2bbce936f71b883e6c8136167e`.
- 4423d1d: Make the TypeScript-Go version suffix patch apply across upstream TypeScript version literal changes.

## 0.14.5

### Patch Changes

- 7f0ca5d: Update TypeScript Go to microsoft/typescript-go@2fb5d4ce13935aef1f3c2896fad76d0ab4d43604.

## 0.14.4

### Patch Changes

- d12134c: Update Effect v4 package references and embedded test fixtures to `4.0.0-beta.83`.
- 2af8382: Update the pinned TypeScript-Go submodule to `551b02d6c10e78610fc5ca7c23b77c9d531ee59b` and refresh generated shims.

## 0.14.3

### Patch Changes

- 011b2fa: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/f20e9196e66c08fd0f752c6fca02c9b45df474eb) to commit `f20e9196e66c08fd0f752c6fca02c9b45df474eb`.

## 0.14.2

### Patch Changes

- ccb9779: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/69b0d53976ef291fa0bb2b636ec1b557a015f6b3) to commit `69b0d53976ef291fa0bb2b636ec1b557a015f6b3`.

## 0.14.1

### Patch Changes

- 498c445: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/254e9a5331fe7e08a8303deecc45521f98e1e5f9) to commit `254e9a5331fe7e08a8303deecc45521f98e1e5f9`.

## 0.14.0

### Minor Changes

- c5a1e55: Add a `schemaNumber` suggestion diagnostic for Effect v4 that recommends `Schema.Finite` and `Schema.FiniteFromString` over number schemas that also accept `NaN` and infinities.

### Patch Changes

- 95d3819: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/69d658da1a632a723275439339e34842d4d8f401) to commit `69d658da1a632a723275439339e34842d4d8f401`.

## 0.13.2

### Patch Changes

- 9819ab5: Update `typescript-go` to commit `70b87f0321dea818af0a648809e2aaf06ec3f6a1` and refresh patches and generated shims for upstream API changes.
- 2619351: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/40f6707e0ae5474907c77bf25ec83c171a55df90) to commit `40f6707e0ae5474907c77bf25ec83c171a55df90`.

## 0.13.1

### Patch Changes

- 6d2a846: Expose additional TypeScript-Go shims for module resolution and language service quick info documentation.
- 227abe1: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/fe932f69bf61cb1b1c22bd936663eb2c994e8a81) to commit `fe932f69bf61cb1b1c22bd936663eb2c994e8a81`.

## 0.13.0

### Minor Changes

- 5c766f1: Add the `redundantOrDie` diagnostic, which suggests hoisting repeated trailing `Effect.orDie` calls from every yielded effect to the generator result.

## 0.12.0

### Minor Changes

- 1d8c203: Add the `catchToOrElseSucceed` diagnostic, which suggests `Effect.orElseSucceed` for `Effect.catch(() => Effect.succeed(value))` in Effect v4 and `Effect.catchAll(() => Effect.succeed(value))` in Effect v3.

## 0.11.5

### Patch Changes

- bf71a50: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/0970dc40fa8308ca76627ffcc3a992414fdf1cf2) to commit `0970dc40fa8308ca76627ffcc3a992414fdf1cf2`.

## 0.11.4

### Patch Changes

- 286456b: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/94f31f32f8b5713e8fb17aaa65ba73b596485459) to commit `94f31f32f8b5713e8fb17aaa65ba73b596485459`.

## 0.11.3

### Patch Changes

- d5c3765: Fix the lazyEffect diagnostic to allow generic zero-argument functions returning Effect values.

## 0.11.2

### Patch Changes

- 7499b7d: Fix shim generation for mirrored struct fields that reference unexported named types.
- 8352a3a: Fix the automated TypeScript-Go update workflow so it tracks submodule commit changes while still ignoring local patched worktree noise inside the `typescript-go` submodule.
- d2a35cf: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/c54ff7cc5528734fd303461719b54b70115f5445) to commit `c54ff7cc5528734fd303461719b54b70115f5445`.

## 0.11.1

### Patch Changes

- d18a012: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/2b43ca072d8e9ba7cefb006520b47d7dfc642066) to commit `2b43ca072d8e9ba7cefb006520b47d7dfc642066`.

## 0.11.0

### Minor Changes

- ad2dd86: Add the `newSchemaClass` diagnostic for Effect v4 to discourage constructing Schema classes with `new` and suggest using `SchemaClass.make(...)` instead.

  Example:

  ```ts
  class User extends Schema.Class<User>("User")({
    name: Schema.String,
  }) {}

  const user = new User({ name: "John" });
  ```

  Now reports `newSchemaClass` and can be rewritten to:

  ```ts
  const user = User.make({ name: "John" });
  ```

### Patch Changes

- b6792a1: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/d99f6b2a5ff82ab5ca80cb679d59600c90bc8f05) to commit `d99f6b2a5ff82ab5ca80cb679d59600c90bc8f05`.

## 0.10.0

### Minor Changes

- db13c5e: Add the `multipleCatchTag` diagnostic to suggest collapsing consecutive `catchTag` transformations into `catchTags` when the chain is uninterrupted and semantics remain equivalent.

### Patch Changes

- 783a75a: Extend the `lazyEffect` diagnostic to also detect lazy `Layer` returns and mention the concrete lazy type in its message.

## 0.9.0

### Minor Changes

- d95c7dc: Add the `lazyEffect` diagnostic for exported zero-argument functions, exported interface members, and `Context.Service` members that lazily return `Effect` or `Stream` values.

### Patch Changes

- eda19ad: Update `preferSchemaOverJson` to be off by default, while keeping the effect-native preset at warning and improving the diagnostic guidance for Effect v3 and v4 projects.

## 0.8.0

### Minor Changes

- 2c8da48: Add the `unnecessaryTypeofType` style diagnostic and quick fix.

  This suggests replacing schema-style annotations such as `typeof UserId.Type`
  with the matching named type when that type is available and equivalent.

  Examples:

  ```ts
  import { UserId } from "./schemas";

  const a: typeof UserId.Type = {};
  ```

  becomes:

  ```ts
  import { UserId } from "./schemas";

  const a: UserId = {};
  ```

  It also supports qualified and namespace-imported names such as
  `typeof UsersRepo.User.Type` and `typeof Schemas.UsersRepo.User.Type`.

## 0.7.5

### Patch Changes

- 49c33b7: Add the `redundantMapError` diagnostic for hoistable repeated `Effect.mapError(...)` usage in `Effect.gen` and generator-form `Effect.fn`.
- f61c7b1: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/2cf002eed790973677ee285b54bd1687ac2c76cb) to commit `2cf002eed790973677ee285b54bd1687ac2c76cb`.

## 0.7.4

### Patch Changes

- b28097a: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/0a7c6b47e6163892880e0e5814e519c435445f11) to commit `0a7c6b47e6163892880e0e5814e519c435445f11`.

## 0.7.3

### Patch Changes

- 2cc75eb: Update the pinned `typescript-go` submodule to `f3911d25a18e746c1d8ec68b175350c2b333da6d` and rebase local patch files so setup and validation continue to pass.

## 0.7.2

### Patch Changes

- c078d7b: Avoid triggering stray TypeScript diagnostics while detecting data-first and data-last Effect APIs during piping-flow analysis.

## 0.7.1

### Patch Changes

- dba3fe7: Update the auto-import style consistency tests to use the mounted Effect fixtures and assert the full rewritten fix shapes for barrel and namespace import behavior.
- a608a39: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/c282336ea94f8a2e63187afd0f8d4bfb3b3f4d3b) to commit `c282336ea94f8a2e63187afd0f8d4bfb3b3f4d3b`.

## 0.7.0

### Minor Changes

- 50753fe: Add a native `getEffectDiagnostics` API entrypoint that runs Effect diagnostics for a specific source file with explicit rule selection and Effect options.

  This also exposes a shared internal rule runner so the checker hook and native API use the same directive, severity, and override behavior.

## 0.6.2

### Patch Changes

- 0af7700: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/01cc06960f62a508d26232175e0fcfd16846ed4f) to commit `01cc06960f62a508d26232175e0fcfd16846ed4f`.

## 0.6.1

### Patch Changes

- ef1d235: Update Effect v4 test coverage and metadata for `4.0.0-beta.66`, including direct `Context.Service` yielding and the restored `Effect.firstSuccessOf` API.
- ad7a8fa: Extend unsafe effect type assertion diagnostics to Stream and Layer assertions.
- 3316083: Update the TypeScript-Go submodule to include upstream declaration emit fixes.

## 0.6.0

### Minor Changes

- 3512164: Add the `unsafeEffectTypeAssertion` diagnostic and quick-fix to detect assertions that unsafely narrow Effect error or requirements channels.

  This also ports the matching v3/v4 examples, preview coverage, and baselines for the new rule.

### Patch Changes

- 2a4738a: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/092b34f534182baf2875887c20ffed2177f14d92) to commit `092b34f534182baf2875887c20ffed2177f14d92`.

## 0.5.4

### Patch Changes

- de8fb53: Fix false-positive `TS2683` diagnostics for `Effect.gen({ self: this }, ...)` by avoiding eager call-signature analysis in affected Effect contexts.

  This includes nested `Effect.gen` generic calls plus related cases such as `this` in callees, `Effect.sync`/`Effect.tryPromise` callbacks, `.pipe()` chains, and curried wrappers.

- 54da1c5: Mount real Effect packages in fourslash tests when `@effect-v3` or `@effect-v4` markers are present, and cover Effect auto-import namespace behavior against the real mounted package.
- 9bab9e4: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/18f93b85d447e24e9a03188dd630e7395cad5fe1) to commit `18f93b85d447e24e9a03188dd630e7395cad5fe1`.

## 0.5.3

### Patch Changes

- 8336e7c: Update the pinned `typescript-go` submodule to `1e58c8419142e35f338840dc50822c48dcc4ec1f` and refresh the local patch metadata needed to apply cleanly on that upstream revision.

  Regenerate shim bindings for upstream API changes introduced by the submodule update.

## 0.5.2

### Patch Changes

- f1c940b: Skip Effect diagnostics for source files resolved from external libraries.
- 561c053: Fix a false-positive `TS2683` when using `this` inside directly yielded expressions in `Effect.gen({ self: this })`.

  This avoids losing contextual `this` typing during data-first call analysis for Effect generator code such as `yield* Scope.close(this.#scope, Exit.void)`.

- 1a562ee: Fix `@effect/language-service` activation when the plugin is inherited through multiple `tsconfig` `extends` hops.

  Effect diagnostics now continue to work for config chains like `tsconfig.json -> worker.json -> base.json` without duplicating the plugin stanza in intermediate configs.

- 7c153f4: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/2de4a3f1746f614fe5a19deec6a4ad9d0640d67a) to commit `2de4a3f1746f614fe5a19deec6a4ad9d0640d67a`.

## 0.5.1

### Patch Changes

- c033c7e: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/98f57c93a4280a2378b19b59ac6bba7d279ed61a) to commit `98f57c93a4280a2378b19b59ac6bba7d279ed61a`.

## 0.5.0

### Minor Changes

- d1f126e: Refresh execution-flow graph baselines after the latest graph relationship updates.

  This captures the current local flow outputs used by tests, including the updated graph edge semantics in execution-flow snapshots.

### Patch Changes

- 2f9e068: Fix the refresh flake hash workflow so scheduled TypeScript Go update runs check out the generated branch before refreshing the vendor hash.
- 8869556: Reduce typeparser property lookup overhead by using direct property type access for Effect-related type detection, and add a regression test covering the plugin-only TS2589 failure path in `effectInFailure`.
- ab3cf34: Update [`typescript-go`](https://github.com/microsoft/typescript-go/commit/1de8d68230f8759af6fb71d9cf0f9c37d2c65507) to commit `1de8d68230f8759af6fb71d9cf0f9c37d2c65507`.

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
