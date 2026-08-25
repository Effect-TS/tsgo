# Effect Language Service (TypeScript-Go)

A wrapper around [TypeScript-Go](https://github.com/microsoft/TypeScript-Go) that builds the Effect Language Service, providing Effect-TS diagnostics and quick fixes.
This project targets **Effect V4** (codename: "smol") primarily and also Effect V3.

## Installation

The setup of the TSGO version of the LSP can be performed via the command line interface:

```bash
# Interactive and guided setup for human friends
npx @effect/tsgo setup
# Non-interactive, for LLM friends
npx @effect/tsgo setup --help
```

This will guide you through the installation process, which includes:
1. Adding the `@effect/tsgo` dependency to your project.
2. Configuring your `tsconfig.json` to use the Effect Language Service plugin.
3. Adjusting plugin options to your preference.
4. Hinting at any additional editor configuration needed to ensure the LSP is active.

> [!NOTE]
> At the moment, you still need a native TypeScript install alongside `@effect/tsgo`: `typescript` >= 7 (e.g. `typescript@latest` or `typescript@next`) or an alias such as `@typescript/native`. `effect-tsgo patch` tries `typescript`, then `@typescript/native`, and accepts `--typescript-package <name>` to try a custom package name first.

## LSP-based linter

The Effect LSP doubles as a tool to perform type-aware linting of Effect code, and ships as well a way to emit additional Oxlint Type Aware rules.

Linting can occur either during the `tsc` typecheck phase (with the benefit of running typechecking only once and caching the output), or via a dedicated `npx @effect/tsgo diagnostics --project tsconfig.json` command (with typechecking occurring again), or via the Oxlint Patch.

<a href="https://github.com/Effect-TS/tsgo/blob/main/docs/README.md">See the Oxlint Setup guide</a> for instructions on how to install and configure Oxlint with the Effect LSP.

When running in `tsc` mode, the Effect diagnostics are emitted as standard TypeScript diagnostics, and can be configured to affect the `tsc` exit code through the options `ignoreEffectSuggestionsInTscExitCode`, `ignoreEffectWarningsInTscExitCode`, and `ignoreEffectErrorsInTscExitCode`.

When running in dedicated diagnostics mode, the Effect diagnostics can be emitted in structured formats, which can be further processed by other tools.

<!-- supported-components:start -->
## Supported Package Versions

The following target package versions are supported by `@effect/tsgo@0.37.0`:

| Component | Supported versions |
|---|---|
| TypeScript | `7.0.2`, `7.1.0-dev.20260824.1` |
| Oxlint | `1.79.0`, `1.80.0` |
| oxlint-tsgolint | `7.0.2001` |
<!-- supported-components:end -->

## Diagnostic Status

Some diagnostics are off by default or have a default severity of suggestion, but you can always enable them or change their default severity in the plugin options.

<!-- diagnostics-table:start -->
<table>
  <thead>
    <tr><th>Rule</th><th>Description</th></tr>
  </thead>
  <tbody>
    <tr><td colspan="2"><strong>Correctness</strong> <em>Wrong, unsafe, or structurally invalid code patterns.</em></td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/any-unknown-in-error-context.md"><code>anyUnknownInErrorContext</code></a></td><td>Detects &#39;any&#39; or &#39;unknown&#39; types in Effect error or requirements channels</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/class-self-mismatch.md"><code>classSelfMismatch</code></a></td><td>Ensures Self type parameter matches the class name in Context/Service/Tag/Schema classes</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/duplicate-package.md"><code>duplicatePackage</code></a></td><td>Warns when multiple versions of an Effect-related package are detected in the program</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-fn-implicit-any.md"><code>effectFnImplicitAny</code></a></td><td>Mirrors noImplicitAny for unannotated Effect.fn, Effect.fnUntraced, and Effect.fnUntracedEager callback parameters when no outer contextual function type exists. Requires TS&#39;s noImplicitAny: true</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/floating-effect.md"><code>floatingEffect</code></a></td><td>Detects Effect values that are neither yielded nor assigned</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/floating-effect-in-vitest.md"><code>floatingEffectInVitest</code></a></td><td>Detects Effects returned from non-Effect-aware Vitest callbacks</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/generic-effect-services.md"><code>genericEffectServices</code></a></td><td>Prevents services with type parameters that cannot be discriminated at runtime</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/missing-effect-context.md"><code>missingEffectContext</code></a></td><td>Detects Effect values with unhandled context requirements</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/missing-effect-error.md"><code>missingEffectError</code></a></td><td>Detects Effect values with unhandled error types</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/missing-layer-context.md"><code>missingLayerContext</code></a></td><td>Detects Layer values with unhandled context requirements</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/missing-return-yield-star.md"><code>missingReturnYieldStar</code></a></td><td>Suggests using return yield* for Effects that never succeed</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/missing-star-in-yield-effect-gen.md"><code>missingStarInYieldEffectGen</code></a></td><td>Detects bare yield (without *) inside Effect generator scopes</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/non-object-effect-service-type.md"><code>nonObjectEffectServiceType</code></a></td><td>Ensures Effect.Service types are objects, not primitives</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/outdated-api.md"><code>outdatedApi</code></a></td><td>Detects usage of APIs that have been removed or renamed in Effect v4</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/overridden-schema-constructor.md"><code>overriddenSchemaConstructor</code></a></td><td>Prevents overriding constructors in Schema classes which breaks decoding behavior</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/promise-in-effect-success.md"><code>promiseInEffectSuccess</code></a></td><td>Detects Promise types in Effect success channels where they are not awaited</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/schema-literal-non-finite.md"><code>schemaLiteralNonFinite</code></a></td><td>Reports statically known non-finite numbers passed to Schema literal constructors</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/schema-opaque-instance-member.md"><code>schemaOpaqueInstanceMember</code></a></td><td>Disallows instance members in classes extending Schema.Opaque</td></tr>
    <tr><td colspan="2"><strong>Anti-pattern</strong> <em>Discouraged patterns that often lead to bugs or confusing behavior.</em></td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/catch-unfailable-effect.md"><code>catchUnfailableEffect</code></a></td><td>Warns when using error handling on Effects that never fail</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-fn-iife.md"><code>effectFnIife</code></a></td><td>Effect.fn or Effect.fnUntraced is called as an IIFE; use Effect.gen instead</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-gen-uses-adapter.md"><code>effectGenUsesAdapter</code></a></td><td>Warns when using the deprecated adapter parameter in Effect.gen</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-in-failure.md"><code>effectInFailure</code></a></td><td>Warns when an Effect is used inside an Effect failure channel</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-in-void-success.md"><code>effectInVoidSuccess</code></a></td><td>Detects nested Effects in void success channels that may cause unexecuted effects</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-error-in-effect-catch.md"><code>globalErrorInEffectCatch</code></a></td><td>Warns when catch callbacks return global Error type instead of typed errors</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-error-in-effect-failure.md"><code>globalErrorInEffectFailure</code></a></td><td>Warns when the global Error type is used in an Effect failure channel</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/layer-merge-all-with-dependencies.md"><code>layerMergeAllWithDependencies</code></a></td><td>Detects interdependencies in Layer.mergeAll calls where one layer provides a service that another layer requires</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/lazy-effect.md"><code>lazyEffect</code></a></td><td>Suggests avoiding exported zero-argument functions and service members that lazily return Effect or Stream values</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/lazy-promise-in-effect-sync.md"><code>lazyPromiseInEffectSync</code></a></td><td>Warns when Effect.sync lazily returns a Promise instead of using an async Effect constructor</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/leaking-requirements.md"><code>leakingRequirements</code></a></td><td>Detects implementation services leaked in service methods</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/multiple-effect-provide.md"><code>multipleEffectProvide</code></a></td><td>Warns against chaining Effect.provide calls which can cause service lifecycle issues</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/prefer-unsafe-constructor.md"><code>preferUnsafeConstructor</code></a></td><td>Suggests replacing Effect.runSync of a pure effect constructor with the synchronous *Unsafe variant exported by the same module</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/return-effect-in-gen.md"><code>returnEffectInGen</code></a></td><td>Warns when returning an Effect in a generator causes nested Effect&lt;Effect&lt;...&gt;&gt;</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/run-effect-inside-effect.md"><code>runEffectInsideEffect</code></a></td><td>Suggests using Runtime or Effect.run*With methods instead of Effect.run* inside Effect contexts</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/schema-sync-in-effect.md"><code>schemaSyncInEffect</code></a></td><td>Suggests using Effect-based Schema methods instead of sync methods inside Effect generators</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/scope-in-layer-effect.md"><code>scopeInLayerEffect</code></a></td><td>Suggests using Layer.scoped instead of Layer.effect when Scope is in requirements</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/strict-effect-provide.md"><code>strictEffectProvide</code></a></td><td>Warns when using Effect.provide with layers outside of application entry points</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/try-catch-in-effect-gen.md"><code>tryCatchInEffectGen</code></a></td><td>Discourages try/catch in Effect generators in favor of Effect error handling</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/unknown-in-effect-catch.md"><code>unknownInEffectCatch</code></a></td><td>Warns when catch callbacks return unknown instead of typed errors</td></tr>
    <tr><td colspan="2"><strong>Effect-native</strong> <em>Prefer Effect-native APIs and abstractions when available.</em></td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/abort-controller-in-effect.md"><code>abortControllerInEffect</code></a></td><td>Warns when manually constructing AbortController inside Effect generators instead of using Effect.abortSignal</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/async-function.md"><code>asyncFunction</code></a></td><td>Warns when declaring async functions and suggests using Effect values and Effect.gen for async control flow</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/crypto-random-uuid.md"><code>cryptoRandomUUID</code></a></td><td>Warns when using crypto.randomUUID() outside Effect generators instead of the Effect Random module, which uses Effect-injected randomness rather than the crypto module behind the scenes</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/crypto-random-uuid-in-effect.md"><code>cryptoRandomUUIDInEffect</code></a></td><td>Warns when using crypto.randomUUID() inside Effect generators instead of the Effect Random module, which uses Effect-injected randomness rather than the crypto module behind the scenes</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/extends-native-error.md"><code>extendsNativeError</code></a></td><td>Warns when a class directly extends the native Error class</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-console.md"><code>globalConsole</code></a></td><td>Warns when using console methods outside Effect generators instead of Effect.log/Logger</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-console-in-effect.md"><code>globalConsoleInEffect</code></a></td><td>Warns when using console methods inside Effect generators instead of Effect.log/Logger</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-date.md"><code>globalDate</code></a></td><td>Warns when using Date.now() or new Date() outside Effect generators instead of Clock/DateTime</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-date-in-effect.md"><code>globalDateInEffect</code></a></td><td>Warns when using Date.now() or new Date() inside Effect generators instead of Clock/DateTime</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-fetch.md"><code>globalFetch</code></a></td><td>Warns when using the global fetch function outside Effect generators instead of the Effect HTTP client</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-fetch-in-effect.md"><code>globalFetchInEffect</code></a></td><td>Warns when using the global fetch function inside Effect generators instead of the Effect HTTP client</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-random.md"><code>globalRandom</code></a></td><td>Warns when using Math.random() outside Effect generators instead of the Random service</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-random-in-effect.md"><code>globalRandomInEffect</code></a></td><td>Warns when using Math.random() inside Effect generators instead of the Random service</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-timers.md"><code>globalTimers</code></a></td><td>Warns when using setTimeout/setInterval outside Effect generators instead of Effect.sleep/Schedule</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/global-timers-in-effect.md"><code>globalTimersInEffect</code></a></td><td>Warns when using setTimeout/setInterval inside Effect generators instead of Effect.sleep/Schedule</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/instance-of-schema.md"><code>instanceOfSchema</code></a></td><td>Suggests using Schema.is instead of instanceof for Effect Schema types</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/new-promise.md"><code>newPromise</code></a></td><td>Warns when constructing promises with new Promise instead of using Effect APIs</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/node-builtin-import.md"><code>nodeBuiltinImport</code></a></td><td>Warns when importing Node.js built-in modules that have Effect-native counterparts</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/prefer-schema-over-json.md"><code>preferSchemaOverJson</code></a></td><td>Suggests using Effect Schema for JSON operations instead of JSON.parse/JSON.stringify</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/process-env.md"><code>processEnv</code></a></td><td>Warns when reading process.env outside Effect generators instead of using Effect Config</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/process-env-in-effect.md"><code>processEnvInEffect</code></a></td><td>Warns when reading process.env inside Effect generators instead of using Effect Config</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/unsafe-effect-type-assertion.md"><code>unsafeEffectTypeAssertion</code></a></td><td>Detects unsafe type assertions that narrow Effect, Stream, or Layer error or requirements channels</td></tr>
    <tr><td colspan="2"><strong>Style</strong> <em>Cleanup, consistency, and idiomatic Effect code.</em></td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/all-of-map-to-for-each.md"><code>allOfMapToForEach</code></a></td><td>Suggests using Effect.forEach instead of Effect.all over an effectful Array#map</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/catch-all-to-map-error.md"><code>catchAllToMapError</code></a></td><td>Suggests using Effect.mapError instead of Effect.catch + Effect.fail</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/catch-chain-to-first-success-of.md"><code>catchChainToFirstSuccessOf</code></a></td><td>Suggests Effect.firstSuccessOf for consecutive error-independent Effect.catch fallbacks when the error type is preserved</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/catch-tag-to-catch-reason.md"><code>catchTagToCatchReason</code></a></td><td>Suggests Effect.catchReason or Effect.catchReasons for handlers that re-fail unmatched reason._tag branches</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/catch-to-ignore.md"><code>catchToIgnore</code></a></td><td>Suggests using Effect.ignore or Effect.ignoreCause instead of Effect.catch/catchCause returning Effect.void</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/catch-to-or-else-succeed.md"><code>catchToOrElseSucceed</code></a></td><td>Suggests using Effect.orElseSucceed instead of Effect.catch + Effect.succeed</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/deterministic-keys.md"><code>deterministicKeys</code></a></td><td>Enforces deterministic naming for service/tag/error identifiers based on class names</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-do-notation.md"><code>effectDoNotation</code></a></td><td>Suggests using Effect.gen or Effect.fn instead of the Effect.Do notation helpers</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-fn-opportunity.md"><code>effectFnOpportunity</code></a></td><td>Suggests using Effect.fn for functions that return an Effect</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-map-flatten.md"><code>effectMapFlatten</code></a></td><td>Suggests using Effect.flatMap instead of Effect.map followed by Effect.flatten in piping flows</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-map-void.md"><code>effectMapVoid</code></a></td><td>Suggests using Effect.asVoid instead of Effect.map(() =&gt; void 0), Effect.map(() =&gt; undefined), or Effect.map(() =&gt; {})</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/effect-succeed-with-void.md"><code>effectSucceedWithVoid</code></a></td><td>Suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0)</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/flat-map-to-map.md"><code>flatMapToMap</code></a></td><td>Suggests using Effect.map instead of Effect.flatMap when the callback only wraps its result with Effect.succeed</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/map-some-to-as-some.md"><code>mapSomeToAsSome</code></a></td><td>Suggests using Effect.asSome instead of Effect.map when the mapper only wraps the success value with Option.some</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/missed-pipeable-opportunity.md"><code>missedPipeableOpportunity</code></a></td><td>Suggests using .pipe() for nested function calls</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/missing-effect-service-dependency.md"><code>missingEffectServiceDependency</code></a></td><td>Checks that Effect.Service dependencies satisfy all required layer inputs</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/missing-pipeable-signature.md"><code>missingPipeableSignature</code></a></td><td>Reports exported fixed-arity functions whose call signatures have no corresponding pipeable overload</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/multiple-catch-tag.md"><code>multipleCatchTag</code></a></td><td>Suggests collapsing consecutive Effect.catchTag transformations into a single Effect.catchTags call when semantics stay equivalent</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/nested-effect-gen-yield.md"><code>nestedEffectGenYield</code></a></td><td>Warns when yielding a nested bare Effect.gen inside an existing Effect generator context</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/new-schema-class.md"><code>newSchemaClass</code></a></td><td>Suggests using Schema make instead of new for Schema classes</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/prefer-schema-type-property.md"><code>preferSchemaTypeProperty</code></a></td><td>Disallows Schema.Schema.Type&lt;typeof X&gt; in favor of typeof X.Type</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/prefer-typed-schema-decoder.md"><code>preferTypedSchemaDecoder</code></a></td><td>Suggests typed Schema decoders when the input is assignable to the schema&#39;s Encoded type</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/redundant-map-error.md"><code>redundantMapError</code></a></td><td>Suggests hoisting a repeated trailing Effect.mapError from every yield in an Effect generator</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/redundant-or-die.md"><code>redundantOrDie</code></a></td><td>Suggests hoisting a repeated trailing Effect.orDie from every yield in an Effect generator</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/redundant-schema-tag-identifier.md"><code>redundantSchemaTagIdentifier</code></a></td><td>Suggests removing redundant identifier argument when it equals the tag value in Schema.TaggedClass/TaggedError/TaggedRequest</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/schema-number.md"><code>schemaNumber</code></a></td><td>Suggests Schema.Finite and Schema.FiniteFromString instead of Schema.Number APIs when describing domain numbers</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/schema-struct-with-tag.md"><code>schemaStructWithTag</code></a></td><td>Suggests using Schema.TaggedStruct instead of Schema.Struct with _tag field</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/schema-union-of-literals.md"><code>schemaUnionOfLiterals</code></a></td><td>Suggests combining multiple Schema.Literal calls in Schema.Union into a single Schema.Literal</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/service-not-as-class.md"><code>serviceNotAsClass</code></a></td><td>Warns when Context.Service is used as a variable instead of a class declaration</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/strict-boolean-expressions.md"><code>strictBooleanExpressions</code></a></td><td>Enforces boolean types in conditional expressions for type safety</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/sync-to-succeed.md"><code>syncToSucceed</code></a></td><td>Suggests using Effect.succeed instead of Effect.sync when the thunk returns a constant value</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/unnecessary-arrow-block.md"><code>unnecessaryArrowBlock</code></a></td><td>Suggests using a concise arrow body when the block only returns an expression</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/unnecessary-effect-gen.md"><code>unnecessaryEffectGen</code></a></td><td>Suggests removing Effect.gen when it contains only a single return statement</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/unnecessary-fail-yieldable-error.md"><code>unnecessaryFailYieldableError</code></a></td><td>Suggests yielding yieldable errors directly instead of wrapping with Effect.fail</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/unnecessary-pipe.md"><code>unnecessaryPipe</code></a></td><td>Removes pipe calls with no arguments</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/unnecessary-pipe-chain.md"><code>unnecessaryPipeChain</code></a></td><td>Simplifies chained pipe calls into a single pipe call</td></tr>
    <tr><td><a href="https://github.com/Effect-TS/tsgo/blob/main/docs/rules/unnecessary-typeof-type.md"><code>unnecessaryTypeofType</code></a></td><td>Suggests replacing typeof Schema.Type style annotations with the matching named type when available</td></tr>
  </tbody>
</table>
<!-- diagnostics-table:end -->

## Refactor Status

| Refactor | V3 | V4 | Notes |
|----------|----|----|-------|
| `asyncAwaitToFn` | ✅ | ✅ | Convert async/await to Effect.fn |
| `asyncAwaitToFnTryPromise` | ✅ | ✅ | Convert async/await to Effect.fn with Error ADT + tryPromise |
| `asyncAwaitToGen` | ✅ | ✅ | Convert async/await to Effect.gen |
| `asyncAwaitToGenTryPromise` | ✅ | ✅ | Convert async/await to Effect.gen with Error ADT + tryPromise |
| `debugPerformance` | ❌ | ❌ | Insert performance timing debug comments |
| `effectGenToFn` | ✅ | ✅ | Convert Effect.gen to Effect.fn |
| `functionToArrow` | ✅ | ✅ | Convert function declaration to arrow function |
| `layerMagic` | ✅ | ✅ | Auto-compose layers with correct merge/provide |
| `makeSchemaOpaque` | ✅ | ✅ | Convert Schema to opaque type aliases |
| `makeSchemaOpaqueWithNs` | ✅ | ✅ | Convert Schema to opaque types with namespace |
| `pipeableToDatafirst` | ✅ | ✅ | Convert pipeable calls to data-first style |
| `removeUnnecessaryEffectGen` | ✅ | ✅ | Remove redundant Effect.gen wrapper |
| `structuralTypeToSchema` | ✅ | ✅ | Generate recursive Schema from type alias |
| `toggleLazyConst` | ✅ | ✅ | Toggle lazy/eager const declarations |
| `togglePipeStyle` | ✅ | ✅ | Toggle pipe(x, f) vs x.pipe(f) |
| `toggleReturnTypeAnnotation` | ✅ | ✅ | Add/remove return type annotation |
| `toggleTypeAnnotation` | ✅ | ✅ | Add/remove variable type annotation |
| `typeToEffectSchema` | ✅ | ✅ | Generate Effect.Schema from type alias |
| `typeToEffectSchemaClass` | ✅ | ✅ | Generate Schema.Class from type alias |
| `wrapWithEffectGen` | ✅ | ✅ | Wrap expression in Effect.gen |
| `wrapWithPipe` | ❌ | ✅ | Wrap selection in pipe(...) |
| `writeTagClassAccessors` | ✅ | ➖ | Generate static accessors for Effect.Service/Tag classes |

### Completion Status

| Completion | V3 | V4 | Notes |
|------------|----|----|-------|
| `contextSelfInClasses` | ✅ | ➖ | Context.Tag self-type snippets in extends clauses (V3-only) |
| `effectDataClasses` | ✅ | ✅ | Data class constructor snippets in extends clauses |
| `effectSchemaSelfInClasses` | ✅ | ✅ | Schema/Model class constructor snippets in extends clauses |
| `effectSelfInClasses` | ✅ | ➖ | Effect.Service/Effect.Tag self-type snippets in extends clauses (V3-only) |
| `genFunctionStar` | ✅ | ✅ | `gen(function*(){})` snippet when dot-accessing `.gen` on objects with callable gen property |
| `effectCodegensComment` | ✅ | ✅ | `@effect-codegens` directive snippet in comments with codegen name choices |
| `effectDiagnosticsComment` | ✅ | ✅ | `@effect-diagnostics` / `@effect-diagnostics-next-line` directive snippets in comments |
| `rpcMakeClasses` | ✅ | ➖ | `Rpc.make` constructor snippet in extends clauses (V3-only) |
| `schemaBrand` | ✅ | ➖ | `brand("varName")` snippet when dot-accessing Schema in variable declarations (V3-only) |
| `serviceMapSelfInClasses` | ✅ | ✅ | Service map self-type snippets in extends clauses |

## Best Practices

### Relationship to Official TypeScript-Go (`tsgo`)

Effect-tsgo is a **superset** of the official [TypeScript-Go](https://github.com/microsoft/TypeScript-Go) — it embeds a pinned version of `tsgo` with a small patch set on top and adds the Effect language service. This means `effect-tsgo` provides all standard TypeScript-Go functionality plus Effect-specific diagnostics, quick fixes, and refactors.

**Use `effect-tsgo` instead of `tsgo`, not alongside it.** Running both in parallel will produce duplicate diagnostics and degrade editor performance. Configure your editor to use `effect-tsgo` as your sole TypeScript language server.

### Version Pinning

Each release of `effect-tsgo` is built against the versioned components recorded in `_packages/tsgo/upstream.json`. The Nix flake consumes the TypeScript `next` tag directly. When upstream `tsgo` releases new features or fixes, `effect-tsgo` will adopt them in a subsequent release after validating compatibility with the Effect diagnostics layer.

### When to Upgrade

- Upgrade `effect-tsgo` when a new release includes upstream `tsgo` fixes you need or new Effect diagnostics you want.
- There is no need to track upstream `tsgo` releases separately — `effect-tsgo` is the single binary to manage.

## Plugin Options

<!-- example-config:start -->
```jsonc
{
  "compilerOptions": {
    "plugins": [
      {
        "name": "@effect/language-service",
        // Controls Effect refactors. (default: true)
        "refactors": true,
        // Controls Effect diagnostics. (default: true)
        "diagnostics": true,
        // When false, suggestion-level Effect diagnostics are omitted from tsc CLI output. (default: true)
        "includeSuggestionsInTsc": true,
        // Controls Effect quickinfo. (default: true)
        "quickinfo": true,
        // Controls Effect completions. (default: true)
        "completions": true,
        // Enables additional debug-only Effect language service output. (default: false)
        "debug": false,
        // Controls Effect goto references support. (default: true)
        "goto": true,
        // Controls Effect rename helpers. (default: true)
        "renames": true,
        // When true, suggestion diagnostics do not affect the tsc exit code. (default: true)
        "ignoreEffectSuggestionsInTscExitCode": true,
        // When true, warning diagnostics do not affect the tsc exit code. (default: false)
        "ignoreEffectWarningsInTscExitCode": false,
        // When true, error diagnostics do not affect the tsc exit code. (default: false)
        "ignoreEffectErrorsInTscExitCode": false,
        // When true, disabled diagnostics are still processed so directives can re-enable them. (default: false)
        "skipDisabledOptimization": false,
        // Mermaid rendering service for layer graph links. Accepts mermaid.live, mermaid.com, or a custom URL. (default: "mermaid.live")
        "mermaidProvider": "mermaid.live",
        // When true, suppresses external Mermaid links in hover output. (default: false)
        "noExternal": false,
        // How many levels deep the layer graph extraction follows symbol references. (default: 0)
        "layerGraphFollowDepth": 0,
        // When true, suppresses redundant return-type inlay hints on supported Effect generator functions. (default: false)
        "inlays": false,
        // Package names that should prefer namespace imports. (default: [])
        "namespaceImportPackages": [],
        // Package names that should prefer barrel named imports. (default: [])
        "barrelImportPackages": [],
        // Package-level import aliases keyed by package name. (default: {})
        "importAliases": {},
        // Controls whether named reexports are followed at package top-level. (default: "ignore")
        "topLevelNamedReexports": "ignore",
        // Configures key pattern formulas for the deterministicKeys rule. (default: [{"target":"service","pattern":"default","skipLeadingPath":["src/"]},{"target":"custom","pattern":"default","skipLeadingPath":["src/"]}])
        "keyPatterns": [
          {
            "target": "service",
            "pattern": "default",
            "skipLeadingPath": [
              "src/"
            ]
          },
          {
            "target": "custom",
            "pattern": "default",
            "skipLeadingPath": [
              "src/"
            ]
          }
        ],
        // Enables matching constructors with @effect-identifier annotations. (default: false)
        "extendedKeyDetection": false,
        // Minimum number of contiguous pipeable transformations to trigger missedPipeableOpportunity. (default: 2)
        "pipeableMinArgCount": 2,
        // Package names allowed to have multiple versions without triggering duplicatePackage. (default: [])
        "allowedDuplicatedPackages": [],
        // Controls which effectFnOpportunity quickfix variants are offered. (default: ["span"])
        "effectFn": [
          "span"
        ],
        // Maps rule names to severity levels. Use {} to enable diagnostics with rule defaults. (default: {})
        "diagnosticSeverity": {},
        // Ordered per-file diagnostic option overrides. (default: [{"include":["src/**/*.ts"],"options":{"diagnosticSeverity":{"floatingEffect":"error"}}}])
        "overrides": [
          {
            "include": [
              "src/**/*.ts"
            ],
            "options": {
              "diagnosticSeverity": {
                "floatingEffect": "error"
              }
            }
          }
        ]
      }
    ]
  }
}
```
<!-- example-config:end -->
