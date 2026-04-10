# Effect Language Service (TypeScript-Go)

A wrapper around [TypeScript-Go](https://github.com/nicolo-ribaudo/TypeScript-Go) that builds the Effect Language Service, providing Effect-TS diagnostics and quick fixes.
This project targets **Effect V4** (codename: "smol") primarily and also Effect V3.

## Currently in Alpha
The TypeScript-Go version of the Effect LSP should be considered in Alpha. Expect breaking changes between releases and some missing features compared to previous version.
Some of them are currently on hold due to not yet complete pipeline on the upstream TypeScript repository.

## Installation

The setup of the TSGO version of the LSP can be performed via the command line interface:

```bash
npx @effect/tsgo setup
```

This will guide you through the installation process, which includes:
1. Adding the `@effect/tsgo` dependency to your project.
2. Configuring your `tsconfig.json` to use the Effect Language Service plugin.
3. Adjusting plugin options to your preference.
4. Hinting at any additional editor configuration needed to ensure the LSP is active.

> [!NOTE]
> At the moment, you still need the standard native TypeScript install (`@typescript/native-preview`) alongside `@effect/tsgo`.

## Diagnostic Status

Some diagnostics are off by default or have a default severity of suggestion, but you can always enable them or change their default severity in the plugin options.

<!-- diagnostics-table:start -->
<table>
  <thead>
    <tr><th>Diagnostic</th><th>Sev</th><th>Fix</th><th>Description</th><th>v3</th><th>v4</th></tr>
  </thead>
  <tbody>
    <tr><td colspan="6"><strong>Correctness</strong> <em>Wrong, unsafe, or structurally invalid code patterns.</em></td></tr>
    <tr><td><code>anyUnknownInErrorContext</code></td><td>➖</td><td></td><td>Detects &#39;any&#39; or &#39;unknown&#39; types in Effect error or requirements channels</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>classSelfMismatch</code></td><td>❌</td><td>🔧</td><td>Ensures Self type parameter matches the class name in Context/Service/Tag/Schema classes</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>duplicatePackage</code></td><td>⚠️</td><td></td><td>Warns when multiple versions of an Effect-related package are detected in the program</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectFnImplicitAny</code></td><td>❌</td><td></td><td>Mirrors noImplicitAny for unannotated Effect.fn, Effect.fnUntraced, and Effect.fnUntracedEager callback parameters when no outer contextual function type exists. Requires TS&#39;s noImplicitAny: true</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>floatingEffect</code></td><td>❌</td><td></td><td>Detects Effect values that are neither yielded nor assigned</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>genericEffectServices</code></td><td>⚠️</td><td></td><td>Prevents services with type parameters that cannot be discriminated at runtime</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>missingEffectContext</code></td><td>❌</td><td></td><td>Detects Effect values with unhandled context requirements</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>missingEffectError</code></td><td>❌</td><td>🔧</td><td>Detects Effect values with unhandled error types</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>missingLayerContext</code></td><td>❌</td><td></td><td>Detects Layer values with unhandled context requirements</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>missingReturnYieldStar</code></td><td>❌</td><td>🔧</td><td>Suggests using return yield* for Effects that never succeed</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>missingStarInYieldEffectGen</code></td><td>❌</td><td>🔧</td><td>Detects bare yield (without *) inside Effect generator scopes</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>nonObjectEffectServiceType</code></td><td>❌</td><td></td><td>Ensures Effect.Service types are objects, not primitives</td><td>✓</td><td></td></tr>
    <tr><td><code>outdatedApi</code></td><td>⚠️</td><td></td><td>Detects usage of APIs that have been removed or renamed in Effect v4</td><td></td><td>✓</td></tr>
    <tr><td><code>overriddenSchemaConstructor</code></td><td>❌</td><td>🔧</td><td>Prevents overriding constructors in Schema classes which breaks decoding behavior</td><td>✓</td><td>✓</td></tr>
    <tr><td colspan="6"><strong>Anti-pattern</strong> <em>Discouraged patterns that often lead to bugs or confusing behavior.</em></td></tr>
    <tr><td><code>catchUnfailableEffect</code></td><td>💡</td><td></td><td>Warns when using error handling on Effects that never fail</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectFnIife</code></td><td>⚠️</td><td>🔧</td><td>Effect.fn or Effect.fnUntraced is called as an IIFE; use Effect.gen instead</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectGenUsesAdapter</code></td><td>⚠️</td><td></td><td>Warns when using the deprecated adapter parameter in Effect.gen</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectInFailure</code></td><td>⚠️</td><td></td><td>Warns when an Effect is used inside an Effect failure channel</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectInVoidSuccess</code></td><td>⚠️</td><td></td><td>Detects nested Effects in void success channels that may cause unexecuted effects</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalErrorInEffectCatch</code></td><td>⚠️</td><td></td><td>Warns when catch callbacks return global Error type instead of typed errors</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalErrorInEffectFailure</code></td><td>⚠️</td><td></td><td>Warns when the global Error type is used in an Effect failure channel</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>layerMergeAllWithDependencies</code></td><td>⚠️</td><td>🔧</td><td>Detects interdependencies in Layer.mergeAll calls where one layer provides a service that another layer requires</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>lazyPromiseInEffectSync</code></td><td>⚠️</td><td></td><td>Warns when Effect.sync lazily returns a Promise instead of using an async Effect constructor</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>leakingRequirements</code></td><td>💡</td><td></td><td>Detects implementation services leaked in service methods</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>multipleEffectProvide</code></td><td>⚠️</td><td>🔧</td><td>Warns against chaining Effect.provide calls which can cause service lifecycle issues</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>returnEffectInGen</code></td><td>💡</td><td>🔧</td><td>Warns when returning an Effect in a generator causes nested Effect&lt;Effect&lt;...&gt;&gt;</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>runEffectInsideEffect</code></td><td>💡</td><td>🔧</td><td>Suggests using Runtime or Effect.run*With methods instead of Effect.run* inside Effect contexts</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>schemaSyncInEffect</code></td><td>💡</td><td></td><td>Suggests using Effect-based Schema methods instead of sync methods inside Effect generators</td><td>✓</td><td></td></tr>
    <tr><td><code>scopeInLayerEffect</code></td><td>⚠️</td><td>🔧</td><td>Suggests using Layer.scoped instead of Layer.effect when Scope is in requirements</td><td>✓</td><td></td></tr>
    <tr><td><code>strictEffectProvide</code></td><td>➖</td><td></td><td>Warns when using Effect.provide with layers outside of application entry points</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>tryCatchInEffectGen</code></td><td>💡</td><td></td><td>Discourages try/catch in Effect generators in favor of Effect error handling</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>unknownInEffectCatch</code></td><td>⚠️</td><td></td><td>Warns when catch callbacks return unknown instead of typed errors</td><td>✓</td><td>✓</td></tr>
    <tr><td colspan="6"><strong>Effect-native</strong> <em>Prefer Effect-native APIs and abstractions when available.</em></td></tr>
    <tr><td><code>asyncFunction</code></td><td>➖</td><td></td><td>Warns when declaring async functions and suggests using Effect values and Effect.gen for async control flow</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>cryptoRandomUUID</code></td><td>➖</td><td></td><td>Warns when using crypto.randomUUID() outside Effect generators instead of the Effect Random module, which uses Effect-injected randomness rather than the crypto module behind the scenes</td><td></td><td>✓</td></tr>
    <tr><td><code>cryptoRandomUUIDInEffect</code></td><td>➖</td><td></td><td>Warns when using crypto.randomUUID() inside Effect generators instead of the Effect Random module, which uses Effect-injected randomness rather than the crypto module behind the scenes</td><td></td><td>✓</td></tr>
    <tr><td><code>extendsNativeError</code></td><td>➖</td><td></td><td>Warns when a class directly extends the native Error class</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalConsole</code></td><td>➖</td><td></td><td>Warns when using console methods outside Effect generators instead of Effect.log/Logger</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalConsoleInEffect</code></td><td>➖</td><td></td><td>Warns when using console methods inside Effect generators instead of Effect.log/Logger</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalDate</code></td><td>➖</td><td></td><td>Warns when using Date.now() or new Date() outside Effect generators instead of Clock/DateTime</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalDateInEffect</code></td><td>➖</td><td></td><td>Warns when using Date.now() or new Date() inside Effect generators instead of Clock/DateTime</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalFetch</code></td><td>➖</td><td></td><td>Warns when using the global fetch function outside Effect generators instead of the Effect HTTP client</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalFetchInEffect</code></td><td>➖</td><td></td><td>Warns when using the global fetch function inside Effect generators instead of the Effect HTTP client</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalRandom</code></td><td>➖</td><td></td><td>Warns when using Math.random() outside Effect generators instead of the Random service</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalRandomInEffect</code></td><td>➖</td><td></td><td>Warns when using Math.random() inside Effect generators instead of the Random service</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalTimers</code></td><td>➖</td><td></td><td>Warns when using setTimeout/setInterval outside Effect generators instead of Effect.sleep/Schedule</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>globalTimersInEffect</code></td><td>➖</td><td></td><td>Warns when using setTimeout/setInterval inside Effect generators instead of Effect.sleep/Schedule</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>instanceOfSchema</code></td><td>➖</td><td>🔧</td><td>Suggests using Schema.is instead of instanceof for Effect Schema types</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>newPromise</code></td><td>➖</td><td></td><td>Warns when constructing promises with new Promise instead of using Effect APIs</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>nodeBuiltinImport</code></td><td>➖</td><td></td><td>Warns when importing Node.js built-in modules that have Effect-native counterparts</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>preferSchemaOverJson</code></td><td>💡</td><td></td><td>Suggests using Effect Schema for JSON operations instead of JSON.parse/JSON.stringify</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>processEnv</code></td><td>➖</td><td></td><td>Warns when reading process.env outside Effect generators instead of using Effect Config</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>processEnvInEffect</code></td><td>➖</td><td></td><td>Warns when reading process.env inside Effect generators instead of using Effect Config</td><td>✓</td><td>✓</td></tr>
    <tr><td colspan="6"><strong>Style</strong> <em>Cleanup, consistency, and idiomatic Effect code.</em></td></tr>
    <tr><td><code>catchAllToMapError</code></td><td>💡</td><td>🔧</td><td>Suggests using Effect.mapError instead of Effect.catch + Effect.fail</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>deterministicKeys</code></td><td>➖</td><td>🔧</td><td>Enforces deterministic naming for service/tag/error identifiers based on class names</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectDoNotation</code></td><td>➖</td><td></td><td>Suggests using Effect.gen or Effect.fn instead of the Effect.Do notation helpers</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectFnOpportunity</code></td><td>💡</td><td>🔧</td><td>Suggests using Effect.fn for functions that return an Effect</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectMapFlatten</code></td><td>💡</td><td></td><td>Suggests using Effect.flatMap instead of Effect.map followed by Effect.flatten in piping flows</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectMapVoid</code></td><td>💡</td><td>🔧</td><td>Suggests using Effect.asVoid instead of Effect.map(() =&gt; void 0), Effect.map(() =&gt; undefined), or Effect.map(() =&gt; {})</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>effectSucceedWithVoid</code></td><td>💡</td><td>🔧</td><td>Suggests using Effect.void instead of Effect.succeed(undefined) or Effect.succeed(void 0)</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>missedPipeableOpportunity</code></td><td>➖</td><td>🔧</td><td>Suggests using .pipe() for nested function calls</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>missingEffectServiceDependency</code></td><td>➖</td><td></td><td>Checks that Effect.Service dependencies satisfy all required layer inputs</td><td>✓</td><td></td></tr>
    <tr><td><code>nestedEffectGenYield</code></td><td>➖</td><td></td><td>Warns when yielding a nested bare Effect.gen inside an existing Effect generator context</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>redundantSchemaTagIdentifier</code></td><td>💡</td><td>🔧</td><td>Suggests removing redundant identifier argument when it equals the tag value in Schema.TaggedClass/TaggedError/TaggedRequest</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>schemaStructWithTag</code></td><td>💡</td><td>🔧</td><td>Suggests using Schema.TaggedStruct instead of Schema.Struct with _tag field</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>schemaUnionOfLiterals</code></td><td>➖</td><td>🔧</td><td>Suggests combining multiple Schema.Literal calls in Schema.Union into a single Schema.Literal</td><td>✓</td><td></td></tr>
    <tr><td><code>serviceNotAsClass</code></td><td>➖</td><td>🔧</td><td>Warns when Context.Service is used as a variable instead of a class declaration</td><td></td><td>✓</td></tr>
    <tr><td><code>strictBooleanExpressions</code></td><td>➖</td><td></td><td>Enforces boolean types in conditional expressions for type safety</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>unnecessaryArrowBlock</code></td><td>➖</td><td>🔧</td><td>Suggests using a concise arrow body when the block only returns an expression</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>unnecessaryEffectGen</code></td><td>💡</td><td>🔧</td><td>Suggests removing Effect.gen when it contains only a single return statement</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>unnecessaryFailYieldableError</code></td><td>💡</td><td>🔧</td><td>Suggests yielding yieldable errors directly instead of wrapping with Effect.fail</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>unnecessaryPipe</code></td><td>💡</td><td>🔧</td><td>Removes pipe calls with no arguments</td><td>✓</td><td>✓</td></tr>
    <tr><td><code>unnecessaryPipeChain</code></td><td>💡</td><td>🔧</td><td>Simplifies chained pipe calls into a single pipe call</td><td>✓</td><td>✓</td></tr>
  </tbody>
</table>

`➖` off by default, `❌` error, `⚠️` warning, `💬` message, `💡` suggestion, `🔧` quick fix available
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

### Codegen Status

| Codegen | V3 | V4 | Notes |
|---------|----|----|-------|
| `accessors` | ❌ | ❌ | Generate Service accessor methods from comment directive |
| `annotate` | ❌ | ❌ | Generate type annotations from comment directive |
| `typeToSchema` | ❌ | ❌ | Generate Schema from type alias comment directive |

### Rename Status

| Rename | V3 | V4 | Notes |
|--------|----|----|-------|
| `keyStrings` | ❌ | ❌ | Extend rename to include key string literals in Effect classes |

## Best Practices

### Relationship to Official TypeScript-Go (`tsgo`)

Effect-tsgo is a **superset** of the official [TypeScript-Go](https://github.com/nicolo-ribaudo/TypeScript-Go) — it embeds a pinned version of `tsgo` with a small patch set on top and adds the Effect language service. This means `effect-tsgo` provides all standard TypeScript-Go functionality plus Effect-specific diagnostics, quick fixes, and refactors.

**Use `effect-tsgo` instead of `tsgo`, not alongside it.** Running both in parallel will produce duplicate diagnostics and degrade editor performance. Configure your editor to use `effect-tsgo` as your sole TypeScript language server.

### Version Pinning

Each release of `effect-tsgo` is built against a specific upstream `tsgo` commit. The pinned commit is recorded in `flake.nix` (`typescript-go-src`). When upstream `tsgo` releases new features or fixes, `effect-tsgo` will adopt them in a subsequent release after validating compatibility with the Effect diagnostics layer.

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
