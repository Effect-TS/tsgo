# tsconfig JSON Schema Generation

## Overview

Generate a JSON Schema file (`schema.json` at the repo root) that extends the standard TypeScript `tsconfig.json` schema with the Effect Language Service plugin configuration. This gives editors (VS Code, etc.) autocomplete and validation for the `@effect/language-service` plugin entry inside `compilerOptions.plugins`.

## Reference Implementation

The JavaScript implementation added this in [Effect-TS/language-service#681](https://github.com/Effect-TS/language-service/commit/1017a5443b2e6919f18e57afb86373ba825037c9). Since our rule definitions and plugin options now live in Go, the generation must happen in Go as well.

## Data Sources

1. **`rules.All`** (from `internal/rules/rules.go`) — provides each rule's `Name`, `Description`, and `DefaultSeverity` for building the `diagnosticSeverity` property schema with per-rule entries and defaults.

2. **`EffectPluginOptions` struct** (from `etscore/options.go`) — provides all other plugin property definitions (types, defaults, enums, descriptions). These are defined explicitly as JSON Schema fragments rather than reflected from struct tags, since descriptions and enum constraints are needed.

3. **Upstream tsconfig schema reference** (`https://json.schemastore.org/tsconfig`) — the upstream TypeScript tsconfig schema that the generated document layers on top of via `allOf`.

## Generated Output

The generator produces a `schema.json` at the repo root. The schema extends the upstream tsconfig schema by constraining `compilerOptions.plugins.items` to an `anyOf` containing:

- An Effect plugin schema object (with `name` constrained to `"@effect/language-service"` and all plugin-specific properties)
- The original plugin schema (with a `not` constraint excluding the Effect plugin name)

The `diagnosticSeverity` property is a dedicated definition (`effectLanguageServicePluginDiagnosticSeverityDefinition`) referenced via `$ref`, containing a property for each rule with its enum of severity values and the rule's default severity.

## Baseline Test Pattern

The test follows the same baseline pattern used elsewhere in the project:

1. The test generates the schema in memory.
2. It writes the result to `testdata/baselines/local/schema.json` using baseline logic — only writing if the content has actually changed.
3. It compares the local baseline against the committed `schema.json` at the repo root (the reference copy).
4. If they differ, the test fails.

To update after a rule or option change, the developer runs the test (which updates the local baseline), then copies it to the repo root.

No `go:generate` step is needed — the test is the source of truth.

## Location

- Generator function: `etscore/options_schema.go`
- Schema comparison test: `etscore/options_schema_test.go`
- Local baseline: `testdata/baselines/local/schema.json`
- Reference (committed): `schema.json` (repo root)
