---
"@effect/tsgo": patch
---

`strictEffectProvide` no longer reports `Effect.provide(layer)` when the next step of the piping flow runs the Effect, as an application entry point does. The recognized runners are `Effect.runPromise`, `runSync`, `runFork`, `runPromiseExit`, `runSyncExit`, `runCallback`, and `runMain` from `@effect/platform-node`, `@effect/platform-bun` and `@effect/platform-browser`.

```ts
// not reported
program.pipe(Effect.provide(AppLayer), Effect.runPromise)
program.pipe(Effect.provide(AppLayer), NodeRuntime.runMain)
Effect.runPromise(program.pipe(Effect.provide(AppLayer)))
NodeRuntime.runMain(Effect.provide(program, AppLayer))

// still reported
export const handler = program.pipe(Effect.provide(AppLayer))
Effect.runPromise(program.pipe(Effect.provide(AppLayer), Effect.asVoid))
```
