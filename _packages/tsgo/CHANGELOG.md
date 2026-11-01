# @effect/tsgo

## 0.64.0

### Minor Changes

- e3b5db8: Add the `brandedSchemaClasses` plugin option, which lists the Schema class kinds that get a nominal brand (`{ readonly brand: unique symbol }`). The values are the Schema API names (`"Class"`, `"TaggedClass"`, `"Error"`, `"TaggedError"`, `"Opaque"`). The default is all five, so nothing changes unless the option is set. Like other per-file options, it can also be set in `overrides`.
  
  - `schemaClassMissingBrand` only reports classes whose kind is in the list.
  - The `Schema.` extends-clause completions only insert the brand for kinds in the list.
  - An empty list means the rule never reports and the completions never insert a brand.
  
  For example, to stop asking for a brand on tagged classes and tagged errors, whose literal `_tag` already keeps different classes apart:
  
  ```jsonc
  {
    "compilerOptions": {
      "plugins": [
        {
          "name": "@effect/language-service",
          "brandedSchemaClasses": ["Class", "Error", "Opaque"]
        }
      ]
    }
  }
  ```
  
  ```ts
  export class User extends Schema.Class<User>("User")({ name: Schema.String }) {}
  //                                     ^^^^ Add `{ readonly brand: unique symbol }` as the second type argument. ...
  export class Created extends Schema.TaggedClass<Created>()("Created", { id: Schema.String }) {}
  // no diagnostic: TaggedClass is not in brandedSchemaClasses
  ```
- f2af3cd: Piping flows now treat calls that pass optional arguments as transformation steps. A call counts when its callee has a single call signature with exactly one required parameter and no rest parameter: the first argument is the subject and the remaining optional arguments are kept with the step. Effect runners with options, `Effect.all` with options, and decoders applied with parse options no longer break a flow.
  
  ```ts
  Effect.runPromise(Effect.exit(program), { signal })
  // before: the flow ended at Effect.exit(program)
  // after:  program -> Effect.exit -> Effect.runPromise({ signal })
  ```
  
  Rules built on piping flows pick this up:
  
  - `missedPipeableOpportunity` keeps a runner and its options outside the suggested pipe, and still reports the inner calls when the runner returns a function, as `Effect.runCallback` does.
  
    ```ts
    Effect.runPromise(Effect.asVoid(Effect.orDie(program)), { signal })
    // quick fix: Effect.runPromise(program.pipe(Effect.orDie, Effect.asVoid), { signal })
    ```
  
  - `raceFirstWithSleepToTimeout` no longer reports `Effect.raceAllFirst` called with options such as `onWinner`, which `Effect.timeoutOrElse` cannot carry over.
  - `runOfExitToRunExit`, `preferTypedSchemaDecoder`, `allOfMapToForEach` and `strictEffectProvide` now read piping flows instead of walking the whole file. Their results are unchanged, except that `strictEffectProvide` no longer reports a standalone `Effect.provide(layer)` that is not applied to an Effect, such as `const withLive = Effect.provide(AppLive)`.
- d757902: Add `effectSucceedWithUndefined` (Effect v4, style, suggestion): suggests `Effect.undefined` instead of `Effect.succeed(undefined)` or `Effect.succeed(void 0)`, keeping the `Effect<undefined>` type that the `Effect.void` rewrite would widen to `Effect<void>`.
  
  `effectSucceedWithUndefined` and `effectSucceedWithVoid` now pick the replacement from the resolved success type instead of the spelling of the argument, so each quickfix keeps the type unchanged. Both also report the pipe form, and the quickfix keeps the later steps.
  
  ```ts
  // Effect v4
  Effect.succeed(undefined)                      // effectSucceedWithUndefined -> Effect.undefined
  Effect.succeed(void 0)                         // effectSucceedWithUndefined -> Effect.undefined
  pipe(undefined, Effect.succeed, Effect.as(1))  // effectSucceedWithUndefined -> pipe(Effect.undefined, Effect.as(1))
  Effect.succeed<void>(undefined)                // effectSucceedWithVoid -> Effect.void
  Effect.succeed<number | undefined>(undefined)  // not reported
  
  // Effect v3 (no Effect.undefined)
  Effect.succeed(undefined)                      // effectSucceedWithVoid -> Effect.void
  Effect.succeed<number | undefined>(undefined)  // no longer reported
  ```
  
  Piping flow analysis now starts a new flow at each value argument of a data-first call, such as the second argument of `Effect.andThen(eff, Effect.succeed(1).pipe(Effect.map(f)))`. Before, these arguments were split into a truncated head and fake flows for their pipe steps, so flow-based diagnostics missed patterns there.
- 75ff7a1: Add the `effectTryWithoutCatch` diagnostic (antipattern, off by default, Effect v3 and v4). It reports short-form `Effect.try(thunk)` / `Effect.tryPromise(thunk)` calls, whose failures surface as the generic `Cause.UnknownError` (v4) / `Cause.UnknownException` (v3), and points to the object form, which maps the cause to a typed, domain-specific error:
  
  ```ts
  // reported
  Effect.try(() => JSON.parse(text))
  // preferred
  Effect.try({ try: () => JSON.parse(text), catch: (cause) => new ParseError({ cause }) })
  ```
  
  Calls are found through piping flows, so the call can also be a data-first argument, a pipe subject, a step inside `Effect.gen` / `Effect.fn`, or (on Effect v3, where it compiles) a pipe step such as `pipe(thunk, Effect.try)`.
- 5fabef2: Add the `globalJson` and `globalJsonInEffect` diagnostics (Effect-native, off by default, Effect v3 and v4). Like `globalRandom` / `globalRandomInEffect`, they report calls to the global `JSON.parse` and `JSON.stringify` outside and inside Effect code (`Effect.gen`, `Effect.fn`, and thunks such as `Effect.sync`, `Effect.try` and `Effect.tryPromise`), with the Schema replacement for the Effect version in use. Aliases of `JSON` are reported; a shadowed local `JSON` is not.
  
  ```ts
  // globalJson: JSON.parse throws instead of returning a failure
  const parseUserResult = (text: string) => Schema.decodeUnknownResult(User)(JSON.parse(text))
  //                                                                         ^^^^^^^^^^^^^^^^
  // This code uses `JSON.parse`, which throws instead of failing with a typed error. Use `Schema.fromJsonString(Schema.Unknown)` for unknown shapes, `Schema.fromJsonString(schema)` for known ones, or `Schema.toCodecJson(schema)` when working with JSON values instead of strings.
  
  // globalJsonInEffect
  const program = Effect.gen(function*() {
    const json = JSON.parse(text)
    //           ^^^^^^^^^^^^^^^ This Effect code uses `JSON.parse`, ...
  })
  ```
  
  `preferSchemaOverJson` is deprecated in favor of `globalJsonInEffect` and `globalJson`. It keeps working with its own message and reports what `globalJsonInEffect` reports, and the `effect-native` preset no longer includes it. Configuring a deprecated rule in `diagnosticSeverity` (or in an override) now reports the new `deprecatedRuleName` diagnostic on the tsconfig key, once per key:
  
  ```
  Effect diagnostic rule `preferSchemaOverJson` is deprecated; use `globalJsonInEffect` or `globalJson` instead. effect(deprecatedRuleName)
  ```
  
  Set `"deprecatedRuleName": "off"` to silence it or `"deprecatedRuleName": "error"` to raise its severity.
- d79c97a: Add the `mapSomeCatchNoSuchElement` diagnostic and quick fix for Effect v4. It suggests `Effect.catchNoSuchElement` when a piping flow wraps successes with `Effect.asSome` / `Effect.map(Option.some)` and immediately catches `NoSuchElementError` into `Option.none()`. It only reports when every error that could carry the `"NoSuchElementError"` tag is the real `Cause.NoSuchElementError`, and the handler ignores the error.
  
  ```ts
  // before
  findUser(id).pipe(
    Effect.map(Option.some),
    Effect.catchTag("NoSuchElementError", () => Effect.succeed(Option.none()))
  )
  // after
  findUser(id).pipe(Effect.catchNoSuchElement)
  ```
- 6e902fd: Add the `provideMergeAllToArray` style diagnostic (off by default, Effect v3 and v4). `Effect.provide` accepts an array of layers and merges them itself, so wrapping them in `Layer.mergeAll` adds nothing. The quick fix "Pass the layers as an array to Effect.provide" rewrites the argument to an array, keeping each layer's text, spreads, comments, and any v4 `options` argument.
  
  ```ts
  // before
  program.pipe(Effect.provide(Layer.mergeAll(ALayer, BLayer, CLayer)))
  Effect.provide(program, Layer.mergeAll(ALayer, ...BCLayers))
  
  // after quick fix
  program.pipe(Effect.provide([ALayer, BLayer, CLayer]))
  Effect.provide(program, [ALayer, ...BCLayers])
  
  // not reported
  const AppLayer = Layer.mergeAll(ALayer, BLayer)
  program.pipe(Effect.provide(AppLayer))
  ```
- a5ad860: Add the `resultMatchToFromResult` style diagnostic (Effect v4). It suggests `Effect.fromResult` when `Result.match` or a `Result.isSuccess`/`Result.isFailure` ternary only lifts the success into `Effect.succeed` and the failure into `Effect.fail`, and offers a quick fix.
  
  ```ts
  // before
  const port = Result.match(parsePort("8080"), {
    onFailure: (error) => Effect.fail(error),
    onSuccess: (value) => Effect.succeed(value)
  })
  const port2 = Result.isSuccess(result) ? Effect.succeed(result.success) : Effect.fail(result.failure)
  
  // after
  const port = Effect.fromResult(parsePort("8080"))
  const port2 = Effect.fromResult(result)
  ```
  
  Handlers and branches may also be written as pipe flows, such as `(value) => pipe(value, Effect.succeed)` or `pipe(result.success, Effect.succeed)`, and `Result.match` is found in every piping form: data-first, data-last, `pipe`, `.pipe`, as a data-first value argument, and inside `Effect.fn` bodies.
  
  The rule does not trigger when a handler changes the value or the error, uses explicit type arguments, recovers or swaps the channels, or when the handlers object isn't an inline literal.
- bdd68f5: `runEffectInsideEffect` now reports `runMain` from `@effect/platform-node`, `@effect/platform-bun` and `@effect/platform-browser` when it is called inside an Effect, such as an `Effect.gen` body or an `Effect.sync` thunk. That call starts a second root runtime, so the outer program cannot interrupt it, see its failures, or close its scope. The `runMain` diagnostic has no quick fix: return the Effect and call `runMain` once at the application entry point.
  
  The rule now also reports runners in pipe position, for both `Effect.run*` and `runMain`. The quick fix for `Effect.run*` works in pipe position too, and keeps optional arguments such as `{ signal }`. In Effect v3 it now writes the curried `Runtime.runPromise(effectRuntime)(effect)` form.
  
  ```ts
  import { NodeRuntime } from "@effect/platform-node"
  import { Effect } from "effect"
  
  const serve = Effect.log("serving")
  
  // reported
  export const handler = Effect.sync(() => {
    NodeRuntime.runMain(serve)
  })
  
  export const piped = Effect.gen(function*() {
    yield* Effect.log("starting")
    const run = () => serve.pipe(Effect.runFork)
    // quick fix: serve.pipe(Effect.runForkWith(effectContext))
    return run
  })
  
  // not reported
  NodeRuntime.runMain(serve)
  ```
- 001eb76: Add the `suspendFailToFailSync` diagnostic, which suggests `Effect.failSync` instead of an `Effect.suspend` whose thunk only returns `Effect.fail`. On Effect v3 it also suggests `Effect.dieSync` for a thunk that only returns `Effect.die`; Effect v4 has no `Effect.dieSync`. Both forms build the error lazily, so the quick fix keeps the behavior.
  
  ```ts
  // before
  Effect.suspend(() => Effect.fail(new ValidationError({ input })))
  // after
  Effect.failSync(() => new ValidationError({ input }))
  ```
  
  It works on piping flows, so the suspend and the fail can each be a call or a pipe step, and the quick fix keeps the shape:
  
  ```ts
  // before
  pipe(() => pipe(error, Effect.fail), Effect.suspend)
  // after
  pipe(() => error, Effect.failSync)
  ```
  
  The rule does not report thunks that do other work, such as a conditional or a statement before the return. When the rewrite would narrow the inferred error type, as with `Effect.fail(cond ? "A" : "B")`, it reports without a fix.
- 73164fc: Add the `typeUnionToSchemaUnion` diagnostic (style group, off by default, Effect v3 and v4). It reports a type alias that is a union of Schema `.Type` (or `.Encoded`) types with no runtime schema behind it, and offers a "Convert to Schema.Union" quickfix:
  
  ```ts
  // before
  export type Shape = typeof Circle.Type | typeof Square.Type
  // after (v4; v3 emits Schema.Union(Circle, Square))
  export const Shape = Schema.Union([Circle, Square])
  export type Shape = typeof Shape.Type
  ```
  
  `Schema.Schema.Type<typeof S>`, `Schema.Schema.Encoded<typeof S>` (v3) and `Schema.Codec.Encoded<typeof S>` (v4) members are also recognized. No fix is offered when a member schema is declared later in the file.
- f8c0a17: Add the `yieldInCallArgument` diagnostic (style, off by default). It reports a `yield*` passed directly as an argument of a call or `new` expression inside `Effect.gen`, `Effect.fn` or `Effect.fnUntraced`, and offers a quick fix that hoists the yield into a const:
  
  ```ts
  // before
  const result = transform(yield* load)
  // after
  const loaded = yield* load
  const result = transform(loaded)
  ```
  
  The fix is offered only when nothing evaluated before the `yield*` has side effects (identifiers, literals and property accesses count as safe), and not in conditional positions or loop conditions, optional calls or unbraced bodies. In those cases the diagnostic is still reported.

### Patch Changes

- d79c97a: Quick fixes that collapse several piping flow steps into one (`timeoutCatchTagToTimeoutOrElse`, `mapSomeCatchNoSuchElement`) now keep source comments. Comments on kept steps stay with them, and comments on the replaced steps move to the replacement.
  
  ```ts
  // before
  lookup.pipe(
    // wrap successes
    Effect.asSome,
    Effect.catchTag("NoSuchElementError", () => Effect.succeedNone)
  )
  // after the fix, the comment is kept
  lookup.pipe(
    // wrap successes
    Effect.catchNoSuchElement
  )
  ```
- cecfbd9: Speed up the test suite (about 2.3x on a 4-core CI runner) without changing any baseline:
  
  - the test VFS answers lookups of missing paths directly instead of scanning every mounted file, which module resolution does for each extension it probes;
  - quick-fix baselines compute each fix's result locally instead of applying and undoing it on the language server, so a test keeps one program and checker;
  - tests no longer force a full GC after every program; `repoctl test` bounds each test binary's heap with `GOMEMLIMIT=2GiB` instead;
  - the diagnostics runner (Go and Rust) reads the Effect packages from a VFS shared by all tests, layered under each test's own files, instead of copying them into every test's VFS.
- 6e822b3: Consolidate the Go test suite (about 14,000 fewer lines of Go test code) without changing any diagnostic behaviour:
  
  - move scenario-style Go tests into `testdata/tests` fixtures (about 50 new fixtures), so their behaviour is pinned by baselines and also runs against the Rust port; for example the `rules` and `rulerunner` shape tests of `flatMapToTap`, `mapConstToAs` or `apiStabilityLeak` now live in fixtures such as `flatMapToTap_malformedCallbacks.ts`;
  - remove tests duplicated across `internal/rules`, `internal/rulerunner` and `internal/effecttest`, and tautological tests;
  - merge near-identical tests into table tests and share one "each quick fix still compiles" helper;
  - remove narrating comments from test files;
  - remove the native Array/Object/String method inventory docs;
  - move the Rust port documentation (`PORTING.md`, `TSRS_PORT.md`, `TSRS_API.md`, `DIVERGENCES.md`) from `docs/` to `porting/`.
- a5ad860: `catchDieToOrDie`, `optionMatchToFromOption` and `mapSomeToAsSome` now recognize identity-forwarding handlers written as a pipe flow, not only as a direct call.
  
  ```ts
  // now reported, like (error) => Effect.die(error)
  failable.pipe(Effect.catch((error) => pipe(error, Effect.die)))
  numberEffect.pipe(Effect.map((value) => pipe(value, Option.some)))
  Option.match(option, { onNone: () => Effect.fail(new MissingValue()), onSome: (value) => pipe(value, Effect.succeed) })
  ```
- 9e38776: Detect `lazyPromiseInEffectSync` and `scopeInLayerEffect` through piping flows instead of a hand-rolled AST walk, so both rules now also report point-free, pipeable and curried forms.
  
  `lazyPromiseInEffectSync` now reports `Effect.sync` used point-free in a pipe:
  
  ```ts
  // now reported, in addition to Effect.sync(() => Promise.resolve(1))
  export const fromPointFree = pipe(() => Promise.resolve(1), Effect.sync)
  export const fromPointFreeChain = pipe(1, (n) => () => Promise.resolve(n), Effect.sync)
  ```
  
  `scopeInLayerEffect` now reports the pipeable and curried `Layer.effect` forms, and its "Use scoped for Layer creation" quick fix rewrites them to `Layer.scoped`:
  
  ```ts
  // now reported, in addition to Layer.effect(MyService, eff)
  export const pipeableLayer = eff.pipe(Layer.effect(MyService))
  export const curriedLayer = Layer.effect(MyService)(eff)
  ```
- 1b70991: Remove the unused execution flow graph parser (`TypeParser.ExecutionFlow`) from the Go build and the Rust port. No rule, refactor or language service feature used it. The test runner no longer writes the `.flows.txt` and `.flows.*.mermaid` baselines. Diagnostics and other user-facing behavior do not change.
- b1c0172: `strictEffectProvide` no longer reports `Effect.provide(layer)` when the next step of the piping flow runs the Effect, as an application entry point does. The recognized runners are `Effect.runPromise`, `runSync`, `runFork`, `runPromiseExit`, `runSyncExit`, `runCallback`, and `runMain` from `@effect/platform-node`, `@effect/platform-bun` and `@effect/platform-browser`.
  
  ```ts
  // not reported
  program.pipe(Effect.provide(AppLayer), Effect.runPromise)
  program.pipe(Effect.provide(AppLayer), NodeRuntime.runMain)
  Effect.runPromise(program.pipe(Effect.provide(AppLayer)))
  NodeRuntime.runMain(Effect.provide(program, AppLayer))
  
  // still reported
  export const handler = program.pipe(Effect.provide(AppLayer))
  Effect.runPromise(program.pipe(Effect.provide(AppLayer), Effect.asVoid))
  ```
- 5b31a36: Refactor the internal type parser API around parse methods. Rules no longer pair a syntactic "can this node be an Effect library type" guard with `GetTypeAtLocation` by hand: new `XxxAtLocation(node)` parsers (`EffectTypeAtLocation`, `StrictEffectTypeAtLocation`, `LayerTypeAtLocation`, `StreamTypeAtLocation`, `EffectSchemaTypesAtLocation`, `ServiceTypeAtLocation`, `ContextTagAtLocation`, `EffectYieldableTypeAtLocation`, `HasEffectTypeIdAtLocation`) skip computing the type of literals, functions and other nodes that can never be one, and the guard is now private.
  
  ```go
  // before
  if tp.NodeCannotBeEffectLibraryType(arg) {
  	continue
  }
  argType := tp.GetTypeAtLocation(arg)
  if argType == nil || tp.LayerType(argType) == nil {
  	continue
  }
  
  // after
  if tp.LayerTypeAtLocation(arg) == nil {
  	continue
  }
  ```
  
  The boolean wrappers `IsEffectType`, `StrictIsEffectType`, `IsEffectSubtype`, `IsFiberType`, `IsLayerType`, `IsServiceType`, `IsContextTag` and `IsSchemaType` are removed in favour of their parse methods, and `IsArrayType`, `IsScopeType` and `IsYieldableErrorType` become `ArrayType` (returning the element type), `ScopeType` and `YieldableErrorType`. A new `typeatlocation` custom lint reports parsing the result of `GetTypeAtLocation` where an `AtLocation` parser can be used instead. Diagnostics are unchanged; rules that previously typed every candidate node now skip the ones that cannot match.
- 20d1149: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261029.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/bdec15a7b3a4a82a076f93344fb6011d6ba309c3) commit `bdec15a7b3a4a82a076f93344fb6011d6ba309c3`, update the TypeScript beta tag to [`typescript@beta`](https://www.npmjs.com/package/typescript/v/7.1.0-beta), and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.63.0

### Minor Changes

- b961835: Add `failCauseToConstructor`, a style suggestion for Effect v3 and v4 that rewrites `Effect.failCause` / `Effect.failCauseSync` wrapping a single `Cause.fail` / `Cause.die` into the direct constructor:
  
  ```ts
  Effect.failCause(Cause.die(new Error("boom")))   // -> Effect.die(new Error("boom"))
  pipe(error, Cause.fail, Effect.failCause)        // -> Effect.fail(error)
  Effect.failCauseSync(() => Cause.fail(error))    // -> Effect.failSync(() => error)
  ```
  
  Piped spellings are detected through piping flows, and later steps are kept. On v3, a lazy `Cause.die` becomes `Effect.dieSync`; v4 has no `dieSync`, so it is not reported there. Forwarded, stored, composed or transformed causes, explicit `failCause<E>` type arguments, `as` assertions and annotated thunks are skipped. Lazy forms written as a `pipe(...)` subject or with a piped body are reported without a quick fix, as are lazy values whose error type would change once returned from a `failSync` thunk (such as `cond ? "A" : "B"`, which `Cause.fail` widens to `string`) and named-import calls where no value `Effect` module binding is in scope (type-only imports do not count).
- cbf958c: Add `manualMetricTapToTrack`, an Effect v4 style suggestion with a quickfix that rewrites `Effect.tap` / `tapError` / `tapDefect` calls which only call `Metric.update` into `Effect.trackSuccesses` / `trackErrors` / `trackDefects`:
  
  ```ts
  handleRequest.pipe(
    Effect.tap((size) => Metric.update(responseSize, size)),
    Effect.tapError(() => Metric.update(requestErrors, 1)),
    Effect.tap(Metric.update(rpcSuccesses, 1))
  )
  // becomes
  handleRequest.pipe(
    Effect.trackSuccesses(responseSize),
    Effect.trackErrors(requestErrors, () => 1),
    Effect.trackSuccesses(Metric.withConstantInput(rpcSuccesses, 1))
  )
  ```
  
  Inside callbacks, only metrics that are stable at the call site (earlier `const` bindings, `declare const`, enum members) are moved, because `track*` evaluates the metric once while a tap callback re-evaluates it on every run. A computed input stays inside a mapper so it is still evaluated on every run.
- e75ca43: Add `mapMapErrorToMapBoth` style diagnostic (default `off`, Effect v3 and v4) with a quickfix. Adjacent `Effect.map` + `Effect.mapError` steps (either order) anywhere in one piping flow are reported: in a `pipe`/`.pipe`/`Effect.fn` pipeline, across chained `.pipe(...).pipe(...)` calls, and in nested data-first, curried or mixed data-first/pipeable calls. The quickfix merges them into `Effect.mapBoth` and removes the other step, collapsing a `.pipe(...)` left without steps to its subject; on v4, `Effect.mapEager` + `Effect.mapErrorEager` suggests `Effect.mapBothEager`. The quickfix is withheld when it cannot keep behavior: a step callee that is not a plain or dotted name (such as `getEffect().map`), explicit type arguments on any call between and including the two steps (a curried application, a containing or empty `pipe`/`.pipe`), no binding in scope that reaches `mapBoth`, a rewrite that would newly start an expression statement with `(` or another token that continues the previous line, or a pair split across calls where merging would move a callback that is not a function literal, or the work of an eager first step, across other code.
  
  ```ts
  // before
  fetchCount.pipe(
    Effect.map((n) => n > 0),
    Effect.mapError((message) => new TaskError({ message }))
  )
  // also reported: fetchCount.pipe(Effect.map((n) => n > 0)).pipe(Effect.mapError(...))
  // after
  fetchCount.pipe(
    Effect.mapBoth({
      onSuccess: (n) => n > 0,
      onFailure: (message) => new TaskError({ message })
    })
  )
  ```

### Patch Changes

- a2751c7: `catchAllToMapError` and `catchToOrElseSucceed` now name the catch variant the callee resolves to in their message. Previously a named import, renamed import or local alias of `Effect.catchAll` was reported as `Effect.catch`:
  
  ```ts
  import { catchAll } from "effect/Effect"
  
  pipe(program, catchAll((error) => fail(error.length)))
  // before: ... more directly than `Effect.catch` followed by `Effect.fail`
  // after:  ... more directly than `Effect.catchAll` followed by `Effect.fail`
  ```
- a2751c7: Quick fixes, refactors and completions now build references to Effect modules (`Effect`, `Layer`, `Schema`, `Data`, `Context`, ...) from the file's imports instead of reusing the receiver of the call being rewritten, and support namespace imports of the package barrel.
  
  ```ts
  import * as X from "effect"
  
  const program = source.pipe(X.Effect.map(() => undefined))
  // effectMapVoid quick fix:
  const program = source.pipe(X.Effect.asVoid)
  ```
  
  When the call site reaches Effect through something other than an import, the fix now uses the import:
  
  ```ts
  import { Effect } from "effect"
  const E = Effect
  const program = source.pipe(E.map(() => undefined))
  // quick fix now produces Effect.asVoid instead of E.asVoid
  ```
- 5ebba91: Perf CI: base and head are now measured on the same runner, and only when a maintainer opts in with the `perf` label. Builds still run in parallel, one job per channel and side, each with that commit's own tooling. A separate `measure` job per channel downloads both binaries and runs 9 rounds of stock, base and head compiles in a rotating order. The report's main column is now head vs base directly, flagged by a paired Wilcoxon signed-rank test (p < 0.01), with base and PR overhead over stock tsgo kept as context. `repoctl perf ci` takes `--base-bin/--base-commit/--base-version`, `--head-bin/--head-commit/--head-version` and `--rounds` instead of `--side/--commit/--version/--patched-bin/--pairs`, and writes version 2 cells:
  
  ```sh
  repoctl perf ci --channel latest \
    --base-bin ./tsgo-base --base-commit "$BASE_SHA" --base-version 7.0.2 \
    --head-bin ./tsgo-head --head-commit "$HEAD_SHA" --head-version 7.0.2 \
    --workload-dir /tmp/effect-workload --output perf/latest.json
  ```
- 611fae3: Rust port bookkeeping: each origin marker now carries the hash of the Go declaration it was last synced to, so `porting/manifest.tsv` and `porting/ported.tsv`, which caused constant merge conflicts, are gone. The `repoctl tsrs port-*` commands compute the manifest in memory, `port-check` no longer fails on a stale marker line number, and `port-ack` writes the hash (and current line) into the markers:
  
  ```rust
  // go: internal/typeparser/effect_type.go:45 TypeParser.EffectType 8a9c52f8a845e081
  ```
- fd5ec0a: `preferSchemaOverJson`'s Effect v4 hint names `Schema.fromJsonString(Schema.Unknown)` for unknown shapes instead of `Schema.UnknownFromJsonString`, which effect marks `@internal` and leaves out of its published types:
  
  ```
  Use `Schema.fromJsonString(Schema.Unknown)` for unknown shapes, `Schema.fromJsonString(schema)` for known ones, or `Schema.toCodecJson(schema)` when working with JSON values instead of strings.
  ```
- 1b8a25b: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261028.1), which ships [`TypeScript`](https://github.com/microsoft/TypeScript/commit/97aedd3b75e8e18a3cf2786ffbdb2e6c88aea23f) commit `97aedd3b75e8e18a3cf2786ffbdb2e6c88aea23f`, and the Rust port to [`tsrs@0.20.1-ts7.1.0-beta`](https://www.npmjs.com/package/tsrs/v/0.20.1-ts7.1.0-beta). Symbol tables are read through per-provider helpers, since TypeScript next replaces the map-based `SymbolTable` with an insertion-ordered one.

## 0.62.0

### Minor Changes

- 3e46087: Add `mapConstToAs`, a style suggestion for Effect v3 and v4 that rewrites `Effect.map` callbacks which ignore their input and return a constant into `Effect.as`:
  
  ```ts
  saveUser("alice").pipe(Effect.map(() => "saved"))
  // becomes
  saveUser("alice").pipe(Effect.as("saved"))
  ```
  
  Literals, negative numbers, enum members, earlier `const` bindings and `declare const` values are recognized; object/array literals, `let` bindings, property reads and computed expressions are not, since `Effect.as` captures its value once while a `map` callback re-evaluates it on every run. Callbacks with a return type annotation are also skipped, because the annotation fixes the success type and `Effect.as` would infer it from the value instead.
  
  `syncToSucceed` and `flatMapIgnoredParamToAndThen` share the same constant detection, so they now also recognize `declare const` values, negative numeric literals and enum members.

### Patch Changes

- 048c91f: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261027.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/49e69830214c55a8cc361aa3c4e7412d0afd02fe) commit `49e69830214c55a8cc361aa3c4e7412d0afd02fe`, update the TypeScript beta tag to [`typescript@beta`](https://www.npmjs.com/package/typescript/v/7.1.0-beta), and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2). Refresh the Oxlint configuration schema from the selected package.

## 0.61.0

### Minor Changes

- 151678b: Add the `flatMapIdentityToFlatten` diagnostic (style, suggestion) and quickfix, which replaces `Effect.flatMap` by `Effect.flatten` when the callback is one of two forms:
  
  - Effect's `identity` export referenced directly (`identity`, `Function.identity`, or an import renamed with `as`). Local variables holding `identity` are not followed.
  - An arrow function or function expression without type annotations that returns its own parameter, such as `(inner) => inner` or `function(inner) { return inner }`.
  
  Annotated callbacks and aliases of `identity` are not reported, because an annotation can change the resulting type.
  
  ```ts
  // before
  loadTask.pipe(Effect.flatMap((inner) => inner))
  Effect.flatMap(loadTask, identity)
  // after
  loadTask.pipe(Effect.flatten)
  Effect.flatten(loadTask)
  ```

### Patch Changes

- 8b0e3b5: Fix the win32-x64 tsrs release build. Git on the Windows runner checked out `tsrs/Cargo.toml` with CRLF line endings, so repoctl could not read the `[workspace.metadata.typescript]` commit and the build failed with `No [workspace.metadata.typescript] commit in tsrs/Cargo.toml`. The parser now accepts CRLF, and the Windows release jobs check out with `core.autocrlf` turned off.

## 0.60.1

### Patch Changes

- 9e00121: Prevent a false TS2488 when a rule such as `promiseInEffectSuccess` asks for the type of a spread over a `never`-typed value. TypeScript checks a spread through its enclosing array literal or call, where `never` counts as array-like, but checking the spread element on its own reported that `never` has no `[Symbol.iterator]()` method.
  
  ```ts
  export function f() {
    const rows = [1] as never
    return [...rows] // no longer reports TS2488
  }
  ```
- 87803ae: Fix the tsrs release build on Linux and macOS. tsrs's PGO training installs its bench projects with `pnpm --pm-on-fail=ignore`, which this repository's pnpm 10 rejects. The training now runs with the pnpm version pinned in tsrs's `package.json`, installed into the release work directory. The rest of the release job keeps the repository's own pnpm.

## 0.60.0

### Minor Changes

- 45b0ce6: Add a `"taggedClass"` target to `keyPatterns` for the `deterministicKeys` rule. It covers `Schema.TaggedClass`, `Data.TaggedClass` and `Schema.TaggedRequest`, and checks the tag. For `Schema.*` classes that is the first argument of the outer call, not the optional identifier. Every pattern works with the new target, and the `TaggedClass` and `TaggedRequest` completions use it to fill in the tag. Nothing changes unless the target is configured.
  
  ```jsonc
  {
    "name": "@effect/language-service",
    "keyPatterns": [{ "target": "taggedClass", "pattern": "identifier" }]
  }
  ```
  
  ```ts
  // reports: the expected key is `Completed`
  export class Completed extends Schema.TaggedClass<Completed>()("completed", {}) {}
  ```
- d7351fb: Build the Effect-patched tsrs binary exactly like tsrs's own npm release, and ship it for every target tsrs publishes a glibc, macOS or Windows binary for.
  
  - PGO on every target, plus BOLT on Linux, with tsrs's flags, profile settings, C compiler and strip behaviour. The training is tsrs's own (conformance suite, Go fourslash suite, xstate and webpack) plus the Effect-TS/effect monorepo with every Effect rule on, so the rules are optimized like the compiler instead of being laid out as cold code.
  - New tsrs targets: `darwin-x64` and `win32-x64`, next to `darwin-arm64`, `linux-x64` and `linux-arm64`.
  
  ```sh
  effect-tsgo patch --tsrs    # now also on Intel macOS and Windows x64
  effect-tsgo tsrs --noEmit
  ```

### Patch Changes

- 32611c0: `preferSchemaTypeProperty` now also reports the `Schema.Codec` helpers, the Effect v3 `Schema.Schema.Encoded` and `Schema.Schema.Context` helpers, and helpers referenced through a named `Schema` or `Codec` import. The quick fix rewrites each one to the matching property.
  
  ```ts
  import { Schema } from "effect"
  
  const User = Schema.Struct({ id: Schema.NumberFromString })
  
  type UserEncoded = Schema.Codec.Encoded<typeof User> // now reported, fixed to: typeof User.Encoded
  type UserDecodingServices = Schema.Codec.DecodingServices<typeof User> // fixed to: typeof User.DecodingServices
  type UserEncodingServices = Schema.Codec.EncodingServices<typeof User> // fixed to: typeof User.EncodingServices
  ```
  
  The message now names the helper as written and the property to use, for example "Do not use `Schema.Codec.Encoded<typeof X>`. Use `typeof X.Encoded` instead."

## 0.59.0

### Minor Changes

- 9b9783c: Add proxy commands that run the Effect-patched tsc, tsrs and Oxlint shipped with `@effect/tsgo` directly, without patching `node_modules`. This works with shared or read-only package stores such as the pnpm global virtual store, where `effect-tsgo patch` cannot rewrite the installed binaries. Every argument after the tool is passed through unchanged.
  
  ```sh
  effect-tsgo tsc -p tsconfig.json         # the build of the installed TypeScript version, else the latest
  effect-tsgo tsc@7.0.2 --lsp --stdio      # a specific supported TypeScript version
  effect-tsgo tsc@next --build             # an upstream tag: latest, beta, next
  effect-tsgo tsrs --noEmit                # the build of the installed tsrs version, else the latest
  effect-tsgo oxlint --type-aware          # the installed oxlint CLI with the patched binding and tsgolint
  ```
  
  The type augmentation that adds the `effecttsgo` plugin to Oxlint's config types is still applied only by `effect-tsgo patch`.

### Patch Changes

- 9b9783c: Fix the tsrs build reusing stale Effect diagnostics in incremental builds after the plugin options change. The build info now records the Effect options, as the Go build does, and a change to them drops the cached semantic diagnostics. For example, with `"incremental": true` and an existing `.tsbuildinfo`, switching `"diagnosticSeverity": { "floatingEffect": "off" }` to `"error"` now reports `floatingEffect` without `--force`, and switching it back stops reporting it.
- 9b9783c: Match upstream tsrs npm packaging by building release binaries with `noembed` and shipping the exact `.d.ts` library files from the pinned tsrs checkout beside each executable. Packaged tsrs binaries now load those files from disk instead of embedded libraries.
- 9b9783c: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261026.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/8a62bbe746909624b69b6e671026b454da2f02cf) commit `8a62bbe746909624b69b6e671026b454da2f02cf`, update the TypeScript beta tag to [`typescript@beta`](https://www.npmjs.com/package/typescript/v/7.1.0-beta), and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2). Refresh the tsconfig schema from JSON Schema Store.
  
  Bump tsrs to [`tsrs@0.20.0-ts7.1.0-beta`](https://www.npmjs.com/package/tsrs/v/0.20.0-ts7.1.0-beta), which ports TypeScript `7.1.0-beta`.

## 0.58.0

### Minor Changes

- ae07583: Add proxy commands that run the Effect-patched tsc, tsrs and Oxlint shipped with `@effect/tsgo` directly, without patching `node_modules`. This works with shared or read-only package stores such as the pnpm global virtual store, where `effect-tsgo patch` cannot rewrite the installed binaries. Every argument after the tool is passed through unchanged.
  
  ```sh
  effect-tsgo tsc -p tsconfig.json         # the build of the installed TypeScript version, else the latest
  effect-tsgo tsc@7.0.2 --lsp --stdio      # a specific supported TypeScript version
  effect-tsgo tsc@next --build             # an upstream tag: latest, beta, next
  effect-tsgo tsrs --noEmit                # the build of the installed tsrs version, else the latest
  effect-tsgo oxlint --type-aware          # the installed oxlint CLI with the patched binding and tsgolint
  ```
  
  The type augmentation that adds the `effecttsgo` plugin to Oxlint's config types is still applied only by `effect-tsgo patch`.

### Patch Changes

- a1b58f1: Fix `apiStabilityLeak` missing the members of class and interface types with the TypeScript next tag, which now resolves those members lazily. For example, this leak is reported again:
  
  ```ts
  /** @stability experimental */
  export interface PreviewExperimental {
    value: string
  }
  
  // `PreviewStable` exposes `PreviewExperimental`, an experimental API.
  export interface PreviewStable {
    leaked: PreviewExperimental
  }
  ```
- 03d36f2: Fix the tsrs build reusing stale Effect diagnostics in incremental builds after the plugin options change. The build info now records the Effect options, as the Go build does, and a change to them drops the cached semantic diagnostics. For example, with `"incremental": true` and an existing `.tsbuildinfo`, switching `"diagnosticSeverity": { "floatingEffect": "off" }` to `"error"` now reports `floatingEffect` without `--force`, and switching it back stops reporting it.
- 2276f61: Match upstream tsrs npm packaging by building release binaries with `noembed` and shipping the exact `.d.ts` library files from the pinned tsrs checkout beside each executable. Packaged tsrs binaries now load those files from disk instead of embedded libraries.
- a1b58f1: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261025.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/8a62bbe746909624b69b6e671026b454da2f02cf) commit `8a62bbe746909624b69b6e671026b454da2f02cf`, update the TypeScript beta tag to [`typescript@beta`](https://www.npmjs.com/package/typescript/v/7.1.0-beta), and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.57.0

### Minor Changes

- 8e4d618: Add experimental support for [tsrs](https://www.npmjs.com/package/tsrs), the Rust port of the TypeScript 7 compiler. All Effect diagnostics, quick fixes, refactors and completions are ported to Rust and produce the same output as the Go version.
  
  `effect-tsgo patch --tsrs` patches your installed `tsrs` package, replacing its binary with an Effect-patched build of the same tsrs version. TypeScript patching is unchanged.
  
  ```sh
  npm install --save-dev tsrs@0.18.0-ts7.1.0-dev.20260929
  npx effect-tsgo patch --tsrs        # patch TypeScript and tsrs
  npx effect-tsgo patch --no-typescript --tsrs   # patch tsrs only
  npx tsrs -p tsconfig.json
  # src/index.ts(4,3): error TS377001: This Effect value is neither yielded nor used in an assignment. effect(floatingEffect)
  npx effect-tsgo unpatch --tsrs      # restore tsrs's original binary
  ```
  
  The patched tsrs binary is available on darwin-arm64, linux-x64 (glibc) and linux-arm64 (glibc). Other platforms, including Linux musl, are skipped with a message explaining why.

## 0.56.1

### Patch Changes

- 93fc061: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261023.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/a91e93821feafd081641f9f5d161eba626d0b2ce) commit `a91e93821feafd081641f9f5d161eba626d0b2ce`, update the TypeScript beta tag to [`typescript@beta`](https://www.npmjs.com/package/typescript/v/7.1.0-beta), and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.56.0

### Minor Changes

- 97d3c56: Track the `typescript@beta` dist-tag alongside `typescript@latest` and `typescript@next`. `repoctl upstream update` now resolves the beta tag into `upstream.json`, so the TypeScript beta compiler (currently `7.1.0-beta`) is built, tested in CI under the `beta` channel, and shipped with the platform packages.

### Patch Changes

- eb044ca: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261020.3), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/28534d7609062699a68b1b66e2367f8d95dbf290) commit `28534d7609062699a68b1b66e2367f8d95dbf290`, update the TypeScript beta tag to [`typescript@beta`](https://www.npmjs.com/package/typescript/v/7.1.0-beta), and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.55.0

### Minor Changes

- d53a707: Add the opt-in `preferEffectString` diagnostic in `effectNative` for native String method invocations. It suggests 32 `effect/String` counterparts, including `trimLeft` to `String.trimStart` and `trimRight` to `String.trimEnd`.
  
  Each suggestion explains typing or runtime differences. For example, `email.indexOf("@")` suggests `String.indexOf`, which returns Option with None replacing -1.
  
  The rule defaults to off and the `effect-native` preset enables it as a warning. Both Effect v3 and v4 are supported. Only suppression quick fixes are available.

## 0.54.1

### Patch Changes

- 70bb791: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261019.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/5237fd46890956e4bb8177a1a05dcd69e4166560) commit `5237fd46890956e4bb8177a1a05dcd69e4166560`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2). Refresh the Oxlint configuration schema from the selected package.

## 0.54.0

### Minor Changes

- e00f33c: Add the `schemaClassObjectLiteral` diagnostic. It warns when an object literal is checked as a Schema class instance. A plain object has no class prototype, so `instanceof`, class methods, and Schema encoding fail at runtime. A Schema brand does not help, because a spread copies the brand property.
  
  ```ts
  class User extends Schema.Class<User, { readonly brand: unique symbol }>("User")({ name: Schema.String }) {}
  
  const copy: User = { ...user } // warning: build it with User.make({ ...user })
  const renamed = User.make({ ...user, name: "b" }) // ok
  ```
  
  In Effect v4, the rule skips objects nested inside a Schema constructor call, such as `Parent.make({ child: { ...child } })`, because v4 builds the nested class instance from the plain object.

## 0.53.5

### Patch Changes

- c83ee4d: `runEffectInsideEffect` now reports `Effect.runSyncExit` and `Effect.runPromiseExit`, the same as `Effect.runSync` and `Effect.runPromise`. The quick fix rewrites them to `Effect.runSyncExitWith` and `Effect.runPromiseExitWith` in Effect v4, and to `Runtime.runSyncExit` and `Runtime.runPromiseExit` in Effect v3.
  
  ```ts
  export const nested = Effect.gen(function*() {
    yield* Effect.void
    setTimeout(() => {
      Effect.runPromiseExit(child)
      // ^- `Effect.runPromiseExit` is called inside an Effect with a separate services invocation.
    }, 0)
  })
  ```
- de42db0: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261018.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/5237fd46890956e4bb8177a1a05dcd69e4166560) commit `5237fd46890956e4bb8177a1a05dcd69e4166560`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.53.4

### Patch Changes

- f57afb9: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261016.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/abcbb2e69d2a86d9ffee66582a0dd0339b674733) commit `abcbb2e69d2a86d9ffee66582a0dd0339b674733`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.53.3

### Patch Changes

- 1ab5264: `effect-tsgo diagnostics` now reports a diagnostic's related locations. They used to be dropped in every format, so `missingEffectContext` never showed which layer could provide the missing service and `apiStabilityLeak` never showed where the unstable declaration lives.
  
  With `--format json`, each diagnostic that has related locations gets a `relatedInformation` array. Each entry has the same location fields as the diagnostic, plus its own `code` and `message`. Diagnostics without related locations are unchanged.
  
  ```json
  {
    "line": 23,
    "column": 3,
    "code": 377004,
    "name": "missingEffectContext",
    "message": "This Effect requires a service that is missing from the expected Effect context: `Db | Fs`.",
    "relatedInformation": [
      {
        "file": ".../src/index.ts",
        "start": 662,
        "length": 14,
        "line": 22,
        "column": 3,
        "endLine": 22,
        "endColumn": 17,
        "code": 377059,
        "message": "Adjusting this layer composition could provide the missing service."
      }
    ]
  }
  ```
  
  With `--format pretty`, the related locations are printed under the diagnostic, each with its own code snippet, as `tsc --pretty` does:
  
  ```
  src/index.ts:23:3 - error effect(missingEffectContext): This Effect requires a service that is missing from the expected Effect context: `Db | Fs`.
  
  23   Effect.runPromise
       ~~~~~~~~~~~~~~~~~
  
    src/index.ts:22:3 - Adjusting this layer composition could provide the missing service.
      22   Effect.provide(live),
           ~~~~~~~~~~~~~~
  ```
  
  The `~` markers in pretty output are also aligned again on lines 10 and above. They used to sit one column to the left of the code.
- 4672ba1: Make `globalDate` and `globalDateInEffect` do less work per file. Their shared walker used to look up the Effect context flags of every node it visited, although only call expressions (`Date.now()`) and `new` expressions (`new Date()`) can produce a diagnostic. It now looks up the flags only on those two node kinds, like `globalFetch`, `globalConsole`, `globalTimers`, `globalRandom` and `cryptoRandomUUID` already do.
  
  Reported diagnostics are unchanged.
- b5df4e9: Speed up `apiStabilityLeak` by reusing one stability analysis per file.
  
  The rule used to allocate a fresh analysis, with its maps and work queues, for every export of a file. It now keeps one analysis per file and resets it before each export. The reset analysis behaves exactly like a fresh one, so the reported diagnostics are unchanged. A published surface that a later export reads from the shared cache is no longer queued for publication again, and a published surface without retained findings no longer allocates a findings map.
  
  On `packages/effect` of effect 4.0.5 with every rule enabled, the rule allocates 252 MB → 140 MB.
- d3b53a7: Make the Effect diagnostics write less to the checker's per-node caches. Rules run on the checker goroutine right after each file is checked, and on the Effect monorepo most of their cost was cache traffic (map misses, nil inserts, map growth) and the garbage collection it caused, not type computation.
  
  - Reference checks such as "is this node `Effect.gen`?" no longer probe or fill the symbol cache for statements, blocks, clauses and tokens, which can never resolve to a symbol.
  - The analysis that tracks whether code runs inside `Effect.gen`, `Effect.fn` or an Effect thunk stores an entry only where the answer changes, about 2% of nodes, instead of two entries for every node of every file.
  - Type lookups reject nodes that can never have a type before touching the cache, the strict Effect type check caches only types named `Effect`, and whether a file has been checked is asked once per file instead of on every query.
  
  Reported diagnostics are unchanged.

## 0.53.2

### Patch Changes

- c75c219: Make `apiStabilityLeak` allocate less. The rule used to create an empty findings map for every type, signature and symbol it inspected, and to copy the stored findings into a new map every time it re-checked an unresolved result. Most results never get a finding, and most re-checks add nothing. A result's map is now created when it gets its first finding, and a re-check that adds nothing keeps the stored map instead of copying it.
  
  For example, on the Effect monorepo the rule allocated 605 MB in 5.3 M objects before and 496 MB in 3.4 M objects after. Its CPU went from 3.4 s to 3.2 s.
  
  Reported diagnostics are unchanged.
- 0076e24: Make `apiStabilityLeak` faster on namespace exports. The rule checks each member of a namespace export in one shared analysis, and every member used to re-check every unresolved result that earlier members left behind. A namespace with thousands of members did millions of repeated checks. Each member now re-checks only the results its own check reached.
  
  For example, on the Effect monorepo the `Generated` namespace export of `@effect/ai-openai` has 3,603 members. Across the whole repository the rule used 22.8 s of CPU before and 3.4 s after.
  
  Reported diagnostics are unchanged.
- 410f21e: Rules now read the type the checker computed for an expression while it checked the file, instead of asking the checker again afterwards. Asking again repeated work, such as flow analysis, and in a few cases gave a different type than the check used. For example, a return literal checked while its enclosing generic call was still resolving:
  
  ```ts
  export const takeWhileDefined = dual(2, <A>(self: Iterable<A>, f: (input: A) => A | undefined) => ({
    [Symbol.iterator]() {
      const iterator = self[Symbol.iterator]()
      return {
        next() {
          const result = iterator.next()
          if (result.done) {
            // the check typed this as { done: boolean; value: undefined }
            // asking again gave { done: true; value: undefined }
            return { done: true, value: undefined }
          }
          return { done: false, value: f(result.value) }
        }
      }
    }
  }))
  ```
  
  Rules now see `{ done: boolean; value: undefined }`, the type the rest of the check is based on.
- c5d3d7a: Resolve Effect API references once per symbol. `IsNodeReferenceToModuleExport` used to answer "is this symbol exported as `gen`?" one member name at a time, caching each answer in a per-symbol map. It now computes, once per symbol, the names its declaring module exports it under, from a per-module index of exports keyed the way the checker compares symbols, and answers every member name from that list. `export =` modules keep the direct lookup.
  
  For example, checking whether a callee is `Effect.gen`, `Effect.fn` or `Effect.sync` now reads one cached list (`["gen"]` for `Effect.gen`) instead of hashing three cache keys.
  
  With 125 rules enabled on the Effect monorepo, wall time, CPU time and peak memory are unchanged within noise, and the reported diagnostics are unchanged.
- f02bf75: Skip Effect type queries for more expressions that always produce a primitive or a `RegExp`. `NodeCannotBeEffectLibraryType` now also rules out `true`, `false`, `null`, regular expression literals, prefix unary expressions such as `!x` and `-x`, `void x` and `typeof x`. The `undefined` identifier still goes through the checker, because a local binding can shadow it.
  
  For example, for `return true` inside `Effect.gen`, `returnEffectInGen` no longer asks the checker for the type of `true`.
  
  Reported diagnostics are unchanged.
- 67d58fc: Add a Perf workflow that compares each pull request against its base for every supported TypeScript channel. Each job runs stock and Effect-patched tsgo on a pinned Effect checkout with every rule executing, and a sticky PR comment reports the change in symbols, types, instantiations, allocations, and check and total time over stock. Run it locally with `repoctl perf ci` and `repoctl perf report`.
- 01f337a: Report each problem once in `effectInFailure`, `anyUnknownInErrorContext` and `promiseInEffectSuccess`. These rules check the type of every node in a file, and the checker gives some pairs of nodes the same type. A declaration and its name share a type, and so do `a.b` and its name `b`. The rules now check one node of each pair.
  
  A variable whose initializer has the problem used to get two warnings, one on the variable name and one on the initializer. Now only the initializer is reported:
  
  ```ts
  // before: warnings on `shouldReport` and on `Effect.try(...)`
  // after: one warning on `Effect.try(...)`
  export const shouldReport = Effect.try({
    try: () => JSON.parse("{"),
    catch: (e) => Effect.log(e)
  })
  ```
  
  A property access is now reported over the whole expression `promiseEffectHolder.current` instead of the name `current`.
  
  `anyUnknownInErrorContext` keeps only the innermost warning when nested expressions report the same problem. It used to drop an outer warning whenever any warning inside it reported something. Now it drops the outer warning only when an inner warning reports every channel the outer one does, with the same `any` or `unknown`. An outer `unknown` error channel that comes from an explicit type argument is still reported when the inner expressions only have `any`:
  
  ```ts
  declare const anyFailure: Effect.Effect<number, any>
  // warnings on `Effect.withFiber(...)` (unknown in the error channel) and on `anyFailure` (any)
  export const keepsDifferentKind = Effect.withFiber<number, unknown>(() => anyFailure)
  ```

## 0.53.1

### Patch Changes

- 06f7842: Stop `experimentalApiUsage` and `unstableApiUsage` from causing TS2589 and follow-on type errors in generic calls such as `Match.when({ kind: "A0" }, ...)` over a wide union.
  
  When a generic call infers a property back to the object literal, the rules look up the type the literal would get without its own inferences. They used to resolve the call again with inference blocked, which could instantiate types the original check never built. They now replay the checker's type-argument inference on the signature it already resolved and leave out the literal's argument, so return type inferences, inferences from the other arguments, defaults, and constraints apply as before. When that type has no matching property, the rules use the type parameters' constraints.
  
  These cases are now reported too:
  
  - Constraint properties inside inferred generator functions.
  - Array and rest-parameter constraints, for example `generic([{ test: 42 }])` when `T extends Reportable[]` marks `test` as experimental.
  - Literals inside a call that is itself an argument of a generic call, for example `generic(id({ test: 42 }))`.
- f4b49a4: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261014.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/9eedcb0dd7257f8a1a175adbef28f3b6d3492d7a) commit `9eedcb0dd7257f8a1a175adbef28f3b6d3492d7a`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.53.0

### Minor Changes

- 13a46c9: The `deterministicKeys` rule and the key completions now compute a key for files without a `package.json`, or whose nearest `package.json` has no `name`. The key is built as usual, but without the package name. The sub-directory segment is relative to the `package.json` directory if one is found, otherwise to the `tsconfig.json` directory, otherwise to the program's common source directory.
  
  Projects that enabled `deterministicKeys` without a `package.json` will now see reports. This differs from the upstream `@effect/language-service`, which skips these files.
  
  ```json
  {
    "name": "@effect/language-service",
    "keyPatterns": [{ "target": "service", "pattern": "default" }]
  }
  ```
  
  ```ts
  // src/services/AuthService.ts, no package.json, tsconfig.json at the project root
  // reports: the expected key is `services/AuthService/MyService`
  export class MyService extends Context.Service<MyService, {}>()("MyService") {}
  ```
- d8c489b: Add `errorPayloadShadowsName` to detect inline `name` payload fields on Effect error classes. For example, `Data.TaggedError("ToolNotFound")<{ name: string }>` copies the payload's `name` onto the error instance and hides the error type in logs. Rename the field to `toolName`. The rule inspects inline payloads only and allows a field whose value is the literal the error already uses as its name.
- 13a46c9: Add an `"identifier"` pattern to `keyPatterns` for the `deterministicKeys` rule. With it, the expected key is the class name alone.
  
  ```json
  {
    "name": "@effect/language-service",
    "keyPatterns": [{ "target": "error", "pattern": "identifier" }]
  }
  ```
  
  ```ts
  // reports: the expected key is `KeybindingsConfigError`
  export class KeybindingsConfigError extends Data.TaggedError("KeybindingsConfigParseError")<{}> {}
  ```
- b1ed4ba: Add `preferNamespaceImport` and `preferBarrelImport`, two diagnostics that report existing imports that contradict the import style configured with `namespaceImportPackages` and `barrelImportPackages`. Auto-import already follows these options; the new rules also cover imports written by hand. Both are off by default and have no quick fix.
  
  `preferNamespaceImport` reports named imports from packages in `namespaceImportPackages`:
  
  ```ts
  // "namespaceImportPackages": ["effect"]
  import { Effect } from "effect" // expected: import * as Effect from "effect/Effect"
  import { map } from "effect/Array" // expected: import * as Array from "effect/Array"
  import { pipe } from "effect" // reported only with "topLevelNamedReexports": "follow"
  ```
  
  `preferBarrelImport` reports imports of individual modules from packages in `barrelImportPackages`:
  
  ```ts
  // "barrelImportPackages": ["effect"]
  import * as Record from "effect/Record" // expected: import { Record } from "effect"
  import * as HttpClient from "effect/http/HttpClient" // expected: import { HttpClient } from "effect/http"
  ```
  
  Both the rules and auto-import find a barrel from the package files, for any package, instead of from a built-in list of Effect paths. A module's barrel is the nearest `index` module above it that forwards it with `export * as Name`, and `Name` is the import name used. Barrel files are read from disk when no file in the program imports them yet. Auto-import with `barrelImportPackages` uses the same lookup, so it no longer rewrites a module that no barrel forwards, and no longer rewrites an import that already comes from a barrel (such as `pipe` from `"effect"`) into `import { effect } from "effect"`.
- b1ed4ba: Allow `namespaceImportPackages`, `barrelImportPackages`, `importAliases`, and `topLevelNamedReexports` in per-file `overrides`. Auto-import, the import quick fixes, and the `preferNamespaceImport` and `preferBarrelImport` diagnostics use the value resolved for the current file, and a scoped value replaces the base value.
  
  ```json
  {
    "namespaceImportPackages": ["effect"],
    "overrides": [{
      "include": ["src/legacy/**/*.ts"],
      "options": {
        "namespaceImportPackages": [],
        "barrelImportPackages": ["effect"]
      }
    }]
  }
  ```

### Patch Changes

- 61eb79c: Compute Effect diagnostics with less work. Rules and the type parser now test node kinds, shapes and argument counts before they query types, symbols or signatures, and they compute types only for steps that can still report:
  
  - `IsNodeReferenceToModuleExport` answers symbols with no declaration in the requested module from a per-symbol bitmask instead of a per-symbol map.
  - `DataFirstOrLastCall` checks the cached signature shape before it types call arguments, and caches its result per call.
  - `missedPipeableOpportunity`, `catchDieToOrDie` and `preferTypedSchemaDecoder` type piping-flow steps only when a step matches.
  - The `experimentalApiUsage` and `unstableApiUsage` rules reuse their own symbol lookup instead of repeating it.
  
  With every rule enabled, `tsc -b` of the Effect monorepo drops from 33.9s to 32.1s (stock TypeScript-Go: 20.4s). The reported diagnostics are unchanged.
- cb5739d: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261013.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/b5c34ba0c61a756c8abaf133236dddf982c27f45) commit `b5c34ba0c61a756c8abaf133236dddf982c27f45`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2). Refresh the Oxlint configuration schema from the selected package.

## 0.52.0

### Minor Changes

- 3e4a55d: Route Effect contextual-type queries through `TypeParser.GetContextualType` and enforce the wrapper with customlint. For example, use `tp.GetContextualType(node, checker.ContextFlagsNone)` instead of the checker method. Query behavior stays unchanged, with one place to add contextual-type safety guards in future changes.
- 0269634: Add `effectSleepZeroToYieldNow`, a style suggestion with a quick fix for zero-duration `Effect.sleep` calls.
  
  For example, `Effect.sleep(0)` becomes `Effect.yieldNow()` in Effect v3 and `Effect.yieldNow` in Effect v4. The rule also recognizes safe zero-duration strings, tuples, and Duration constructors. The replacement yields through the fiber scheduler instead of the Clock service.
- 181b2b3: Add the `flatMapToTap` style suggestion and quick fix for Effect v3 and v4. Callbacks that run an effect and restore their unchanged parameter with terminal `Effect.as` or `Effect.map` can use `Effect.tap` instead. The fix preserves callback statements and earlier effect transformations.
- 7d051dd: Route internal symbol and type location queries through TypeParser and enforce this with customlint. Reuse the existing TypeParser throughout pipeable signature comparison.
  
  Prevent false TS2790 diagnostics for valid index-signature deletes such as `declare const map: { [key: string]: number }; delete map.x;`. Symbol queries skip deleted property accesses and their names, including parenthesized and optional targets. Receiver and computed-key queries remain available, and invalid required-property and readonly deletes still report compiler errors.
- f178ff2: Add the opt-in `preferEffectArray` diagnostic in `effectNative` for native Array method calls and references. It suggests 16 corresponding Effect Array APIs and explains migration differences, such as `values.find(predicate)` becoming `Array.findFirst(values, predicate)` with an `Option` result.
  
  The rule is disabled by default and enabled as a warning by the `effect-native` preset. It supports readonly arrays, tuples, aliases, and literal bracket access. Native Array recognition uses a lazy cache shared per checker. Conversion autofixes are not included.
- f59b4b8: Add the opt-in `preferEffectRecord` diagnostic in `effectNative` for native Object calls and references. It suggests `Record.keys`, `Record.values`, `Record.toEntries`, `Record.fromEntries`, and `Record.has`, including native `hasOwnProperty` references.
  
  Each suggestion explains input typing or runtime differences. For example, `Object.entries(data)` suggests `Record.toEntries(data)` with typed keys and a caveat about getter or proxy mutations. `data.hasOwnProperty(key)` suggests passing the receiver explicitly to `Record.has(data, key)`.
  
  The rule defaults to off and the `effect-native` preset enables it as a warning. Both Effect v3 and v4 are supported. Only suppression quick fixes are available.
- 2672190: Add the opt-in `preferSchemaTaggedError` diagnostic for Effect v3 and v4. It suggests `Schema.TaggedError` for class declarations that extend `Data.TaggedError`, including anonymous default exports. For example, `class NotFound extends Data.TaggedError("NotFound")<{ id: string }> {}` reports a warning when the rule is enabled.
- ab20f55: Expose parser results through getters and readonly collection views. For Go consumers, `result.A` becomes `result.A()`, and `flow.Transformations[i]` becomes `flow.Transformations().At(i)`. Collection views support iteration and explicit owned copies. Compiler AST and type pointers remain borrowed.
- c4aa628: Include `{ readonly brand: unique symbol }` in Effect v4 Schema class completions for `Class`, `TaggedClass`, `Error`, and `TaggedError`. Add a branded completion for `Schema.Opaque`.
  
  For example, the `Class<Person>` completion inserts `Schema.Class<Person, { readonly brand: unique symbol }>("Person")`. Effect v3 and `Model.Class` completions keep their existing type arguments.
- 22f6106: Add `schemaClassMissingBrand`, a correctness suggestion enabled by default for Effect v4 schema classes, errors, and opaque classes.
  
  The diagnostic suggests adding a brand when the second brand type argument is omitted. For example, `Schema.Class<Person>("Person")` triggers a suggestion, while `Schema.Class<Person, { readonly brand: unique symbol }>("Person")` supplies a nominal brand. An explicit `{}` keeps structural typing and suppresses the suggestion. Effect v3 is excluded.
- d565ab8: Recognize `Predicate.isTagged(value, "Foo")` and `Predicate.isTagged("Foo")(value)` as tag comparisons in dispatch analysis. Existing catch-tag diagnostics and quick fixes now handle both forms in Effect v3 and v4.

### Patch Changes

- 4ceb27f: Reduce repeated type inference when checking experimental and unstable properties in generic object literals.
- e57688f: Clear an inherited Effect plugin when a child `tsconfig` defines an empty or replacement `plugins` list.
- 9696634: Report experimental and unstable properties supplied in contextually typed object literals, including shorthand properties. For example, `x({ test: 42 })` and `x({ test })` now warn when the parameter type marks `test` with `@stability experimental`, even if the function and interface are stable.
- 0dfc0be: Fix a crash in the "Convert to Effect.fn" refactor when `gen` is a named import or a local alias.
  
  ```ts
  import { gen } from "effect/Effect"
  export const foo = () => gen(function*() { return 1 })
  ```
  
  Requesting code actions on `gen` used to fail the whole `textDocument/codeAction` request with a nil pointer panic. The refactor now rewrites to `Effect.fn`, using the file's existing Effect import name when there is one:
  
  ```ts
  import { gen } from "effect/Effect"
  export const foo = Effect.fn("foo")(function* () { return 1; });
  ```
  
  The refactor does not add a missing `Effect` import. The user adds it.
- 6107975: Update the shim generator's Go tools dependency to support Go 1.27.2 export data and restore compiler setup.
  
  Preserve pointers to unexported structs in generated shim mirrors so fields that follow them retain their offsets and accessors return the correct values.
- 8e0b5bf: Resolve the types carried by typeparser results only when a rule reads them, and reuse the resolved type on later reads. For example, a piping flow no longer queries the type of every transformation up front, so a rule that filters flows by callee skips those queries. Diagnostics are unchanged.
- 6512cfb: Cache module export checks per symbol. Rules check whether most identifiers and callees refer to a module export, such as `gen`, `fn`, or `map` from the `effect` package's `Effect` module. Each symbol now gets a slot in a paged array indexed by symbol id, the layout the TypeScript 7.1 checker uses for its own symbol caches. The slot holds a small map of answers keyed by module and member name. Before, every answer shared one large hash map. For example, `tsc -b` on the Effect monorepo with every rule enabled runs about 1% faster with identical diagnostics. On TypeScript 7.0.2, which lacks block-allocated symbol ids, the per-symbol slots stay in a hash map.
- b780f66: Fix the Effect.fn and async-to-Effect refactors for functions nested inside another declaration's value. A function in a catch binding default no longer makes the code action request fail, and a function in an object, array, call argument, class field, or later declarator is now replaced in place instead of overwriting the whole value. For example, converting `task` in `export const inObject = { task: () => Effect.gen(function*() { return 1 }) }` now produces `export const inObject = { task: Effect.fn(function* () { return 1; }) }`.
- 959570b: Store the Effect caches that cover most nodes of a file in paged arrays indexed by node id, the layout the TypeScript 7.1 checker uses for its own hottest node caches. A lookup now indexes an array instead of hashing a node pointer. For example, `tsc -b` on the Effect monorepo runs about 2% faster with identical diagnostics. On TypeScript 7.0.2, which lacks block-allocated node ids, these caches stay hash maps.
- 60eac10: Run Effect diagnostics after TypeScript checks unused identifiers so plugin symbol queries cannot suppress TS6133. For example, with `noUnusedParameters` enabled, `export function unusedPredicate(value: unknown): value is number { return true; }` now reports the unused `value` parameter with the plugin enabled, just as it does with the plugin disabled.
- 7450a52: Remove the `debug` plugin option and the debug-only `Flows` group from the document outline.
- 3e4a55d: Cache object-literal contextual types per checker across experimental and unstable API diagnostics. Keep normal and inference-blocked queries separate, including nil results, to avoid repeating the same contextual-type queries.
- 4d9fba4: Ship the latest TypeScript compiler once in each platform package. It was packaged twice, as `lib/tsc` and as `artifacts/typescript/<latest>/tsc`, each with its own `lib.*.d.ts` files. It now ships only as `lib/tsc`, which makes `@effect/tsgo-darwin-arm64` 29 MB smaller unpacked.
  
  `get-exe-path`, `diagnostics`, `patch` and `getExePath()` resolve the latest TypeScript version to `lib/tsc` and other versions to `artifacts/typescript/<version>/tsc`. Tools that run `lib/tsc` directly keep working.
- 3e4a55d: Prevent false TypeScript iterator and object-spread diagnostics when experimental or unstable API checks query generic constraints inside inferred generator function expressions. Skip inference-blocked contextual queries in these expressions while retaining normal contextual lookups. This conservative fallback can omit constraint-based property warnings inside inferred generators.
- da2f46d: Skip type queries for expressions that can never have an Effect library type, such as literals, functions and, on Effect v4, plain object literals. Diagnostics are unchanged, but they are cheaper to compute: on the Effect monorepo a `tsc -b` build is about 1.3s faster with the default rules and about 2.4s faster with every rule except `apiStabilityLeak` enabled.
  
  For example, rules such as `effectInVoidSuccess` no longer ask the checker for the type of the array and object literal arguments here, because neither can be an Effect:
  
  ```ts
  Effect.forEach([1, 2, 3], (n) => Effect.log(n), { concurrency: 2 })
  ```
- 1c7cbf9: Update Effect v4 dependencies and test fixtures from `4.0.0-rc.118` to stable `4.0.2`. Align `@effect/platform-node`, `@effect/platform-node-shared`, and `@effect/vitest` with the same release.
- fea53cc: Use the configured `keyPatterns` for `Data.TaggedError` and `Schema.TaggedError` completions, so the inserted tag already matches the key expected by `deterministicKeys`.
  
  For example, with `{ "target": "error", "pattern": "package-identifier" }` in `keyPatterns`, completing `class NotFound extends Data.` in `src/errors.ts` of package `@app/core` now inserts `Data.TaggedError("@app/core/errors/NotFound")<{}>{}` instead of `Data.TaggedError("NotFound")<{}>{}`. Without an `error` key pattern the completion still uses the class name.
- cb7567e: Support both field-based and method-based compiler symbols through generated shim helpers.
- 7450a52: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261007.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/d61a7d235906e78be9b111de0ba060e28e67ecfa) commit `d61a7d235906e78be9b111de0ba060e28e67ecfa`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.51.1

### Patch Changes

- a5bd7f1: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20261006.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/a1ef42b9ea7032fa60df127d42b4c86fd2a110ee) commit `a1ef42b9ea7032fa60df127d42b4c86fd2a110ee`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.51.0

### Minor Changes

- 9ca6ea2: Add related declaration locations to `apiStabilityLeak` diagnostics. For example, when a stable export exposes an experimental type, the diagnostic on the export now includes a child pointing to the experimental type's declaration, including declarations in other files.

### Patch Changes

- caee22f: Fix `apiStabilityLeak` to ignore inherited bases whose public members are all optional and explicitly stability-tagged, including bases reached through augmentations and superclass factories. Direct properties and parameters typed as those bases remain checked. Exclude `@internal` properties and methods from the public surface throughout the leak check.

## 0.50.0

### Minor Changes

- 69c8ce5: Add the opt-in `apiStabilityLeak` diagnostic for Effect v4 package maintainers.
  It reports exports whose public types, parameters, or return values expose a
  less stable API: stable exports must not expose unstable or experimental types,
  and unstable exports must not expose experimental types. Untagged exports are
  treated as stable; experimental exports impose no restriction.
  
  For example, `export interface StableApi { value: ExperimentalType }` reports
  a leak when `ExperimentalType` has an `@stability experimental` tag. Public
  members, generic arguments, inferred types, and re-exports are checked without
  recursively instantiating types.
  
  The rule is off by default and excluded from the recommended, strict, and
  correctness presets. Enable the new `maintainers` preset to report it as a
  warning, or configure `apiStabilityLeak` directly. Declared stability lookups are
  shared with the existing unstable and experimental API usage diagnostics.

## 0.49.0

### Minor Changes

- 75e6ca5: Migrate compiler integrations to the current TypeScript path and configuration APIs, and refresh the TypeScript patch stack for the new upstream snapshot. Keep TypeScript 7.0.2 supported through adapters for the legacy typescript-go provider.
  
  For example, plugin option and diagnostic severity resolution now accept rooted source and config file paths directly, while the legacy shims adapt those calls to TypeScript 7.0.2. Preserve Effect options through upstream option cloning and comparison, and fix include patterns for files in filesystem roots.
  
  Keep the Nix compiler source pin, lock file, and vendor hash synchronized with the updated snapshot, and apply Git binary compiler patches during Nix builds. Preserve the original TypeScript 7.0.2 provider shim APIs for tsgolint, with modern API adapters confined to the canonical shim facade.

## 0.48.1

### Patch Changes

- c964cca: Fix `unstableApiUsage` and `experimentalApiUsage` naming overloaded (dual) exports by module, which prevented `allowedUnstableApis` / `allowedExperimentalApis` from allowing them per export.
  
  A dual export whose call signatures carry their own `@stability` tag — for example `effect/http/HttpClientRequest#bodyText` — was reported under the bare module name (`effect/http/HttpClientRequest`) because the selected call signature is not the module export. An allow-list entry for `effect/http/HttpClientRequest#bodyText` therefore never matched, and the only way to allow the use was a whole-module entry.
  
  The reported name now resolves to the export that owns the selected overload signature, matching the README's `package/module#exportName` form:
  
  ```ts
  import { HttpClientRequest } from "effect/http"
  
  export const request = HttpClientRequest.post("http://x").pipe(
    HttpClientRequest.setHeader("a", "b"),
    HttpClientRequest.bodyText("{}", "application/json")
  )
  ```
  
  With:
  
  ```json
  "allowedUnstableApis": [
    "effect/http/HttpClientRequest#post",
    "effect/http/HttpClientRequest#setHeader",
    "effect/http/HttpClientRequest#bodyText"
  ]
  ```
  
  all three uses are allowed, and any unlisted dual export still warns under `package/module#exportName`.

## 0.48.0

### Minor Changes

- a954eb5: Add `allowedUnstableApis` and `allowedExperimentalApis` to selectively allow unstable or experimental declaration modules or exported APIs. For example, `effect/http/HttpClient` allows the entire module, while `effect/http/HttpClient#get` allows only its exported `get` API. Both options support per-file overrides, and both diagnostics display the declaration-based name.

### Patch Changes

- 0900778: Build packaged TypeScript compilers without embedded standard libraries so editor go-to-definition opens real library files. Ship matching `lib.*.d.ts` files beside each versioned compiler and the latest `lib/tsc` alias, keeping `diagnostics` and `get-exe-path` working when running the packaged executable directly.

## 0.47.2

### Patch Changes

- 536f83a: Fix a panic in `unstableApiUsage` and `experimentalApiUsage` on the `default` of a dynamically imported `export =` or JSON module.
- bddb623: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260929.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/0681ef7fa3a2378ccf49645b6d5b7463bdca74bb) commit `0681ef7fa3a2378ccf49645b6d5b7463bdca74bb`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.47.1

### Patch Changes

- d4f19b3: Allow `effect-tsgo patch` to run without the deprecated `--force` flag.

## 0.47.0

### Minor Changes

- b67ef81: Update the Effect v4 integration to `4.0.0-rc.118` and use its public module paths. Namespace auto-imports for nested modules now resolve to paths such as `effect/cli/Prompt`.
- 66668e2: Add the `experimentalApiUsage` and `unstableApiUsage` diagnostics for APIs marked with `@stability experimental` or `@stability unstable`. Both rules warn by default. For example, a reference to an unstable API now reports that breaking changes may happen between versions.

### Patch Changes

- 8067475: Fix `getExePath` rejecting the schema version shipped by platform packages.
  
  Discover installed Oxlint musl bindings without rejecting them before component selection, so TypeScript-only commands work when Oxlint is installed on Alpine Linux. Report unsupported Oxlint replacements during patch preparation instead, allowing `--skip-missing` and restoring original binaries with `unpatch`.
- 2e215ce: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260928.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/4f5ddae224b9529a69b571f631fe3a2cb283162f) commit `4f5ddae224b9529a69b571f631fe3a2cb283162f`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2). Refresh the Oxlint configuration schema from the selected package.

## 0.46.1

### Patch Changes

- 09644ee: `effect-tsgo diagnostics` now reports files it skipped because their tsconfig does not enable the `@effect/language-service` plugin, instead of only printing `Checked 0 files out of N files.` The JSON summary gains a matching `filesWithoutPlugin` count.
  
  ```
  Checked 0 files out of 8 files.
  Skipped 8 files because their tsconfig does not enable the @effect/language-service plugin.
  0 errors, 0 warnings and 0 messages.
  ```
- dfa24e8: Fix `effectFnOpportunity` conversions to preserve enclosing expressions, sibling declarations, comments, literal spelling, and source formatting. Avoid suggesting conversions for function declarations referenced before their declaration.
  
  Wrap non-call pipe arguments in unary callbacks so they do not receive the converted function's arguments. For example, `.pipe(Effect.ignore)` becomes an `Effect.fn` pipeable `_ => Effect.ignore(_)`, while factory calls such as `Effect.map(f)` are retained.
- 8d89b43: Deduplicate strictBooleanExpressions diagnostics when multiple union members display the same type name.
- 7611be0: Preserve Effect plugin options in the Oxlint integration when `tsconfig.json` uses `extends`, whether the options are inherited or declared locally. For example, `effectFn: ["span", "inferred-span", "suggested-span"]` now enables the same `effect-fn-opportunity` reports as an inline configuration without `extends`.

## 0.46.0

### Minor Changes

- 6db6fa6: Add the `catchIfTagToCatchTag` diagnostic and quick fix for replacing direct `_tag` equality predicates in `Effect.catchIf` with `Effect.catchTag`.
- ae1ed02: Add the v4-only `catchRefailToTapError` diagnostic. It suggests `Effect.tapError` when an `Effect.catch` handler sequences an effect with `Effect.andThen` or a zero-argument `Effect.flatMap` callback and then fails with the original, unmodified error. Generator handlers and selective catches are excluded.
- 437b17e: Retain the latest three stable Oxlint releases and the Oxlint runtimes required by the latest three stable Vite+ releases, including their compiler dependencies. Add versioned compatibility profiles so CI tests every distinct retained runtime pair.
  
  The refreshed upstream metadata supports Oxlint 1.77.0, 1.79.0, 1.80.0, 1.81.0, and 1.82.0, covering Vite+ 0.2.9, 0.3.0, and 0.3.1, and advances TypeScript next to 7.1.0-dev.20260911.1.
- 223f9fa: Add the `flatMapIgnoredParamToAndThen` diagnostic and quick fix for replacing zero-parameter `Effect.flatMap` callbacks that return an existing constant Effect value with `Effect.andThen`. Existing diagnostics now also recognize Effect APIs referenced through constant aliases, and their quick fixes rebuild the replacement API from the source file's imported module name without modifying the shared alias.
- 7173762: Add a `strict` diagnostic preset that promotes every default-enabled diagnostic to an error. Expose the complete preset catalog consistently through TypeScript setup and the generated Oxlint presets.
- 9652243: Warn when `diagnosticSeverity` contains an unknown Effect rule name, including in overrides and inherited configurations. For example, `"floatingEfect": "error"` now reports `effect(unknownRuleName)` instead of being silently ignored.
  
  The check uses the fully merged configuration. Set `"unknownRuleName": "off"` to disable it or `"unknownRuleName": "error"` to raise its severity. Local keys are underlined in tsconfig; inherited keys without local syntax produce a diagnostic without a source location.

### Patch Changes

- f110e11: Restore compatibility with TypeScript 7.1.0-dev.20260924.1 by updating diagnostic generation and plugin parsing patches, adapting single-file diagnostic requests to the upstream snapshot API, and regenerating compiler shims and Nix inputs.
- 7c0ddbc: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260920.1), which ships [`TypeScript`](https://github.com/microsoft/TypeScript/commit/f29aeb9f825d96feea27841f3f7342dbf0df68a8) commit `f29aeb9f825d96feea27841f3f7342dbf0df68a8`, and update the repository toolchain to Go 1.27.

## 0.45.0

### Minor Changes

- 066c4b0: Add the opt-in `schemaSync` diagnostic to prefer Effect-based Schema decoding and encoding over `decodeSync`, `decodeUnknownSync`, `encodeSync`, and `encodeUnknownSync` in any context. It recommends the corresponding Effect v3 or v4 method and is enabled by the `effect-native` preset.

### Patch Changes

- bc87c9f: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260909.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/f3b04fe05642d53b4ff126a4af05fe2587b43748) commit `f3b04fe05642d53b4ff126a4af05fe2587b43748`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.44.0

### Minor Changes

- a64e17d: Add `obsoleteMatchImport` diagnostic (`TS377127`) to warn when importing `@effect/match` in projects targeting Effect v4. In Effect v4, pattern matching is built directly into `effect` (`import { Match } from "effect"` or `import * as Match from "effect/Match"`).
- 63331b6: Add `obsoleteSchemaImport` diagnostic (`TS377128`) to warn when importing `@effect/schema` or `@effect/schema/*` in projects targeting Effect v4. In Effect v4, Schema is built directly into `effect` (`import { Schema } from "effect"` or `import * as Schema from "effect/Schema"`).
- 29733f1: Add the `timeoutCatchTagToTimeoutOrElse` diagnostic and quick fixes for Effect v4. Suggest `Effect.timeoutOrElse` for `Effect.timeout` followed by `Effect.catchTag("TimeoutError", ...)`, and `Effect.timeoutOption` for the corresponding Some/None pattern. Only suggest a rewrite when the input error channel excludes `TimeoutError` and the handler does not use the caught error.

### Patch Changes

- a05d76b: Reuse the normalized piping-flow shape and sequence matchers across existing diagnostics.
- d990b0a: Fix `cryptoRandomUUID` and `cryptoRandomUUIDInEffect` diagnostic messages and rule descriptions in Effect v4 to recommend the Effect `Crypto` module instead of `Random`. In Effect v4, `Random` does not provide `randomUUID` and uses non-cryptographic `Math.random`, whereas cryptographic UUID generation is provided by `Crypto.Crypto` (such as `yield* crypto.randomUUIDv4`).
- 7993db9: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260908.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/1f70213d4922b434345f639b441681e470c7cfc1) commit `1f70213d4922b434345f639b441681e470c7cfc1`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.43.0

### Minor Changes

- 956b8d6: Extend `nodeBuiltinImport` to recommend Effect-native alternatives for `console`, `timers`, `timers/promises`, `stream`, `stream/promises`, and `stream/web`, including their `node:` forms.
  
  For Effect v4, also flag `crypto` and `node:crypto` imports and recommend `Crypto` from `effect`. For example, `import { randomUUID } from "node:crypto"` is now diagnosed; use the `Crypto` service's `randomUUIDv4` effect instead. Effect v3 crypto imports remain allowed because that version has no corresponding Crypto service.
  
  Correct the Effect v4 `child_process` recommendation to point to `effect/unstable/process`. The rule remains disabled by default; configure `nodeBuiltinImport` with error severity to prohibit covered imports.

## 0.42.0

### Minor Changes

- 7565750: Add the `matchEffectToMapBoth` style diagnostic and quick fix for replacing `Effect.matchEffect` handlers that return `Effect.fail` and `Effect.succeed` with `Effect.mapBoth`.
- a09feae: Add the `catchAllTagDispatchToCatchTag` style diagnostic and quick fix for replacing manual tagged-error dispatch with `Effect.catchTag` or `Effect.catchTags`.
- 2d501a2: Add the `provideLayerSucceedToProvideService` diagnostic and quick fix for replacing inline `Layer.succeed` and `Layer.effect` provision with direct service provision.
- 46c6e68: Add the `matchEffectToMatch` style diagnostic and quick fix for replacing `Effect.matchEffect` or `Effect.matchCauseEffect` whose handlers only return `Effect.succeed` with their non-effectful counterparts.
  
  Make lazy-expression parsing synchronous and non-generator by default, with flags for callers that explicitly accept thunks, async functions, or generators.
- 59c5fff: Adopt the piping flow parser in more diagnostics.
  
  - `promiseInEffectSuccess`: an explicit promise-success annotation (type arguments on `Effect.succeed`/`as`/`map`/`zipWith`) now suppresses the diagnostic from any position in the surrounding pipe, not only the last argument. `base.pipe(Effect.as<Promise<number>>(promiseValue), Effect.as(promiseValue))` no longer reports, matching the reversed order that was already accepted.
  - `allOfMapToForEach`: now also detects the data-last form expressed through piping flows, e.g. `pipe(values.map(effectful), Effect.all)` and `pipe(values.map(effectful), Effect.all, Effect.asVoid)`, which were previously invisible to the call-expression walk. These matches are diagnostic-only: the existing quick fix remains limited to the standalone `Effect.all(xs.map(f), options?)` call it can safely rewrite.
  - The piping flow parser now keeps the type arguments of parenthesized pipe arguments, e.g. `pipe(x, (Effect.as<...>(v)))`.
  - The piping flow parser now normalizes curried pipeable applications, so `Effect.catch(handler)(effect)` has `effect` as its subject, `Effect.catch` as its callee, and `handler` as its transformation argument. The normalization is limited to calls whose signatures verify that they are the pipeable counterpart of a data-first overload of the same combinator, leaving unrelated curried APIs such as `Effect.fn("name")(body)` unchanged.
- db28a74: Add the `runOfExitToRunExit` diagnostic, which replaces `Effect.runPromise` applied to `Effect.exit` with the dedicated `Effect.runPromiseExit` runner.

### Patch Changes

- 348021d: Update TypeScript next to 7.1.0-dev.20260906.1 and adapt diagnostics snapshot cleanup to its new API while preserving compatibility with stable TypeScript.
- 928506b: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260907.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/1f70213d4922b434345f639b441681e470c7cfc1) commit `1f70213d4922b434345f639b441681e470c7cfc1`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2). Refresh the Oxlint configuration schema from the selected package.

## 0.41.0

### Minor Changes

- d095234: Normalize piping-flow transformations around their callee, arguments, and explicit type arguments without exposing a representation-dependent transformation node. Add shared rewriter operations for replacing transformations and flow prefixes while preserving data-first, data-last, `pipe(...)`, and `.pipe(...)` source forms.
- d3efa70: Remove the AST location parameter from type-only `TypeParser` operations. Effect, Layer, Stream, Schema, service, Context.Tag, Scope, and related type predicates now derive instantiated property types directly from the supplied checker type.

### Patch Changes

- 1d0dfb8: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260903.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/cf7b9361a33fa0c8e1afa3cf45fde29c9ab23ec0) commit `cf7b9361a33fa0c8e1afa3cf45fde29c9ab23ec0`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.40.0

### Minor Changes

- 815273c: Add the Effect v4 `raceFirstWithSleepToTimeout` style diagnostic, which suggests `Effect.timeoutOrElse` when a first-completion race has exactly one `Effect.sleep`- or `Effect.delay`-based timer arm.
  
  Reclassify `acquireReleaseDisposable` as style and `unsafeEffectTypeAssertion` as correctness.

### Patch Changes

- c1ac8e5: Avoid reporting the `schemaNumber` diagnostic when `Schema.Number` is refined with the built-in `isFinite` or `isInt` checks, including pipe-style refinements.
- 0fba9d1: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260902.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/43a90f4c105bc9db7cb7aa299beddafbabe1d23e) commit `43a90f4c105bc9db7cb7aa299beddafbabe1d23e`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.39.1

### Patch Changes

- c2e4353: Update the TypeScript next tag to [`7.1.0-dev.20260901.1`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260901.1), update Oxlint to `1.81.0`, and refresh the generated configuration schemas. Support SchemaStore's draft-07 nullable schema representation when injecting the Effect language-service plugin schema.

## 0.39.0

### Minor Changes

- 7e30dc9: Add the `acquireReleaseDisposable` rule to replace manual disposal finalizers passed to `Effect.acquireRelease` with `Effect.acquireDisposable`.
- f134c31: Add the `optionMatchToFromOption` diagnostic and quick fix for Option-to-Effect conversions that can use `Effect.fromOption`.
  
  The rule recognizes supported `Option.match` call styles and `Option.isSome` / `Option.isNone` conditional expressions whose branches only wrap the value with `Effect.succeed` or produce `Effect.fail`. It suggests the one-argument `Effect.fromOption` form for the default `Cause.NoSuchElementError`, and preserves custom failures in a lazy callback.
- 90c884e: Expose piping-flow extraction through the `etsgoapi` Go API.
  
  The type parser already understands "piping flows" — a subject expression followed by an ordered list of transformations, unifying the `pipe(...)`, pipeable `.pipe(...)`, data-first, data-last and `Effect.fn` forms — but that analysis was only reachable from internal packages. `etsgoapi.TypeParser` now surfaces it so external Go integrations can consume it without importing `internal/...`.
  
  ```go
  tp := etsgoapi.NewTypeParser(program, checker)
  for _, flow := range tp.PipingFlows(sourceFile, true /* includeEffectFn */) {
  	// flow.Subject.Node / flow.Subject.OutType — the starting expression and its type
  	for _, step := range flow.Transformations {
  		// step.Kind (pipe | pipeable | dataFirst | dataLast | call | effectFn | effectFnUntraced)
  		// step.Callee / step.Args / step.OutType
  	}
  }
  ```
  
  Adds the public `PipingFlow`, `PipingFlowSubject`, `PipingFlowTransformation` and `TransformationKind` types alongside the new `(*TypeParser).PipingFlows` method.
- 804c2a9: Add the `flatMapConditionalToFilterOrFail` diagnostic and quick fix, which replaces identity-succeed `Effect.flatMap` conditionals with `Effect.filterOrFail` or `Effect.filterOrElse`.
- 4e14641: Add the `preferSucceedSomeOrNone` diagnostic and autofix for replacing `Effect.succeed(Option.none())` and `Effect.succeed(Option.some(value))`, including equivalent pipe forms, with `Effect.succeedNone` and `Effect.succeedSome(value)`.

### Patch Changes

- fcc641e: Keep the Nix TypeScript source pin synchronized with the TypeScript next metadata so Nix builds compile generated shims against the matching compiler revision.
- 36f2524: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260831.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/9a8581c393a38961489cc8409ae4dfbe97fc25ec) commit `9a8581c393a38961489cc8409ae4dfbe97fc25ec`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.38.0

### Minor Changes

- 773b245: Add the `allOfMapToForEach` style diagnostic, which suggests replacing
  `Effect.all(values.map(callback), options)` with the equivalent
  `Effect.forEach(values, callback, options)` form.
- fe7a29e: Add the `catchAllDieToOrDie` style diagnostic, which suggests replacing
  `Effect.catch` or `Effect.catchAll` with `Effect.orDie` when the catch-all
  handler forwards its typed failure unchanged to `Effect.die`.
- 32c575a: Add the `catchConditionalRefailToCatchIf` diagnostic for conditional `Effect.catch` and `Effect.catchCause` handlers that re-fail their untouched input, suggesting `Effect.catchIf`, `Effect.catchCauseIf`, or `Effect.catchTag` as appropriate.
- 41ec458: Detect the data-first form of `effectMapVoid` by driving detection off piping flows.
  
  `effectMapVoid` previously matched only the data-last / pipeable form of the pattern, so a data-first `Effect.map(self, () => {})` was missed. Detection now runs over the piping-flow transformations, which normalize the subject away, so both forms are reported uniformly and the quick fix rewrites each one correctly.
  
  For example, the data-first call:
  
  ```ts
  Effect.map(Effect.succeed(1), () => {})
  ```
  
  is now flagged and fixed to:
  
  ```ts
  Effect.asVoid(Effect.succeed(1))
  ```
  
  while the existing data-last form (`pipe(self, Effect.map(() => {}))`) continues to be fixed to a bare `Effect.asVoid`.
- 1ab4380: Improve dual function detection by matching alpha-equivalent data-first and pipeable overload declarations without instantiating their generic signatures.
  
  For example, data-first calls such as `Option.match(value, handlers)` now normalize to the same piping-flow representation as `value.pipe(Option.match(handlers))`, even when the overloads reorder generic parameters or return a generic union.
- 2ae20c4: Refactor the `mapSomeToAsSome` mapper detection onto the shared `UnwrapIdentityForwarder` helper. The diagnostic and quick fix now also recognize identity-forwarding function expressions:
  
  ```ts
  // now triggers, alongside Effect.map(Option.some) and Effect.map((v) => Option.some(v))
  numberEffect.pipe(
    Effect.map(function (value) {
      return Option.some(value)
    })
  )
  ```
  
  Annotated or optional mapper parameters and explicit type arguments on the inner `Option.some` call still do not trigger, since they can intentionally widen the resulting `Option` type.
- 2a02aae: Add the `mapSomeToAsSome` diagnostic and quick fix, which replaces `Effect.map(Option.some)` and its exact eta-expanded form with `Effect.asSome` in pipeable and data-first flows.

### Patch Changes

- 68a603e: Bound CI Go caches with a resolved cache identity
  
  `repoctl upstream resolve` now emits a Go cache identity derived from `upstream.json`: the `typescript` component owns and saves its Go caches, `oxlint-tsgolint` restores the compiler objects already cached by the typescript jobs under its TypeScript dependency version (Go's build cache is content-addressed, so the shared compiler packages hit directly), and the Rust-based `oxlint` component skips Go caching entirely instead of saving a duplicate GOCACHE snapshot.
  
  The go-build cache key no longer includes the commit SHA, so a multi-GB cache entry is written once per dependency state instead of on every push to main, and non-materialized setups are restore-only. Previously each push saved ~15GB of duplicate GOCACHE snapshots, evicting every other cache in the repository (GitHub caps repository caches at 10GB) and forcing all validation jobs to run cold.
  
  The generated shim cache key now also hashes `_patches/typescript/**`, so patch changes for the migrated TypeScript compiler repository invalidate cached shims correctly.
- e30bbdb: Fix `supportedEffect` metadata for two rules whose declaration disagreed with their actual behavior:
  
  - `genericEffectServices` declared `["v3", "v4"]` but is gated to run only on Effect v3 projects. It now declares `["v3"]`, so v4 users no longer enable a rule that can never fire.
  - `schemaSyncInEffect` declared `["v3"]` but fully supports Effect v4 (suggesting `decodeEffect`/`encodeEffect` variants). It now declares `["v3", "v4"]`.
  
  `metadata.json` and the generated rule docs were regenerated accordingly. No diagnostic behavior changed.
- 24cbb0d: Fix `repoctl upstream update` failing when a TypeScript commit is reachable from both supported provider repositories.
  
  Previously, `resolveTypeScriptProvider` and `readRemoteTypeScriptGitlink` in `_tools/repoctl/src/upstream.ts` errored out with `TypeScript commit <sha> matched 2 supported repositories` whenever a single revision existed in both `microsoft/TypeScript` (the canonical upstream) and the legacy `microsoft/typescript-go` fork. This blocked the daily "Update Upstreams" workflow for revisions shared between the two.
  
  Both functions now prefer `microsoft/TypeScript` when it matches and fall back to `microsoft/typescript-go` otherwise. A "no supported repository matched" error is still raised when neither matches.
- 7a07115: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260826.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/5739027c9a7df24e27123f453a50c011b37717b6) commit `5739027c9a7df24e27123f453a50c011b37717b6`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.37.0

### Minor Changes

- afb17ac: Support both the legacy `microsoft/typescript-go` compiler and the migrated
  `microsoft/TypeScript/tsc` compiler behind the canonical TypeScript shim
  namespace.

### Patch Changes

- a3af171: Build the Nix package from the migrated TypeScript monorepo source and update the pinned compiler revision correctly.
- eba879b: Suppress the `unnecessaryPipeChain` diagnostic when merging chained pipe calls would exceed the available overload arity.
- 83b8e2a: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260824.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/e9e477458d7b975ed5e43117fb85f6bc87363a6a) commit `e9e477458d7b975ed5e43117fb85f6bc87363a6a`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2). Refresh the Oxlint configuration schema from the selected package.

## 0.36.5

### Patch Changes

- 9ddab5d: Report unsupported target package versions with the current `@effect/tsgo` version and the supported alternatives.
- 922903b: Document the target package versions supported by each published release.
- ca311a5: Exclude type-only heritage nodes from execution flow graphs across supported TypeScript versions.
- 7c54018: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260813.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/34ffe2a2531a226da0046d213899ae0b721229b0) commit `34ffe2a2531a226da0046d213899ae0b721229b0`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.36.4

### Patch Changes

- 7a616ec: Allow declaration emit with `noEmitOnError` when all diagnostics are ignored Effect diagnostics.

## 0.36.3

### Patch Changes

- 73f7f28: Refresh patched binaries when an updated `@effect/tsgo` package provides a different replacement artifact.
- 9190801: Update Effect v4 dependencies and embedded test fixtures to `4.0.0-beta.107`.

## 0.36.2

### Patch Changes

- 8362acc: Extend the walker-rule prefilter to call expressions.
  
  A call expression's type is its resolved signature's return type, and both the signature and that return type are already cached from the main check phase. `NodeCouldBeStrictEffect` now consults them for call nodes and skips the expensive flow-analysis re-check when the declared return type conclusively cannot be a strict Effect. Signature-less calls, optional chains, and every inconclusive return type stay conservative, and `promiseInEffectSuccess` no longer computes a location type for calls only to discard it. Emitted diagnostics are unchanged; on a large Effect monorepo build this removes a further ~4.6% of wall time on top of the reference-node prefilter, bringing the total Effect diagnostics overhead versus a pristine tsgo build of the same commit down to ~17%.
- 9020153: Skip diagnostic rules below the minimum visible severity before executing them.
  
  In `tsc` CLI mode without `includeSuggestionsInTsc`, suggestion- and message-severity diagnostics are dropped from the output after rules run. The rule runner now receives the minimum severity the caller can surface and skips such rules up front, avoiding their type-checker queries entirely. A rule below the threshold still runs when any directive in the file references it (for example `// @effect-diagnostics ruleName:error` or a wildcard), since directives can raise its severity and must be tracked for `unusedDirective` reporting. Emitted diagnostics are unchanged; on a large Effect monorepo build this removes roughly 1–2s of check time.
- 257af25: Skip flow-analysis type queries for references that conclusively cannot be an Effect.
  
  The `effectInFailure` and `promiseInEffectSuccess` rules walk every node of a file and query its flow type just to test whether it is a strict Effect type. The new `TypeParser.NodeCouldBeStrictEffect` prefilter inspects the referenced symbol's declared type first — flow narrowing can only refine the declared type, so a declared type that conclusively contains no possibly-Effect constituent (primitives, plain objects with a different type name, unions thereof) can never produce a strict Effect flow type, and the expensive query is skipped. The predicate is conservative: `any`/`unknown`, type parameters, conditionals, symbol-less types, and deep unions always fall through to the full query. Emitted diagnostics are unchanged; on a large Effect monorepo build this removes ~10% of build wall time (~2.7s of ~26.8s).

## 0.36.1

### Patch Changes

- 4db1d4b: Preserve existing indentation and newline styles when setup updates JSON configuration files.
- c154a04: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260808.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/24fabe95acba758c05fcb349bf427a3a0c8ad676) commit `24fabe95acba758c05fcb349bf427a3a0c8ad676`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.36.0

### Minor Changes

- 8423f68: Add recommended and category-specific shared configurations for Oxlint.

## 0.35.0

### Minor Changes

- f25821b: Generate detailed upstream update pull request descriptions with explicit version changes and the TypeScript-Go commits introduced by the update.

### Patch Changes

- 3fe06c8: Avoid false positive TS2731 diagnostics for symbol-valued interpolations in tagged template literals when Effect diagnostic rules traverse the template expression.
- 989964b: Prevent `preferTypedSchemaDecoder` from panicking when a decoder input is produced by a preceding call or pipe transformation.
- 86c30b1: Persist Effect plugin options in TypeScript build information so incremental builds recheck semantic diagnostics when extended tsconfig plugin settings change.
- b8e7314: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260806.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/86cc4767d4ebadb9b7845d0ab8eb2b05785c3fee) commit `86cc4767d4ebadb9b7845d0ab8eb2b05785c3fee`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.34.0

### Minor Changes

- 974513e: Add a non-interactive `setup` workflow with explicit project, integration, diagnostic, editor, preview, and apply options. Setup now also recommends running the package manager install command whenever it changes `package.json`.

### Patch Changes

- ba4ec9a: Cache generated TypeScript-Go shims by their effective inputs during local repository setup and CI runs.
- 6d0ffda: Remove unused internal helpers and add dead-code analysis to the repository lint workflow.
- 892021a: Use absolute GitHub URLs for links in the published README so documentation links remain clickable on npm.

## 0.33.0

### Minor Changes

- ae0d537: Add the `preferTypedSchemaDecoder` style suggestion, which recommends typed Schema decoder APIs when an input is assignable to the schema's encoded type.

### Patch Changes

- 42534d2: Reuse Go build cache entries across shim generation by preparing package-loading inputs at stable, content-addressed paths.
- cfea036: Update Effect v4 dependencies and embedded test fixtures to `4.0.0-beta.104`, update Schema class completions for the `Error` and `TaggedError` renames, and suppress auto-imports from blocked Effect internal modules.
- ec0b11f: Update the TypeScript next tag to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260805.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/12318e599d21f516defea3b20e5d44b9369da723) commit `12318e599d21f516defea3b20e5d44b9369da723`, and update the TypeScript latest tag to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.32.1

### Patch Changes

- 479f268: Avoid installing redundant Oxlint dependencies when Vite+ is present and identify already-patched components in skip messages.

## 0.32.0

### Minor Changes

- 52ec481: Normalize upstream metadata into versioned TypeScript, Oxlint, and oxlint-tsgolint components, including the versions required by Vite+. Repository setup and validation now operate on unique component versions instead of named upstream profiles.
- 7447a69: Patch Oxlint's TypeScript declarations with the `effecttsgo` plugin and rule names.

## 0.31.0

### Minor Changes

- 9cfdd81: Add a `yield*` quick fix for floating Effects in yieldable Effect contexts, and preserve narrowed wrapper error references when converting hand-rolled reason dispatch to `Effect.catchReason` or `Effect.catchReasons`.

## 0.30.0

### Minor Changes

- f155e33: Ship an Effect-aware Oxlint configuration schema and configure existing `.oxlintrc.json` files to use it when enabling the Oxlint integration. Keep the bundled TypeScript and Oxlint base schemas synchronized through the upstream update workflow.

## 0.29.0

### Minor Changes

- 02035a1: Add TypeScript and Oxlint integration selection to `effect-tsgo setup`, including supported Oxlint dependency pinning and integration-aware prepare scripts.
- 9e21c6c: Split release builds for TSC, tsgolint, and the Oxlint native binding so Go artifacts can be cross-compiled on Linux independently of the platform-specific Rust builds.

## 0.28.0

### Minor Changes

- 0ee639c: Expose Effect code actions as lazy Oxlint suggestions through the renamed `etsoxlintrunner` integration.

## 0.27.1

### Patch Changes

- 421bda3: Install Oxlint cross-compilation targets for its pinned Rust toolchain.

## 0.27.0

### Minor Changes

- 8d18239: Render related Effect diagnostics as labeled Oxlint source ranges and link Oxlint diagnostics to the corresponding Effect rule documentation.

### Patch Changes

- 7c504fe: Cache compiled Rust artifacts across Oxlint validation and release builds.
- 0fc7aa9: Allow the automated version pull request to stay synchronized with main.
- 7ef9550: Safely replace stale automation pull request branches with force-with-lease.
- c026ccf: Update the next profile to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260803.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/5b1047d10d32e7d5b446be4de56b126ff42f82bb) commit `5b1047d10d32e7d5b446be4de56b126ff42f82bb`, and update the latest profile to [`typescript@latest`](https://www.npmjs.com/package/typescript/v/7.0.2).

## 0.26.7

### Patch Changes

- 452eca4: Increase the release profile build timeout to 45 minutes.

## 0.26.6

### Patch Changes

- 23c0f3e: Migrate release automation to Changesets v3 and pnpm 10.

## 0.26.5

### Patch Changes

- 0450ae7: Run Node command shims through the Windows shell so Oxlint release builds can install and build their dependencies.

## 0.26.4

### Patch Changes

- ce6cc3c: Install cross-compilation targets for the Rust toolchain pinned by Oxlint during release builds.

## 0.26.3

### Patch Changes

- c6dde57: Force patch files to use LF line endings so TypeScript-Go patches apply on Windows release runners.

## 0.26.2

### Patch Changes

- b131307: Enable long Git paths before Windows release builds check out recursive submodules.
- af50d36: Fix Windows release builds that invoke Node package-manager command shims.

## 0.26.1

### Patch Changes

- 0bf0586: Enable Oxlint type-aware mode in generated rule configuration examples.

## 0.26.0

### Minor Changes

- 701a607: Add the `abortControllerInEffect` diagnostic for global `AbortController` construction inside Effect generator contexts, suggesting `Effect.abortSignal` for Effect-managed cancellation.
- 58d2eb9: Add the V4-only `catchTagToCatchReason` diagnostic and identity-preserving quick fixes for `Effect.catchTag` handlers that re-fail unmatched `reason._tag` branches.
- bb09f28: Add the `catchChainToFirstSuccessOf` diagnostic for consecutive error-independent `Effect.catch` fallbacks whose error channel is preserved by `Effect.firstSuccessOf`.
- 643022d: Expose `@effect/tsgo/lib/getExePath`, which resolves the packaged `tsc` or `tsc-next` executable matching the installed native TypeScript version.
- 7bbafe4: Generate drift-checked documentation pages for every Effect diagnostic, including annotated previews, metadata, and Language Service and Oxlint configuration examples.
- 41d08c6: Generate native tsgolint adapters and Oxlint registrations that expose Effect diagnostics as qualified `effecttsgo/*` rules, and package the supported tsgolint executables and Oxlint N-API bindings for release.
- c048582: Add the `preferUnsafeConstructor` diagnostic and quickfix: `Effect.runSync` applied directly to a pure effect-package constructor call (e.g. `Effect.runSync(Scope.make())`) is reported when the same module exports a type-equivalent synchronous `*Unsafe` sibling, with a fix rewriting to `Scope.makeUnsafe()` while preserving arguments and type arguments.
- 7af1a90: Add the `promiseInEffectSuccess` diagnostic for Promise types that enter Effect success channels instead of being awaited.
- 087df5f: Rename the public Go integration package from `etsapi` to `etsgoapi`.
- 087df5f: Centralize next, latest, and Oxlint upstream pins in one profile manifest, and bundle that manifest with each platform package for TypeScript binary compatibility.

### Patch Changes

- 1c7dc94: Improve diagnostic performance by caching module export identity checks.
- c9b4499: Create package tags and GitHub Releases with changelog notes when publishing npm packages.
- 4813687: Make shim generation reproducible from canonical configuration and optional overlays, and standardize the TypeScript-Go patch layout.
- 5f4fd3e: Update to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260730.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/37357ae666e7af11989d4ba416a763f2da590dee) commit `37357ae666e7af11989d4ba416a763f2da590dee`.

## 0.25.0

### Minor Changes

- f52d65a: Add the `schemaLiteralNonFinite` diagnostic for statically known `NaN` and infinite values passed to Effect v4 Schema literal constructors.
- 0062f79: Add the `floatingEffectInVitest` diagnostic for Effects returned from non-Effect-aware Vitest tests and hooks.

### Patch Changes

- c746cc7: Ensure the packaged diagnostics binary is executable immediately before launch instead of relying on file permissions preserved during publishing.
- 53ca7a4: Ensure `get-exe-path` marks the Effect Language Service binary as executable before returning its path.
- 88c37c4: Fix `namespaceImportPackages` completions for namespace reexports such as `Effect`, generating `import * as Effect from "effect/Effect"` instead of `import { Effect } from "effect"`.
- b2f5040: Update Effect v4 package references and embedded test fixtures to `4.0.0-beta.101`.
- f75c9b9: Update to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260726.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/8d29e62f3585c2fb5179e4412895aa3a8f40b9f5) commit `8d29e62f3585c2fb5179e4412895aa3a8f40b9f5`.

## 0.24.3

### Patch Changes

- bd05925: Preserve executable permissions on packaged Unix binaries so the diagnostics command can launch them after installation.
- 6c171c9: Document using the Effect LSP for type-aware linting.
- b91f3e8: Avoid `effectInVoidSuccess` introducing TS2589 for assignments with recursive expected types and non-Effect values.
- 874bd2e: Ignore Effect v4 local `Effect.provide(..., { local: true })` calls when reporting chained provides with the `multipleEffectProvide` diagnostic.
- edf29c9: Update to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260722.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/01cbcdd8643cfa17cc8156b60849559c56324601) commit `01cbcdd8643cfa17cc8156b60849559c56324601`.

## 0.24.2

### Patch Changes

- fb9e78b: Return enhanced Effect quickinfo when hovering the asterisk or trailing space in a `yield*` expression.
- 9dd9de9: Update to [`typescript@next`](https://www.npmjs.com/package/typescript/v/7.1.0-dev.20260719.1), which ships [`typescript-go`](https://github.com/microsoft/typescript-go/commit/61aabc2f45771d716e83af114154ae58b64a9705) commit `61aabc2f45771d716e83af114154ae58b64a9705`.

## 0.24.1

### Patch Changes

- fd7a8bb: Avoid excessive type-depth diagnostics when matching data-first and pipeable overloads with complex return types.

## 0.24.0

### Minor Changes

- 7c6a940: Add the off-by-default `preferSchemaTypeProperty` diagnostic and quick fix for replacing `Schema.Schema.Type<typeof X>` with `typeof X.Type`.

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
