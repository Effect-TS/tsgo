---
"@effect/tsgo": minor
---

Add `effectSucceedWithUndefined` (Effect v4, style, suggestion): suggests `Effect.undefined` instead of `Effect.succeed(undefined)` or `Effect.succeed(void 0)`, keeping the `Effect<undefined>` type that the `Effect.void` rewrite would widen to `Effect<void>`.

`effectSucceedWithUndefined` and `effectSucceedWithVoid` now pick the replacement from the resolved success type instead of the spelling of the argument, so each quickfix keeps the type unchanged. Both also report the pipe form, and the quickfix keeps the later steps.

```ts
// Effect v4
Effect.succeed(undefined)                      // effectSucceedWithUndefined -> Effect.undefined
Effect.succeed(void 0)                         // effectSucceedWithUndefined -> Effect.undefined
pipe(undefined, Effect.succeed, Effect.as(1))  // effectSucceedWithUndefined -> pipe(Effect.undefined, Effect.as(1))
Effect.succeed<void>(undefined)                // effectSucceedWithVoid -> Effect.void
Effect.succeed<number | undefined>(undefined)  // not reported

// Effect v3 (no Effect.undefined)
Effect.succeed(undefined)                      // effectSucceedWithVoid -> Effect.void
Effect.succeed<number | undefined>(undefined)  // no longer reported
```

Piping flow analysis now starts a new flow at each value argument of a data-first call, such as the second argument of `Effect.andThen(eff, Effect.succeed(1).pipe(Effect.map(f)))`. Before, these arguments were split into a truncated head and fake flows for their pipe steps, so flow-based diagnostics missed patterns there.
