# Auto Import Style Overrides

## Goal

Allow users to configure auto-import style per package so Effect projects can choose between namespace-style imports and named imports from barrel packages.

## Background

The previous `effect-language-service` exposed package-scoped auto-import options and used them to rewrite import behavior for both completion-driven auto-imports and import-related quick fixes.

In this TypeScript-Go fork, auto-imports for completions and import quick fixes already share a single core pipeline. This makes it possible to apply one consistent style policy in one place.

## Scope

- Porting the auto-import style option surface from the previous language service.
- Parsing those options from `compilerOptions.plugins` into Effect plugin options.
- Implementing style-rewrite behavior so completions and import quick fixes apply the configured import style consistently.
- Defining where style logic should be injected so completions and import quick fixes stay consistent.

## Requirements

### Option Surface

1. The Effect plugin option model must support package-scoped auto-import style controls:
   - `namespaceImportPackages: string[]`
   - `barrelImportPackages: string[]`
   - `importAliases: Record<string, string>`
   - `topLevelNamedReexports: "ignore" | "follow"`
2. Default values:
   - `namespaceImportPackages = []`
   - `barrelImportPackages = []`
   - `importAliases = {}`
   - `topLevelNamedReexports = "ignore"`
3. Package-name matching must be case-insensitive.

### Option Parsing

4. The options above must be parsed from the Effect plugin entry in `compilerOptions.plugins` and stored in Effect plugin options on compiler options.
5. Invalid types must fall back to defaults rather than failing config parsing.
6. Parsing coverage must include unit tests for defaults, valid inputs, and invalid-input fallback behavior.

### Shared Injection Strategy

7. Auto-import style policy must be applied in a single shared auto-import decision path used by:
   - completion auto-import items
   - import-related code fixes
8. The implementation must avoid duplicating style logic separately in completion-specific and code-fix-specific wrappers.
9. Any required usage-site text adjustment (for example, introducing a namespace qualifier when needed) must be produced through the same shared fix/edit pipeline so completion-resolve and code-fix application stay behaviorally aligned.

### Workspace Consistency

16. `typescript-go/` must be treated as resettable working state during checks; persistent changes must be represented through `_patches/`.
17. If expected files under `typescript-go/` disappear or drift mid-implementation, the implementation flow must restore from git/submodule state and continue via patches, rather than relying on direct workspace edits as source of truth.

### Style-Rewrite Behavior

10. The configured package style options must affect generated auto-import edits, not only option parsing.
11. For packages matched by `namespaceImportPackages`, generated auto-imports should prefer namespace-style imports and apply any required usage-site qualifier updates.
12. For packages matched by `barrelImportPackages`, generated auto-imports should prefer named imports from the configured barrel resolution path.
13. `importAliases` must be applied when generating import clauses and any introduced usage-site qualifier text.
14. `topLevelNamedReexports` must control whether top-level named reexports are ignored or followed when choosing the import target.
15. Completion-triggered auto-import edits and import-related code-fix edits must produce consistent results for the same symbol and config.

## Non-Goals

- Defining new style modes beyond the option surface listed above.

## Acceptance Criteria

1. The Effect plugin config can express package-scoped auto-import style overrides using the options in this spec.
2. Parsed options are available in the Effect plugin options object with the documented defaults.
3. Auto-import behavior (completion and code-fix flows) reflects configured namespace/barrel/alias/reexport style settings.
4. Completion-triggered auto-import edits and import-related code-fix edits remain consistent under the same configuration.
5. README plugin-options documentation includes the new options when they are shipped.
6. The architecture for style injection is explicitly anchored to one shared auto-import pipeline so completions and import code fixes remain consistent.
