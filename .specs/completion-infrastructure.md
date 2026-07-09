# Completion Infrastructure

## Goal
Add custom Effect completions support to the Go language service, following the same abstraction pattern used by rules, fixables, and refactors.

## Background

### Current state
No custom completions exist in the Go version. The completion pipeline goes straight from TypeScript-Go's `ProvideCompletion()` (in `typescript-go/internal/ls/completions.go`) to the LSP client with no Effect customization point. No patches in `_patches/` touch completions.

### Reference implementation
The reference (`effect-language-service`) has 14 custom completions that hook into TypeScript's `getCompletionsAtPosition` via a proxy, run each completion definition's `apply()` function, and merge results with native completions.

Key reference files:
- Registry: `.repos/effect-language-service/packages/language-service/src/completions.ts`
- Individual completions: `.repos/effect-language-service/packages/language-service/src/completions/*.ts` (14 files)
- Core interface: `.repos/effect-language-service/packages/language-service/src/core/LSP.ts` (lines 109-139)
- Tests: `.repos/effect-language-service/packages/language-service/test/completions.test.ts`

### Auto-import middleware
The reference has a `middlewareAutoImports.ts` that post-processes completions for auto-import support. This is **not needed** in the Go version — auto-completion is handled differently.

## Existing Hook Patterns

The codebase has three established abstraction layers that completions should mirror:

| Abstraction | Struct | Context | Registry | Hook |
|---|---|---|---|---|
| **Rule** | `internal/rule/rule.go` | `rule.Context` (checker, sourceFile, severity) | `internal/rules/rules.go` → `All` slice | `RegisterAfterCheckSourceFileCallback` |
| **Fixable** | `internal/fixable/fixable.go` | `fixable.Context` (checker via `GetTypeCheckerForFile`, span, errorCode) | `internal/fixables/fixables.go` → `All` slice | `RegisterCodeFixProvider` |
| **Refactor** | `internal/refactor/refactor.go` | `refactor.Context` (checker via `GetTypeCheckerForFile`, span) | `internal/refactors/refactors.go` → `All` slice | `RegisterRefactorProvider` |

All three follow the same shape:
1. A struct with `Name`, `Description`, and a `Run` function
2. A context type providing checker access, source file, and request-specific data
3. A registry (`All` slice) with lookup helpers
4. A hook registered in `etslshooks/init.go` that dispatches to all registered instances

## Design

### Naming
The abstraction is called **"completion"** (singular for the type, plural `completions` for the registry package) — consistent with rule/rules, fixable/fixables, refactor/refactors.

### File structure

| Component | Path | Purpose |
|---|---|---|
| Struct definition | `internal/completion/completion.go` | `Completion` struct: `Name`, `Description`, `Run func(ctx *Context) []CompletionEntry` |
| Context | `internal/completion/context.go` | Checker (from `Program.GetTypeChecker(ctx)`), source file, position, existing completion list |
| Registry | `internal/completions/completions.go` | `All` slice, helper functions |
| Implementations | `internal/completions/*.go` | One file per completion |
| Registration | `etslshooks/init.go` | Hook registration alongside existing hooks |

### Checker access
The checker should be obtained via `Program.GetTypeChecker(ctx)`:
```go
func (p *Program) GetTypeChecker(ctx context.Context) (*checker.Checker, func()) {
    return p.checkerPool.GetChecker(ctx)
}
```
This matches how fixables and refactors access the checker through `GetTypeCheckerForFile`.

### Hook pattern
The "after callback" pattern (like hover and inlay hints) is the best fit:
- An `AfterCompletionCallback` receives position, source file, and the existing completion list
- The Effect layer iterates `completions.All`, runs each, and merges results into the existing list
- This allows appending custom entries to the native completion list

This differs from the "provider" pattern (used by fixables/refactors) because completions need access to the existing completion list to avoid duplicates.

### Injection points in TypeScript-Go
1. **`typescript-go/internal/ls/completions.go`** — Add `AfterCompletionCallback` variable, call it at the end of `ProvideCompletion()` before returning
2. **`shim/ls/shim.go`** — Expose `RegisterAfterCompletionCallback` via `//go:linkname`
3. **A new patch in `_patches/`** — To add the callback to the TypeScript-Go submodule (following the pattern of `012-ls-hover.patch` and `015-ls-inlay-hints.patch`)

## Completions to Port

### V4-compatible (higher priority)

| Completion | Trigger | Needs Checker | Reference file |
|---|---|---|---|
| `effectSchemaSelfInClasses` | Extending Schema.Class, TaggedErrorClass, etc. | No (AST) | `completions/effectSchemaSelfInClasses.ts` |
| `effectDataClasses` | Extending Data.TaggedError, TaggedClass | No (AST) | `completions/effectDataClasses.ts` |
| `serviceMapSelfInClasses` | Extending ServiceMap.Service (V4 only) | No (AST) | `completions/serviceMapSelfInClasses.ts` |
| `genFunctionStar` | Dot-accessing `.gen` | Yes | `completions/genFunctionStar.ts` |
| `fnFunctionStar` | Dot-accessing `.fn` | No (AST) | `completions/fnFunctionStar.ts` |
| `effectDiagnosticsComment` | `@effect-diagnostics` in comments | No (regex) | `completions/effectDiagnosticsComment.ts` |
| `effectCodegensComment` | `@effect-codegens` in comments | No (regex) | `completions/effectCodegensComment.ts` |
| `effectJsdocComment` | `@effect-identifier` in comments | No (regex) | `completions/effectJsdocComment.ts` |
| `durationInput` | String literal with Duration contextual type | Yes | `completions/durationInput.ts` |

### V3-only (lower priority)

| Completion | Trigger | Needs Checker | Reference file |
|---|---|---|---|
| `contextSelfInClasses` | Extending Context.Tag | No (AST) | `completions/contextSelfInClasses.ts` |
| `effectSelfInClasses` | Extending Effect.Service or Effect.Tag | No (AST) | `completions/effectSelfInClasses.ts` |
| `effectSqlModelSelfInClasses` | Extending `@effect/sql` Model.Class | No (AST) | `completions/effectSqlModelSelfInClasses.ts` |
| `rpcMakeClasses` | Extending `@effect/rpc` Rpc | No (AST) | `completions/rpcMakeClasses.ts` |
| `schemaBrand` | Dot-accessing Schema for `.brand()` | No (AST) | `completions/schemaBrand.ts` |

## Non-Goals
- Auto-import post-processing middleware (handled differently in the Go version).
- Changing TypeScript-Go's native completion behavior.
- Completion sorting/priority beyond what TypeScript-Go provides natively.
