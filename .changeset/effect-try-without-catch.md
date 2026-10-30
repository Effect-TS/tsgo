---
"@effect/tsgo": minor
---

Add the `effectTryWithoutCatch` diagnostic (antipattern, off by default, Effect v3 and v4). It reports short-form `Effect.try(thunk)` / `Effect.tryPromise(thunk)` calls, whose failures surface as the generic `Cause.UnknownError` (v4) / `Cause.UnknownException` (v3), and offers a quick fix to the object form:

```ts
// before
Effect.try(() => JSON.parse(text))
// after the quick fix
Effect.try({ try: () => JSON.parse(text), catch: (cause) => cause })
```

Calls are found through piping flows, so the call can also be a data-first argument, a pipe subject, a step inside `Effect.gen` / `Effect.fn`, or (on Effect v3, where it compiles) a pipe step such as `pipe(thunk, Effect.try)`.

The placeholder `catch` returns `unknown`, so `unknownInEffectCatch` then nudges you towards a domain error.
