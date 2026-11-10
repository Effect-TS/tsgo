---
"@effect/tsgo": minor
---

Add `onExitFailureToOnError` style suggestion: it reports an `Effect.onExit` callback that only acts on failure (`Exit.isFailure`, `Exit.isSuccess`, `exit._tag` checks, in a conditional, `if` or `switch`, with `Effect.void` on the other side) and recommends `Effect.onError`, which expresses the same intent more directly.

```ts
program.pipe(
  Effect.onExit((exit) =>
    Exit.isFailure(exit) ? Effect.logError("failed", { cause: exit.cause }) : Effect.void
  )
)
// can be written as
program.pipe(Effect.onError((cause) => Effect.logError("failed", { cause })))
```
