# Completion Infrastructure

## Goal
Add custom Effect completions support to the Go language service, following the same abstraction pattern used by rules, fixables, and refactors.

## Status
Spec only — no backlog items yet. To be revisited.

## Background

### Current state
No custom completions exist in the Go version. The completion pipeline goes straight from TypeScript-Go's `ProvideCompletion()` (in `typescript-go/internal/ls/completions.go`) to the LSP client with no Effect customization point. No patches in `_patches/` touch completions.

### Reference implementation
The reference (`effect-language-service`) has 14 custom completions that hook into TypeScript's `getCompletionsAtPosition` via a proxy, run each completion definition's `apply()` function, and merge results with native completions.

Key reference files:
- Registry: `.repos/effect-language-service/packages/language-service/src/completions.ts`
- Individual completions: `.repos/effect-language-service/packages/language-service/src/completions/*.ts` (14 files)
- Core interface: `.repos/effect-language-service/packages/language-service/src/core/LSP.ts` (lines 109-139)
- Auto-import middleware: `.repos/effect-language-service/packages/language-service/src/completions/middlewareAutoImports.ts`
- Tests: `.repos/effect-language-service/packages/language-service/test/completions.test.ts`

Reference completions include: gen(function*(){}), fn(function(){}), Duration input units, Schema brand, Effect.Service self-completions, Context.Tag, RPC Make, Data classes, JSDoc directives (`@effect-diagnostics`, `@effect-codegens`, `@effect-identifier`), and more.

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

## Proposed Design

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
- This allows both appending custom entries and post-processing existing entries (e.g., auto-import middleware)

This differs from the "provider" pattern (used by fixables/refactors) because completions need access to the existing completion list to avoid duplicates and to post-process entries.

### Injection points in TypeScript-Go
1. **`typescript-go/internal/ls/completions.go`** — Add `AfterCompletionCallback` variable, call it at the end of `ProvideCompletion()` before returning
2. **`shim/ls/shim.go`** — Expose `RegisterAfterCompletionCallback` via `//go:linkname`
3. **A new patch in `_patches/`** — To add the callback to the TypeScript-Go submodule (following the pattern of `012-ls-hover.patch` and `015-ls-inlay-hints.patch`)

## Open Questions
- Exact shape of `CompletionEntry` return type — should it mirror `lsproto.CompletionItem` directly or use an Effect-specific intermediate type?
- Whether the context should expose the full existing completion list or just provide a way to check for duplicates.
- Which of the 14 reference completions to port first, and whether any are not applicable to the Go version.
- Whether auto-import post-processing middleware needs its own abstraction or can be handled within individual completions.
