---
"@effect/tsgo": minor
---

Add `mapMapErrorToMapBoth` style diagnostic (default `off`, Effect v3 and v4) with a quickfix. Adjacent `Effect.map` + `Effect.mapError` steps (either order) anywhere in one piping flow are reported: in a `pipe`/`.pipe`/`Effect.fn` pipeline, across chained `.pipe(...).pipe(...)` calls, and in nested data-first, curried or mixed data-first/pipeable calls. The quickfix merges them into `Effect.mapBoth` and removes the other step, collapsing a `.pipe(...)` left without steps to its subject; on v4, `Effect.mapEager` + `Effect.mapErrorEager` suggests `Effect.mapBothEager`. The quickfix is withheld when it cannot keep behavior: a step callee that is not a plain or dotted name (such as `getEffect().map`), explicit type arguments on any call between and including the two steps (a curried application, a containing or empty `pipe`/`.pipe`), no binding in scope that reaches `mapBoth`, a rewrite that would newly start an expression statement with `(` or another token that continues the previous line, or a pair split across calls where merging would move a callback that is not a function literal, or the work of an eager first step, across other code.

```ts
// before
fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.mapError((message) => new TaskError({ message }))
)
// also reported: fetchCount.pipe(Effect.map((n) => n > 0)).pipe(Effect.mapError(...))
// after
fetchCount.pipe(
  Effect.mapBoth({
    onSuccess: (n) => n > 0,
    onFailure: (message) => new TaskError({ message })
  })
)
```
