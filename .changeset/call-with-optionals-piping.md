---
"@effect/tsgo": minor
---

Piping flows now treat calls that pass optional arguments as transformation steps. A call counts when its callee has a single call signature with exactly one required parameter and no rest parameter: the first argument is the subject and the remaining optional arguments are kept with the step. Effect runners with options, `Effect.all` with options, and decoders applied with parse options no longer break a flow.

```ts
Effect.runPromise(Effect.exit(program), { signal })
// before: the flow ended at Effect.exit(program)
// after:  program -> Effect.exit -> Effect.runPromise({ signal })
```

Rules built on piping flows pick this up:

- `missedPipeableOpportunity` keeps a runner and its options outside the suggested pipe, and still reports the inner calls when the runner returns a function, as `Effect.runCallback` does.

  ```ts
  Effect.runPromise(Effect.asVoid(Effect.orDie(program)), { signal })
  // quick fix: Effect.runPromise(program.pipe(Effect.orDie, Effect.asVoid), { signal })
  ```

- `raceFirstWithSleepToTimeout` no longer reports `Effect.raceAllFirst` called with options such as `onWinner`, which `Effect.timeoutOrElse` cannot carry over.
- `runOfExitToRunExit`, `preferTypedSchemaDecoder`, `allOfMapToForEach` and `strictEffectProvide` now read piping flows instead of walking the whole file. Their results are unchanged, except that `strictEffectProvide` no longer reports a standalone `Effect.provide(layer)` that is not applied to an Effect, such as `const withLive = Effect.provide(AppLive)`.
