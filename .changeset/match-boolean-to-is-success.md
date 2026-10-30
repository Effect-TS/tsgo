---
"@effect/tsgo": minor
---

Add `matchBooleanToIsSuccess`, a style suggestion for Effect v3 and v4. It reports `Effect.match` and `Effect.matchEager` calls whose handlers ignore their input and fold to complementary boolean literals, and fixes them to `Effect.isSuccess` / `Effect.isFailure`:

```ts
Effect.match(saveUser, { onFailure: () => false, onSuccess: () => true })
saveUser.pipe(Effect.match({ onFailure: () => true, onSuccess: () => false }))
// become
Effect.isSuccess(saveUser)
saveUser.pipe(Effect.isFailure)
```

Calls nested inside other calls are reported too, such as `Effect.andThen(Effect.void, Effect.match(saveUser, { ... }))`.

The fix keeps the qualifier the call is written with (`Fx.match` becomes `Fx.isSuccess`, and a named `match` import gets a named `isSuccess` import next to it).

`Effect.matchCause` / `matchCauseEager` and `Effect.map(Effect.exit(self), Exit.isSuccess)` are intentionally not reported: they turn defects and interruptions into booleans too, while `Effect.isSuccess` / `Effect.isFailure` let them propagate.
