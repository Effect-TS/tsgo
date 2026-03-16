# Effect Language Service (TypeScript-Go)

A wrapper around [TypeScript-Go](https://github.com/nicolo-ribaudo/TypeScript-Go) that builds the Effect Language Service, providing Effect-TS diagnostics and quick fixes. 
This project targets **Effect V4** (codename: "smol") primarily and also Effect V3.

## Currently in Alpha
The TypeScript-Go version of the Effect LSP should be considered in Alpha. Expect breaking changes between releases and some missing features compared to previous version.
Some of them are currently on hold due to not yet complete pipeline on the upstream TypeScript repository.

## Diagnostic Status

| Rule | Sev | V3 | V4 | 🔧 | Notes |
|------|-----|----|----|-----|-------|
| `anyUnknownInErrorContext` | — | ✅ | ✅ | | |
| `catchAllToMapError` | 💡 | ✅ | ✅ | `catchAllToMapError_fix` | |
| `catchUnfailableEffect` | 💡 | ✅ | ✅ | | |
| `classSelfMismatch` | ❌ | ✅ | ✅ | `classSelfMismatch_fix` | |
| `deterministicKeys` | — | ✅ | ✅ | `deterministicKeys_fix` | Off by default |
| `duplicatePackage` | ⚠️ | ✅ | ✅ | | |
| `effectFnIife` | ⚠️ | ✅ | ✅ | `effectFnIife_toEffectGen` | |
| `effectFnOpportunity` | 💡 | ✅ | ✅ | `effectFnOpportunity_toEffectFnWithSpan`, `effectFnOpportunity_toEffectFnUntraced`, `effectFnOpportunity_toEffectFnNoSpan`, `effectFnOpportunity_toEffectFnSpanInferred` | |
| `effectGenUsesAdapter` | ⚠️ | ✅ | ➖ | | V3-only — not applicable to V4 |
| `effectInFailure` | ⚠️ | ✅ | ✅ | | |
| `effectInVoidSuccess` | ⚠️ | ✅ | ✅ | | |
| `effectMapVoid` | 💡 | ✅ | ✅ | `effectMapVoid_fix` | |
| `effectSucceedWithVoid` | 💡 | ✅ | ✅ | `effectSucceedWithVoid_fix` | |
| `extendsNativeError` | — | ✅ | ✅ | | Off by default |
| `floatingEffect` | ❌ | ✅ | ✅ | | Excludes Effect subtypes (Exit, Pool, etc.) and Fiber types; uses "Effect-able {Type}" message for non-strict Effect types |
| `genericEffectServices` | ⚠️ | ✅ | ➖ | | V3-only — not applicable to V4 |
| `globalErrorInEffectCatch` | ⚠️ | ✅ | ✅ | | |
| `globalErrorInEffectFailure` | ⚠️ | ✅ | ✅ | | |
| `importFromBarrel` | | ❌ | ❌ | `replaceWithUnbarrelledImport` (not ported yet) | Needs: resolveExternalModuleName, getModuleSpecifier |
| `instanceOfSchema` | — | ✅ | ✅ | `instanceOfSchema_fix` | |
| `layerMergeAllWithDependencies` | ⚠️ | ✅ | ✅ | `layerMergeAllWithDependencies_fix` | |
| `leakingRequirements` | 💡 | ✅ | ✅ | | |
| `middlewareAutoImportQuickfixes` | | ❌ | ❌ | | Not a diagnostic — auto-import middleware |
| `missedPipeableOpportunity` | — | ✅ | ✅ | `missedPipeableOpportunity_fix` | Off by default |
| `missingEffectContext` | ❌ | ✅ | ✅ | | |
| `missingEffectError` | ❌ | ✅ | ✅ | `missingEffectError_catchAll`/`missingEffectError_catch`, `missingEffectError_tagged` | |
| `missingEffectServiceDependency` | — | ✅ | ➖ | | V3-only |
| `missingLayerContext` | ❌ | ✅ | ✅ | | |
| `missingReturnYieldStar` | ❌ | ✅ | ✅ | `missingReturnYieldStar_fix` | Also detects yieldable wrappers (Option, Either) via asEffect() |
| `missingStarInYieldEffectGen` | ❌ | ✅ | ✅ | `missingStarInYieldEffectGen_fix` | |
| `multipleEffectProvide` | ⚠️ | ✅ | ✅ | `multipleEffectProvide_fix` | |
| `nodeBuiltinImport` | — | ✅ | ✅ | | Off by default |
| `nonObjectEffectServiceType` | ❌ | ✅ | ➖ | | V3-only |
| `outdatedApi` | ⚠️ | ➖ | ✅ | | V4-only — detects Effect v3 APIs in v4 projects |
| `outdatedEffectCodegen` | | ❌ | ❌ | `outdatedEffectCodegen_fix` (not ported yet), `outdatedEffectCodegen_ignore` (not ported yet) | Needs: codegen system |
| `overriddenSchemaConstructor` | ❌ | ✅ | ✅ | `overriddenSchemaConstructor_static`, `overriddenSchemaConstructor_fix` | |
| `preferSchemaOverJson` | 💡 | ✅ | ✅ | | |
| `redundantSchemaTagIdentifier` | 💡 | ✅ | ➖ | `redundantSchemaTagIdentifier_removeIdentifier` | V3-only — not applicable to V4 |
| `returnEffectInGen` | 💡 | ✅ | ✅ | `returnEffectInGen_fix` | |
| `runEffectInsideEffect` | 💡 | ✅ | ➖ | `runEffectInsideEffect_fix` | V3-only — not applicable to V4 |
| `schemaStructWithTag` | 💡 | ✅ | ✅ | `schemaStructWithTag_fix` | |
| `schemaSyncInEffect` | 💡 | ✅ | ✅ | | |
| `schemaUnionOfLiterals` | — | ✅ | ✅ | `schemaUnionOfLiterals_fix` | V3-only — not applicable to V4 |
| `scopeInLayerEffect` | ⚠️ | ✅ | ➖ | `scopeInLayerEffect_scoped` | V3-only — not applicable to V4 |
| `serviceNotAsClass` | — | ➖ | ✅ | `serviceNotAsClass_fix` | V4-only — off by default |
| `strictBooleanExpressions` | — | ✅ | ✅ | | |
| `strictEffectProvide` | — | ✅ | ✅ | | |
| `tryCatchInEffectGen` | 💡 | ✅ | ✅ | | |
| `unknownInEffectCatch` | ⚠️ | ✅ | ✅ | | |
| `unnecessaryEffectGen` | 💡 | ✅ | ✅ | `unnecessaryEffectGen_fix` | |
| `unnecessaryFailYieldableError` | 💡 | ✅ | ✅ | `unnecessaryFailYieldableError_fix` | |
| `unnecessaryPipe` | 💡 | ✅ | ✅ | `unnecessaryPipe_fix` | |
| `unnecessaryPipeChain` | 💡 | ✅ | ✅ | `unnecessaryPipeChain_fix` | |
| `unsupportedServiceAccessors` | | ❌ | ❌ | `unsupportedServiceAccessors_enableCodegen` (not ported yet) | Needs: refactor analysis |

**Severity icons:** ❌ error · ⚠️ warning · 💡 suggestion · ℹ️ message · — off

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

These options are configured in `tsconfig.json` under `compilerOptions.plugins` for the `@effect/language-service` plugin entry.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `diagnosticSeverity` | `Record<string, Severity>` | (all defaults) | Maps rule names to severity levels. Set to `{}` to enable diagnostics with defaults. |
| `ignoreEffectSuggestionsInTscExitCode` | `boolean` | `true` | When true, Effect suggestion/message-category diagnostics do not affect the tsc exit code. |
| `ignoreEffectWarningsInTscExitCode` | `boolean` | `false` | When true, Effect warning-category diagnostics do not affect the tsc exit code. |
| `ignoreEffectErrorsInTscExitCode` | `boolean` | `false` | When true, Effect error-category diagnostics do not affect the tsc exit code. |
| `skipDisabledOptimization` | `boolean` | `false` | When true, disabled diagnostics are still processed so per-line or per-section directive overrides can enable them. |
| `keyPatterns` | `KeyPattern[]` | (see defaults) | Configures key pattern formulas for the `deterministicKeys` rule. |
| `extendedKeyDetection` | `boolean` | `false` | Enables matching constructors with `@effect-identifier` annotations. |
| `pipeableMinArgCount` | `number` | `2` | Minimum number of contiguous pipeable transformations to trigger `missedPipeableOpportunity`. |
| `mermaidProvider` | `string` | `"mermaid.live"` | Mermaid rendering service for Layer hover links. Accepted values: `"mermaid.live"`, `"mermaid.com"`, or a custom URL. |
| `noExternal` | `boolean` | `false` | When true, suppresses external links (Mermaid diagram URLs) in hover output. |
| `inlays` | `boolean` | `false` | When true, suppresses redundant return-type inlay hints on `Effect.gen`, `Effect.fn`, and `Effect.fnUntraced` generator functions. |
| `allowedDuplicatedPackages` | `string[]` | `[]` | Package names allowed to have multiple versions without triggering the `duplicatePackage` diagnostic. |
| `layerGraphFollowDepth` | `number` | `0` | How many levels deep the layer graph extraction follows symbol references. |
| `namespaceImportPackages` | `string[]` | `[]` | Package names that should prefer namespace imports. Package matching is case-insensitive. |
| `barrelImportPackages` | `string[]` | `[]` | Package names that should prefer barrel named imports. Package matching is case-insensitive. |
| `importAliases` | `Record<string, string>` | `{}` | Package-level import aliases keyed by package name. Alias keys are case-insensitive package matches. |
| `topLevelNamedReexports` | `"ignore" \| "follow"` | `"ignore"` | Controls whether named reexports are followed at package top-level. Accepted values are case-insensitive; invalid values fall back to `"ignore"`. |

For the four auto-import style options above (`namespaceImportPackages`, `barrelImportPackages`, `importAliases`, `topLevelNamedReexports`), package-name matching is case-insensitive, and invalid option types/values fall back to defaults.
