---
"@effect/tsgo": minor
---

Add the `matchToResultOrOption` diagnostic (style, suggestion). It reports `Effect.match` / `Effect.matchEffect` calls whose handlers only wrap the value and the error, and adjacent `Effect.map` + `Effect.catch` steps (`catchAll` in v3) that do the same. The quick fix replaces them with `Effect.result` or `Effect.option` (v4), or `Effect.either` or `Effect.option` (v3). These combinators also carry the error type through precisely, instead of producing a wider union type.

```ts
// reported, fix: Effect.result(fetchUser("u1"))
Effect.match(fetchUser("u1"), { onFailure: Result.fail, onSuccess: Result.succeed })
// reported, fix: fetchUser("u1").pipe(Effect.option)
fetchUser("u1").pipe(Effect.map(Option.some), Effect.catch(() => Effect.succeed(Option.none())))
// not reported: the handler transforms the error
Effect.match(fetchUser("u1"), { onFailure: (e) => Result.fail(e.id), onSuccess: Result.succeed })
```
