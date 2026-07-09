# Fixable Context and Tracker Ownership

## Goal
Make code-fix implementations self-contained by giving each fixable a context object that can create and finalize its own change tracker, instead of receiving an externally managed tracker argument.

## Scope
- Applies to Effect fixables in `internal/fixables/`.
- Applies to the fixable contract in `internal/fixable/`.
- Applies to the Effect language-service code-fix provider wiring in `etslshooks`.
- Applies to shim exposure needed to construct change trackers from code-fix context data.

## Requirements
1. The fixable run contract must no longer accept a `*change.Tracker` argument.
2. A dedicated fixable context type must be introduced and passed to fixables.
3. The fixable context must expose core data fields:
   - `SourceFile`
   - `Span`
   - `ErrorCode`
4. The fixable context must expose a `GetTypeCheckerForFile(sourceFile)` helper for analyzer-based fixables. It returns the checker and cleanup handle so fixables do not need direct `Program` access.
5. The fixable context must expose a `NodeFactory` for AST node construction in fixables.
6. The fixable context must expose `NewFixAction`, the primary API for producing code actions (see below).
7. Each fixable must create and manage its own tracker lifecycle for edit generation via `NewFixAction`.
8. Fixables must build code-fix edits through tracker operations and context helpers rather than manually constructing text spans and `TextEdit` ranges.
9. The code-fix provider must not allocate and pass a shared tracker instance to all fixables.
10. Required shim exposure for language-service converters must be explicit and generated through the shim workflow (no direct `typescript-go/internal/...` imports in Effect code).
11. Existing quick-fix titles and user-visible semantics must remain unchanged.

## NewFixAction

`NewFixAction` is the primary API for fixables to produce code actions. It encapsulates tracker creation, edit generation, and CodeAction wrapping in a single call.

### Signature

```go
func (ctx *Context) NewFixAction(action FixAction) *ls.CodeAction
```

### FixAction type

```go
type FixAction struct {
    Description string
    Run         func(tracker *change.Tracker)
}
```

- `Description`: the user-visible code action name
- `Run`: a closure that receives a fresh tracker and performs edits on it

### Return value

Returns `*ls.CodeAction`. Returns `nil` if the tracker produced no edits after `Run` completes. Callers must nil-check before using the result.

### Behavior

1. Creates a new tracker (using the current code-fix request context: compiler options, format options, converters)
2. Calls `Run(tracker)` — the closure performs edits on the tracker
3. Finalizes the tracker and extracts changes for the current `SourceFile`
4. If no changes were produced, returns `nil`
5. Otherwise, returns a `*ls.CodeAction` with the given `Description` and the computed changes

### Usage

```go
if action := ctx.NewFixAction(fixable.FixAction{
    Description: "Rewrite using the static 'new' pattern",
    Run: func(tracker *change.Tracker) {
        tracker.ReplaceNode(sf, match.ConstructorNode, staticMethod)
    },
}); action != nil {
    actions = append(actions, *action)
}
```

### Supersedes

`NewFixAction` supersedes the separate `NewTracker()` and `GetChangesForSourceFile(tracker)` methods. Once all fixables have been migrated to `NewFixAction`, those methods must be removed from the fixable context.

## Edit Patterns

Fixables should use tracker operations rather than manual string construction. There are two primary edit patterns depending on the scenario:

### Pattern 1: Replace with synthesized nodes

Use when the replacement is a new or different construct (e.g., replacing an identifier, building a new call expression).

Construct new nodes via `ctx.NodeFactory` and pass them to `tracker.ReplaceNode`.

**Important**: `tracker.ReplaceNode` only accepts fully synthesized nodes as the replacement. The printer panics on non-synthesized nodes from the parsed source tree. When replacing a node with a copy of an existing non-synthesized node, use `tracker.DeepCloneNode(node)` to create a synthesized copy first.

```go
// New node — fully synthesized
tracker.ReplaceNode(sf, match.MethodIdentifier, ctx.NodeFactory.NewIdentifier("scoped"), nil)

// Copy of existing node — must deep-clone to synthesize
tracker.ReplaceNode(sf, parentNode, tracker.DeepCloneNode(childNode), nil)
```

### Pattern 2: Minimal deletion (unwrapping)

Use when the goal is to strip a wrapper and keep an existing child node in place (e.g., `Effect.fail(arg)` → `arg`, `pipe(x)` → `x`).

Instead of replacing the whole parent with a cloned child, delete the prefix and suffix around the child using `tracker.DeleteRange`. This produces the smallest possible diff, preserves the original text exactly (formatting, comments, trivia), and avoids cloning entirely.

```go
// Effect.fail(arg) → arg: delete "Effect.fail(" before arg and ")" after arg
tracker.DeleteRange(sf, core.NewTextRange(failCall.Pos(), arg.Pos()))
tracker.DeleteRange(sf, core.NewTextRange(arg.End(), failCall.End()))
```

Prefer this pattern over `DeepCloneNode` + `ReplaceNode` when the child's original text should be preserved exactly.

## Non-Goals
- Redesigning language-service protocol types for multi-file workspace edits.
- Forcing every fixable to use AST node replacement if plain text edits are still appropriate.
- Changing diagnostic detection logic or rule semantics.

## Acceptance Criteria
1. All fixables compile against the new fixable context contract without a tracker argument in the run signature.
2. All fixables use `NewFixAction` to produce code actions.
3. `NewTracker()`, `GetChangesForSourceFile()`, and `ByteRangeToLSPRange()` are removed from the fixable context after migration.
4. Fixables emit edits via tracker operations (not manual range/text-edit assembly) and return edits scoped to the source file for the current code-fix request.
5. User-visible quick-fix behavior remains equivalent, and repository validation workflow passes.
