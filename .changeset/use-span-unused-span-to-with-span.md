---
"@effect/tsgo": minor
---

Add the `useSpanUnusedSpanToWithSpan` diagnostic (antipattern, suggestion by default, Effect v3 and v4). It reports an `Effect.useSpan` call whose inline callback never uses the span it receives: `useSpan` does not make that span the parent of spans created inside the callback, while `Effect.withSpan` does. A quick fix rewrites the call to `Effect.withSpan`, carrying the span options over.

```ts
// before
Effect.useSpan("fetch-user", (_span) => fetchUser(id))
Effect.useSpan("fetch-user", { attributes: { userId: id } }, () => fetchUser(id))
// after
fetchUser(id).pipe(Effect.withSpan("fetch-user"))
fetchUser(id).pipe(Effect.withSpan("fetch-user", { attributes: { userId: id } }))
```

The rule does not report when the span is referenced (even only in a nested closure or forwarded), when the parameter is destructured, has a default value or is a rest parameter, when a function-expression callback reads its own `arguments`, or when the callback is not an inline function. The fix is withheld when a block body is anything other than a single `return <expr>`, when `useSpan` is a bare named import or has explicit type arguments, when a function-expression callback uses its own `this` or name, or when the body eagerly reads a `let`/`var` binding, a reassigned parameter or a binding declared after the call.
