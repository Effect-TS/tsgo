---
"@effect/tsgo": patch
---

`strictEffectProvide` no longer reports `Effect.provide(layer)` when the provided Effect goes straight into a runner: `Effect.runSync`, `runSyncExit`, `runPromise`, `runPromiseExit`, `runFork`, `runCallback`, or the platform `NodeRuntime.runMain`, `BunRuntime.runMain` and `BrowserRuntime.runMain`. That is where an application entrypoint provides its layers. "Straight into" means the `provide` call, or the `pipe` that ends with it, is the runner's argument, or the runner is the next step of the same `pipe`.

```ts
// not reported
Effect.runPromise(program.pipe(Effect.provide(AppLayer)))
NodeRuntime.runMain(Effect.provide(program, AppLayer))
program.pipe(Effect.provide(AppLayer), NodeRuntime.runMain)

// still reported
const handler = program.pipe(Effect.provide(AppLayer))
```
