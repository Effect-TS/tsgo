---
"@effect/tsgo": minor
---

Add the `runProvidedContextToRunWith` style diagnostic (Effect v4, default severity suggestion). It reports an effect whose outermost step is `Effect.provideContext` or `Effect.setContext` being run with `Effect.runPromise`, `runPromiseExit`, `runSync`, `runSyncExit`, `runFork` or `runCallback` (called directly, as a pipe step or as an `Effect.fn` step), and offers a quick fix to the dedicated `Effect.run*With` runner. Run options are carried over.

```ts
// before
Effect.runPromise(Effect.provideContext(program, services), { signal })
program.pipe(Effect.provideContext(services), Effect.runPromise)
Effect.fn(function*() { return yield* program }, Effect.provideContext(services), Effect.runSync)

// after
Effect.runPromiseWith(services)(program, { signal })
program.pipe(Effect.runPromiseWith(services))
Effect.fn(function*() { return yield* program }, Effect.runSyncWith(services))
```
