---
"@effect/tsgo": minor
---

Add the `suspendFailToFailSync` diagnostic, which suggests `Effect.failSync` instead of an `Effect.suspend` whose thunk only returns `Effect.fail`. On Effect v3 it also suggests `Effect.dieSync` for a thunk that only returns `Effect.die`; Effect v4 has no `Effect.dieSync`. Both forms build the error lazily, so the quick fix keeps the behavior.

```ts
// before
Effect.suspend(() => Effect.fail(new ValidationError({ input })))
// after
Effect.failSync(() => new ValidationError({ input }))
```

It works on piping flows, so the suspend and the fail can each be a call or a pipe step, and the quick fix keeps the shape:

```ts
// before
pipe(() => pipe(error, Effect.fail), Effect.suspend)
// after
pipe(() => error, Effect.failSync)
```

The rule does not report thunks that do other work, such as a conditional or a statement before the return. When the rewrite would narrow the inferred error type, as with `Effect.fail(cond ? "A" : "B")`, it reports without a fix.
