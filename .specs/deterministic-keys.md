# deterministicKeys Rule

## Overview

The `deterministicKeys` rule ensures that string key literals passed to Effect service/tag/error class constructors follow a deterministic, location-based naming convention. When enabled, it compares the actual key string against an expected key computed from the class name, file path, and package name — and reports a diagnostic when they differ.

This rule is **off by default** and must be explicitly enabled (e.g., via `// @effect-diagnostics deterministicKeys:error`).

## Matched Patterns

The rule walks every class declaration and tries to match it against these constructor patterns:

| Pattern | V3 | V4 | Target | Example |
|---------|----|----|--------|---------|
| `Effect.Service<Self>()(key, options)` | ✅ | ❌ | `service` | `class Foo extends Effect.Service<Foo>()("key", { ... }) {}` |
| `ServiceMap.Service<Self, Shape>()(key)` | ❌ | ✅ | `service` | `class Foo extends ServiceMap.Service<Foo, Shape>()("key") {}` |
| `Context.Tag(key)<Self, Shape>()` | ✅ | ❌ | `service` | `class Foo extends Context.Tag("key")<Foo, Shape>() {}` |
| `Effect.Tag(key)<Self, Shape>()` | ✅ | ❌ | `service` | `class Foo extends Effect.Tag("key")<Foo, Shape>() {}` |
| `Data.TaggedError(key)<Fields>` | ✅ | ✅ | `error` | `class Foo extends Data.TaggedError("key")<{ msg: string }> {}` |
| `Schema.TaggedError<Self>(key)(tag, fields)` | ✅ | ❌ | `error` | `class Foo extends Schema.TaggedError<Foo>("key")("tag", {}) {}` |
| Custom (`@effect-identifier` annotation) | ✅ | ✅ | `custom` | Any constructor with `/** @effect-identifier */` parameter annotation |

For each matched class, the rule extracts the class name, the key string literal, and the target category (`"service"`, `"error"`, or `"custom"`).

The `custom` target is only active when the `extendedKeyDetection` option is enabled.

## Key Builder Module

The KeyBuilder module computes expected key strings from class metadata and source file location. It requires:

- **Package name**: from the nearest `package.json`'s `name` field (already available via `typeparser.PackageJsonForSourceFile`)
- **Package directory**: the directory containing that `package.json` (available via source file metadata's `PackageJsonDirectory`)
- **File path**: the source file's full path
- **Class name**: the declared class identifier

### Key Patterns

The key builder iterates over the configured `keyPatterns` list, matching on the `target` field (`"service"`, `"error"`, or `"custom"`). The first pattern whose target matches the class's target category is used.

Three pattern formulas are supported:

1. **`default`** (the standard pattern):
   `<packageName>/<subDirectory>/<fileName>/<className>`
   - Sub-directory is the file's directory relative to the package root, with configurable leading prefixes stripped (default: `["src/"]`)
   - If the class name matches the file name (case-insensitive), the class name segment is omitted
   - `index` file names are stripped
   - Example: `@effect/myapp/services/AuthService`

2. **`package-identifier`**:
   `<packageName>/<fileName>/<className>`
   - Same as `default` but omits the sub-directory segment entirely
   - Example: `@effect/myapp/AuthService`

3. **`default-hashed`**:
   - Computes the same key as `default`, then hashes it with cyrb53 (a fast non-cryptographic hash producing a 16-character hex string)
   - Example: `fc438e0396fbb4f7`

### cyrb53 Hash

The hash function used for `default-hashed` is cyrb53: a fast, non-cryptographic string hash that produces two 32-bit halves, output as a 16-character zero-padded hex string. The reference implementation is in `.repos/effect-language-service/packages/language-service/src/core/LSP.ts` (line 530).

## Plugin Options

The `deterministicKeys` rule depends on plugin options that must be parsed from the `compilerOptions.plugins` configuration in `tsconfig.json`. The following options are needed:

### `keyPatterns`

An array of key pattern objects, each with:
- `target`: `"service"` | `"error"` | `"custom"` (default: `"service"`)
- `pattern`: `"default"` | `"package-identifier"` | `"default-hashed"` (default: `"default"`)
- `skipLeadingPath`: array of path prefixes to strip from the sub-directory (default: `["src/"]`)

Default value:
```json
[
  { "target": "service", "pattern": "default", "skipLeadingPath": ["src/"] },
  { "target": "custom",  "pattern": "default", "skipLeadingPath": ["src/"] }
]
```

Note: by default there is no pattern for `"error"` targets — error keys are only validated when the user explicitly adds an `"error"` pattern.

### `extendedKeyDetection`

A boolean (default: `false`). When enabled, the rule also matches constructors whose parameter is annotated with `/** @effect-identifier */` JSDoc, using the `"custom"` target category.

### Current State of Plugin Options

`EffectPluginOptions` (in `etscore/options.go`) currently parses only `diagnosticSeverity`, `ignoreEffectSuggestionsInTscExitCode`, and `ignoreEffectWarningsInTscExitCode`. It needs to be extended with `keyPatterns` and `extendedKeyDetection` fields, along with their parsing logic in `etscore/parser.go`.

The full set of language service plugin options from the reference implementation is documented in `.repos/effect-language-service/packages/language-service/src/core/LanguageServicePluginOptions.ts`. Only the options relevant to ported rules need to be added.

## Diagnostic

When the actual key does not match the expected key, the rule reports:

> Key should be '`<expectedKey>`'

with rule name `effect(deterministicKeys)`.

A new diagnostic message must be added to `internal/diagnostics/effectDiagnosticMessages.json`, followed by `pnpm setup-repo` to regenerate the diagnostic code.

## Code Fix

A quick fix (`deterministicKeys_fix`) replaces the incorrect key string literal with the expected key.

## Reference Implementation

The TypeScript reference implementation is in:
- Rule: `.repos/effect-language-service/packages/language-service/src/diagnostics/deterministicKeys.ts`
- Key builder: `.repos/effect-language-service/packages/language-service/src/core/KeyBuilder.ts`
- Plugin options: `.repos/effect-language-service/packages/language-service/src/core/LanguageServicePluginOptions.ts`
- cyrb53 hash: `.repos/effect-language-service/packages/language-service/src/core/LSP.ts` (line 530)

## Type Parser Dependencies

The rule requires the following type parser helpers to return the **key string literal** node (in addition to class name and self type):

- `ExtendsContextTag` — exists, needs to expose key
- `ExtendsEffectTag` — exists, needs to expose key
- `ExtendsServiceMapService` — exists, needs to expose key
- `ExtendsEffectService` — exists, needs to expose key
- `ExtendsSchemaTaggedError` — exists, already exposes `KeyStringLiteral`
- `ExtendsDataTaggedError` — does not exist, needs to be created
- `PackageJsonForSourceFile` — exists, provides package name and directory

## Tests

Test fixtures should be ported from the reference language service for both V3 and V4:

- `.repos/effect-language-service/packages/harness-effect-v4/examples/diagnostics/deterministicKeys.ts`
- `.repos/effect-language-service/packages/harness-effect-v4/examples/diagnostics/deterministicKeys_defaultHashed.ts`
- `.repos/effect-language-service/packages/harness-effect-v4/examples/diagnostics/deterministicKeys_packageIdentifier.ts`
- `.repos/effect-language-service/packages/harness-effect-v4/examples/diagnostics/deterministicKeys_custom.ts`
- `.repos/effect-language-service/packages/harness-effect-v3/examples/diagnostics/deterministicKeys.ts`
- `.repos/effect-language-service/packages/harness-effect-v3/examples/diagnostics/deterministicKeys_defaultHashed.ts`
- `.repos/effect-language-service/packages/harness-effect-v3/examples/diagnostics/deterministicKeys_packageIdentifier.ts`
- `.repos/effect-language-service/packages/harness-effect-v3/examples/diagnostics/deterministicKeys_custom.ts`
