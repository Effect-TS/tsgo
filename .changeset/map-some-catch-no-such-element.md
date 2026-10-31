---
"@effect/tsgo": minor
---

Add the `mapSomeCatchNoSuchElement` diagnostic and quick fix for Effect v4. It suggests `Effect.catchNoSuchElement` when a piping flow wraps successes with `Effect.asSome` / `Effect.map(Option.some)` and immediately catches `NoSuchElementError` into `Option.none()`. It only reports when every error that could carry the `"NoSuchElementError"` tag is the real `Cause.NoSuchElementError`, and the handler ignores the error.

```ts
// before
findUser(id).pipe(
  Effect.map(Option.some),
  Effect.catchTag("NoSuchElementError", () => Effect.succeed(Option.none()))
)
// after
findUser(id).pipe(Effect.catchNoSuchElement)
```
