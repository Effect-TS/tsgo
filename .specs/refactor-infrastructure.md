# Refactor Infrastructure

## Goal

Provide a refactor framework mirroring the existing fixable infrastructure (`internal/fixable/` and `internal/fixables/`), but for selection-based refactoring actions that are not tied to a specific diagnostic.

## Background

The fixable infrastructure handles code fixes triggered by diagnostics: a fixable declares the error codes it handles, and the LS dispatches to it when a code action is requested for a matching diagnostic. Refactors are different — they are triggered by any text selection, regardless of whether a diagnostic is present. In the LSP protocol, both flow through `textDocument/codeAction` but differ in their `CodeActionKind` (`quickfix` vs `refactor.rewrite.*`).

## Scope

- A new `internal/refactor/` package providing the refactor context and types.
- A new `internal/refactors/` package providing the registry of all refactors.
- A new `RegisterRefactorProvider` hook in typescript-go (patch + shim).
- Wiring in `etslshooks`.

## Requirements

### Registration Hook (typescript-go patch)

1. A `RefactorProvider` type must be added to the language service, analogous to `CodeFixProvider`:
   - It must expose a `GetRefactorActions` function that receives a context, source file, and text span, and returns refactor code actions.
   - It must not require an error code or diagnostic — refactors apply to any selection.
2. A `RegisterRefactorProvider` function must be added, appending to an `externalRefactorProviders` slice (analogous to `externalCodeFixProviders`).
3. The `ProvideCodeActions` function must be extended with a code path that invokes refactor providers when the request's `CodeActionKind` filter includes `refactor.*` kinds, or when no filter is specified.
4. The server capability advertisement must include `refactor.rewrite` as a supported code action kind.
5. The shim must be regenerated to expose the new hook.

### Refactor Context (`internal/refactor/`)

6. A `Refactor` struct must be introduced (analogous to `fixable.Fixable`):
   - `Name string` — unique identifier for the refactor
   - `Description string` — user-visible label in the refactor menu
   - `Kind string` — the `CodeActionKind` suffix (e.g., `"rewrite.effect.wrapWithPipe"`)
   - `Run func(ctx *Context) []ls.CodeAction` — the refactor implementation

7. A `Context` struct must be introduced (analogous to `fixable.Context`), wrapping the refactor request data:
   - `SourceFile *ast.SourceFile` — the file being refactored
   - `Span core.TextRange` — the selected text range

8. The context must expose the same helper methods as the fixable context:
   - `GetTypeCheckerForFile(sf *ast.SourceFile) (*checker.Checker, func())` — returns a checker and cleanup handle
   - `NodeFactory` — for AST node construction
   - `NewRefactorAction(action RefactorAction) *ls.CodeAction` — analogous to `NewFixAction`
   - `FormatOptions() *lsutil.FormatCodeSettings`

9. `RefactorAction` must mirror `FixAction`:
   - `Description string` — user-visible code action name
   - `Run func(tracker *change.Tracker)` — closure that performs edits on a fresh tracker

10. `NewRefactorAction` must follow the same lifecycle as `NewFixAction`: create tracker, call `Run`, finalize, return `*ls.CodeAction` (or `nil` if no edits).

### Refactors Registry (`internal/refactors/`)

11. A `var All []refactor.Refactor` registry must be maintained, listing all registered refactors (analogous to `fixables.All`).
12. Each refactor must be declared in its own file within `internal/refactors/`.
13. Adding a new refactor requires: creating the file, declaring the `refactor.Refactor` var, and appending it to `All`.

### Wiring (`etslshooks`)

14. `etslshooks/init.go` must register a single `RefactorProvider` that dispatches to all refactors in the registry.
15. The dispatch logic iterates all refactors in `All`, calls each `Run(ctx)`, and concatenates the resulting code actions.
16. Unlike fixables (which dispatch by error code), refactors all run on every request — each refactor's `Run` function is responsible for checking applicability and returning an empty slice when not applicable.

### Applicability Pattern

17. Each refactor's `Run` function must check whether the refactor is applicable to the current selection. If not applicable, it returns `nil` (no actions). If applicable, it returns one or more code actions via `ctx.NewRefactorAction`.
18. Applicability checks are the responsibility of individual refactors — the framework does not pre-filter.

### Testing

19. Refactor tests live in `testdata/tests/effect-v4-refactors/`. Each `.ts` test file generates a `*.refactors.txt` baseline.
20. Selection ranges for refactor tests must be specified using a comment on the first line of the file. The comment is NOT stripped from the source — it remains part of the file so that line numbers in the range specifications stay correct.
21. The format follows the convention from the original language service: `// refactor: L:C-L:C,L:C-L:C` where each `L:C-L:C` is a range (startLine:startCol-endLine:endCol, 1-based lines and columns), and multiple ranges are separated by commas. A single `L:C` (without dash) denotes a point selection.
22. The test runner must parse the `// refactor:` comment to extract selection ranges but must NOT remove it from the source before processing.
23. A test file without a `// refactor:` comment must be tested with an empty selection (0:0–0:0), verifying that no refactors are offered.
24. The refactor baseline runner must also test with an empty selection (0:0–0:0) in addition to the marked selections, to verify that refactors are not offered when nothing is selected.
25. The baseline output format must match the established quick-fix baseline style: an inventory section listing available refactors per selection, followed by application-result sections showing the post-apply source.

## Non-Goals

- Multi-file workspace edits (refactors are scoped to a single source file, same as fixables).
- "Fix all" / "Refactor all" support (no equivalent of `FixIds` or `GetAllCodeActions` for refactors).
- Changing existing fixable infrastructure or diagnostic detection.

## Acceptance Criteria

1. A `RegisterRefactorProvider` hook exists in the shim and is callable from Effect code.
2. The `refactor.Context` provides the same capabilities as `fixable.Context` (checker access, node factory, tracker-based edit generation via `NewRefactorAction`).
3. Refactors registered in `internal/refactors/All` are invoked for `textDocument/codeAction` requests that include `refactor.*` kinds.
4. Refactor actions appear in the editor's refactor menu (not the quick-fix menu).
5. The framework supports adding new refactors by creating a file + appending to the registry, with no other wiring needed.
