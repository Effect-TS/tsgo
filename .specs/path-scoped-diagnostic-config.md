# Path-Scoped Diagnostic Config

## Goal

Add tsconfig-style path-scoped diagnostic overrides to the `@effect/language-service` plugin so users can vary diagnostic severities and selected diagnostic options by file path, with behavior that stays correct across both `extends` and project `references`.

## Background

Today the Effect plugin stores one flattened `EffectPluginOptions` object on compiler options and applies it uniformly to all files in a project. That is sufficient for project-wide settings such as `diagnosticSeverity`, but it does not let users express common workflows such as:

- relaxing specific rules in test files
- disabling selected diagnostics in generated sources
- using different rule thresholds in examples, scripts, or migrations

The desired behavior is intentionally close to TypeScript's own `include` and `exclude` semantics:

- patterns are evaluated relative to the config file that declares them
- inherited config fragments keep their own relative roots under `extends`
- referenced projects evaluate their own config independently

The repository already contains reusable TypeScript-Go config parsing and path-matching machinery that should be reused rather than reimplemented.

## Proposed User-Facing Shape

The Effect plugin config should continue to support global defaults and add an ordered list of path-scoped overrides.

If multiple `overrides` entries match the same file, they are applied cumulatively in declaration order. Each matching entry acts as a partial override of the effective config built so far, and for any individual diagnostic or option key, the last matching entry that sets that key wins.

```json
{
  "name": "@effect/language-service",
  "diagnosticSeverity": {
    "deterministicKeys": "error",
    "noUncheckedDependencies": "error"
  },
  "pipeableMinArgCount": 2,
  "overrides": [
    {
      "include": ["src/**/*.ts"],
      "exclude": ["src/**/*.test.ts"],
      "diagnostics": {
        "deterministicKeys": "warning"
      },
      "options": {
        "pipeableMinArgCount": 3,
        "extendedKeyDetection": true
      }
    },
    {
      "include": ["test/**/*.ts"],
      "diagnostics": {
        "noUncheckedDependencies": "off"
      }
    }
  ]
}
```

## Scope

- Defining the config surface for path-scoped diagnostic overrides.
- Reusing TypeScript-Go path matching semantics for `include` and `exclude`.
- Preserving correct config-relative behavior across `extends`.
- Preserving per-project behavior across project `references`.
- Defining where the effective per-file diagnostic config is computed and consumed.

## Non-Goals

- Replacing the existing top-level `diagnosticSeverity` map.
- Introducing path-scoped controls for non-diagnostic language-service features such as completions, goto, hover, rename, or refactors.
- Reimplementing tsconfig glob semantics with a new glob engine.
- Making `etscore` depend directly on `typescript-go/internal/...` packages.

## Reusable TypeScript-Go Pieces

### Config Parsing Context

The tsconfig parser already knows both the declaring config path and its base directory while parsing config JSON.

- `typescript-go/internal/tsoptions/tsconfigparsing.go:843` `ParseJsonConfigFileContent(..., basePath, configFileName, ...)`
- `typescript-go/internal/tsoptions/tsconfigparsing.go:698` `ParseJsonSourceFileConfigFileContent(..., basePath, configFileName, ...)`
- `typescript-go/internal/tsoptions/tsconfigparsing.go:1763` `GetParsedCommandLineOfConfigFilePath(...)`
- `typescript-go/internal/tsoptions/tsconfigparsing.go:1784` passes `tspath.GetDirectoryPath(configFileName)` as `basePath`

Compiler options also retain config provenance after parsing.

- `typescript-go/internal/tsoptions/tsconfigparsing.go:1172`
- `typescript-go/internal/core/compileroptions.go:134`

This means the parsing pipeline already has the information needed to resolve path-scoped Effect config correctly.

### Extends Semantics

TypeScript-Go already preserves declaring-config-relative semantics when inheriting `include`, `exclude`, and `files` from extended configs.

- `typescript-go/internal/tsoptions/tsconfigparsing.go:1043` `applyExtendedConfig`
- `typescript-go/internal/tsoptions/tsconfigparsing.go:1053` handles `include` / `exclude` / `files`
- `typescript-go/internal/tsoptions/tsconfigparsing.go:1065` computes the relative difference between the extended config directory and the current config directory

This is the most important behavior to mirror for Effect diagnostic scopes.

### Matcher Machinery

TypeScript-Go already contains the lower-level spec matcher used for config-style path matching.

- `typescript-go/internal/vfs/vfsmatch/vfsmatch.go:682` `NewSpecMatcher`
- `typescript-go/internal/vfs/vfsmatch/vfsmatch.go:662` `SpecMatcher.MatchString`
- `typescript-go/internal/vfs/vfsmatch/vfsmatch.go:672` `SpecMatcher.MatchIndex`

Config parsing also uses this matcher in tsconfig-specific helpers.

- `typescript-go/internal/tsoptions/tsconfigparsing.go:103` `matchesExclude`
- `typescript-go/internal/tsoptions/tsconfigparsing.go:122` `getMatchedIncludeSpec`
- `typescript-go/internal/tsoptions/parsedcommandline.go:367` `GetMatchedIncludeSpec`
- `typescript-go/internal/tsoptions/parsedcommandline.go:321` `PossiblyMatchesFileName`

An existing language-service usage point also shows that `SpecMatcher` is already considered the right abstraction for user-configured file patterns.

- `typescript-go/internal/ls/lsutil/userpreferences.go:757` `ParsedAutoImportFileExcludePatterns`

### Project References

Each referenced project is already parsed as its own `ParsedCommandLine`, and the compiler already tracks referenced project configs separately.

- `typescript-go/internal/compiler/projectreferenceparser.go:18` parses referenced project configs
- `typescript-go/internal/compiler/projectreferenceparser.go:89` stores referenced `ParsedCommandLine` instances
- `typescript-go/internal/compiler/program.go:348` exposes the current program's `CommandLine()`

This means path-scoped diagnostic config should be evaluated per project/config, not globally across all referenced projects.

## Architectural Constraints

The repo rule is that Effect code must not import `typescript-go/internal/...` directly and must use shims instead.

- `AGENTS.md`

`etscore` is intentionally a leaf package and currently only depends on shim packages.

- `etscore/parser.go:3`

However, the current plugin parsing boundary is too narrow for path-relative config:

- `typescript-go/internal/tsoptions/parsinghelpers.go:455` handles `plugins`
- `typescript-go/internal/tsoptions/parsinghelpers.go:457` calls `etscore.ParseFromPlugins(value)`
- `etscore/parser.go:12` only receives the raw `plugins` value

Because `etscore.ParseFromPlugins` does not receive `basePath`, `configFileName`, or inheritance context, path-scoped config cannot be implemented correctly there as-is.

## Requirements

### Option Surface

1. The Effect plugin option model must add `overrides` as an ordered array.
2. Each `overrides` entry must support:
   - `include?: string[]`
   - `exclude?: string[]`
   - `diagnostics?: Record<string, Severity>`
   - `options?: OverrideOptions`
3. `diagnostics` must override top-level `diagnosticSeverity` only for files matched by that scope entry.
4. `options` must override only a curated subset of diagnostic-relevant options, not the full plugin config surface.
5. The top-level `diagnosticSeverity` map must remain the baseline default for the whole project.

### Matching Semantics

6. `include` and `exclude` inside a scope entry must behave as closely as possible to TypeScript's own tsconfig `include` and `exclude` semantics.
7. Patterns must be interpreted relative to the config file that declared the scope entry.
8. If `include` is omitted or empty, the scope entry must be considered applicable to all files in that project before `exclude` is applied.
9. If `exclude` is omitted or empty, the scope entry must exclude nothing.
10. A file matches a scope entry only if it is included by that entry and not excluded by that entry.
11. `exclude` must affect only the current scope entry and must not globally suppress the file from other matching entries.
12. Matching must use TypeScript-Go's existing config-style spec matcher behavior rather than a new custom glob implementation.

### Extends Semantics

13. Path-scoped diagnostic entries declared in extended configs must preserve declaring-config-relative behavior in the same way tsconfig `include` and `exclude` do.
14. A scope entry inherited through `extends` must keep path semantics relative to the config where it was originally declared.
15. The implementation must either:
   - rewrite inherited scope patterns using the same relative-difference strategy used by `applyExtendedConfig`, or
   - preserve each scope entry's declaring base path and compile matchers against that base path.
16. The implementation must not reinterpret all inherited scope patterns relative to the leaf config file.

### Reference Semantics

17. Each referenced project must evaluate diagnostics using its own resolved Effect plugin config.
18. A file in a referenced project must use the diagnostic scopes resolved from that referenced project's config, not the parent project's config.
19. Path-scoped diagnostic evaluation must therefore operate at per-`ParsedCommandLine` or per-program granularity.

### Effective Per-File Config

20. The implementation must compute an effective diagnostic config for a concrete file by starting from the top-level Effect plugin options and applying all matching `overrides` entries in declaration order.
21. Later matching scope entries must win over earlier entries on a per-field basis.
22. Scope entries must be partial overrides: omitted fields leave the current effective config unchanged.
23. Rules must consume the effective per-file diagnostic config rather than individually reimplementing scope matching.

### Scope-Overrideable Options

24. Scope `options` must be restricted to diagnostic-relevant settings only.
25. The initial scope-overrideable subset should include only options that change diagnostic evaluation for a file, such as:
   - `pipeableMinArgCount`
   - `keyPatterns`
   - `extendedKeyDetection`
   - `allowedDuplicatedPackages`
26. Non-diagnostic language-service feature flags such as `refactors`, `quickinfo`, `completions`, `goto`, and `renames` must remain global.
27. Exit-code behavior controls such as `ignoreEffectSuggestionsInTscExitCode`, `ignoreEffectWarningsInTscExitCode`, `ignoreEffectErrorsInTscExitCode`, and `includeSuggestionsInTsc` must remain global.

## Recommended Architecture

### Data Model Split

1. `etscore` should define the raw schema types for `overrides` and `OverrideOptions`.
2. `etscore` should not import `internal/vfs/vfsmatch` or other `typescript-go/internal/...` packages directly.
3. The TypeScript-Go integration layer should be responsible for attaching config-relative context or precompiled matcher state.

### Parsing Boundary Change

4. The current `etscore.ParseFromPlugins(value any)` API should be replaced or extended so path-scoped parsing has access to config context.
5. The parser boundary must receive enough information to preserve config-relative behavior across inheritance, at minimum the declaring config `basePath` and ideally the `configFileName`.
6. If the repository chooses not to thread context into `etscore`, then TypeScript-Go must perform a post-parse normalization pass before storing Effect config on compiler options.

### Matching Implementation

7. The implementation should reuse `vfsmatch.NewSpecMatcher` semantics rather than reimplement glob rules.
8. The preferred implementation is to compile scope include/exclude matchers from config-relative specs and cache them in a TS-integrated layer.
9. If direct matcher reuse is needed from Effect-facing code, the repo should add a narrow shim rather than importing `internal/vfs/vfsmatch` directly from Effect packages.

### Runtime Consumption

10. Checker hooks should stop treating `p.Options().Effect` as the final per-file config.
11. Checker hooks should instead resolve an effective config for the current source file, based on the current program/config and that file's path.
12. That effective config should then be passed through the existing rule collection and directive pipeline.

## Implementation Notes

### Why Not Reuse ParsedCommandLine File Inclusion Directly

The `ParsedCommandLine` inclusion helpers are useful references, but they answer a different question: whether a file belongs to the project. Diagnostic scopes need the lower-level matcher semantics, not the full project-membership pipeline.

- `typescript-go/internal/tsoptions/parsedcommandline.go:321`
- `typescript-go/internal/tsoptions/parsedcommandline.go:367`

### Why Extends Must Be Handled Explicitly

TypeScript already rewrites inherited config specs when needed because config-relative semantics cannot be recovered from a flattened string list alone.

- `typescript-go/internal/tsoptions/tsconfigparsing.go:1053`

Effect diagnostic scopes have the same problem. A design that stores only raw inherited patterns on the final merged options object, without either base-path provenance or rewrite normalization, will produce incorrect matches.

### Why References Should Be Natural

Project references already create separate parsed configs and program state. Path-scoped diagnostics should follow that existing model instead of inventing cross-project merging.

- `typescript-go/internal/compiler/projectreferenceparser.go:18`
- `typescript-go/internal/compiler/program.go:348`

## Acceptance Criteria

1. The Effect plugin config can express ordered path-scoped diagnostic overrides with `include`, `exclude`, `diagnostics`, and `options`.
2. `include` and `exclude` behave with TypeScript-like config-relative semantics for matching files.
3. A scope entry inherited through `extends` still matches relative to the config that declared it.
4. A file in a referenced project uses the referenced project's own resolved diagnostic scopes.
5. Later matching scope entries override earlier matching entries on a per-field basis.
6. Rules consume one effective per-file diagnostic config rather than each rule reimplementing path matching.
7. The implementation reuses existing TypeScript-Go path-matching machinery or a thin shim over it instead of introducing a separate glob engine.
8. The final design preserves the `etscore` leaf-package boundary and does not introduce direct imports of `typescript-go/internal/...` from Effect packages.
