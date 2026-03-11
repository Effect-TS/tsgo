# Rule Context

## Goal

Reduce boilerplate in diagnostic rule implementations by introducing a `rule.Context` type that replaces the current `(c *checker.Checker, sf *ast.SourceFile)` signature with a single context parameter, and provides a `NewDiagnostic` helper that hides the verbose `ast.NewDiagnosticFromSerialized` call.

## Scope

- The `rule.Context` type in the `rule` package.
- The `Run` function signature on `rule.Rule`.
- All rule implementations in `internal/rules/`.
- The `Analyze*` functions and their match structs in `internal/rules/`.
- The fixables in `internal/fixables/` that consume `Analyze*` results.
- The call site in `etscheckerhooks/init.go` that invokes `rule.Run`.

## Requirements

### rule.Context type

1. `rule.Context` must contain:
   - `SourceFile` — the source file being checked.
   - `Checker` — the type checker instance.
   - A `NewDiagnostic` method for creating diagnostics.

2. `NewDiagnostic` must accept:
   - A location as `core.TextRange` (the pre-computed error range).
   - A diagnostic message (the `*diagnostics.Message` struct, e.g. `tsdiag.Effect_must_be_...`). This replaces the separate code, category, and key parameters.
   - Related diagnostics as `[]*ast.Diagnostic` (pass `nil` when none).
   - Message args as variadic `...string` (for `{0}`, `{1}` placeholders).

   The `rule.Context` must also provide a `GetErrorRange(node *ast.Node) core.TextRange` helper that wraps `scanner.GetErrorRangeForNode(ctx.SourceFile, node)`. Rules that don't use `Analyze*` functions can use this to compute a location from a node before passing it to `NewDiagnostic`.

3. `NewDiagnostic` must NOT accept a severity parameter. The severity used in the created diagnostic must come from the rule's `DefaultSeverity` field. This is the initial category embedded in the diagnostic; the existing `transformDiagnostics` pipeline continues to override it based on user config and directives.

4. `NewDiagnostic` must return an `*ast.Diagnostic` with the same structure as today's `ast.NewDiagnosticFromSerialized`, using `false` for `reportsUnnecessary`, `reportsDeprecated`, and `skippedOnNoEmit`.

### Rule.DefaultSeverity

5. The `DefaultSeverity` field on `rule.Rule` must change from `*etscore.Severity` (pointer, nil = error) to `etscore.Severity` (plain value). Every rule must explicitly set its default severity. The `EffectiveDefaultSeverity()` method and the `etscore.SeverityPtr` helper function must be removed since they are no longer needed.

### Rule.Run signature

6. The `Run` field on `rule.Rule` must change from `func(c *checker.Checker, sf *ast.SourceFile) []*ast.Diagnostic` to `func(ctx *rule.Context) []*ast.Diagnostic`.

### Call site updates

7. `etscheckerhooks/init.go` must construct a `rule.Context` for each rule invocation, passing the checker, source file, and the rule's default severity (so that `NewDiagnostic` can use it internally). References to `EffectiveDefaultSeverity()` must be replaced with direct access to `DefaultSeverity`.

### Rule migration

8. All existing rules in `internal/rules/` must be migrated to use `*rule.Context`:
   - Replace `c` with `ctx.Checker` and `sf` with `ctx.SourceFile`.
   - Replace `create*Diagnostic` helper functions with calls to `ctx.NewDiagnostic`.
   - Remove the now-unnecessary `create*Diagnostic` helper functions.
   - Remove imports that are no longer needed (`directives`, `etscore`, `scanner`) from rules that no longer reference them directly.

### Analyze functions and match structs

9. The `Analyze*` functions must NOT create diagnostics. They must return match structs that contain only the data needed for both diagnostic creation and code fixes — primarily AST nodes and parsed results.

10. Every match struct must replace its `Diagnostic *ast.Diagnostic` field with a `Location core.TextRange` field containing the pre-computed error range. The `Analyze*` function computes this via `scanner.GetErrorRangeForNode(sf, node)` at detection time.

11. The rule's `Run` function must create diagnostics from the match data using `ctx.NewDiagnostic(match.Location, ...)`.

12. Fixables in `internal/fixables/` must use `match.Location` directly for range comparison instead of extracting it from `match.Diagnostic.Loc()`.

## Non-Goals

- Changing how directives or severity overrides work (the `transformDiagnostics` pipeline stays as-is).
- Changing the diagnostic message definitions or the `diagnostics.Message` type.
- Moving the rules to a different package.
- Changing rule registration or the `rules.All` slice.

## Acceptance Criteria

1. All existing rules compile and pass tests after migration.
2. No rule directly calls `ast.NewDiagnosticFromSerialized` for its primary diagnostics — they all use `ctx.NewDiagnostic` instead.
3. No rule imports `directives` or `etscore` solely for diagnostic creation boilerplate.
4. The `Run` signature on `rule.Rule` is `func(ctx *rule.Context) []*ast.Diagnostic`.
5. No `Analyze*` function creates diagnostics or returns a `Diagnostic` field in its match struct.
6. All fixables use `match.Location` directly, not a diagnostic object.
7. The full validation workflow passes without errors.
