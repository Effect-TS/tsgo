---
"@effect/tsgo": minor
---

Add `mapMapErrorToMapBoth` style diagnostic (default `suggestion`, Effect v3 and v4) with a quickfix. Adjacent `Effect.map` + `Effect.mapError` steps (either order) in the same `pipe`/`.pipe`/`Effect.fn` pipeline, or directly nested data-first calls, are reported and can be merged into `Effect.mapBoth`; on v4, `Effect.mapEager` + `Effect.mapErrorEager` suggests `Effect.mapBothEager`.

```ts
// before
fetchCount.pipe(
  Effect.map((n) => n > 0),
  Effect.mapError((message) => new TaskError({ message }))
)
// after
fetchCount.pipe(
  Effect.mapBoth({
    onSuccess: (n) => n > 0,
    onFailure: (message) => new TaskError({ message })
  })
)
```
