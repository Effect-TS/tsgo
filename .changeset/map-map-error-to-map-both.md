---
"@effect/tsgo": minor
---

Add `mapMapErrorToMapBoth` style diagnostic (default `suggestion`, Effect v3 and v4) with a quickfix. Adjacent `Effect.map` + `Effect.mapError` steps (either order) anywhere in one piping flow are reported: in a `pipe`/`.pipe`/`Effect.fn` pipeline, across chained `.pipe(...).pipe(...)` calls, and in nested data-first, curried or mixed data-first/pipeable calls. The quickfix merges them into `Effect.mapBoth` and removes the other step, collapsing a `.pipe(...)` left without steps to its subject; on v4, `Effect.mapEager` + `Effect.mapErrorEager` suggests `Effect.mapBothEager`. The quickfix is withheld when it cannot keep behavior: explicit type arguments on any call it edits (including a curried application or the containing `pipe`), no binding in scope that reaches `mapBoth`, a curried or mixed shape where merging would reorder the evaluation of a callback that is not a function literal, or an eager pair split across an evaluation point whose second callback is not a function literal.

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
