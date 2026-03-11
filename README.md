# Effect Language Service (TypeScript-Go)

A wrapper around [TypeScript-Go](https://github.com/nicolo-ribaudo/TypeScript-Go) that builds the Effect Language Service, providing Effect-TS diagnostics and quick fixes. This project targets **Effect V4** (codename: "smol") primarily and also Effect V3.

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
| `floatingEffect` | ❌ | ✅ | ✅ | | |
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

## Plugin Options

These options are configured in `tsconfig.json` under `compilerOptions.plugins` for the `@effect/tsgo` plugin entry.

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
