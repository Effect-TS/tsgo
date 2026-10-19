---
"@effect/tsgo": patch
---

`runEffectInsideEffect` now reports `Effect.runSyncExit` and `Effect.runPromiseExit`, the same as `Effect.runSync` and `Effect.runPromise`. The quick fix rewrites them to `Effect.runSyncExitWith` and `Effect.runPromiseExitWith` in Effect v4, and to `Runtime.runSyncExit` and `Runtime.runPromiseExit` in Effect v3.

```ts
export const nested = Effect.gen(function*() {
  yield* Effect.void
  setTimeout(() => {
    Effect.runPromiseExit(child)
    // ^- `Effect.runPromiseExit` is called inside an Effect with a separate services invocation.
  }, 0)
})
```
