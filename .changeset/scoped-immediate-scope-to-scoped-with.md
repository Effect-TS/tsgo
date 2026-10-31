---
"@effect/tsgo": minor
---

Add the `scopedImmediateScopeToScopedWith` diagnostic, which suggests `Effect.scopedWith` when `Effect.scoped` wraps a body that immediately reads the scope back with `Effect.scope`. It is not reported when other code in the body still needs the ambient `Scope` (for example `Effect.acquireRelease` or `Effect.addFinalizer`), because `Effect.scopedWith` does not provide one.

```ts
// before
Effect.gen(function*() {
  const scope = yield* Effect.scope
  return yield* openConnection(scope)
}).pipe(Effect.scoped)
// after
Effect.scopedWith((scope) => Effect.gen(function*() {
  return yield* openConnection(scope)
}))

// before
Effect.scoped(Effect.flatMap(Effect.scope, openConnection))
// after
Effect.scopedWith(openConnection)
```
