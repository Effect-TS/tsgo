---
"@effect/tsgo": minor
---

Add the `discardedRunPromiseToRunFork` diagnostic (correctness, warning, Effect v3 and v4). It reports a `runPromise` run whose Promise is discarded (bare statement, `void`, comma operand), where a failure or defect becomes an unhandled rejection, and offers a rename to the fire-and-forget Fiber runner:

```ts
// reported
void Effect.runPromise(program)
// fixed
void Effect.runFork(program)
```

Covers `Effect.runPromise`, `Effect.runPromiseWith(ctx)(…)` (v4), `Runtime.runPromise` curried and data-first (v3), hoisted runners and `ManagedRuntime#runPromise`. The rename is only offered on a receiver that owns both runners: the `Effect` or `Runtime` module itself, or a plain `ManagedRuntime<R, ER>`. Narrowed, mapped, intersected or generic receivers, bare-identifier callees and v3 calls with options keep the diagnostic without a fix.
