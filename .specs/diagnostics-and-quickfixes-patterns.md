# Diagnostics and Quick Fixes Patterns

## Goal

Document the established conventions and patterns for implementing diagnostic rules and quick fixes (fixables) in the Effect Language Service, so that new rules and fixables follow a consistent structure.

## Diagnostic Rules

### Rule Structure

Each diagnostic rule is defined as a `rule.Rule` value in a dedicated file under `internal/rules/`. A rule has:

- **Name**: a camelCase identifier used in tsconfig and directives (e.g. `"missingEffectError"`).
- **Description**: a human-readable summary.
- **DefaultSeverity**: one of `SeverityError`, `SeverityWarning`, `SeveritySuggestion`, or `SeverityOff`. Plain value, not a pointer. `SeverityOff` means the rule is disabled unless explicitly enabled by user configuration or directives.
- **Codes**: the diagnostic codes this rule emits (from `tsdiag.<MessageIdentifier>.Code()`).
- **Run**: `func(ctx *rule.Context) []*ast.Diagnostic` — the entry point. Must NOT call `c.AddDiagnostic`; it only returns diagnostics.

### Diagnostic Messages

New diagnostic messages are defined in `internal/diagnostics/effectDiagnosticMessages.json`. Each entry specifies a human-readable message key, a category (`"Error"`, `"Warning"`, `"Suggestion"`), and a numeric code in the `377000–377999` range. After editing this file, `pnpm setup-repo` regenerates the Go constants in the `shim/diagnostics` package.

### Rule Registration

All rules are listed explicitly in the `All` slice in `internal/rules/rules.go`. No `init()` magic or auto-discovery.

### The `Analyze*` Function Pattern

Every rule that has a corresponding quick fix must expose two things:

1. An exported **`Analyze<RuleName>`** function with signature:
   `func Analyze<RuleName>(c *checker.Checker, sf *ast.SourceFile) []<RuleName>Match`

2. An exported **`<RuleName>Match`** struct containing only data — no diagnostics.

This separation allows both the rule and the fixable to share the same detection logic.

#### Match Struct Conventions

- Must contain a `Location core.TextRange` field, pre-computed via `scanner.GetErrorRangeForNode(sf, node)` at detection time.
- Must contain whatever AST nodes or computed data the quick fix needs (e.g. the call node, an argument node, a parsed string value).
- Must NOT contain a `*ast.Diagnostic` field. Diagnostics are created only in the rule's `Run` function, not in the analyze function.

#### Analyze Function Conventions

- Must NOT create diagnostics. Returns match data only.
- Receives `*checker.Checker` and `*ast.SourceFile` as arguments (not `*rule.Context`).
- Computes `Location` via `scanner.GetErrorRangeForNode(sf, node)` for each match.
- Must treat AST shape checks as untrusted input: before any `As*` cast, the corresponding `Is*` predicate (or equivalent guard) must succeed.
- Must never panic on malformed, partial, or unexpected syntax trees. If a required node shape is missing, the analyzer must skip that candidate and continue traversal.

### Analyzer Robustness Contract

Diagnostic analyzers run inside checker hooks and must not crash compilation/type-check execution. Rule behavior is fail-closed:

- If a node does not match the expected shape, no match is emitted for that node.
- If any intermediate extraction step fails, the analyzer abandons only that candidate path and continues.
- Rule-specific diagnostics may be missed in edge cases, but checker stability and completion are prioritized over emitting a diagnostic.

#### Rule Run Function Convention

The `Run` function delegates to the `Analyze*` function, then maps each match to a diagnostic:

```
matches := AnalyzeMyRule(ctx.Checker, ctx.SourceFile)
diags := make([]*ast.Diagnostic, len(matches))
for i, m := range matches {
    diags[i] = ctx.NewDiagnostic(m.Location, tsdiag.<Message>, nil, m.OptionalArgs...)
}
return diags
```

### AST Traversal Patterns

Two established patterns for walking the AST inside `Analyze*` functions:

1. **Recursive walk** — for simpler traversals: define a local `var walk func(n *ast.Node)`, recurse via `n.IterChildren()`.
2. **Stack-based traversal** — for iterative cases: use a `nodeToVisit` slice, pop from the end, push children.

### Plugin Options Access

Rules that depend on plugin options access them via `c.Program().Options().Effect` inside the `Analyze*` function, with an early return if the config is nil.

### Version Branching

Rules that differ between Effect V3 and V4 branch on `typeparser.SupportedEffectVersion(c)`, which returns `typeparser.EffectMajorV3` or `typeparser.EffectMajorV4` (unknown defaults to V3).

## Quick Fixes (Fixables)

### Fixable Structure

Each quick fix is defined as a `fixable.Fixable` value in a dedicated file under `internal/fixables/`. A fixable has:

- **Name**: a unique camelCase identifier (e.g. `"unnecessaryPipe"`).
- **Description**: a human-readable summary.
- **ErrorCodes**: diagnostic codes this fixable handles (from `tsdiag.<MessageIdentifier>.Code()`).
- **FixIDs**: "fix all" identifiers following the convention `<ruleName>_<actionVariant>` (e.g. `"missingEffectError_catch"`, `"missingEffectError_catchAll"`).
- **Run**: `func(ctx *fixable.Context) []ls.CodeAction` — returns code actions (empty/nil = no fixes applicable).

### Fixable Registration

All fixables are listed explicitly in the `All` slice in `internal/fixables/fixables.go`. No `init()` magic or auto-discovery.

### Fixable Run Function Convention

Every fixable's `Run` function follows this sequence:

1. Obtain the type checker: `c, done := ctx.GetTypeCheckerForFile(ctx.SourceFile)` with a nil-check and `defer done()`.
2. Call the corresponding `Analyze*` function from `internal/rules/`.
3. Iterate matches and filter by span intersection using both checks:
   `!match.Location.Intersects(ctx.Span) && !ctx.Span.ContainedBy(match.Location)`
   Both checks are needed because the cursor position from the IDE can land anywhere within the diagnostic range.
4. For the matching diagnostic, call `ctx.NewFixAction(...)` to produce code actions.
5. Nil-check the result of `NewFixAction` before appending.

### NewFixAction

`NewFixAction` is the primary API for producing code actions. It receives a `FixAction` with:

- `Description`: the user-visible code action name.
- `Run`: a closure that receives a fresh `*change.Tracker` and performs edits on it.

Returns `nil` if the tracker produced no edits. Callers must nil-check before using the result.

### Tracker Operations

The tracker (received inside the `NewFixAction` closure) provides node-level edit operations:

- **`ReplaceNode(sf, oldNode, newNode, options)`** — replace an existing AST node with a synthesized node.
- **`Delete(sf, node)`** — delete a node (handles list context automatically).
- **`DeleteRange(sf, textRange)`** — delete a raw byte range.
- **`InsertNodeAt(sf, pos, node, options)`** — insert a synthesized node at a byte position. `NodeOptions.Prefix` / `NodeOptions.Suffix` add surrounding text.
- **`InsertText(sf, lspPosition, text)`** — insert raw text at an LSP position (use `ctx.BytePosToLSPPosition(pos)` to convert).
- **`InsertNodeAfter(sf, afterNode, newNode)`** — insert a node immediately after an existing node.
- **`InsertNodeBefore(sf, beforeNode, newNode, blankLine)`** — insert a node before an existing node.
- **`InsertAtTopOfFile(sf, stmts, blankLineBetween)`** — add statements at the top of the file.

### Node Factory (via Tracker)

Because `Tracker` embeds `*ast.NodeFactory`, all node construction methods are callable directly on the tracker: `tracker.NewIdentifier(...)`, `tracker.NewCallExpression(...)`, `tracker.NewPropertyAccessExpression(...)`, `tracker.NewNodeList(...)`, `tracker.NewStringLiteral(...)`, `tracker.DeepCloneNode(...)`, etc.

### Critical Rule: Synthesized Nodes

`ReplaceNode` only accepts fully synthesized nodes as the replacement. The printer panics on non-synthesized (parsed) nodes. When replacing with a copy of an existing parsed node, always use `tracker.DeepCloneNode(node)` first.

After building complex replacement trees, call `ast.SetParentInChildren(replacementNode)` before passing to `ReplaceNode`.

## Edit Patterns

### Pattern 1: Replace with Synthesized Nodes

Use when the replacement is a new or different construct. Build new nodes via the tracker's factory methods and pass to `tracker.ReplaceNode`.

### Pattern 2: Minimal Deletion (Unwrapping)

Use when stripping a wrapper and keeping a child node. Delete the prefix and suffix around the child using `tracker.DeleteRange`. This preserves original text exactly (comments, formatting, trivia) and avoids cloning.

### Pattern 3: Text Insertion

Use when inserting raw text around existing nodes (e.g. wrapping an expression). Use `tracker.InsertText` with `ctx.BytePosToLSPPosition(pos)`.

### Pattern 4: Deep Clone + New AST Construction

Use for complex rewrites that build a fully new AST tree from parts of the existing tree. Deep-clone parsed nodes, construct new nodes via the tracker factory, call `ast.SetParentInChildren(...)`, then use `tracker.InsertNodeAt` or `tracker.ReplaceNode`.

## The Disable Fixable

A special fixable (`EffectDisable`) handles ALL Effect diagnostic codes. It provides two fix actions per diagnostic: "Disable `<ruleName>` for this line" and "Disable `<ruleName>` for entire file". It uses `tracker.InsertText` to insert directive comments and does not call any `Analyze*` function.

## Test VFS Setup

### Mounted Packages

`MountEffect` populates the test VFS with the packages needed to compile Effect tests. It must mount:

- `effect`, `pure-rand`, `@standard-schema/spec`, `fast-check` — the Effect runtime and its dependencies.
- `@types/node` — Node.js type definitions, so that tests importing Node built-in modules resolve without spurious TS2591 errors.

All packages are read from `testdata/tests/{version}/node_modules/` and cached per (version, package) pair.

### Tests That Import Node Built-ins

Tests that import Node.js built-in modules (e.g. `nodeBuiltinImport`) must include a `@filename tsconfig.json` section with `"types": ["node"]` in `compilerOptions`, so that the compiler picks up the mounted `@types/node` definitions.

## Testing

Quick-fix tests live in `testdata/tests/effect-v4/` and `testdata/tests/effect-v3/`. Each `.ts` test file generates a `*.quickfixes.txt` baseline alongside `*.errors.txt`. V3 test files must begin with `// @effect-v3`. V4 files have no marker.

Legacy snapshot metadata migration rule: old LSP snapshot directives using `@test-config` must be converted to fourslash-style `@filename tsconfig.json` inputs in the Go test corpus/spec parity migrations.

Disable-style fixes are listed in the inventory but recorded as `skipped by default` in the application results.

Refactor tests live in `testdata/tests/effect-v4-refactors/`. Each `.ts` test file generates a `*.refactors.txt` baseline. Selection ranges are specified via a `// refactor: L:C-L:C,L:C-L:C` comment on the first line (1-based lines and columns, dash separates start-end of a range, comma separates multiple ranges, single `L:C` for point selections). The comment is NOT stripped from the source — it stays so line numbers remain correct. The runner also tests with an empty selection to verify refactors are not offered without a selection.
