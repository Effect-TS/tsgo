---
"@effect/tsgo": minor
---

Add the `zipViaNestedFlatMapToAll` diagnostic (style, suggestion by default, Effect v3 and v4). It reports an `Effect.flatMap` whose callback only returns `Effect.map` over an independent effect, pairing the two results into a tuple or a two-key record, and offers a quick fix to `Effect.all`, which runs the effects sequentially by default.

```ts
// before
Effect.flatMap(getUser, (user) => Effect.map(getOrders, (orders) => [user, orders] as const))
fetchCount.pipe(Effect.flatMap((result) => Clock.currentTimeMillis.pipe(Effect.map((at) => ({ result, at })))))

// after
Effect.all([getUser, getOrders])
Effect.all({ result: fetchCount, at: Clock.currentTimeMillis })
```

The rule finds the `flatMap` as a step of any piping flow (data-first, data-last, `x.pipe(...)`, `pipe(x, ...)`, curried and `Effect.fn` steps) and the inner `map` as the last step of the flow the callback returns. The fix replaces the flow up to the `flatMap` and keeps the steps after it:

```ts
// before
getUser.pipe(
  Effect.tap(audit),
  Effect.flatMap((user) => Effect.map(getOrders, (orders) => [user, orders] as const)),
  Effect.asVoid
)

// after
Effect.all([getUser.pipe(Effect.tap(audit)), getOrders]).pipe(Effect.asVoid)
```

The rule does not report when the inner effect references the outer value, reads a `let`/`var` binding or a reassigned parameter eagerly, or uses a binding declared later. Both callbacks must be arrow functions with a single plain parameter, and the combiner must list the outer value first. The fix is not offered for an `Effect.fn` pipeline step or when either step has explicit type arguments, or when an operand is a `pipe(...)`/`.pipe(...)` call cut before the step and that call uses `?.` or explicit type arguments.
