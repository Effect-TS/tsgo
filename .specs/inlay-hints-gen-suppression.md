# Inlay Hints: Effect.gen Return Type Suppression

## Goal

Suppress redundant return-type inlay hints on `Effect.gen`, `Effect.fn` (gen variant), and `Effect.fn.untraced` (gen variant) generator functions. When TypeScript's built-in "show function return type inlay hints" feature is enabled, it displays a return type annotation after the closing parenthesis of these generator functions — but in the Effect context, this type is implicit and noisy. The inlay hint middleware filters these out.

## Background

TypeScript-Go already provides a complete inlay hints implementation. The Effect language service needs to post-process the inlay hint results — not to add new hints, but to **remove** specific ones that are unhelpful for Effect users.

The reference TypeScript language service implements this as a `middlewareGenLike` function that intercepts `provideInlayHints`, filters the result array, and returns the filtered list.

## Scope

- Adding a registration hook in typescript-go that allows Effect code to post-process inlay hint results before they are returned.
- Porting the `middlewareGenLike` suppression logic to Go.
- Exposing an `inlays` plugin option to control whether this middleware is active.

## Requirements

### Registration Hook

1. A new registration function (analogous to `RegisterAfterQuickInfoCallback`) must be added to the typescript-go language service, allowing an external callback to post-process inlay hint results.
2. The callback receives the source file, the requested text span, and the computed inlay hints array. It returns the (possibly filtered) inlay hints array.
3. The hook must be invoked after the built-in `provideInlayHints` logic completes, so the callback operates on the full set of computed hints.
4. The shim layer must be regenerated to expose the new hook.

### Suppression Logic

5. The middleware must only run when the user has enabled `includeInlayFunctionLikeReturnTypeHints` in their editor preferences. If that preference is not enabled, return the hints unchanged.
6. For each inlay hint of kind `Type`, find the AST node at `hint.position - 1`.
7. Check whether the node's parent is a call to `Effect.gen`, `Effect.fn` (gen variant), or `Effect.fn.untraced` (gen variant) — using the existing `TypeParser` patterns already available in the Go codebase.
8. If it is, and the hint position falls between the generator function's closing parenthesis and the function body start, suppress (omit) that hint.
9. All other hints pass through unchanged.

### Plugin Option

10. An `inlays` boolean plugin option must be added (default: `false`). When `false`, the middleware is not active and all inlay hints are returned as-is from the built-in provider.
11. When `inlays` is `true`, the middleware post-processes the results.

## Non-Goals

- Adding new inlay hints (this feature only removes unhelpful ones).
- Suppressing inlay hints for non-Effect generator patterns.
- Changes to TypeScript-Go's built-in inlay hint computation.

## Acceptance Criteria

1. With `inlays: true` and `includeInlayFunctionLikeReturnTypeHints: true`, hovering over code containing `Effect.gen(function*() { ... })` does **not** show a return-type inlay hint on the generator function.
2. The same applies to `Effect.fn` and `Effect.fn.untraced` gen variants.
3. Non-Effect generator functions still show their return-type inlay hints normally.
4. With `inlays: false` (or option not set), all inlay hints are returned unchanged regardless of Effect usage.
5. The `inlays` plugin option is documented in the README Plugin Options table.
